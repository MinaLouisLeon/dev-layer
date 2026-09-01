use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many diff lines one call may carry before it reports itself truncated.
///
/// A generated file, a lockfile or a vendored bundle can produce a diff nobody
/// wants and no pane can render. The same bargain [`crate::types::SearchHits`]
/// makes: a partial answer that says it is partial beats a pane that stops
/// responding.
pub const MAX_DIFF_LINES: u32 = 20_000;

/// How much unchanged context sits around each change. Git's own default, said
/// out loud so the two transports cannot disagree about it.
pub const DIFF_CONTEXT_LINES: u32 = 3;

/// What to compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct DiffRequest {
    /// One file, or the whole tree when `None`. Guarded like every other path.
    pub path: Option<String>,
    /// The staged side (`--cached`) rather than the working tree.
    pub staged: bool,
    /// Compare against this commit instead of the index. A sha, a branch name,
    /// or anything else git resolves as a revision.
    pub against: Option<String>,
}

impl DiffRequest {
    /// The working tree against the index - what the viewer shows by default.
    pub fn worktree() -> Self {
        Self {
            path: None,
            staged: false,
            against: None,
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn staged(mut self, staged: bool) -> Self {
        self.staged = staged;
        self
    }

    pub fn against(mut self, against: impl Into<String>) -> Self {
        self.against = Some(against.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitDiff {
    pub files: Vec<GitFileDiff>,
    /// True when the diff was cut at [`MAX_DIFF_LINES`], so the UI can say the
    /// answer is partial rather than implying the rest of the file is
    /// unchanged.
    pub truncated: bool,
}

impl GitDiff {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// One file's worth of a diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitFileDiff {
    /// Repository-relative, forward slashes - what git said.
    pub relative_path: String,
    /// Where a rename or copy came from. `None` otherwise.
    pub old_path: Option<String>,
    /// True when git refused to diff the contents. `hunks` is then empty, on
    /// purpose: the alternative is megabytes of noise nobody can read.
    pub binary: bool,
    pub hunks: Vec<GitHunk>,
}

/// One `@@ ... @@` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHunk {
    #[ts(type = "number")]
    pub old_start: u32,
    #[ts(type = "number")]
    pub old_lines: u32,
    #[ts(type = "number")]
    pub new_start: u32,
    #[ts(type = "number")]
    pub new_lines: u32,
    /// The text git puts after the second `@@` - usually the enclosing
    /// function. Empty when git had nothing to say.
    pub header: String,
    pub lines: Vec<GitDiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitDiffLineKind {
    Context,
    Added,
    Removed,
}

/// One line of a hunk, with its number on each side already worked out.
///
/// The numbers are computed here rather than in the UI because a renderer that
/// counted lines itself would be a second implementation of the diff format -
/// the same reason the parsing is in Rust at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitDiffLine {
    pub kind: GitDiffLineKind,
    /// Without the leading `+`, `-` or space.
    pub content: String,
    /// `None` on an added line, which has no old side.
    #[ts(type = "number | null")]
    pub old_line: Option<u32>,
    /// `None` on a removed line.
    #[ts(type = "number | null")]
    pub new_line: Option<u32>,
    /// True when git said `\ No newline at end of file` about this line. The
    /// UI marks it; nothing else depends on it.
    pub no_newline: bool,
}
