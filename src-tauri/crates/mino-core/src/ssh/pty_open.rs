//! Opening a remote PTY.
//!
//! Split from `transport_impl` because choosing the program, resolving the
//! working directory and building the launch line is a sequence worth reading
//! on its own.

use crate::error::Result;
use crate::types::{PtySession, PtySessionId, PtySpawnSpec, PtyStream, ShellKind};

use super::pty::PtyRegistry;
use super::{command, fs, Connected};

pub(super) async fn open(
    ptys: &PtyRegistry,
    connected: &Connected,
    spec: PtySpawnSpec,
) -> Result<PtyStream> {
    let cwd = match spec.cwd.as_deref() {
        Some(requested) => fs::resolve(&connected.sftp, &connected.root, requested).await?,
        None => connected.root.root().to_string(),
    };

    let probe = &connected.shell;
    let program = match &probe.nu_path {
        Some(nu) => nu.clone(),
        None => probe.fallback_program.clone(),
    };
    let id = PtySessionId::new();
    // The shell is started inside the session root; `command` is built
    // from quoted paths only, never from free text.
    let launch = command::command_line_shell(&program, &cwd)?;

    let events = ptys
        .open(&connected.handle, &id, spec.size, Some(&launch))
        .await?;

    Ok(PtyStream {
        session: PtySession {
            id,
            program,
            shell: if probe.nu_available {
                ShellKind::Nu
            } else {
                ShellKind::Fallback
            },
            cwd,
            size: spec.size.sanitised(),
            fell_back: !probe.nu_available,
        },
        events,
    })
}
