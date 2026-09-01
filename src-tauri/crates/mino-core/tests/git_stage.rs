//! Staging and unstaging, against real repositories.
//!
//! The argv has unit tests of its own; what is asserted here is that the
//! commands actually move an entry from one side of the index to the other,
//! which is the only way to know that the argv, the exit codes and the git
//! that is installed all agree.
//!
//! Every test returns early when git is absent, like its siblings.

use mino_core::types::GitFileState;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

/// The two sides of one path, or `None` when git says nothing about it.
async fn sides(transport: &LocalTransport, relative: &str) -> Option<(GitFileState, GitFileState)> {
    let status = surface(transport).status().await.unwrap();
    status
        .entries
        .iter()
        .find(|entry| entry.relative_path == relative)
        .map(|entry| (entry.index, entry.worktree))
}

#[tokio::test]
async fn stage_moves_an_entry_to_the_index_and_unstage_moves_it_back() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    let transport = connected(root).await;
    let path = root.join("src/main.rs").to_string_lossy().into_owned();

    assert_eq!(
        sides(&transport, "src/main.rs").await,
        Some((GitFileState::Unmodified, GitFileState::Modified))
    );

    surface(&transport)
        .stage(std::slice::from_ref(&path))
        .await
        .unwrap();
    assert_eq!(
        sides(&transport, "src/main.rs").await,
        Some((GitFileState::Modified, GitFileState::Unmodified))
    );

    surface(&transport).unstage(&[path]).await.unwrap();
    assert_eq!(
        sides(&transport, "src/main.rs").await,
        Some((GitFileState::Unmodified, GitFileState::Modified))
    );
}

#[tokio::test]
async fn unstage_works_on_an_unborn_branch() {
    if !git_available() {
        return;
    }
    // The case `git restore --staged` cannot serve: there is no HEAD to
    // restore from, and this is exactly when someone stages something and
    // changes their mind.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "--initial-branch=main"]);
    std::fs::write(root.join("first.txt"), "x\n").unwrap();
    let transport = connected(root).await;
    let path = root.join("first.txt").to_string_lossy().into_owned();

    surface(&transport)
        .stage(std::slice::from_ref(&path))
        .await
        .unwrap();
    assert_eq!(
        sides(&transport, "first.txt").await.map(|s| s.0),
        Some(GitFileState::Added)
    );

    surface(&transport).unstage(&[path]).await.unwrap();
    assert_eq!(
        sides(&transport, "first.txt").await.map(|s| s.0),
        Some(GitFileState::Untracked)
    );
}

#[tokio::test]
async fn staging_an_empty_slice_stages_everything() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    std::fs::write(root.join("added.txt"), "new\n").unwrap();
    let transport = connected(root).await;

    surface(&transport).stage(&[]).await.unwrap();
    let status = surface(&transport).status().await.unwrap();
    // Untracked files included: `git add --all` is what the group control
    // means by "everything".
    for entry in &status.entries {
        assert_eq!(
            entry.worktree,
            GitFileState::Unmodified,
            "{} should be fully staged",
            entry.relative_path
        );
    }
    assert!(status
        .entries
        .iter()
        .any(|e| e.relative_path == "added.txt"));
}
