//! What the mutating git calls refuse to do.
//!
//! The behaviour is in `git_mutate.rs`; the limit is here, and it is one
//! limit asserted against all four methods: **a path outside the connected
//! root never reaches git.**
//!
//! `discard` is why this file is separate rather than a case tacked onto the
//! behaviour suite. It deletes work that exists nowhere else, so "could it be
//! aimed at a file the session does not own" is the question that matters most
//! about it, and a regression here is a leak rather than an inconvenience.

use mino_core::types::CommitRequest;
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
async fn every_mutating_method_refuses_a_path_outside_the_root() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("victim.txt");
    std::fs::write(&victim, "not yours\n").unwrap();
    let transport = connected(dir.path()).await;

    let paths = vec![victim.to_string_lossy().into_owned()];
    assert!(escapes(
        surface(&transport).stage(&paths).await.unwrap_err()
    ));
    assert!(escapes(
        surface(&transport).unstage(&paths).await.unwrap_err()
    ));
    assert!(escapes(
        surface(&transport).discard(&paths).await.unwrap_err()
    ));

    // The file is untouched, which is the assertion that actually matters:
    // the refusal happened before git was spawned, not after.
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "not yours\n");
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
        surface(&transport)
            .discard(std::slice::from_ref(&escape))
            .await
            .unwrap_err()
    ));
    assert!(escapes(
        surface(&transport).stage(&[escape]).await.unwrap_err()
    ));
}

#[tokio::test]
async fn one_bad_path_refuses_the_whole_batch() {
    if !git_available() {
        return;
    }
    // Half-applying a stage and then reporting a failure would leave the index
    // in a state nobody asked for and the UI unable to say what happened.
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    let transport = connected(root).await;

    let batch = vec![
        root.join("src/main.rs").to_string_lossy().into_owned(),
        "/etc/passwd".to_string(),
    ];
    assert!(escapes(
        surface(&transport).stage(&batch).await.unwrap_err()
    ));

    // Nothing was staged: the good path did not go through on its own.
    let status = surface(&transport).status().await.unwrap();
    let entry = status
        .entries
        .iter()
        .find(|e| e.relative_path == "src/main.rs")
        .unwrap();
    assert_eq!(entry.index, mino_core::types::GitFileState::Unmodified);
}

#[tokio::test]
async fn naming_the_root_itself_is_not_a_way_to_discard_everything() {
    if !git_available() {
        return;
    }
    // An empty slice is how a caller asks for everything, and it confirms with
    // a count in the UI. Passing the root as a path is not that, and letting
    // it through would turn a one-file discard into a whole-tree one.
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "ruined\n").unwrap();
    let transport = connected(root).await;

    let named_root = vec![root.to_string_lossy().into_owned()];
    assert!(surface(&transport).discard(&named_root).await.is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("src/main.rs")).unwrap(),
        "ruined\n",
        "the discard must not have run"
    );
}

#[tokio::test]
async fn commit_is_refused_outside_a_repository() {
    if !git_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let transport = connected(dir.path()).await;
    assert!(surface(&transport)
        .commit(CommitRequest::new("nowhere to put this"))
        .await
        .is_err());
}

#[tokio::test]
async fn a_path_spelled_differently_but_naming_the_same_file_is_accepted() {
    if !git_available() {
        return;
    }
    // `connect` canonicalises the session root, so the guard compares against
    // the canonical form. A caller can legitimately hold another spelling of
    // the same file - a Windows 8.3 short name (`RUNNER~1`), a symlinked temp
    // directory on macOS, a `.` segment - and refusing those would be refusing
    // a path the session plainly owns.
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    let transport = connected(root).await;

    let spelled_oddly = root.join("src/./main.rs").to_string_lossy().into_owned();
    surface(&transport)
        .stage(std::slice::from_ref(&spelled_oddly))
        .await
        .unwrap();

    let status = surface(&transport).status().await.unwrap();
    let entry = status
        .entries
        .iter()
        .find(|e| e.relative_path == "src/main.rs")
        .unwrap();
    assert_eq!(entry.index, mino_core::types::GitFileState::Modified);
}
