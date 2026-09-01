//! The one caller value that is neither a path nor a message.
//!
//! `DiffRequest::against` and the sha handed to `show` are revisions: a sha, a
//! branch name, `HEAD~3`, `origin/main`. The path guard cannot rule on them -
//! none of them is a filesystem path, and it would refuse every one - so they
//! get their own check.
//!
//! What the check is actually for: a revision reaches argv, and over SSH it
//! reaches a command line. Two things must be impossible.
//!
//! - **It must not be readable as an option.** `--upload-pack=...` and
//!   `--output=...` are real git options that run a program or write a file. A
//!   leading `-` is refused outright, which is why the argv builders also place
//!   revisions in front of the `--` separator and never behind it.
//! - **It must not carry anything a shell would notice.** Quoting over SSH
//!   already refuses a single quote, but a revision has no legitimate reason to
//!   contain a space, a semicolon or a backtick either, so the allow-list below
//!   is the narrow one rather than the broad one.

use crate::error::{Result, TransportError};

/// Longest revision worth accepting. A sha is 40, and the longest sensible
/// expression - a branch name with `~` and `^` on it - is far short of this.
const MAX_LENGTH: usize = 256;

/// Everything git's revision grammar needs, and nothing else.
///
/// `^` and `~` walk to ancestors, `@{...}` reaches the reflog and upstream,
/// `:` separates a revision from a path in `HEAD:file`, and `/` appears in
/// every remote-tracking name. Alphanumerics, `.`, `-` and `_` cover the names
/// themselves.
fn allowed(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || "/._-^~@{}:".contains(ch)
}

/// Returns the revision unchanged, or says why it cannot be used.
pub fn validate(revision: &str) -> Result<String> {
    let trimmed = revision.trim();
    if trimmed.is_empty() {
        return Err(TransportError::invalid("a revision cannot be empty"));
    }
    if trimmed.len() > MAX_LENGTH {
        return Err(TransportError::invalid(
            "that revision is too long to be a real one",
        ));
    }
    // Refused first, and separately, because this is the one that matters:
    // everything git reads as an option starts here.
    if trimmed.starts_with('-') {
        return Err(TransportError::invalid(format!(
            "`{trimmed}` cannot be used as a revision: it would be read as an option"
        )));
    }
    if let Some(bad) = trimmed.chars().find(|ch| !allowed(*ch)) {
        return Err(TransportError::invalid(format!(
            "`{bad}` is not a character a git revision can contain"
        )));
    }
    Ok(trimmed.to_string())
}

/// The same, for an optional revision. `None` stays `None` rather than
/// becoming an error, because most diffs do not name one.
pub fn validate_optional(revision: Option<&str>) -> Result<Option<String>> {
    revision.map(validate).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordinary_revisions_are_all_accepted() {
        for revision in [
            "HEAD",
            "main",
            "origin/main",
            "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
            "HEAD~3",
            "HEAD^2",
            "feat/git-history",
            "v1.2.3",
            "@{upstream}",
        ] {
            assert!(validate(revision).is_ok(), "{revision} should be usable");
        }
    }

    #[test]
    fn anything_that_could_be_read_as_an_option_is_refused() {
        // The case this file exists for. `--upload-pack` runs a program and
        // `--output` writes a file, and both are real git options.
        for revision in ["--upload-pack=sh", "--output=/tmp/x", "-n", "--"] {
            let refused = validate(revision).unwrap_err();
            assert!(
                format!("{refused}").contains("option")
                    || format!("{refused}").contains("not a character"),
                "{revision} should be refused: {refused}"
            );
        }
    }

    #[test]
    fn anything_a_shell_would_notice_is_refused() {
        for revision in ["main; rm -rf /", "$(whoami)", "a`b`", "main branch", "it's"] {
            assert!(validate(revision).is_err(), "{revision} should be refused");
        }
    }

    #[test]
    fn empty_and_absurd_lengths_are_refused() {
        assert!(validate("   ").is_err());
        assert!(validate(&"a".repeat(MAX_LENGTH + 1)).is_err());
    }

    #[test]
    fn absent_is_not_an_error() {
        assert_eq!(validate_optional(None).unwrap(), None);
        assert_eq!(
            validate_optional(Some(" HEAD ")).unwrap(),
            Some("HEAD".to_string())
        );
    }
}
