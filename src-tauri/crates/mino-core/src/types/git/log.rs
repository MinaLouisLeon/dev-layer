use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::commit::GitCommit;
use super::status::GitFileState;

/// Commits per page when the caller does not ask for a number, and the ceiling
/// it is clamped to. A history pane shows a screenful and pages; asking for
/// ten thousand commits costs the walk and helps nobody.
pub const DEFAULT_LOG_LIMIT: u32 = 50;
pub const MAX_LOG_LIMIT: u32 = 500;

/// A page of history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct LogRequest {
    /// `None` uses [`DEFAULT_LOG_LIMIT`]; anything above [`MAX_LOG_LIMIT`] is
    /// clamped down to it.
    #[ts(type = "number | null")]
    pub limit: Option<u32>,
    /// Commits to skip. This is the paging control: the second page asks for
    /// the same limit with `skip` set to the first page's length.
    #[ts(type = "number")]
    pub skip: u32,
    /// Only commits touching this path. Guarded like every other path.
    pub path: Option<String>,
}

impl LogRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn skip(mut self, skip: u32) -> Self {
        self.skip = skip;
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// The limit to actually honour, with the default and the ceiling applied.
    /// Mirrors `SearchQuery::effective_limit`.
    pub fn effective_limit(&self) -> u32 {
        self.limit
            .unwrap_or(DEFAULT_LOG_LIMIT)
            .clamp(1, MAX_LOG_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitLog {
    pub commits: Vec<GitCommit>,
    /// True when git had more to give than the limit allowed, so the UI can
    /// offer another page instead of implying this is all the history there is.
    pub truncated: bool,
}

/// One file a commit touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitChangedFile {
    pub relative_path: String,
    /// Where a rename or copy came from. `None` otherwise.
    pub old_path: Option<String>,
    /// What happened to it. The same enum the tree and the panel already use,
    /// so one file state means one thing everywhere.
    pub state: GitFileState,
}

/// One commit, with the files it touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitCommitDetail {
    pub commit: GitCommit,
    pub files: Vec<GitChangedFile>,
}
