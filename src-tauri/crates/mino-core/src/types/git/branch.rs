use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::commit::GitCommit;

/// Ceiling on a branch name. Git's own limit is a path component limit rather
/// than a number, but a name this long is not one anybody typed, and the check
/// is worth having before a value reaches argv.
pub const MAX_BRANCH_NAME_BYTES: usize = 255;

/// One ref the branch picker can offer.
///
/// Local and remote branches are the same shape deliberately: the picker lists
/// both, and a remote one differs only in what checking it out means. Naming
/// `origin/feature` to git detaches HEAD; naming `feature` creates a local
/// branch tracking it, which is what somebody choosing a remote row means. That
/// is a decision about `is_remote`, and modelling the two as separate types
/// would put it in the type system - where the UI does not need it - and out of
/// this flag, where it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitBranch {
    /// The short name: `main`, or `origin/main` for a remote-tracking ref.
    pub name: String,
    /// True for the branch HEAD is on. Exactly one local branch has this,
    /// and none does on a detached HEAD.
    pub is_head: bool,
    pub is_remote: bool,
    /// What this branch tracks, when it tracks anything.
    pub upstream: Option<String>,
    /// Commits this branch has that its upstream does not, and the reverse.
    /// Both zero when there is no upstream, which is not the same as being in
    /// step - `upstream` is what says which.
    #[ts(type = "number")]
    pub ahead: u32,
    #[ts(type = "number")]
    pub behind: u32,
    /// The commit at the tip. `None` only when git could not describe it,
    /// which a freshly created branch on an unborn HEAD can produce.
    pub last_commit: Option<GitCommit>,
}

/// A request to create a branch.
///
/// SECURITY: `name` is a caller value that reaches git. It is checked by
/// [`crate::git::refname`] - a local refusal for anything that could be read
/// as an option, then `git check-ref-format` for git's own rules - before it
/// is placed in argv, and it is never spliced into a command line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct CreateBranchRequest {
    pub name: String,
    /// Where the branch starts. `None` means HEAD, which is what the picker
    /// sends: creating from where you are is the ordinary case.
    pub from: Option<String>,
    /// Switch to it once it exists. The picker's create control sets this;
    /// creating without switching is the deliberate other choice.
    pub checkout: bool,
}

impl CreateBranchRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            from: None,
            checkout: false,
        }
    }

    pub fn from(mut self, from: impl Into<String>) -> Self {
        self.from = Some(from.into());
        self
    }

    pub fn checkout(mut self, checkout: bool) -> Self {
        self.checkout = checkout;
        self
    }
}
