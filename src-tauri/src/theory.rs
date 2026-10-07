use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const SHARP_NAMES: [&str; 12] = ["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "G♯", "A", "B♭", "B"];
const FLAT_NAMES: [&str; 12] = ["C", "D♭", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];

/// Root spelling: minor-flavoured keys prefer sharps (C♯m), major ones flats (D♭).
pub fn pitch_name(pc: u8, minor: bool) -> &'static str {
    let names = if minor { &SHARP_NAMES } else { &FLAT_NAMES };
    names[(pc % 12) as usize]
}

/// Name with octave for a MIDI note, middle C = C4.
pub fn note_name(note: u8) -> String {
    format!("{}{}", pitch_name(note % 12, false), note as i32 / 12 - 1)
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
    fn labels() {
        assert_eq!(Challenge { root: D, kind: Kind::Chord(ChordType::Minor7) }.short(), "Dm7");
        assert_eq!(Challenge { root: 1, kind: Kind::Chord(ChordType::Minor) }.short(), "C♯m");
        assert_eq!(Challenge { root: 1, kind: Kind::Chord(ChordType::Major) }.short(), "D♭");
        assert_eq!(Challenge { root: D, kind: Kind::Interval(Interval::MinorThird) }.short(), "D → m3");
        assert_eq!(note_name(60), "C4");
    }

    #[test]
    fn random_respects_kinds() {
        let kinds = [Kind::Scale(ScaleType::Major)];
        for _ in 0..50 {
            assert_eq!(Challenge::random(&kinds, None).kind, kinds[0]);
        }
    }
}
