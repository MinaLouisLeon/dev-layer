//! Running one program on the remote host, and turning an argv array into the
//! command line `exec` insists on.
//!
//! Extracted from `git_run.rs` when `gh` arrived. The awkward half of the
//! shell-out decision is here rather than in two places: `exec` hands a
//! **string** to the remote login shell, so the argv array has to become a
//! command line somewhere, and somewhere should be once.
//!
//! Three rules, and they are the ones both callers depend on:
//!
//! - **Every argument is quoted**, not merely joined. Paths, branch names and
//!   pull request titles are all caller values.
//! - Those values have already been through their own guard - the path guard,
//!   the refname guard, the create validation - before they arrive, so quoting
//!   is the second line of defence and not the first.
//! - [`super::command::quote`] **refuses** a value containing a single quote
//!   rather than escaping it. That is why the two largest free-text values on
//!   this interface, a commit message and a pull request body, travel on
//!   **stdin** and never through here at all.
//!
//! Building that line is [`line`]; this file is about running it.

use russh::client::Handle;
use russh::ChannelMsg;

use crate::error::{Result, TransportError};
use crate::git::GitOutput;

use super::handler::ClientHandler;

mod line;

pub(in crate::ssh) use line::command_line;

/// Runs `program` with `argv` in `cwd`, optionally sending `input` before EOF.
///
/// `label` names the program in a timeout, so a stalled `gh` and a stalled
/// `git` do not produce the same sentence.
pub async fn run(
    handle: &Handle<ClientHandler>,
    program: &str,
    cwd: &str,
    argv: &[String],
    input: Option<&str>,
    timeout_ms: u64,
    label: &str,
) -> Result<GitOutput> {
    with_env(handle, program, cwd, argv, input, timeout_ms, label, &[]).await
}

/// The same, with `env` set for the remote command.
///
/// A POSIX shell takes `NAME=value cmd …` as "run `cmd` with `NAME` set", which
/// is how [`crate::git::command::NO_PROMPT`] reaches a remote git. Every pair
/// is fixed program text from this crate, so there is no caller value among
/// them - and both halves are quoted anyway, because a value that reaches a
/// command line is quoted here whatever it is.
#[allow(clippy::too_many_arguments)]
pub async fn with_env(
    handle: &Handle<ClientHandler>,
    program: &str,
    cwd: &str,
    argv: &[String],
    input: Option<&str>,
    timeout_ms: u64,
    label: &str,
    env: &[(&str, &str)],
) -> Result<GitOutput> {
    let line = command_line(program, cwd, argv, env)?;
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        exchange(handle, line, input.map(str::to_string)),
    )
    .await
    .map_err(|_| TransportError::Timeout {
        operation: label.to_string(),
        ms: timeout_ms,
    })?
}

async fn exchange(
    handle: &Handle<ClientHandler>,
    line: String,
    input: Option<String>,
) -> Result<GitOutput> {
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| TransportError::protocol(format!("could not open a session: {e}")))?;
    channel
        .exec(true, line.as_bytes())
        .await
        .map_err(|e| TransportError::shell(format!("could not start the remote command: {e}")))?;

    if let Some(text) = input {
        channel
            .data(text.as_bytes())
            .await
            .map_err(|e| TransportError::shell(format!("could not send the input: {e}")))?;
    }
    // Always, input or not. Without EOF a `git commit --file -` or a
    // `gh pr create --body-file -` waits on stdin forever and the call times
    // out instead of doing what it was asked.
    channel
        .eof()
        .await
        .map_err(|e| TransportError::shell(format!("could not close the input: {e}")))?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut code: Option<i32> = None;

    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
            // Extended data type 1 is stderr; anything else is not ours.
            ChannelMsg::ExtendedData { ref data, ext: 1 } => stderr.extend_from_slice(data),
            ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status as i32),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = channel.close().await;

    Ok(GitOutput {
        // A channel that closed without an exit status did not succeed, and
        // saying "unknown" is more honest than assuming zero.
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}
