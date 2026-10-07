//! Auto-update from GitHub Releases: checks shortly after launch and every few hours, installs in
//! the background, then restarts into the new version once you're not in the middle of something.

use serde::Serialize;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const FIRST_CHECK: Duration = Duration::from_secs(15);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Serialize, Clone, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Installing { version: String },
    /// Installed; takes effect on restart.
    Ready { version: String },
    Failed { error: String },
}

#[derive(Serialize, Clone)]
pub struct Info {
    current: String,
    #[serde(flatten)]
    status: Status,
}

pub struct Updates {
    status: Mutex<Status>,
    /// The user asked for this update, so don't wait for them to look away before restarting.
    restart_now: Mutex<bool>,
}

impl Updates {
    pub fn new() -> Self {
        Self { status: Mutex::new(Status::Idle), restart_now: Mutex::new(false) }
    }
}

fn state(app: &AppHandle) -> &Updates {
    app.state::<Updates>().inner()
}

fn current(app: &AppHandle) -> Status {
    state(app).status.lock().unwrap().clone()
}

pub fn info(app: &AppHandle) -> Info {
    Info { current: app.package_info().version.to_string(), status: current(app) }
}

fn set(app: &AppHandle, status: Status) {
    *state(app).status.lock().unwrap() = status;
    let _ = app.emit("update", info(app));
}

async fn download(app: &AppHandle) -> Result<Option<String>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(None) };
    if cfg!(debug_assertions) {
        return Err(format!("v{} is out, but dev builds don't install updates", update.version));
    }
    set(app, Status::Installing { version: update.version.clone() });
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    Ok(Some(update.version))
}

/// `manual` = the user clicked "Check for updates": report the outcome and restart as soon as allowed.
pub async fn check(app: AppHandle, manual: bool) {
    match current(&app) {
        Status::Checking | Status::Installing { .. } => return,
        Status::Ready { .. } => {
            *state(&app).restart_now.lock().unwrap() |= manual;
            return;
        }
        _ => {}
    }
    set(&app, Status::Checking);
    let status = match download(&app).await {
        Ok(Some(version)) => {
            *state(&app).restart_now.lock().unwrap() |= manual;
            Status::Ready { version }
        }
        Ok(None) if manual => Status::UpToDate,
        Err(error) if manual => Status::Failed { error },
        // Background checks stay quiet.
        Ok(None) | Err(_) => Status::Idle,
    };
    set(&app, status);
}

pub fn spawn(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK);
        loop {
            tauri::async_runtime::block_on(check(app.clone(), false));
            thread::sleep(CHECK_EVERY);
        }
    });
}

/// Called every tick. Never while the alarm is up (no escaping it by updating); otherwise right away
/// if the user asked, or as soon as the panel and settings are closed.
pub fn restart_if_ready(app: &AppHandle, overdue: bool, busy: bool) {
    let ready = matches!(current(app), Status::Ready { .. });
    let asked = *state(app).restart_now.lock().unwrap();
    if ready && !overdue && (asked || !busy) {
        app.restart();
    }
}
