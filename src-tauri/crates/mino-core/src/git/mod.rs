//! Git: what the answers mean, decided in one place.
//!
//! This module mirrors [`crate::search`]. The per-transport code *runs* git -
//! `tokio::process` locally, an exec channel over SSH - and everything it
//! *decides* is here: which argv to use, what an exit code means, and how
//! `--porcelain=v2` output becomes a [`crate::types::GitStatus`]. Local and
//! SSH cannot drift into disagreeing about what git said, because neither of
//! them reads it.
//!
//! Two rules this module exists to hold:
//!
//! - **Argv only.** No caller value is interpolated into a git command line.
//!   See [`command`]: paths travel as argv elements behind a `--` separator
//!   and are guarded by [`guard`] first, and a commit message travels on stdin
//!   rather than in argv at all. Branch names are checked by [`refname`]
//!   against git's own rules, and a stash entry is named by a number that
//!   [`command::selector`] turns into `stash@{N}`, so no caller text reaches
//!   the selector at all.
//! - **Absence is not an error.** A folder that is not inside a repository
//!   answers `Ok(None)`, because most folders are not repositories and the UI
//!   renders that as a quiet state, not a failure.
//!
//! A third rule arrives with phase 4, and it is not about safety: **a call
//! that changes the working tree reports what it did, and the panes re-read
//! from git rather than assuming.** A checkout either switched or it did not,
//! and [`branches::failure`] is where the difference becomes a sentence.
//!
//! A fourth arrives with phase 6, and it is about safety again: **no text from
//! a call that talked to a remote reaches a message, a result or a log without
//! going through [`redact`] first.** Not when it looks suspicious - always. A
//! remote configured as `https://user:token@host/o/r` is an ordinary thing to
//! find in somebody's `.git/config`, and git will print it back in a progress
//! line without being asked. See `plan/decisions.md` D3, which is what makes
//! that the *only* place a credential can appear: this app holds none.
//!
//! Git *missing* is a different thing and is an error - a typed one, with a
//! sentence the reader can act on. The UI asks once, through `repository()`,
//! and every git surface goes quiet for the session when the answer comes back
//! that way. That is the whole probe: a third trait method would be a second
//! thing to keep in agreement with the first.

pub mod blame;
pub mod branch;
pub mod branches;
pub mod command;
pub mod commit;
pub mod conflicts;
pub mod diff;
pub mod guard;
pub mod history;
mod interpret;
pub mod paths;
pub mod porcelain;
pub mod redact;
pub mod refname;
pub mod remote;
pub mod revision;
pub mod stash;

use crate::error::TransportError;

pub use interpret::{ignored_from, repository_root, status_from};

/// What one git call produced. Both transports fill this in; only
/// [`interpret`] reads it.
#[derive(Debug, Clone)]
pub struct GitOutput {
    /// `None` when the process was killed by a signal rather than exiting.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl GitOutput {
    pub fn succeeded(&self) -> bool {
        self.code == Some(0)
    }

    /// The stderr line worth showing, or a fallback. Git is terse on failure
    /// and an empty notice helps nobody.
    pub(crate) fn message(&self, what: &str) -> String {
        let text = self.stderr.trim();
        if text.is_empty() {
            format!("git {what} failed")
        } else {
            text.to_string()
        }
    }
}

/// Absolute path to `git`, or `None` when it is not on PATH. The counterpart
/// of [`crate::shell::find_nu`].
pub fn find_git() -> Option<String> {
    which::which(command::GIT_PROGRAM)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// The one sentence a machine without git gets, wherever it is discovered.
pub fn missing() -> TransportError {
    TransportError::shell(
        "git is not installed, or is not on PATH, so this folder cannot be read as a \
         repository. Install git and reopen the folder.",
    )
}

/// What a failed call should say. Git's own words when it had any, and a
/// sentence naming the operation when it did not.
pub fn message_or(output: &GitOutput, what: &str) -> String {
    output.message(what)
}

/// Raised by `status()` when the connected folder is not inside a repository.
/// Callers are expected to ask `repository()` first and never get here; this
/// is what makes the mistake say so rather than inventing an empty status.
pub fn not_a_repository() -> TransportError {
    TransportError::invalid("the connected folder is not inside a git repository")
}
