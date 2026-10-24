//! The guitar neck: where notes sit, fret windows, chord shapes to name and target notes to find.

use crate::theory::{Challenge, ChordType, Kind};

/// Standard tuning, high E first (like the on-screen fretboard).
pub const STRINGS: [u8; 6] = [64, 59, 55, 50, 45, 40];
pub const FRETS: u8 = 21;
/// Frets in a fret window.
pub const WINDOW: u8 = 5;
/// How far a target note can be from its root, in frets.
pub const REACH: u8 = 4;
/// Frets a chord shape spans at most (a hand's stretch).
const SHAPE_SPAN: u8 = 4;

/// A place on the neck: (string, fret), string 0 = high E.
pub type Spot = (u8, u8);

pub fn note((string, fret): Spot) -> u8 {
    STRINGS[string as usize] + fret
}

/// The spot `note` was clicked on, from its string.
pub fn spot(string: u8, note: u8) -> Option<Spot> {
    let fret = note.checked_sub(*STRINGS.get(string as usize)?)?;
    (fret <= FRETS).then_some((string, fret))
}

/// Last fret of the window starting at `lo`.
pub fn window_end(lo: u8) -> u8 {
    lo + WINDOW - 1
}

/// True if `fret` is in the window starting at `lo`. A window from the 1st fret takes in the open strings.
pub fn in_window(lo: u8, fret: u8) -> bool {
    (lo..=window_end(lo)).contains(&fret) || (lo == 1 && fret == 0)
}

fn all_spots() -> impl Iterator<Item = Spot> {
    (0..6u8).flat_map(|s| (0..=FRETS).map(move |f| (s, f)))
}

/// True if `c` can be played on the fretboard using only the window starting at `lo`.
pub fn fits(c: &Challenge, lo: u8) -> bool {
    let notes: Vec<u8> = all_spots().filter(|s| in_window(lo, s.1)).map(note).collect();
    let have = |n: u8| notes.contains(&n);
    match c.kind {
        Kind::Chord(t) => {
            let pcs: Vec<u8> = t.intervals().iter().map(|i| (c.root + i) % 12).collect();
            fits_chord(&pcs, lo, &mut [false; 6])
        }
        Kind::Interval(i) => notes.iter().any(|&n| n % 12 == c.root && have(n + i.semitones())),
        Kind::Scale(_) => {
            let answer = c.notes();
            notes.iter().any(|&n| n % 12 == c.root && answer.iter().all(|&a| have(n + a - answer[0])))
        }
        Kind::Arpeggio(..) => c.notes().iter().all(|&a| notes.iter().any(|n| n % 12 == a % 12)),
    }
}

/// True if each pitch class can go on a string of its own within the window.
fn fits_chord(pcs: &[u8], lo: u8, used: &mut [bool; 6]) -> bool {
    let Some((&pc, rest)) = pcs.split_first() else { return true };
    (0..6u8).any(|s| {
        let here = !used[s as usize] && (0..=FRETS).any(|f| in_window(lo, f) && note((s, f)) % 12 == pc);
        if !here {
            return false;
        }
        used[s as usize] = true;
        let ok = fits_chord(rest, lo, used);
        used[s as usize] = false;
        ok
    })
}

/// A random window `c` fits in, by its first fret.
pub fn window_for(c: &Challenge) -> Option<u8> {
    let mut starts: Vec<u8> = (1..=FRETS + 1 - WINDOW).collect();
    fastrand::shuffle(&mut starts);
    starts.into_iter().find(|&lo| fits(c, lo))
}

/// A random playable voicing of `chord` on `root`: adjacent strings, the root in the bass, every
/// chord tone, the fretted notes within a hand's stretch. Open strings ring for free.
pub fn shape(root: u8, chord: ChordType) -> Vec<Spot> {
    let mut starts: Vec<u8> = (1..=FRETS + 1 - SHAPE_SPAN).collect();
    fastrand::shuffle(&mut starts);
    starts.into_iter().find_map(|lo| {
        let shapes = shapes_from(root, chord, lo);
        (!shapes.is_empty()).then(|| shapes[fastrand::usize(..shapes.len())].clone())
    })
    .unwrap_or_default()
}

/// Every voicing with its fretted notes between `lo` and `lo + SHAPE_SPAN - 1`.
fn shapes_from(root: u8, chord: ChordType, lo: u8) -> Vec<Vec<Spot>> {
    let pcs: Vec<u8> = chord.intervals().iter().map(|i| (root + i) % 12).collect();
    let min_strings = pcs.len() as u8;
    let frets = |s: u8| -> Vec<u8> {
        (0..=FRETS)
            .filter(|&f| (f == 0 && lo == 1) || (lo..lo + SHAPE_SPAN).contains(&f))
            .filter(|&f| pcs.contains(&(note((s, f)) % 12)))
            .collect()
    };
    let mut out = Vec::new();
    // Bass on a lower string (higher index), up to `top`.
    for bass in 0..6u8 {
        for top in 0..=bass {
            if bass - top + 1 < min_strings {
                continue;
            }
            for f in frets(bass).into_iter().filter(|&f| note((bass, f)) % 12 == root) {
                let mut picked = vec![(bass, f)];
                voicings(&pcs, top, bass, &frets, &mut picked, &mut out);
            }
        }
    }
    out
}

