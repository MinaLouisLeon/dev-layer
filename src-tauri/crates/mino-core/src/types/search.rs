use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::entry::DirEntry;

/// Result ceiling when the caller does not name one, and the ceiling it is
/// clamped to. A search pane shows a screenful; asking for ten thousand rows
/// costs the walk time and helps nobody.
pub const DEFAULT_SEARCH_LIMIT: u32 = 200;
pub const MAX_SEARCH_LIMIT: u32 = 500;

/// How many entries a single walk may visit before it gives up and reports
/// itself truncated. Reached only in very large trees; the skip list keeps
/// ordinary repositories far below it.
pub const MAX_SCANNED_ENTRIES: u32 = 40_000;

/// Wall-clock ceiling for one walk. A search that cannot finish in this long
/// returns what it found rather than holding the pane.
pub const SEARCH_TIMEOUT_MS: u64 = 5_000;

/// Directory names never descended into.
///
/// These are build outputs and dependency caches: entries a person searching
/// their own source tree does not mean, and the difference between a search
/// that answers instantly and one that walks a hundred thousand files. A
/// directory named here is skipped whole - its children are never visited.
pub const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".venv",
    "__pycache__",
];

/// A recursive filename search over the connected root.
///
/// `query` is matched as a fuzzy subsequence, the way VS Code's quick-open
/// works: `ftp` finds `FileTreePane.tsx`. Matching happens in Rust so every
/// transport ranks identically - see [`crate::search::fuzzy`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct SearchQuery {
    pub query: String,
    /// `None` uses [`DEFAULT_SEARCH_LIMIT`]; anything above
    /// [`MAX_SEARCH_LIMIT`] is clamped down to it.
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
    /// Whether hidden entries (dot-files, and hidden-attributed files on
    /// Windows) may match. The tree dims them rather than hiding them, and
    /// search follows the same instinct: they match by default.
    pub include_hidden: bool,
    /// Whether directories may match, or only files.
    pub include_directories: bool,
}

impl SearchQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: None,
            include_hidden: true,
            include_directories: true,
        }
    }

    /// The limit to actually honour, with the default and the ceiling applied.
    pub fn effective_limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT) as usize
    }
}

/// One ranked match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct SearchHit {
    pub entry: DirEntry,
    /// `entry.path` with the connected root removed, always with `/`
    /// separators. This is what was matched and what the UI renders.
    pub relative_path: String,
    /// Higher is better. Comparable only within one result set.
    #[ts(type = "number")]
    pub score: i32,
    /// Character indices into `relative_path` that the query matched, in
    /// ascending order. The UI highlights exactly these.
    #[ts(type = "number[]")]
    pub match_indices: Vec<u32>,
}

/// The answer to one [`SearchQuery`], already ranked and cut to the limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct SearchHits {
    pub hits: Vec<SearchHit>,
    /// True when the walk stopped early - at the entry cap, the time ceiling
    /// or the result limit - so the UI can say the list is partial rather
    /// than implying it is complete.
    pub truncated: bool,
    /// Entries visited. Shown as "searched N files" and useful when a result
    /// set looks emptier than expected.
    #[ts(type = "number")]
    pub scanned: u32,
}
