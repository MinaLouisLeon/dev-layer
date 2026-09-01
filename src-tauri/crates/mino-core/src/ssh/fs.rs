//! Directory listing and stat over SFTP.
//!
//! `resolve` is the only place a caller-supplied path becomes a path this
//! module will act on: it canonicalises through the server and then puts the
//! answer past the root guard. Every function here starts with it.

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileAttributes;

use crate::error::{Result, TransportError};
use crate::types::{DirEntry, EntryKind};

use super::roots::{base_name, RemoteRoot};

/// Canonicalise, then prove containment. Never call the SFTP session with a
/// path that has not been through here.
pub async fn resolve(sftp: &SftpSession, root: &RemoteRoot, path: &str) -> Result<String> {
    let candidate = root.candidate(path);
    let canonical = sftp
        .canonicalize(candidate.clone())
        .await
        .map_err(|e| map_error(path, e))?;
    root.ensure(path, canonical)
}

pub async fn list_dir(sftp: &SftpSession, root: &RemoteRoot, path: &str) -> Result<Vec<DirEntry>> {
    let dir = resolve(sftp, root, path).await?;
    let listing = sftp
        .read_dir(dir.clone())
        .await
        .map_err(|e| map_error(path, e))?;

    let mut entries = Vec::new();
    for item in listing {
        let name = item.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let child = if dir.ends_with('/') {
            format!("{dir}{name}")
        } else {
            format!("{dir}/{name}")
        };
        // Defence in depth: the server produced these names, so they are
        // checked against the root exactly like a caller-supplied path.
        if !root.contains(&child) {
            continue;
        }
        entries.push(entry_from(child, name, item.metadata().into()));
    }
    sort_entries(&mut entries);
    Ok(entries)
}

pub async fn stat(sftp: &SftpSession, root: &RemoteRoot, path: &str) -> Result<DirEntry> {
    let resolved = resolve(sftp, root, path).await?;
    let meta = sftp
        .metadata(resolved.clone())
        .await
        .map_err(|e| map_error(path, e))?;
    let name = base_name(&resolved);
    Ok(entry_from(resolved, name, meta.into()))
}

/// Builds an entry from a path and the attributes the server reported. Shared
/// with the writer, which needs the entry as it stands after a save.
pub fn entry_for(path: String, attrs: FileAttributes) -> DirEntry {
    let name = base_name(&path);
    entry_from(path, name, attrs.into())
}

fn entry_from(path: String, name: String, attrs: Attrs) -> DirEntry {
    DirEntry {
        path,
        // A leading dot is what "hidden" means on a POSIX host.
        hidden: name.starts_with('.'),
        name,
        kind: attrs.kind,
        size: attrs.size,
        modified_ms: attrs.modified_ms,
        readonly: attrs.readonly,
    }
}

/// The fields this transport needs out of SFTP's attribute bag, with the
/// absent-value decisions made once.
struct Attrs {
    kind: EntryKind,
    size: u64,
    modified_ms: Option<u64>,
    readonly: bool,
}

impl From<FileAttributes> for Attrs {
    fn from(value: FileAttributes) -> Self {
        // Symlink first: a link to a directory carries both bits, and the tree
        // should say what the entry is rather than what it points at.
        let kind = if value.is_symlink() {
            EntryKind::Symlink
        } else if value.is_dir() {
            EntryKind::Directory
        } else if value.is_regular() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        Self {
            kind,
            size: value.size.unwrap_or(0),
            // SFTP reports seconds; the UI and the local transport both work
            // in epoch milliseconds.
            modified_ms: value.mtime.map(|s| u64::from(s) * 1000),
            // No owner-write bit means read-only to somebody. The UI uses this
            // only to dim the row, so the coarse answer is enough.
            readonly: value.permissions.is_some_and(|p| p & 0o200 == 0),
        }
    }
}

/// Directories first, then case-insensitive by name - the same order the local
/// transport produces, so the tree looks the same over either.
pub fn sort_entries(entries: &mut [DirEntry]) {
    entries.sort_by(|a, b| {
        let by_kind =
            matches!(b.kind, EntryKind::Directory).cmp(&matches!(a.kind, EntryKind::Directory));
        by_kind.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// SFTP status codes carry the same distinctions as `std::io::ErrorKind`, so
/// the UI can branch on not-found vs permission-denied without string matching.
pub fn map_error(path: &str, err: russh_sftp::client::error::Error) -> TransportError {
    use russh_sftp::protocol::StatusCode;
    match &err {
        russh_sftp::client::error::Error::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => TransportError::NotFound {
                path: path.to_string(),
            },
            StatusCode::PermissionDenied => TransportError::PermissionDenied {
                path: path.to_string(),
            },
            _ => TransportError::io(format!("{path}: {}", status.error_message)),
        },
        other => TransportError::io(format!("{path}: {other}")),
    }
}
