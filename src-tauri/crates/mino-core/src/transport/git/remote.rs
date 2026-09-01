//! The remote half of the git surface: the only calls in this app that leave
//! the machine.
//!
//! A supertrait of [`super::GitTransport`], like branches and the stash, so
//! callers still see one object.
//!
//! ## The credential position, which is why this phase came last
//!
//! `plan/decisions.md` D3 asked how this app authenticates to a remote. The
//! answer taken is that **it does not**. Git uses its own credential helper,
//! the SSH agent, or the OS keychain, and this process never sees a secret -
//! which is what keeps the standing "no credential, ever" rule true without
//! qualification, and matches the position phase 5 took with `gh`.
//!
//! Two consequences run through every method here:
//!
//! - **Nothing can be asked a question.** Every call runs with
//!   `GIT_TERMINAL_PROMPT=0` and under
//!   [`crate::git::command::REMOTE_TIMEOUT_MS`], so a machine with no helper
//!   configured gets a sentence naming what to set up rather than a pane that
//!   never finishes. A *graphical* helper is still allowed to appear: that is
//!   what delegation is.
//! - **No text from these calls is repeated raw.** A remote URL can carry a
//!   token, and git prints remote URLs unprompted, so every string that
//!   reaches a result or an error goes through [`crate::git::redact`] first.
//!
//! ## What each one can lose
//!
//! Ordered by how much:
//!
//! | Method | Can lose | Guard |
//! | --- | --- | --- |
//! | `remotes` | Nothing. It reads config | – |
//! | `fetch` | Nothing in the working tree. It moves remote-tracking refs | – |
//! | `pull` | Uncommitted work, by merging over it | Refused outright when the tree is dirty |
//! | `push` | Nothing local; with `force`, commits **on the remote** | `--force-with-lease`, and a separate confirmation |

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{
    GitFetchResult, GitPullResult, GitPushResult, GitRemote, PullRequest, PushRequest,
};

#[async_trait]
pub trait GitRemoteTransport: Send + Sync + 'static {
    /// Every configured remote, with its fetch and push URLs.
    ///
    /// The URLs arrive **redacted**. A repository whose `origin` is
    /// `https://user:token@host/o/r` is ordinary, and this interface is not
    /// the place that string becomes visible.
    ///
    /// A repository with no remotes answers with an empty list, which is a
    /// state - a local-only repository - and not a failure.
    async fn remotes(&self) -> Result<Vec<GitRemote>>;

    /// Bring down refs without touching the working tree.
    ///
    /// The safe one, and the one worth reaching for first: it changes nothing
    /// you could lose, and it is what makes the header's ahead/behind counts
    /// true rather than however stale they were.
    ///
    /// `remote` of `None` fetches the branch's configured remote. Prunes
    /// remote-tracking refs for branches deleted upstream - a cache of
    /// somebody else's state, never your work.
    async fn fetch(&self, remote: Option<String>) -> Result<GitFetchResult>;

    /// Bring down refs and merge them into the branch you are on.
    ///
    /// **Refused outright when the working tree is dirty.** A pull over
    /// uncommitted changes can lose them, and the alternative - stashing on
    /// the reader's behalf - would move their work somewhere they did not put
    /// it. The refusal names the two things they can do instead; see
    /// [`crate::git::remote::dirty`].
    ///
    /// The outcome is one of four rather than a boolean, because the reader's
    /// next move differs for each - including
    /// [`crate::types::GitPullOutcome::Conflicted`], which is a **state and
    /// not a failure**: the merge stopped, the files are where it left them,
    /// and [`super::GitConflictTransport`] is how they get settled.
    async fn pull(&self, request: PullRequest) -> Result<GitPullResult>;

    /// Send commits to a remote.
    ///
    /// A rejection is a **typed error** rather than an outcome: nothing was
    /// pushed, and the sentence says so and names the fix. It is never
    /// retried as a force push - see below.
    ///
    /// [`PushRequest::force`] is the one control on this interface that can
    /// destroy work **belonging to somebody else**, on a branch they may have
    /// pulled. Three things hold it:
    ///
    /// - it is `--force-with-lease`, never `--force`, so it refuses unless the
    ///   remote is where this repository last saw it;
    /// - it is a separate, explicit action, and never a fallback offered after
    ///   a normal push was rejected;
    /// - the UI confirms it on its own, naming the remote and the branch.
    async fn push(&self, request: PushRequest) -> Result<GitPushResult>;
}
