//! Argv for the three calls that leave the machine, and the environment they
//! run in.
//!
//! ## The environment is the interesting part
//!
//! Everything else in this crate is about keeping caller values out of a
//! command line. These three calls have that problem too - a remote name is a
//! caller value - but they have a second one nothing before them had: **git
//! may want to ask a question**, and there is nobody at the other end of a
//! child process to answer it.
//!
//! D3 settled how this app authenticates: it does not. Git uses its own
//! credential helper, the SSH agent or the OS keychain, and this process never
//! sees a secret. The failure mode that comes with that choice is a prompt
//! with nowhere to go, which is a hang - so every call here runs with
//! [`NO_PROMPT`] and under a timeout. A missing credential becomes a typed
//! error naming what to configure, in a second or two, rather than a pane that
//! never finishes.
//!
//! A **graphical** helper - Git Credential Manager, an OS keychain dialog - is
//! deliberately still allowed to appear. That is what delegation *is*; the
//! thing being prevented is the invisible prompt on a terminal that is not
//! there.

use super::{GLOBAL, PATH_SEPARATOR};
use crate::types::{PullRequest, PushRequest};

fn owned<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    args.iter().map(|arg| arg.as_ref().to_string()).collect()
}

/// The environment every remote call runs with.
///
/// `GIT_TERMINAL_PROMPT=0` makes git fail rather than ask when it needs a
/// username or a password and has no other way to get one. Without it, a
/// repository whose helper is not configured hangs until the timeout - and the
/// reader is left looking at a spinner instead of a sentence telling them to
/// run `git credential-manager configure` or add a key to their agent.
pub const NO_PROMPT: &[(&str, &str)] = &[("GIT_TERMINAL_PROMPT", "0")];

/// Wall-clock ceiling for a call that talks to a network.
///
/// Far longer than [`super::DEFAULT_TIMEOUT_MS`], because this one is fetching
/// objects over somebody's connection and a large first fetch is genuinely
/// slow. Still finite, because the whole point of `NO_PROMPT` is that nothing
/// here is ever allowed to wait forever.
pub const REMOTE_TIMEOUT_MS: u64 = 120_000;

/// `git remote --verbose`.
///
/// Takes no caller value at all. Its *output* carries URLs, which is why
/// [`crate::git::remote::parse`] puts every one of them through
/// [`crate::git::redact`] before it becomes a [`crate::types::GitRemote`].
pub fn remotes_argv() -> Vec<String> {
    let mut argv = owned(GLOBAL);
    argv.extend(owned(&["remote", "--verbose"]));
    argv
}

/// `git fetch [<remote>] --prune`.
///
/// `--prune` drops remote-tracking refs for branches that no longer exist on
/// the remote. It changes nothing in the working tree and nothing you could
/// lose - a remote-tracking ref is a cache of somebody else's state - and
/// without it the branch picker slowly fills with branches that were merged
/// and deleted months ago.
///
/// The remote is a caller value and travels as its own argv element. It has
/// been through [`crate::git::remote::name`] first, which refuses anything
/// readable as an option.
pub fn fetch_argv(remote: Option<&str>) -> Vec<String> {
    let mut argv = owned(&["fetch", "--prune", "--quiet"]);
    if let Some(remote) = remote {
        argv.push(remote.to_string());
    }
    argv
}

/// `git pull [--rebase] [<remote>]`.
///
/// No `--ff-only`. A pull that cannot fast-forward is a real situation with a
/// real answer - merge, or rebase - and refusing it outright would send the
/// reader to a terminal to do the thing the button is for. What the plan
/// forbids is *guessing*, and nothing here guesses: the outcome comes back as
/// [`crate::types::GitPullOutcome`], including `Conflicted`, so the caller is
/// told which of the four happened rather than being left to infer it.
///
/// The dirty-tree check happens before this argv is ever built - see
/// [`crate::git::remote::dirty`].
pub fn pull_argv(request: &PullRequest) -> Vec<String> {
    let mut argv = owned(&["pull"]);
    if request.rebase {
        argv.push("--rebase".to_string());
    }
    if let Some(remote) = &request.remote {
        argv.push(remote.clone());
    }
    argv
}

/// `git push [--force-with-lease] [--set-upstream] <remote> <branch>`.
///
/// **`--force-with-lease`, never `--force`.** The difference is the whole
/// safety of the operation: `--force` overwrites the remote branch whatever is
/// on it, including a colleague's commit pushed thirty seconds ago;
/// `--force-with-lease` refuses unless the remote is where this repository
/// last saw it. A force push from a workbench should not be able to destroy
/// work the pusher has never seen.
///
/// Both values are caller-influenced and both are checked first - the remote
/// by [`crate::git::remote::name`] and the branch by
/// [`crate::git::refname::precheck`] - then placed after `--` so neither can
/// be read as a flag.
pub fn push_argv(request: &PushRequest, remote: &str, branch: &str) -> Vec<String> {
    let mut argv = owned(&["push", "--porcelain"]);
    if request.force {
        argv.push("--force-with-lease".to_string());
    }
    if request.set_upstream {
        argv.push("--set-upstream".to_string());
    }
    argv.push(PATH_SEPARATOR.to_string());
    argv.push(remote.to_string());
    argv.push(branch.to_string());
    argv
}

#[cfg(test)]
mod tests;
