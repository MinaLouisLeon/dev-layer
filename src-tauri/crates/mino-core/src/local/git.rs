//! `impl GitTransport for LocalTransport`.
//!
//! Dispatch only, in the same spirit as `transport_impl.rs`: resolve the
//! guard, run one git call, hand the output to [`crate::git`] to interpret.
//! Nothing here parses anything - the helpers are in `git_read.rs` and
//! `git_history.rs`, and the parsers are shared with the SSH transport.
//!
//! Every call runs with the *session* root as its working directory, not the
//! repository root. Git walks up to find the repository itself, and starting
//! anywhere else would mean choosing a directory the path guard has not
//! already approved.
//!
//! Two kinds of caller value are made safe here, once, so no method below can
//! forget: paths through [`guard_paths`], and revisions through
//! [`crate::git::revision`].

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle, revision};
use crate::transport::GitTransport;
use crate::types::{
    CommitRequest, DiffRequest, GitBlame, GitCommit, GitCommitDetail, GitDiff, GitLog,
    GitRepository, GitStatus, LogRequest,
};

use super::git_guard::expect_success;
use super::git_history as history;
use super::git_read::toplevel;
use super::git_run::{run, run_with_input};
use super::LocalTransport;

#[async_trait]
impl GitTransport for LocalTransport {
    async fn repository(&self) -> Result<Option<GitRepository>> {
        let root = self.guard()?.root_display();
        let Some(repository_root) = toplevel(&root).await? else {
            return Ok(None);
        };
        // The branch, head and counts all live in the status headers, so the
        // repository is read from a real status rather than from a `git
        // branch` that could disagree with the one the tree reads. This is the
        // header-only form: nothing here needs the file rows, and the
        // untracked walk is the expensive half.
        let output = run(&root, &command::branch_argv()).await?;
        let status = git::status_from(&output, repository_root, &root, PathStyle::local())?;
        Ok(Some(status.repository))
    }

    async fn status(&self) -> Result<GitStatus> {
        let root = self.guard()?.root_display();
        let repository_root = toplevel(&root).await?.ok_or_else(git::not_a_repository)?;
        let output = run(&root, &command::status_argv()).await?;
        git::status_from(&output, repository_root, &root, PathStyle::local())
    }

    async fn stage(&self, paths: &[String]) -> Result<()> {
        let (root, guarded) = self.guarded(paths)?;
        expect_success(&root, command::stage_argv(&guarded), "stage").await
    }

    async fn unstage(&self, paths: &[String]) -> Result<()> {
        let (root, guarded) = self.guarded(paths)?;
        expect_success(&root, command::unstage_argv(&guarded), "unstage").await
    }

    async fn discard(&self, paths: &[String]) -> Result<()> {
        let (root, guarded) = self.guarded(paths)?;
        expect_success(&root, command::discard_argv(&guarded), "discard").await
    }

    async fn commit(&self, request: CommitRequest) -> Result<GitCommit> {
        git::commit::validate(&request)?;
        let root = self.guard()?.root_display();

        let output = run_with_input(
            &root,
            &command::commit_argv(&request),
            Some(request.trimmed()),
        )
        .await?;
        if !output.succeeded() {
            return Err(git::commit::failure(&output));
        }
        // Asked separately rather than scraped out of `git commit`'s own
        // human-readable output, which changes between versions.
        git::commit::parse(&run(&root, &command::head_commit_argv()).await?)
    }

    async fn diff(&self, request: DiffRequest) -> Result<GitDiff> {
        let (root, path) = self.guarded_one(request.path.as_deref())?;
        let request = DiffRequest {
            against: revision::validate_optional(request.against.as_deref())?,
            ..request
        };
        history::diff(&root, &request, path.as_deref()).await
    }

    async fn log(&self, request: LogRequest) -> Result<GitLog> {
        let (root, path) = self.guarded_one(request.path.as_deref())?;
        history::log(&root, &request, path.as_deref()).await
    }

    async fn show(&self, revision: &str) -> Result<GitCommitDetail> {
        let root = self.guard()?.root_display();
        history::show(&root, &revision::validate(revision)?).await
    }

    async fn commit_diff(&self, revision: &str, path: Option<&str>) -> Result<GitDiff> {
        let (root, path) = self.guarded_one(path)?;
        history::commit_diff(&root, &revision::validate(revision)?, path.as_deref()).await
    }

    async fn blame(&self, path: &str) -> Result<GitBlame> {
        let (root, guarded) = self.guarded(std::slice::from_ref(&path.to_string()))?;
        history::blame(&root, &guarded[0]).await
    }
}
