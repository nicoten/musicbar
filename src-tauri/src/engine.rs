use crate::settings::Settings;
use crate::theory::{Challenge, Kind, Spelled};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// Enough for a scale played as a position across the fretboard.
const HISTORY_LEN: usize = 32;
/// How long the last note stays in the menu bar after it stops.
const NOTE_LINGER: Duration = Duration::from_secs(3);
/// How long a solved challenge stays up, with what you played, before the next one loads.
pub const CELEBRATE: Duration = Duration::from_secs(3);

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
    /// Notes held down by clicking the on-screen fretboard (chord challenges only), by string: one
    /// note per string, like a real guitar.
    clicked: BTreeMap<u8, u8>,
    /// Set once a note of this challenge is clicked on the fretboard: scales are then played as a
    /// whole position, not just an octave.
    on_fretboard: bool,
    history: Vec<u8>,
    last_note: Option<u8>,
    /// When `last_note` stopped sounding; `None` while it's still held.
    last_note_off: Option<Instant>,
    /// Set while celebrating a solved challenge: when it was solved and the notes that solved it.
    solved: Option<(Instant, Vec<Spelled>)>,
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
            clicked: BTreeMap::new(),
            on_fretboard: false,
            history: Vec::new(),
            last_note: None,
            last_note_off: None,
            solved: None,
        }
    }

    /// New challenge, timer restarted (stays paused if paused).
    pub fn next(&mut self, now: Instant) {
        self.challenge = Challenge::random(&self.settings.kinds(), Some(&self.challenge));
        self.overdue = false;
        self.solved = None;
        self.history.clear();
        self.on_fretboard = false;
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

    /// The notes played so far that are correct for the current challenge (all of them once solved).
    pub fn progress(&self) -> Vec<Spelled> {
        match &self.solved {
            Some((_, notes)) => notes.clone(),
            // Notes held while paused don't count, so they aren't shown as progress either.
            None if self.paused() => self.progress_with(&BTreeSet::new()),
            None => self.progress_with(&self.held),
        }
    }

    fn as_position(&self) -> bool {
        self.on_fretboard && matches!(self.challenge.kind, Kind::Scale(_))
    }

    fn progress_with(&self, held: &BTreeSet<u8>) -> Vec<Spelled> {
        if self.as_position() {
            self.challenge.position_progress(&self.history)
        } else {
            self.challenge.progress(held, &self.history)
        }
    }

    fn is_satisfied(&self) -> bool {
        if self.as_position() {
            self.challenge.position_played(&self.history)
        } else {
            self.challenge.is_satisfied(&self.held, &self.history)
        }
    }

    /// True for a moment after solving, before the next challenge loads.
    pub fn solved(&self) -> bool {
        self.solved.is_some()
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

    /// Returns true on the tick the timer runs out. Moves on once a solved challenge has been shown.
    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some((at, _)) = self.solved {
            if now.saturating_duration_since(at) >= CELEBRATE {
                self.next(now);
            }
            return false;
        }
        if self.paused() || self.overdue || !self.remaining(now).is_zero() {
            return false;
        }
        self.overdue = true;
        true
    }

    /// Returns true if this note completed the challenge (the next one loads after `CELEBRATE`).
    /// While paused a note still shows as played but doesn't count towards the challenge.
    pub fn note_on(&mut self, note: u8, now: Instant) -> bool {
        self.held.insert(note);
        self.last_note = Some(note);
        self.last_note_off = None;
        if self.paused() {
            return false;
        }
        self.history.push(note);
        if self.history.len() > HISTORY_LEN {
            self.history.remove(0);
        }
        let solved = self.solved.is_none() && self.is_satisfied();
        if solved {
            self.solve(now);
        }
        solved
    }

    fn solve(&mut self, now: Instant) {
        self.overdue = false;
        self.solved = Some((now, self.progress_with(&self.held)));
    }

    /// A note clicked on the fretboard stays held until clicked again, whatever the inputs say (the
    /// mic hears MusicBar pluck it and reports it stopping).
    pub fn note_off(&mut self, note: u8, now: Instant) {
        if !self.is_held_by_click(note) {
            self.held.remove(&note);
        }
        if self.last_note == Some(note) {
            self.last_note_off = Some(now);
        }
    }

    /// A note clicked on `string` of the on-screen fretboard: plucked for intervals and scales,
    /// toggled held for chords. A string holds one note, so a new note on it lets go of the old one.
    /// Returns true if it solved the challenge.
    pub fn click(&mut self, string: u8, note: u8, now: Instant) -> bool {
        if !matches!(self.challenge.kind, Kind::Chord(_)) {
            self.on_fretboard = true;
            let solved = self.note_on(note, now);
            self.note_off(note, now);
            return solved;
        }
        if let Some(old) = self.clicked.remove(&string) {
            self.note_off(old, now);
            if old == note {
                return false;
            }
        }
        // The same note held on another string moves here.
        self.clicked.retain(|_, n| *n != note);
        self.clicked.insert(string, note);
        self.note_on(note, now)
    }

    /// True if `note` is held down by a click on `string`, so clicking it again lets go.
    pub fn is_clicked(&self, string: u8, note: u8) -> bool {
        self.clicked.get(&string) == Some(&note)
    }

    fn is_held_by_click(&self, note: u8) -> bool {
        self.clicked.values().any(|&n| n == note)
    }

    /// Lets go of every note held by clicking.
    pub fn clear_clicks(&mut self) {
        for note in std::mem::take(&mut self.clicked).into_values() {
            self.held.remove(&note);
        }
    }

    /// A chord heard from audio input, as pitch classes. Returns true if it solved the challenge.
    pub fn chord_heard(&mut self, pcs: &BTreeSet<u8>, now: Instant) -> bool {
        let solved = self.solved.is_none()
            && !self.paused()
            && matches!(self.challenge.kind, Kind::Chord(_))
            && self.challenge.is_satisfied(pcs, &[]);
        if solved {
            self.solve(now);
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
        assert!(e.solved());
        let first = e.challenge;
        assert!(!play_triad(&mut e, t1), "can't solve twice while celebrating");
        e.tick(t1 + Duration::from_secs(1));
        assert_eq!(e.challenge, first, "solved challenge stays up for a moment");
        let t2 = t1 + CELEBRATE;
        e.tick(t2);
        assert!(!e.solved());
        assert_eq!(e.remaining(t2), Duration::from_secs(300));
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
    fn notes_played_while_paused_show_but_dont_count() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        let root = e.challenge.root;
        e.toggle_pause(t0);
        for n in [60 + root, 64 + root, 67 + root] {
            assert!(!e.note_on(n, t0), "a paused challenge can't be solved");
        }
        assert_eq!(e.held().len(), 3, "the notes still show as played");
        assert_eq!(e.last_note(t0), Some(67 + root));
        assert!(e.progress().is_empty());
        for n in [60 + root, 64 + root, 67 + root] {
            e.note_off(n, t0);
        }
        let major: BTreeSet<u8> = [root, (root + 4) % 12, (root + 7) % 12].into();
        assert!(!e.chord_heard(&major, t0));
        e.toggle_pause(t0);
        assert!(play_triad(&mut e, t0), "counts again once unpaused");
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
        assert!(!e.click(5, root, t0));
        assert!(!e.click(4, root + 3, t0));
        assert!(!e.click(4, root + 3, t0), "clicking again lets go");
        assert!(!e.click(3, root + 7, t0));
        assert!(e.click(2, root + 16, t0), "major 3rd an octave up");
        assert_eq!(e.held().len(), 3, "still showing while celebrating");
        assert_eq!(e.progress().len(), 3);
        e.tick(t0 + CELEBRATE);
        assert!(e.held().is_empty(), "clicked notes released for the next challenge");
    }

    #[test]
    fn input_note_off_doesnt_release_a_clicked_note() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        let root = 48 + e.challenge.root;
        e.click(5, root, t0);
        e.note_off(root, t0);
        assert!(e.held().contains(&root), "the mic hearing the pluck stop doesn't let go");
        e.click(5, root, t0);
        assert!(e.held().is_empty(), "clicking again does");
    }

    #[test]
    fn a_string_holds_one_clicked_note() {
        let t0 = Instant::now();
        let mut e = engine(t0);
        let root = 48 + e.challenge.root;
        e.click(5, root, t0);
        e.click(5, root + 4, t0);
        assert_eq!(e.held().len(), 1, "the new fret on the same string replaces the old one");
        assert!(e.held().contains(&(root + 4)));
        e.click(4, root + 4, t0);
        assert!(e.held().contains(&(root + 4)), "the same note moves to another string");
        assert!(!e.is_clicked(5, root + 4));
        e.click(4, root + 4, t0);
        assert!(e.held().is_empty());
    }

    fn major_scale_engine(t0: Instant) -> Engine {
        let settings = Settings { chords: vec![], intervals: vec![], scales: vec![crate::theory::ScaleType::Major], ..Settings::default() };
        Engine::new(settings, t0)
    }

    fn octave_up(root: u8) -> Vec<u8> {
        [0, 2, 2, 1, 2, 2, 2, 1].into_iter().scan(48 + root, |n, step| { *n += step; Some(*n) }).collect()
    }

    #[test]
    fn octave_of_scale_from_an_instrument() {
        let t0 = Instant::now();
        let mut e = major_scale_engine(t0);
        let notes = octave_up(e.challenge.root);
        for (i, &n) in notes.iter().enumerate() {
            assert_eq!(e.note_on(n, t0), i == 7);
        }
    }

    #[test]
    fn clicked_scale_needs_the_whole_position() {
        let t0 = Instant::now();
        let mut e = major_scale_engine(t0);
        for &n in &octave_up(e.challenge.root) {
            assert!(!e.click(0, n, t0), "an octave isn't the whole position");
            assert!(e.held().is_empty(), "clicks pluck");
        }
        // Root on the A string's fret r (3..=14): frets r-1..=r+3 on all strings.
        let r = (e.challenge.root + 12 - 9) % 12 + 3;
        let scale = [0, 2, 4, 5, 7, 9, 11].map(|s| (e.challenge.root + s) % 12);
        let mut position: Vec<u8> = [40u8, 45, 50, 55, 59, 64]
            .iter()
            .flat_map(|o| (r - 1..=r + 3).map(move |f| o + f))
            .filter(|n| scale.contains(&(n % 12)))
            .collect();
        position.sort();
        position.dedup();
        let last = position.len() - 1;
        for (i, &n) in position.iter().enumerate() {
            assert_eq!(e.click(0, n, t0), i == last);
        }
        assert_eq!(e.progress().len(), position.len());
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
