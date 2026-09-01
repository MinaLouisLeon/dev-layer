//! The `git blame --porcelain` parser.
//!
//! Porcelain reports a commit's details **only the first time that commit
//! appears**, and afterwards gives the sha alone:
//!
//! ```text
//! 6651cee… 1 1 2          <sha> <line in the original> <line in the file> [count]
//! author A Author
//! author-time 1788027873
//! summary first subject
//! filename f.txt
//! \talpha                 the content, always tab-prefixed
//! 6651cee… 2 2            same commit again - no header block this time
//! \tbeta
//! ```
//!
//! That is why `--porcelain` is used rather than `--line-porcelain`, which
//! repeats every header for every line: on a large file the difference is a
//! few kilobytes against a few megabytes. Re-attaching the details per line is
//! this file's job, so the gutter does no lookups and no arithmetic.

use std::collections::HashMap;

use crate::error::{Result, TransportError};
use crate::types::{GitBlame, GitBlameLine, BLAME_SHA_LENGTH, MAX_BLAME_LINES};

use super::GitOutput;

/// What porcelain tells us about one commit, held until its lines arrive.
#[derive(Default, Clone)]
struct Author {
    name: String,
    timestamp_ms: u64,
    summary: String,
}

pub fn parse(output: &GitOutput, relative_path: &str) -> Result<GitBlame> {
    if !output.succeeded() {
        return Err(TransportError::shell(output.message("blame")));
    }

    let mut commits: HashMap<String, Author> = HashMap::new();
    let mut lines: Vec<GitBlameLine> = Vec::new();
    let mut pending: Option<(String, u32)> = None;
    let mut current = Author::default();
    let mut truncated = false;

    for raw in output.stdout.lines() {
        // The content line. It closes whatever header block came before it and
        // is the only thing that produces an entry.
        if let Some(_content) = raw.strip_prefix('\t') {
            let Some((sha, line)) = pending.take() else {
                continue;
            };
            if lines.len() >= MAX_BLAME_LINES as usize {
                truncated = true;
                break;
            }
            // The first sighting of a commit filled `current`; a later one did
            // not, so the map is what answers for it.
            let author = commits
                .entry(sha.clone())
                .or_insert_with(|| current.clone());
            lines.push(GitBlameLine {
                short_sha: sha.chars().take(BLAME_SHA_LENGTH).collect(),
                sha,
                line,
                author: author.name.clone(),
                timestamp_ms: author.timestamp_ms,
                summary: author.summary.clone(),
            });
            continue;
        }

        if let Some(rest) = raw.strip_prefix("author ") {
            current.name = rest.to_string();
            continue;
        }
        if let Some(rest) = raw.strip_prefix("author-time ") {
            current.timestamp_ms = rest
                .trim()
                .parse::<u64>()
                .unwrap_or(0)
                .saturating_mul(1_000);
            continue;
        }
        if let Some(rest) = raw.strip_prefix("summary ") {
            current.summary = rest.to_string();
            continue;
        }
        // Everything else before a content line is either a header this does
        // not need - `author-mail`, `committer`, `boundary`, `previous` - or
        // the `<sha> <orig> <final> [count]` line that opens an entry.
        if let Some(started) = header(raw) {
            pending = Some(started);
            current = Author::default();
        }
    }

    Ok(GitBlame {
        relative_path: relative_path.to_string(),
        lines,
        truncated,
    })
}

/// `<sha> <line in the original> <line in the final file> [<count>]`.
fn header(raw: &str) -> Option<(String, u32)> {
    let mut parts = raw.split(' ');
    let sha = parts.next()?;
    // A sha and nothing else that looks like one: this rules out every other
    // header line without needing to list them.
    if sha.len() < BLAME_SHA_LENGTH || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let _original = parts.next()?;
    let final_line: u32 = parts.next()?.parse().ok()?;
    Some((sha.to_string(), final_line))
}

#[cfg(test)]
mod tests;
