use crate::settings::Settings;
use crate::theory::{Challenge, Kind, Spelled};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const HISTORY_LEN: usize = 16;
/// How long the last note stays in the menu bar after it stops.
const NOTE_LINGER: Duration = Duration::from_secs(3);

/// Challenge + countdown state machine. Time is passed in so it can be tested.
pub struct Engine {
    pub settings: Settings,
    pub challenge: Challenge,
    deadline: Instant,
    /// Set while the timer is frozen, by the user or by a call.
    paused_remaining: Option<Duration>,
    user_paused: bool,
    in_call: bool,
    overdue: bool,
    held: BTreeSet<u8>,
    /// Notes held down by clicking the on-screen fretboard (chord challenges only).
    clicked: BTreeSet<u8>,
    history: Vec<u8>,
    last_note: Option<u8>,
    /// When `last_note` stopped sounding; `None` while it's still held.
    last_note_off: Option<Instant>,
}

impl Engine {
    pub fn new(settings: Settings, now: Instant) -> Self {
        Self {
            challenge: Challenge::random(&settings.kinds(), None),
            deadline: now + settings.duration(),
            settings,
            paused_remaining: None,
            user_paused: false,
            in_call: false,
            overdue: false,
            held: BTreeSet::new(),
            clicked: BTreeSet::new(),
            history: Vec::new(),
            last_note: None,
            last_note_off: None,
        }
    }

    /// New challenge, timer restarted (stays paused if paused).
    pub fn next(&mut self, now: Instant) {
        self.challenge = Challenge::random(&self.settings.kinds(), Some(&self.challenge));
        self.overdue = false;
        self.history.clear();
        self.clear_clicks();
        let full = self.settings.duration();
        match self.paused_remaining {
            Some(_) => self.paused_remaining = Some(full),
            None => self.deadline = now + full,
        }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        self.paused_remaining.unwrap_or_else(|| self.deadline.saturating_duration_since(now))
    }

    pub fn paused(&self) -> bool {
        self.paused_remaining.is_some()
    }

    pub fn user_paused(&self) -> bool {
        self.user_paused
    }

    pub fn in_call(&self) -> bool {
        self.in_call
    }

    pub fn overdue(&self) -> bool {
        self.overdue
    }

    /// The notes played so far that are correct for the current challenge.
    pub fn progress(&self) -> Vec<Spelled> {
        self.challenge.progress(&self.held, &self.history)
    }

    pub fn held(&self) -> &BTreeSet<u8> {
        &self.held
    }

    /// The note most recently played, while it rings and briefly after.
    pub fn last_note(&self, now: Instant) -> Option<u8> {
        self.last_note.filter(|_| self.last_note_off.is_none_or(|off| now.saturating_duration_since(off) < NOTE_LINGER))
    }

    pub fn toggle_pause(&mut self, now: Instant) {
        self.user_paused = !self.user_paused;
        self.refreeze(now);
    }

    /// Returns true if the value changed.
    pub fn set_in_call(&mut self, in_call: bool, now: Instant) -> bool {
        let changed = self.in_call != in_call;
        self.in_call = in_call;
        self.refreeze(now);
        changed
    }

    fn refreeze(&mut self, now: Instant) {
        let frozen = self.user_paused || self.in_call;
        match (frozen, self.paused_remaining) {
            (true, None) => self.paused_remaining = Some(self.remaining(now)),
            (false, Some(left)) => {
                self.paused_remaining = None;
                self.deadline = now + left;
            }
            _ => {}
        }
    }

