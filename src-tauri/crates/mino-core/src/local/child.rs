//! Running one child process on this machine, and collecting what it said.
//!
//! Extracted from `git_run.rs` when `gh` arrived and needed the same four
//! things git already needed: an argv array rather than a shell line, an
//! optional value on stdin, a wall-clock ceiling, and `kill_on_drop`. Two
//! copies of that would have been two places for the stdin close or the kill
//! to be forgotten - and each of those has a failure mode that looks like a
//! hang rather than an error.
//!
//! The rules it holds are the ones both callers depend on:
//!
//! - **Never a shell line.** The arguments reach the process as separate
//!   arguments, so a value in one of them is data and can never become syntax.
//! - **The working directory is set, not written.** `current_dir`, not a `cd`
//!   in a command.
//! - **stdin is closed after the write.** Without the close, `git commit
//!   --file -` and `gh pr create --body-file -` both wait on stdin forever and
//!   the call times out instead of doing what it was asked.
//! - **`kill_on_drop`.** A dropped future must not leave a child running
//!   against the index, or holding a network connection open.

use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{Result, TransportError};
use crate::git::GitOutput;
use crate::local::no_window::hide_console;

/// Runs `program` with `argv` in `cwd`, optionally writing `input` to its
/// standard input, and collects its exit code and both streams.
///
/// `label` names the program in the error sentences - "git", "gh" - so a
/// failure to start or a timeout says which one it was talking about.
pub async fn output(
    program: &str,
    cwd: &str,
    argv: &[String],
    input: Option<&str>,
    timeout_ms: u64,
    label: &str,
) -> Result<GitOutput> {
    with_env(program, cwd, argv, input, timeout_ms, label, &[]).await
}

/// The same, with `env` added to the child's environment.
///
/// Every pair is fixed program text from this crate - there is no caller value
/// among them - so nothing here needs the treatment an argument gets.
pub async fn with_env(
    program: &str,
    cwd: &str,
    argv: &[String],
    input: Option<&str>,
    timeout_ms: u64,
    label: &str,
    env: &[(&str, &str)],
) -> Result<GitOutput> {
    let mut command = Command::new(program);
    // Before anything else, so no path through this function can spawn a
    // console window over the workbench - see `no_window`.
    hide_console(&mut command);
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command
        .args(argv.iter().map(String::as_str))
        .current_dir(cwd)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| TransportError::shell(format!("could not start {label}: {e}")))?;

    if let Some(text) = input {
        // Taken and dropped, so the pipe closes. Without the close the child
        // waits on stdin forever and the call times out instead of finishing.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::shell(format!("{label} did not accept its input")))?;
        stdin.write_all(text.as_bytes()).await.map_err(|e| {
            TransportError::shell(format!("could not write the {label} input: {e}"))
        })?;
        stdin.shutdown().await.map_err(|e| {
            TransportError::shell(format!("could not close the {label} input: {e}"))
        })?;
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| TransportError::Timeout {
        operation: format!("{label} {}", argv.first().map_or("call", String::as_str)),
        ms: timeout_ms,
    })?
    .map_err(|e| TransportError::shell(e.to_string()))?;

    Ok(GitOutput {
        code: output.status.code(),
        // Lossy on purpose. Git's `-z` keeps paths unquoted, and a path that
        // is not valid UTF-8 should cost one mangled row rather than the whole
        // status.
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
