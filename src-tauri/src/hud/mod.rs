//! HUD windows: exactly one per monitor, created and destroyed as the
//! topology changes.
//!
//! Each HUD is undecorated, transparent, skips the taskbar, and sits
//! **always-on-bottom**: app windows launched later stack above it, inside the
//! region the HUD reserves. That is what makes apps look embedded without
//! reparenting anyone's HWND.

use std::collections::HashMap;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::geometry::Insets;
use crate::monitors::MonitorInfo;

pub mod hit;

pub const LABEL_PREFIX: &str = "hud--";

/// What a HUD window needs to know about itself, resolved by window label.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudContext {
    pub monitor: MonitorInfo,
    /// Total logical-pixel margins the HUD occupies, gutter included. This is
    /// what the window manager must keep clear.
    pub reserved: Insets,
    /// Width of the untouched strip on the left. Chrome starts after it.
    pub desktop_gutter: i32,
    pub chrome: ChromeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChromeMode {
    /// Full rails: topology, dock, gauges. Primary monitor by default.
    Full,
    /// Just the frame and status strip — keeps secondary displays for work.
    Minimal,
}

/// Tauri window labels allow a restricted character set, but monitor ids look
/// like `\\.\DISPLAY1`. We sanitize for the label and keep the mapping here.
#[derive(Default)]
pub struct HudManager {
    /// monitor id -> window label
    windows: Mutex<HashMap<String, String>>,
    /// Labels whose overlay panel is open, and which therefore take the mouse
    /// everywhere rather than only over their chrome.
    overlays: Mutex<std::collections::HashSet<String>>,
}

impl HudManager {
    /// Brings HUD windows in line with the current monitor set: creates missing
    /// ones, re-places existing ones, closes orphans.
    pub fn reconcile(
        &self,
        app: &AppHandle,
        monitors: &[MonitorInfo],
        config: &Config,
    ) -> Result<()> {
        let mut windows = self.windows.lock();

        for monitor in monitors {
            let label = windows
                .entry(monitor.id.clone())
                .or_insert_with(|| label_for(&monitor.id))
                .clone();

            match app.get_webview_window(&label) {
                Some(window) => place(&window, monitor)?,
                None => {
                    let window = build(app, &label, monitor, config)?;
                    place(&window, monitor)?;
                    tracing::info!(label, monitor = %monitor.id, "HUD window created");
                }
            }
        }

        // Monitors that went away take their HUD with them.
        windows.retain(|monitor_id, label| {
            let alive = monitors.iter().any(|m| &m.id == monitor_id);
            if !alive {
                if let Some(window) = app.get_webview_window(label) {
                    let _ = window.close();
                }
                tracing::info!(label, monitor = %monitor_id, "HUD window closed");
            }
            alive
        });

        Ok(())
    }

    pub fn monitor_id_for_label(&self, label: &str) -> Option<String> {
        self.windows
            .lock()
            .iter()
            .find(|(_, l)| l.as_str() == label)
            .map(|(id, _)| id.clone())
    }

    pub fn context(&self, monitor: &MonitorInfo, config: &Config) -> HudContext {
        let chrome = if monitor.is_primary || config.hud.full_chrome_on_secondary {
            ChromeMode::Full
        } else {
            ChromeMode::Minimal
        };
        let mut reserved = match chrome {
            ChromeMode::Full => config.hud.reserved,
            ChromeMode::Minimal => config.hud.reserved_minimal,
        };

        // Only the primary display carries desktop icons, so only the primary
        // gives up a gutter for them.
        let desktop_gutter = if monitor.is_primary {
            config.hud.desktop_gutter.max(0)
        } else {
            0
        };
        reserved.left += desktop_gutter;

        HudContext {
            monitor: monitor.clone(),
            reserved,
            desktop_gutter,
            chrome,
        }
    }

    pub fn label_for_monitor(&self, monitor_id: &str) -> Option<String> {
        self.windows.lock().get(monitor_id).cloned()
    }

    /// An open panel is raised above the tiled windows, so its HUD window has
    /// to take the mouse everywhere rather than only over its chrome.
    pub fn set_overlay_open(&self, label: &str, open: bool) {
        let mut overlays = self.overlays.lock();
        if open {
            overlays.insert(label.to_string());
        } else {
            overlays.remove(label);
        }
    }

    pub fn overlay_open(&self, label: &str) -> bool {
        self.overlays.lock().contains(label)
    }
}

fn label_for(monitor_id: &str) -> String {
    let slug: String = monitor_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("{LABEL_PREFIX}{}", slug.trim_matches('-'))
}

fn build(
    app: &AppHandle,
    label: &str,
    monitor: &MonitorInfo,
    config: &Config,
) -> Result<tauri::WebviewWindow> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(format!("dev-layer :: {}", monitor.name))
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        // Never steal focus: creating a HUD must not interrupt typing.
        .focused(false)
        // Positioned explicitly right after creation, in physical pixels.
        .visible(false)
        .inner_size(
            config.hud.reserved.left as f64 + 640.0,
            config.hud.reserved.top as f64 + 480.0,
        )
        .build()
        .map_err(|e| Error::Hud(format!("could not create {label}: {e}")))
}

/// Places a HUD over the whole monitor, in physical pixels.
///
/// Physical coordinates are the only unambiguous currency on a mixed-DPI
/// multi-monitor desktop: logical coordinates are relative to a scale factor
/// that differs per display.
fn place(window: &tauri::WebviewWindow, monitor: &MonitorInfo) -> Result<()> {
    let b = monitor.bounds;
    window
        .set_position(PhysicalPosition::new(b.x, b.y))
        .map_err(|e| Error::Hud(format!("position: {e}")))?;
    window
        .set_size(PhysicalSize::new(
            b.width.max(1) as u32,
            b.height.max(1) as u32,
        ))
        .map_err(|e| Error::Hud(format!("size: {e}")))?;
    // Below every app window, above the desktop.
    window
        .set_always_on_bottom(true)
        .map_err(|e| Error::Hud(format!("z-order: {e}")))?;
    window
        .show()
        .map_err(|e| Error::Hud(format!("show: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_tauri_safe_and_unique_per_display() {
        assert_eq!(label_for(r"\\.\DISPLAY1"), "hud--DISPLAY1");
        assert_ne!(label_for(r"\\.\DISPLAY1"), label_for(r"\\.\DISPLAY2"));
    }
}
