//! A repository stopped mid-merge.
//!
//! A conflict is one of the few states in git that is genuinely hard to fake -
//! the index gets three entries for one path, and only git puts them there -
//! so this makes a real one by merging two branches that changed the same line.
#![allow(dead_code)]

use super::git::{git, identify};

/// A repository mid-merge, with `a.txt` conflicted both ways.
///
/// `main` and `other` each changed the same line, and the merge has stopped.
pub fn conflicted() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    git(root, &["init", "--initial-branch=main"]);
    identify(root);

    std::fs::write(root.join("a.txt"), "original\n").unwrap();
    std::fs::write(root.join("kept.txt"), "untouched\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "first"]);

    git(root, &["checkout", "-b", "other"]);
    std::fs::write(root.join("a.txt"), "theirs\n").unwrap();
    git(root, &["commit", "-am", "theirs"]);

    git(root, &["checkout", "main"]);
    std::fs::write(root.join("a.txt"), "ours\n").unwrap();
    git(root, &["commit", "-am", "ours"]);

    // Expected to fail: that is the point.
    let merge = std::process::Command::new("git")
        .args(["merge", "other"])
        .current_dir(root)
        .output()
        .expect("git should run");
    assert!(!merge.status.success(), "the merge should have conflicted");
    dir
}
