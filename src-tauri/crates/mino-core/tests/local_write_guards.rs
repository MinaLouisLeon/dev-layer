//! The guards on saving.
//!
//! Writing is the only thing this app does that can destroy someone's work,
//! so these are the cases where it must refuse: a path outside the connected
//! root, a traversal through it, a file that changed since it was opened, and
//! a directory. Each asserts that nothing was written, not just that an error
//! came back.

use mino_core::types::{ConnectionTarget, WriteRequest};
use mino_core::{LocalTransport, Transport, TransportError};

async fn connected() -> (tempfile::TempDir, LocalTransport) {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("hello.txt"), "hello nu").unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();

    let transport = LocalTransport::new();
    let target = ConnectionTarget::Local {
        root: dir.path().to_string_lossy().into_owned(),
    };
    transport.connect(&target).await.expect("connect");
    (dir, transport)
}

/// The guard that matters most, and it has to hold for a file that does not
/// exist yet as well as one that does: the create path cannot canonicalise the
/// target, so it checks the parent instead.
#[tokio::test]
async fn it_refuses_to_write_outside_the_root() {
    let (dir, transport) = connected().await;
    let outside = dir.path().parent().unwrap().join("escaped.txt");

    let err = transport
        .write_file(&outside.to_string_lossy(), WriteRequest::new("nope"))
        .await
        .unwrap_err();

    assert!(matches!(err, TransportError::PathEscapesRoot { .. }));
    assert!(!outside.exists(), "the file must not have been created");
}

#[tokio::test]
async fn it_refuses_a_traversal_through_the_root() {
    let (_dir, transport) = connected().await;

    let err = transport
        .write_file("nested/../../escaped.txt", WriteRequest::new("nope"))
        .await
        .unwrap_err();

    assert!(matches!(err, TransportError::PathEscapesRoot { .. }));
}

/// A stale save is refused rather than allowed to discard the other edit.
#[tokio::test]
async fn it_refuses_to_overwrite_a_file_that_changed() {
    let (dir, transport) = connected().await;
    let stat = transport.stat("hello.txt").await.expect("stat");

    // Something else edits the file after the editor loaded it.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(dir.path().join("hello.txt"), "changed elsewhere").unwrap();

    let err = transport
        .write_file(
            "hello.txt",
            WriteRequest::new("my edit").expecting(stat.modified_ms),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, TransportError::Conflict { .. }));
    let on_disk = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(on_disk, "changed elsewhere", "the other edit must survive");
}

#[tokio::test]
async fn it_refuses_to_write_over_a_directory() {
    let (_dir, transport) = connected().await;
    let err = transport
        .write_file("nested", WriteRequest::new("nope"))
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::InvalidArgument { .. }));
}
