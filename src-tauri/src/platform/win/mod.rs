//! Win32 implementation of the platform surface.

mod apps;
mod display;
mod shell;
mod wm;

pub use apps::{extract_icon_png, launch, resolve_shortcut, ComGuard, Shortcut};
pub use display::{enumerate_monitors, spawn_display_notifier, DisplayNotifier};
pub use shell::{set_taskbar_state, taskbar_state};
pub use wm::{
    close_window, enumerate_windows, focus_window, foreground_window, restore_window,
    set_window_rect, spawn_window_watcher, window_info, window_styles, NativeWindow, WindowEvent,
    WindowWatcher,
};
