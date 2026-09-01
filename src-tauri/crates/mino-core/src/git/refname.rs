//! Branch names: the caller value phase 4 adds, and how it is made safe.
//!
//! A branch name is neither a path nor a revision, so neither
//! [`super::guard`] nor [`super::revision`] can rule on it. It gets this,
//! which is deliberately two steps rather than one regex:
//!
//! 1. [`precheck`] refuses, without running anything, the shapes that must
//!    never reach argv at all - an empty name, an absurd length, a leading
//!    `-`, an ASCII control character, and anything the SSH quoting rule would
//!    have to refuse later anyway.
//! 2. [`check_argv`] hands the survivor to **`git check-ref-format`**, which
//!    is git's own answer to "is this a legal branch name". A hand-rolled
//!    regex here would be a second implementation of a rule git already owns,
//!    and the one place it disagreed would be a bug nobody could see.
//!
//! Why the leading `-` is refused *locally* and not left to git: `refs/heads/-x`
//! is a perfectly legal ref name, so `check-ref-format` would accept `-x` -
//! and `git checkout -x` reads it as an option. The two checks cover different
//! things, and only both of them together cover this.

use crate::error::{Result, TransportError};
use crate::types::MAX_BRANCH_NAME_BYTES;

/// The prefix `check-ref-format` is asked about.
///
/// `refs/heads/<name>` rather than `--branch <name>`: the `--branch` form
/// expands `@{-1}` and friends, which would make the check answer for a
/// *different* name than the one about to be used, and prefixing also means a
/// name beginning with a dash cannot be read as an option by the check itself.
const HEADS: &str = "refs/heads/";

/// Refusals that do not need git. Returns the trimmed name to use.
pub fn precheck(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(TransportError::invalid("a branch needs a name"));
    }
    if trimmed.len() > MAX_BRANCH_NAME_BYTES {
        return Err(TransportError::invalid(format!(
            "that branch name is {} bytes, above the {MAX_BRANCH_NAME_BYTES} byte ceiling",
            trimmed.len()
        )));
    }
    // The one that matters most, and the one `check-ref-format` will not make
    // for us: `-x` is a legal ref name and an illegal thing to hand `checkout`.
    if trimmed.starts_with('-') {
        return Err(TransportError::invalid(format!(
            "`{trimmed}` cannot be used as a branch name: it would be read as an option"
        )));
    }
    if let Some(bad) = trimmed.chars().find(|ch| ch.is_control()) {
        return Err(TransportError::invalid(format!(
            "a branch name cannot contain the control character U+{:04X}",
            bad as u32
        )));
    }
    // Refused here rather than by the SSH quoting rule later, so a name that
    // cannot work everywhere fails the same way on every transport instead of
    // working locally and erroring over SSH. `check-ref-format` refuses these
    // too; saying so before the call is a clearer sentence.
    if let Some(bad) = trimmed.chars().find(|ch| " \t'\"\\".contains(*ch)) {
        return Err(TransportError::invalid(format!(
            "a branch name cannot contain `{bad}`"
        )));
    }
    Ok(trimmed.to_string())
}

/// `git check-ref-format refs/heads/<name>`. Exit 0 means git accepts it.
pub fn check_argv(name: &str) -> Vec<String> {
    vec!["check-ref-format".to_string(), format!("{HEADS}{name}")]
}

/// What a failed [`check_argv`] should say. Git prints nothing on a refusal,
/// so the sentence is ours.
pub fn refused(name: &str) -> TransportError {
    TransportError::invalid(format!(
        "`{name}` is not a name git will accept for a branch. Branch names cannot \
         contain a space, `~`, `^`, `:`, `?`, `*`, `[`, `\\` or `..`, and cannot \
         end with `/`, `.` or `.lock`."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordinary_names_survive_the_precheck() {
        for name in ["main", "feat/git-branches-stash", "v1.2.3", "origin/main"] {
            assert_eq!(precheck(name).unwrap(), name);
        }
    }

    #[test]
    fn a_leading_dash_is_refused_here_and_not_left_to_git() {
        // `refs/heads/-x` is a legal ref name, so `check-ref-format` accepts
        // `-x`. This is the only check that catches it.
        let refusal = precheck("-x").unwrap_err().to_string();
        assert!(refusal.contains("option"), "{refusal}");
    }

    #[test]
    fn whitespace_quotes_and_control_characters_are_refused() {
        for name in ["a branch", "it's", "a\"b", "a\\b", "a\tb", "a\nb"] {
            assert!(precheck(name).is_err(), "{name:?} should be refused");
        }
    }

    #[test]
    fn empty_and_absurd_lengths_are_refused() {
        assert!(precheck("   ").is_err());
        assert!(precheck(&"a".repeat(MAX_BRANCH_NAME_BYTES + 1)).is_err());
    }

    #[test]
    fn the_check_asks_about_a_full_ref_never_a_bare_name() {
        // Prefixed, so the name cannot be read as an option by the check
        // itself and `@{-1}` cannot expand into a different name.
        assert_eq!(
            check_argv("main"),
            vec!["check-ref-format", "refs/heads/main"]
        );
    }
}
