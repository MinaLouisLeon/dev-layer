//! Reading `git log` and `git show`.
//!
//! Both use the one format constant in [`crate::git::command`], so a commit
//! looks the same whichever call produced it. Fields are separated by `\x1f`
//! and records by NUL:
//!
//! ```text
//! <sha>\x1f<short>\x1f<subject>\x1f<author>\x1f<unix time>\0
//! ```
//!
//! NUL for records because git forbids it inside a commit object outright, so
//! there is no message that can break the split. `\x1f` for fields because a
//! subject can contain a tab or a newline but has no reason to contain a unit
//! separator.

use crate::error::{Result, TransportError};
use crate::types::{GitChangedFile, GitCommit, GitCommitDetail, GitFileState, GitLog, LogRequest};

use super::GitOutput;

const FIELD: char = '\x1f';

/// Parses a page of history.
///
/// The caller asked git for one commit more than the limit; that extra row is
/// how `truncated` is answered, and it is dropped rather than returned.
pub fn log_from(output: &GitOutput, request: &LogRequest) -> Result<GitLog> {
    if !output.succeeded() {
        // An unborn branch is the case worth being careful about: `git log`
        // fails on a repository with no commits, and "there is no history yet"
        // is an answer rather than an error.
        if is_unborn(output) {
            return Ok(GitLog {
                commits: Vec::new(),
                truncated: false,
            });
        }
        return Err(TransportError::shell(output.message("log")));
    }

    let mut commits: Vec<GitCommit> = output
        .stdout
        .split('\0')
        .filter_map(|record| commit_from(record.trim_start_matches('\n')))
        .collect();

    let limit = request.effective_limit() as usize;
    let truncated = commits.len() > limit;
    commits.truncate(limit);
    Ok(GitLog { commits, truncated })
}

/// Parses one commit and the files it touched.
///
/// The output is the commit record, a NUL, a newline, and then the name-status
/// entries NUL-separated - which is why every field is trimmed of a leading
/// newline before it is read.
pub fn detail_from(output: &GitOutput) -> Result<GitCommitDetail> {
    if !output.succeeded() {
        return Err(TransportError::shell(output.message("show")));
    }
    let mut fields = output
        .stdout
        .split('\0')
        .map(|f| f.trim_start_matches('\n'));

    let commit = fields
        .next()
        .and_then(commit_from)
        .ok_or_else(|| TransportError::protocol("git did not describe that commit"))?;

    let mut files = Vec::new();
    while let Some(status) = fields.next().filter(|s| !s.is_empty()) {
        // A rename or a copy is reported as `R100`/`C75` and consumes two
        // paths: where it came from, then where it went.
        let renamed = status.starts_with('R') || status.starts_with('C');
        let first = fields.next().filter(|p| !p.is_empty());
        let second = renamed
            .then(|| fields.next().filter(|p| !p.is_empty()))
            .flatten();

        let (old_path, relative_path) = match (first, second) {
            (Some(from), Some(to)) => (Some(from.to_string()), to.to_string()),
            (Some(only), None) => (None, only.to_string()),
            _ => break,
        };
        files.push(GitChangedFile {
            relative_path,
            old_path,
            state: state_from(status),
        });
    }

    Ok(GitCommitDetail { commit, files })
}

fn commit_from(record: &str) -> Option<GitCommit> {
    let mut fields = record.split(FIELD);
    let sha = fields.next()?.trim();
    if sha.is_empty() {
        return None;
    }
    let short_sha = fields.next()?.to_string();
    let summary = fields.next()?.to_string();
    let author = fields.next()?.to_string();
    let seconds: u64 = fields.next()?.trim().parse().ok()?;
    Some(GitCommit {
        sha: sha.to_string(),
        short_sha,
        summary,
        author,
        // Git counts in seconds; everything else on this interface is in
        // milliseconds, and `DirEntry::modified_ms` set that habit.
        timestamp_ms: seconds.saturating_mul(1_000),
    })
}

/// `--name-status` letters, onto the enum the tree and the panel already use,
/// so one file state means one thing everywhere.
fn state_from(status: &str) -> GitFileState {
    match status.chars().next() {
        Some('A') => GitFileState::Added,
        Some('D') => GitFileState::Deleted,
        Some('R') => GitFileState::Renamed,
        Some('C') => GitFileState::Copied,
        Some('T') => GitFileState::TypeChanged,
        Some('U') => GitFileState::Conflicted,
        _ => GitFileState::Modified,
    }
}

/// Git's wording for a repository that has no commits yet.
fn is_unborn(output: &GitOutput) -> bool {
    let stderr = output.stderr.to_lowercase();
    stderr.contains("does not have any commits")
        || stderr.contains("bad default revision")
        || stderr.contains("unknown revision")
}

#[cfg(test)]
mod tests;
