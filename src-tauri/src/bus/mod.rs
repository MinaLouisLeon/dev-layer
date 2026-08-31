//! The command bus: the single surface through which anything asks dev-layer
//! to do something.
//!
//! Today the callers are the HUD windows over Tauri IPC. From milestone 6 the
//! AI/voice layer resolves natural language onto these same commands, so every
//! new capability belongs here rather than in ad-hoc frontend logic.

use tauri::{AppHandle, Manager, State};

use crate::config::Config;
use crate::hud::{HudContext, LABEL_PREFIX};
use crate::metrics::MetricsSnapshot;
use crate::monitors::MonitorInfo;
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
