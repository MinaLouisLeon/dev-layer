//! Making caller paths safe, and reporting a mutating call that failed.
//!
//! Split from `git.rs` so that file stays a list of trait methods. Both
//! helpers exist so that no method can forget them - `discard` in particular
//! must never be reachable with an unguarded path.

use crate::error::{Result, TransportError};
use crate::git::{self, guard::guard_paths, paths::PathStyle};

use super::git_run::run_with_input;
use super::LocalTransport;

impl LocalTransport {
    /// The session root, and the caller's paths made safe to hand to git.
    ///
    /// One place, so no mutating method can forget it - `discard` in
    /// particular must never be reachable with an unguarded path.
    pub(super) fn guarded(&self, paths: &[String]) -> Result<(String, Vec<String>)> {
        let root = self.guard()?.root_display();
        let resolved: Vec<String> = paths.iter().map(|path| resolve(path)).collect();
        let guarded = guard_paths(&root, &resolved, PathStyle::local())?;
        Ok((root, guarded))
    }

    /// The same for the reads, where a path is optional: `None` means the
    /// whole tree and is not something to guard.
    pub(super) fn guarded_one(&self, path: Option<&str>) -> Result<(String, Option<String>)> {
        let root = self.guard()?.root_display();
        let guarded = match path {
            Some(path) => Some(guard_paths(&root, &[resolve(path)], PathStyle::local())?.remove(0)),
            None => None,
        };
        Ok((root, guarded))
    }
}

/// Puts a caller's path into the same spelling the guard compares against.
///
/// `connect` canonicalises the session root, so the guard's containment test
/// is against the canonical form. A caller can legitimately hold *another*
/// spelling of the same file, and refusing those would be refusing a path the
/// session plainly owns:
///
/// - a Windows 8.3 short name - `C:\Users\RUNNER~1\…` for `runneradmin`, which
///   is what `%TEMP%` expands to on a GitHub Actions runner;
/// - a symlinked temporary directory, which is how `/tmp` and `/var/folders`
///   work on macOS;
/// - a `.` segment, or a different case on a case-insensitive filesystem.
///
/// A path that does **not** resolve is handed on untouched, which is the case
/// that matters most: staging a *deleted* file is half of what the source
/// control panel is for, and `canonicalize` has nothing to say about a path
/// that is no longer there. [`guard_paths`] then rules on the string, refusing
/// `..` outright.
///
/// This is local-only on purpose. Over SSH the path names a file on the remote
/// host, where this process's filesystem cannot answer for it at all - those
/// paths arrive from SFTP `realpath` already resolved.
///
/// Nothing here decides what is *allowed*. Containment is still checked
/// afterwards, against the canonical root, so a resolved path outside the
/// session is refused exactly as an unresolved one would be.
fn resolve(path: &str) -> String {
    match std::fs::canonicalize(path) {
        // Windows canonicalisation returns the `\\?\` extended-length form,
        // which the root was already stripped of.
        Ok(found) => super::roots::display_path(&found),
        Err(_) => path.to_string(),
    }
}

/// Runs a mutating call and turns a non-zero exit into git's own sentence.
pub(super) async fn expect_success(root: &str, argv: Vec<String>, what: &str) -> Result<()> {
    let output = run_with_input(root, &argv, None).await?;
    if output.succeeded() {
        return Ok(());
    }
    // Never reported as a partial success: the batch either ran or it did not,
    // and saying otherwise would leave the UI unable to describe the index.
    Err(TransportError::shell(git::message_or(&output, what)))
}
