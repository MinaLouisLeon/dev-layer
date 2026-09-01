//! What a failed branch call should say.
//!
//! Split from the parser because it is a different job: one reads what git
//! listed, this reads why git refused. Both are shared by every transport, so
//! a checkout that fails over SSH produces the same sentence it would locally.
//!
//! Every branch below names the branch the caller asked for. Git's own
//! wording often does not - `error: pathspec 'feat' did not match any
//! file(s) known to git` is about a branch, and reads as though it is about a
//! file.

use crate::error::TransportError;

use super::super::GitOutput;

/// Turns a failed branch call into the sentence worth showing.
pub fn failure(output: &GitOutput, name: &str, what: &str) -> TransportError {
    let combined = format!("{} {}", output.stdout, output.stderr).to_lowercase();

    if combined.contains("did not match any file(s) known to git")
        || combined.contains("invalid reference")
        || combined.contains("not a valid object name")
        || combined.contains("not found")
    {
        return TransportError::invalid(format!("there is no branch named `{name}`"));
    }
    // The one the phase exists to get right: git switched nothing, and saying
    // so is what stops the UI reporting a checkout that did not happen.
    if combined.contains("would be overwritten") || combined.contains("would be lost") {
        return TransportError::invalid(format!(
            "switching to `{name}` would overwrite changes in the working tree.              Commit or stash them first; nothing has been changed."
        ));
    }
    if combined.contains("already exists") {
        return TransportError::invalid(format!("a branch named `{name}` already exists"));
    }
    if combined.contains("not fully merged") {
        return TransportError::invalid(format!(
            "`{name}` has commits that are not merged anywhere else. Deleting it              would lose them, so it needs the force option."
        ));
    }
    if combined.contains("checked out") || combined.contains("used by worktree") {
        return TransportError::invalid(format!(
            "`{name}` is the branch you are on, so it cannot be deleted. Switch to              another branch first."
        ));
    }
    TransportError::shell(output.message(what))
}
