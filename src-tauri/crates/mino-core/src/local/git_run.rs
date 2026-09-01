//! Running `git` on this machine.
//!
//! The same shape `structured.rs` uses for `nu`: an argv array handed to
//! `tokio::process::Command`, never a shell line, so nothing anywhere in the
//! arguments is parsed as syntax. The arguments come from
//! [`crate::git::command`]; the caller values among them - paths - are guarded
//! by [`crate::git::guard`] before they arrive. The working directory is set
//! with `current_dir` rather than written into a command, and a commit message
//! goes on stdin rather than into the arguments at all.
//!
//! The spawning itself lives in [`super::child`], which `github_run.rs` shares:
//! the argv rule, the stdin close and `kill_on_drop` are the same four things
//! whichever binary is being run, and two copies of them would be two places
//! to forget one.
//!
//! [`run_remote`] is the exception worth knowing about. `fetch`, `pull` and
//! `push` are the only git calls in this app that talk to a network, which
//! makes them the only ones that can be asked for a credential - and under
//! `plan/decisions.md` D3 there is nothing here to answer with.

use crate::error::Result;
use crate::git::command::{DEFAULT_TIMEOUT_MS, NO_PROMPT, REMOTE_TIMEOUT_MS};
use crate::git::{self, GitOutput};

use super::child;

/// Runs one git call in `cwd` and collects its output.
///
/// `kill_on_drop` matters here: a status on a cold, very large repository can
/// outlast the timeout, and a dropped future must not leave git running
/// against the index. See [`super::child::output`].
pub async fn run(cwd: &str, argv: &[&str]) -> Result<GitOutput> {
    let owned: Vec<String> = argv.iter().map(|arg| (*arg).to_string()).collect();
    run_with_input(cwd, &owned, None).await
}

/// The same, with `input` written to git's stdin and the pipe then closed.
///
/// This is how a commit message reaches git. It is not an optimisation: over
/// SSH the argv becomes a command line, and a message containing an apostrophe
/// would be refused by the quoting rule. On stdin it is bytes, and nothing
/// parses it. See `git::command`.
pub async fn run_with_input(cwd: &str, argv: &[String], input: Option<&str>) -> Result<GitOutput> {
    let program = git::find_git().ok_or_else(git::missing)?;
    child::output(&program, cwd, argv, input, DEFAULT_TIMEOUT_MS, "git").await
}

/// The runner for the three calls that leave the machine.
///
/// Two differences from the one above, and both come out of
/// `plan/decisions.md` D3. `NO_PROMPT` makes git fail rather than ask when it
/// wants a credential, because this app has none to give and a prompt on a
/// terminal that is not there is a hang. And the timeout is the network one -
/// far longer, because a first fetch of a large repository is genuinely slow,
/// but still finite, because "never hangs" is the property being bought.
pub async fn run_remote(cwd: &str, argv: &[String]) -> Result<GitOutput> {
    let program = git::find_git().ok_or_else(git::missing)?;
    child::with_env(
        &program,
        cwd,
        argv,
        None,
        REMOTE_TIMEOUT_MS,
        "git",
        NO_PROMPT,
    )
    .await
}
