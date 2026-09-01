//! `gh issue list` rows.

use serde_json::Value;

use crate::error::Result;
use crate::types::{GitHubIssue, GitHubIssueState};

use super::{array, count, document, instant, login, protocol, text};

const ISSUES: &str = "the issues";

/// Every issue `gh` listed, in the order it listed them.
pub fn issues(stdout: &str) -> Result<Vec<GitHubIssue>> {
    let document = document(stdout, ISSUES)?;
    array(&document, ISSUES)?.iter().map(issue).collect()
}

fn issue(row: &Value) -> Result<GitHubIssue> {
    Ok(GitHubIssue {
        number: count(row, "number", ISSUES)?,
        title: text(row, "title", ISSUES)?,
        author: login(row, "author"),
        url: text(row, "url", ISSUES)?,
        state: state(&text(row, "state", ISSUES)?)?,
        labels: labels(row.get("labels")),
        updated_ms: instant(row, "updatedAt"),
    })
}

/// The two words an issue can be.
fn state(word: &str) -> Result<GitHubIssueState> {
    match word.to_ascii_uppercase().as_str() {
        "OPEN" => Ok(GitHubIssueState::Open),
        "CLOSED" => Ok(GitHubIssueState::Closed),
        _ => Err(protocol(ISSUES)),
    }
}

/// Label names, and nothing else `gh` carries about them.
///
/// A label with no name is dropped rather than reported: labels decorate a
/// row, and losing the whole issue because one of its decorations was odd
/// would be the wrong trade. That is the same judgement [`super::login`]
/// makes - a missing *value* is not a shape change.
fn labels(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("name")?.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
