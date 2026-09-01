//! Git domain types.
//!
//! Split along the lines the transport itself is split on.
//!
//! - [`status`] is what git says the tree looks like right now. Phase 1 built
//!   it and every later phase reads it.
//! - [`commit`] is what a caller asks git to do and what git did. Nothing here
//!   is a command; a [`commit::CommitRequest`] is data the transport turns into
//!   argv, and a [`commit::GitCommit`] is the answer.
//! - [`diff`], [`log`] and [`blame`] are what happened: three ways of reading
//!   history, none of which changes anything.
//! - [`branch`] and [`stash`] are the two things that move the working tree
//!   out from under the other panes. They are separate from [`status`] on
//!   purpose: a `GitBranch` is a ref the picker can offer, while
//!   `GitRepository` is the one branch HEAD is actually on, and collapsing
//!   them would leave the header reading a list to find a single fact.
//! - [`remote`] is the only part of this module that leaves the machine. Its
//!   URLs arrive **redacted**, and its `PullRequest` is a request to perform a
//!   pull rather than a GitHub pull request - see the type's own doc.
//! - [`conflict`] is what a merge could not settle. It exists as its own type
//!   rather than as a `GitFileState` because a badge needs one word and a
//!   control needs seven: taking theirs on a both-modified file keeps a file,
//!   and on a deleted-by-them file removes one.
//!
//! Everything git's own formats need decoding into lives here rather than
//! being handed to the UI as raw text. A renderer that parsed a patch, or a
//! gutter that worked out its own line numbers, would be a second
//! implementation of git's format - and with two transports, eventually two
//! disagreeing ones.

mod blame;
mod branch;
mod commit;
mod conflict;
mod diff;
mod log;
mod remote;
mod stash;
mod status;

pub use blame::{GitBlame, GitBlameLine, BLAME_SHA_LENGTH, MAX_BLAME_LINES};
pub use branch::{CreateBranchRequest, GitBranch, MAX_BRANCH_NAME_BYTES};
pub use commit::{CommitRequest, GitCommit, MAX_COMMIT_MESSAGE_BYTES};
pub use conflict::{ConflictResolution, GitConflict, GitConflictKind};
pub use diff::{
    DiffRequest, GitDiff, GitDiffLine, GitDiffLineKind, GitFileDiff, GitHunk, DIFF_CONTEXT_LINES,
    MAX_DIFF_LINES,
};
pub use log::{
    GitChangedFile, GitCommitDetail, GitLog, LogRequest, DEFAULT_LOG_LIMIT, MAX_LOG_LIMIT,
};
pub use remote::{
    GitFetchResult, GitPullOutcome, GitPullResult, GitPushOutcome, GitPushResult, GitRemote,
    PullRequest, PushRequest,
};
pub use stash::{GitStash, StashRequest, MAX_STASH_MESSAGE_BYTES};
pub use status::{GitEntry, GitFileState, GitRepository, GitStatus, MAX_STATUS_ENTRIES};