    /// Returns true on the tick the timer runs out.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.paused() || self.overdue || !self.remaining(now).is_zero() {
            return false;
        }
        self.overdue = true;
        true
    }

    /// Returns true if this note completed the challenge (a new one is already loaded).
    pub fn note_on(&mut self, note: u8, now: Instant) -> bool {
        self.held.insert(note);
        self.last_note = Some(note);
        self.last_note_off = None;
        self.history.push(note);
        if self.history.len() > HISTORY_LEN {
            self.history.remove(0);
        }
        let solved = self.challenge.is_satisfied(&self.held, &self.history);
        if solved {
            self.next(now);
        }
        solved
    }

    pub fn note_off(&mut self, note: u8, now: Instant) {
        self.held.remove(&note);
        if self.last_note == Some(note) {
            self.last_note_off = Some(now);
        }
    }

    /// A note clicked on the on-screen fretboard: plucked for intervals and scales, toggled held for
    /// chords. Returns true if it solved the challenge.
    pub fn click(&mut self, note: u8, now: Instant) -> bool {
        if !matches!(self.challenge.kind, Kind::Chord(_)) {
            let solved = self.note_on(note, now);
            self.note_off(note, now);
            return solved;
        }
        if self.clicked.remove(&note) {
            self.note_off(note, now);
            return false;
        }
        self.clicked.insert(note);
        self.note_on(note, now)
    }

    /// Lets go of every note held by clicking.
    pub fn clear_clicks(&mut self) {
        for note in std::mem::take(&mut self.clicked) {
            self.held.remove(&note);
        }
    }

    /// A chord heard from audio input, as pitch classes. Returns true if it solved the challenge.
    pub fn chord_heard(&mut self, pcs: &BTreeSet<u8>, now: Instant) -> bool {
        let solved = matches!(self.challenge.kind, Kind::Chord(_)) && self.challenge.is_satisfied(pcs, &[]);
        if solved {
            self.next(now);
        }
        solved
    }

    pub fn apply_settings(&mut self, settings: Settings, now: Instant) {
        self.settings = settings;
        self.next(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::ChordType;

    fn engine(now: Instant) -> Engine {
        let settings = Settings { chords: vec![ChordType::Major], intervals: vec![], scales: vec![], minutes: 5, ..Settings::default() };
        Engine::new(settings, now)
    }

    fn play_triad(e: &mut Engine, now: Instant) -> bool {
        let root = 60 + e.challenge.root;
        let mut solved = false;
        for n in [root, root + 4, root + 7] {
            solved = e.note_on(n, now);
        }
        for n in [root, root + 4, root + 7] {
            e.note_off(n, now);
        }
        solved
    }

    #[test]
    fn times_out_then_solving_resets() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        assert!(!e.tick(t0 + Duration::from_secs(299)));
        assert!(e.tick(t0 + Duration::from_secs(300)));
        assert!(!e.tick(t0 + Duration::from_secs(301)), "only fires once");
        assert!(e.overdue());

        let t1 = t0 + Duration::from_secs(400);
        assert!(play_triad(&mut e, t1));
        assert!(!e.overdue());
        assert_eq!(e.remaining(t1), Duration::from_secs(300));
        assert!(matches!(e.challenge.kind, Kind::Chord(ChordType::Major)));
    }

    #[test]
    fn pause_freezes_timer() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        e.toggle_pause(t0 + Duration::from_secs(100));
        assert!(!e.tick(t0 + Duration::from_secs(1000)));
        assert_eq!(e.remaining(t0 + Duration::from_secs(1000)), Duration::from_secs(200));
        e.toggle_pause(t0 + Duration::from_secs(1000));
        assert!(e.tick(t0 + Duration::from_secs(1200)));
    }

    #[test]
    fn call_pause_and_user_pause_stack() {
        let t0 = Instant::now();
        let s = |n| t0 + Duration::from_secs(n);
        let mut e = engine(t0);
        e.toggle_pause(s(10));
        assert!(e.set_in_call(true, s(20)));
        assert!(!e.set_in_call(true, s(25)), "no change");
        e.set_in_call(false, s(100));
        assert!(e.paused(), "call ending doesn't undo a manual pause");
        e.toggle_pause(s(100));
        assert_eq!(e.remaining(s(100)), Duration::from_secs(290));

        e.set_in_call(true, s(110));
        e.toggle_pause(s(120));
        e.toggle_pause(s(130));
        assert!(e.paused(), "still in the call");
        e.set_in_call(false, s(500));
        assert_eq!(e.remaining(s(500)), Duration::from_secs(280));
    }

    #[test]
    fn wrong_notes_dont_solve() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        let root = 60 + e.challenge.root;
        assert!(!e.note_on(root, t0));
        assert!(!e.note_on(root + 3, t0));
        assert!(!e.note_on(root + 7, t0));
    }

    #[test]
    fn heard_chord_solves_only_chord_challenges() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        let root = e.challenge.root;
        let minor: BTreeSet<u8> = [root, (root + 3) % 12, (root + 7) % 12].into();
        assert!(!e.chord_heard(&minor, t0));
        let major: BTreeSet<u8> = [root, (root + 4) % 12, (root + 7) % 12].into();
        assert!(e.chord_heard(&major, t0));
    }

    #[test]
    fn clicks_toggle_chord_notes_and_release_on_solve() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        let root = 48 + e.challenge.root;
        assert!(!e.click(root, t0));
        assert!(!e.click(root + 3, t0));
        assert!(!e.click(root + 3, t0), "clicking again lets go");
        assert!(!e.click(root + 7, t0));
        assert!(e.click(root + 16, t0), "major 3rd an octave up");
        assert!(e.held().is_empty(), "clicked notes released for the next challenge");
    }

    #[test]
    fn clicks_pluck_scale_notes() {
        let t0 = Instant::now();
        let settings = Settings { chords: vec![], intervals: vec![], scales: vec![crate::theory::ScaleType::Major], ..Settings::default() };
        let mut e = Engine::new(settings, t0);
        let mut note = 48 + e.challenge.root;
        assert!(!e.click(note, t0));
        for (i, step) in [2, 2, 1, 2, 2, 2, 1].into_iter().enumerate() {
            note += step;
            assert_eq!(e.click(note, t0), i == 6);
            assert!(e.held().is_empty());
        }
    }

    #[test]
    fn last_note_lingers_after_release() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        assert_eq!(e.last_note(t0), None);
        e.note_on(45, t0);
        assert_eq!(e.last_note(t0 + Duration::from_secs(60)), Some(45), "shown while held");
        e.note_on(52, t0);
        e.note_off(45, t0 + Duration::from_secs(1));
        assert_eq!(e.last_note(t0 + Duration::from_secs(30)), Some(52), "releasing an older note doesn't hide the newest");
        e.note_off(52, t0 + Duration::from_secs(1));
        assert_eq!(e.last_note(t0 + Duration::from_secs(2)), Some(52));
        assert_eq!(e.last_note(t0 + Duration::from_secs(5)), None);
    }
}
