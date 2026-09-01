//! The read-only history calls, locally.
//!
//! Split from `git.rs` so that file stays a list of trait methods. Each of
//! these is the same three steps every git call in this crate takes: argv from
//! [`crate::git::command`], run it, hand the output to a shared parser. None
//! of them reads git's output itself.
//!
//! Paths arrive already guarded and revisions already validated; that happens
//! in `git.rs`, once, so no call here can skip it.

use crate::error::Result;
use crate::git::{self, command};
use crate::types::{DiffRequest, GitBlame, GitCommitDetail, GitDiff, GitLog, LogRequest};

use super::git_run::run_with_input;

pub async fn diff(root: &str, request: &DiffRequest, path: Option<&str>) -> Result<GitDiff> {
    let output = run_with_input(root, &command::diff_argv(request, path), None).await?;
    Ok(git::diff::parse(&output.stdout))
}

/// The diff one commit introduced.
pub async fn commit_diff(root: &str, revision: &str, path: Option<&str>) -> Result<GitDiff> {
    let output = run_with_input(root, &command::commit_diff_argv(revision, path), None).await?;
    Ok(git::diff::parse(&output.stdout))
}

pub async fn log(root: &str, request: &LogRequest, path: Option<&str>) -> Result<GitLog> {
    let output = run_with_input(root, &command::log_argv(request, path), None).await?;
    git::history::log_from(&output, request)
}

pub async fn show(root: &str, revision: &str) -> Result<GitCommitDetail> {
    let output = run_with_input(root, &command::show_argv(revision), None).await?;
    git::history::detail_from(&output)
}

pub async fn blame(root: &str, path: &str) -> Result<GitBlame> {
    let output = run_with_input(root, &command::blame_argv(path), None).await?;
    git::blame::parse(&output, path)
}
