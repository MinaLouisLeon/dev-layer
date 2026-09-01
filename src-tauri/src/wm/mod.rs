//! The window manager: what makes launched apps look like they live inside
//! the HUD.
//!
//! Real windows, owned by their own processes, positioned into the region the
//! HUD reserves. Nothing is reparented — see `docs/architecture.md` §1 for why
//! that approach was rejected.
//!
//! Invariant: every window this module moves has its original rectangle
//! recorded *before* the first move, and [`WindowManager::restore_all`] is
//! registered with `safety`, so a crash cannot leave the desktop rearranged.

pub mod layout;
mod watcher;

use std::collections::{HashMap, HashSet};

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::config::WmConfig;
use crate::geometry::Rect;
use crate::monitors::MonitorInfo;
use crate::platform::sys;

pub use layout::{LayoutKind, LayoutParams};
pub use watcher::{retile, spawn_watcher, WINDOWS_EVENT};

/// A window under management, as the HUD sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedWindow {
    /// The native window handle. Unique while the window lives.
    pub id: i64,
    pub title: String,
    pub process: String,
    /// Which display it is assigned to, by monitor id.
    pub monitor_id: String,
    /// Excluded from tiling, by user choice or per-app rule.
    pub floating: bool,
    pub focused: bool,
    pub minimized: bool,
    pub rect: Rect,
    /// Position in its monitor's layout; `None` when floating or minimized.
    pub slot: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorLayout {
    pub monitor_id: String,
    pub kind: LayoutKind,
    /// The rectangle tiling actually fills, in physical pixels.
    pub region: Rect,
    pub window_count: usize,
}

/// Everything the WM knows. One instance, shared through `AppState`.
pub struct WindowManager {
    config: WmConfig,
    enabled: RwLock<bool>,
    windows: RwLock<Vec<ManagedWindow>>,
    layouts: RwLock<HashMap<String, LayoutKind>>,
    regions: RwLock<HashMap<String, Rect>>,
    floating: RwLock<HashSet<i64>>,
    focused: RwLock<Option<i64>>,
    /// Stable tiling order. New windows are appended rather than promoted, so
    /// opening something does not shuffle the window you are working in.
    order: Mutex<Vec<i64>>,
    /// Geometry each window had before we first touched it.
    original: Mutex<HashMap<i64, Rect>>,
    /// The last rectangle we *asked* for, per window. Windows with minimum
    /// sizes never reach their target; without this the reconcile loop would
    /// re-issue the same move forever.
    applied: Mutex<HashMap<i64, Rect>>,
}

impl WindowManager {
    pub fn new(config: WmConfig) -> Self {
        Self {
            enabled: RwLock::new(config.enabled),
            config,
            windows: RwLock::default(),
            layouts: RwLock::default(),
            regions: RwLock::default(),
            floating: RwLock::default(),
            focused: RwLock::default(),
            order: Mutex::default(),
            original: Mutex::default(),
            applied: Mutex::default(),
        }
    }

    pub fn windows(&self) -> Vec<ManagedWindow> {
        self.windows.read().clone()
    }

    pub fn enabled(&self) -> bool {
        *self.enabled.read()
    }

    pub fn layouts(&self) -> Vec<MonitorLayout> {
        let layouts = self.layouts.read();
        let regions = self.regions.read();
        let windows = self.windows.read();

        regions
            .iter()
            .map(|(monitor_id, region)| MonitorLayout {
                monitor_id: monitor_id.clone(),
                kind: layouts
                    .get(monitor_id)
                    .copied()
                    .unwrap_or(self.config.default_layout),
                region: *region,
                window_count: windows
                    .iter()
                    .filter(|w| &w.monitor_id == monitor_id && !w.floating && !w.minimized)
                    .count(),
            })
            .collect()
    }

    pub fn layout_for(&self, monitor_id: &str) -> LayoutKind {
        self.layouts
            .read()
            .get(monitor_id)
            .copied()
            .unwrap_or(self.config.default_layout)
    }

    pub fn set_layout(&self, monitor_id: &str, kind: LayoutKind) {
        self.layouts.write().insert(monitor_id.to_string(), kind);
        // A new layout means new targets; forget what we last asked for.
        self.applied.lock().clear();
    }

