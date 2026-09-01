//! The stash half of the git surface.
//!
//! A supertrait of [`super::GitTransport`] for the same reason
//! [`super::GitBranchTransport`] is: one object for callers, one readable file
//! per implementation.
//!
//! ## Indices are positions, not identities
//!
//! Git names a stash entry `stash@{0}`, and that name means "the top of the
//! stack" rather than "this entry". Dropping one renumbers every entry below
//! it. So the rule every caller here follows is: **act, then re-read.** A list
//! edited locally after a drop would be a list whose numbers no longer point
//! at the entries it is showing, and the next click would act on the wrong
//! one.
//!
//! That is why [`GitStashTransport::stash_apply`] and
//! [`GitStashTransport::stash_drop`] take a `u32` and return `()`: there is
//! nothing useful to hand back that is not stale the moment it is made.
//!
//! ## Conflicts are reported, not resolved
//!
//! A pop that conflicts leaves the entry on the stack and the conflict markers
//! in the files. This interface says so in a sentence; resolving it is phase 6.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{GitStash, StashRequest};

#[async_trait]
pub trait GitStashTransport: Send + Sync + 'static {
    /// The stack, most recent first. An empty stack is an empty list, not an
    /// error - most repositories have nothing stashed.
    async fn stash_list(&self) -> Result<Vec<GitStash>>;

    /// Set the working tree aside and return it to the last commit.
    ///
    /// **This changes files under the other panes**, exactly as a checkout
    /// does, and the same refresh follows it.
    ///
    /// Untracked files are left alone unless `request.include_untracked` says
    /// otherwise, and that default is deliberate: nothing git has never seen
    /// should move because of a control someone reached for to *keep* their
    /// work. Stashing a clean tree is a typed error rather than a silent
    /// success.
    async fn stash_push(&self, request: StashRequest) -> Result<()>;

    /// Put an entry back. `pop` drops it afterwards; `apply` leaves it.
    ///
    /// A conflict is reported as one and leaves the entry on the stack either
    /// way, so nothing is lost by a pop that could not finish.
    async fn stash_apply(&self, index: u32, pop: bool) -> Result<()>;

    /// Remove an entry without applying it.
    ///
    /// **Destructive.** What it removes is reachable only through the reflog
    /// afterwards, which is not something this app offers, so the UI confirms
    /// first and names the entry - the discard rule, applied to the stack.
    async fn stash_drop(&self, index: u32) -> Result<()>;
}
