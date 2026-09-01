//! Running `gh` on this machine.
//!
//! The same shape `git_run.rs` has, sharing the same spawner in
//! [`super::child`]: an argv array, an optional value on stdin, a wall-clock
//! ceiling and `kill_on_drop`. The differences are the binary, a longer
//! timeout because these calls go over the network, and one thing neither git
//! nor `nu` needs - the environment.

use crate::error::Result;
use crate::git::GitOutput;
use crate::github::{self, DEFAULT_TIMEOUT_MS};

use super::child;

/// Runs one `gh` call in `cwd` and collects its output.
///
/// `cwd` is the session root, which is what lets `gh` work out which
/// repository is being asked about - the same arrangement every git call here
/// uses, and the reason no repository name is ever passed as an argument.
///
/// A machine without `gh` is not reached through here: the probe answers
/// [`crate::types::GitHubAvailability::Absent`] before any query is planned,
/// so this returning "could not start gh" would mean a caller skipped the
/// probe. It is still a typed error rather than a panic.
pub async fn run(cwd: &str, argv: &[String], input: Option<&str>) -> Result<GitOutput> {
    let program = github::find_gh().ok_or_else(|| {
        crate::TransportError::shell(
            "the GitHub CLI (gh) is not installed, or is not on PATH. Install it from \
             cli.github.com.",
        )
    })?;
    child::output(&program, cwd, argv, input, DEFAULT_TIMEOUT_MS, "gh").await
}
