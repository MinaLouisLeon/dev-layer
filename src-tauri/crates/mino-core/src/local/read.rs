//! File reading with the size ceiling and the binary sniff.
//!
//! Both guards live here, in the transport, so no caller can bypass them by
//! calling something lower level.

use base64::Engine;

use crate::error::{Result, TransportError};
use crate::types::{FileEncoding, FilePayload, ReadFileOptions};

use super::fs::modified_ms;
use super::roots::{display_path, RootGuard};

/// How many leading bytes the binary sniff inspects.
const SNIFF_BYTES: usize = 8192;

pub fn read_file(guard: &RootGuard, path: &str, options: ReadFileOptions) -> Result<FilePayload> {
    let resolved = guard.resolve(path)?;
    let meta = std::fs::metadata(&resolved).map_err(|e| TransportError::from_io(path, e))?;

    if meta.is_dir() {
        return Err(TransportError::invalid(format!("{path} is a directory")));
    }

    let limit = options.limit();
    let size = meta.len();
    // Checked before the read, so an oversized file is never pulled into
    // memory just to be rejected.
    if size > limit {
        return Err(TransportError::TooLarge {
            path: display_path(&resolved),
            size,
            limit,
        });
    }

    let bytes = std::fs::read(&resolved).map_err(|e| TransportError::from_io(path, e))?;
    let display = display_path(&resolved);
    let extension = resolved
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());

    if looks_binary(&bytes) {
        if !options.allow_binary {
            return Err(TransportError::BinaryFile {
                path: display,
                size,
            });
        }
        return Ok(FilePayload {
            path: display,
            size,
            modified_ms: modified_ms(&meta),
            encoding: FileEncoding::Base64,
            content: base64::engine::general_purpose::STANDARD.encode(&bytes),
            extension,
        });
    }

    // looks_binary already rejected invalid UTF-8, so this cannot lose data.
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(FilePayload {
        path: display,
        size,
        modified_ms: modified_ms(&meta),
        encoding: FileEncoding::Utf8,
        content,
        extension,
    })
}

/// A NUL byte in the first `SNIFF_BYTES`, or content that is not valid UTF-8,
/// counts as binary. Cheap and matches what editors do.
pub fn looks_binary(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(SNIFF_BYTES)];
    if head.contains(&0) {
        return true;
    }
    std::str::from_utf8(bytes).is_err()
}
