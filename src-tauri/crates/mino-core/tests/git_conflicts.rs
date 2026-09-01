//! Conflicts, listed against a real merge.
//!
//! Made by merging two real branches that changed the same line, because a
//! conflict is one of the few states in git that is genuinely hard to fake:
//! the index gets three entries for one path, and only git puts them there.
//!
//! Settling one is next door, in `git_resolve.rs`.

use mino_core::types::GitConflictKind;
use mino_core::GitConflictTransport;

mod fixture;

use fixture::conflicted::conflicted;
use fixture::git::{connected, git_available};

#[tokio::test]
async fn a_conflicted_merge_lists_the_path_and_says_which_kind() {
    if !git_available() {
        return;
    }
    let dir = conflicted();
    let transport = connected(dir.path()).await;

    let conflicts = transport.conflicts().await.expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].relative_path, "a.txt");
    // The kind is the reason this is a type of its own: taking theirs on a
    // both-modified file keeps a file, and on a deleted-by-them file removes
    // one. A control cannot be drawn without knowing which.
    assert_eq!(conflicts[0].kind, GitConflictKind::BothModified);
}

#[tokio::test]
async fn a_clean_repository_has_no_conflicts_rather_than_an_error() {
    if !git_available() {
        return;
    }
    let dir = fixture::git::repository();
    let transport = connected(dir.path()).await;
    assert!(transport.conflicts().await.expect("conflicts").is_empty());
}

#[tokio::test]
async fn a_folder_that_is_not_a_repository_has_no_conflicts() {
    if !git_available() {
        return;
    }
    // A true answer, not an error worth raising: the panel asks this of every
    // session, and most folders are not checkouts.
    let dir = tempfile::tempdir().expect("temp dir");
    let transport = connected(dir.path()).await;
    assert!(transport.conflicts().await.expect("conflicts").is_empty());
}
