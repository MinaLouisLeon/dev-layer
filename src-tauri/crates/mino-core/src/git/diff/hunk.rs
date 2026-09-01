//! One `@@` block: its header, and the lines inside it.
//!
//! Split from the file loop above it so that file is about *which file* a
//! patch is talking about, and this one is about where each line sits.
//!
//! The line numbers are the reason any of this is in Rust. A renderer that
//! counted lines itself would have to know that an added line advances only
//! the new side and a removed one only the old - which is the diff format,
//! reimplemented in a component, in a codebase with two transports.

use crate::types::{GitDiffLine, GitDiffLineKind, GitHunk};

/// `@@ -old,count +new,count @@ optional header`
pub(super) fn start_hunk(rest: &str, old_line: &mut u32, new_line: &mut u32) -> Option<GitHunk> {
    let (ranges, header) = match rest.split_once(" @@") {
        Some((ranges, header)) => (ranges, header.trim_start().to_string()),
        None => (rest.trim_end_matches(" @@"), String::new()),
    };
    let mut parts = ranges.split_whitespace();
    let (old_start, old_lines) = range(parts.next()?.strip_prefix('-')?)?;
    let (new_start, new_lines) = range(parts.next()?.strip_prefix('+')?)?;

    *old_line = old_start;
    *new_line = new_start;
    Some(GitHunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        header,
        lines: Vec::new(),
    })
}

/// `12,5` or, when the count is one, just `12`.
fn range(value: &str) -> Option<(u32, u32)> {
    match value.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((value.parse().ok()?, 1)),
    }
}

pub(super) fn absorb_line(line: &str, hunk: Option<&mut GitHunk>, old: &mut u32, new: &mut u32) {
    let Some(hunk) = hunk else { return };

    // Belongs to the line above it, not to itself. Git emits it after the
    // content line whose file ends without a newline.
    if line.starts_with('\\') {
        if let Some(last) = hunk.lines.last_mut() {
            last.no_newline = true;
        }
        return;
    }

    let (kind, content) = match line.split_at_checked(1) {
        Some(("+", rest)) => (GitDiffLineKind::Added, rest),
        Some(("-", rest)) => (GitDiffLineKind::Removed, rest),
        Some((" ", rest)) => (GitDiffLineKind::Context, rest),
        // An empty line inside a hunk is a context line git wrote without its
        // leading space. Real output does this.
        _ if line.is_empty() => (GitDiffLineKind::Context, ""),
        _ => return,
    };

    let (old_line, new_line) = match kind {
        GitDiffLineKind::Added => {
            let at = *new;
            *new += 1;
            (None, Some(at))
        }
        GitDiffLineKind::Removed => {
            let at = *old;
            *old += 1;
            (Some(at), None)
        }
        GitDiffLineKind::Context => {
            let (o, n) = (*old, *new);
            *old += 1;
            *new += 1;
            (Some(o), Some(n))
        }
    };

    hunk.lines.push(GitDiffLine {
        kind,
        content: content.to_string(),
        old_line,
        new_line,
        no_newline: false,
    });
}

#[cfg(test)]
mod tests;
