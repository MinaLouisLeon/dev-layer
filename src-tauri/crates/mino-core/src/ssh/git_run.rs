//! Running `git` on the remote host.
//!
//! The awkward half of the shell-out decision, and the reason it is still the
//! right one: the remote host has its own git, so a repository over SSH works
//! with no extra machinery - but `exec` hands a *string* to the remote login
//! shell, so the argv array has to become a command line somewhere.
//!
//! It becomes one in [`super::exec`], which `github_run.rs` shares, under
//! three rules:
//!
//! - **Every argument is quoted**, not merely joined. Phase 2 passes paths, and
//!   a path is a caller value.
//! - Paths have already been through [`crate::git::guard`] against the session
//!   root before they arrive, so quoting is the second line of defence and not
//!   the first.
//! - [`super::command::quote`] *refuses* a value containing a single quote
//!   rather than escaping it, so a remote file whose name contains one is a
//!   typed error rather than a mangled command. That is why the commit
//!   message - the caller value most likely to contain an apostrophe - travels
//!   on **stdin** and never through here at all.

use russh::client::Handle;

use crate::error::Result;
use crate::git::command::{GIT_PROGRAM, NO_PROMPT, REMOTE_TIMEOUT_MS};
use crate::git::GitOutput;

use super::exec;
use super::handler::ClientHandler;

/// Ceiling for one remote git call. Longer than the local one: the same status
/// is running over a network round trip.
const TIMEOUT_MS: u64 = 20_000;

pub async fn run(handle: &Handle<ClientHandler>, cwd: &str, argv: &[&str]) -> Result<GitOutput> {
    let owned: Vec<String> = argv.iter().map(|arg| (*arg).to_string()).collect();
    run_with_input(handle, cwd, &owned, None).await
}

/// The same, with `input` sent on the channel's stdin before EOF.
///
/// This is the whole reason a commit message is not an argument. The command
/// line is parsed by the remote login shell, and `quote` refuses a value
/// containing a single quote - which would refuse every commit message with an
/// apostrophe in it. On stdin the message is bytes, and nothing parses it.
pub async fn run_with_input(
    handle: &Handle<ClientHandler>,
    cwd: &str,
    argv: &[String],
    input: Option<&str>,
) -> Result<GitOutput> {
    exec::run(handle, GIT_PROGRAM, cwd, argv, input, TIMEOUT_MS, "git").await
}

/// The runner for the three calls that leave the *remote* machine.
///
/// The credential that answers for these belongs to the remote host - its
/// helper, its agent, its keychain - and nothing about this machine's git is
/// involved. `NO_PROMPT` matters more here than it does locally: a prompt on
/// an exec channel has nowhere at all to go, and without it a remote push
/// against an unconfigured account would hold the channel open until the
/// timeout.
pub async fn run_remote(
    handle: &Handle<ClientHandler>,
    cwd: &str,
    argv: &[String],
) -> Result<GitOutput> {
    exec::with_env(
        handle,
        GIT_PROGRAM,
        cwd,
        argv,
        None,
        REMOTE_TIMEOUT_MS,
        "git",
        NO_PROMPT,
    )
    .await
}

#[cfg(test)]
mod tests;
