//! Keeps the topology snapshot honest.
//!
//! Two independent signals feed one debounced refresh:
//!   * the platform notifier (Windows `WM_DISPLAYCHANGE` / `WM_DPICHANGED`) —
//!     instant, but broadcast messages are easy to miss;
//!   * a slow poll — cheap insurance so a missed message costs seconds, not a
//!     session with a HUD stranded on a monitor that no longer exists.
//!
//! With 3–4 displays, docking/undocking and resolution changes are routine, so
//! this path is load-bearing rather than an edge case.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::platform::sys;
use crate::AppState;

pub const MONITORS_CHANGED_EVENT: &str = "monitors::changed";

/// Safety net interval when no platform notification arrives.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Display changes arrive as bursts; let them settle before re-enumerating.
const DEBOUNCE: Duration = Duration::from_millis(250);

pub fn spawn_watcher(app: AppHandle) -> Result<()> {
    let (tx, rx) = mpsc::channel::<()>();
    let notifier = sys::spawn_display_notifier(tx)?;

    let running = Arc::new(AtomicBool::new(true));
    let stop_flag = running.clone();
    crate::safety::register("monitor watcher", move || {
        stop_flag.store(false, Ordering::SeqCst);
        notifier.stop();
    });

    std::thread::Builder::new()
        .name("dev-layer/monitors".into())
        .spawn(move || {
            while running.load(Ordering::SeqCst) {
                match rx.recv_timeout(POLL_INTERVAL) {
                    // A display message arrived: let the burst settle, then drain.
                    Ok(()) => {
                        std::thread::sleep(DEBOUNCE);
                        while rx.try_recv().is_ok() {}
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                if !running.load(Ordering::SeqCst) {
                    break;
                }
                reconcile(&app);
            }
            tracing::debug!("monitor watcher stopped");
        })
        .map_err(|e| crate::error::Error::Platform(e.to_string()))?;

    Ok(())
}

fn reconcile(app: &AppHandle) {
    let state = app.state::<AppState>();

    let change = match state.monitors.refresh() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "monitor enumeration failed");
            return;
        }
    };
    if change.is_empty() {
        return;
    }

    tracing::info!(
        added = change.added.len(),
        removed = change.removed.len(),
        changed = change.changed.len(),
        "topology changed"
    );

    let monitors = state.monitors.snapshot();
    if let Err(e) = state.hud.reconcile(app, &monitors, &state.config) {
        tracing::error!(error = %e, "HUD reconcile failed");
    }
    if let Err(e) = app.emit(MONITORS_CHANGED_EVENT, &change) {
        tracing::warn!(error = %e, "could not emit topology change");
    }
}
