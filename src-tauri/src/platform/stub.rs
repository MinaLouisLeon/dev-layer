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

/// COM has no meaning off Windows; the guard exists so callers stay uniform.
pub struct ComGuard;

impl ComGuard {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

pub struct Shortcut {
    pub target: std::path::PathBuf,
    pub arguments: String,
    pub working_dir: Option<std::path::PathBuf>,
}

pub fn resolve_shortcut(_lnk: &std::path::Path) -> Result<Shortcut> {
    Err(crate::error::Error::Platform(
        "shortcuts are Windows-only".into(),
    ))
}

pub fn extract_icon_png(_path: &std::path::Path) -> Result<Vec<u8>> {
    Err(crate::error::Error::Platform(
        "icon extraction is Windows-only".into(),
    ))
}

pub fn launch(_path: &std::path::Path, _args: &str, _dir: Option<&std::path::Path>) -> Result<()> {
    Err(crate::error::Error::Platform(
        "launching is Windows-only".into(),
    ))
}

pub fn taskbar_state() -> u32 {
    0
}

pub fn set_taskbar_state(_state: u32) -> Result<()> {
    Ok(())
}
