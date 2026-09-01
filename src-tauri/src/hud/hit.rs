//! Which parts of a HUD window take the mouse, and which let it through.
//!
//! The HUD covers a whole monitor, so without this it swallows every click
//! that does not land on another app's window — including double-clicks on
//! desktop icons. Tauri can only toggle click-through for a window as a whole,
//! so a poller watches the cursor and flips the flag as it crosses between
//! painted chrome and the transparent parts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::geometry::Insets;
use crate::platform::sys;
use crate::AppState;

/// Fast enough that the flip is never noticed, cheap enough to ignore:
/// one `GetCursorPos` call.
const POLL: Duration = Duration::from_millis(60);

/// True when a monitor-local, logical-pixel point is over painted HUD chrome.
///
/// The gutter is checked first and wins over everything: it is the strip the
/// desktop icons live in, and nothing the HUD draws may take a click there.
pub fn is_interactive(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    reserved: Insets,
    gutter: i32,
) -> bool {
    if x < f64::from(gutter) {
        return false;
    }
    if y < f64::from(reserved.top) {
        return true;
    }
    if y >= height - f64::from(reserved.bottom) {
        return true;
    }
    if x < f64::from(reserved.left) {
        return true;
    }
    if reserved.right > 0 && x >= width - f64::from(reserved.right) {
        return true;
    }
    // The window region: tiled app windows sit here, and where none does the
    // desktop should be reachable.
    false
}

pub fn spawn_pointer_watcher(app: AppHandle) -> crate::error::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    crate::safety::register("pointer watcher", move || {
        stop.store(false, Ordering::SeqCst)
    });

    std::thread::Builder::new()
        .name("dev-layer/pointer".into())
        .spawn(move || {
            // Remembering the last state per window keeps this to one Win32
            // call per tick on an idle desktop.
            let mut applied: std::collections::HashMap<String, bool> =
                std::collections::HashMap::new();

            while running.load(Ordering::SeqCst) {
                std::thread::sleep(POLL);
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let Some((cursor_x, cursor_y)) = sys::cursor_position() else {
                    continue;
                };

                let state = app.state::<AppState>();
                for monitor in state.monitors.snapshot() {
                    let Some(label) = state.hud.label_for_monitor(&monitor.id) else {
                        continue;
                    };
                    let context = state.hud.context(&monitor, &state.config);

                    let inside = cursor_x >= monitor.bounds.x
                        && cursor_x < monitor.bounds.right()
                        && cursor_y >= monitor.bounds.y
                        && cursor_y < monitor.bounds.bottom();

                    // An open panel is raised above the app windows and must
                    // take the mouse everywhere, wherever the cursor is.
                    let interactive = state.hud.overlay_open(&label)
                        || (inside
                            && is_interactive(
                                f64::from(cursor_x - monitor.bounds.x) / monitor.scale_factor,
                                f64::from(cursor_y - monitor.bounds.y) / monitor.scale_factor,
                                f64::from(monitor.bounds.width) / monitor.scale_factor,
                                f64::from(monitor.bounds.height) / monitor.scale_factor,
                                context.reserved,
                                context.desktop_gutter,
                            ));

                    if applied.get(&label) == Some(&interactive) {
                        continue;
                    }
                    if let Some(window) = app.get_webview_window(&label) {
                        if window.set_ignore_cursor_events(!interactive).is_ok() {
                            applied.insert(label, interactive);
                        }
                    }
                }
            }
            tracing::debug!("pointer watcher stopped");
        })
        .map_err(|e| crate::error::Error::Platform(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESERVED: Insets = Insets {
        top: 34,
        right: 0,
        bottom: 92,
        left: 400,
    };
    const GUTTER: i32 = 120;
    const W: f64 = 1280.0;
    const H: f64 = 800.0;

    fn hit(x: f64, y: f64) -> bool {
        is_interactive(x, y, W, H, RESERVED, GUTTER)
    }

    #[test]
    fn the_desktop_icon_gutter_never_takes_a_click() {
        // Top-left, where "This PC" sits — and all the way down the strip,
        // including alongside the top and bottom rails.
        assert!(!hit(40.0, 10.0));
        assert!(!hit(40.0, 400.0));
        assert!(!hit(40.0, H - 10.0));
        assert!(!hit(119.9, 400.0));
    }

    #[test]
    fn the_rails_take_clicks() {
        assert!(hit(300.0, 200.0), "left rail");
        assert!(hit(700.0, 10.0), "top rail");
        assert!(hit(700.0, H - 10.0), "bottom rail: the dock lives here");
        assert!(hit(120.0, 400.0), "first pixel after the gutter is rail");
    }

    #[test]
    fn the_window_region_passes_clicks_through() {
        assert!(!hit(700.0, 400.0));
        assert!(!hit(RESERVED.left as f64, 400.0), "just inside the region");
        assert!(!hit(W - 1.0, H - RESERVED.bottom as f64 - 1.0));
    }

    #[test]
    fn a_monitor_without_a_gutter_still_has_reachable_desktop() {
        // Secondary displays get no gutter: the region must still pass through.
        let reserved = Insets {
            top: 34,
            right: 0,
            bottom: 34,
            left: 0,
        };
        assert!(!is_interactive(600.0, 400.0, W, H, reserved, 0));
        assert!(is_interactive(600.0, 10.0, W, H, reserved, 0));
    }
}
