//! What one git call's output *means*.
//!
//! Split from `mod.rs` so that file stays the shape of the module - the probe,
//! the error sentences, and the type both transports fill in - and this one
//! holds the reading of it. Neither transport reads git's output itself, which
//! is what stops local and SSH drifting into disagreeing about what it said.

use crate::error::{Result, TransportError};
use crate::types::{GitEntry, GitStatus};

use super::paths::PathStyle;
use super::{porcelain, GitOutput};

/// Reads `git rev-parse --show-toplevel`.
///
/// A non-zero exit that says "not a git repository" is the answer, not a
/// failure. Anything else - a broken install, git's dubious-ownership refusal -
/// is reported, because those are conditions the user can act on and silently
/// calling them "no repository" would hide them.
pub fn repository_root(output: &GitOutput) -> Result<Option<String>> {
    if output.succeeded() {
        let root = output.stdout.trim();
        return Ok((!root.is_empty()).then(|| root.to_string()));
    }
    if output
        .stderr
        .to_lowercase()
        .contains("not a git repository")
    {
        return Ok(None);
    }
    Err(TransportError::shell(output.message("rev-parse")))
}

/// Turns one status call into a [`GitStatus`], keeping only the entries inside
/// `session_root`.
///
/// The filter is the reason this takes two roots. The repository root may sit
/// *above* the connected root - opening `repo/src` is ordinary - and git
/// answers for the whole tree, so rows for files the session cannot otherwise
/// see are dropped here rather than being handed to a UI that would offer to
/// open them.
pub fn status_from(
    output: &GitOutput,
    repository_root: String,
    session_root: &str,
    style: PathStyle,
) -> Result<GitStatus> {
    if !output.succeeded() {
        return Err(TransportError::shell(output.message("status")));
    }
    let parsed = porcelain::parse(&output.stdout, &repository_root, style);
    let entries: Vec<GitEntry> = parsed
        .entries
        .into_iter()
        .filter(|entry| style.within(session_root, &entry.path))
        .collect();

    Ok(GitStatus {
        repository: parsed.headers.into_repository(repository_root),
        entries,
        truncated: parsed.truncated,
    })
}

/// The ignore rows from a status, for the search walk. A failed call is not an
/// error here: search must keep working with git absent or broken, so the
/// caller gets an empty list and falls back to the built-in skip list.
pub fn ignored_from(output: &GitOutput) -> Vec<String> {
    if !output.succeeded() {
        return Vec::new();
    }
    porcelain::parse_ignored(&output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(code: i32, stdout: &str, stderr: &str) -> GitOutput {
        GitOutput {
            code: Some(code),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn not_a_repository_is_an_answer_not_a_failure() {
        let err = output(
            128,
            "",
            "fatal: not a git repository (or any of the parent directories)",
        );
        assert_eq!(repository_root(&err).unwrap(), None);
    }

    #[test]
    fn a_different_failure_is_still_a_failure() {
        let err = output(
            128,
            "",
            "fatal: detected dubious ownership in repository at '/srv/app'",
        );
        let reported = repository_root(&err).unwrap_err();
        assert!(
            matches!(reported, TransportError::Shell { message } if message.contains("dubious"))
        );
    }

    #[test]
    fn rows_outside_the_session_root_never_appear() {
        // The repository root sits above the connected folder, which is what
        // happens whenever someone opens a sub-directory of a checkout.
        let recorded = "# branch.head main\0\
                        1 .M N... 100644 100644 100644 a b src/main.rs\0\
                        1 .M N... 100644 100644 100644 a b docs/readme.md\0";
        let status = status_from(
            &output(0, recorded, ""),
            "/srv/app".to_string(),
            "/srv/app/src",
            PathStyle::posix(),
        )
        .unwrap();
        let paths: Vec<&str> = status.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["/srv/app/src/main.rs"]);
        // The repository still reports its own root, which is above the session.
        assert_eq!(status.repository.root, "/srv/app");
    }

    #[test]
    fn a_failed_ignore_call_degrades_to_an_empty_list() {
        assert!(ignored_from(&output(128, "", "fatal: whatever")).is_empty());
        assert_eq!(
            ignored_from(&output(0, "! target/\0", "")),
            vec!["target".to_string()]
        );
    }
}
