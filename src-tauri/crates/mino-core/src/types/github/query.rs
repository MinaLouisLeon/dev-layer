use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many rows a list query asks for when the caller does not say.
pub const DEFAULT_GITHUB_LIMIT: u32 = 20;

/// The ceiling on any list query. Not a performance guard: it is the rate
/// limit. Every one of these is a real API call made on somebody's account,
/// and a pane that asked for five hundred rows to show ten would be spending
/// a budget the reader cannot see.
pub const MAX_GITHUB_LIMIT: u32 = 100;

/// Ceiling on a pull request title. GitHub's own limit is 256 characters;
/// this is bytes and generous enough that no honest title meets it.
pub const MAX_PR_TITLE_BYTES: usize = 1024;

/// Ceiling on a pull request body. Well under GitHub's 65536-character limit,
/// and large enough for any description written in this pane.
pub const MAX_PR_BODY_BYTES: usize = 60_000;

/// Which pull requests to list. A filter, not a state: `All` is a thing to
/// ask for and not a thing a pull request can be - see
/// [`super::pull_request::GitHubPrState`] for the other one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum PrState {
    #[default]
    Open,
    Closed,
    Merged,
    All,
}

/// Which issues to list. The same distinction, one variant shorter: an issue
/// is never merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum IssueState {
    #[default]
    Open,
    Closed,
    All,
}

/// One `gh` subcommand, named rather than spelled.
///
/// SECURITY, and the reason this is an enum rather than a string: the caller
/// picks a *variant*, and the program text for it lives in
/// [`crate::github::command`]. There is no shape of this type that lets a
/// caller name a subcommand, add a flag, or reach a `gh` call this crate has
/// not written down. The fields are values, and each one travels as its own
/// argv element - or, for the pull request body, on stdin.
///
/// Five features share two transport methods because of this. The
/// alternative - ten methods, each with its own signature, its own Tauri
/// command and its own client method - is the same surface written five times
/// over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitHubQuery {
    /// `gh run list` for one branch. The checks section's whole read.
    #[serde(rename_all = "camelCase")]
    Runs {
        branch: String,
        #[ts(type = "number")]
        limit: u32,
    },

    /// `gh run view --json jobs`: which job in a run failed.
    ///
    /// Not in the original five, and it earns its place: `gh run list` reports
    /// a run's conclusion and never its jobs, so "the pipeline failed" is all
    /// one call can say. Naming the job that failed is the difference between
    /// a notification and something worth acting on, and it is one more fixed
    /// subcommand rather than a second shape of call.
    #[serde(rename_all = "camelCase")]
    RunJobs {
        #[ts(type = "number")]
        run_id: u64,
    },

    #[serde(rename_all = "camelCase")]
    PullRequests {
        state: PrState,
        #[ts(type = "number")]
        limit: u32,
    },

    /// One pull request, with its body. The list deliberately does not carry
    /// bodies: they are the largest field and are only read one at a time.
    #[serde(rename_all = "camelCase")]
    PullRequest {
        #[ts(type = "number")]
        number: u32,
    },

    #[serde(rename_all = "camelCase")]
    Issues {
        state: IssueState,
        #[ts(type = "number")]
        limit: u32,
    },

    /// **The one query that writes.** Confirmed in the UI before it is sent,
    /// showing exactly what will be created, and answering with the URL it
    /// made rather than leaving the author to go and look.
    ///
    /// The body travels on stdin, for the same reason a commit message does:
    /// over SSH the argv becomes a command line, and a description with an
    /// apostrophe in it must not be a refusal.
    #[serde(rename_all = "camelCase")]
    CreatePullRequest {
        title: String,
        body: String,
        base: String,
        draft: bool,
    },

    /// Every review thread on one pull request, for #17.
    ///
    /// Read-only, and read through `gh api` rather than a `gh pr` subcommand:
    /// line-anchored review comments are not on any `--json` field `gh pr
    /// view` offers. The path is fixed program text with `{owner}` and
    /// `{repo}` placeholders `gh` fills in from the checkout, so this cannot
    /// be pointed at another repository.
    #[serde(rename_all = "camelCase")]
    ReviewComments {
        #[ts(type = "number")]
        number: u32,
    },

    /// **The second query that writes.** Adds a reply to an existing thread.
    ///
    /// A reply and not a new comment, which is a deliberate limit: a
    /// top-level review comment has to name a commit and a diff position, and
    /// getting either wrong puts somebody's objection against the wrong line.
    /// Replying needs only the thread, which the reader is looking at.
    ///
    /// The body travels on **stdin** as JSON, like a pull request body.
    #[serde(rename_all = "camelCase")]
    ReplyToReviewComment {
        /// The pull request the thread is on. Carried because the reply is
        /// answered by re-reading the thread rather than by appending the new
        /// comment to a list the caller already holds - the same judgement
        /// `create_branch` makes about the branch it just made.
        #[ts(type = "number")]
        number: u32,
        #[ts(type = "number")]
        comment_id: u64,
        body: String,
    },

    /// The web address of one file, for #19. Answers with the URL and opens
    /// nothing: what to do with it is the caller's decision, and a transport
    /// that launched a browser as a side effect of a method called `query`
    /// would be a surprise.
    ///
    /// `path` is guarded against the connected root exactly as a git path is,
    /// so this cannot name a file the session does not own.
    #[serde(rename_all = "camelCase")]
    BrowseUrl {
        path: String,
        #[ts(type = "number | null")]
        line: Option<u32>,
        /// The branch to link to. `None` takes `gh`'s default, which is the
        /// repository's default branch - correct for a link somebody will
        /// keep, wrong for a line in a file you are working on right now.
        branch: Option<String>,
    },
}
