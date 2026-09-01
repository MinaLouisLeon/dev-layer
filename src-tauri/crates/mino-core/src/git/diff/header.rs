//! The lines between `diff --git` and the first `@@`.
//!
//! Everything that says *what happened to the file* rather than *what changed
//! inside it*: which paths it has, whether it was renamed or copied, and
//! whether git declined to diff it at all.

use crate::types::GitFileDiff;

use super::path;

/// The extended header lines, and the two that name the file.
pub(super) fn absorb_header(line: &str, file: &mut GitFileDiff) {
    if let Some(rest) = line.strip_prefix("--- ") {
        if let Some(found) = path::from_header(rest) {
            // Only a fallback: `+++` is the authoritative name, and this is
            // what a deleted file is left with.
            if file.relative_path.is_empty() {
                file.relative_path = found;
            }
        }
        return;
    }
    if let Some(rest) = line.strip_prefix("+++ ") {
        if let Some(found) = path::from_header(rest) {
            file.relative_path = found;
        }
        return;
    }
    if let Some(rest) = line.strip_prefix("rename from ") {
        file.old_path = path::from_rename(rest);
        return;
    }
    if let Some(rest) = line
        .strip_prefix("rename to ")
        .or_else(|| line.strip_prefix("copy to "))
    {
        if let Some(found) = path::from_rename(rest) {
            file.relative_path = found;
        }
        return;
    }
    if let Some(rest) = line.strip_prefix("copy from ") {
        file.old_path = path::from_rename(rest);
        return;
    }
    // Both spellings: the first is a real diff git declined to show, the
    // second is one it encoded. Neither is readable, and both mean the same
    // thing to a reader.
    if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
        file.binary = true;
    }
}
