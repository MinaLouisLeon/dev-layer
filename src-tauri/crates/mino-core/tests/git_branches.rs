//! Listing branches, against real repositories.
//!
//! The parser has unit tests of its own against recorded records. What is
//! asserted here is that git actually *emits* those records - `--format` is a
//! documented interface, and the only way to know this parser reads the git
//! people have installed is to run it.
//!
//! Switching between them is `git_checkout.rs`; reading what there is to
//! switch to is here.
//!
//! Every test returns early when git is absent, like its siblings.

use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn branches_lists_local_and_remote_and_marks_the_one_head_is_on() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    git(root, &["branch", "dev"]);
    // A remote-tracking ref without a remote to fetch from: `update-ref` is
    // enough, because a remote branch *is* a ref under `refs/remotes/`.
    git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    let transport = connected(root).await;

    let branches = surface(&transport).branches().await.unwrap();
    let names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"main"), "{names:?}");
    assert!(names.contains(&"dev"), "{names:?}");
    assert!(names.contains(&"origin/main"), "{names:?}");

    let main = branches.iter().find(|b| b.name == "main").unwrap();
    assert!(main.is_head);
    assert!(!main.is_remote);
    // The tip is read from git, not invented: a branch with a commit on it
    // has one here.
    assert!(main.last_commit.is_some());

    let remote = branches.iter().find(|b| b.name == "origin/main").unwrap();
    assert!(remote.is_remote);
    assert!(!remote.is_head);
}

#[tokio::test]
async fn the_remote_head_symref_is_not_offered_as_a_second_name_for_a_branch() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    // `origin/HEAD` points at `origin/main`, which is already in the list.
    // Note git shortens its name to plain `origin`, so a picker that did not
    // drop it would show a branch called `origin` that is really another one.
    git(
        root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );
    let transport = connected(root).await;

    let names: Vec<String> = surface(&transport)
        .branches()
        .await
        .unwrap()
        .into_iter()
        .map(|branch| branch.name)
        .collect();
    assert!(names.contains(&"origin/main".to_string()), "{names:?}");
    assert!(!names.contains(&"origin".to_string()), "{names:?}");
}

#[tokio::test]
async fn a_tracking_branch_reports_how_far_it_has_drifted() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    // A remote has to be *configured*, not just have a ref under it: without
    // `remote.origin.fetch` git cannot map `refs/heads/main` to a
    // remote-tracking ref, and `--set-upstream-to` refuses. The remote points
    // at the repository itself, so nothing is fetched over a network.
    git(root, &["remote", "add", "origin", "."]);
    git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(root, &["branch", "--set-upstream-to=origin/main", "main"]);
    // One commit the upstream does not have.
    std::fs::write(root.join("readme.md"), "# changed\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "second"]);
    let transport = connected(root).await;

    let branches = surface(&transport).branches().await.unwrap();
    let main = branches.iter().find(|b| b.name == "main").unwrap();
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));
    assert_eq!((main.ahead, main.behind), (1, 0));
}
