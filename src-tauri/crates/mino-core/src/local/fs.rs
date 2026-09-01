//! Plain filesystem listing and stat.
//!
//! This is the degrade path for the tree: it runs when `nu` is unavailable or
//! when the structured Nushell channel fails for any reason, and it is the
//! only path that produces typed `NotFound`/`PermissionDenied` errors.

use std::fs::Metadata;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::error::{Result, TransportError};
use crate::types::{DirEntry, EntryKind};

use super::roots::{display_path, RootGuard};

pub fn list_dir(guard: &RootGuard, path: &str) -> Result<Vec<DirEntry>> {
    let dir = guard.resolve(path)?;
    let reader = std::fs::read_dir(&dir).map_err(|e| TransportError::from_io(path, e))?;

    let mut entries = Vec::new();
    for item in reader {
        let item = match item {
            Ok(item) => item,
            // A single unreadable child must not fail the whole listing.
            Err(err) => {
                tracing::debug!(%err, "skipping unreadable directory entry");
                continue;
            }
        };
        let child = item.path();
        // symlink_metadata so a link is reported as a link and never followed
        // during a listing; following happens only when the user selects it,
        // and then the guard re-checks containment.
        let meta = match std::fs::symlink_metadata(&child) {
            Ok(meta) => meta,
            Err(err) => {
                tracing::debug!(%err, "skipping entry without metadata");
                continue;
            }
        };
        entries.push(entry_from(&child, &meta));
    }

    sort_entries(&mut entries);
    Ok(entries)
}

pub fn stat(guard: &RootGuard, path: &str) -> Result<DirEntry> {
    let resolved = guard.resolve(path)?;
    let meta = std::fs::metadata(&resolved).map_err(|e| TransportError::from_io(path, e))?;
    Ok(entry_from(&resolved, &meta))
}

/// Directories first, then case-insensitive by name. Matches what the tree
/// renders, so the UI never re-sorts.
pub fn sort_entries(entries: &mut [DirEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub fn entry_from(path: &Path, meta: &Metadata) -> DirEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| display_path(path));

    DirEntry {
        kind: kind_of(meta),
        size: meta.len(),
        modified_ms: modified_ms(meta),
        readonly: meta.permissions().readonly(),
        hidden: is_hidden(&name, meta),
        path: display_path(path),
        name,
    }
}

fn kind_of(meta: &Metadata) -> EntryKind {
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

/// Shared with the writer, which compares it against what the editor loaded.
pub fn modified_ms(meta: &Metadata) -> Option<u64> {
    meta.modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

#[cfg(windows)]
fn is_hidden(name: &str, meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    name.starts_with('.') || meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _meta: &Metadata) -> bool {
    name.starts_with('.')
}
