//! Turns raw audio (a guitar through an interface, or a mic) into note and chord events.
//! Single notes: YIN pitch detection. Strummed chords: a chromagram matched against
//! harmonic-aware templates, since YIN only ever hears one pitch.

use crate::midi::NoteEvent;
use crate::theory::ChordType;
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

/// Just under drop-D low D (73 Hz).
const F_MIN: f32 = 70.0;
/// Around the 24th fret of the high E string.
const F_MAX: f32 = 1400.0;
const YIN_THRESHOLD: f32 = 0.15;
/// RMS needed to start a note (about -48 dBFS), and the lower level that keeps it going.
const GATE_ON: f32 = 0.004;
const GATE_OFF: f32 = 0.002;
/// A jump in level this big while a note rings counts as plucking it again.
const ONSET_RATIO: f32 = 2.0;
/// Frames a new pitch must hold before it counts, and unpitched frames before a note ends.
const STABLE_FRAMES: u32 = 2;
const RELEASE_FRAMES: u32 = 3;
/// Run the chord matcher every this many frames.
const CHROMA_EVERY: u64 = 2;
const CHORD_MIN_SCORE: f32 = 0.85;
/// Every chord tone must reach this fraction of the loudest pitch class.
const CHORD_MIN_TONE: f32 = 0.2;
const CHORD_STABLE: u32 = 2;
/// Pitch-class offset of each harmonic of a note (2nd = octave, 3rd = fifth, 5th = major third...).
const HARMONICS: [u8; 8] = [0, 0, 7, 0, 4, 7, 10, 0];

#[derive(Clone, Debug, PartialEq)]
pub enum AudioEvent {
    Note(NoteEvent),
    /// Pitch classes of a chord that's ringing.
    Chord(BTreeSet<u8>),
}

fn to_midi(freq: f32) -> u8 {
    (69.0 + 12.0 * (freq / 440.0).log2()).round().clamp(0.0, 127.0) as u8
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
}

/// Fundamental frequency via YIN, or `None` if `x` isn't clearly pitched.
fn yin(x: &[f32], sr: f32) -> Option<f32> {
    let tau_min = (sr / F_MAX) as usize;
    let tau_max = ((sr / F_MIN) as usize).min(x.len() / 2);
    let w = x.len() - tau_max;
    let mut cmnd = vec![1f32; tau_max + 1];
    let mut sum = 0.0;
    for tau in 1..=tau_max {
        let d: f32 = x[..w].iter().zip(&x[tau..tau + w]).map(|(a, b)| (a - b) * (a - b)).sum();
        sum += d;
        cmnd[tau] = if sum > 0.0 { d * tau as f32 / sum } else { 1.0 };
    }
    let mut tau = (tau_min..tau_max).find(|&t| cmnd[t] < YIN_THRESHOLD)?;
    while tau < tau_max && cmnd[tau + 1] < cmnd[tau] {
        tau += 1;
    }
    let shift = if tau < tau_max {
        let (a, b, c) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
        let denom = a - 2.0 * b + c;
        if denom.abs() > 1e-9 { 0.5 * (a - c) / denom } else { 0.0 }
    } else {
        0.0
    };
    Some(sr / (tau as f32 + shift))
}

/// What a chord's pitch classes sound like once each note's overtones are added in.
struct Template {
    profile: [f32; 12],
    chord: BTreeSet<u8>,
}

fn template(pcs: BTreeSet<u8>) -> Template {
    let mut profile = [0f32; 12];
    for &pc in &pcs {
        for (h, off) in HARMONICS.iter().enumerate() {
            profile[((pc + off) % 12) as usize] += 1.0 / (h + 1) as f32;
        }
    }
    let norm = profile.iter().map(|v| v * v).sum::<f32>().sqrt();
    profile.iter_mut().for_each(|v| *v /= norm);
    Template { profile, chord: pcs }
}

fn templates() -> &'static [Template] {
    static T: OnceLock<Vec<Template>> = OnceLock::new();
    T.get_or_init(|| {
        (0..12u8)
            .flat_map(|root| ChordType::ALL.map(|c| template(c.intervals().iter().map(|i| (root + i) % 12).collect())))
            .collect()
    })
}

