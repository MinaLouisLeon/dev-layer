//! One query turned into one `gh` call, and one `gh` call read back.
//!
//! This is the file that makes two transports out of one implementation. A
//! transport's whole job for `query` is:
//!
//! 1. [`plan`] - decide the argv, the stdin and what the answer will look
//!    like. Every guard runs here: the path guard, the branch guard, and the
//!    validation in front of the one call that writes.
//! 2. Run it. The *only* part that differs between local and SSH.
//! 3. [`read`], or [`failure`] - turn the output into a typed response, or
//!    into a sentence.
//!
//! Neither transport parses anything and neither builds an argument, so the
//! two cannot drift into disagreeing about what `gh` said - the same
//! arrangement [`crate::git`] and [`crate::search`] already use.

use crate::error::{Result, TransportError};
use crate::git::paths::PathStyle;
use crate::types::{GitHubQuery, GitHubResponse};

use super::{browse, command, create, message_or, parse, GhOutput};

/// What the answer to a call will look like.
///
/// Carried alongside the argv rather than inferred from it afterwards, so
/// reading an answer never involves guessing which question produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Runs,
    Jobs,
    PullRequests,
    PullRequest,
    Issues,
    Created,
    Url,
    ReviewThreads,
    /// The thread a reply landed in, found by the comment it answered.
    ///
    /// The only shape that carries a value. `gh api` answers a reply with the
    /// new comment alone, so the thread is read back - and reading it back
    /// means knowing which of them to pick out.
    ReviewThread {
        comment_id: u64,
    },
}

impl Shape {
    /// The operation named in a failure sentence, in `gh`'s own vocabulary so
    /// a reader can go and run the same thing.
    fn what(self) -> &'static str {
        match self {
            Self::Runs => "run list",
            Self::Jobs => "run view",
            Self::PullRequests => "pr list",
            Self::PullRequest => "pr view",
            Self::Issues => "issue list",
            Self::Created => "pr create",
            Self::Url => "browse",
            Self::ReviewThreads | Self::ReviewThread { .. } => "api",
        }
    }
}

/// Everything a transport needs to make one call, and nothing about how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhCall {
    pub argv: Vec<String>,
    /// Written to `gh`'s standard input, then closed. Only the two bodies -
    /// a pull request's and a review reply's - travel this way, and that is
    /// the whole reason the field exists; see [`super::command`].
    pub input: Option<String>,
    /// A second call, run only if the first succeeded, whose output is what
    /// [`read`] is given.
    ///
    /// One variant needs it. A reply is a `POST` that answers with the new
    /// comment alone, and what the caller wants is the thread - so the thread
    /// is **read back** rather than assembled by appending the answer to a
    /// list the UI already held. Putting it here rather than in the two
    /// transports keeps them at "plan, run, read" and keeps the sequence in
    /// one place.
    pub follow_up: Option<Vec<String>>,
    pub shape: Shape,
}

