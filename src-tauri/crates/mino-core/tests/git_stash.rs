//! The stash, against real repositories.
//!
//! The round trip is the point: push, list, apply, pop and drop have to agree
//! about *which* entry they mean, and the only thing naming one is a number
//! whose meaning shifts as the stack changes. A parser that read the wrong
//! selector would look exactly like one that read the right one until an
//! entry was dropped.
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

fn readme(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("readme.md")).unwrap()
}

#[tokio::test]
async fn push_list_apply_and_drop_round_trip() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    dirty(root, "# half written\n");
    let transport = connected(root).await;

    surface(&transport)
        .stash_push(StashRequest::new().message("half a refactor"))
        .await
        .unwrap();
    // The tree is back to the last commit, which is what "set aside" means.
    assert_eq!(readme(root), "# test\n");

    let entries = surface(&transport).stash_list().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].message, "half a refactor");
    assert_eq!(entries[0].branch.as_deref(), Some("main"));
    assert!(entries[0].timestamp_ms > 0);

    // `apply`, so the entry survives.
    surface(&transport).stash_apply(0, false).await.unwrap();
    assert_eq!(readme(root), "# half written\n");
    assert_eq!(surface(&transport).stash_list().await.unwrap().len(), 1);

    surface(&transport).stash_drop(0).await.unwrap();
    assert!(surface(&transport).stash_list().await.unwrap().is_empty());
}

#[tokio::test]
async fn pop_applies_and_removes_in_one_call() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    dirty(root, "# half written\n");
    let transport = connected(root).await;

    surface(&transport)
        .stash_push(StashRequest::new())
        .await
        .unwrap();
    surface(&transport).stash_apply(0, true).await.unwrap();

    assert_eq!(readme(root), "# half written\n");
    assert!(surface(&transport).stash_list().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_stash_with_no_message_still_says_which_branch_it_came_from() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    dirty(root, "# half written\n");
    let transport = connected(root).await;

    surface(&transport)
        .stash_push(StashRequest::new())
        .await
        .unwrap();

    let entries = surface(&transport).stash_list().await.unwrap();
    // Git writes `WIP on main: <sha> <subject>`, and the branch is split out
    // of it rather than shown again inside the message.
    assert_eq!(entries[0].branch.as_deref(), Some("main"));
    assert!(!entries[0].message.contains("WIP on"), "{:?}", entries[0]);
}
