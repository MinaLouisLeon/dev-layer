//! `impl GitTransport for SshTransport`.
//!
//! The same three steps the local transport takes - resolve the root, run one
//! call, hand the output to [`crate::git`] - over the exec channel instead of
//! `tokio::process`. Nothing here parses anything, which is the whole point of
//! the shared module: local and remote read one parser.
//!
//! Remote paths are POSIX regardless of what this client runs on, so the
//! [`PathStyle`] is fixed rather than probed.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle, revision};
use crate::transport::GitTransport;
use crate::types::{
    CommitRequest, DiffRequest, GitBlame, GitCommit, GitCommitDetail, GitDiff, GitLog,
    GitRepository, GitStatus, LogRequest,
};

use super::git_guard::{expect_success, guard_many, guard_optional};
use super::git_history::{self as history, toplevel};
use super::git_run::{run, run_with_input};
use super::SshTransport;

#[async_trait]
impl GitTransport for SshTransport {
    async fn repository(&self) -> Result<Option<GitRepository>> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let Some(repository_root) = toplevel(&connected.handle, &root).await? else {
            return Ok(None);
        };
        let output = run(&connected.handle, &root, &command::branch_argv()).await?;
        let status = git::status_from(&output, repository_root, &root, PathStyle::posix())?;
        Ok(Some(status.repository))
    }

    async fn status(&self) -> Result<GitStatus> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let repository_root = toplevel(&connected.handle, &root)
            .await?
            .ok_or_else(git::not_a_repository)?;
        let output = run(&connected.handle, &root, &command::status_argv()).await?;
        git::status_from(&output, repository_root, &root, PathStyle::posix())
    }

    async fn stage(&self, paths: &[String]) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let guarded = guard_many(&root, paths)?;
        expect_success(
            &connected.handle,
            &root,
            command::stage_argv(&guarded),
            "stage",
        )
        .await
    }

    async fn unstage(&self, paths: &[String]) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let guarded = guard_many(&root, paths)?;
        expect_success(
            &connected.handle,
            &root,
            command::unstage_argv(&guarded),
            "unstage",
        )
        .await
    }

    async fn discard(&self, paths: &[String]) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let guarded = guard_many(&root, paths)?;
        expect_success(
            &connected.handle,
            &root,
            command::discard_argv(&guarded),
            "discard",
        )
        .await
    }

    async fn commit(&self, request: CommitRequest) -> Result<GitCommit> {
        git::commit::validate(&request)?;
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();

        let output = run_with_input(
            &connected.handle,
            &root,
            &command::commit_argv(&request),
            Some(request.trimmed()),
        )
        .await?;
        if !output.succeeded() {
            return Err(git::commit::failure(&output));
        }
        let described = run(&connected.handle, &root, &command::head_commit_argv()).await?;
        git::commit::parse(&described)
    }

    async fn diff(&self, request: DiffRequest) -> Result<GitDiff> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let path = guard_optional(&root, request.path.as_deref())?;
        let request = DiffRequest {
            against: revision::validate_optional(request.against.as_deref())?,
            ..request
        };
        history::diff(&connected.handle, &root, &request, path.as_deref()).await
    }

    async fn log(&self, request: LogRequest) -> Result<GitLog> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let path = guard_optional(&root, request.path.as_deref())?;
        history::log(&connected.handle, &root, &request, path.as_deref()).await
    }

    async fn show(&self, rev: &str) -> Result<GitCommitDetail> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        history::show(&connected.handle, &root, &revision::validate(rev)?).await
    }

    async fn commit_diff(&self, rev: &str, path: Option<&str>) -> Result<GitDiff> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let path = guard_optional(&root, path)?;
        let rev = revision::validate(rev)?;
        history::commit_diff(&connected.handle, &root, &rev, path.as_deref()).await
    }

    async fn blame(&self, path: &str) -> Result<GitBlame> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let guarded = guard_many(&root, std::slice::from_ref(&path.to_string()))?;
        history::blame(&connected.handle, &root, &guarded[0]).await
    }
}
