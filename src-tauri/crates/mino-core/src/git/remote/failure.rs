//! Why a remote call was refused, as a sentence somebody can act on.
//!
//! Split from the parsers beside it for the reason `branches/failure.rs` is
//! split from `branches.rs`: one file reads what git listed, and this one
//! reads why git said no. Both are shared by every transport, so a rejected
//! push produces the same sentence over SSH that it does locally.
//!
//! **Everything here is redacted.** This is the one module in the crate whose
//! whole input is text written by somebody else's git, on a call that talked
//! to a network - which makes it the one place a credential can appear without
//! anybody having put it there. See [`super::super::redact`].

use crate::error::TransportError;

use super::super::redact::{redact, summary};
use super::super::GitOutput;

/// What a failed remote call should say.
///
/// The three that matter are recognised by git's own stable phrases and turned
/// into sentences that name the next move. Everything else falls through to
/// git's words - **redacted**, which is the whole reason this function exists
/// rather than callers reaching for `output.message()` as they do elsewhere.
pub fn failure(output: &GitOutput, what: &str) -> TransportError {
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let lower = combined.to_lowercase();

    // The one the reader can definitely fix, and the one D3 makes possible.
    if lower.contains("could not read username")
        || lower.contains("terminal prompts disabled")
        || lower.contains("authentication failed")
        || lower.contains("permission denied (publickey")
    {
        return TransportError::invalid(
            "git could not authenticate to the remote, and this app never holds a credential \
             of its own. Configure a git credential helper, or add your key to the SSH agent, \
             then try again.",
        );
    }
    // Before the general rejection below, and the order is the point: a
    // refused lease *also* prints `[rejected]`, and it is a different thing
    // with a different reassurance attached. Matching the general case first
    // would tell somebody who was force-pushing to go and pull.
    if lower.contains("stale info") {
        return TransportError::invalid(
            "the remote branch moved since this repository last saw it, so the force push was \
             refused rather than overwriting work you have not seen. Fetch first.",
        );
    }
    // A rejection is not a failure of the push - it is the remote saying you
    // are out of date. Naming the fix is most of the value here.
    if lower.contains("non-fast-forward") || lower.contains("[rejected]") {
        return TransportError::invalid(
            "the remote has commits this branch does not. Fetch and pull first, then push \
             again. Nothing was pushed.",
        );
    }
    if lower.contains("no such remote") || lower.contains("does not appear to be a git repository")
    {
        return TransportError::invalid(redact(
            "that remote is not configured, or is not reachable as a git repository.",
        ));
    }
    TransportError::shell(
        summary(&combined).unwrap_or_else(|| format!("git {what} failed with no explanation")),
    )
}

/// Raised when a pull is asked for on a working tree that has uncommitted
/// changes.
///
/// **Refused rather than stashed.** Stashing on the reader's behalf would be
/// this app moving their work somewhere they did not put it, and a stash they
/// did not make is a stash they will not think to look for. Naming the two
/// things they can do is the whole of the help worth giving.
pub fn dirty() -> TransportError {
    TransportError::invalid(
        "there are uncommitted changes in the working tree, and a pull could overwrite them. \
         Commit or stash them first - the Stash section below will set them aside and give \
         them back afterwards.",
    )
}

/// Raised when a push is asked for on a detached HEAD.
pub fn detached() -> TransportError {
    TransportError::invalid(
        "there is no branch checked out, so there is nothing to push. Check out a branch first.",
    )
}
