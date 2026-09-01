//! What a remote call refuses, and how quickly.
//!
//! Beside `git_remote.rs` the way `git_status_guards.rs` sits beside
//! `git_status.rs`: one file is about the calls working, and this one is about
//! them refusing.
//!
//! The timing assertion is the one worth keeping. `plan/decisions.md` D3 chose
//! to delegate authentication to the system, and the cost of that choice is
//! that git may want to ask a question this process cannot answer. The whole
//! mitigation is `GIT_TERMINAL_PROMPT=0` plus a timeout - so "fails rather than
//! waits" is not a nicety here, it is the property being bought.

use mino_core::{GitRemoteTransport, TransportError};

mod fixture;

use fixture::git::{connected, git, git_available};
use fixture::git_remote::with_remote;

#[tokio::test]
async fn a_remote_that_is_not_there_fails_without_hanging() {
    if !git_available() {
        return;
    }
    // The property `GIT_TERMINAL_PROMPT=0` buys, asserted by the clock: under
    // D3 this app has no credential to answer a prompt with, so a call that
    // could be asked one has to fail rather than wait.
    let (dir, work) = with_remote();
    git(
        &work,
        &["remote", "set-url", "origin", "/nonexistent/repository.git"],
    );

    let transport = connected(&work).await;
    let started = std::time::Instant::now();
    let refusal = transport.fetch(None).await;

    assert!(refusal.is_err());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "a missing remote must fail rather than wait for the timeout"
    );
    drop(dir);
}

#[tokio::test]
async fn a_remote_name_that_could_be_read_as_an_option_never_reaches_git() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let transport = connected(&work).await;

    for bad in ["-x", "--upload-pack=touch pwned", ""] {
        let refusal = transport.fetch(Some(bad.to_string())).await.unwrap_err();
        assert!(
            matches!(refusal, TransportError::InvalidArgument { .. }),
            "{bad:?} produced {refusal:?}"
        );
    }
    assert!(!work.join("pwned").exists());
    drop(dir);
}
