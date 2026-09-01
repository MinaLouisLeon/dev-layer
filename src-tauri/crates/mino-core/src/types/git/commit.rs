use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ceiling on a commit message. Generous - a long body is normal - but not
/// unbounded, because the message crosses a process boundary and "how much
/// text may this carry" should have an answer rather than being whatever the
/// OS happens to allow.
pub const MAX_COMMIT_MESSAGE_BYTES: usize = 64 * 1024;

/// A request to commit what is staged.
///
/// SECURITY: `message` is a caller value and it never reaches a command line.
/// Git reads it from **stdin** (`git commit --file -`), which is not a
/// nicety - it is what lets a message contain an apostrophe, a newline or a
/// backtick on a target reached over SSH, where `ssh::command::quote` would
/// otherwise refuse it. See `crates/mino-core/src/git/command/write.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct CommitRequest {
    pub message: String,
    /// Stage every tracked modification first - `git commit --all`. Untracked
    /// files are still never included, which is git's own rule and not one
    /// this app adds.
    pub all: bool,
    /// Replace the previous commit instead of adding one. Only ever the last
    /// commit; rewriting further back is not on this interface.
    pub amend: bool,
}

impl CommitRequest {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            all: false,
            amend: false,
        }
    }

    pub fn all(mut self, all: bool) -> Self {
        self.all = all;
        self
    }

    pub fn amend(mut self, amend: bool) -> Self {
        self.amend = amend;
        self
    }

    /// The message with surrounding blank space removed, which is what git
    /// would store anyway under `--cleanup=strip`.
    pub fn trimmed(&self) -> &str {
        self.message.trim()
    }
}

/// A commit that exists. Returned by `commit` so the UI can say it landed and
/// name it, rather than reporting success and hoping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitCommit {
    pub sha: String,
    /// The abbreviated form git itself chose, not a slice of `sha` - the
    /// length git needs for uniqueness grows with the repository.
    pub short_sha: String,
    /// The first line of the message.
    pub summary: String,
    pub author: String,
    /// Author time, Unix epoch milliseconds.
    #[ts(type = "number")]
    pub timestamp_ms: u64,
}
