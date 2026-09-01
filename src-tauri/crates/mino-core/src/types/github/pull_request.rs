use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::check::GitHubCheckState;

/// What a pull request *is*, as opposed to what may be asked for.
///
/// Three variants where [`super::query::PrState`] has four: `All` is a filter
/// and not a condition anything can be in. Keeping the two apart is what stops
/// a list request and a row's own state being the same type by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitHubPrState {
    Open,
    Closed,
    Merged,
}

/// One pull request.
///
/// **Every text field here is untrusted.** The title and the body were written
/// by whoever opened it, which on a public repository is anybody at all. They
/// are rendered as text, never as markup, and never interpolated into a
/// command - the same discipline the transport already applies to filenames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubPullRequest {
    #[ts(type = "number")]
    pub number: u32,
    pub title: String,
    /// The login that opened it, or an empty string for a deleted account.
    pub author: String,
    pub url: String,
    pub state: GitHubPrState,
    pub is_draft: bool,
    /// The branch being merged, and the branch it is merging into.
    pub head_ref: String,
    pub base_ref: String,
    /// What the checks on the head commit add up to.
    pub checks: GitHubCheckState,
    #[ts(type = "number | null")]
    pub updated_ms: Option<u64>,
    /// The description. **`None` in a list**, and filled only by
    /// [`super::query::GitHubQuery::PullRequest`]: a body is the largest field
    /// on this type and is read one at a time, so paying for twenty of them to
    /// show twenty titles would be paying for nineteen nobody opened.
    pub body: Option<String>,
}

/// What a successful create answered with.
///
/// The URL is the point. A pull request that was made and whose address the
/// author has to go and find is a pull request the app only half opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubCreated {
    pub url: String,
    /// `None` when `gh` printed a URL this build could not read a number out
    /// of. The URL is what the reader needs; the number is a convenience.
    #[ts(type = "number | null")]
    pub number: Option<u32>,
}
