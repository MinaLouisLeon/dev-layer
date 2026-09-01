//! The one cheap question, and how its four answers are told apart.
//!
//! Three calls at most, in an order chosen so each failure means exactly one
//! thing:
//!
//! | Step | Fails when | Answer |
//! | --- | --- | --- |
//! | `gh` is on PATH | It is not installed | [`GitHubAvailability::Absent`] |
//! | `gh auth status` | Nobody is logged in | [`GitHubAvailability::Unauthenticated`] |
//! | `gh repo view` | Not a repository, no remote, or not a GitHub one | [`GitHubAvailability::Unsupported`] |
//!
//! Asking `auth status` separately from `repo view` is the point of the
//! order. Both fail without credentials, and only the first fails *because* of
//! them; collapsing the two would make "run `gh auth login`" and "this is not
//! a GitHub repository" the same sentence, which is the one thing a reader
//! must not be told.
//!
//! None of the first three answers is an error. Each is a state the GitHub
//! view renders in one sentence before going quiet for the rest of the
//! session - the same shape the `nu` and `git` probes already have.

use crate::error::Result;
use crate::types::{GitHubAvailability, GitHubProbe, GitHubRepository};

use super::parse::{document, optional_text, text};
use super::GhOutput;

/// The sentence a machine without `gh` gets. Not an error: a workbench on a
/// machine with no GitHub CLI is a workbench with a quiet GitHub view.
pub fn absent() -> GitHubProbe {
    GitHubProbe::quiet(
        GitHubAvailability::Absent,
        Some(
            "The GitHub CLI (gh) is not installed, or is not on PATH. Install it from \
             cli.github.com to see checks, pull requests and issues here."
                .to_string(),
        ),
    )
}

/// What a failed `gh auth status` means.
///
/// The sentence names the command and stops there. This app cannot log
/// anybody in - the handshake is interactive and the credential belongs to
/// `gh`'s keychain entry, not to this process - and pretending otherwise would
/// be the beginning of holding a token.
pub fn unauthenticated(output: &GhOutput) -> GitHubProbe {
    GitHubProbe::quiet(
        GitHubAvailability::Unauthenticated,
        Some(format!(
            "The GitHub CLI is not signed in. Run `gh auth login` in the terminal below, then \
             refresh this view.{}",
            trailing(output)
        )),
    )
}

/// What a failed `gh repo view` means.
///
/// Deliberately one state for three causes - no repository, no remote, a
/// remote pointing somewhere else - because the reader's next move is the same
/// for all three and `gh`'s own sentence, carried in `detail`, already says
/// which it was.
pub fn unsupported(output: &GhOutput) -> GitHubProbe {
    GitHubProbe::quiet(
        GitHubAvailability::Unsupported,
        Some(format!(
            "This folder has no GitHub repository. Only remotes pointing at GitHub are shown \
             here.{}",
            trailing(output)
        )),
    )
}

/// The repository `gh repo view --json` described.
pub fn repository(output: &GhOutput) -> Result<GitHubProbe> {
    const WHAT: &str = "the repository";
    let value = document(&output.stdout, WHAT)?;
    Ok(GitHubProbe::ready(GitHubRepository {
        name_with_owner: text(&value, "nameWithOwner", WHAT)?,
        url: text(&value, "url", WHAT)?,
        // A repository with no commits has no default branch yet, which is an
        // ordinary state and not a shape this build cannot read.
        default_branch: value
            .get("defaultBranchRef")
            .and_then(|reference| optional_text(reference, "name")),
    }))
}

/// `gh`'s own words appended to ours, when it had any.
///
/// Untrusted text like everything else `gh` prints, and carried as text. The
/// first line only: `gh auth status` is chatty on failure and a notice is not
/// a transcript.
fn trailing(output: &GhOutput) -> String {
    match output
        .stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
    {
        Some(line) => format!(" {line}"),
        None => String::new(),
    }
}
