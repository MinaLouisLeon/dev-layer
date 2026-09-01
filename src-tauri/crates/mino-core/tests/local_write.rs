//! Saving files through the local transport: the cases that should succeed.
//!
//! The refusals - writing outside the root, or over a file that changed
//! underneath - are in `local_write_guards.rs`.

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

#[tokio::test]
async fn writes_before_connect_are_not_connected() {
    let transport = LocalTransport::new();
    let err = transport
        .write_file("hello.txt", WriteRequest::new("x"))
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::NotConnected));
}

#[tokio::test]
async fn it_saves_and_reports_the_new_entry() {
    let (dir, transport) = connected().await;

    let entry = transport
        .write_file("hello.txt", WriteRequest::new("edited"))
        .await
        .expect("write");

    assert_eq!(entry.name, "hello.txt");
    assert_eq!(entry.size, "edited".len() as u64);
    let on_disk = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(on_disk, "edited");
}

#[tokio::test]
async fn it_creates_a_file_that_did_not_exist() {
    let (dir, transport) = connected().await;

    transport
        .write_file("nested/new.txt", WriteRequest::new("fresh"))
        .await
        .expect("write");

    let on_disk = std::fs::read_to_string(dir.path().join("nested").join("new.txt")).unwrap();
    assert_eq!(on_disk, "fresh");
}

/// The mtime check must not stand in the way of an ordinary save.
#[tokio::test]
async fn it_saves_when_the_file_is_unchanged() {
    let (_dir, transport) = connected().await;
    let stat = transport.stat("hello.txt").await.expect("stat");

    let entry = transport
        .write_file(
            "hello.txt",
            WriteRequest::new("my edit").expecting(stat.modified_ms),
        )
        .await
        .expect("write");

    assert_eq!(entry.size, "my edit".len() as u64);
}

/// A successful save must not leave its staging file behind.
#[tokio::test]
async fn it_leaves_no_staging_file() {
    let (dir, transport) = connected().await;
    transport
        .write_file("hello.txt", WriteRequest::new("edited"))
        .await
        .expect("write");

    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("mino-save"))
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}
