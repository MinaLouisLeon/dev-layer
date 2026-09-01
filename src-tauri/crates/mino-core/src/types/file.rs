use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 2 MiB. Above this the viewer refuses to load and shows the size notice
/// instead; the ceiling is enforced in the transport, not in the UI, so no
/// caller can bypass it.
pub const DEFAULT_READ_LIMIT_BYTES: u64 = 2 * 1024 * 1024;

// `Default` is derived: no ceiling override (so `limit()` falls back to
// DEFAULT_READ_LIMIT_BYTES) and binary files refused, which is what every
// caller but a future image previewer wants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct ReadFileOptions {
    /// Hard ceiling in bytes. `None` means `DEFAULT_READ_LIMIT_BYTES`.
    #[ts(type = "number | null")]
    pub max_bytes: Option<u64>,
    /// When true, a file sniffed as binary is returned base64-encoded instead
    /// of raising `BinaryFile`. The viewer never sets this; it exists for
    /// future previewers (images) that can render bytes.
    pub allow_binary: bool,
}

impl ReadFileOptions {
    pub fn limit(&self) -> u64 {
        self.max_bytes.unwrap_or(DEFAULT_READ_LIMIT_BYTES)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum FileEncoding {
    Utf8,
    Base64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct FilePayload {
    pub path: String,
    #[ts(type = "number")]
    pub size: u64,
    /// Modification time when this content was read. The editor sends it back
    /// on save so a file changed underneath is refused rather than clobbered.
    #[ts(type = "number | null")]
    pub modified_ms: Option<u64>,
    pub encoding: FileEncoding,
    pub content: String,
    /// Lower-case extension without the dot, used to pick the CodeMirror
    /// language. `None` for extensionless files.
    pub extension: Option<String>,
}

/// 8 MiB. A write ceiling as well as a read one: the editor should refuse a
/// pathological buffer rather than stream it at a remote host. Higher than the
/// read limit because a file that was opened is already under that.
pub const DEFAULT_WRITE_LIMIT_BYTES: u64 = 8 * 1024 * 1024;

/// A request to save a file.
///
/// `expected_modified_ms` is the lost-update guard. The editor sends the
/// modification time it loaded, and the transport refuses the write if the
/// file has changed since - which is what stops a save from silently
/// discarding an edit made in another program. `None` means "do not check",
/// used only when creating a file that did not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct WriteRequest {
    /// UTF-8 text. Binary editing is deliberately not offered: the viewer
    /// refuses to open binary files, so there is nothing to edit.
    pub content: String,
    #[ts(type = "number | null")]
    pub expected_modified_ms: Option<u64>,
}

impl WriteRequest {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            expected_modified_ms: None,
        }
    }

    pub fn expecting(mut self, modified_ms: Option<u64>) -> Self {
        self.expected_modified_ms = modified_ms;
        self
    }

    /// Size the write would put on disk, for the ceiling check.
    pub fn byte_len(&self) -> u64 {
        self.content.len() as u64
    }
}
