//! Fetch and pull, against a **local bare repository** used as a remote.
//!
//! No network, no credential, no third-party service. A bare repository on
//! disk is a real git remote in every way this phase cares about - refs,
//! fast-forwards, non-fast-forward rejections, `--force-with-lease` leases -
//! and a test that reached github.com would be a test that fails on a train.
//!
//! The remote is made to move by a *second clone* pushing to it, which is how
//! being behind is produced rather than invented.
//!
//! Push is next door, in `git_push.rs`.

use mino_core::types::{GitPullOutcome, PullRequest};
use mino_core::{GitRemoteTransport, TransportError};

mod fixture;

use fixture::git::{commit_file, connected, git, git_available};
use fixture::git_remote::{second_clone, with_remote};

#[tokio::test]
async fn it_lists_the_configured_remote() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let transport = connected(&work).await;

    let remotes = transport.remotes().await.expect("remotes");
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].name, "origin");
    // A path is a legitimate remote URL, and the one that needs nothing.
    assert!(remotes[0].fetch_url.contains("origin.git"));
    drop(dir);
}

#[tokio::test]
async fn a_fetch_brings_refs_down_and_touches_no_file() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let other = second_clone(dir.path(), "other");
    commit_file(&other, "readme.md", "# moved\n", "second");
    git(&other, &["push", "origin", "main"]);

    let transport = connected(&work).await;
    let before = std::fs::read_to_string(work.join("readme.md")).unwrap();

    let result = transport.fetch(None).await.expect("fetch");
    assert_eq!(result.remote, "origin");

    // The whole point of fetch: the working tree is exactly as it was.
    assert_eq!(
        std::fs::read_to_string(work.join("readme.md")).unwrap(),
        before
    );
    drop(dir);
}

#[tokio::test]
async fn a_pull_with_nothing_to_bring_reports_it_rather_than_nothing() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let transport = connected(&work).await;

    let result = transport.pull(PullRequest::default()).await.expect("pull");
    assert_eq!(result.outcome, GitPullOutcome::AlreadyUpToDate);
    drop(dir);
}

#[tokio::test]
async fn a_pull_that_can_fast_forward_does_and_says_which() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let other = second_clone(dir.path(), "other");
    commit_file(&other, "readme.md", "# moved\n", "second");
    git(&other, &["push", "origin", "main"]);

    let transport = connected(&work).await;
    let result = transport.pull(PullRequest::default()).await.expect("pull");

    assert_eq!(result.outcome, GitPullOutcome::FastForwarded);
    // And the file really moved, which is the half a summary cannot prove.
    assert_eq!(
        std::fs::read_to_string(work.join("readme.md")).unwrap(),
        "# moved\n"
    );
    drop(dir);
}

#[tokio::test]
async fn a_pull_over_uncommitted_work_is_refused_before_anything_is_sent() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    std::fs::write(work.join("readme.md"), "# mine, unsaved\n").unwrap();

    let transport = connected(&work).await;
    let refusal = transport.pull(PullRequest::default()).await.unwrap_err();

    assert!(matches!(refusal, TransportError::InvalidArgument { .. }));
    // The edit is still there. Never stashed on the reader's behalf: a stash
    // they did not make is a stash they will not think to look for.
    assert_eq!(
        std::fs::read_to_string(work.join("readme.md")).unwrap(),
        "# mine, unsaved\n"
    );
    drop(dir);
}
