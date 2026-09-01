//! The canonical Nushell pipelines, and the mapping from their output to
//! domain types.
//!
//! Pipeline text lives in Rust only. Callers pass parameters, never program
//! text, which is what keeps the structured channel injection-free.

use serde_json::Value;

use crate::error::{Result, TransportError};
use crate::types::{DirEntry, EntryKind};

/// Parameter key used by both pipelines. Referenced as `$env.MINO_PATH`.
pub const PARAM_PATH: &str = "PATH";

/// One directory level. `-a` keeps hidden entries; the UI dims them rather
/// than hiding them.
pub const LIST_DIR: &str = "ls -a $env.MINO_PATH | select name type size | to json";

/// Maps `LIST_DIR` output onto `DirEntry`.
///
/// `modified_ms` and `readonly` come back empty here - `ls` is queried for the
/// three columns every Nushell version agrees on. The filesystem degrade path
/// fills both. See the quirks section of docs/mino-workbench/overview.md.
pub fn entries_from_list(value: &Value, dir: &str) -> Result<Vec<DirEntry>> {
    let rows = value.as_array().ok_or_else(|| TransportError::Protocol {
        message: "expected a table from `ls`".to_string(),
    })?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(path) = row.get("name").and_then(Value::as_str) else {
            continue;
        };
        // Defence in depth: nu should only ever return children of `dir`.
        if !path.starts_with(dir) {
            continue;
        }
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
        entries.push(DirEntry {
            kind: kind_from(row.get("type").and_then(Value::as_str)),
            size: row.get("size").and_then(Value::as_u64).unwrap_or(0),
            modified_ms: None,
            readonly: false,
            hidden: name.starts_with('.'),
            path: path.to_string(),
            name,
        });
    }
    Ok(entries)
}

fn kind_from(raw: Option<&str>) -> EntryKind {
    match raw {
        Some("dir") => EntryKind::Directory,
        Some("file") => EntryKind::File,
        Some("symlink") => EntryKind::Symlink,
        _ => EntryKind::Other,
    }
}
