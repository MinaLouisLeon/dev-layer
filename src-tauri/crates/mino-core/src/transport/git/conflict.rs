//! The conflict half of the git surface.
//!
//! A supertrait of [`super::GitTransport`], like the other three.
//!
//! ## Two methods, and deliberately no third
//!
//! There is no three-way merge editor here and none planned. That is a large
//! piece of UI on its own and would be its own phase; what this offers instead
//! is the three things that settle most conflicts without one - take mine,
//! take theirs, or **edit the file and say you are done** - which is exactly
//! what somebody does in a terminal with `git checkout --ours` and `git add`.
//!
//! `Manual` is the one that makes the other two optional. The conflicted file
//! is already open-able in the viewer, markers and all, and the editor already
//! saves through the transport. So the flow is: open it, fix it, mark it
//! resolved. Nothing about that needs a bespoke merge pane.
//!
//! ## The rule a commit depends on
//!
//! Git refuses to commit while any path is unmerged, and this app refuses
//! earlier and more clearly: [`GitConflictTransport::conflicts`] is what the
//! source control panel reads to disable its commit button and say why. The
//! two are not redundant - the panel's check is the one the reader sees, and
//! git's is the one that is definitely right - but the panel's is the one that
//! stops somebody typing a commit message they cannot use.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{ConflictResolution, GitConflict};

#[async_trait]
pub trait GitConflictTransport: Send + Sync + 'static {
    /// Every path a merge could not settle, with **which kind** of conflict
    /// each one is.
    ///
    /// The kind matters because the controls differ by it: taking theirs on a
    /// both-modified file keeps a file, and on a deleted-by-them file removes
    /// one. [`crate::types::GitFileState::Conflicted`] is enough for a badge
    /// and is not enough for a button.
    ///
    /// An empty list is the ordinary answer and not a failure - most
    /// repositories are not mid-merge. Paths outside the connected root are
    /// dropped, exactly as status rows are.
    async fn conflicts(&self) -> Result<Vec<GitConflict>>;

    /// Settle one path.
    ///
    /// **Two of the three throw something away.** `Ours` discards the incoming
    /// side and `Theirs` discards yours, and neither is recoverable from the
    /// working tree afterwards - the other version is still in the merge's
    /// object store, but nothing in this app will show it to you. So the UI
    /// names which side each button keeps rather than labelling them "ours"
    /// and "theirs", which every reader has to translate at least once.
    ///
    /// `Manual` throws nothing away: it takes the file exactly as it is on
    /// disk and marks it resolved. It is the only one that is safe to press
    /// without reading first, and it is the one that does nothing at all if
    /// the conflict markers are still in the file - git will stage them, and
    /// `<<<<<<<` will be in the commit. The UI says so.
    ///
    /// `path` is guarded against the connected root before git sees it, like
    /// every other path-taking call on this interface.
    async fn resolve(&self, path: &str, resolution: ConflictResolution) -> Result<()>;
}
