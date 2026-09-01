//! Remote file reading, with the same two guards the local transport applies.
//!
//! The size ceiling matters more here than locally: without it an accidental
//! click on a multi-gigabyte file would pull it across the network before
//! anything could refuse it. So the size is taken from `stat` first, and the
//! transfer only starts once it is known to be under the limit.
//!
//! The binary sniff is shared with the local transport rather than reimplemented
//! - see [`crate::local::looks_binary`].

use base64::Engine;
use russh_sftp::client::SftpSession;

use crate::error::{Result, TransportError};
use crate::local::looks_binary;
use crate::types::{FileEncoding, FilePayload, ReadFileOptions};

use super::fs::{map_error, resolve};
use super::roots::RemoteRoot;

pub async fn read_file(
    sftp: &SftpSession,
    root: &RemoteRoot,
    path: &str,
    options: ReadFileOptions,
) -> Result<FilePayload> {
    let resolved = resolve(sftp, root, path).await?;

    let meta = sftp
        .metadata(resolved.clone())
        .await
        .map_err(|e| map_error(path, e))?;

    if meta.is_dir() {
        return Err(TransportError::invalid(format!("{path} is a directory")));
    }

    let limit = options.limit();
    let size = meta.size.unwrap_or(0);
    // Checked before the transfer, so an oversized file never crosses the wire.
    if size > limit {
        return Err(TransportError::TooLarge {
            path: resolved,
            size,
            limit,
        });
    }

    let bytes = sftp
        .read(resolved.clone())
        .await
        .map_err(|e| map_error(path, e))?;

    // The server is free to disagree with its own stat; a file that grew
    // between the two calls is still refused rather than delivered.
    let actual = bytes.len() as u64;
    if actual > limit {
        return Err(TransportError::TooLarge {
            path: resolved,
            size: actual,
            limit,
        });
    }

    let extension = extension_of(&resolved);
    // SFTP reports seconds; the rest of the app works in milliseconds.
    let modified_ms = meta.mtime.map(|s| u64::from(s) * 1000);

    if looks_binary(&bytes) {
        if !options.allow_binary {
            return Err(TransportError::BinaryFile {
                path: resolved,
                size: actual,
            });
        }
        return Ok(FilePayload {
            path: resolved,
            size: actual,
            modified_ms,
            encoding: FileEncoding::Base64,
            content: base64::engine::general_purpose::STANDARD.encode(&bytes),
            extension,
        });
    }

    Ok(FilePayload {
        path: resolved,
        size: actual,
        modified_ms,
        encoding: FileEncoding::Utf8,
        content: String::from_utf8_lossy(&bytes).into_owned(),
        extension,
    })
}

/// Lower-cased extension of the last segment, or `None`. A dot-file with no
/// other dot (`.bashrc`) has no extension, which is what the viewer expects.
fn extension_of(path: &str) -> Option<String> {
    let name = super::roots::base_name(path);
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext.to_lowercase())
}
