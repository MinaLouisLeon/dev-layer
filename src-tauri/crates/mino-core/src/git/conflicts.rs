//! The `u` records of a `--porcelain=v2` status, kept whole.
//!
//! Phase 1 reads the same records and collapses every one of them to
//! [`crate::types::GitFileState::Conflicted`], which is exactly right for a
//! badge in a tree: `DU` and `UU` are the same colour and the same one-word
//! label. This file exists because a *control* needs what a badge does not.
//! "Take theirs" on a both-modified file keeps a file; on a deleted-by-them
//! file it removes one. A reader about to press that button has to be told
//! which they are looking at.
//!
//! So this is a second reading of the same format rather than a change to the
//! first. The alternative - widening `GitEntry` with a conflict kind that is
//! `None` for every row in a repository that is not mid-merge - would put a
//! merge-only field on the type every pane in the app already reads.

use crate::types::{GitConflict, GitConflictKind};

use super::paths::PathStyle;
use super::GitOutput;

/// The unmerged paths in a status, in the order git listed them.
///
/// Records are NUL-terminated because the status was asked for with `-z`,
/// which is the only way a filename containing a newline survives intact.
///
/// Rows outside the session root are dropped, exactly as
/// [`super::interpret::status_from`] drops them: opening `repo/src` and being
/// shown a conflict in `repo/docs` would be the panel offering a control for a
/// file the session does not own.
pub fn parse(output: &GitOutput, root: &str, style: PathStyle) -> Vec<GitConflict> {
    output
        .stdout
        .split('\0')
        .filter_map(|record| conflict(record, root, style))
        .collect()
}

/// ```text
/// u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
/// ```
///
/// Ten fixed fields and then the path, which may contain spaces - so the path
/// is taken as "everything from field ten" rather than by splitting.
fn conflict(record: &str, root: &str, style: PathStyle) -> Option<GitConflict> {
    let rest = record.strip_prefix("u ")?;
    let xy = rest.split(' ').next()?;
    // Nine more fields after XY before the path begins.
    let relative = field_rest(rest, 9)?;
    let relative = relative.trim_end_matches('/');
    if relative.is_empty() {
        return None;
    }

    let path = style.absolute(root, relative);
    // The same containment test the status filter uses.
    if !style.within(root, &path) {
        return None;
    }
    Some(GitConflict {
        path,
        relative_path: relative.to_string(),
        kind: GitConflictKind::from_xy(xy),
    })
}

/// Everything from field `index` to the end, spaces included.
fn field_rest(record: &str, index: usize) -> Option<&str> {
    let mut rest = record;
    for _ in 0..index {
        rest = rest.split_once(' ')?.1;
    }
    (!rest.is_empty()).then_some(rest)
}

#[cfg(test)]
mod tests;