fn voicings(pcs: &[u8], top: u8, string: u8, frets: &dyn Fn(u8) -> Vec<u8>, picked: &mut Vec<Spot>, out: &mut Vec<Vec<Spot>>) {
    if string == top {
        let covered = pcs.iter().all(|pc| picked.iter().any(|&s| note(s) % 12 == *pc));
        if covered {
            out.push(picked.clone());
        }
        return;
    }
    for f in frets(string - 1) {
        picked.push((string - 1, f));
        voicings(pcs, top, string - 1, frets, picked, out);
        picked.pop();
    }
}

/// The spots `semis` above the note at `root`, within reach of it on any string.
pub fn targets(root: Spot, semis: u8) -> Vec<Spot> {
    all_spots().filter(|&s| note(s) == note(root) + semis && s.1.abs_diff(root.1) <= REACH).collect()
}

/// A random spot with somewhere to find `semis` above it.
pub fn target_root(semis: u8) -> Spot {
    let mut spots: Vec<Spot> = all_spots().collect();
    fastrand::shuffle(&mut spots);
    spots.into_iter().find(|&s| !targets(s, semis).is_empty()).unwrap_or((5, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::{Interval, ScaleType};
    use std::collections::BTreeSet;

    #[test]
    fn windows_take_open_strings_only_from_the_first_fret() {
        assert!(in_window(1, 0) && in_window(1, 5) && !in_window(1, 6));
        assert!(!in_window(6, 0) && !in_window(6, 5) && in_window(6, 10) && !in_window(6, 11));
        assert_eq!(spot(5, 45), Some((5, 5)));
        assert_eq!(spot(0, 50), None, "below the open string");
    }

    #[test]
    fn challenges_fit_their_window() {
        let dm7 = Challenge { root: 2, kind: Kind::Chord(ChordType::Minor7) };
        assert!(fits(&dm7, 6), "C-A-F-D down from the D string at 10");
        let interval = Challenge { root: 0, kind: Kind::Interval(Interval::MajorSeventh) };
        assert!(fits(&interval, 1));
        for kind in [Kind::Scale(ScaleType::Major), Kind::Chord(ChordType::Major7), Kind::Interval(Interval::Octave)] {
            for root in 0..12 {
                let c = Challenge { root, kind };
                let lo = window_for(&c).expect("somewhere on the neck");
                assert!(fits(&c, lo));
            }
        }
    }

    #[test]
    fn scale_needs_the_whole_octave_in_the_window() {
        let c = Challenge { root: 0, kind: Kind::Scale(ScaleType::Major) };
        // C major from the A string at fret 3 runs up to C on the B string at fret 1.
        assert!(fits(&c, 1));
        // F on the low E string at 1 to F on the D string at 3.
        let octave = Challenge { root: 5, kind: Kind::Interval(Interval::Octave) };
        assert!(fits(&octave, 1));
    }

    #[test]
    fn shapes_are_playable_voicings() {
        for chord in ChordType::ALL {
            for root in 0..12 {
                let s = shape(root, chord);
                assert!(!s.is_empty(), "{root} {chord:?}");
                let bass = s.iter().max_by_key(|sp| sp.0).unwrap();
                assert_eq!(note(*bass) % 12, root, "root in the bass");
                let pcs: BTreeSet<u8> = s.iter().map(|&sp| note(sp) % 12).collect();
                let want: BTreeSet<u8> = chord.intervals().iter().map(|i| (root + i) % 12).collect();
                assert_eq!(pcs, want);
                let fretted: Vec<u8> = s.iter().map(|sp| sp.1).filter(|&f| f > 0).collect();
                if let (Some(lo), Some(hi)) = (fretted.iter().min(), fretted.iter().max()) {
                    assert!(hi - lo < SHAPE_SPAN);
                }
                let strings: BTreeSet<u8> = s.iter().map(|sp| sp.0).collect();
                assert_eq!(strings.len(), s.len(), "one note per string");
                assert_eq!(strings.last().unwrap() - strings.first().unwrap() + 1, s.len() as u8, "adjacent strings");
            }
        }
    }

    #[test]
    fn targets_are_within_reach() {
        // D on the A string, 5th fret: its 5th, A3, is on the D string at 7 or the G string at 2.
        assert_eq!(targets((4, 5), 7), [(2, 2), (3, 7)]);
        assert!(targets((4, 5), 7).iter().all(|&t| note(t) == 57));
        for semis in 1..=12 {
            let root = target_root(semis);
            assert!(!targets(root, semis).is_empty());
        }
    }
}
