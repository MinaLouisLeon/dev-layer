//! Every shape of `u` record a merge can leave, and what this crate makes
//! of each.
//!
//! The kinds are the point. `GitFileState::Conflicted` is enough for a
//! badge and is not enough for a button: taking theirs on a both-modified
//! file keeps a file, and on a deleted-by-them file removes one.

use super::*;

/// A real `u` record, minus its NUL. The ten fields before the path are
/// what the index in `field_rest` is counted off, so they are spelled out
/// here rather than abbreviated.
fn record(xy: &str, path: &str) -> String {
    format!("u {xy} N... 100644 100644 100644 100644 aaa bbb ccc {path}")
}

fn output(records: &[String]) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: format!("{}\0", records.join("\0")),
        stderr: String::new(),
    }
}

#[test]
fn every_shape_of_conflict_keeps_its_own_kind() {
    let listing = output(&[
        record("UU", "both.txt"),
        record("AA", "added.txt"),
        record("DD", "gone.txt"),
        record("AU", "ours.txt"),
        record("UA", "theirs.txt"),
        record("DU", "we-deleted.txt"),
        record("UD", "they-deleted.txt"),
    ]);
    let kinds: Vec<_> = parse(&listing, "/root", PathStyle::posix())
        .into_iter()
        .map(|conflict| conflict.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            GitConflictKind::BothModified,
            GitConflictKind::BothAdded,
            GitConflictKind::BothDeleted,
            GitConflictKind::AddedByUs,
            GitConflictKind::AddedByThem,
            GitConflictKind::DeletedByUs,
            GitConflictKind::DeletedByThem,
        ]
    );
}

#[test]
fn a_path_containing_spaces_survives_whole() {
    let listing = output(&[record("UU", "docs/release notes.md")]);
    let conflicts = parse(&listing, "/root", PathStyle::posix());
    assert_eq!(conflicts[0].relative_path, "docs/release notes.md");
    assert_eq!(conflicts[0].path, "/root/docs/release notes.md");
}

#[test]
fn ordinary_and_untracked_records_are_not_conflicts() {
    let listing = GitOutput {
        code: Some(0),
        stdout: "1 .M N... 100644 100644 100644 aaa bbb a.txt\0? new.txt\0".to_string(),
        stderr: String::new(),
    };
    assert!(parse(&listing, "/root", PathStyle::posix()).is_empty());
}

#[test]
fn a_clean_tree_has_no_conflicts_rather_than_an_error() {
    let listing = GitOutput {
        code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    };
    assert!(parse(&listing, "/root", PathStyle::posix()).is_empty());
}

#[test]
fn a_kind_this_build_does_not_know_reads_as_both_modified() {
    // The commonest shape, and the one whose three controls all mean
    // something for a file that exists on both sides.
    assert_eq!(
        GitConflictKind::from_xy("ZZ"),
        GitConflictKind::BothModified
    );
}

#[test]
fn the_delete_shapes_know_they_are_deletes() {
    // The controls read this: there is no "open and edit" for a path where
    // the choice is between a file and no file.
    assert!(GitConflictKind::DeletedByUs.is_delete());
    assert!(GitConflictKind::DeletedByThem.is_delete());
    assert!(GitConflictKind::BothDeleted.is_delete());
    assert!(!GitConflictKind::BothModified.is_delete());
}
