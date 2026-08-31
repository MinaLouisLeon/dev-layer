//! Non-Windows stand-in: one synthetic 1920×1080 display, no shell mutation.
//! Enough to run `cargo test` and to develop the HUD frontend off-Windows.

use std::sync::mpsc::Sender;

use crate::error::Result;
use crate::geometry::Rect;
use crate::monitors::MonitorInfo;

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    Ok(vec![MonitorInfo {
        id: "stub-display-1".into(),
        name: "Stub Display".into(),
        is_primary: true,
        index: 0,
        bounds: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        work_area: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        },
        scale_factor: 1.0,
    }])
}

pub struct DisplayNotifier;

impl DisplayNotifier {
    pub fn stop(self) {}
}

/// No native display events off-Windows; the watcher's poll covers it.
pub fn spawn_display_notifier(_tx: Sender<()>) -> Result<DisplayNotifier> {
    Ok(DisplayNotifier)
}

pub fn taskbar_state() -> u32 {
    0
}

pub fn set_taskbar_state(_state: u32) -> Result<()> {
    Ok(())
}
