use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// One row of a directory listing, and also what `stat` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct DirEntry {
    /// Absolute path, always in the remote's own separator style.
    pub path: String,
    pub name: String,
    pub kind: EntryKind,
    #[ts(type = "number")]
    pub size: u64,
    /// Unix epoch milliseconds, or `None` when the platform withholds it.
    #[ts(type = "number | null")]
    pub modified_ms: Option<u64>,
    pub readonly: bool,
    /// True for dot-files on unix and for hidden-attributed files elsewhere.
    /// The UI dims them rather than hiding them.
    pub hidden: bool,
}

impl DirEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, EntryKind::Directory)
    }
}
