//! One `--porcelain=v2` record, decoded.
//!
//! Split from the loop above it so that file is about *sequencing* records -
//! headers, the rename pair, the cap - and this one is about reading a single
//! line. The five shapes, each already stripped of its NUL terminator:
//!
//! ```text
//! 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>              ordinary change
//! 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <Xscore> <path>     rename or copy
//! u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>    unmerged
//! ? <path>                                                  untracked
//! ! <path>                                                  ignored
//! ```
//!
//! The field indices below are counted off those lines. They are the one thing
//! in this file that would silently produce wrong paths if they were wrong,
//! which is why every shape has a test against recorded output.

use crate::types::{GitEntry, GitFileState};

use super::super::paths::PathStyle;

pub fn entry_from(
    record: &str,
    original: Option<&str>,
    root: &str,
    style: PathStyle,
) -> Option<GitEntry> {
    let (index, worktree, relative) = match record.chars().next()? {
        '1' => {
            let (index, worktree) = states(field(record, 1)?);
            (index, worktree, field_rest(record, 8)?)
        }
        '2' => {
            let (index, worktree) = states(field(record, 1)?);
            (index, worktree, field_rest(record, 9)?)
        }
        // Both sides of an unmerged path are the conflict. Which of DD, AU or
        // UU it is matters to the merge tool, not to a badge in a tree.
        'u' => (
            GitFileState::Conflicted,
            GitFileState::Conflicted,
            field_rest(record, 10)?,
        ),
        '?' => (
            GitFileState::Untracked,
            GitFileState::Untracked,
            field_rest(record, 1)?,
        ),
        '!' => (
            GitFileState::Ignored,
            GitFileState::Ignored,
            field_rest(record, 1)?,
        ),
        _ => return None,
    };

    // A wholly-ignored directory arrives with a trailing slash; every other
    // row is a file and has none.
    let relative = relative.trim_end_matches('/').to_string();
    if relative.is_empty() {
        return None;
    }
    Some(GitEntry {
        path: style.absolute(root, &relative),
        relative_path: relative,
        index,
        worktree,
        original_path: original.map(str::to_string),
    })
}

/// The `XY` pair: X is the staged side, Y the unstaged one.
fn states(xy: &str) -> (GitFileState, GitFileState) {
    let mut chars = xy.chars();
    (
        state(chars.next().unwrap_or('.')),
        state(chars.next().unwrap_or('.')),
    )
}

fn state(code: char) -> GitFileState {
    match code {
        'M' => GitFileState::Modified,
        'A' => GitFileState::Added,
        'D' => GitFileState::Deleted,
        'R' => GitFileState::Renamed,
        'C' => GitFileState::Copied,
        'T' => GitFileState::TypeChanged,
        'U' => GitFileState::Conflicted,
        // '.' and anything git adds later. An unrecognised code is a clean
        // side, never a guess at what it might have meant.
        _ => GitFileState::Unmodified,
    }
}

/// One space-delimited field. Only used for the fixed-width head of a record;
/// the path is taken by [`field_rest`], because a path may contain spaces.
fn field(record: &str, index: usize) -> Option<&str> {
    record.split(' ').nth(index)
}

/// Everything from field `index` to the end of the record, spaces included.
fn field_rest(record: &str, index: usize) -> Option<&str> {
    let mut rest = record;
    for _ in 0..index {
        rest = rest.split_once(' ')?.1;
    }
    (!rest.is_empty()).then_some(rest)
}

#[cfg(test)]
mod tests;
