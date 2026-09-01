//! The argv every git call is made from.
//!
//! SECURITY, and the rule the whole module exists to hold: **nothing here
//! builds a shell string.** Each function returns an array whose elements are
//! passed to the process as separate arguments, so a value in one of them is
//! data and can never become syntax.
//!
//! Phase 1 could say something stronger - that no caller value appeared at all.
//! Phase 2 stages, discards and commits, so two kinds of caller value do
//! arrive, and each has its own answer:
//!
//! | Value | How it stays data |
//! | --- | --- |
//! | Paths | Guarded by [`crate::git::guard`] first, then placed after a `--` separator so a path beginning with a dash is never read as a flag |
//! | The commit message | Never in argv at all. Git reads it from **stdin** |
//! | Revisions | Validated by [`crate::git::revision`], and placed *in front* of the `--` separator - behind it, git would read `main` as a filename |
//!
//! The message rule is not a nicety. Over SSH the argv has to become a command
//! line, and `ssh::command::quote` refuses a value containing a single quote -
//! which would refuse every commit message with an apostrophe in it. Reading
//! it from stdin sidesteps quoting entirely, and is what makes
//! `Fix Bob's bug` a commit message rather than an error.
//!
//! Phase 4 adds two more, in [`branch`] and [`stash`]:
//!
//! | Value | How it stays data |
//! | --- | --- |
//! | Branch names | Checked by [`crate::git::refname`] - a local refusal for anything readable as an option, then `git check-ref-format` for git's own rules - and placed beside a `--` separator so a name and a filename can never be confused |
//! | Stash indices | Not strings at all. A `u32` becomes `stash@{N}` in [`stash::selector`], so no caller text reaches the selector |
//!
//! The one exception is a **stash message**, which has no stdin form and
//! travels in argv. Locally that is safe; over SSH it meets the quoting rule
//! below, which refuses an apostrophe rather than escaping it.
//!
//! Phase 6 adds two more, in [`remote`] and [`conflict`]:
//!
//! | Value | How it stays data |
//! | --- | --- |
//! | A remote name | Checked by [`crate::git::remote::name`], which refuses an empty name, anything readable as an option, and anything the SSH quoting rule would refuse later |
//! | A push's branch | The refname guard again, then placed after `--` beside the remote |
//!
//! And one thing that is not a value at all. The three calls in [`remote`] are
//! the only ones in this crate that leave the machine, so they are the only
//! ones that can be **asked a question** by a server. They run with
//! `GIT_TERMINAL_PROMPT=0` and a long timeout, because under D3 this app has
//! no credential to answer with and a prompt with nowhere to go is a hang.
//!
//! The one caller-influenced value that still reaches the command line is the
//! working directory, and that is quoted (and refused when it cannot be) by
//! `ssh::command::quote`.

mod branch;
mod conflict;
mod history;
mod read;
mod remote;
mod stash;
mod write;

pub use branch::{branches_argv, checkout_argv, create_argv, delete_argv, BRANCH_FORMAT};
pub use conflict::{conflicts_argv, mark_resolved_argv, take_side_argv};
pub use history::{blame_argv, commit_diff_argv, diff_argv, log_argv, show_argv, COMMIT_FORMAT};
pub use read::{
    branch_argv, head_commit_argv, ignored_argv, repository_argv, status_argv, version_argv,
};
pub use remote::{fetch_argv, pull_argv, push_argv, remotes_argv, NO_PROMPT, REMOTE_TIMEOUT_MS};
pub use stash::{
    selector, stash_apply_argv, stash_drop_argv, stash_list_argv, stash_push_argv, STASH_FORMAT,
};
pub use write::{commit_argv, discard_argv, stage_argv, unstage_argv};

pub const GIT_PROGRAM: &str = "git";

/// Wall-clock ceiling for one git call. Long enough for a cold status on a
/// large repository, short enough that a wedged git never becomes a hang.
pub const DEFAULT_TIMEOUT_MS: u64 = 15_000;

/// Options that go *before* the subcommand, so they are git's and not the
/// subcommand's.
///
/// `--no-optional-locks` keeps a status from refreshing the index on disk.
/// Status runs whenever a file is saved or the window regains focus, and a
/// background process taking the index lock is how a workbench ends up
/// fighting a terminal the user is typing `git commit` into.
///
/// It is deliberately *not* applied to the mutating calls in [`write`]: those
/// are supposed to take the lock, because they are changing the index.
pub(super) const GLOBAL: &[&str] = &["--no-optional-locks"];

/// Separates options from paths. Everything after it is a pathspec, so a file
/// genuinely named `-f` is a file and not a flag.
pub(super) const PATH_SEPARATOR: &str = "--";
