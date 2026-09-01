//! The path guard.
//!
//! Every local filesystem call resolves its path through a `RootGuard` first.
//! A path that canonicalises outside the connected root is rejected with
//! `PathEscapesRoot` before any syscall touches it, which also covers `..`
//! sequences and symlinks pointing out of the tree.

use std::path::{Path, PathBuf};

use crate::error::{Result, TransportError};

#[derive(Debug, Clone)]
pub struct RootGuard {
    root: PathBuf,
}

impl RootGuard {
    pub fn new(root: &str) -> Result<Self> {
        let path = PathBuf::from(root);
        let canonical =
            std::fs::canonicalize(&path).map_err(|e| TransportError::from_io(root, e))?;
        if !canonical.is_dir() {
            return Err(TransportError::invalid(format!(
                "{root} is not a directory"
            )));
        }
        Ok(Self { root: canonical })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn root_display(&self) -> String {
        display_path(&self.root)
    }

    /// Canonicalises `path` and proves it sits inside the root. The returned
    /// buffer is the only path callers may hand to the filesystem.
    pub fn resolve(&self, path: &str) -> Result<PathBuf> {
        let requested = PathBuf::from(path);
        let joined = if requested.is_absolute() {
            requested
        } else {
            self.root.join(requested)
        };
        let canonical =
            std::fs::canonicalize(&joined).map_err(|e| TransportError::from_io(path, e))?;
        if !canonical.starts_with(&self.root) {
            return Err(TransportError::PathEscapesRoot {
                path: path.to_string(),
            });
        }
        Ok(canonical)
    }
}

/// Windows canonicalisation returns the `\\?\` extended-length form, which is
/// correct for syscalls and wrong for a breadcrumb. Everything the UI sees
/// goes through here.
pub fn display_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    match raw.strip_prefix(r"\\?\") {
        Some(stripped) => stripped.to_string(),
        None => raw.into_owned(),
    }
}
