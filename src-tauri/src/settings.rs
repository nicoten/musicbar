use crate::theory::{ChordType, Interval, Kind, ScaleType};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub chords: Vec<ChordType>,
    pub intervals: Vec<Interval>,
    pub scales: Vec<ScaleType>,
    pub minutes: u32,
    /// `None` listens to every MIDI input.
    pub midi_port: Option<String>,
    /// Listen to an audio input (guitar through an interface, or a mic) as well as MIDI.
    pub audio_enabled: bool,
    /// `None` uses the system default input.
    pub audio_device: Option<String>,
    pub sound: bool,
    pub pause_in_calls: bool,
    pub launch_at_login: bool,
    /// Show the clickable fretboard on the alarm screen.
    pub fretboard_on_alarm: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            chords: ChordType::ALL.to_vec(),
            intervals: Interval::ALL.to_vec(),
            scales: ScaleType::ALL.to_vec(),
            minutes: 5,
            midi_port: None,
            audio_enabled: true,
            audio_device: None,
            sound: true,
            pause_in_calls: true,
            launch_at_login: true,
            fretboard_on_alarm: true,
        }
    }
}

impl Settings {
    pub fn kinds(&self) -> Vec<Kind> {
        let chords = self.chords.iter().map(|&c| Kind::Chord(c));
        let intervals = self.intervals.iter().map(|&i| Kind::Interval(i));
        let scales = self.scales.iter().map(|&s| Kind::Scale(s));
        chords.chain(intervals).chain(scales).collect()
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.minutes.max(1) as u64 * 60)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.kinds().is_empty() {
            return Err("Pick at least one chord, interval or scale.".into());
        }
        if !(1..=240).contains(&self.minutes) {
            return Err("Timer must be between 1 and 240 minutes.".into());
        }
        Ok(())
    }

    /// Where to listen, in the shape `audio::spawn` wants.
    pub fn audio_input(&self) -> Option<Option<String>> {
        self.audio_enabled.then(|| self.audio_device.clone())
    }

    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}
