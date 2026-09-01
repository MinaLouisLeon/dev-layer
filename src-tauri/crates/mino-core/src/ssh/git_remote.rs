//! `impl GitRemoteTransport for SshTransport`.
//!
//! The same shape as the local implementation, over the exec channel, sharing
//! every decision in [`crate::git::remote`] - so a rejected push produces the
//! same sentence whichever end ran it.
//!
//! **Where the credential lives is the thing worth knowing.** These calls run
//! on the *remote host*, so the credential helper, the SSH agent and the
//! keychain that answer for them are that machine's. Nothing about this
//! machine's git configuration is involved, and no secret crosses the
//! connection - there is none at this end to cross it. That is the same
//! property the GitHub surface has over SSH, and it falls out of D3 rather
//! than being arranged separately.
//!
//! One consequence is worth stating plainly: a push from a remote session can
//! fail for a reason that is invisible from here - the remote account has no
//! key for the upstream - and the sentence it produces names the fix on the
//! *remote* machine.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle, remote};
use crate::transport::{GitConflictTransport, GitRemoteTransport, GitTransport};
use crate::types::{
    GitFetchResult, GitPullResult, GitPushResult, GitRemote, PullRequest, PushRequest,
};

use super::git_history::toplevel;
use super::git_run::{run, run_remote, run_with_input};
use super::SshTransport;

/// The remote git would have chosen, named so a result can say which it used.
const DEFAULT_REMOTE: &str = "origin";

#[async_trait]
impl GitRemoteTransport for SshTransport {
    async fn remotes(&self) -> Result<Vec<GitRemote>> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let output =
            run_with_input(&connected.handle, &root, &command::remotes_argv(), None).await?;
        if !output.succeeded() {
            return Err(remote::failure(&output, "remote"));
        }
        Ok(remote::parse(&output))
    }

    async fn fetch(&self, name: Option<String>) -> Result<GitFetchResult> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let named = remote::optional_name(name.as_deref())?;

        let argv = command::fetch_argv(named.as_deref());
        let output = run_remote(&connected.handle, &root, &argv).await?;
        if !output.succeeded() {
            return Err(remote::failure(&output, "fetch"));
        }
        Ok(GitFetchResult {
            remote: named.unwrap_or_else(|| DEFAULT_REMOTE.to_string()),
            summary: remote::said(&output),
        })
    }

    async fn pull(&self, request: PullRequest) -> Result<GitPullResult> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let named = remote::optional_name(request.remote.as_deref())?;
        if dirty(self).await? {
            return Err(remote::dirty());
        }

        let request = PullRequest {
            remote: named.clone(),
            ..request
        };
        let output = run_remote(&connected.handle, &root, &command::pull_argv(&request)).await?;
        // The tree decides before the exit code does: a pull that hit a
        // conflict exits non-zero and is a state rather than a failure.
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
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let named = remote::optional_name(request.remote.as_deref())?;

        let branch = match request.branch.as_deref() {
            Some(branch) => git::refname::precheck(branch)?,
            None => self
                .repository()
                .await?
                .and_then(|repository| repository.branch)
                .ok_or_else(remote::detached)?,
        };
        let target = named.unwrap_or_else(|| DEFAULT_REMOTE.to_string());

        let argv = command::push_argv(&request, &target, &branch);
        let output = run_remote(&connected.handle, &root, &argv).await?;
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

/// Whether the remote working tree has anything uncommitted.
async fn dirty(transport: &SshTransport) -> Result<bool> {
    let connected = transport.connected().await?;
    let root = connected.root.root().to_string();
    let repository_root = toplevel(&connected.handle, &root)
        .await?
        .ok_or_else(git::not_a_repository)?;
    let output = run(&connected.handle, &root, &command::status_argv()).await?;
    let status = git::status_from(&output, repository_root, &root, PathStyle::posix())?;
    Ok(status
        .entries
        .iter()
        .any(|entry| entry.index.is_dirty() || entry.worktree.is_dirty()))
}
