mod audio;
mod calls;
mod engine;
mod midi;
mod pitch;
mod settings;
mod synth;
mod theory;
mod updates;

use engine::Engine;
use midi::NoteEvent;
use pitch::AudioEvent;
use serde::Serialize;
use settings::Settings;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const TICK: Duration = Duration::from_millis(250);
const NAG_EVERY: Duration = Duration::from_secs(3);
const CALL_POLL: Duration = Duration::from_secs(2);
const WARN_SECS: u64 = 10;
const WARN_SOUND: &str = "/System/Library/Sounds/Tink.aiff";
/// Config dir name from before the app was renamed to MusicBar.
const OLD_CONFIG_DIR: &str = "com.nicotejera.musicblock";

struct Shared {
    engine: Mutex<Engine>,
    settings_path: PathBuf,
    panel_hidden_at: Mutex<Option<Instant>>,
}

impl Shared {
    fn engine(&self) -> MutexGuard<'_, Engine> {
        self.engine.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Serialize, Clone, PartialEq)]
struct Snapshot {
    category: &'static str,
    short: String,
    long: String,
    remaining_secs: u64,
    total_secs: u64,
    paused: bool,
    user_paused: bool,
    in_call: bool,
    overdue: bool,
    held: Vec<String>,
    /// Same as `held`, as MIDI note numbers.
    held_notes: Vec<u8>,
    fretboard_on_alarm: bool,
    /// How to name each pitch class (C = 0) for this challenge, e.g. D♯ rather than E♭ in B major.
    names: Vec<theory::PitchName>,
    /// Just solved: shown for a moment before the next challenge.
    solved: bool,
    /// The notes played correctly so far, for the staff.
    progress: Vec<theory::Spelled>,
    /// Last note played on any input, e.g. "A2".
    note: Option<String>,
}

fn snapshot(e: &Engine, now: Instant) -> Snapshot {
    Snapshot {
        category: e.challenge.category(),
        short: e.challenge.short(),
        long: e.challenge.long(),
        remaining_secs: e.remaining(now).as_millis().div_ceil(1000) as u64,
        total_secs: e.settings.duration().as_secs(),
        paused: e.paused(),
        user_paused: e.user_paused(),
        in_call: e.in_call(),
        overdue: e.overdue(),
        held: e.held().iter().map(|&n| e.challenge.note_name(n)).collect(),
        held_notes: e.held().iter().copied().collect(),
        fretboard_on_alarm: e.settings.fretboard_on_alarm,
        progress: e.progress(),
        solved: e.solved(),
        note: e.last_note(now).map(|n| e.challenge.note_name(n)),
        names: e.challenge.pitch_names(),
    }
}

fn tray_title(s: &Snapshot) -> String {
    if s.in_call {
        // Menu bar is visible when screen sharing: don't advertise anything.
        return "♪ ⏸".to_string();
    }
    let quiz = if s.overdue {
        format!("⚠ {}", s.short)
    } else if s.paused {
        format!("⏸ {}", s.short)
    } else {
        format!("♪ {} · {}:{:02}", s.short, s.remaining_secs / 60, s.remaining_secs % 60)
    };
    match &s.note {
        Some(note) => format!("{quiz} │ {note}"),
        None => quiz,
    }
}

/// Pushes the current state to the tray title and all windows.
fn publish(app: &AppHandle) -> Snapshot {
    let snap = snapshot(&app.state::<Shared>().inner().engine(), Instant::now());
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_title(Some(tray_title(&snap)));
    }
    let _ = app.emit("state", &snap);
    snap
}

fn show_alarm(app: &AppHandle) {
    let Some(w) = app.get_webview_window("alarm") else { return };
    if let Ok(Some(m)) = w.primary_monitor() {
        let _ = w.set_position(*m.position());
        let _ = w.set_size(*m.size());
    }
    let _ = w.show();
    let _ = w.set_always_on_top(true);
    let _ = w.set_focus();
}

fn hide_alarm(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("alarm") {
        let _ = w.hide();
    }
}

