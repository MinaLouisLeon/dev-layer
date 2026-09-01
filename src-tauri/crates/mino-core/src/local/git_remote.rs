//! `impl GitRemoteTransport for LocalTransport`.
//!
//! The same three steps every git call here takes - argv, run, interpret -
//! with two things nothing before phase 6 needed.
//!
//! **A different runner.** These calls go through
//! [`super::git_run::run_remote`], which adds
//! [`crate::git::command::NO_PROMPT`] and the longer network timeout. That is
//! not an optimisation: under D3 this app has no credential to answer a prompt
//! with, so a call that could be asked one has to be a call that refuses to be.
//!
//! **A check in front of `pull`.** Every other mutating call in this crate can
//! be sent straight to git, because git's own refusal is the right answer. A
//! pull is different: git will happily merge over a dirty tree in cases where
//! it thinks it can, and the cases where it thinks wrong are the ones that
//! lose work. So the tree is read first and a dirty one is refused here.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle, remote};
use crate::transport::{GitConflictTransport, GitRemoteTransport, GitTransport};
use crate::types::{
    GitFetchResult, GitPullResult, GitPushResult, GitRemote, PullRequest, PushRequest,
};

use super::git_run::{run, run_remote, run_with_input};
use super::LocalTransport;

#[async_trait]
impl GitRemoteTransport for LocalTransport {
    async fn remotes(&self) -> Result<Vec<GitRemote>> {
        let root = self.guard()?.root_display();
        let output = run_with_input(&root, &command::remotes_argv(), None).await?;
        if !output.succeeded() {
            return Err(remote::failure(&output, "remote"));
        }
        Ok(remote::parse(&output))
    }

    async fn fetch(&self, name: Option<String>) -> Result<GitFetchResult> {
        let root = self.guard()?.root_display();
        let named = remote::optional_name(name.as_deref())?;

        let output = run_remote(&root, &command::fetch_argv(named.as_deref())).await?;
        if !output.succeeded() {
            return Err(remote::failure(&output, "fetch"));
        }
        Ok(GitFetchResult {
            remote: named.unwrap_or_else(|| DEFAULT_REMOTE.to_string()),
            summary: remote::said(&output),
        })
    }

    async fn pull(&self, request: PullRequest) -> Result<GitPullResult> {
        let root = self.guard()?.root_display();
        let named = remote::optional_name(request.remote.as_deref())?;
        // Before anything is sent. A pull over uncommitted changes can lose
        // them, and stashing on the reader's behalf would move their work
        // somewhere they did not put it.
        if dirty(self).await? {
            return Err(remote::dirty());
        }

        let request = PullRequest {
            remote: named.clone(),
            ..request
        };
        let output = run_remote(&root, &command::pull_argv(&request)).await?;
        // A pull that hit a conflict exits non-zero and is *not* a failure, so
        // the tree decides before the exit code does.
        let conflicts = self.conflicts().await.unwrap_or_default();
        if !output.succeeded() && conflicts.is_empty() {
            return Err(remote::failure(&output, "pull"));
        }
        Ok(GitPullResult {
            remote: named.unwrap_or_else(|| DEFAULT_REMOTE.to_string()),
            outcome: remote::pull_outcome(&output, &conflicts),
            summary: remote::said(&output),
        })
    }

    async fn push(&self, request: PushRequest) -> Result<GitPushResult> {
        let root = self.guard()?.root_display();
        let named = remote::optional_name(request.remote.as_deref())?;
        let (target, branch) = self.push_target(named, request.branch.as_deref()).await?;

        let output = run_remote(&root, &command::push_argv(&request, &target, &branch)).await?;
        if !output.succeeded() {
            return Err(remote::failure(&output, "push"));
        }
        Ok(GitPushResult {
            remote: target,
            branch,
            outcome: remote::push_outcome(&output),
            summary: remote::said(&output),
            forced: request.force,
        })
    }
}

/// The remote git would have chosen, named so a result can say which one it
/// used. Only reached when the caller did not name one.
const DEFAULT_REMOTE: &str = "origin";

impl LocalTransport {
    /// The remote and branch a push should name, resolved before it runs.
    ///
    /// Resolved rather than assumed, because the confirmation the reader
    /// answered named a branch and the call must push *that* one. A detached
    /// HEAD has no branch to push and is refused here rather than by git,
    /// whose wording for it is about `HEAD` rather than about what to do.
    async fn push_target(
        &self,
        remote: Option<String>,
        branch: Option<&str>,
    ) -> Result<(String, String)> {
        let branch = match branch {
            Some(branch) => git::refname::precheck(branch)?,
            None => self
                .repository()
                .await?
                .and_then(|repository| repository.branch)
                .ok_or_else(remote::detached)?,
        };
        Ok((remote.unwrap_or_else(|| DEFAULT_REMOTE.to_string()), branch))
    }
}

/// Whether the working tree has anything uncommitted.
///
/// Ignored files do not count and neither does a clean side, which is the same
/// rule the header's dirty marker uses - so a pull refused for a dirty tree is
/// refused for a tree the reader can see is dirty.
async fn dirty(transport: &LocalTransport) -> Result<bool> {
    let root = transport.guard()?.root_display();
    let repository_root = super::git_read::toplevel(&root)
        .await?
        .ok_or_else(git::not_a_repository)?;
    let output = run(&root, &command::status_argv()).await?;
    let status = git::status_from(&output, repository_root, &root, PathStyle::local())?;
    Ok(status
        .entries
        .iter()
        .any(|entry| entry.index.is_dirty() || entry.worktree.is_dirty()))
}
