//! Switching branches, against real repositories.
//!
//! Split from `git_branches.rs`, which is about reading the list. This is the
//! call that **changes files under the other panes**, and every test here is
//! about the same promise: git either switched or it did not, and a refusal
//! leaves HEAD and the working tree exactly as they were.
//!
//! Every test returns early when git is absent, like its siblings.

use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

/// The branch HEAD is on, read back through `repository()`.
async fn head(transport: &LocalTransport) -> Option<String> {
    surface(transport)
        .repository()
        .await
        .unwrap()
        .and_then(|repository| repository.branch)
}

#[tokio::test]
async fn checkout_moves_head_and_back_again() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    git(root, &["branch", "dev"]);
    let transport = connected(root).await;

    assert_eq!(head(&transport).await.as_deref(), Some("main"));
    surface(&transport).checkout("dev").await.unwrap();
    assert_eq!(head(&transport).await.as_deref(), Some("dev"));
    surface(&transport).checkout("main").await.unwrap();
    assert_eq!(head(&transport).await.as_deref(), Some("main"));
}

#[tokio::test]
async fn checking_out_a_branch_that_is_not_there_is_a_typed_error() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let refused = surface(&transport).checkout("nope").await.unwrap_err();
    let message = refused.to_string();
    assert!(message.contains("no branch named"), "{message}");
    // And HEAD did not move.
    assert_eq!(head(&transport).await.as_deref(), Some("main"));
}

#[tokio::test]
async fn a_checkout_that_would_overwrite_local_changes_leaves_the_tree_alone() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    // `dev` has a different `readme.md`, and the working tree has a third
    // version of it. Git refuses rather than choosing.
    git(root, &["checkout", "-b", "dev"]);
    std::fs::write(root.join("readme.md"), "# on dev\n").unwrap();
    git(root, &["commit", "-am", "dev version"]);
    git(root, &["checkout", "main"]);
    std::fs::write(root.join("readme.md"), "# uncommitted\n").unwrap();

    let transport = connected(root).await;
    let refused = surface(&transport).checkout("dev").await.unwrap_err();
    let message = refused.to_string();
    assert!(message.contains("overwrite"), "{message}");

    // The whole point: nothing moved. Not HEAD, and not the file.
    assert_eq!(head(&transport).await.as_deref(), Some("main"));
    assert_eq!(
        std::fs::read_to_string(root.join("readme.md")).unwrap(),
        "# uncommitted\n"
    );
}
