//! What a stash call actually did, and what a refusal should say.
//!
//! Split from the parser because it is a different job - one reads the stack,
//! this reads whether an action happened - and both are shared by every
//! transport, so a conflicting pop says the same thing over SSH as it does
//! locally.
//!
//! The case this file exists for is [`pushed`]: `git stash push` on a clean
//! tree **exits zero**. Reading the exit code alone would report work set
//! aside that was never touched.

use crate::error::{Result, TransportError};

use super::super::GitOutput;

/// Git's own wording when there was nothing to set aside. Matched on stdout as
/// well as stderr, because stdout is where git puts it.
const NOTHING_TO_STASH: &str = "no local changes to save";

/// Whether a `stash push` actually set anything aside.
///
/// **Not the same question as "did it exit zero".** A clean tree gets exit 0
/// and `No local changes to save`, so the exit code alone would have this
/// report a stash that does not exist. Both transports go through here rather
/// than checking `succeeded()` themselves, so neither can forget.
pub fn pushed(output: &GitOutput) -> Result<()> {
    if !output.succeeded() {
        return Err(failure(output, "stash push"));
    }
    if said_nothing_to_stash(output) {
        return Err(nothing_to_stash());
    }
    Ok(())
}

fn said_nothing_to_stash(output: &GitOutput) -> bool {
    format!("{} {}", output.stdout, output.stderr)
        .to_lowercase()
        .contains(NOTHING_TO_STASH)
}

fn nothing_to_stash() -> TransportError {
    TransportError::invalid("there is nothing to stash: the working tree matches the last commit.")
}

/// Turns a failed stash call into the sentence worth showing.
///
/// The conflict case is the one that matters: `pop` leaves the entry on the
/// stack when it conflicts, so the reader has to be told the work is still
/// there. Full conflict resolution is phase 6; saying where things stand is
/// this phase's job.
pub fn failure(output: &GitOutput, what: &str) -> TransportError {
    let combined = format!("{} {}", output.stdout, output.stderr).to_lowercase();

    if combined.contains("conflict") || combined.contains("could not restore untracked files") {
        return TransportError::invalid(
            "the stash could not be applied cleanly - it conflicts with the working \
             tree. The entry is still on the stack, and the conflicting files are \
             marked in the working tree.",
        );
    }
    if combined.contains("is not a valid reference")
        || combined.contains("is not a stash")
        || combined.contains("no stash entries found")
        || combined.contains("log for 'refs/stash' only has")
    {
        return TransportError::invalid(
            "that stash entry is no longer there. The list has been re-read.",
        );
    }
    if combined.contains(NOTHING_TO_STASH) {
        return nothing_to_stash();
    }
    TransportError::shell(output.message(what))
}
