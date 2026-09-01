//! Settling a conflict, three ways.
//!
//! Split from `git_conflicts.rs` because listing and settling are different
//! subjects, and this is the one that writes: two of the three resolutions
//! discard a side, and the third is the only one in git's vocabulary that
//! discards nothing.
//!
//! The `Manual` assertion is the one to read. It exists because "open the file,
//! fix it, mark it settled" is how most conflicts are actually resolved, and
//! an implementation that checked a side out first would silently throw away
//! the edit somebody had just made.

use mino_core::types::ConflictResolution;
use mino_core::{GitConflictTransport, GitTransport};

mod fixture;

use fixture::conflicted::conflicted;
use fixture::git::connected;
use fixture::git::git_available;

#[tokio::test]
async fn taking_our_side_keeps_this_branch_version_and_settles_it() {
    if !git_available() {
        return;
    }
    let dir = conflicted();
    let path = dir.path().join("a.txt");
    let transport = connected(dir.path()).await;

    transport
        .resolve(&path.to_string_lossy(), ConflictResolution::Ours)
        .await
        .expect("resolve");

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "ours\n");
    // Settled, not merely written: without the `add` the file would look
    // resolved and the commit would still refuse.
    assert!(transport.conflicts().await.unwrap().is_empty());
}

#[tokio::test]
async fn taking_their_side_keeps_the_incoming_version() {
    if !git_available() {
        return;
    }
    let dir = conflicted();
    let path = dir.path().join("a.txt");
    let transport = connected(dir.path()).await;

    transport
        .resolve(&path.to_string_lossy(), ConflictResolution::Theirs)
        .await
        .expect("resolve");

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theirs\n");
    assert!(transport.conflicts().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_manual_resolution_takes_the_file_exactly_as_it_is() {
    if !git_available() {
        return;
    }
    let dir = conflicted();
    let path = dir.path().join("a.txt");
    let transport = connected(dir.path()).await;

    // What somebody does after editing the markers out in the viewer.
    std::fs::write(&path, "ours and theirs, reconciled\n").unwrap();
    transport
        .resolve(&path.to_string_lossy(), ConflictResolution::Manual)
        .await
        .expect("resolve");

    // Nothing overwrote the edit - which is the whole point of `Manual`, and
    // the thing `ours`/`theirs` would have destroyed.
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ours and theirs, reconciled\n"
    );
    assert!(transport.conflicts().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_commit_is_refused_while_a_conflict_remains() {
    if !git_available() {
        return;
    }
    let dir = conflicted();
    let transport = connected(dir.path()).await;

    let refusal = transport
        .commit(mino_core::types::CommitRequest::new("too soon"))
        .await;
    assert!(refusal.is_err(), "a commit with conflicts must be refused");

    // And it goes through once the path is settled, which is what makes the
    // refusal a state rather than a wall.
    let path = dir.path().join("a.txt");
    transport
        .resolve(&path.to_string_lossy(), ConflictResolution::Ours)
        .await
        .expect("resolve");
    assert!(transport
        .commit(mino_core::types::CommitRequest::new("settled"))
        .await
        .is_ok());
}

#[tokio::test]
async fn resolving_a_path_outside_the_root_is_refused() {
    if !git_available() {
        return;
    }
    let dir = conflicted();
    let transport = connected(dir.path()).await;

    for path in ["/etc/passwd", "../outside.txt"] {
        let refusal = transport.resolve(path, ConflictResolution::Ours).await;
        assert!(refusal.is_err(), "{path} should be refused");
    }
    // And the real conflict is untouched by any of it.
    assert_eq!(transport.conflicts().await.unwrap().len(), 1);
}
