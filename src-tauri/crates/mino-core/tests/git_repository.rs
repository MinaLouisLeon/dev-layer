//! What `repository` says about the checkout the session is inside.
//!
//! Four shapes a repository can be in, and every one of them is real: no
//! repository at all, a committed branch, an unborn branch that has no commit
//! yet, and a detached HEAD. The header renders all four, so all four are
//! asserted here rather than assumed.
//!
//! Every test returns early when git is absent, like its siblings
//! `git_status.rs` and `git_status_guards.rs`.

use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn a_folder_that_is_not_a_repository_answers_none() {
    if !git_available() {
        return;
    }
    // A plain temp directory. Most folders are not repositories, and that is
    // an answer rather than a failure.
    let dir = tempfile::tempdir().unwrap();
    let transport = connected(dir.path()).await;
    assert_eq!(surface(&transport).repository().await.unwrap(), None);

    // `status` is the one that may complain, because a caller reaching it
    // without asking `repository` first has made a mistake worth reporting.
    assert!(surface(&transport).status().await.is_err());
}
#[tokio::test]
async fn a_clean_tree_has_a_branch_and_no_entries() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let repository = surface(&transport).repository().await.unwrap().unwrap();
    assert_eq!(repository.branch.as_deref(), Some("main"));
    assert!(
        repository.head.is_some(),
        "a committed repository has a head"
    );
    assert!(!repository.detached);
    assert_eq!(repository.upstream, None);
    assert_eq!((repository.ahead, repository.behind), (0, 0));

    let status = surface(&transport).status().await.unwrap();
    assert!(status.entries.is_empty(), "{:?}", status.entries);
    assert!(!status.is_dirty());
    assert!(!status.truncated);
}
#[tokio::test]
async fn an_unborn_branch_has_a_name_but_no_head() {
    if !git_available() {
        return;
    }
    // A fresh `git init`: a repository with a branch that has no commit yet.
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--initial-branch=main"]);
    let transport = connected(dir.path()).await;

    let repository = surface(&transport).repository().await.unwrap().unwrap();
    assert_eq!(repository.branch.as_deref(), Some("main"));
    assert_eq!(repository.head, None);
    assert!(!repository.detached);
}
#[tokio::test]
async fn a_detached_head_has_a_sha_but_no_branch() {
    if !git_available() {
        return;
    }
    let dir = repository();
    git(dir.path(), &["checkout", "--detach", "HEAD"]);
    let transport = connected(dir.path()).await;

    let repository = surface(&transport).repository().await.unwrap().unwrap();
    assert!(repository.detached);
    assert_eq!(repository.branch, None);
    assert!(repository.head.is_some());
}
