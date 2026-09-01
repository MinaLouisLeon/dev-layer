//! The command bus: the single surface through which anything asks dev-layer
//! to do something.
//!
//! Today the callers are the HUD windows over Tauri IPC. From milestone 6 the
//! AI/voice layer resolves natural language onto these same commands, so every
//! new capability belongs here rather than in ad-hoc frontend logic.

pub mod registry;

use tauri::{AppHandle, Manager, State};

use crate::apps::AppEntry;
use crate::config::Config;
use crate::hud::{HudContext, LABEL_PREFIX};
use crate::metrics::MetricsSnapshot;
use crate::monitors::MonitorInfo;
use crate::panels::{HttpRequest, HttpResponse, RequestHistory, ShellProbe};
use crate::wm::{LayoutKind, ManagedWindow, MonitorLayout};
use crate::AppState;

#[tauri::command]
pub fn list_monitors(state: State<'_, AppState>) -> Vec<MonitorInfo> {
    state.monitors.snapshot()
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Config {
    state.config.clone()
}

/// Resolves which monitor a HUD window is showing on, from its own label.
#[tauri::command]
pub fn hud_context(label: String, state: State<'_, AppState>) -> Result<HudContext, String> {
    if !label.starts_with(LABEL_PREFIX) {
        return Err(format!("{label} is not a HUD window"));
    }
    let monitor_id = state
        .hud
        .monitor_id_for_label(&label)
        .ok_or_else(|| format!("no monitor mapped to {label}"))?;
    let monitor = state
        .monitors
        .get(&monitor_id)
        .ok_or_else(|| format!("monitor {monitor_id} is gone"))?;

    Ok(state.hud.context(&monitor, &state.config))
}

/// The most recent sample, so a HUD window opened mid-session paints
/// immediately instead of waiting for the next tick.
#[tauri::command]
pub fn latest_metrics(state: State<'_, AppState>) -> Option<MetricsSnapshot> {
    state.metrics.latest()
}

/// Static machine facts (CPU model, core counts, OS), fetched once.
#[tauri::command]
pub fn host_info() -> crate::metrics::HostInfo {
    crate::metrics::host_info()
}

/// Everything the dock and launcher can open. Empty until the background
/// scan finishes; the `apps::catalog` event carries the filled-in list.
#[tauri::command]
pub fn list_apps(state: State<'_, AppState>) -> Vec<AppEntry> {
    state.apps.entries()
}

/// Launches through the shell, so the shortcut's own arguments, working
/// directory and elevation behaviour all apply.
#[tauri::command]
pub fn launch_app(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let entry = state
        .apps
        .get(&id)
        .ok_or_else(|| format!("unknown app {id}"))?;
    tracing::info!(app = %entry.name, "launching");

    crate::platform::sys::launch(&entry.launch_path, "", None).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_pinned(app: AppHandle, id: String, pinned: bool) -> Result<Vec<AppEntry>, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    state
        .apps
        .set_pinned(&id, pinned, &config_dir)
        .ok_or_else(|| format!("unknown app {id}"))?;
    Ok(state.apps.entries())
}

/// Rescans the Start Menu. Cheap on a warm icon cache, seconds on a cold one.
#[tauri::command]
pub fn refresh_apps(app: AppHandle) {
    crate::apps::spawn_scan(app);
}

// ------------------------------------------------------------ window manager

#[tauri::command]
pub fn list_windows(state: State<'_, AppState>) -> Vec<ManagedWindow> {
    state.wm.windows()
}

#[tauri::command]
pub fn window_layouts(state: State<'_, AppState>) -> Vec<MonitorLayout> {
    state.wm.layouts()
}

#[tauri::command]
pub fn set_window_layout(app: AppHandle, monitor_id: String, kind: LayoutKind) {
    app.state::<AppState>().wm.set_layout(&monitor_id, kind);
    crate::wm::retile(&app);
}

#[tauri::command]
pub fn cycle_window_layout(app: AppHandle, monitor_id: String) -> LayoutKind {
    let kind = app.state::<AppState>().wm.cycle_layout(&monitor_id);
    crate::wm::retile(&app);
    kind
}

#[tauri::command]
pub fn focus_window(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.wm.set_focused(Some(id));
    crate::platform::sys::focus_window(id as isize).map_err(|e| e.to_string())
}

/// Takes a window out of tiling (restoring where the user had it) or back in.
#[tauri::command]
pub fn toggle_window_float(app: AppHandle, id: i64) -> Result<bool, String> {
    let floating = app
        .state::<AppState>()
        .wm
        .toggle_float(id)
        .ok_or_else(|| format!("unknown window {id}"))?;
    crate::wm::retile(&app);
    Ok(floating)
}

/// Moves a window to the front of the tiling order — the main pane.
#[tauri::command]
pub fn promote_window(app: AppHandle, id: i64) {
    app.state::<AppState>().wm.promote(id);
    crate::wm::retile(&app);
}

#[tauri::command]
pub fn close_window(id: i64) -> Result<(), String> {
    crate::platform::sys::close_window(id as isize).map_err(|e| e.to_string())
}

/// Turning tiling off restores every window we moved, immediately.
#[tauri::command]
pub fn set_wm_enabled(app: AppHandle, enabled: bool) {
    app.state::<AppState>().wm.set_enabled(enabled);
    crate::wm::retile(&app);
}

#[tauri::command]
pub fn retile(app: AppHandle) {
    crate::wm::retile(&app);
}

/// Lifts one HUD window above the tiled apps, for the launcher and other
/// panels — the HUD normally sits *below* every app window, so an overlay
/// opened down there would be invisible.
#[tauri::command]
pub fn set_hud_overlay(app: AppHandle, label: String, on: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("no window {label}"))?;

    // The pointer watcher reads this: an open panel takes the mouse over the
    // whole window, not just over the rails.
    app.state::<AppState>().hud.set_overlay_open(&label, on);

    if on {
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
        // Focus so the launcher's search field can actually receive typing.
        let _ = window.set_focus();
    } else {
        window.set_always_on_top(false).map_err(|e| e.to_string())?;
        window
            .set_always_on_bottom(true)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// -------------------------------------------------------------- panels

#[tauri::command]
pub fn terminal_open(
    app: AppHandle,
    cols: u16,
    rows: u16,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let shell = state.config.panels.shell.clone();
    let cwd = state
        .config
        .panels
        .startup_dir
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(|| app.path().home_dir().ok());

    state
        .terminals
        .open(&app, cols, rows, shell, cwd)
        .map_err(|e| e.to_string())
}

/// What the next terminal will run, and whether nushell was found. The panel
/// shows this so a fallback is never silent.
#[tauri::command]
pub fn terminal_shell(state: State<'_, AppState>) -> ShellProbe {
    crate::panels::probe_shell(state.config.panels.shell.as_deref())
}

#[tauri::command]
pub fn terminal_write(id: String, data: String, state: State<'_, AppState>) -> Result<(), String> {
    state.terminals.write(&id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_resize(
    id: String,
    cols: u16,
    rows: u16,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .terminals
        .resize(&id, cols, rows)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_close(id: String, state: State<'_, AppState>) {
    state.terminals.close(&id);
}

/// Sends one HTTP request and records it in history. Async so a slow endpoint
/// never blocks the HUD.
#[tauri::command]
pub async fn http_send(app: AppHandle, request: HttpRequest) -> Result<HttpResponse, String> {
    let response = crate::panels::send_request(request.clone())
        .await
        .map_err(|e| e.to_string())?;

    if let Ok(dir) = app.path().app_config_dir() {
        let mut history = RequestHistory::load(&dir);
        history.record(&request);
        if let Err(e) = history.save(&dir) {
            tracing::warn!(error = %e, "could not save request history");
        }
    }
    Ok(response)
}

#[tauri::command]
pub fn http_history(app: AppHandle) -> Vec<HttpRequest> {
    app.path()
        .app_config_dir()
        .map(|dir| RequestHistory::load(&dir).entries)
        .unwrap_or_default()
}

// ------------------------------------------------------------- agent

/// One natural-language turn. Progress arrives as `agent::event`; this returns
/// the final answer text.
#[tauri::command]
pub async fn agent_ask(app: AppHandle, prompt: String) -> Result<String, String> {
    if !app.state::<AppState>().config.agent.enabled {
        return Err("the command layer is disabled in config".into());
    }
    crate::agent::run(app, prompt).await
}

#[tauri::command]
pub fn agent_status(app: AppHandle) -> crate::agent::AgentStatus {
    crate::agent::status(&app)
}

#[tauri::command]
pub fn agent_reset(state: State<'_, AppState>) {
    state.agent.reset();
}

// ---------------------------------------------------------- workbench

#[tauri::command]
pub fn workbench_state(state: State<'_, AppState>) -> crate::workbench::WorkbenchState {
    state.workbench.state()
}

/// Roots the workbench at a folder. Defaults to the home directory, which is
/// what the panel asks for when it has no folder yet.
#[tauri::command]
pub async fn workbench_open(
    app: AppHandle,
    path: Option<String>,
) -> Result<crate::workbench::WorkbenchState, String> {
    let root = match path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        Some(path) => path,
        None => app
            .path()
            .home_dir()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
    };
    app.state::<AppState>().workbench.open(root).await
}

#[tauri::command]
pub async fn workbench_list_dir(
    app: AppHandle,
    path: String,
) -> Result<Vec<mino_core::types::DirEntry>, String> {
    let workbench = &app.state::<AppState>().workbench;
    workbench.list_dir(path).await
}

#[tauri::command]
pub async fn workbench_read_file(
    app: AppHandle,
    path: String,
) -> Result<mino_core::types::FilePayload, String> {
    let workbench = &app.state::<AppState>().workbench;
    workbench.read_file(path).await
}

/// Restores the desktop and quits. The only intended way out, and the same
/// path the global exit hotkey takes.
#[tauri::command]
pub fn shutdown(app: AppHandle) {
    tracing::info!("shutdown requested");
    crate::shutdown(&app);
}

/// Forces a topology re-read. Useful when a display change slips past both the
/// notifier and the poll.
#[tauri::command]
pub fn refresh_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let state = app.state::<AppState>();
    state.monitors.refresh().map_err(|e| e.to_string())?;
    let monitors = state.monitors.snapshot();
    state
        .hud
        .reconcile(&app, &monitors, &state.config)
        .map_err(|e| e.to_string())?;
    Ok(monitors)
}
