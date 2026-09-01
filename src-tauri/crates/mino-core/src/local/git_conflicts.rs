//! `impl GitConflictTransport for LocalTransport`.
//!
//! Two methods, and the second is the only place in this crate where one trait
//! call runs two mutating git commands in sequence. That is worth being
//! explicit about, because a sequence can stop halfway:
//!
//! - `checkout --ours|--theirs` writes one side over the file on disk;
//! - `add` is what tells git the path is settled.
//!
//! If the checkout fails, the add never runs and the path stays conflicted -
//! which is the safe way round. If the checkout succeeds and the add fails,
//! the file on disk is one side and git still calls it unmerged, which the
//! next `conflicts()` reports honestly. Neither leaves a path that looks
//! settled and is not.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle};
use crate::transport::GitConflictTransport;
use crate::types::{ConflictResolution, GitConflict};

use super::git_guard::expect_success;
use super::git_run::run_with_input;
use super::LocalTransport;

#[async_trait]
impl GitConflictTransport for LocalTransport {
    async fn conflicts(&self) -> Result<Vec<GitConflict>> {
        let root = self.guard()?.root_display();
        // Not `not_a_repository`: a folder that is not a checkout has no
        // conflicts, which is a true answer and the one the panel wants. Only
        // git being unusable is worth an error here.
        let Some(_) = super::git_read::toplevel(&root).await? else {
            return Ok(Vec::new());
        };
        let output = run_with_input(&root, &command::conflicts_argv(), None).await?;
        Ok(git::conflicts::parse(&output, &root, PathStyle::local()))
    }

    async fn resolve(&self, path: &str, resolution: ConflictResolution) -> Result<()> {
        // The same guard `stage` and `discard` use. A path the session does
        // not own must not be reachable by a call that overwrites a file.
        let (root, guarded) = self.guarded(std::slice::from_ref(&path.to_string()))?;
        let target = &guarded[0];

        if let Some(argv) = command::take_side_argv(resolution, target) {
            expect_success(&root, argv, "take one side of a conflict").await?;
        }
        // Always, whichever resolution it was. This is the call that marks the
        // path settled; without it a checked-out side is a file git still
        // reports as unmerged and a commit still refuses.
        expect_success(&root, command::mark_resolved_argv(target), "mark resolved").await
    }
}
