//! Saving a file, remotely.

use russh_sftp::client::SftpSession;

use crate::error::{Result, TransportError};
use crate::types::{DirEntry, WriteRequest, DEFAULT_WRITE_LIMIT_BYTES};

use super::fs::{entry_for, map_error, resolve};
use super::roots::{normalise, RemoteRoot};

/// Saving a file, remotely.
///
/// The same three guards as the local writer, with one deliberate difference:
/// the write is not staged and renamed. SFTP `rename` is not required to
/// replace an existing file - servers without the `posix-rename` extension
/// fail - so staging would break saving on some hosts. A direct write is the
/// portable behaviour, and the cost is that a connection dropped mid-save can
/// leave a truncated file.
pub async fn write_file(
    sftp: &SftpSession,
    root: &RemoteRoot,
    path: &str,
    request: WriteRequest,
) -> Result<DirEntry> {
    if request.byte_len() > DEFAULT_WRITE_LIMIT_BYTES {
        return Err(TransportError::TooLarge {
            path: path.to_string(),
            size: request.byte_len(),
            limit: DEFAULT_WRITE_LIMIT_BYTES,
        });
    }

    let resolved = resolve_for_write(sftp, root, path).await?;

    // Only an existing file can have been changed underneath the editor.
    if let Ok(meta) = sftp.metadata(resolved.clone()).await {
        if meta.is_dir() {
            return Err(TransportError::invalid(format!("{path} is a directory")));
        }
        if let Some(expected) = request.expected_modified_ms {
            let actual = meta.mtime.map(|s| u64::from(s) * 1000);
            // SFTP reports seconds, so a sub-second change is invisible here.
            // Comparing at that resolution is what the server offers.
            if let Some(actual) = actual {
                if actual != expected {
                    return Err(TransportError::conflict(path, Some(expected), Some(actual)));
                }
            }
        }
    }

    sftp.write(resolved.clone(), request.content.as_bytes())
        .await
        .map_err(|e| map_error(path, e))?;

    let meta = sftp
        .metadata(resolved.clone())
        .await
        .map_err(|e| map_error(path, e))?;
    Ok(entry_for(resolved, meta))
}

/// A save may create a file, which cannot be canonicalised because it does not
/// exist yet - so the *parent* is what gets resolved and checked.
async fn resolve_for_write(sftp: &SftpSession, root: &RemoteRoot, path: &str) -> Result<String> {
    if let Ok(existing) = resolve(sftp, root, path).await {
        return Ok(existing);
    }
    let candidate = normalise(&root.candidate(path));
    let (parent, name) = candidate
        .rsplit_once('/')
        .ok_or_else(|| TransportError::invalid(format!("{path} has no parent directory")))?;
    if name.is_empty() {
        return Err(TransportError::invalid(format!(
            "{path} does not name a file"
        )));
    }
    let parent = if parent.is_empty() { "/" } else { parent };
    let canonical_parent = sftp
        .canonicalize(parent.to_string())
        .await
        .map_err(|e| map_error(path, e))?;
    let checked = root.ensure(path, canonical_parent)?;
    Ok(format!("{}/{name}", checked.trim_end_matches('/')))
}
