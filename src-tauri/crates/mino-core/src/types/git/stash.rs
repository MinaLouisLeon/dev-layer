use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ceiling on a stash message. Far smaller than the commit message ceiling
/// because this is a *reflog subject*: git keeps one line of it, and anything
/// past that is text nobody will ever read back.
pub const MAX_STASH_MESSAGE_BYTES: usize = 1024;

/// A request to set work aside.
///
/// SECURITY, and the difference from [`super::commit::CommitRequest`]: a stash
/// message has no stdin form. `git stash push -m` takes it in argv, which is
/// safe locally and, over SSH, goes through the quoting rule in
/// `ssh::command::quote` - which refuses a value containing a single quote
/// rather than escaping it. A stash message with an apostrophe in it is
/// therefore a typed error on a remote target and works locally. That is the
/// documented limit of the SSH transport, not a silent difference; see
/// `docs/mino-workbench/git-module.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct StashRequest {
    /// `None` lets git write its own `WIP on <branch>` subject.
    pub message: Option<String>,
    /// Stash untracked files too.
    ///
    /// Off by default, and that default matters: with it off, a stash leaves
    /// untracked files exactly where they are, so nothing git has never seen
    /// can go missing because of a control the user reached for to *keep*
    /// their work.
    pub include_untracked: bool,
}

impl StashRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn include_untracked(mut self, include: bool) -> Self {
        self.include_untracked = include;
        self
    }

    /// The message with surrounding blank space removed, or `None` when there
    /// is nothing left after trimming. A message of spaces is not a message,
    /// and passing one would give git an empty `-m`.
    pub fn trimmed(&self) -> Option<&str> {
        self.message
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    }
}

/// One entry on the stash stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitStash {
    /// Position on the stack, and the only thing that names it to git:
    /// `stash@{0}` is the most recent. **It is not an identity.** Dropping an
    /// entry renumbers every entry below it, which is why every call that
    /// takes an index is followed by a re-read rather than a local edit of the
    /// list.
    #[ts(type = "number")]
    pub index: u32,
    /// The part of git's reflog subject that is the message: what the user
    /// wrote, or the commit summary git chose for them.
    pub message: String,
    /// The branch the stash was made on, when git's subject named one.
    pub branch: Option<String>,
    /// When it was made, Unix epoch milliseconds - the same unit every other
    /// time on this interface uses.
    #[ts(type = "number")]
    pub timestamp_ms: u64,
}
