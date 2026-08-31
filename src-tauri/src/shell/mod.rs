//! Reversible mutations of the Windows shell.
//!
//! Deliberately conservative: dev-layer auto-hides the taskbar, it does not
//! replace `explorer.exe`. Nothing here touches the Winlogon `Shell` registry
//! key — that is how a machine ends up booting to a blank desktop.

use crate::config::Config;
use crate::error::Result;
use crate::platform::sys;

/// `ABS_AUTOHIDE | ABS_ALWAYSONTOP` — hidden, but still reachable by hovering
/// the screen edge, so the user is never stranded without a taskbar.
const AUTOHIDE_ON_TOP: u32 = 0x1 | 0x2;

pub fn apply(config: &Config) -> Result<()> {
    if !config.shell.hide_taskbar {
        return Ok(());
    }

    let previous = sys::taskbar_state();
    sys::set_taskbar_state(AUTOHIDE_ON_TOP)?;
    tracing::info!(previous, "taskbar set to auto-hide");

    // Registered immediately after the change, so a panic one line later still
    // gives the taskbar back.
    crate::safety::register("restore taskbar", move || {
        if let Err(e) = sys::set_taskbar_state(previous) {
            tracing::error!(error = %e, "could not restore taskbar state");
        }
    });

    Ok(())
}
