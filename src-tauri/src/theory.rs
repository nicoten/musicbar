use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const SHARP_NAMES: [&str; 12] = ["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "G♯", "A", "B♭", "B"];
const FLAT_NAMES: [&str; 12] = ["C", "D♭", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];

/// Root spelling: minor-flavoured keys prefer sharps (C♯m), major ones flats (D♭).
pub fn pitch_name(pc: u8, minor: bool) -> &'static str {
    let names = if minor { &SHARP_NAMES } else { &FLAT_NAMES };
    names[(pc % 12) as usize]
}

/// A note as written on a staff: letter (0 = C … 6 = B), sharps (+) or flats (-), octave (middle C = 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Spelled {
    pub letter: u8,
    pub alter: i8,
    pub octave: i8,
}

const NATURAL_PCS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

const LETTERS: &str = "CDEFGAB";
/// Fallback names for notes outside the challenge, by the accidentals it uses.
const ALL_SHARPS: [&str; 12] = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"];
const ALL_FLATS: [&str; 12] = ["C", "D♭", "D", "E♭", "E", "F", "G♭", "G", "A♭", "A", "B♭", "B"];
/// Letters spanned by an interval of 0..=12 semitones (the tritone as an augmented 4th).
const INTERVAL_LETTERS: [u8; 13] = [0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6, 7];

fn accidental(alter: i8) -> &'static str {
    match alter {
        -2 => "𝄫",
        -1 => "♭",
        1 => "♯",
        2 => "𝄪",
        _ => "",
    }
}

/// Letter and alteration of a name like "E♭".
fn parse_name(name: &str) -> (u8, i8) {
    let letter = LETTERS.find(name.chars().next().unwrap()).unwrap() as u8;
    let alter = name.chars().skip(1).map(|c| if c == '♭' { -1 } else { 1 }).sum();
    (letter, alter)
}

/// How a pitch class is written in some context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PitchName {
    pub name: String,
    pub alter: i8,
}

impl Spelled {
    /// e.g. "D♯4".
    pub fn name(&self) -> String {
        let letter = LETTERS.as_bytes()[self.letter as usize] as char;
        format!("{letter}{}{}", accidental(self.alter), self.octave)
    }
}

