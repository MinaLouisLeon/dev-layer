use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How much of a sha the gutter shows.
///
/// Fixed at seven rather than asking git for its adaptive abbreviation:
/// `--porcelain` always reports the full sha, and a gutter whose column width
/// changed with the repository would be worse than one that is always seven.
pub const BLAME_SHA_LENGTH: usize = 7;

/// Lines one blame may carry before it reports itself truncated. Blame is
/// asked for on demand, but a hundred-thousand-line file should still not be
/// able to fill a pane's memory.
pub const MAX_BLAME_LINES: u32 = 50_000;

/// Who last touched one line.
///
/// Already expanded per line - `git blame --porcelain` reports a commit's
/// details only the first time it appears, and re-attaching them is the
/// parser's job so the gutter does no arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitBlameLine {
    /// One-based, matching the editor's own line numbers.
    #[ts(type = "number")]
    pub line: u32,
    pub sha: String,
    pub short_sha: String,
    pub author: String,
    /// Author time, Unix epoch milliseconds.
    #[ts(type = "number")]
    pub timestamp_ms: u64,
    /// The commit's subject, for the gutter's tooltip.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitBlame {
    /// Repository-relative, as git reported it.
    pub relative_path: String,
    /// One entry per line, in file order.
    pub lines: Vec<GitBlameLine>,
    pub truncated: bool,
}
