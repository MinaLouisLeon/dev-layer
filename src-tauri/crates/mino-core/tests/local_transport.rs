//! Local transport behaviour, including both guards and the path guard.
//!
//! Rust tests live beside their crate per Cargo convention; the root `test/`
//! folder rule governs the TypeScript side.

use mino_core::types::{ConnectionTarget, EntryKind, ReadFileOptions};
use mino_core::{LocalTransport, Transport, TransportError};

fn fixture() -> (tempfile::TempDir, LocalTransport) {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("hello.txt"), "hello nu").unwrap();
    std::fs::write(dir.path().join("binary.bin"), [0u8, 1, 2, 3]).unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    (dir, LocalTransport::new())
}

async fn connected() -> (tempfile::TempDir, LocalTransport, String) {
    let (dir, transport) = fixture();
    let target = ConnectionTarget::Local {
        root: dir.path().to_string_lossy().into_owned(),
    };
    let info = transport.connect(&target).await.expect("connect");
    (dir, transport, info.root)
}

#[tokio::test]
async fn calls_before_connect_are_typed_not_connected() {
    let transport = LocalTransport::new();
    let err = transport.list_dir(".").await.unwrap_err();
    assert!(matches!(err, TransportError::NotConnected));
}

#[tokio::test]
async fn lists_one_level_with_directories_first() {
    let (_dir, transport, root) = connected().await;
    let entries = transport.list_dir(&root).await.expect("listing");
    assert_eq!(entries.first().map(|e| e.kind), Some(EntryKind::Directory));
    assert!(entries.iter().any(|e| e.name == "hello.txt"));
}

#[tokio::test]
async fn reads_a_utf8_file() {
    let (dir, transport, _root) = connected().await;
    let path = dir.path().join("hello.txt").to_string_lossy().into_owned();
    let payload = transport
        .read_file(&path, ReadFileOptions::default())
        .await
        .expect("read");
    assert_eq!(payload.content, "hello nu");
    assert_eq!(payload.extension.as_deref(), Some("txt"));
}

#[tokio::test]
async fn refuses_a_binary_file() {
    let (dir, transport, _root) = connected().await;
    let path = dir.path().join("binary.bin").to_string_lossy().into_owned();
    let err = transport
        .read_file(&path, ReadFileOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::BinaryFile { .. }));
}

#[tokio::test]
async fn refuses_a_file_above_the_ceiling() {
    let (dir, transport, _root) = connected().await;
    let path = dir.path().join("hello.txt").to_string_lossy().into_owned();
    let options = ReadFileOptions {
        max_bytes: Some(2),
        allow_binary: false,
    };
    let err = transport.read_file(&path, options).await.unwrap_err();
    assert!(matches!(err, TransportError::TooLarge { limit: 2, .. }));
}

#[tokio::test]
async fn refuses_a_path_outside_the_root() {
    let (_dir, transport, root) = connected().await;
    let escape = format!("{root}/..");
    let err = transport.list_dir(&escape).await.unwrap_err();
    assert!(matches!(err, TransportError::PathEscapesRoot { .. }));
}

#[tokio::test]
async fn missing_paths_are_typed_not_found() {
    let (_dir, transport, root) = connected().await;
    let missing = format!("{root}/does-not-exist.txt");
    let err = transport
        .read_file(&missing, ReadFileOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::NotFound { .. }));
}

#[tokio::test]
async fn structured_pipelines_must_end_in_to_json() {
    let (_dir, transport, _root) = connected().await;
    if mino_core::shell::find_nu().is_none() {
        // Nushell is not installed on this machine; the guard is covered by
        // the TypeScript contract test instead.
        return;
    }
    let request = mino_core::types::StructuredRequest::new("ls");
    let err = transport.run_structured(request).await.unwrap_err();
    assert!(matches!(err, TransportError::InvalidArgument { .. }));
}
