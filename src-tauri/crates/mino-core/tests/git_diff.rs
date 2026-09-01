//! Diffs, against real repositories.
//!
//! The parser has unit tests against recorded output. What is asserted here is
//! the half those cannot reach: that the argv asks git the question we think
//! it does, and that the answer lines up with the file on disk.

use mino_core::types::{DiffRequest, GitDiffLineKind};
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn a_modified_file_produces_the_hunks_git_reports() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    println!();\n}\n",
    )
    .unwrap();
    let transport = connected(root).await;

    let diff = surface(&transport)
        .diff(DiffRequest::worktree())
        .await
        .unwrap();

    assert_eq!(diff.files.len(), 1);
    let file = &diff.files[0];
    assert_eq!(file.relative_path, "src/main.rs");
    assert!(!file.binary);
    assert!(!diff.truncated);

    let added: Vec<&str> = file.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind == GitDiffLineKind::Added)
        .map(|l| l.content.as_str())
        .collect();
    assert!(added.contains(&"    println!();"), "{added:?}");
    // Every added line knows where it landed and has no old side.
    for line in file.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind == GitDiffLineKind::Added)
    {
        assert!(line.new_line.is_some());
        assert_eq!(line.old_line, None);
    }
}

#[tokio::test]
async fn staged_and_unstaged_diffs_differ_after_a_partial_stage() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { /* staged */ }\n").unwrap();
    git(root, &["add", "src/main.rs"]);
    std::fs::write(root.join("src/main.rs"), "fn main() { /* and again */ }\n").unwrap();
    let transport = connected(root).await;

    let staged = surface(&transport)
        .diff(DiffRequest::worktree().staged(true))
        .await
        .unwrap();
    let worktree = surface(&transport)
        .diff(DiffRequest::worktree())
        .await
        .unwrap();

    let text = |diff: &mino_core::types::GitDiff| {
        diff.files[0].hunks[0]
            .lines
            .iter()
            .filter(|l| l.kind == GitDiffLineKind::Added)
            .map(|l| l.content.clone())
            .collect::<Vec<_>>()
            .join("")
    };
    assert!(text(&staged).contains("staged"), "{:?}", text(&staged));
    assert!(
        text(&worktree).contains("and again"),
        "{:?}",
        text(&worktree)
    );
}

#[tokio::test]
async fn one_path_narrows_the_diff_to_that_file() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    std::fs::write(root.join("readme.md"), "# changed\n").unwrap();
    let transport = connected(root).await;

    let all = surface(&transport)
        .diff(DiffRequest::worktree())
        .await
        .unwrap();
    assert_eq!(all.files.len(), 2);

    let one = surface(&transport)
        .diff(DiffRequest::worktree().path(root.join("readme.md").to_string_lossy().into_owned()))
        .await
        .unwrap();
    assert_eq!(one.files.len(), 1);
    assert_eq!(one.files[0].relative_path, "readme.md");
}

#[tokio::test]
async fn a_clean_tree_diffs_to_nothing() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let transport = connected(dir.path()).await;
    let diff = surface(&transport)
        .diff(DiffRequest::worktree())
        .await
        .unwrap();
    assert!(diff.is_empty());
}
