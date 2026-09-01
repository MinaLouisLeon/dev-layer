//! Saving a file, locally.
//!
//! The only code in the app that writes to a filesystem, so every guard that
//! matters lives here:
//!
//! - the path goes through the same `RootGuard` as a read, so a save outside
//!   the connected root is refused before a byte moves;
//! - a file that changed since the editor loaded it is refused rather than
//!   overwritten, which is what stops a save from discarding someone else's
//!   edit;
//! - the write is staged in a sibling temp file and renamed over the target,
//!   so a crash mid-save leaves either the old file or the new one, never a
//!   half-written one.

use std::path::{Path, PathBuf};

use crate::error::{Result, TransportError};
use crate::types::{DirEntry, WriteRequest, DEFAULT_WRITE_LIMIT_BYTES};

use super::fs::{entry_from, modified_ms};
use super::roots::RootGuard;

/// Suffix for the staging file. Sits beside the target so the rename stays on
/// one volume, which is what makes it atomic.
const TEMP_SUFFIX: &str = ".mino-save";

pub fn write_file(guard: &RootGuard, path: &str, request: WriteRequest) -> Result<DirEntry> {
    if request.byte_len() > DEFAULT_WRITE_LIMIT_BYTES {
        return Err(TransportError::TooLarge {
            path: path.to_string(),
            size: request.byte_len(),
            limit: DEFAULT_WRITE_LIMIT_BYTES,
        });
    }

    let resolved = resolve_for_write(guard, path)?;
    let existing = std::fs::metadata(&resolved).ok();

    if let Some(meta) = &existing {
        if meta.is_dir() {
            return Err(TransportError::invalid(format!("{path} is a directory")));
        }
        guard_against_lost_update(path, meta, request.expected_modified_ms)?;
    }

    let staged = staging_path(&resolved);
    std::fs::write(&staged, request.content.as_bytes())
        .map_err(|e| TransportError::from_io(path, e))?;
    if let Err(err) = std::fs::rename(&staged, &resolved) {
        // Leaving the staging file behind would litter the user's folder.
        let _ = std::fs::remove_file(&staged);
        return Err(TransportError::from_io(path, err));
    }

    let meta = std::fs::metadata(&resolved).map_err(|e| TransportError::from_io(path, e))?;
    Ok(entry_from(&resolved, &meta))
}

/// A write needs the *parent* to exist and to be inside the root; the file
/// itself may not exist yet, which `RootGuard::resolve` cannot express because
/// it canonicalises.
fn resolve_for_write(guard: &RootGuard, path: &str) -> Result<PathBuf> {
    if let Ok(existing) = guard.resolve(path) {
        return Ok(existing);
    }

    let requested = PathBuf::from(path);
    let joined = if requested.is_absolute() {
        requested
    } else {
        guard.root().join(requested)
    };
    let parent = joined
        .parent()
        .ok_or_else(|| TransportError::invalid(format!("{path} has no parent directory")))?;
    let name = joined
        .file_name()
        .ok_or_else(|| TransportError::invalid(format!("{path} does not name a file")))?;

    // The parent must resolve inside the root. Canonicalising it is what
    // stops `root/../outside/file.txt` from being created.
    let canonical_parent =
        std::fs::canonicalize(parent).map_err(|e| TransportError::from_io(path, e))?;
    if !canonical_parent.starts_with(guard.root()) {
        return Err(TransportError::PathEscapesRoot {
            path: path.to_string(),
        });
    }
    Ok(canonical_parent.join(name))
}

fn guard_against_lost_update(
    path: &str,
    meta: &std::fs::Metadata,
    expected: Option<u64>,
) -> Result<()> {
    // `None` means the caller did not load the file first - a new file - so
    // there is nothing to compare against.
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = modified_ms(meta);
    // A filesystem that withholds mtime cannot support the check; refusing
    // every save there would be worse than allowing it.
    let Some(actual) = actual else {
        return Ok(());
    };
    if actual != expected {
        return Err(TransportError::conflict(path, Some(expected), Some(actual)));
    }
    Ok(())
}

fn staging_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(TEMP_SUFFIX);
    target.with_file_name(name)
}
