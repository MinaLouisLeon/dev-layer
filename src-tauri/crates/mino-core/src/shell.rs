//! Locating `nu` and choosing the platform fallback shell.
//!
//! Shared by the local transport and (later) by the agent daemon running on
//! the remote host. Missing `nu` is never fatal: the workbench falls back and
//! says so.

use crate::types::ShellProbe;

pub const NU_PROGRAM: &str = "nu";

/// Absolute path to `nu`, or `None` when it is not on PATH.
pub fn find_nu() -> Option<String> {
    which::which(NU_PROGRAM)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Program spawned when `nu` is absent.
///
/// Unix reads `$SHELL` and falls back to `/bin/sh`, which is the one shell
/// guaranteed to exist. Windows prefers PowerShell and falls back to
/// `%COMSPEC%`, then `cmd.exe`.
#[cfg(windows)]
pub fn fallback_program() -> String {
    if which::which("powershell.exe").is_ok() {
        return "powershell.exe".to_string();
    }
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

#[cfg(not(windows))]
pub fn fallback_program() -> String {
    match std::env::var("SHELL") {
        Ok(shell) if !shell.trim().is_empty() => shell,
        _ => "/bin/sh".to_string(),
    }
}

/// Display name for the fallback, used in the terminal notice.
pub fn fallback_label(program: &str) -> String {
    let base = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .trim_end_matches(".exe");
    if base.is_empty() {
        program.to_string()
    } else {
        base.to_string()
    }
}

pub fn probe() -> ShellProbe {
    let nu_path = find_nu();
    let fallback_program = fallback_program();
    ShellProbe {
        nu_available: nu_path.is_some(),
        nu_path,
        fallback_label: fallback_label(&fallback_program),
        fallback_program,
    }
}
