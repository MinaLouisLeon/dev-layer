//! `gh pr list` and `gh pr view` rows.

use serde_json::Value;

use crate::error::Result;
use crate::types::{GitHubCheckState, GitHubPrState, GitHubPullRequest};

use super::{array, count, document, flag, instant, login, optional_text, protocol, text};

const PRS: &str = "the pull requests";

/// Every open pull request, in the order `gh` listed them.
pub fn pull_requests(stdout: &str) -> Result<Vec<GitHubPullRequest>> {
    let document = document(stdout, PRS)?;
    array(&document, PRS)?.iter().map(pull).collect()
}

/// One pull request, with its body. `gh pr view` answers with the object
/// itself rather than a list of one.
pub fn pull_request(stdout: &str) -> Result<GitHubPullRequest> {
    pull(&document(stdout, PRS)?)
}

fn pull(row: &Value) -> Result<GitHubPullRequest> {
    Ok(GitHubPullRequest {
        number: count(row, "number", PRS)?,
        title: text(row, "title", PRS)?,
        author: login(row, "author"),
        url: text(row, "url", PRS)?,
        state: state(&text(row, "state", PRS)?)?,
        is_draft: flag(row, "isDraft"),
        head_ref: text(row, "headRefName", PRS)?,
        base_ref: text(row, "baseRefName", PRS)?,
        checks: rollup(row.get("statusCheckRollup")),
        updated_ms: instant(row, "updatedAt"),
        // Absent from a list and present in a detail, which is the one
        // difference between the two calls that reach this function.
        body: optional_text(row, "body"),
    })
}

/// GitHub's three words for what a pull request is.
///
/// A word this build does not know is a protocol error rather than a fourth
/// state, because unlike a check conclusion there is nothing sensible to
/// render for it: a row that is neither open, closed nor merged is a row whose
/// meaning this build does not have.
fn state(word: &str) -> Result<GitHubPrState> {
    match word.to_ascii_uppercase().as_str() {
        "OPEN" => Ok(GitHubPrState::Open),
        "CLOSED" => Ok(GitHubPrState::Closed),
        "MERGED" => Ok(GitHubPrState::Merged),
        _ => Err(protocol(PRS)),
    }
}

/// What the checks on the head commit add up to.
///
/// `statusCheckRollup` is a list of two different shapes: a check run carries
/// `status` and `conclusion`, and an older commit status carries `state`. Both
/// are read, because a repository can have both at once, and a rollup that
/// silently ignored one half would report green for a red build.
///
/// Absent or empty means [`GitHubCheckState::Unknown`]: a repository with no
/// CI at all is not a repository whose checks passed.
fn rollup(value: Option<&Value>) -> GitHubCheckState {
    let Some(entries) = value.and_then(Value::as_array) else {
        return GitHubCheckState::Unknown;
    };
    GitHubCheckState::rollup(entries.iter().map(|entry| {
        match entry.get("status").and_then(Value::as_str) {
            Some(status) => {
                GitHubCheckState::from_run(status, entry.get("conclusion").and_then(Value::as_str))
            }
            None => GitHubCheckState::from_conclusion(
                entry
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
        }
    }))
}
