//! The search fixture, shared by `local_search.rs` and
//! `local_search_guards.rs` so the two suites cannot drift onto different
//! trees and then disagree about what the walk should have found.
//!
//! `allow(dead_code)` for the whole module: Cargo compiles it into every
//! integration test binary that declares it, and no single suite uses all of
//! it. The alternative - one fixture file per suite - is how the drift this
//! module exists to prevent starts.
#![allow(dead_code)]

use mino_core::types::ConnectionTarget;
use mino_core::{LocalTransport, Transport};

/// Real git repositories, for `git_status.rs`.
pub mod git;

/// A repository with a local bare remote, for `git_remote.rs`.
pub mod git_remote;

/// A repository stopped mid-merge, for `git_conflicts.rs`.
pub mod conflicted;

/// The whole git surface asserted `Unimplemented`, for
/// `unimplemented_transports.rs`.
pub mod unbuilt_git;

/// The same for the remote, conflict and GitHub surfaces.
pub mod unbuilt_remote;

/// A tree with a nested match, a decoy at the top level, a hidden file, and a
/// file buried inside a directory the walk must skip.
pub fn fixture() -> (tempfile::TempDir, LocalTransport) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();

    std::fs::create_dir_all(root.join("src/features/file-tree")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(root.join("src/features/file-tree/TreePane.tsx"), "x").unwrap();
    std::fs::write(root.join("readme.md"), "x").unwrap();
    std::fs::write(root.join(".hidden.toml"), "x").unwrap();
    // The one that must never be found: same name, inside the skip list.
    std::fs::write(root.join("node_modules/react/main.rs"), "x").unwrap();

    (dir, LocalTransport::new())
}

pub async fn connected() -> (tempfile::TempDir, LocalTransport) {
    let (dir, transport) = fixture();
    let target = ConnectionTarget::Local {
        root: dir.path().to_string_lossy().into_owned(),
    };
    transport.connect(&target).await.expect("connect");
    (dir, transport)
}