fn is_visible(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

fn play(sound: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("afplay").arg(sound).spawn();
}

fn spawn_ticker(app: AppHandle) {
    thread::spawn(move || {
        let mut last_nag = Instant::now();
        let mut last_snap: Option<Snapshot> = None;
        loop {
            thread::sleep(TICK);
            let (fired, sound) = {
                let mut e = app.state::<Shared>().inner().engine();
                (e.tick(Instant::now()), e.settings.sound)
            };
            let snap = snapshot(&app.state::<Shared>().inner().engine(), Instant::now());
            if last_snap.as_ref() != Some(&snap) {
                // Countdown beep once per second over the last few seconds.
                let new_second = last_snap.as_ref().is_none_or(|l| l.remaining_secs != snap.remaining_secs);
                if sound && new_second && !snap.paused && (1..=WARN_SECS).contains(&snap.remaining_secs) {
                    play(WARN_SOUND);
                }
                publish(&app);
                last_snap = Some(snap.clone());
            }
            // A solved alarm stays up (congratulating) until the next challenge loads.
            if !snap.overdue && !snap.solved && is_visible(&app, "alarm") {
                hide_alarm(&app);
            }
            let busy = is_visible(&app, "panel") || is_visible(&app, "settings");
            updates::restart_if_ready(&app, snap.overdue, busy);
            if !snap.overdue || snap.paused {
                continue;
            }
            // Keep shoving the alarm back in front (silently), unless you're using the panel/settings.
            if fired || (!busy && last_nag.elapsed() >= NAG_EVERY) {
                show_alarm(&app);
                last_nag = Instant::now();
            }
        }
    });
}

/// Registers/unregisters the login item. Skipped in dev so the debug binary never gets registered.
fn apply_launch_at_login(app: &AppHandle, on: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let launcher = app.autolaunch();
    if launcher.is_enabled().ok() != Some(on) {
        let _ = if on { launcher.enable() } else { launcher.disable() };
    }
}

fn spawn_call_watcher(app: AppHandle) {
    thread::spawn(move || loop {
        let enabled = app.state::<Shared>().inner().engine().settings.pause_in_calls;
        let in_call = enabled && calls::in_call();
        let changed = app.state::<Shared>().inner().engine().set_in_call(in_call, Instant::now());
        if changed {
            if in_call {
                hide_alarm(&app);
            }
            publish(&app);
        }
        thread::sleep(CALL_POLL);
    });
}

fn on_note(app: &AppHandle, ev: NoteEvent) {
    {
        let mut e = app.state::<Shared>().inner().engine();
        match ev {
            NoteEvent::On(n) => {
                if e.note_on(n, Instant::now()) {
                    synth::answer(&e.challenge);
                }
            }
            NoteEvent::Off(n) => e.note_off(n, Instant::now()),
        }
    }
    publish(app);
}

fn on_audio(app: &AppHandle, ev: AudioEvent) {
    match ev {
        // Don't take what MusicBar itself is playing (heard through the mic) for you playing.
        AudioEvent::Note(NoteEvent::On(_)) | AudioEvent::Chord(_) if synth::sounding() => {}
        AudioEvent::Note(n) => on_note(app, n),
        AudioEvent::Chord(pcs) => {
            let mut e = app.state::<Shared>().inner().engine();
            if e.chord_heard(&pcs, Instant::now()) {
                synth::answer(&e.challenge);
                drop(e);
                publish(app);
            }
        }
    }
}

/// Carries settings over from before the rename, once.
fn migrate_settings(path: &Path) {
    let old = path.parent().and_then(Path::parent).map(|d| d.join(OLD_CONFIG_DIR).join("settings.json"));
    if let Some(old) = old.filter(|old| old.exists() && !path.exists()) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::copy(old, path);
    }
}

fn toggle_panel(tray: &TrayIcon, rect: tauri::Rect) {
    let app = tray.app_handle();
    let Some(panel) = app.get_webview_window("panel") else { return };
    // A click on the tray blurs (and hides) the panel first; don't immediately reopen it.
    let just_hidden = app.state::<Shared>().panel_hidden_at.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if panel.is_visible().unwrap_or(false) || just_hidden {
        let _ = panel.hide();
        return;
    }
    let scale = panel.scale_factor().unwrap_or(1.0);
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    let width = panel.outer_size().map(|s| s.width as f64).unwrap_or(300.0 * scale);
    let mut x = pos.x + size.width / 2.0 - width / 2.0;
    // Keep the (wide) panel on the screen the tray icon is on.
    if let Ok(Some(m)) = panel.monitor_from_point(pos.x, pos.y) {
        let right = m.position().x as f64 + m.size().width as f64;
        x = x.min(right - width).max(m.position().x as f64);
    }
    let _ = panel.set_position(PhysicalPosition::new(x, pos.y + size.height));
    let _ = panel.show();
    let _ = panel.set_focus();
}

#[derive(Serialize)]
struct Choice {
    id: serde_json::Value,
    label: String,
}

#[derive(Serialize)]
struct Options {
    chords: Vec<Choice>,
    intervals: Vec<Choice>,
    scales: Vec<Choice>,
}

fn choice<T: Serialize>(id: T, label: String) -> Choice {
    Choice { id: serde_json::to_value(id).unwrap(), label }
}