/// The chord (as pitch classes) that best explains `chroma`, if it's convincingly a chord.
/// Requiring every chord tone to be clearly audible is what keeps a lone note's overtones,
/// or a power chord, from passing for a triad.
fn recognize_chord(chroma: &[f32; 12]) -> Option<BTreeSet<u8>> {
    let max = chroma.iter().copied().fold(0.0, f32::max);
    let norm = chroma.iter().map(|v| v * v).sum::<f32>().sqrt();
    if max <= 0.0 {
        return None;
    }
    let score = |t: &Template| t.profile.iter().zip(chroma).map(|(a, b)| a * b).sum::<f32>() / norm;
    let best = templates().iter().max_by(|a, b| score(a).total_cmp(&score(b)))?;
    let pcs = &best.chord;
    let loud = pcs.iter().all(|&pc| chroma[pc as usize] >= CHORD_MIN_TONE * max);
    (score(best) >= CHORD_MIN_SCORE && loud).then(|| pcs.clone())
}

/// Streaming analyzer: feed it mono samples, it emits note on/off and chord events.
pub struct Analyzer {
    sr: f32,
    yin_len: usize,
    hop: usize,
    chroma_len: usize,
    /// The most recent `chroma_len` samples, plus whatever has arrived since the last frame.
    buf: Vec<f32>,
    pending: usize,
    frames: u64,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    current: Option<u8>,
    candidate: Option<(u8, u32)>,
    quiet: u32,
    prev_rms: f32,
    chord_candidate: Option<(BTreeSet<u8>, u32)>,
    last_chord: Option<BTreeSet<u8>>,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        // ~40 ms: room for two periods of the lowest note.
        let yin_len = ((sr * 0.04) as usize).next_power_of_two();
        let chroma_len = yin_len * 4;
        let window = (0..chroma_len)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / chroma_len as f32).cos())
            .collect();
        Self {
            sr,
            yin_len,
            hop: yin_len / 2,
            chroma_len,
            buf: Vec::with_capacity(chroma_len * 2),
            pending: 0,
            frames: 0,
            fft: FftPlanner::new().plan_fft_forward(chroma_len),
            window,
            current: None,
            candidate: None,
            quiet: 0,
            prev_rms: 0.0,
            chord_candidate: None,
            last_chord: None,
        }
    }

    pub fn push(&mut self, mut samples: &[f32], emit: &mut impl FnMut(AudioEvent)) {
        while !samples.is_empty() {
            let take = (self.hop - self.pending).min(samples.len());
            self.buf.extend_from_slice(&samples[..take]);
            samples = &samples[take..];
            self.pending += take;
            if self.pending == self.hop {
                self.pending = 0;
                let excess = self.buf.len().saturating_sub(self.chroma_len);
                self.buf.drain(..excess);
                self.frame(emit);
            }
        }
    }

    /// Ends any ringing note (e.g. when the input goes away).
    pub fn flush(&mut self, emit: &mut impl FnMut(AudioEvent)) {
        self.release(emit);
    }

    fn frame(&mut self, emit: &mut impl FnMut(AudioEvent)) {
        let n = self.buf.len();
        if n < self.yin_len {
            return;
        }
        let x = &self.buf[n - self.yin_len..];
        let level = rms(x);
        let gate = if self.current.is_some() { GATE_OFF } else { GATE_ON };
        let pitch = if level >= gate { yin(x, self.sr).map(to_midi) } else { None };
        let onset = level >= GATE_ON && level > self.prev_rms * ONSET_RATIO;
        self.prev_rms = level;
        self.track_note(pitch, onset, emit);

        self.frames += 1;
        if self.frames % CHROMA_EVERY == 0 && n == self.chroma_len {
            let chord = if rms(&self.buf) >= GATE_ON { recognize_chord(&self.chroma()) } else { None };
            self.track_chord(chord, emit);
        }
    }

    fn track_note(&mut self, pitch: Option<u8>, onset: bool, emit: &mut impl FnMut(AudioEvent)) {
        if onset {
            self.release(emit);
        }
        let Some(p) = pitch else {
            self.candidate = None;
            self.quiet += 1;
            if self.quiet >= RELEASE_FRAMES {
                self.release(emit);
            }
            return;
        };
        self.quiet = 0;
        if self.current == Some(p) {
            self.candidate = None;
            return;
        }
        let count = match self.candidate {
            Some((c, k)) if c == p => k + 1,
            _ => 1,
        };
        self.candidate = Some((p, count));
        if count >= STABLE_FRAMES {
            self.release(emit);
            self.current = Some(p);
            self.candidate = None;
            emit(AudioEvent::Note(NoteEvent::On(p)));
        }
    }

    fn release(&mut self, emit: &mut impl FnMut(AudioEvent)) {
        if let Some(n) = self.current.take() {
            emit(AudioEvent::Note(NoteEvent::Off(n)));
        }
    }

    /// Emits a chord once it's been heard on a couple of consecutive checks.
    fn track_chord(&mut self, chord: Option<BTreeSet<u8>>, emit: &mut impl FnMut(AudioEvent)) {
        let Some(pcs) = chord else {
            self.chord_candidate = None;
            self.last_chord = None;
            return;
        };
        let count = match &self.chord_candidate {
            Some((c, k)) if *c == pcs => k + 1,
            _ => 1,
        };
        if count >= CHORD_STABLE && self.last_chord.as_ref() != Some(&pcs) {
            emit(AudioEvent::Chord(pcs.clone()));
            self.last_chord = Some(pcs.clone());
        }
        self.chord_candidate = Some((pcs, count));
    }

    /// Spectral peaks between 60 Hz and 2.5 kHz folded into 12 pitch classes.
    fn chroma(&self) -> [f32; 12] {
        let mut spec: Vec<Complex<f32>> =
            self.buf.iter().zip(&self.window).map(|(s, w)| Complex::new(s * w, 0.0)).collect();
        self.fft.process(&mut spec);
        let mag: Vec<f32> = spec[..self.chroma_len / 2].iter().map(|c| c.norm()).collect();
        let bin_hz = self.sr / self.chroma_len as f32;
        let mut chroma = [0f32; 12];
        for k in 1..mag.len() - 1 {
            let f = k as f32 * bin_hz;
            if !(60.0..=2500.0).contains(&f) || mag[k] < mag[k - 1] || mag[k] < mag[k + 1] {
                continue;
            }
            let midi = 69.0 + 12.0 * (f / 440.0).log2();
            chroma[(midi.round() as i32).rem_euclid(12) as usize] += mag[k];
        }
        chroma
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 48_000;

    fn freq(midi: u8) -> f32 {
        440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
    }

    /// A plucked string: harmonics shaped by the pluck position, higher ones dying faster,
    /// and a touch of inharmonicity.
    fn pluck(out: &mut [f32], sr: u32, midi: u8, start: usize, amp: f32) {
        let f0 = freq(midi);
        let sr = sr as f32;
        for h in 1..=16 {
            let hf = h as f32;
            let f = f0 * hf * (1.0 + 1e-4 * hf * hf).sqrt();
            if f > sr / 2.0 {
                break;
            }
            let a = amp * (PI * hf * 0.18).sin().abs() / hf;
            for (i, s) in out[start..].iter_mut().enumerate() {
                let t = i as f32 / sr;
                *s += a * (-t * (1.5 + 0.7 * hf)).exp() * (2.0 * PI * f * t).sin();
            }
        }
    }

    fn noise(out: &mut [f32], level: f32) {
        let mut rng = fastrand::Rng::with_seed(7);
        out.iter_mut().for_each(|s| *s += level * (rng.f32() * 2.0 - 1.0));
    }

    fn run(sr: u32, audio: &[f32]) -> Vec<AudioEvent> {
        let mut a = Analyzer::new(sr);
        let mut events = vec![];
        // Odd block size, like a real audio callback.
        for block in audio.chunks(471) {
            a.push(block, &mut |e| events.push(e));
        }
        a.flush(&mut |e| events.push(e));
        events
    }

    fn ons(events: &[AudioEvent]) -> Vec<u8> {
        events.iter().filter_map(|e| match e {
            AudioEvent::Note(NoteEvent::On(n)) => Some(*n),
            _ => None,
        }).collect()
    }

    fn chords(events: &[AudioEvent]) -> Vec<BTreeSet<u8>> {
        events.iter().filter_map(|e| match e {
            AudioEvent::Chord(c) => Some(c.clone()),
            _ => None,
        }).collect()
    }

    fn pcs(root: u8, c: ChordType) -> BTreeSet<u8> {
        c.intervals().iter().map(|i| (root + i) % 12).collect()
    }

    #[test]
    fn detects_single_notes_across_the_neck() {
        for sr in [44_100, 48_000] {
            // Open strings, some fretted notes, and up to the 20th fret.
            for midi in [38, 40, 45, 50, 55, 59, 61, 64, 69, 76, 84] {
                let mut audio = vec![0f32; sr as usize];
                pluck(&mut audio, sr, midi, 0, 0.3);
                noise(&mut audio, 0.001);
                let events = run(sr, &audio);
                assert_eq!(ons(&events), vec![midi], "sr {sr} midi {midi}: {events:?}");
                assert_eq!(events.last(), Some(&AudioEvent::Note(NoteEvent::Off(midi))));
            }
        }
    }

    #[test]
    fn melody_and_repeated_notes() {
        let mut audio = vec![0f32; SR as usize * 3];
        let seq = [57, 59, 60, 60, 64];
        let step = SR as usize / 2;
        for (i, &m) in seq.iter().enumerate() {
            pluck(&mut audio[..(i + 1) * step], SR, m, i * step, 0.3);
        }
        noise(&mut audio, 0.001);
        assert_eq!(ons(&run(SR, &audio)), seq);
    }

    #[test]
    fn silence_and_noise_are_quiet() {
        let mut audio = vec![0f32; SR as usize];
        noise(&mut audio, 0.002);
        assert!(run(SR, &audio).is_empty());
    }

    #[test]
    fn recognizes_strummed_chords() {
        use ChordType::*;
        let cases: &[(&str, &[u8], u8, ChordType)] = &[
            ("open E", &[40, 47, 52, 56, 59, 64], 4, Major),
            ("open A", &[45, 52, 57, 61, 64], 9, Major),
            ("open C", &[48, 52, 55, 60, 64], 0, Major),
            ("open G", &[43, 47, 50, 55, 59, 67], 7, Major),
            ("open D", &[50, 57, 62, 66], 2, Major),
            ("open Am", &[45, 52, 57, 60, 64], 9, Minor),
            ("open Em", &[40, 47, 52, 55, 59, 64], 4, Minor),
            ("open Dm", &[50, 57, 62, 65], 2, Minor),
            ("barre Fm", &[41, 48, 53, 56, 60, 65], 5, Minor),
            ("open E7", &[40, 47, 50, 56, 59, 64], 4, Dominant7),
            ("open A7", &[45, 52, 55, 61, 64], 9, Dominant7),
            ("open Am7", &[45, 52, 55, 60, 64], 9, Minor7),
            ("Cmaj7", &[48, 52, 55, 59, 64], 0, Major7),
            ("Bm7b5", &[47, 53, 57, 62], 11, HalfDiminished7),
            ("Bdim7", &[47, 53, 56, 62], 11, Diminished7),
            ("Bdim", &[47, 50, 53, 59], 11, Diminished),
            ("C aug", &[48, 52, 56, 60], 0, Augmented),
        ];
        for sr in [44_100, 48_000] {
            for &(name, notes, root, kind) in cases {
                let mut audio = vec![0f32; sr as usize];
                for (i, &n) in notes.iter().enumerate() {
                    // Strum: strings 12 ms apart, slightly uneven.
                    let amp = 0.12 * (1.0 - 0.06 * (i % 3) as f32);
                    pluck(&mut audio, sr, n, i * sr as usize * 12 / 1000, amp);
                }
                noise(&mut audio, 0.001);
                let heard = chords(&run(sr, &audio));
                assert_eq!(heard, vec![pcs(root, kind)], "sr {sr} {name}");
            }
        }
    }

    #[test]
    fn single_notes_and_power_chords_arent_chords() {
        let shapes: &[&[u8]] = &[&[48], &[40], &[57, 69], &[40, 47, 52], &[45, 52, 57]];
        for notes in shapes {
            let mut audio = vec![0f32; SR as usize];
            for (i, &n) in notes.iter().enumerate() {
                pluck(&mut audio, SR, n, i * 600, 0.15);
            }
            noise(&mut audio, 0.001);
            let heard = chords(&run(SR, &audio));
            assert!(heard.is_empty(), "{notes:?} heard as {heard:?}");
        }
    }
}

