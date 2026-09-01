//! Running `gh` on the remote host.
//!
//! The same exec channel `git_run.rs` uses, through the same command line
//! builder in [`super::exec`]. What differs is the binary, the timeout - these
//! calls make a network request *from the remote host*, so they are the
//! slowest thing on this transport - and one thing that cannot be checked the
//! way it is locally.
//!
//! **There is no `which` at a distance.** The local probe asks
//! `github::find_gh()` before spawning anything; here the only way to find out
//! whether `gh` exists is to try to run it. A POSIX shell answers a command it
//! cannot find with exit status 127, and [`NOT_FOUND`] is where that
//! convention is written down - it is what turns "the remote shell could not
//! find gh" into [`crate::types::GitHubAvailability::Absent`] rather than a
//! failure sentence about authentication.

use russh::client::Handle;

use crate::error::Result;
use crate::git::GitOutput;
use crate::github::GH_PROGRAM;

use super::exec;
use super::handler::ClientHandler;

/// Ceiling for one remote `gh` call. Longer than the remote git ceiling,
/// because this is a network request made at the end of a network connection:
/// the SSH round trip and the GitHub round trip are in series.
const TIMEOUT_MS: u64 = 30_000;

/// The exit status a POSIX shell uses for a command it could not find.
///
/// The remote counterpart of `which`. It is a convention rather than a
/// guarantee, which is why it decides between two *quiet* states and never
/// between quiet and broken: the worst a wrong reading here can do is say
/// "gh is not installed" about a host where something else went wrong, and
/// `gh`'s own words are carried alongside either way.
pub const NOT_FOUND: i32 = 127;

pub async fn run(
    handle: &Handle<ClientHandler>,
    cwd: &str,
    argv: &[String],
    input: Option<&str>,
) -> Result<GitOutput> {
    exec::run(handle, GH_PROGRAM, cwd, argv, input, TIMEOUT_MS, "gh").await
}

/// Whether an output is the remote shell saying it has never heard of `gh`.
pub fn is_missing(output: &GitOutput) -> bool {
    output.code == Some(NOT_FOUND)
}
