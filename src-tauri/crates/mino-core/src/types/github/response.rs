use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::issue::GitHubIssue;
use super::pull_request::{GitHubCreated, GitHubPullRequest};
use super::review::GitHubReviewThread;
use super::run::{GitHubJob, GitHubRun};

/// What one [`super::query::GitHubQuery`] answered with.
///
/// A tagged union rather than seven return types, because there is one
/// transport method rather than seven. The caller matched on a variant to ask
/// and matches on one to read, which is the trade `query` makes: two methods
/// serve five features, and the cost is that a caller has to say what it
/// expected.
///
/// The variants line up with the query's one for one, which is what makes a
/// mismatch - asking for issues and being handed runs - a case the client can
/// name rather than a shape it silently mis-reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitHubResponse {
    Runs(Vec<GitHubRun>),
    Jobs(Vec<GitHubJob>),
    PullRequests(Vec<GitHubPullRequest>),
    PullRequest(GitHubPullRequest),
    Issues(Vec<GitHubIssue>),
    ReviewThreads(Vec<GitHubReviewThread>),
    /// The answer to a create. One of the two queries that write.
    Created(GitHubCreated),
    /// The answer to a reply: the thread as it now stands, so the caller shows
    /// what exists rather than appending its own guess at it.
    ReviewThread(GitHubReviewThread),
    /// The answer to `BrowseUrl`. A URL, not an opened browser.
    Url(String),
}
