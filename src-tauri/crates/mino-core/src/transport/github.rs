//! The GitHub half of a session.
//!
//! A third trait, reached through [`super::Transport::github`], for exactly
//! the reason git is a second one: "is there a GitHub repository here?" is
//! better asked once, at the type level, than answered by every method
//! separately. See `plan/decisions.md` D2 for the argument, which this trait
//! inherits rather than re-makes.
//!
//! What is different here is the **size**. Git needs twenty-five methods and
//! got them. GitHub needs two, because five features share one enumerated
//! query rather than each bringing a method, a Tauri command and a client
//! method of its own. That is a deliberate trade and it costs something: a
//! caller matches on a response variant instead of being handed the type it
//! asked for. It buys a surface that does not grow by three files every time
//! somebody wants another list.
//!
//! ## No credential, ever
//!
//! Every call under this trait shells out to the `gh` CLI, which owns its own
//! authentication and stores it in the operating system keychain under its own
//! name. This application never holds a GitHub token - not on disk, not in a
//! log, not in memory for the length of one call. The standing rule is
//! honoured by there being nothing to keep. See [`crate::github`].
//!
//! ## No timer
//!
//! Nothing on this trait polls. A query is made on mount, on a branch change,
//! and when a reader asks for one - never on a schedule. Two reasons, and both
//! are real: the rate limit is somebody's account budget, and a workbench that
//! quietly makes network calls forever is a surprise nobody consented to.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{GitHubProbe, GitHubQuery, GitHubResponse};

#[async_trait]
pub trait GitHubTransport: Send + Sync + 'static {
    /// Whether `gh` is present and authenticated, and what repository the
    /// remote points at.
    ///
    /// Cheap enough to call on mount, and the only call any GitHub surface may
    /// make before it has an answer. Its four states are four different facts
    /// - see [`crate::types::GitHubAvailability`] - and three of them are
    /// quiet absences rather than errors: no `gh`, no login, no GitHub remote.
    ///
    /// An `Err` here means something else went wrong entirely. It is rendered
    /// as a sentence like any other failure.
    async fn probe(&self) -> Result<GitHubProbe>;

    /// One `gh` subcommand with `--json`, parsed.
    ///
    /// The program text lives in [`crate::github::command`] and the caller
    /// picks a variant; there is no shape of [`GitHubQuery`] that names a
    /// subcommand or adds a flag. Caller values travel as argv, except a pull
    /// request body, which travels on stdin.
    ///
    /// **The probe is not re-asked here.** Callers ask [`Self::probe`] once
    /// and act on the answer; putting a probe in front of every query would
    /// mean two extra `gh` processes per call, on a surface whose whole
    /// polling policy is about not spending somebody's rate limit. A query
    /// sent to a session that is not ready gets `gh`'s own refusal, as a
    /// sentence.
    ///
    /// Every call is bounded by [`crate::github::DEFAULT_TIMEOUT_MS`]: a
    /// stalled request becomes a sentence in one section rather than a pane
    /// that never finishes loading.
    ///
    /// **One of the seven variants writes.**
    /// [`GitHubQuery::CreatePullRequest`] creates something public and
    /// visible to everybody watching the repository. It is confirmed in the
    /// UI first, showing exactly what will be made, and it answers with the
    /// URL it created rather than leaving the author to go and look.
    async fn query(&self, request: GitHubQuery) -> Result<GitHubResponse>;
}
