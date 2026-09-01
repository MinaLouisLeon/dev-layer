//! The read-only history calls, on the remote host.
//!
//! The mirror of `local/git_history.rs`, and deliberately the same three
//! steps: argv from [`crate::git::command`], run it over the exec channel,
//! hand the output to the *same* shared parser. Neither transport reads git's
//! output itself, which is what stops a diff rendering differently depending
//! on where the repository is.

use russh::client::Handle;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle};
use crate::types::{DiffRequest, GitBlame, GitCommitDetail, GitDiff, GitLog, LogRequest};

use super::git_run::{run, run_with_input};
use super::handler::ClientHandler;

type Channel<'a> = &'a Handle<ClientHandler>;

pub async fn diff(
    handle: Channel<'_>,
    root: &str,
    request: &DiffRequest,
    path: Option<&str>,
) -> Result<GitDiff> {
    let output = run_with_input(handle, root, &command::diff_argv(request, path), None).await?;
    Ok(git::diff::parse(&output.stdout))
}

pub async fn commit_diff(
    handle: Channel<'_>,
    root: &str,
    revision: &str,
    path: Option<&str>,
) -> Result<GitDiff> {
    let argv = command::commit_diff_argv(revision, path);
    let output = run_with_input(handle, root, &argv, None).await?;
    Ok(git::diff::parse(&output.stdout))
}

pub async fn log(
    handle: Channel<'_>,
    root: &str,
    request: &LogRequest,
    path: Option<&str>,
) -> Result<GitLog> {
    let output = run_with_input(handle, root, &command::log_argv(request, path), None).await?;
    git::history::log_from(&output, request)
}

pub async fn show(handle: Channel<'_>, root: &str, revision: &str) -> Result<GitCommitDetail> {
    let output = run_with_input(handle, root, &command::show_argv(revision), None).await?;
    git::history::detail_from(&output)
}

pub async fn blame(handle: Channel<'_>, root: &str, path: &str) -> Result<GitBlame> {
    let output = run_with_input(handle, root, &command::blame_argv(path), None).await?;
    git::blame::parse(&output, path)
}

/// The work tree root containing `root`, or `None` when there is not one.
pub async fn toplevel(handle: Channel<'_>, root: &str) -> Result<Option<String>> {
    let output = run(handle, root, &command::repository_argv()).await?;
    Ok(git::repository_root(&output)?.map(|found| PathStyle::posix().normalise(&found)))
}
