//! Spawning a console program from a windowed app without flashing a console.
//!
//! `git`, `gh` and `nu` are console-subsystem programs. The desktop app is a
//! windowed one, so it owns no console for them to inherit, and Windows
//! answers that by allocating a fresh console window per child - a black
//! rectangle that appears, sits in front of the workbench and disappears when
//! the call returns.
//!
//! On its own that is ugly. What made it a bug worth a module is the loop it
//! closes: each console window takes the foreground and hands it back on
//! exit, the workbench hears a `focus` event, `useGitStatus` treats a focus
//! event as the moment to re-read the working tree - correctly, it is - and
//! those reads spawn the next two windows. Opening a repository started it and
//! nothing stopped it, because every flash paid for the flash after it.
//!
//! `CREATE_NO_WINDOW` is the whole fix. The child still gets its pipes, its
//! exit code and both streams; it simply gets no console to draw. Nothing is
//! suppressed that anyone could have read - the windows closed far too fast to
//! be read, and the output was already being captured rather than displayed.
//!
//! This is **not** applied to the PTY. That child is the terminal pane, it is
//! spawned through ConPTY, and its console is the thing the user asked for.

/// Applies the platform's "no console window" flag to `command`.
///
/// A no-op off Windows, where a console child inherits the parent's terminal
/// or none at all and no window is ever created.
pub fn hide_console(command: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(windows)]
    {
        // 0x0800_0000. Named rather than imported: pulling `windows-sys` in
        // for one constant would put a platform dependency in the crate that
        // must build for three transports.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
