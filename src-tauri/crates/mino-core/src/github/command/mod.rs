//! The argv every `gh` call is made from.
//!
//! SECURITY, and the rule the whole module exists to hold: **nothing here
//! builds a shell string, and nothing here takes a subcommand from a caller.**
//! Each function returns an array whose elements are passed to the process as
//! separate arguments, and which function runs at all is decided by matching
//! on [`crate::types::GitHubQuery`] - an enum this crate defines. A caller
//! chooses a variant; it cannot spell a `gh` call.
//!
//! The caller values that do travel, and how each one stays data:
//!
//! | Value | How it stays data |
//! | --- | --- |
//! | A branch name | Checked by [`crate::git::refname`] - the same guard branch checkout uses - then passed to `--branch` as its own argv element |
//! | A limit, a number, a run id | Not text at all. A `u32`/`u64` is formatted here, so nothing a caller typed reaches argv |
//! | A list filter | An enum. [`state_word`] and [`issue_state_word`] are the only places those words are written |
//! | A file path | Guarded by [`crate::github::browse`] against the connected root first, then placed after a `--` separator so a name beginning with a dash is never read as a flag |
//! | A pull request title and base | Argv elements, behind explicit `--title` and `--base` flags |
//! | A pull request **body** | Never in argv at all. `gh` reads it from **stdin** via `--body-file -` |
//!
//! The body rule is the same one a commit message follows in
//! [`crate::git::command`], and for the same reason: over SSH the argv has to
//! become a command line, and `ssh::command::quote` refuses a value containing
//! a single quote. A description with an apostrophe in it must be a
//! description, not an error. A *title* still travels in argv and so still
//! meets that quoting rule on a remote target - the documented limit, exactly
//! as it is for a stash message.

mod list;
mod probe;
mod review;
mod write;

pub use list::{issues_argv, pull_request_argv, pull_requests_argv, run_jobs_argv, runs_argv};
pub use probe::{auth_status_argv, repo_view_argv};
pub use review::{reply_argv, reply_body, review_comments_argv, thread_argv};
pub use write::{browse_argv, create_pr_argv};

use crate::types::{IssueState, PrState, MAX_GITHUB_LIMIT};

/// `--state`'s word for a pull request filter. The only place these are
/// written, so a filter cannot become free text on the way to argv.
pub(super) fn state_word(state: PrState) -> &'static str {
    match state {
        PrState::Open => "open",
        PrState::Closed => "closed",
        PrState::Merged => "merged",
        PrState::All => "all",
    }
}

/// The same for an issue filter, which has no merged state to ask about.
pub(super) fn issue_state_word(state: IssueState) -> &'static str {
    match state {
        IssueState::Open => "open",
        IssueState::Closed => "closed",
        IssueState::All => "all",
    }
}

/// A caller's limit, brought inside the ceiling.
///
/// Clamped rather than refused: a section asking for more rows than the rate
/// limit deserves is a bug worth capping, not a reason to show the reader an
/// error about a number they never typed. Zero becomes one, because `gh`
/// reads `--limit 0` as an error.
pub(super) fn limit(requested: u32) -> String {
    requested.clamp(1, MAX_GITHUB_LIMIT).to_string()
}

pub(crate) fn owned<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    args.iter().map(|arg| arg.as_ref().to_string()).collect()
}
