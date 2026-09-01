//! Committing, against real repositories.
//!
//! The commit message travels on stdin rather than in argv, which is why one
//! of these deliberately contains an apostrophe: that is an ordinary commit
//! message, and it would be a refused one if the message were an argument.
//!
//! Every test returns early when git is absent, like its siblings.

use mino_core::types::CommitRequest;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn commit_returns_a_sha_that_git_log_agrees_with() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    let transport = connected(root).await;
    surface(&transport)
        .stage(&[root.join("src/main.rs").to_string_lossy().into_owned()])
        .await
        .unwrap();

    // An apostrophe on purpose: the message travels on stdin precisely so
    // this is an ordinary commit message and not an error.
    let commit = surface(&transport)
        .commit(CommitRequest::new("Fix Bob's bug\n\nWith a body line.\n"))
        .await
        .unwrap();

    assert_eq!(commit.summary, "Fix Bob's bug");
    assert!(commit.sha.starts_with(&commit.short_sha));
    assert_eq!(commit.author, "Test");
    assert!(commit.timestamp_ms > 1_600_000_000_000);

    let logged = std::process::Command::new("git")
        .args(["log", "-1", "--format=%H"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&logged.stdout).trim(), commit.sha);
    // And the tree is clean again, which is the point of having committed.
    assert!(surface(&transport)
        .status()
        .await
        .unwrap()
        .entries
        .is_empty());
}

#[tokio::test]
async fn commit_with_nothing_staged_fails_rather_than_doing_nothing() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    let refused = surface(&transport)
        .commit(CommitRequest::new("nothing to say"))
        .await
        .unwrap_err();
    assert!(
        format!("{refused}").contains("nothing staged"),
        "{refused:?}"
    );

    // An empty message is refused too, and before git is ever spawned.
    assert!(surface(&transport)
        .commit(CommitRequest::new("   \n "))
        .await
        .is_err());
}

#[tokio::test]
async fn amend_replaces_the_previous_commit_rather_than_adding_one() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    let transport = connected(root).await;

    let before = count_commits(root);
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    surface(&transport)
        .commit(CommitRequest::new("amended message").all(true).amend(true))
        .await
        .unwrap();

    assert_eq!(count_commits(root), before, "amend must not add a commit");
    let summary = std::process::Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&summary.stdout).trim(),
        "amended message"
    );
}

fn count_commits(root: &std::path::Path) -> usize {
    let output = std::process::Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}
