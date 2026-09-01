//! Asking the remote host what shells it has.
//!
//! The local transport answers this by looking at `PATH` itself. Over SSH the
//! only way to know is to ask the host, so this runs one small command whose
//! text is fixed - no caller value goes into it - and parses two lines out.
//!
//! A host without `nu` is not an error: the terminal falls back to the login
//! shell and the UI says so, exactly as it does locally.

use russh::client::Handle;
use russh::ChannelMsg;

use crate::error::{Result, TransportError};
use crate::types::ShellProbe;

use super::handler::ClientHandler;

/// Fixed program text. `command -v` is POSIX; the `|| true` keeps a missing
/// `nu` from turning into a non-zero exit and an error.
const PROBE: &str = "command -v nu || true; printf '\\n---\\n'; printf '%s' \"${SHELL:-/bin/sh}\"";
const FALLBACK_SHELL: &str = "/bin/sh";

pub async fn probe(handle: &Handle<ClientHandler>) -> Result<ShellProbe> {
    let output = run(handle, PROBE).await?;
    let (nu_part, shell_part) = output.split_once("---").unwrap_or((&output, ""));

    let nu_path = nu_part
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string);

    let fallback_program = match shell_part.trim() {
        "" => FALLBACK_SHELL.to_string(),
        found => found.to_string(),
    };
    let fallback_label = fallback_program
        .rsplit('/')
        .next()
        .unwrap_or(&fallback_program)
        .to_string();

    Ok(ShellProbe {
        nu_available: nu_path.is_some(),
        nu_path,
        fallback_program,
        fallback_label,
    })
}

/// Runs one fixed command and collects stdout. Used only by this module.
async fn run(handle: &Handle<ClientHandler>, command: &str) -> Result<String> {
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| TransportError::protocol(format!("could not open a session: {e}")))?;
    channel
        .exec(true, command.as_bytes())
        .await
        .map_err(|e| TransportError::shell(format!("could not probe the remote shell: {e}")))?;

    let mut stdout = Vec::new();
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = channel.close().await;
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}
