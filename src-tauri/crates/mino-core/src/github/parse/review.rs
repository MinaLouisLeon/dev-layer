//! `gh api …/pulls/<n>/comments` into threads.
//!
//! Two things happen here that no other parser in this module does.
//!
//! **Comments become threads.** The endpoint answers a flat list; a reply
//! carries `in_reply_to_id` naming the comment it answers. Grouping is done
//! here rather than in the UI so both transports - and any later caller -
//! agree about what a thread is.
//!
//! **A thread that cannot be placed says so.** A review comment is anchored to
//! a position in a diff, and when the pull request gains commits that diff
//! stops being current: GitHub then reports `line: null`, and this reports
//! `outdated: true` with no line. The alternative - falling back to
//! `original_line` - would pin somebody's objection to whatever now happens to
//! sit at that number, which is worse than not placing it at all.

use serde_json::Value;

use crate::error::Result;
use crate::types::{GitHubReviewComment, GitHubReviewThread};

use super::{array, document, instant, login, number, optional_text, protocol, text};

const THREADS: &str = "the review comments";

/// Every thread on the pull request, in the order its opening comments were
/// left.
pub fn review_threads(stdout: &str) -> Result<Vec<GitHubReviewThread>> {
    let document = document(stdout, THREADS)?;
    let rows = array(&document, THREADS)?;

    let mut threads: Vec<GitHubReviewThread> = Vec::new();
    for row in rows {
        let comment = comment(row)?;
        match reply_to(row) {
            // A reply joins the thread it answers, when that thread is here.
            // GitHub can report a reply whose parent was deleted; it becomes
            // a thread of its own rather than being dropped, because a
            // comment nobody can see is worse than one shown alone.
            Some(parent) => match threads.iter_mut().find(|thread| thread.id == parent) {
                Some(thread) => thread.comments.push(comment),
                None => threads.push(thread_from(row, comment)?),
            },
            None => threads.push(thread_from(row, comment)?),
        }
    }
    Ok(threads)
}

/// One thread, opened by `row`.
fn thread_from(row: &Value, opening: GitHubReviewComment) -> Result<GitHubReviewThread> {
    // `line` is null exactly when the diff this was written against is no
    // longer the current one. That is the whole outdated test - and it is
    // GitHub's own answer rather than something inferred here.
    let line = row
        .get("line")
        .and_then(Value::as_u64)
        .and_then(|line| u32::try_from(line).ok());

    Ok(GitHubReviewThread {
        id: opening.id,
        path: text(row, "path", THREADS)?,
        line,
        outdated: line.is_none(),
        // GitHub's REST comment carries no thread resolution, so this is false
        // rather than guessed. It is on the type because the UI has somewhere
        // to put it the day the GraphQL read lands, and because a thread that
        // silently *looked* unresolved would be worse than one that says so.
        resolved: false,
        comments: vec![opening],
    })
}

fn comment(row: &Value) -> Result<GitHubReviewComment> {
    Ok(GitHubReviewComment {
        id: number(row, "id", THREADS)?,
        author: login(row, "user"),
        // Untrusted text, carried as text. Rendered by the UI in a preformatted
        // block, never through a Markdown renderer.
        body: optional_text(row, "body").unwrap_or_default(),
        url: text(row, "html_url", THREADS)?,
        created_ms: instant(row, "created_at"),
    })
}

/// The comment this one replies to, when it is a reply.
fn reply_to(row: &Value) -> Option<u64> {
    row.get("in_reply_to_id").and_then(Value::as_u64)
}

/// Kept beside the readers above because it is the same question asked of the
/// same document: the thread a reply just landed in.
///
/// `gh api` answers a reply with the new comment alone, so the thread is read
/// back rather than assembled from what the caller already held - the same
/// judgement `create_branch` makes about the branch it just made.
pub fn thread_containing(stdout: &str, comment_id: u64) -> Result<GitHubReviewThread> {
    let threads = review_threads(stdout)?;
    threads
        .into_iter()
        .find(|thread| {
            thread.id == comment_id || thread.comments.iter().any(|reply| reply.id == comment_id)
        })
        .ok_or_else(|| {
            protocol("the thread that reply was added to, which GitHub did not list afterwards")
        })
}
