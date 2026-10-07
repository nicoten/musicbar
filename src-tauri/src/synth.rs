//! A small plucked-string synth (Karplus-Strong) for clicked frets and solved challenges.
use crate::audio::name;
use crate::theory::{Challenge, Kind};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// How long a plucked note rings before it's cut off.
const RING: Duration = Duration::from_millis(1500);
const FADE: Duration = Duration::from_millis(300);
/// Gap between a solving note and the answer played back.
const ANSWER_AFTER: Duration = Duration::from_millis(400);
const STRUM: Duration = Duration::from_millis(15);
const INTERVAL_GAP: Duration = Duration::from_millis(600);
const SCALE_GAP: Duration = Duration::from_millis(250);
const GAIN: f32 = 0.35;

/// A MIDI note to pluck `Duration` from now.
type Pluck = (u8, Duration);

static QUIET_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// Plucks `note` now.
pub fn pluck(note: u8) {
    play(vec![(note, Duration::ZERO)]);
}

/// Plays the answer to `c` after a short pause: a chord together, an interval's two notes one after
/// the other, a scale up the octave.
pub fn answer(c: &Challenge) {
    let gap = match c.kind {
        Kind::Chord(_) => STRUM,
        Kind::Interval(_) => INTERVAL_GAP,
        Kind::Scale(_) => SCALE_GAP,
    };
    play(c.notes().into_iter().zip(0..).map(|(n, i)| (n, ANSWER_AFTER + gap * i)).collect())
}

/// True while something played here may still be ringing, so audio input doesn't hear it as you.
pub fn sounding() -> bool {
    QUIET_AT.lock().unwrap().is_some_and(|at| Instant::now() < at)
}

fn play(plucks: Vec<Pluck>) {
    let Some(last) = plucks.iter().map(|&(_, at)| at).max() else { return };
    {
        let mut quiet = QUIET_AT.lock().unwrap();
        let end = Instant::now() + last + RING;
        *quiet = Some(quiet.map_or(end, |q| q.max(end)));
    }
    static TX: OnceLock<Sender<Vec<Pluck>>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = channel();
        thread::spawn(move || run(rx));
        tx
    });
    let _ = tx.send(plucks);
}

struct Output {
    device: String,
    rate: f32,
    voices: Sender<Voice>,
    failed: Arc<AtomicBool>,
    stream: Stream,
}

/// Owns the output stream: opens it on the first note (again if the default output changes or
/// fails), and pauses it once everything has rung out so an idle app doesn't hold the audio device.
fn run(rx: Receiver<Vec<Pluck>>) {
    let mut out: Option<Output> = None;
    let mut idle_at: Option<Instant> = None;
    loop {
        let wait = idle_at.map_or(Duration::MAX, |at| at.saturating_duration_since(Instant::now()));
        let plucks = match rx.recv_timeout(wait) {
            Ok(plucks) => plucks,
            Err(RecvTimeoutError::Timeout) => {
                if let Some(o) = &out {
                    let _ = o.stream.pause();
                }
                idle_at = None;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let device = cpal::default_host().default_output_device();
        let device_name = device.as_ref().and_then(name);
        if out.as_ref().is_none_or(|o| o.failed.load(Ordering::Relaxed) || Some(&o.device) != device_name.as_ref()) {
            out = device.and_then(open);
        }
        let Some(o) = &out else { continue };
        let last = plucks.iter().map(|&(_, at)| at).max().unwrap_or_default();
        for (note, at) in plucks {
            let _ = o.voices.send(Voice::new(note, o.rate, (at.as_secs_f32() * o.rate) as usize));
        }
        let _ = o.stream.play();
        let end = Instant::now() + last + RING + Duration::from_millis(500);
        idle_at = Some(idle_at.map_or(end, |at| at.max(end)));
    }
}

fn open(device: Device) -> Option<Output> {
    let supported = device.default_output_config().ok()?;
    let config = supported.config();
    let (tx, rx) = channel();
    let failed = Arc::new(AtomicBool::new(false));
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config, rx, &failed),
        SampleFormat::I16 => build::<i16>(&device, &config, rx, &failed),
        SampleFormat::I32 => build::<i32>(&device, &config, rx, &failed),
        SampleFormat::U16 => build::<u16>(&device, &config, rx, &failed),
        _ => None,
    }?;
    Some(Output { device: name(&device)?, rate: config.sample_rate as f32, voices: tx, failed, stream })
}

/// Mixes the ringing voices into every channel.
fn build<T>(device: &Device, config: &StreamConfig, rx: Receiver<Voice>, failed: &Arc<AtomicBool>) -> Option<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut voices: Vec<Voice> = Vec::new();
    let on_data = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        voices.extend(rx.try_iter());
        for frame in data.chunks_mut(channels) {
            let mix: f32 = voices.iter_mut().map(Voice::next).sum();
            frame.fill(T::from_sample((mix * GAIN).tanh()));
        }
        voices.retain(|v| v.left > 0);
    };
    let failed = failed.clone();
    let on_error = move |_| failed.store(true, Ordering::Relaxed);
    device.build_output_stream(config, on_data, on_error, None).ok()
}

/// One plucked string: a delay line one period long, filled with noise and averaged as it goes round.
struct Voice {
    line: Vec<f32>,
    pos: usize,
    /// Samples of silence before it's plucked.
    wait: usize,
    /// Samples left to ring.
    left: usize,
    fade: usize,
}

impl Voice {
    fn new(note: u8, rate: f32, wait: usize) -> Self {
        let freq = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
        let len = ((rate / freq).round() as usize).max(2);
        let mut line: Vec<f32> = (0..len).map(|_| fastrand::f32() * 2.0 - 1.0).collect();
        // Soften the attack so it sounds more like a finger than a pick.
        for _ in 0..2 {
            for i in 1..len {
                line[i] = 0.5 * (line[i] + line[i - 1]);
            }
        }
        let mean = line.iter().sum::<f32>() / len as f32;
        line.iter_mut().for_each(|s| *s -= mean);
        Voice { line, pos: 0, wait, left: (RING.as_secs_f32() * rate) as usize, fade: (FADE.as_secs_f32() * rate) as usize }
    }

    fn next(&mut self) -> f32 {
        if self.wait > 0 {
            self.wait -= 1;
            return 0.0;
        }
        if self.left == 0 {
            return 0.0;
        }
        self.left -= 1;
        let next = (self.pos + 1) % self.line.len();
        let out = self.line[self.pos];
        self.line[self.pos] = 0.996 * 0.5 * (out + self.line[next]);
        self.pos = next;
        out * (self.left as f32 / self.fade as f32).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_waits_rings_and_dies_out() {
        let mut v = Voice::new(57, 48_000.0, 100);
        assert!((0..100).all(|_| v.next() == 0.0), "silent until plucked");
        let attack: f32 = (0..4_800).map(|_| v.next().abs()).sum();
        assert!(attack > 100.0, "audible: {attack}");
        let mut tail = 0.0;
        while v.left > 0 {
            tail = v.next().abs();
            assert!(tail.is_finite() && tail <= 1.0);
        }
        assert_eq!((tail, v.next()), (0.0, 0.0), "faded out");
    }
}
