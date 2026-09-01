use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What an issue *is*. Two variants where [`super::query::IssueState`] has
/// three, for the same reason a pull request's state has three where its
/// filter has four: `All` is a question, not an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitHubIssueState {
    Open,
    Closed,
}

/// One issue.
///
/// Untrusted text, like every other row that came from GitHub: the title and
/// the labels were written by whoever filed it. Rendered as text, never as
/// markup, never as argv.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubIssue {
    #[ts(type = "number")]
    pub number: u32,
    pub title: String,
    pub author: String,
    pub url: String,
    pub state: GitHubIssueState,
    /// Label names only. The colour and description GitHub also carries are
    /// not asked for: a list that has to be read at a glance is not improved
    /// by twenty tinted pills, and every field asked for is a field that can
    /// change shape between `gh` versions.
    pub labels: Vec<String>,
    #[ts(type = "number | null")]
    pub updated_ms: Option<u64>,
}
