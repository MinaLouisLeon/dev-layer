//! The argv a conflict is listed and settled with.
//!
//! The `Manual` assertion is the one that matters: it must check *nothing*
//! out, because the file on disk is the answer and overwriting it with
//! either side would throw away the edit somebody just made.

use super::*;

#[test]
fn taking_a_side_names_it_from_the_enum_and_never_from_text() {
    let ours = take_side_argv(ConflictResolution::Ours, "a.txt").unwrap();
    assert_eq!(ours[..2], ["checkout", "--ours"]);
    let theirs = take_side_argv(ConflictResolution::Theirs, "a.txt").unwrap();
    assert_eq!(theirs[..2], ["checkout", "--theirs"]);
}

#[test]
fn a_manual_resolution_checks_nothing_out() {
    // The whole point of `Manual`: the file on disk is the answer, and
    // overwriting it with either side would throw away the edit.
    assert!(take_side_argv(ConflictResolution::Manual, "a.txt").is_none());
}

#[test]
fn every_path_sits_behind_the_separator() {
    for argv in [
        take_side_argv(ConflictResolution::Ours, "-f").unwrap(),
        mark_resolved_argv("-f"),
    ] {
        let separator = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
        let path = argv.iter().position(|a| a == "-f").unwrap();
        assert!(separator < path, "{argv:?}");
    }
}

#[test]
fn listing_conflicts_skips_the_untracked_walk() {
    // The section re-reads after every resolution, and the untracked walk
    // is most of what a status costs.
    let argv = conflicts_argv();
    assert!(argv.contains(&"--untracked-files=no".to_string()));
    assert!(argv.contains(&"--porcelain=v2".to_string()));
}