/// The call one query becomes, with every caller value already ruled on.
///
/// `root` and `style` are the session's, and are used by exactly one variant:
/// `BrowseUrl` names a file, and a file the session does not own must not be
/// nameable. Everything else here takes values that are numbers, enums or
/// branch names, each guarded by the module that owns that kind of value.
pub fn plan(query: &GitHubQuery, root: &str, style: PathStyle) -> Result<GhCall> {
    Ok(match query {
        GitHubQuery::Runs { branch, limit } => {
            // The same guard a checkout uses. A branch name that could be read
            // as an option never reaches an argv anywhere in this codebase.
            let branch = crate::git::refname::precheck(branch)?;
            GhCall {
                argv: command::runs_argv(&branch, *limit),
                input: None,
                follow_up: None,
                shape: Shape::Runs,
            }
        }
        GitHubQuery::RunJobs { run_id } => GhCall {
            argv: command::run_jobs_argv(*run_id),
            input: None,
            follow_up: None,
            shape: Shape::Jobs,
        },
        GitHubQuery::PullRequests { state, limit } => GhCall {
            argv: command::pull_requests_argv(*state, *limit),
            input: None,
            follow_up: None,
            shape: Shape::PullRequests,
        },
        GitHubQuery::PullRequest { number } => GhCall {
            argv: command::pull_request_argv(*number),
            input: None,
            follow_up: None,
            shape: Shape::PullRequest,
        },
        GitHubQuery::Issues { state, limit } => GhCall {
            argv: command::issues_argv(*state, *limit),
            input: None,
            follow_up: None,
            shape: Shape::Issues,
        },
        GitHubQuery::CreatePullRequest {
            title,
            body,
            base,
            draft,
        } => {
            let (title, base) = create::validate(title, body, base)?;
            GhCall {
                argv: command::create_pr_argv(&title, &base, *draft),
                // The body, and the only value on this interface that never
                // reaches an argument list.
                input: Some(body.to_string()),
                follow_up: None,
                shape: Shape::Created,
            }
        }
        GitHubQuery::ReviewComments { number } => GhCall {
            argv: command::review_comments_argv(*number),
            input: None,
            follow_up: None,
            shape: Shape::ReviewThreads,
        },
        GitHubQuery::ReplyToReviewComment {
            number,
            comment_id,
            body,
        } => GhCall {
            argv: command::reply_argv(*comment_id),
            // JSON, built by serde rather than formatted, and on stdin - so a
            // reply containing a quote or a newline is a reply.
            input: Some(command::reply_body(body)),
            // The thread as it now stands, rather than the one comment `gh`
            // hands back. See the field's own doc.
            follow_up: Some(command::thread_argv(*number)),
            shape: Shape::ReviewThread {
                comment_id: *comment_id,
            },
        },
        GitHubQuery::BrowseUrl { path, line, branch } => {
            let target = browse::target(root, path, *line, style)?;
            let branch = browse::branch(branch.as_deref())?;
            GhCall {
                argv: command::browse_argv(&target, branch.as_deref()),
                input: None,
                follow_up: None,
                shape: Shape::Url,
            }
        }
    })
}

/// A successful call's output as a typed response.
pub fn read(shape: Shape, output: &GhOutput) -> Result<GitHubResponse> {
    Ok(match shape {
        Shape::Runs => GitHubResponse::Runs(parse::runs(&output.stdout)?),
        Shape::Jobs => GitHubResponse::Jobs(parse::jobs(&output.stdout)?),
        Shape::PullRequests => GitHubResponse::PullRequests(parse::pull_requests(&output.stdout)?),
        Shape::PullRequest => GitHubResponse::PullRequest(parse::pull_request(&output.stdout)?),
        Shape::Issues => GitHubResponse::Issues(parse::issues(&output.stdout)?),
        Shape::ReviewThreads => {
            GitHubResponse::ReviewThreads(parse::review_threads(&output.stdout)?)
        }
        // Read out of the thread list the follow-up call fetched, rather than
        // out of the reply's own answer - see `GhCall::follow_up`.
        Shape::ReviewThread { comment_id } => {
            GitHubResponse::ReviewThread(parse::thread_containing(&output.stdout, comment_id)?)
        }
        Shape::Created => {
            let (url, number) = create::parse(&output.stdout)?;
            GitHubResponse::Created(crate::types::GitHubCreated { url, number })
        }
        // The URL and nothing else. `gh browse --no-browser` prints one line.
        Shape::Url => GitHubResponse::Url(
            output
                .stdout
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_string(),
        ),
    })
}

/// What a non-zero exit should say.
///
/// `gh`'s own words wherever it had any, because they are usually the most
/// useful thing available - a rate limit, a network failure, a repository that
/// has moved. `browse` is the exception: it says almost nothing on failure, so
/// [`super::browse::refused`] supplies the sentence instead.
pub fn failure(call: &GhCall, query: &GitHubQuery, output: &GhOutput) -> TransportError {
    if let (Shape::Url, GitHubQuery::BrowseUrl { path, .. }) = (call.shape, query) {
        if output.stderr.trim().is_empty() {
            return browse::refused(path);
        }
    }
    TransportError::shell(message_or(output, call.shape.what()))
}
