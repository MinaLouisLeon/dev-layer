//! Turns shell window events into re-tiling passes.
//!
//! Events arrive in bursts — opening one window produces create, show,
//! location-change and foreground events, and dragging produces a location
//! change per frame — so everything is debounced into a single reconcile.
//!
//! Our own `SetWindowPos` calls also generate location-change events. That
//! feedback loop terminates because `WindowManager::place` skips a target it
//! has already asked for, so a self-triggered pass issues no moves and
//! produces no further events.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::monitors::MonitorInfo;
use crate::platform::sys::{self, WindowEvent};
use crate::wm::{tiling_region, ManagedWindow};
use crate::AppState;

pub const WINDOWS_EVENT: &str = "wm::windows";

/// Let a burst settle before re-tiling.
const DEBOUNCE: Duration = Duration::from_millis(120);
/// Safety net for anything the hook misses.
const IDLE_POLL: Duration = Duration::from_secs(2);

pub fn spawn_watcher(app: AppHandle) -> Result<()> {
    let (tx, rx) = mpsc::channel::<WindowEvent>();
    let watcher = sys::spawn_window_watcher(tx)?;

    let running = Arc::new(AtomicBool::new(true));
    let stop_flag = running.clone();
    crate::safety::register("window watcher", move || {
        stop_flag.store(false, Ordering::SeqCst);
        watcher.stop();
    });

    std::thread::Builder::new()
        .name("dev-layer/wm".into())
        .spawn(move || {
            let mut last_signature = String::new();

            while running.load(Ordering::SeqCst) {
                match rx.recv_timeout(IDLE_POLL) {
                    Ok(event) => {
                        apply_event(&app, event);
                        std::thread::sleep(DEBOUNCE);
                        while let Ok(event) = rx.try_recv() {
                            apply_event(&app, event);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                if !running.load(Ordering::SeqCst) {
                    break;
                }
                reconcile(&app, &mut last_signature);
            }
            tracing::debug!("window manager stopped");
        })
        .map_err(|e| crate::error::Error::Platform(e.to_string()))?;

    Ok(())
}

/// Foreground changes only update focus; they never re-tile.
fn apply_event(app: &AppHandle, event: WindowEvent) {
    if let WindowEvent::Foreground(hwnd) = event {
        app.state::<AppState>().wm.set_focused(Some(hwnd as i64));
    }
}

/// Forces an immediate reconcile and emit. Used by the commands, which change
/// layout state and want the result on screen without waiting for an event.
pub fn retile(app: &AppHandle) {
    let mut force = String::new();
    reconcile(app, &mut force);
}

pub fn reconcile(app: &AppHandle, last_signature: &mut String) {
    let state = app.state::<AppState>();
    let monitors = state.monitors.snapshot();
    if monitors.is_empty() {
        return;
    }

    // Reserved space comes from the HUD's own per-monitor context, so chrome
    // and tiling can never disagree about how much room the HUD takes.
    let reserved = |monitor: &MonitorInfo| {
        let context = state.hud.context(monitor, &state.config);
        tiling_region(monitor, context.reserved)
    };

    state.wm.reconcile(&monitors, reserved);

    let windows = state.wm.windows();
    let signature = signature_of(&windows);
    if signature == *last_signature {
        return;
    }
    *last_signature = signature;

    if let Err(e) = app.emit(WINDOWS_EVENT, &windows) {
        tracing::warn!(error = %e, "could not emit window list");
    }
}

/// Cheap change detection, so an idle desktop emits nothing.
fn signature_of(windows: &[ManagedWindow]) -> String {
    let mut signature = String::with_capacity(windows.len() * 48);
    for window in windows {
        signature.push_str(&format!(
            "{}:{}:{},{},{},{}:{}:{}:{};",
            window.id,
            window.monitor_id,
            window.rect.x,
            window.rect.y,
            window.rect.width,
            window.rect.height,
            window.floating as u8,
            window.focused as u8,
            window.slot.map(|s| s as i64).unwrap_or(-1),
        ));
    }
    signature
}
