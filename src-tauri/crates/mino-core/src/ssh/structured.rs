//! The non-interactive Nushell channel, over SSH.
//!
//! This is the one place in the SSH transport where a remote command line is
//! built, so it is where the injection rule has to hold. It holds like this:
//!
//! - the command string is assembled only from fixed program text - the nu
//!   binary name, its flags, and the pipeline constant from `pipelines.rs`;
//! - caller values never appear in it. They are serialised to JSON and written
//!   to the channel's *stdin*, where a preamble turns them into environment
//!   variables with `from json | load-env`.
//!
//! So `$env.MINO_PATH` means the same thing here as it does locally, while a
//! path containing a quote, a semicolon or a backtick is inert data that the
//! remote shell never parses.

use std::collections::BTreeMap;

use russh::client::Handle;
use russh::ChannelMsg;

use crate::error::{Result, TransportError};
use crate::types::{StructuredOutput, StructuredRequest};

use super::command::{command_line, validate};
use super::handler::ClientHandler;

pub const DEFAULT_TIMEOUT_MS: u64 = 15_000;
pub const MAX_TIMEOUT_MS: u64 = 60_000;
pub const PARAM_PREFIX: &str = "MINO_";

pub async fn run(
    handle: &Handle<ClientHandler>,
    nu: &str,
    request: &StructuredRequest,
) -> Result<StructuredOutput> {
    validate(request, nu)?;
    let timeout_ms = request
        .timeout_ms
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .min(MAX_TIMEOUT_MS);

    let payload = serde_json::to_string(&bound(&request.params))
        .map_err(|e| TransportError::protocol(format!("could not encode parameters: {e}")))?;

    let command = command_line(nu, &request.pipeline, request.cwd.as_deref())?;

    let future = exchange(handle, command, payload);
    tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future)
        .await
        .map_err(|_| TransportError::Timeout {
            operation: "run_structured".to_string(),
            ms: timeout_ms,
        })?
}

async fn exchange(
    handle: &Handle<ClientHandler>,
    command: String,
    payload: String,
) -> Result<StructuredOutput> {
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| TransportError::protocol(format!("could not open a session: {e}")))?;

    channel
        .exec(true, command.as_bytes())
        .await
        .map_err(|e| TransportError::shell(format!("could not start nu: {e}")))?;

    channel
        .data(payload.as_bytes())
        .await
        .map_err(|e| TransportError::shell(format!("could not send parameters: {e}")))?;
    // Without EOF the remote `$in` never completes and nu waits forever.
    channel
        .eof()
        .await
        .map_err(|e| TransportError::shell(format!("could not close the input: {e}")))?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut status: Option<u32> = None;

    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
            // Extended data type 1 is stderr; anything else is not ours.
            ChannelMsg::ExtendedData { ref data, ext: 1 } => stderr.extend_from_slice(data),
            ChannelMsg::ExitStatus { exit_status } => status = Some(exit_status),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }

    let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
    if status.is_some_and(|code| code != 0) {
        return Err(TransportError::shell(if stderr.is_empty() {
            "the remote nushell command failed".to_string()
        } else {
            stderr
        }));
    }

    let text = String::from_utf8_lossy(&stdout);
    let value = serde_json::from_str(text.trim()).map_err(|e| {
        TransportError::protocol(format!(
            "remote nushell returned output that is not json: {e}"
        ))
    })?;
    Ok(StructuredOutput { value, stderr })
}

fn bound(params: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    params
        .iter()
        .map(|(key, value)| (format!("{PARAM_PREFIX}{key}"), value.clone()))
        .collect()
}
