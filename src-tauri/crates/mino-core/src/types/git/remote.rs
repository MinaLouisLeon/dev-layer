use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One configured remote.
///
/// The URLs are **redacted** before they reach this type - see
/// [`crate::git::redact`]. A remote configured as
/// `https://user:token@github.com/o/r` is a real and common thing to find in
/// somebody's `.git/config`, and this app must not be the reason that string
/// ends up on a screen, in a log, or in a bug report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitRemote {
    pub name: String,
    /// Where a fetch reads from, with any userinfo removed.
    pub fetch_url: String,
    /// Where a push writes to. Usually the same; git allows them to differ.
    pub push_url: String,
}

/// A request to **perform a pull**.
///
/// Not a GitHub pull request. The name follows this crate's `…Request`
/// convention for "the arguments to a call" - as [`super::CommitRequest`] and
/// [`super::StashRequest`] do - and the GitHub kind is
/// [`crate::types::GitHubPullRequest`], which is a different thing entirely.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct PullRequest {
    /// `None` lets git use the branch's configured remote, which is what the
    /// terminal would do. Naming one is for the case where there are several.
    pub remote: Option<String>,
    /// Rebase rather than merge. Off by default: a merge is what `git pull`
    /// does unless configured otherwise, and quietly rewriting local commits
    /// is not something a button should do without being asked.
    pub rebase: bool,
}

/// What a pull actually did.
///
/// Four outcomes rather than a boolean, because the reader's next move differs
/// for each of them - and because the plan's rule for this call is that a pull
/// which cannot fast-forward **reports the state rather than guessing at a
/// merge**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitPullOutcome {
    /// Nothing to bring down. The commonest answer, and worth saying plainly
    /// rather than leaving the reader to compare two lists.
    AlreadyUpToDate,
    /// The branch moved forward with no merge commit.
    FastForwarded,
    /// Histories had diverged and git merged them.
    Merged,
    /// The same, by replaying local commits on top.
    Rebased,
    /// **The working tree now has conflict markers in it.** A state, not a
    /// failure: the files are exactly where the merge left them, and resolving
    /// them is what [`super::conflict::ConflictResolution`] is for.
    Conflicted,
}

/// A request to push.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct PushRequest {
    pub remote: Option<String>,
    /// `None` pushes the branch that is checked out, which is what the
    /// confirmation names.
    pub branch: Option<String>,
    /// **Destructive.** A force push can drop commits that exist nowhere
    /// else, on a branch other people may have pulled.
    ///
    /// It is never a fallback for a rejected push: a rejection is reported as
    /// a rejection, and this is a separate action the reader chooses and
    /// confirms on its own. When it is set, git is given
    /// `--force-with-lease` rather than `--force` - see
    /// [`crate::git::command::push_argv`].
    pub force: bool,
    /// Set the branch's upstream while pushing, for a branch that has none.
    pub set_upstream: bool,
}

/// What a push did.
///
/// A **rejection is not here.** It is a typed error, because it means nothing
/// was pushed and the reader has something to do about it - see
/// [`crate::git::remote::failure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitPushOutcome {
    Pushed,
    AlreadyUpToDate,
}

/// What a fetch found.
///
/// Deliberately thin. The thing worth knowing after a fetch is how far ahead
/// or behind the branch now is, and that lives in
/// [`super::GitRepository`] - which every caller re-reads anyway, because a
/// fetch changes what the header should say. Inventing a second source for it
/// here would be two things to keep in agreement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitFetchResult {
    pub remote: String,
    /// Git's own summary, **redacted**, when it had one worth showing.
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitPullResult {
    pub remote: String,
    pub outcome: GitPullOutcome,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitPushResult {
    pub remote: String,
    /// The branch that was pushed, named so the confirmation and the
    /// confirmation's answer agree.
    pub branch: String,
    pub outcome: GitPushOutcome,
    pub summary: Option<String>,
    /// True when this was a force push. Carried so the UI can say what it did
    /// rather than what it was asked to do.
    pub forced: bool,
}
