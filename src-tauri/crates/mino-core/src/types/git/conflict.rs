use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which of the seven ways a merge can leave a path unmerged.
///
/// [`super::GitFileState::Conflicted`] is enough for a badge in a tree - phase
/// 1 collapsed all seven into it on purpose - but it is **not** enough for the
/// controls that resolve one. "Take theirs" on a both-modified file keeps a
/// file; "take theirs" on a deleted-by-them file removes one. The reader has
/// to be told which they are looking at before they choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitConflictKind {
    /// `UU` - both sides edited it. The ordinary case, and the only one where
    /// opening the file and editing the markers is the usual answer.
    BothModified,
    /// `AA` - both sides added a file of the same name.
    BothAdded,
    /// `DD` - both sides deleted it. Nothing to keep; marking it resolved is
    /// the whole of the work.
    BothDeleted,
    /// `AU` - we added it, they did not have it.
    AddedByUs,
    /// `UA` - they added it, we did not have it.
    AddedByThem,
    /// `DU` - we deleted it, they changed it.
    DeletedByUs,
    /// `UD` - they deleted it, we changed it.
    DeletedByThem,
}

impl GitConflictKind {
    /// The `XY` pair from a `--porcelain=v2` `u` record.
    ///
    /// An unrecognised pair answers [`Self::BothModified`], which is the
    /// commonest shape and the one whose controls are safe for any of them:
    /// take-ours, take-theirs and mark-resolved all mean something for a file
    /// that exists on both sides.
    pub fn from_xy(xy: &str) -> Self {
        match xy {
            "AA" => Self::BothAdded,
            "DD" => Self::BothDeleted,
            "AU" => Self::AddedByUs,
            "UA" => Self::AddedByThem,
            "DU" => Self::DeletedByUs,
            "UD" => Self::DeletedByThem,
            _ => Self::BothModified,
        }
    }

    /// True when one side has no file at all.
    ///
    /// The controls read this: there is no point offering "open and edit" for
    /// a path where the choice is between a file and no file.
    pub fn is_delete(self) -> bool {
        matches!(
            self,
            Self::BothDeleted | Self::DeletedByUs | Self::DeletedByThem
        )
    }
}

/// One path a merge could not settle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitConflict {
    /// Absolute, in the target's own separator style, so it compares against
    /// [`crate::types::DirEntry::path`] without further work.
    pub path: String,
    /// Repository-relative, always forward slashes - what git itself said.
    pub relative_path: String,
    pub kind: GitConflictKind,
}

/// How one conflicted path is to be settled.
///
/// Three answers, and the third is the one that matters most. `Ours` and
/// `Theirs` throw one side away; `Manual` throws nothing away and means "I
/// have edited this file, take it as it now stands" - which is what makes
/// resolving by hand a first-class option rather than something you do in a
/// terminal after giving up on the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum ConflictResolution {
    /// Keep the version on the branch you are on. **Discards their side.**
    Ours,
    /// Keep the version being merged in. **Discards your side.**
    Theirs,
    /// Take the file exactly as it is on disk now. Nothing is discarded and
    /// nothing is overwritten; the file is simply marked resolved.
    Manual,
}

impl ConflictResolution {
    /// The word git uses for the side, or `None` for a manual resolution -
    /// which is not a side at all and does not check anything out.
    pub fn checkout_flag(self) -> Option<&'static str> {
        match self {
            Self::Ours => Some("--ours"),
            Self::Theirs => Some("--theirs"),
            Self::Manual => None,
        }
    }
}
