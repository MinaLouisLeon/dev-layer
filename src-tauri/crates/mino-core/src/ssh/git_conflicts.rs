//! `impl GitConflictTransport for SshTransport`.
//!
//! The same two steps the local implementation takes, over the exec channel,
//! reading the same parser - so a conflict listed here and a conflict listed
//! locally cannot disagree about what kind it is.
//!
//! Resolving is still two commands in sequence, and still in the safe order:
//! the checkout that writes a side, then the add that marks the path settled.
//! A sequence that stops halfway leaves a path git still calls unmerged, which
//! is the honest state and the one the next `conflicts()` reports.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle};
use crate::transport::GitConflictTransport;
use crate::types::{ConflictResolution, GitConflict};

use super::git_guard::{expect_success, guard_many};
use super::git_history::toplevel;
use super::git_run::run_with_input;
use super::SshTransport;

#[async_trait]
impl GitConflictTransport for SshTransport {
    async fn conflicts(&self) -> Result<Vec<GitConflict>> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        // A folder that is not a checkout has no conflicts, which is a true
        // answer rather than an error worth raising.
        let Some(_) = toplevel(&connected.handle, &root).await? else {
            return Ok(Vec::new());
        };
        let output =
            run_with_input(&connected.handle, &root, &command::conflicts_argv(), None).await?;
        Ok(git::conflicts::parse(&output, &root, PathStyle::posix()))
    }

    async fn resolve(&self, path: &str, resolution: ConflictResolution) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let guarded = guard_many(&root, std::slice::from_ref(&path.to_string()))?;
        let target = &guarded[0];

        if let Some(argv) = command::take_side_argv(resolution, target) {
            expect_success(
                &connected.handle,
                &root,
                argv,
                "take one side of a conflict",
            )
            .await?;
        }
        expect_success(
            &connected.handle,
            &root,
            command::mark_resolved_argv(target),
            "mark resolved",
        )
        .await
    }
}
