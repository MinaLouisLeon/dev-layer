//! Win32 implementation of the platform surface.

mod apps;
mod display;
mod shell;

pub use apps::{extract_icon_png, launch, resolve_shortcut, ComGuard, Shortcut};
pub use display::{enumerate_monitors, spawn_display_notifier, DisplayNotifier};
pub use shell::{set_taskbar_state, taskbar_state};