/// Spells `note` on the given letter, e.g. MIDI 60 on B is B♯3.
fn spell(note: u8, letter: u8) -> Spelled {
    let alter = ((note % 12 + 18 - NATURAL_PCS[letter as usize]) % 12) as i8 - 6;
    let natural = note as i32 - alter as i32;
    Spelled { letter, alter, octave: (natural.div_euclid(12) - 1) as i8 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChordType {
    Major,
    Minor,
    Diminished,
    Augmented,
    Major7,
    Dominant7,
    Minor7,
    HalfDiminished7,
    Diminished7,
}

impl ChordType {
    pub const ALL: [ChordType; 9] = [
        ChordType::Major,
        ChordType::Minor,
        ChordType::Diminished,
        ChordType::Augmented,
        ChordType::Major7,
        ChordType::Dominant7,
        ChordType::Minor7,
        ChordType::HalfDiminished7,
        ChordType::Diminished7,
    ];

    pub fn intervals(self) -> &'static [u8] {
        match self {
            ChordType::Major => &[0, 4, 7],
            ChordType::Minor => &[0, 3, 7],
            ChordType::Diminished => &[0, 3, 6],
            ChordType::Augmented => &[0, 4, 8],
            ChordType::Major7 => &[0, 4, 7, 11],
            ChordType::Dominant7 => &[0, 4, 7, 10],
            ChordType::Minor7 => &[0, 3, 7, 10],
            ChordType::HalfDiminished7 => &[0, 3, 6, 10],
            ChordType::Diminished7 => &[0, 3, 6, 9],
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            ChordType::Major => "",
            ChordType::Minor => "m",
            ChordType::Diminished => "°",
            ChordType::Augmented => "+",
            ChordType::Major7 => "maj7",
            ChordType::Dominant7 => "7",
            ChordType::Minor7 => "m7",
            ChordType::HalfDiminished7 => "ø7",
            ChordType::Diminished7 => "°7",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ChordType::Major => "major",
            ChordType::Minor => "minor",
            ChordType::Diminished => "diminished",
            ChordType::Augmented => "augmented",
            ChordType::Major7 => "major 7th",
            ChordType::Dominant7 => "dominant 7th",
            ChordType::Minor7 => "minor 7th",
            ChordType::HalfDiminished7 => "half-diminished 7th",
            ChordType::Diminished7 => "diminished 7th",
        }
    }

    fn minor(self) -> bool {
        !matches!(self, ChordType::Major | ChordType::Augmented | ChordType::Major7 | ChordType::Dominant7)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interval {
    MinorSecond,
    MajorSecond,
    MinorThird,
    MajorThird,
    PerfectFourth,
    Tritone,
    PerfectFifth,
    MinorSixth,
    MajorSixth,
    MinorSeventh,
    MajorSeventh,
    Octave,
}

impl Interval {
    pub const ALL: [Interval; 12] = [
        Interval::MinorSecond,
        Interval::MajorSecond,
        Interval::MinorThird,
        Interval::MajorThird,
        Interval::PerfectFourth,
        Interval::Tritone,
        Interval::PerfectFifth,
        Interval::MinorSixth,
        Interval::MajorSixth,
        Interval::MinorSeventh,
        Interval::MajorSeventh,
        Interval::Octave,
    ];

    pub fn semitones(self) -> u8 {
        Self::ALL.iter().position(|&i| i == self).unwrap() as u8 + 1
    }

    pub fn short(self) -> &'static str {
        ["m2", "M2", "m3", "M3", "P4", "TT", "P5", "m6", "M6", "m7", "M7", "P8"][self.semitones() as usize - 1]
    }

    pub fn name(self) -> &'static str {
        [
            "minor 2nd",
            "major 2nd",
            "minor 3rd",
            "major 3rd",
            "perfect 4th",
            "tritone",
            "perfect 5th",
            "minor 6th",
            "major 6th",
            "minor 7th",
            "major 7th",
            "octave",
        ][self.semitones() as usize - 1]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleType {
    Major,
    NaturalMinor,
    HarmonicMinor,
    MelodicMinor,
}

impl ScaleType {
    pub const ALL: [ScaleType; 4] = [
        ScaleType::Major,
        ScaleType::NaturalMinor,
        ScaleType::HarmonicMinor,
        ScaleType::MelodicMinor,
    ];

    /// Ascending semitone steps over one octave (melodic minor uses its ascending form).
    pub fn steps(self) -> [u8; 7] {
        match self {
            ScaleType::Major => [2, 2, 1, 2, 2, 2, 1],
            ScaleType::NaturalMinor => [2, 1, 2, 2, 1, 2, 2],
            ScaleType::HarmonicMinor => [2, 1, 2, 2, 1, 3, 1],
            ScaleType::MelodicMinor => [2, 1, 2, 2, 2, 2, 1],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ScaleType::Major => "major",
            ScaleType::NaturalMinor => "natural minor",
            ScaleType::HarmonicMinor => "harmonic minor",
            ScaleType::MelodicMinor => "melodic minor",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Chord(ChordType),
    Interval(Interval),
    Scale(ScaleType),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub root: u8,
    pub kind: Kind,
}

impl Challenge {
    /// Picks a random kind and root, avoiding an exact repeat of `prev` when possible.
    pub fn random(kinds: &[Kind], prev: Option<&Challenge>) -> Challenge {
        let fallback = [Kind::Chord(ChordType::Major)];
        let kinds = if kinds.is_empty() { &fallback[..] } else { kinds };
        let mut c = Self::pick(kinds);
        for _ in 0..10 {
            if Some(&c) != prev {
                break;
            }
            c = Self::pick(kinds);
        }
        c
    }

    fn pick(kinds: &[Kind]) -> Challenge {
        Challenge { root: fastrand::u8(0..12), kind: kinds[fastrand::usize(..kinds.len())] }
    }

    pub fn category(&self) -> &'static str {
        match self.kind {
            Kind::Chord(_) => "Chord",
            Kind::Interval(_) => "Interval",
            Kind::Scale(_) => "Scale",
        }
    }

    fn root_name(&self) -> &'static str {
        let minor = match self.kind {
            Kind::Chord(c) => c.minor(),
            Kind::Interval(_) => false,
            Kind::Scale(s) => s != ScaleType::Major,
        };
        pitch_name(self.root, minor)
    }

    /// Compact label for the menu bar, e.g. "Dm7", "D → m3", "D harm. minor".
    pub fn short(&self) -> String {
        let root = self.root_name();
        match self.kind {
            Kind::Chord(c) => format!("{root}{}", c.suffix()),
            Kind::Interval(i) => format!("{root} → {}", i.short()),
            Kind::Scale(s) => {
                let name = match s {
                    ScaleType::Major => "major",
                    ScaleType::NaturalMinor => "nat. minor",
                    ScaleType::HarmonicMinor => "harm. minor",
                    ScaleType::MelodicMinor => "mel. minor",
                };
                format!("{root} {name}")
            }
        }
    }

    pub fn long(&self) -> String {
        let root = self.root_name();
        match self.kind {
            Kind::Chord(c) => format!("{root} {} chord", c.name()),
            Kind::Interval(i) => format!("{} up from {root}", i.name()),
            Kind::Scale(s) => format!("{root} {} scale, one octave up", s.name()),
        }
    }

    /// The challenge's own notes as (semitones above the root, letters above the root's letter).
    fn tones(&self) -> Vec<(u8, u8)> {
        match self.kind {
            Kind::Chord(c) => c.intervals().iter().zip([0, 2, 4, 6]).map(|(&i, l)| (i, l)).collect(),
            Kind::Interval(i) => vec![(0, 0), (i.semitones(), INTERVAL_LETTERS[i.semitones() as usize])],
            Kind::Scale(s) => {
                let mut up = 0;
                (0..7u8).map(|l| { let tone = (up, l); up += s.steps()[l as usize]; tone }).collect()
            }
        }
    }

    /// Letter and alteration for each pitch class (C = 0) in the context of this challenge: its own
    /// notes in key (B major → D♯, not E♭), the rest with sharps or flats to match.
    fn spelling(&self) -> [(u8, i8); 12] {
        let root_letter = parse_name(self.root_name()).0;
        let tones: Vec<(u8, u8, i8)> = self
            .tones()
            .into_iter()
            .map(|(semis, letters)| {
                let letter = (root_letter + letters) % 7;
                (self.root + semis, letter, spell(self.root + semis, letter).alter)
            })
            .collect();
        let fallback = if tones.iter().any(|t| t.2 < 0) {
            &ALL_FLATS
        } else if tones.iter().any(|t| t.2 > 0) {
            &ALL_SHARPS
        } else {
            &FLAT_NAMES
        };
        let mut out = fallback.map(parse_name);
        for (note, letter, alter) in tones {
            out[(note % 12) as usize] = (letter, alter);
        }
        out
    }

    /// A MIDI note named in the context of this challenge, e.g. "D♯4" for B major.
    pub fn note_name(&self, note: u8) -> String {
        spell(note, self.spelling()[(note % 12) as usize].0).name()
    }

    /// Names for each pitch class (C = 0) in the context of this challenge, without octave.
    pub fn pitch_names(&self) -> Vec<PitchName> {
        self.spelling()
            .iter()
            .map(|&(letter, alter)| PitchName {
                name: format!("{}{}", LETTERS.as_bytes()[letter as usize] as char, accidental(alter)),
                alter,
            })
            .collect()
    }

    /// The notes played so far that are correct, spelled in key, for the staff:
    /// scales, the latest run that correctly starts the scale; chords, the held chord tones, low to
    /// high; intervals, the root once it's the last note played.
    pub fn progress(&self, held: &BTreeSet<u8>, history: &[u8]) -> Vec<Spelled> {
        let notes: Vec<u8> = match self.kind {
            Kind::Scale(s) => {
                let fits = |run: &[u8]| {
                    run[0] % 12 == self.root
                        && run.windows(2).zip(s.steps()).all(|(w, step)| w[1].checked_sub(w[0]) == Some(step))
                };
                (1..=history.len().min(8))
                    .rev()
                    .map(|k| &history[history.len() - k..])
                    .find(|run| fits(run))
                    .map(<[u8]>::to_vec)
                    .unwrap_or_default()
            }
            Kind::Chord(c) => {
                held.iter().copied().filter(|n| c.intervals().iter().any(|i| (self.root + i) % 12 == n % 12)).collect()
            }
            Kind::Interval(_) => history.last().copied().filter(|n| n % 12 == self.root).into_iter().collect(),
        };
        let spelling = self.spelling();
        notes.into_iter().map(|n| spell(n, spelling[(n % 12) as usize].0)).collect()
    }

    /// `held` = currently pressed notes; `history` = recent note-ons, oldest first.
    pub fn is_satisfied(&self, held: &BTreeSet<u8>, history: &[u8]) -> bool {
        match self.kind {
            Kind::Chord(c) => {
                let want: BTreeSet<u8> = c.intervals().iter().map(|i| (self.root + i) % 12).collect();
                let got: BTreeSet<u8> = held.iter().map(|n| n % 12).collect();
                want == got
            }
            Kind::Interval(i) => {
                let fits = |lo: u8, hi: u8| lo % 12 == self.root && hi.checked_sub(lo) == Some(i.semitones());
                let sequential = matches!(history, [.., a, b] if fits(*a, *b));
                let together = held.len() == 2 && fits(*held.first().unwrap(), *held.last().unwrap());
                sequential || together
            }
            Kind::Scale(s) => {
                let Some(start) = history.len().checked_sub(8) else { return false };
                let run = &history[start..];
                run[0] % 12 == self.root
                    && run.windows(2).zip(s.steps()).all(|(w, step)| w[1].checked_sub(w[0]) == Some(step))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(notes: &[u8]) -> BTreeSet<u8> {
        notes.iter().copied().collect()
    }

    const D: u8 = 2;

    #[test]
    fn chord_matches_any_voicing() {
        let c = Challenge { root: D, kind: Kind::Chord(ChordType::Minor7) };
        assert!(c.is_satisfied(&set(&[62, 65, 69, 72]), &[]));
        // inversion spread over octaves, doubled root
        assert!(c.is_satisfied(&set(&[48, 53, 57, 62, 74]), &[]));
        assert!(!c.is_satisfied(&set(&[62, 65, 69]), &[]), "missing 7th");
        assert!(!c.is_satisfied(&set(&[62, 65, 69, 72, 76]), &[]), "extra note");
    }

    #[test]
    fn interval_sequential_or_together() {
        let c = Challenge { root: D, kind: Kind::Interval(Interval::MinorThird) };
        assert!(c.is_satisfied(&set(&[]), &[60, 62, 65]));
        assert!(c.is_satisfied(&set(&[62, 65]), &[65, 62]));
        assert!(!c.is_satisfied(&set(&[]), &[62, 53]), "descending doesn't count");
        assert!(!c.is_satisfied(&set(&[]), &[62, 77]), "compound interval doesn't count");
        assert!(!c.is_satisfied(&set(&[]), &[64, 67]), "wrong root");
    }

    #[test]
    fn octave_interval() {
        let c = Challenge { root: 0, kind: Kind::Interval(Interval::Octave) };
        assert!(c.is_satisfied(&set(&[]), &[60, 72]));
        assert!(!c.is_satisfied(&set(&[]), &[60, 60]));
    }

    #[test]
    fn scale_needs_last_eight_ascending() {
        let c = Challenge { root: 9, kind: Kind::Scale(ScaleType::HarmonicMinor) };
        let a_harm = [57, 59, 60, 62, 64, 65, 68, 69];
        assert!(c.is_satisfied(&set(&[]), &a_harm));
        let mut with_mistake_before = vec![40, 41];
        with_mistake_before.extend(a_harm);
        assert!(c.is_satisfied(&set(&[]), &with_mistake_before));
        let natural = [57, 59, 60, 62, 64, 65, 67, 69];
        assert!(!c.is_satisfied(&set(&[]), &natural));
        assert!(!c.is_satisfied(&set(&[]), &a_harm[..7]));
    }

    #[test]
    fn melodic_minor_ascending() {
        let c = Challenge { root: 0, kind: Kind::Scale(ScaleType::MelodicMinor) };
        assert!(c.is_satisfied(&set(&[]), &[60, 62, 63, 65, 67, 69, 71, 72]));
    }

    #[test]
    fn progress_spells_the_correct_notes() {
        let eb = Challenge { root: 3, kind: Kind::Scale(ScaleType::Major) };
        let letters = |p: Vec<Spelled>| p.iter().map(|s| (s.letter, s.alter, s.octave)).collect::<Vec<_>>();
        assert_eq!(letters(eb.progress(&BTreeSet::new(), &[51, 53, 55, 56])), [(2, -1, 3), (3, 0, 3), (4, 0, 3), (5, -1, 3)], "E♭ F G A♭");
        assert!(eb.progress(&BTreeSet::new(), &[51, 53, 54]).is_empty(), "wrong note resets");
        assert_eq!(eb.progress(&BTreeSet::new(), &[51, 53, 54, 51]).len(), 1, "restarted from the root");
        assert!(eb.progress(&BTreeSet::new(), &[]).is_empty());

        let gs = Challenge { root: 8, kind: Kind::Scale(ScaleType::HarmonicMinor) };
        assert_eq!(gs.progress(&BTreeSet::new(), &[56, 58, 59, 61, 63, 64, 67]).last(), Some(&Spelled { letter: 3, alter: 2, octave: 4 }), "F𝄪");
        let cs = Challenge { root: 1, kind: Kind::Scale(ScaleType::HarmonicMinor) };
        assert_eq!(cs.progress(&BTreeSet::new(), &[49, 51, 52, 54, 56, 57, 60])[6], Spelled { letter: 6, alter: 1, octave: 3 }, "B♯3");

        let b = Challenge { root: 11, kind: Kind::Chord(ChordType::Major) };
        let held = BTreeSet::from([47, 50, 51, 66]);
        assert_eq!(b.progress(&held, &[]).iter().map(Spelled::name).collect::<Vec<_>>(), ["B2", "D♯3", "F♯4"], "wrong D left out");
        let m3 = Challenge { root: 2, kind: Kind::Interval(Interval::MinorThird) };
        assert_eq!(m3.progress(&BTreeSet::new(), &[50]).len(), 1);
        assert!(m3.progress(&BTreeSet::new(), &[50, 52]).is_empty(), "wrong second note");
    }

    #[test]
    fn notes_are_named_in_the_challenge_key() {
        let b_major = Challenge { root: 11, kind: Kind::Chord(ChordType::Major) };
        assert_eq!(b_major.note_name(63), "D♯4");
        assert_eq!(b_major.note_name(66), "F♯4");
        assert_eq!(b_major.note_name(68), "G♯4", "outside the chord: sharps to match");
        let ab_major = Challenge { root: 8, kind: Kind::Chord(ChordType::Major) };
        assert_eq!(ab_major.note_name(60), "C4");
        assert_eq!(ab_major.note_name(63), "E♭4");
        assert_eq!(ab_major.note_name(66), "G♭4");
        let c_dim7 = Challenge { root: 0, kind: Kind::Chord(ChordType::Diminished7) };
        assert_eq!(c_dim7.note_name(57), "B𝄫3");
        let cs_harm = Challenge { root: 1, kind: Kind::Scale(ScaleType::HarmonicMinor) };
        assert_eq!(cs_harm.note_name(60), "B♯3");
        let f_tritone = Challenge { root: 5, kind: Kind::Interval(Interval::Tritone) };
        assert_eq!(f_tritone.note_name(59), "B3");
        assert_eq!(b_major.pitch_names()[3], PitchName { name: "D♯".into(), alter: 1 });
    }

    #[test]
    fn labels() {
        assert_eq!(Challenge { root: D, kind: Kind::Chord(ChordType::Minor7) }.short(), "Dm7");
        assert_eq!(Challenge { root: 1, kind: Kind::Chord(ChordType::Minor) }.short(), "C♯m");
        assert_eq!(Challenge { root: 1, kind: Kind::Chord(ChordType::Major) }.short(), "D♭");
        assert_eq!(Challenge { root: D, kind: Kind::Interval(Interval::MinorThird) }.short(), "D → m3");
    }

    #[test]
    fn random_respects_kinds() {
        let kinds = [Kind::Scale(ScaleType::Major)];
        for _ in 0..50 {
            assert_eq!(Challenge::random(&kinds, None).kind, kinds[0]);
        }
    }
}
