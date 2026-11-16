//! Action mode: chords and controller pads/knobs/buttons mapped to things the Mac does.

use crate::theory::ChordType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long the held notes must stay the same before they're read as a chord, so the notes of a
/// chord pressed together (a few ms apart) don't fire whatever their first three make.
pub const SETTLE: Duration = Duration::from_millis(100);

/// A controller message that isn't a key: a knob or button (CC), a program change, or a pad that
/// sends a note. Channels are 0-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    Cc { channel: u8, number: u8 },
    Program { channel: u8, number: u8 },
    Note { channel: u8, note: u8 },
}

impl Control {
    pub fn label(self) -> String {
        match self {
            Control::Cc { channel, number } => format!("Control {number} · ch {}", channel + 1),
            Control::Program { channel, number } => format!("Program {} · ch {}", number + 1, channel + 1),
            Control::Note { channel, note } => format!("Pad {note} · ch {}", channel + 1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// Any chord of this type: any root, voicing or inversion.
    Chord { chord: ChordType },
    Control { control: Control },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    OpenApp { path: String },
}

impl Action {
    pub fn label(&self) -> String {
        match self {
            Action::OpenApp { path } => format!("Open {}", app_name(path)),
        }
    }

    pub fn run(&self) {
        match self {
            Action::OpenApp { path } => {
                let _ = std::process::Command::new("open").arg("-a").arg(path).spawn();
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub trigger: Trigger,
    pub action: Action,
}

/// "/Applications/Spotify.app" -> "Spotify".
pub fn app_name(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

/// The chord type these pitch classes make exactly, at some root.
pub fn chord_type(pcs: &BTreeSet<u8>) -> Option<ChordType> {
    ChordType::ALL.into_iter().find(|c| {
        (0..12).any(|root| c.intervals().iter().map(|i| (root + i) % 12).collect::<BTreeSet<u8>>() == *pcs)
    })
}

/// Apps to offer: everything in the usual Applications folders (one level of subfolders, for the
/// likes of /Applications/Utilities), sorted by name.
pub fn list_apps() -> Vec<String> {
    let mut dirs = vec![PathBuf::from("/Applications"), PathBuf::from("/System/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    let mut apps = Vec::new();
    for dir in dirs {
        collect_apps(&dir, 1, &mut apps);
    }
    apps.sort_by_key(|p| app_name(p).to_lowercase());
    apps.dedup();
    apps
}

fn collect_apps(dir: &Path, depth: u8, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "app") {
            out.push(path.to_string_lossy().into_owned());
        } else if depth > 0 && path.is_dir() {
            collect_apps(&path, depth - 1, out);
        }
    }
}

/// Reads notes and controller messages for triggers. Time-free so it can be tested: the caller
/// waits `SETTLE` after a note goes down, then asks `settled`.
#[derive(Default)]
pub struct Listener {
    held: BTreeSet<u8>,
    /// Bumped on every note change; a pending chord check is stale once it moves on.
    generation: u64,
    /// A chord fired since all notes were last let go: nothing more fires until they are.
    fired: bool,
    /// Last value of each knob/button, to fire on the way up through the middle.
    levels: HashMap<Control, u8>,
}

impl Listener {
    /// Returns the generation to pass to `settled` after `SETTLE`.
    pub fn note_on(&mut self, note: u8) -> u64 {
        self.held.insert(note);
        self.generation += 1;
        self.generation
    }

    pub fn note_off(&mut self, note: u8) {
        self.held.remove(&note);
        self.generation += 1;
        if self.held.is_empty() {
            self.fired = false;
        }
    }

    /// The chord held, if nothing changed since `generation` and nothing fired yet for this hand.
    pub fn settled(&mut self, generation: u64) -> Option<ChordType> {
        if generation != self.generation || self.fired {
            return None;
        }
        let chord = chord_type(&self.held.iter().map(|n| n % 12).collect())?;
        self.fired = true;
        Some(chord)
    }

    /// True when a control is pressed: a CC crossing from the bottom half to the top half (a button
    /// going down, a knob turned past the middle), any program change, any pad hit.
    pub fn control(&mut self, control: Control, value: u8) -> bool {
        match control {
            Control::Cc { .. } => {
                let before = self.levels.insert(control, value).unwrap_or(0);
                before < 64 && value >= 64
            }
            Control::Program { .. } | Control::Note { .. } => true,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcs(notes: &[u8]) -> BTreeSet<u8> {
        notes.iter().map(|n| n % 12).collect()
    }

    #[test]
    fn recognises_chord_types_in_any_key_and_voicing() {
        assert_eq!(chord_type(&pcs(&[60, 64, 67, 71])), Some(ChordType::Major7));
        assert_eq!(chord_type(&pcs(&[45, 61, 64, 68, 69])), Some(ChordType::Major7), "Amaj7 spread, doubled");
        assert_eq!(chord_type(&pcs(&[62, 65, 69])), Some(ChordType::Minor));
        assert_eq!(chord_type(&pcs(&[60, 64, 67, 71, 62])), None, "extra note");
        assert_eq!(chord_type(&pcs(&[60, 64])), None);
    }

    #[test]
    fn chord_fires_once_after_settling_until_released() {
        let mut l = Listener::default();
        l.note_on(60);
        l.note_on(64);
        let early = l.note_on(67);
        let last = l.note_on(71);
        assert_eq!(l.settled(early), None, "C major on the way to Cmaj7 doesn't fire");
        assert_eq!(l.settled(last), Some(ChordType::Major7));
        // Letting go of the 7th leaves a major triad: still the same hand, so nothing fires.
        l.note_off(71);
        let g = l.note_on(72);
        assert_eq!(l.settled(g), None);
        for n in [60, 64, 67, 72] {
            l.note_off(n);
        }
        l.note_on(62);
        l.note_on(65);
        let g = l.note_on(69);
        assert_eq!(l.settled(g), Some(ChordType::Minor));
    }

    #[test]
    fn released_before_settling_does_not_fire() {
        let mut l = Listener::default();
        l.note_on(60);
        l.note_on(64);
        let g = l.note_on(67);
        l.note_off(67);
        assert_eq!(l.settled(g), None);
    }

    #[test]
    fn knobs_and_buttons_fire_on_the_way_up() {
        let mut l = Listener::default();
        let knob = Control::Cc { channel: 0, number: 21 };
        assert!(!l.control(knob, 30));
        assert!(l.control(knob, 70));
        assert!(!l.control(knob, 90), "still up");
        assert!(!l.control(knob, 10));
        assert!(l.control(knob, 127));
        let button = Control::Cc { channel: 0, number: 64 };
        assert!(l.control(button, 127));
        assert!(!l.control(button, 0));
        assert!(l.control(Control::Note { channel: 9, note: 36 }, 100));
    }

    #[test]
    fn labels() {
        assert_eq!(Control::Note { channel: 9, note: 36 }.label(), "Pad 36 · ch 10");
        assert_eq!(Action::OpenApp { path: "/Applications/Spotify.app".into() }.label(), "Open Spotify");
    }
}
