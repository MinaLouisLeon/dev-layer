//! `git log` paging, against real repositories.
//!
//! `show`, a commit's own diff and blame are in `git_show.rs`.

use mino_core::types::LogRequest;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

/// A repository with `count` commits on top of the fixture's first one.
fn with_commits(count: usize) -> tempfile::TempDir {
    let dir = repository();
    for index in 0..count {
        std::fs::write(
            dir.path().join("src/main.rs"),
            format!("fn main() {{ {index} }}\n"),
        )
        .unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", &format!("commit {index}")]);
    }
    dir
}

#[tokio::test]
async fn log_returns_commits_newest_first() {
    if !git_available() {
        return;
    }
    let dir = with_commits(3);
    let transport = connected(dir.path()).await;

    let log = surface(&transport).log(LogRequest::new()).await.unwrap();
    let subjects: Vec<&str> = log.commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(subjects, vec!["commit 2", "commit 1", "commit 0", "first"]);
    assert!(!log.truncated);
    assert!(log.commits[0].timestamp_ms > 1_600_000_000_000);
    assert!(log.commits[0].sha.starts_with(&log.commits[0].short_sha));
}

#[tokio::test]
async fn log_honours_its_limit_and_says_there_is_more() {
    if !git_available() {
        return;
    }
    let dir = with_commits(5);
    let transport = connected(dir.path()).await;

    let page = surface(&transport)
        .log(LogRequest::new().limit(2))
        .await
        .unwrap();
    assert_eq!(page.commits.len(), 2);
    assert!(page.truncated);

    // And the next page continues where it stopped rather than repeating.
    let next = surface(&transport)
        .log(LogRequest::new().limit(2).skip(2))
        .await
        .unwrap();
    assert_eq!(next.commits.len(), 2);
    assert_ne!(next.commits[0].sha, page.commits[0].sha);
    assert_eq!(next.commits[0].summary, "commit 2");
}

#[tokio::test]
async fn log_on_an_unborn_branch_is_empty_rather_than_an_error() {
    if !git_available() {
        return;
    }
    // `git log` fails outright on a repository with no commits. Having no
    // history yet is a state, not a failure.
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--initial-branch=main"]);
    let transport = connected(dir.path()).await;

    let log = surface(&transport).log(LogRequest::new()).await.unwrap();
    assert!(log.commits.is_empty());
    assert!(!log.truncated);
}

#[tokio::test]
async fn log_can_be_narrowed_to_one_path() {
    if !git_available() {
        return;
    }
    let dir = with_commits(2);
    let root = dir.path();
    std::fs::write(root.join("only-once.txt"), "x\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "touched only-once"]);
    let transport = connected(root).await;

    let log = surface(&transport)
        .log(LogRequest::new().path(root.join("only-once.txt").to_string_lossy().into_owned()))
        .await
        .unwrap();
    let subjects: Vec<&str> = log.commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(subjects, vec!["touched only-once"]);
}
