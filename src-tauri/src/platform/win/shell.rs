//! Shell mutations: taskbar state, and launching things through the shell.

use windows::core::w;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, APPBARDATA};
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

use crate::error::{Error, Result};

/// Current appbar state bitmask (`ABS_AUTOHIDE` | `ABS_ALWAYSONTOP`).
pub fn taskbar_state() -> u32 {
    unsafe {
        let mut data = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            ..Default::default()
        };
        SHAppBarMessage(ABM_GETSTATE, &mut data) as u32
    }
}

/// Applies an appbar state bitmask. Used both to auto-hide the taskbar and to
/// put it back exactly as we found it.
pub fn set_taskbar_state(state: u32) -> Result<()> {
    unsafe {
        let tray = FindWindowW(w!("Shell_TrayWnd"), None)
            .map_err(|e| Error::Platform(format!("taskbar not found: {e}")))?;

        let mut data = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: tray,
            lParam: LPARAM(state as isize),
            ..Default::default()
        };
        SHAppBarMessage(ABM_SETSTATE, &mut data);
    }
    Ok(())
}
