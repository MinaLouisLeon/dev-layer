//! What the history calls refuse.
//!
//! Two limits, and both matter for a different reason. A **path** outside the
//! connected root must not be readable - blame and diff would otherwise print
//! the contents of a file the session does not own. A **revision** is not a
//! path, so the path guard cannot rule on it, and the thing to prove is that
//! it cannot be read as a git option: `--upload-pack` runs a program and
//! `--output` writes a file.

use mino_core::types::{DiffRequest, LogRequest};
use mino_core::{GitTransport, LocalTransport, Transport, TransportError};

mod fixture;
use fixture::git::{connected, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

fn escapes(error: TransportError) -> bool {
    matches!(error, TransportError::PathEscapesRoot { .. })
}

#[tokio::test]
async fn diff_and_blame_refuse_a_path_outside_the_root() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "not yours\n").unwrap();
    let transport = connected(dir.path()).await;
    let path = secret.to_string_lossy().into_owned();

    assert!(escapes(
        surface(&transport)
            .diff(DiffRequest::worktree().path(path.clone()))
            .await
            .unwrap_err()
    ));
    assert!(escapes(surface(&transport).blame(&path).await.unwrap_err()));
    assert!(escapes(
        surface(&transport)
            .log(LogRequest::new().path(path))
            .await
            .unwrap_err()
    ));
}

#[tokio::test]
async fn a_traversal_out_of_the_root_is_refused() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;
    let escape = dir
        .path()
        .join("src/../../elsewhere.txt")
        .to_string_lossy()
        .into_owned();

    assert!(escapes(
        surface(&transport).blame(&escape).await.unwrap_err()
    ));
    assert!(escapes(
        surface(&transport)
            .diff(DiffRequest::worktree().path(escape))
            .await
            .unwrap_err()
    ));
}

#[tokio::test]
async fn a_revision_that_would_be_read_as_an_option_is_refused() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;
    let target = dir.path().join("pwned.txt");

    for revision in ["--upload-pack=touch pwned.txt", "--output=pwned.txt", "-n1"] {
        assert!(
            surface(&transport)
                .diff(DiffRequest::worktree().against(revision))
                .await
                .is_err(),
            "{revision} should be refused"
        );
        assert!(surface(&transport).show(revision).await.is_err());
        assert!(surface(&transport)
            .commit_diff(revision, None)
            .await
            .is_err());
    }

    // The assertion that actually matters: none of it reached git.
    assert!(!target.exists(), "a refused revision must not have run");
}

#[tokio::test]
async fn a_revision_carrying_shell_syntax_is_refused() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;

    for revision in ["main; touch pwned", "$(whoami)", "HEAD`id`", "it's"] {
        assert!(
            surface(&transport).show(revision).await.is_err(),
            "{revision} should be refused"
        );
    }
    assert!(!dir.path().join("pwned").exists());
}

#[tokio::test]
async fn an_ordinary_revision_is_still_accepted() {
    if !git_available() {
        return;
    }
    // The guard must not be so strict that it refuses the real thing.
    let dir = repository();
    let transport = connected(dir.path()).await;
    assert!(surface(&transport).show("HEAD").await.is_ok());
    assert!(surface(&transport)
        .diff(DiffRequest::worktree().against("HEAD"))
        .await
        .is_ok());
}
