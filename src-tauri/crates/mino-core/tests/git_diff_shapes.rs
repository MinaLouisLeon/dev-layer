//! The two diffs that carry no readable lines: a binary file, and a pure
//! rename.
//!
//! Both matter more than they look. Neither has `---`/`+++` header lines, so
//! neither has a name unless the parser takes one from somewhere else - and a
//! diff with no name is one this transport drops.

use mino_core::types::DiffRequest;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn a_binary_file_says_so_and_carries_no_hunks() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("logo.bin"), [0u8, 1, 2, 3, 0, 9]).unwrap();
    git(root, &["add", "logo.bin"]);
    git(root, &["commit", "-m", "add binary"]);
    std::fs::write(root.join("logo.bin"), [0u8, 9, 9, 9, 0, 1]).unwrap();
    let transport = connected(root).await;

    let diff = surface(&transport)
        .diff(DiffRequest::worktree())
        .await
        .unwrap();
    let file = &diff.files[0];
    // Named, even though a binary diff has no ---/+++ lines to name it.
    assert_eq!(file.relative_path, "logo.bin");
    assert!(file.binary);
    assert!(file.hunks.is_empty());
}

#[tokio::test]
async fn a_renamed_file_carries_where_it_came_from() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    git(root, &["mv", "readme.md", "README.md"]);
    let transport = connected(root).await;

    let diff = surface(&transport)
        .diff(DiffRequest::worktree().staged(true))
        .await
        .unwrap();
    let renamed = diff
        .files
        .iter()
        .find(|f| f.old_path.is_some())
        .unwrap_or_else(|| panic!("a rename should be reported: {:?}", diff.files));
    assert_eq!(renamed.old_path.as_deref(), Some("readme.md"));
    assert_eq!(renamed.relative_path, "README.md");
}
