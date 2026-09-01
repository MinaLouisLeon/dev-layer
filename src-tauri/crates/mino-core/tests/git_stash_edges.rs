//! The stash's edges: untracked files, and the two refusals.
//!
//! Split from `git_stash.rs`, which is the round trip. These are the cases
//! where the answer is "no" - and each one has to say *which* no, because
//! "there is nothing to stash" and "that entry is gone" are two different
//! things the reader can do two different things about.
//!
//! Every test returns early when git is absent, like its siblings.

use mino_core::types::StashRequest;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

/// Dirties the tracked `readme.md` so there is something to set aside.
fn dirty(root: &std::path::Path, content: &str) {
    std::fs::write(root.join("readme.md"), content).unwrap();
}

#[tokio::test]
async fn untracked_files_stay_put_unless_they_are_asked_for() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    dirty(root, "# half written\n");
    std::fs::write(root.join("scratch.txt"), "notes\n").unwrap();
    let transport = connected(root).await;

    // The default. Nothing git has never seen moves because of a control
    // somebody reached for to *keep* their work.
    surface(&transport)
        .stash_push(StashRequest::new())
        .await
        .unwrap();
    assert!(root.join("scratch.txt").exists());
    surface(&transport).stash_apply(0, true).await.unwrap();

    // And with it asked for, the untracked file goes too.
    surface(&transport)
        .stash_push(StashRequest::new().include_untracked(true))
        .await
        .unwrap();
    assert!(!root.join("scratch.txt").exists());

    surface(&transport).stash_apply(0, true).await.unwrap();
    assert!(root.join("scratch.txt").exists());
}

#[tokio::test]
async fn stashing_a_clean_tree_says_so_rather_than_failing_obscurely() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let refused = surface(&transport)
        .stash_push(StashRequest::new())
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("nothing to stash"),
        "{refused}"
    );
}

#[tokio::test]
async fn an_index_that_is_not_there_is_a_typed_error() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let refused = surface(&transport).stash_drop(7).await.unwrap_err();
    assert!(refused.to_string().contains("no longer there"), "{refused}");
}