#[tauri::command]
fn get_state(app: AppHandle) -> Snapshot {
    snapshot(&app.state::<Shared>().inner().engine(), Instant::now())
}

#[tauri::command]
fn toggle_pause(app: AppHandle) -> Snapshot {
    let paused = {
        let mut e = app.state::<Shared>().inner().engine();
        e.toggle_pause(Instant::now());
        e.paused()
    };
    if paused {
        hide_alarm(&app);
    }
    publish(&app)
}

#[tauri::command]
fn skip(app: AppHandle) -> Snapshot {
    {
        let mut e = app.state::<Shared>().inner().engine();
        // No escaping the alarm by skipping: you have to play it (or pause).
        if !e.overdue() {
            e.next(Instant::now());
        }
    }
    publish(&app)
}

#[tauri::command]
fn fret_click(app: AppHandle, note: u8) {
    {
        let mut e = app.state::<Shared>().inner().engine();
        // Clicking a held chord tone again lets go of it, silently.
        if !e.is_clicked(note) {
            synth::pluck(note);
        }
        if e.click(note, Instant::now()) {
            synth::answer(&e.challenge);
        }
    }
    publish(&app);
}

#[tauri::command]
fn fret_clear(app: AppHandle) {
    app.state::<Shared>().inner().engine().clear_clicks();
    publish(&app);
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    app.state::<Shared>().inner().engine().settings.clone()
}

#[tauri::command]
fn get_options() -> Options {
    use theory::{ChordType, Interval, ScaleType};
    Options {
        chords: ChordType::ALL.iter().map(|&c| choice(c, format!("C{} ({})", c.suffix(), c.name()))).collect(),
        intervals: Interval::ALL.iter().map(|&i| choice(i, format!("{} ({})", i.short(), i.name()))).collect(),
        scales: ScaleType::ALL.iter().map(|&s| choice(s, s.name().to_string())).collect(),
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings.validate()?;
    let shared = app.state::<Shared>();
    settings.save(&shared.settings_path)?;
    apply_launch_at_login(&app, settings.launch_at_login);
    shared.engine().apply_settings(settings, Instant::now());
    hide_alarm(&app);
    publish(&app);
    Ok(())
}

#[tauri::command]
fn list_midi_ports() -> Vec<String> {
    midi::list_ports()
}

#[tauri::command]
fn list_audio_inputs() -> Vec<String> {
    audio::list_inputs()
}

#[tauri::command]
fn get_update_info(app: AppHandle) -> updates::Info {
    updates::info(&app)
}

#[tauri::command]
fn check_for_updates(app: AppHandle) {
    tauri::async_runtime::spawn(updates::check(app, true));
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    if let Some(panel) = app.get_webview_window("panel") {
        let _ = panel.hide();
    }
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.emit("reload-settings", ());
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::new())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_path = app.path().app_config_dir()?.join("settings.json");
            migrate_settings(&settings_path);
            let settings = Settings::load(&settings_path);
            apply_launch_at_login(app.handle(), settings.launch_at_login);
            let engine = Engine::new(settings, Instant::now());
            let title = tray_title(&snapshot(&engine, Instant::now()));
            app.manage(Shared { engine: Mutex::new(engine), settings_path, panel_hidden_at: Mutex::new(None) });

            TrayIconBuilder::with_id("main")
                .title(title)
                .tooltip("MusicBar")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                        toggle_panel(tray, rect);
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            midi::spawn(
                {
                    let handle = handle.clone();
                    move || handle.state::<Shared>().inner().engine().settings.midi_port.clone()
                },
                {
                    let handle = handle.clone();
                    move |ev| on_note(&handle, ev)
                },
            );
            audio::spawn(
                {
                    let handle = handle.clone();
                    move || handle.state::<Shared>().inner().engine().settings.audio_input()
                },
                {
                    let handle = handle.clone();
                    move |ev| on_audio(&handle, ev)
                },
            );
            spawn_call_watcher(handle.clone());
            updates::spawn(handle.clone());
            spawn_ticker(handle);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                // Windows are reused; the alarm can't be closed at all.
                api.prevent_close();
                if window.label() != "alarm" {
                    let _ = window.hide();
                }
            }
            WindowEvent::Focused(false) if window.label() == "panel" => {
                let _ = window.hide();
                *window.state::<Shared>().panel_hidden_at.lock().unwrap() = Some(Instant::now());
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            toggle_pause,
            skip,
            get_settings,
            get_options,
            save_settings,
            list_midi_ports,
            list_audio_inputs,
            get_update_info,
            check_for_updates,
            open_settings,
            fret_click,
            fret_clear,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
