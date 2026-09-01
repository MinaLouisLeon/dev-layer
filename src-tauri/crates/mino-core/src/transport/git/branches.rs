//! The branch half of the git surface.
//!
//! A supertrait of [`super::GitTransport`] rather than four more methods on
//! it, so each implementation stays a readable file. Callers see one object
//! either way.
//!
//! ## What makes these different from everything before them
//!
//! [`GitBranchTransport::checkout`] is the first call on this interface that
//! **changes files under the other three panes**. The tree, the viewer, the
//! editor's drafts and the search results are all keyed by path, and after a
//! checkout some of those paths hold different bytes and some are not there at
//! all. Nothing here refreshes anything - that is the caller's job, done once
//! from one event - but the contract that makes it possible is here:
//!
//! - **A checkout either happened or it did not.** Git does not switch
//!   halfway, and neither does this. A failure means HEAD is where it was and
//!   the working tree is untouched, which is what lets the caller re-read from
//!   truth rather than guessing at a partial state.
//! - **The failure says which failure it was.** A missing branch, a working
//!   tree that would be overwritten and a name already taken are three
//!   different things the reader can do three different things about. See
//!   [`crate::git::branches::failure`].
//!
//! The one risk this interface cannot cover is an unsaved editor draft: git
//! knows nothing about it, so warning before a checkout that would strand one
//! is the UI's job and is written down in `docs/mino-workbench/git-module.md`.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{CreateBranchRequest, GitBranch};

#[async_trait]
pub trait GitBranchTransport: Send + Sync + 'static {
    /// Every branch the picker can offer - local and remote in one call, each
    /// with its upstream, its ahead/behind counts and its tip commit.
    ///
    /// One call rather than one per branch, and one rather than two: listing
    /// local and remote separately could answer from either side of a fetch
    /// and show a picker that disagrees with itself.
    ///
    /// A repository with no commits answers with an empty list. That is a
    /// state - `git init` and nothing since - and not a failure.
    async fn branches(&self) -> Result<Vec<GitBranch>>;

    /// Switch HEAD, and the working tree with it.
    ///
    /// **The call that moves the ground under the other panes.** Everything
    /// with state keyed by path has to re-read afterwards; see the module doc.
    ///
    /// `name` is a caller value and is checked by [`crate::git::refname`]
    /// before git sees it. A name that is not a branch, and a working tree
    /// that would be overwritten, are both typed errors that leave the
    /// repository exactly as it was.
    async fn checkout(&self, name: &str) -> Result<()>;

    /// Create a branch, and switch to it when `request.checkout` is set.
    ///
    /// Returns the branch it made rather than `()`, so the caller can show
    /// what exists now instead of assuming its own request came true. A
    /// duplicate name is a typed error, not a silent no-op.
    async fn create_branch(&self, request: CreateBranchRequest) -> Result<GitBranch>;

    /// Delete a branch.
    ///
    /// Without `force` this is safe: git refuses a branch whose commits are
    /// nowhere else, and refuses the branch you are on. With `force` it is
    /// **destructive** - commits reachable only from that branch are left to
    /// the reflog - so the UI says what that means before sending it, the way
    /// it does for discard.
    async fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
}
