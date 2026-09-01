//! What the git surface refuses to do, and what it does when git cannot help.
//!
//! The behaviour of `repository` and `status` is in `git_status.rs`; the
//! limits are here - the path guard, and search's degrade path. These are the
//! ones where a regression is a leak or a lost feature rather than a wrong
//! badge.
//!
//! Like its sibling, every test returns early when git is absent.

use mino_core::types::SearchQuery;
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn paths_outside_the_connected_root_never_appear() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    std::fs::write(root.join("readme.md"), "# changed\n").unwrap();

    // Connected *below* the repository root, which is an ordinary thing to
    // do. Git still answers for the whole tree; the session must not.
    let transport = connected(&root.join("src")).await;
    let status = surface(&transport).status().await.unwrap();

    let paths: Vec<&str> = status
        .entries
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect();
    assert_eq!(paths, vec!["src/main.rs"]);
    // The repository still reports its own root, which sits above the session
    // root. That is what the separate field is for.
    assert!(root.ends_with(
        std::path::Path::new(&status.repository.root)
            .file_name()
            .unwrap()
    ));
}

#[tokio::test]
async fn search_skips_what_git_ignores_but_still_finds_everything_else() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join(".gitignore"), "generated/\n").unwrap();
    std::fs::create_dir_all(root.join("generated")).unwrap();
    std::fs::write(root.join("generated/main.rs"), "x").unwrap();
    std::fs::write(root.join("src/handler.rs"), "x").unwrap();

    let transport = connected(root).await;
    let hits = transport
        .search_files(SearchQuery::new("main.rs"))
        .await
        .unwrap();
    let found: Vec<&str> = hits
        .hits
        .iter()
        .map(|hit| hit.relative_path.as_str())
        .collect();

    assert!(found.contains(&"src/main.rs"), "{found:?}");
    assert!(
        !found.iter().any(|path| path.starts_with("generated/")),
        "an ignored directory should not be searched: {found:?}"
    );
}

#[tokio::test]
async fn search_outside_a_repository_is_unchanged() {
    if !git_available() {
        return;
    }
    // The degrade path, and the one that would be a regression if it broke:
    // a folder with no repository must search exactly as it did before git
    // existed in this app.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("generated")).unwrap();
    std::fs::write(dir.path().join("generated/main.rs"), "x").unwrap();

    let transport = connected(dir.path()).await;
    let hits = transport
        .search_files(SearchQuery::new("main.rs"))
        .await
        .unwrap();
    assert_eq!(hits.hits.len(), 1);
    assert_eq!(hits.hits[0].relative_path, "generated/main.rs");
}
