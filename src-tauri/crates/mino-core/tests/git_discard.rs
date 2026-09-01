//! Discarding working-tree changes, against real repositories.
//!
//! The one operation on this interface that destroys data, so it gets its own
//! file. Two things are asserted about every case: that the change it was
//! aimed at is gone, and that **nothing else moved** - an over-broad discard
//! is not a bug found later, it is work somebody has lost.
//!
//! What discard refuses to do is in `git_mutate_guards.rs`.

use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn discard_restores_a_modified_file_and_removes_nothing_else() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "fn main() { ruined }\n").unwrap();
    std::fs::write(root.join("keep-me.txt"), "untracked\n").unwrap();
    let transport = connected(root).await;
    let path = root.join("src/main.rs").to_string_lossy().into_owned();

    surface(&transport).discard(&[path]).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(root.join("src/main.rs")).unwrap(),
        "fn main() {}\n"
    );
    // The untracked file is still there. `git restore` does not delete files
    // git has never seen, and this app does not offer to either.
    assert!(root.join("keep-me.txt").exists());
    assert!(root.join("readme.md").exists());
}

#[tokio::test]
async fn discarding_everything_restores_every_tracked_change() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("src/main.rs"), "ruined\n").unwrap();
    std::fs::write(root.join("readme.md"), "ruined\n").unwrap();
    let transport = connected(root).await;

    surface(&transport).discard(&[]).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("src/main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("readme.md")).unwrap(),
        "# test\n"
    );
}
