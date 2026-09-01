//! Creating and deleting branches, and refusing a name git would not take.
//!
//! Split from `git_branches.rs` so neither file grows past the project's
//! ceiling: that one is about reading and switching, this one is about the two
//! calls that add and remove a ref.
//!
//! The refusals are the interesting half: each is a different thing the reader
//! can do something about, and reporting all three as one shell error would
//! leave nobody able to act on any of them.

use mino_core::types::CreateBranchRequest;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

async fn head(transport: &LocalTransport) -> Option<String> {
    surface(transport)
        .repository()
        .await
        .unwrap()
        .and_then(|repository| repository.branch)
}

async fn names(transport: &LocalTransport) -> Vec<String> {
    surface(transport)
        .branches()
        .await
        .unwrap()
        .into_iter()
        .map(|branch| branch.name)
        .collect()
}

#[tokio::test]
async fn create_without_checkout_makes_the_branch_and_stays_put() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let made = surface(&transport)
        .create_branch(CreateBranchRequest::new("feat/thing"))
        .await
        .unwrap();

    // Returned rather than assumed: the tip and the HEAD flag are things git
    // decided, and inventing them would be showing a branch nobody read.
    assert_eq!(made.name, "feat/thing");
    assert!(!made.is_head);
    assert!(names(&transport).await.contains(&"feat/thing".to_string()));
    assert_eq!(head(&transport).await.as_deref(), Some("main"));
}

#[tokio::test]
async fn create_with_checkout_switches_to_it() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let made = surface(&transport)
        .create_branch(CreateBranchRequest::new("feat/thing").checkout(true))
        .await
        .unwrap();

    assert!(made.is_head);
    assert_eq!(head(&transport).await.as_deref(), Some("feat/thing"));
}

#[tokio::test]
async fn a_duplicate_name_is_a_typed_error_and_not_a_silent_no_op() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let refused = surface(&transport)
        .create_branch(CreateBranchRequest::new("main"))
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("already exists"), "{refused}");
}

#[tokio::test]
async fn a_name_git_would_not_accept_is_refused_before_anything_is_written() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;
    let before = names(&transport).await;

    // Two different checks. `-x` is a legal *ref* name that would be read as
    // an option, so only the local pre-check catches it; the rest are git's
    // own rules, answered by `git check-ref-format`.
    for name in ["-x", "a branch", "feat..thing", "feat/", "ends.lock"] {
        let refused = surface(&transport)
            .create_branch(CreateBranchRequest::new(name))
            .await
            .unwrap_err();
        assert!(
            format!("{refused}").len() > 10,
            "{name} should be refused with a sentence"
        );
    }

    // Nothing was created by any of them.
    assert_eq!(names(&transport).await, before);
}

#[tokio::test]
async fn delete_refuses_the_branch_you_are_on_unless_forced() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let refused = surface(&transport)
        .delete_branch("main", false)
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("`main`"), "{refused}");
    assert!(names(&transport).await.contains(&"main".to_string()));
}

#[tokio::test]
async fn delete_removes_a_branch_that_is_not_checked_out() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    git(root, &["branch", "spare"]);
    let transport = connected(root).await;

    surface(&transport)
        .delete_branch("spare", false)
        .await
        .unwrap();
    assert!(!names(&transport).await.contains(&"spare".to_string()));
}
