# MusicBar

Menu bar app that drills you on chords, intervals and scales over MIDI or your guitar (any audio
input). Play the challenge before the timer runs out, or a full-screen alarm takes over until you do.

- Chords: any voicing/inversion, exact pitch classes held together (MIDI), or strummed (audio).
- Intervals: root then the note above (sequential or together), ascending within an octave.
- Scales: one octave ascending from the root (melodic minor = ascending form).
- No instrument handy? Click the menu bar item and click the notes on a guitar neck: in order
  for intervals and scales, or click each chord tone to hold it, one per string (click again
  to let go). The fretboard is on the alarm screen too (Settings can hide it there).
- Clicked frets sound like a plucked string, and every solved challenge is played back: chords
  together, intervals one note then the other, scales up the octave. While that rings, audio input
  ignores new notes so the mic doesn't hear MusicBar as you.
- The last note you played shows next to the challenge in the menu bar, e.g. `♪ Dm7 · 4:32 │ A2`.
- Beeps every second for the last 10 seconds.
- Auto-pauses during calls: any camera on, or another app using the mic while Zoom/Teams/Webex/FaceTime is running.
- Launches at login (toggle in Settings; release builds only).
- Updates itself from GitHub Releases: checks shortly after launch and every 6 hours, installs in the
  background and restarts once the panel is closed (never while the alarm is up). "Check for
  updates" is at the bottom of the panel.
  The panel and alarm windows are excluded from screen capture; in a call the menu bar shows only `♪ ⏸`.

## Audio input

Single notes are tracked with YIN pitch detection (one note at a time, so intervals and scales work
from the guitar); strummed chords are matched against chord templates from a chromagram. Plug the
guitar into an audio interface and pick it under **Settings → Audio input**, or let the mic hear an
acoustic. Audio is analyzed live and never recorded. macOS asks for microphone access the first
time (in dev, the permission goes to your terminal app).

Settings are saved to `~/Library/Application Support/com.nicotejera.musicbar/settings.json`
(copied over from the old `com.nicotejera.musicblock` folder on first launch).

```sh
npm install
npm run tauri dev      # run
cd src-tauri && cargo test
npm run build          # signed .app / .dmg (use this, not `tauri build`: see below)
```

## Releasing

```sh
scripts/release.sh 0.2.0           # bump, build universal, commit, tag, push, publish to nicoten/musicbar
scripts/release.sh 0.2.0 --local   # bump and build only
```

Updates are signed with `~/.tauri/musicbar.key` (no password; back it up: losing it means installed
copies can't verify future updates). `npm run build` and the release script pass it explicitly
because the shell exports another app's `TAURI_SIGNING_PRIVATE_KEY`. Apple signing/notarization uses
the `APPLE_*` variables from the environment.
