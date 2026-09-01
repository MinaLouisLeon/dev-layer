//! What `status` says about a working tree, against real repositories built by
//! real `git`.
//!
//! Recorded output is parsed in `git::porcelain`'s own unit tests. What is
//! asserted here is the half those cannot reach: that the argv is right, that
//! git's exit codes are read the way the module thinks, and that the two agree
//! with the git actually installed.
//!
//! Every test returns early when git is absent. That is not a gap - a machine
//! without git is one this app degrades on by design, and a red suite there
//! would be reporting something untrue.
//!
//! Its two siblings: `git_repository.rs` covers what `repository` says about
//! the checkout itself, and `git_status_guards.rs` covers the limits - the
//! path guard, and search's degrade path.

use mino_core::types::GitFileState;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

/// The git surface of a connected local transport.
fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn every_kind_of_change_maps_to_the_right_state() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();

    // The rename first, and committed on its own: everything after this point
    // is meant to still be uncommitted when the status is taken.
    std::fs::write(root.join("before.txt"), "x\n").unwrap();
    git(root, &["add", "before.txt"]);
    git(root, &["commit", "-m", "add before"]);
    git(root, &["mv", "before.txt", "after.txt"]);

    // Modified, not staged.
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    // Added: a new file, staged.
    std::fs::write(root.join("src/added.rs"), "pub fn added() {}\n").unwrap();
    git(root, &["add", "src/added.rs"]);
    // Deleted from the work tree.
    std::fs::remove_file(root.join("readme.md")).unwrap();
    // Untracked.
    std::fs::write(root.join("notes.txt"), "x\n").unwrap();

    let transport = connected(root).await;
    let status = surface(&transport).status().await.unwrap();
    let find = |relative: &str| {
        status
            .entries
            .iter()
            .find(|entry| entry.relative_path == relative)
            .unwrap_or_else(|| panic!("{relative} should be in {:?}", status.entries))
    };

    assert_eq!(find("src/main.rs").worktree, GitFileState::Modified);
    assert_eq!(find("src/added.rs").index, GitFileState::Added);
    assert_eq!(find("readme.md").worktree, GitFileState::Deleted);
    assert_eq!(find("notes.txt").worktree, GitFileState::Untracked);

    let renamed = find("after.txt");
    assert_eq!(renamed.index, GitFileState::Renamed);
    assert_eq!(renamed.original_path.as_deref(), Some("before.txt"));

    assert!(status.is_dirty());
    // The absolute path is the one the tree addresses files by, so it has to
    // be in this platform's separator style, not git's forward slashes.
    assert!(find("src/main.rs")
        .path
        .ends_with(&format!("src{}main.rs", std::path::MAIN_SEPARATOR)));
}

#[tokio::test]
async fn staged_and_then_modified_again_reports_both_sides() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { /* staged */ }\n").unwrap();
    git(root, &["add", "src/main.rs"]);
    std::fs::write(root.join("src/main.rs"), "fn main() { /* and again */ }\n").unwrap();

    let transport = connected(root).await;
    let status = surface(&transport).status().await.unwrap();
    let entry = &status.entries[0];
    assert_eq!(entry.relative_path, "src/main.rs");
    // The condition the two-state shape exists for. One state would have to
    // pick a side and lose the other.
    assert_eq!(entry.index, GitFileState::Modified);
    assert_eq!(entry.worktree, GitFileState::Modified);
}