    pub fn cycle_layout(&self, monitor_id: &str) -> LayoutKind {
        let next = self.layout_for(monitor_id).next();
        self.set_layout(monitor_id, next);
        next
    }

    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.write() = enabled;
        // Guard released before restoring: restore_all touches other locks and
        // can take a while with many windows.
        if !enabled {
            self.restore_all();
        }
    }

    /// Returns the new floating state, or `None` if the window is unknown.
    pub fn toggle_float(&self, id: i64) -> Option<bool> {
        self.windows.read().iter().find(|w| w.id == id)?;

        let mut floating = self.floating.write();
        let now_floating = if floating.remove(&id) {
            false
        } else {
            floating.insert(id);
            true
        };
        drop(floating);

        self.applied.lock().remove(&id);
        if !now_floating {
            // Coming back under management: nothing to restore to, it will be
            // tiled on the next reconcile.
        } else if let Some(rect) = self.original.lock().get(&id).copied() {
            // Floating again: put it back where the user had it.
            let _ = sys::set_window_rect(id as isize, rect);
        }
        Some(now_floating)
    }

    /// Moves a window to the front of the tiling order — the main pane in
    /// `MainStack`, the first tile everywhere else.
    pub fn promote(&self, id: i64) {
        let mut order = self.order.lock();
        order.retain(|existing| *existing != id);
        order.insert(0, id);
        drop(order);
        self.applied.lock().clear();
    }

    /// Re-reads every window and re-tiles. The one entry point; the watcher,
    /// the hotkeys and the HUD all funnel here.
    pub fn reconcile(&self, monitors: &[MonitorInfo], hud_reserved: impl Fn(&MonitorInfo) -> Rect) {
        let native = sys::enumerate_windows();
        let floating = self.floating.read().clone();
        let focused = *self.focused.read();

        // Drop windows that no longer exist from every side table.
        let live: HashSet<i64> = native.iter().map(|w| w.hwnd as i64).collect();
        self.order.lock().retain(|id| live.contains(id));
        self.applied.lock().retain(|id, _| live.contains(id));
        self.original.lock().retain(|id, _| live.contains(id));

        let mut order = self.order.lock();
        for window in &native {
            let id = window.hwnd as i64;
            if !order.contains(&id) {
                order.push(id);
            }
        }
        let order_index: HashMap<i64, usize> =
            order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        drop(order);

        let mut managed: Vec<ManagedWindow> = Vec::new();
        for window in native {
            let id = window.hwnd as i64;
            if self.config.is_ignored(&window.process) {
                continue;
            }
            let monitor = assign_monitor(&window.rect, monitors);
            managed.push(ManagedWindow {
                id,
                floating: floating.contains(&id) || self.config.floats_by_default(&window.process),
                focused: focused == Some(id),
                monitor_id: monitor.map(|m| m.id.clone()).unwrap_or_default(),
                title: window.title,
                process: window.process,
                minimized: window.minimized,
                rect: window.rect,
                slot: None,
            });
        }
        managed.sort_by_key(|w| order_index.get(&w.id).copied().unwrap_or(usize::MAX));

        let enabled = *self.enabled.read();
        if enabled {
            self.tile(&mut managed, monitors, &hud_reserved);
        }
        *self.windows.write() = managed;
    }

    fn tile(
        &self,
        managed: &mut [ManagedWindow],
        monitors: &[MonitorInfo],
        hud_reserved: &impl Fn(&MonitorInfo) -> Rect,
    ) {
        let params = LayoutParams {
            gap: self.config.gap,
            main_ratio: self.config.main_ratio,
        };

        for monitor in monitors {
            let region = hud_reserved(monitor);
            self.regions.write().insert(monitor.id.clone(), region);

            let indices: Vec<usize> = managed
                .iter()
                .enumerate()
                .filter(|(_, w)| w.monitor_id == monitor.id && !w.floating && !w.minimized)
                .map(|(i, _)| i)
                .collect();

            let kind = self.layout_for(&monitor.id);
            let rects = layout::arrange(region, indices.len(), kind, &params);

            for (slot, index) in indices.into_iter().enumerate() {
                let Some(target) = rects.get(slot).copied() else {
                    continue;
                };
                managed[index].slot = Some(slot);
                self.place(managed[index].id, target);
            }
        }
    }

    /// Applies one window's target, remembering where it was first.
    fn place(&self, id: i64, target: Rect) {
        if self.applied.lock().get(&id) == Some(&target) {
            // Already asked for exactly this. Either it worked, or the window
            // refuses to be this size — re-issuing would spin forever.
            return;
        }

        if let Some(current) = sys::window_info(id as isize) {
            self.original.lock().entry(id).or_insert(current.rect);
            if current.rect == target {
                self.applied.lock().insert(id, target);
                return;
            }
        }

        sys::restore_window(id as isize);
        match sys::set_window_rect(id as isize, target) {
            Ok(()) => {
                self.applied.lock().insert(id, target);
            }
            Err(e) => {
                // Elevated windows cannot be moved by a non-elevated process.
                // Record the target anyway so we stop retrying every event.
                tracing::debug!(id, error = %e, "could not place window");
                self.applied.lock().insert(id, target);
            }
        }
    }

    pub fn set_focused(&self, id: Option<i64>) {
        *self.focused.write() = id;
        let mut windows = self.windows.write();
        for window in windows.iter_mut() {
            window.focused = Some(window.id) == id;
        }
    }

    /// Puts every window we ever moved back where we found it. Registered with
    /// `safety`, so this also runs on panic and on Ctrl-C.
    pub fn restore_all(&self) {
        let original = std::mem::take(&mut *self.original.lock());
        if original.is_empty() {
            return;
        }
        tracing::info!(count = original.len(), "restoring window geometry");

        for (id, rect) in original {
            if let Err(e) = sys::set_window_rect(id as isize, rect) {
                tracing::debug!(id, error = %e, "could not restore window");
            }
        }
        self.applied.lock().clear();
    }
}

