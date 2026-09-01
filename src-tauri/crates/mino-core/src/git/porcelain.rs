//! The `--porcelain=v2 -z` parser.
//!
//! This lives here, once, rather than in each transport, for the same reason
//! [`crate::search`] holds the ranking: local and SSH must never end up
//! disagreeing about what git said. They run the command; this decides what it
//! means.
//!
//! `-z` is not a detail. Without it git quotes any path containing a space, a
//! quote or a non-ASCII byte, and the parser would have to unquote C-style
//! escapes correctly to avoid mangling real filenames. With it, a path is
//! whatever sits between two NULs.
//!
//! A rename record is the one shape that spans two NUL-terminated fields: the
//! new path, then the original. That is why this walks an iterator it can pull
//! twice from rather than mapping over records. Decoding a single record is
//! [`record`]'s job; sequencing them is this file's.

use crate::types::{GitEntry, MAX_STATUS_ENTRIES};

use super::branch::BranchHeaders;
use super::paths::PathStyle;

mod record;

pub struct ParsedStatus {
    pub headers: BranchHeaders,
    pub entries: Vec<GitEntry>,
    pub truncated: bool,
}

/// Parses one status into entries whose paths are absolute in `style`, hanging
/// off the work tree `root`.
pub fn parse(output: &str, root: &str, style: PathStyle) -> ParsedStatus {
    let mut headers = BranchHeaders::default();
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut records = output.split('\0').filter(|record| !record.is_empty());

    while let Some(line) = records.next() {
        if line.starts_with("# ") {
            headers.absorb(line);
            continue;
        }
        if entries.len() >= MAX_STATUS_ENTRIES as usize {
            truncated = true;
            break;
        }
        // A rename consumes the following record as its original path, so the
        // pull happens here whether or not the entry is kept - skipping it on
        // a malformed record would leave the loop reading a path as a record.
        let original = line.starts_with("2 ").then(|| records.next()).flatten();
        if let Some(entry) = record::entry_from(line, original, root, style) {
            entries.push(entry);
        }
    }

    ParsedStatus {
        headers,
        entries,
        truncated,
    }
}

/// The ignore rows alone, repository-relative, for the search walk's
/// predicate. A directory that matched a pattern arrives with a trailing
/// slash, which is git telling us it did not look inside.
pub fn parse_ignored(output: &str) -> Vec<String> {
    output
        .split('\0')
        .filter_map(|record| record.strip_prefix("! "))
        .map(|path| path.trim_end_matches('/').to_string())
        .filter(|path| !path.is_empty())
        .collect()
}

#[cfg(test)]
mod tests;
