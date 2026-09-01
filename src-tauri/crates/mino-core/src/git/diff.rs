//! The unified-diff parser.
//!
//! Here, once, rather than in the UI. A renderer that read a patch would be a
//! second implementation of git's format, and with two transports eventually
//! two disagreeing ones - the same reason [`crate::search::fuzzy`] holds the
//! ranking and [`crate::git::porcelain`] holds the status format.
//!
//! The shape of one file's entry, with everything optional actually optional:
//!
//! ```text
//! diff --git a/before.rs b/after.rs   always first; starts a new file
//! similarity index 100%              \
//! rename from before.rs               |  extended headers, any subset
//! rename to after.rs                  |  (a pure rename stops here - it has
//! new file mode 100644                |   no ---/+++ lines and no hunks)
//! index b2f931a..820620b 100644       /
//! --- a/before.rs                     the old side, or /dev/null
//! +++ b/after.rs                      the new side, or /dev/null
//! @@ -1,5 +1,5 @@ fn main()           a hunk, then its lines
//!  context
//! -removed
//! +added
//! \ No newline at end of file         belongs to the line above it
//! ```
//!
//! Line numbers are worked out here, from the hunk header, so the UI never
//! counts lines itself.

mod header;
mod hunk;
mod path;

use crate::types::{GitDiff, GitFileDiff, GitHunk, MAX_DIFF_LINES};

use header::absorb_header;
use hunk::{absorb_line, start_hunk};

/// Parses one `git diff` patch.
///
/// Never fails. Git's own output is the input, and a shape this does not
/// recognise costs one skipped line rather than the whole diff: a viewer that
/// showed nothing because of one unexpected header would be worse than one
/// that showed the rest.
pub fn parse(patch: &str) -> GitDiff {
    let mut files: Vec<GitFileDiff> = Vec::new();
    let mut current: Option<GitFileDiff> = None;
    let mut hunk: Option<GitHunk> = None;
    let mut old_line = 0u32;
    let mut new_line = 0u32;
    let mut counted = 0u32;
    let mut truncated = false;

    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush(&mut files, &mut current, &mut hunk);
            let mut file = empty_file();
            // A provisional name, overwritten by `+++` or `rename to` when one
            // arrives. It is the *only* name a binary file or a mode-only
            // change ever gets: neither has `---`/`+++` lines, and without
            // this such a file would be dropped as nameless.
            if let Some(found) = path::from_pair(rest) {
                file.relative_path = found;
            }
            current = Some(file);
            continue;
        }
        let Some(file) = current.as_mut() else {
            // Anything before the first `diff --git` is not ours.
            continue;
        };

        if let Some(rest) = line.strip_prefix("@@ ") {
            close_hunk(file, &mut hunk);
            if let Some(started) = start_hunk(rest, &mut old_line, &mut new_line) {
                hunk = Some(started);
            }
            continue;
        }

        if hunk.is_some() {
            if counted >= MAX_DIFF_LINES {
                truncated = true;
                break;
            }
            counted += 1;
            absorb_line(line, hunk.as_mut(), &mut old_line, &mut new_line);
            continue;
        }

        absorb_header(line, file);
    }

    flush(&mut files, &mut current, &mut hunk);
    GitDiff { files, truncated }
}

fn empty_file() -> GitFileDiff {
    GitFileDiff {
        relative_path: String::new(),
        old_path: None,
        binary: false,
        hunks: Vec::new(),
    }
}

fn close_hunk(file: &mut GitFileDiff, hunk: &mut Option<GitHunk>) {
    if let Some(done) = hunk.take() {
        file.hunks.push(done);
    }
}

fn flush(
    files: &mut Vec<GitFileDiff>,
    current: &mut Option<GitFileDiff>,
    hunk: &mut Option<GitHunk>,
) {
    if let Some(mut file) = current.take() {
        close_hunk(&mut file, hunk);
        // A file with no name is a header shape this parser did not recognise.
        // Dropping it is better than showing a nameless entry.
        if !file.relative_path.is_empty() {
            files.push(file);
        }
    }
    *hunk = None;
}

#[cfg(test)]
mod tests;
