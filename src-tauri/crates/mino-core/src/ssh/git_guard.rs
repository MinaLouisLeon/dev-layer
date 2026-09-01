//! Making caller paths safe, and reporting a mutating call that failed.
//!
//! The mirror of `local/git_guard.rs`, split from `git.rs` for the same
//! reason: that file stays a list of trait methods, and no method can forget
//! the guard because there is one place to reach it through.

use russh::client::Handle;

use crate::error::{Result, TransportError};
use crate::git::{self, guard::guard_paths, paths::PathStyle};

use super::git_run::run_with_input;
use super::handler::ClientHandler;

/// Runs a mutating call and turns a non-zero exit into git's own sentence.
pub(super) async fn expect_success(
    handle: &Handle<ClientHandler>,
    root: &str,
    argv: Vec<String>,
    what: &str,
) -> Result<()> {
    let output = run_with_input(handle, root, &argv, None).await?;
    if output.succeeded() {
        return Ok(());
    }
    Err(TransportError::shell(git::message_or(&output, what)))
}

/// Every path a caller named, made safe to hand to git.
pub(super) fn guard_many(root: &str, paths: &[String]) -> Result<Vec<String>> {
    guard_paths(root, paths, PathStyle::posix())
}

/// A path a read may or may not name. `None` means the whole tree and is not
/// something to guard; anything else goes through the same guard the mutating
/// calls use.
pub(super) fn guard_optional(root: &str, path: Option<&str>) -> Result<Option<String>> {
    match path {
        Some(path) => Ok(Some(
            guard_many(root, std::slice::from_ref(&path.to_string()))?.remove(0),
        )),
        None => Ok(None),
    }
}
