//! GitHub domain types.
//!
//! Split the way the surface is: what the probe found, what may be asked, and
//! what came back.
//!
//! - [`probe`] is the one cheap question asked on mount, and its four answers
//!   are four different facts - three of which are quiet absences rather than
//!   failures.
//! - [`query`] and [`response`] are the two halves of the single call every
//!   section makes. The query is an **enum of named subcommands**, never a
//!   string, so no caller can reach a `gh` call this crate has not written
//!   down.
//! - [`run`], [`pull_request`], [`issue`] and [`check`] are the rows.
//! - [`review`] is phase 6's addition, and the one type here with a problem of
//!   its own: a review comment is anchored to a position in a *diff*, not to a
//!   line in a file, so a thread whose diff is no longer current says it is
//!   outdated rather than being pinned somewhere it might not belong.
//!
//! One rule runs through all of them, and it is the reason this module reads
//! the way it does: **everything here that is text came from somebody else.**
//! A pull request title, an issue label, a workflow name, a branch on a fork -
//! all of it is written by whoever opened the thing, which on a public
//! repository is anybody at all. It is carried as text, rendered as text, and
//! never interpolated into a command line or handed to a renderer as markup.
//! That is the same discipline the transport already applies to filenames; the
//! only thing that changes here is how much further away the author is.

mod check;
mod issue;
mod probe;
mod pull_request;
mod query;
mod response;
mod review;
mod run;

pub use check::GitHubCheckState;
pub use issue::{GitHubIssue, GitHubIssueState};
pub use probe::{GitHubAvailability, GitHubProbe, GitHubRepository};
pub use pull_request::{GitHubCreated, GitHubPrState, GitHubPullRequest};
pub use query::{
    GitHubQuery, IssueState, PrState, DEFAULT_GITHUB_LIMIT, MAX_GITHUB_LIMIT, MAX_PR_BODY_BYTES,
    MAX_PR_TITLE_BYTES,
};
pub use response::GitHubResponse;
pub use review::{GitHubReviewComment, GitHubReviewThread};
pub use run::{GitHubJob, GitHubRun};