/// A window belongs to the monitor its centre sits on; windows straddling an
/// edge go to whichever side holds the middle, which is what users expect.
fn assign_monitor<'a>(rect: &Rect, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo> {
    let centre_x = rect.x + rect.width / 2;
    let centre_y = rect.y + rect.height / 2;

    monitors
        .iter()
        .find(|m| {
            centre_x >= m.bounds.x
                && centre_x < m.bounds.right()
                && centre_y >= m.bounds.y
                && centre_y < m.bounds.bottom()
        })
        // Fully offscreen (a window restored from a monitor that is gone):
        // adopt it onto the primary rather than losing it.
        .or_else(|| monitors.iter().find(|m| m.is_primary))
        .or_else(|| monitors.first())
}

/// The tileable area of a monitor: its bounds minus the HUD chrome, converted
/// from the HUD's logical pixels to physical ones.
pub fn tiling_region(monitor: &MonitorInfo, reserved: crate::geometry::Insets) -> Rect {
    monitor
        .bounds
        .inset(reserved.to_physical(monitor.scale_factor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Insets;

    fn monitor(id: &str, x: i32, primary: bool, scale: f64) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            name: id.into(),
            is_primary: primary,
            index: 0,
            bounds: Rect {
                x,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x,
                y: 0,
                width: 1920,
                height: 1040,
            },
            scale_factor: scale,
        }
    }

    #[test]
    fn windows_belong_to_the_monitor_holding_their_centre() {
        let monitors = [
            monitor("left", 0, true, 1.0),
            monitor("right", 1920, false, 1.0),
        ];

        // Straddling the seam, but mostly on the right.
        let straddling = Rect {
            x: 1800,
            y: 100,
            width: 400,
            height: 400,
        };
        assert_eq!(assign_monitor(&straddling, &monitors).unwrap().id, "right");

        let clearly_left = Rect {
            x: 10,
            y: 10,
            width: 800,
            height: 600,
        };
        assert_eq!(assign_monitor(&clearly_left, &monitors).unwrap().id, "left");
    }

    #[test]
    fn offscreen_windows_fall_back_to_the_primary_monitor() {
        let monitors = [
            monitor("left", 0, true, 1.0),
            monitor("right", 1920, false, 1.0),
        ];
        let ghost = Rect {
            x: -9000,
            y: -9000,
            width: 400,
            height: 300,
        };
        assert_eq!(assign_monitor(&ghost, &monitors).unwrap().id, "left");
    }

    #[test]
    fn reserved_insets_scale_to_physical_pixels() {
        let insets = Insets {
            top: 34,
            right: 0,
            bottom: 92,
            left: 280,
        };

        let region = tiling_region(&monitor("a", 0, true, 1.0), insets);
        assert_eq!(
            region,
            Rect {
                x: 280,
                y: 34,
                width: 1640,
                height: 954
            }
        );

        // At 150% the same logical chrome must reserve 1.5x the pixels.
        let scaled = tiling_region(&monitor("b", 0, true, 1.5), insets);
        assert_eq!(scaled.x, 420);
        assert_eq!(scaled.y, 51);
        assert_eq!(scaled.width, 1920 - 420);
    }
}
