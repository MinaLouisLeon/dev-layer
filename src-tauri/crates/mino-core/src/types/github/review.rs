use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One comment on a line of a pull request's diff.
///
/// **Untrusted text**, like everything else on the GitHub surface: the body
/// was written by whoever left the review, which on a public repository is
/// anybody at all. It is rendered as text, never as markup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubReviewComment {
    /// GitHub's own comment id, and the only thing that names it to
    /// [`super::query::GitHubQuery::ReplyToReviewComment`]. A number, so no
    /// text a caller holds reaches that argv.
    #[ts(type = "number")]
    pub id: u64,
    pub author: String,
    pub body: String,
    pub url: String,
    #[ts(type = "number | null")]
    pub created_ms: Option<u64>,
}

/// A comment and its replies, anchored to a line - or explicitly not anchored.
///
/// ## Why `line` is optional, and why `outdated` exists
///
/// A review comment is attached to a **position in a diff**, not to a line in
/// a file. When the pull request gets new commits, the diff it was written
/// against stops being the current one, and GitHub reports the comment with a
/// null position. The comment is still real and still worth reading; what it
/// is no longer is *placeable*.
///
/// This type says so rather than guessing. An outdated thread has no `line`,
/// is never drawn in the gutter against a line it might not belong to, and is
/// listed with the reason. Pinning it to `original_line` would put somebody
/// else's objection next to code that may have nothing to do with it, which is
/// worse than not showing it in place at all.
///
/// The second half of the same problem has no fix here and is worth knowing:
/// **the file open in the editor may not be the revision the comment was
/// written against.** Even a current thread's line number is a line number in
/// the pull request's head commit, and the editor is showing the working tree.
/// The gutter is therefore a convenience, and the thread carries its path and
/// its own address so the reader can always get to the real thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubReviewThread {
    /// The id of the comment that started it. Replies are sent to this.
    #[ts(type = "number")]
    pub id: u64,
    /// Repository-relative, as GitHub reports it - always forward slashes.
    pub path: String,
    /// The line in the pull request's diff, or `None` when the thread is
    /// outdated and has no current position.
    #[ts(type = "number | null")]
    pub line: Option<u32>,
    /// True when the diff this was written against is no longer current.
    pub outdated: bool,
    /// True when the thread has been marked resolved on GitHub.
    pub resolved: bool,
    /// The opening comment first, then its replies in the order they were
    /// left.
    pub comments: Vec<GitHubReviewComment>,
}

impl GitHubReviewThread {
    /// True when this thread can be drawn against a line of `path`.
    ///
    /// Asked by the gutter rather than re-derived from two fields at every
    /// call site, so "outdated threads are never pinned" is one rule in one
    /// place.
    pub fn is_placeable(&self) -> bool {
        !self.outdated && self.line.is_some()
    }
}
