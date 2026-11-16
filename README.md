# MusicBar

Menu bar app that drills you on chords, intervals, scales and arpeggios over MIDI or your guitar (any audio
input). Play the challenge before the timer runs out, or a full-screen alarm takes over until you do.

It has two modes, picked at the top of Settings (one at a time):

- **Action** (the default): chords and controller pads, knobs and buttons run things on your Mac.
- **Quiz**: the timed challenges described below.

## Action mode

Map a trigger to an action under **Settings → Actions**. For now the action is always "open an app" (any
app in /Applications, /System/Applications or ~/Applications).

- **Chord triggers** fire for a chord type in any key, voicing or inversion: "any maj7" fires for Cmaj7,
  an F♯maj7 in second inversion, and so on. They work from MIDI or a strummed guitar (audio input). The notes
  have to hold still for 100 ms, so Cmaj7 doesn't first fire "major" on its way there. Each chord fires once,
  and nothing else fires until you let go of every note.
- **Controller triggers** come from pads, knobs and buttons, never from keys. Pick "Pad, knob or button…",
  click **Learn** and touch the control. Buttons fire when pressed. Knobs fire when turned past halfway, and
  fire again only after going back below it. Pads that send notes are recorded with their channel, and then
  only fire their trigger and never count toward a chord.
- No challenges, timer or alarm. The menu bar shows `♪` and the last note played. The panel shows what
  fired, e.g. `maj7 → Open Spotify`, and the fretboard and tuner work as usual.

## Quiz mode

- Chords: any voicing/inversion, exact pitch classes held together (MIDI), or strummed (audio).
- Intervals: root then the note above (sequential or together), ascending within an octave.
- Scales: one octave ascending from the root (melodic minor = ascending form).
- Arpeggios: 7, m7, maj7 and ø7 (half-diminished) chords, tones one after another in a given order (1-3-5-7, 3-5-7-1,
  7-5-3-1… all 24 orders, pick which in Settings), any octave.
- No instrument handy? Click the menu bar item and click the notes on a guitar neck: in order
  for intervals, scales and arpeggios, or click each chord tone to hold it, one per string (click again
  to let go). The fretboard is on the alarm screen too (Settings can hide it there).
- Fretboard modes (Settings, pick any; each challenge uses one of them):
  - **Free**: any fret, as above.
  - **Fret window**: the challenge comes with a 5-fret window (e.g. `Dm7 · frets 6–10`); only those frets
    take clicks, so you find it in one position. Windows always fit the challenge.
  - **Name the shape**: a chord voicing shows as dots with the root ringed. Pick the root and the type
    (from the chords and arpeggios you picked) by clicking, or type it: a letter, then `#` or `b`, then the
    type's number, shown on its button (`e` `b` then the number on `m7` = E♭m7). Playing it on an
    instrument doesn't count.
  - **Target notes**: a named root lights up; click the interval asked for (your intervals plus 3rds, 5th
    and 7ths) within 4 frets of it, on any string. Five in a row solve it.
- Clicked frets sound like a plucked string, and every solved challenge is played back: chords
  together, intervals one note then the other, scales up the octave. While that rings, audio input
  ignores new notes so the mic doesn't hear MusicBar as you. Silent during calls.
- The last note you played shows next to the challenge in the menu bar, e.g. `♪ Dm7 · 4:32 │ A2`.
- Beeps every second for the last 10 seconds.
- Auto-pauses during calls: any camera on, or another app using the mic while Zoom/Teams/Webex/FaceTime is running.
  Pausing (in a call or by hand) only stops the timer: you can still answer challenges.
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
