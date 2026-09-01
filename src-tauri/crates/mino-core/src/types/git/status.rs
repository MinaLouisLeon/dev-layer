use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many status entries one call may carry before it reports itself
/// truncated. Reached only in a repository with an extraordinary number of
/// changes; the honest answer there is a partial list plus
/// [`GitStatus::truncated`], the same bargain [`crate::types::SearchHits`]
/// makes.
pub const MAX_STATUS_ENTRIES: u32 = 10_000;

/// The repository containing the connected root.
///
/// Its `root` is the work tree root, which may sit **above** the session root:
/// opening `repo/src` is a normal thing to do, and git still answers for the
/// whole tree. The two are separate fields for exactly that reason, and the
/// path guard keeps checking against the session root regardless.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitRepository {
    /// Absolute path of the work tree root, in the target's separator style.
    pub root: String,
    /// `None` on a detached HEAD, and on an unborn branch that has no commit
    /// yet - a fresh `git init` is both nameless and headless.
    pub branch: Option<String>,
    /// Short sha of HEAD. `None` before the first commit.
    pub head: Option<String>,
    pub detached: bool,
    /// The tracking branch, e.g. `origin/main`. `None` when none is set.
    pub upstream: Option<String>,
    /// Commits ahead of and behind `upstream`. Both zero when there is none.
    #[ts(type = "number")]
    pub ahead: u32,
    #[ts(type = "number")]
    pub behind: u32,
}

/// What happened to one file on one side of the index.
///
/// `Unmodified` is a real answer rather than an absence: an entry staged and
/// then left alone is `index: Added, worktree: Unmodified`, and the two
/// together are what the source control panel groups on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitFileState {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Conflicted,
    TypeChanged,
}

impl GitFileState {
    /// True for everything but a clean side. The dirty marker and the tree
    /// badge both ask this rather than matching on ten variants.
    pub fn is_dirty(self) -> bool {
        !matches!(self, Self::Unmodified | Self::Ignored)
    }
}

/// One path git had something to say about.
///
/// Two states, not one, because staged-and-then-modified-again is a common
/// condition and both sides have to survive the trip to the UI. Collapsing
/// them into a single state here would be re-litigated by every later feature
/// that needs to know which side a change is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitEntry {
    /// Absolute path, in the target's own separator style, so it compares
    /// against [`crate::types::DirEntry::path`] without further work.
    pub path: String,
    /// Repository-relative, always forward slashes - what git itself said.
    pub relative_path: String,
    /// The staged side.
    pub index: GitFileState,
    /// The unstaged side.
    pub worktree: GitFileState,
    /// Where a rename or a copy came from, repository-relative. `None`
    /// otherwise.
    pub original_path: Option<String>,
}

/// The working tree as git sees it, for the whole repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitStatus {
    pub repository: GitRepository,
    pub entries: Vec<GitEntry>,
    /// True when the list was cut at [`MAX_STATUS_ENTRIES`], so the UI can say
    /// the answer is partial rather than implying the rest of the tree is
    /// clean.
    pub truncated: bool,
}

impl GitStatus {
    /// True when anything at all is uncommitted. Drives the header's dirty
    /// marker, which is why ignored entries do not count.
    pub fn is_dirty(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.index.is_dirty() || entry.worktree.is_dirty())
    }
}
