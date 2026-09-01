//! A repository with a **local bare repository as its `origin`**.
//!
//! The whole of phase 6's remote testing runs against this. A bare repository
//! on disk is a real git remote in every way that matters here - refs,
//! fast-forwards, non-fast-forward rejections, `--force-with-lease` leases -
//! and it needs no network, no credential and no third-party service. A test
//! that reached github.com would be a test that fails on a train.
#![allow(dead_code)]

use std::path::Path;

use super::git::{git, identify};

/// The pair, already pushed once.
///
/// Returns the directory holding both, plus the path to the working clone.
pub fn with_remote() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let bare = dir.path().join("origin.git");
    let work = dir.path().join("work");

    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "--bare", "--initial-branch=main"]);

    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "--initial-branch=main"]);
    identify(&work);
    std::fs::write(work.join("readme.md"), "# test\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-m", "first"]);
    // A path is a legitimate remote URL, and the one that needs nothing.
    git(&work, &["remote", "add", "origin", &bare.to_string_lossy()]);
    git(&work, &["push", "--set-upstream", "origin", "main"]);

    (dir, work)
}

/// A second clone of the same bare repository, for making the remote move.
///
/// This is how a non-fast-forward rejection and a refused lease are produced
/// without inventing them: somebody else pushes, and the first clone is now
/// behind exactly as it would be in life.
pub fn second_clone(dir: &Path, name: &str) -> std::path::PathBuf {
    let bare = dir.join("origin.git");
    let clone = dir.join(name);
    // `-c` rather than a `config` call afterwards: a clone checks out with
    // whatever the *global* setting is, and on Windows that is
    // `core.autocrlf=true` - which would give this clone CRLFs and make every
    // byte comparison in a test a comparison of platforms.
    git(
        dir,
        &[
            "-c",
            "core.autocrlf=false",
            "clone",
            &bare.to_string_lossy(),
            &clone.to_string_lossy(),
        ],
    );
    identify(&clone);
    clone
}
