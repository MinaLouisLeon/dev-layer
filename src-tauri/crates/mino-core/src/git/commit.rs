//! Reading a commit, and reading a refusal to make one.
//!
//! The counterpart of [`super::branch`]: one thing git tells us, parsed in one
//! place so both transports read it the same way.
//!
//! Git reports "there was nothing to commit" as a **failure** - exit 1, with
//! the explanation on stdout rather than stderr. Passing that through as a
//! generic shell error would show the user `On branch main / nothing to commit,
//! working tree clean` as if something had broken. It has not; they asked for
//! a commit with nothing staged, and that is worth one clear sentence.

use crate::error::{Result, TransportError};
use crate::types::{CommitRequest, GitCommit, MAX_COMMIT_MESSAGE_BYTES};

use super::GitOutput;

/// Git's own wording when there is nothing staged. Matched on stdout *and*
/// stderr because git puts this on stdout, unlike its real errors.
const NOTHING_TO_COMMIT: [&str; 3] = [
    "nothing to commit",
    "no changes added to commit",
    "nothing added to commit",
];

/// Checked before git is spawned, so an empty message costs nothing and says
/// what is wrong rather than letting git open an editor or refuse obscurely.
pub fn validate(request: &CommitRequest) -> Result<()> {
    if request.trimmed().is_empty() {
        return Err(TransportError::invalid(
            "a commit needs a message. Write one and try again.",
        ));
    }
    if request.message.len() > MAX_COMMIT_MESSAGE_BYTES {
        return Err(TransportError::invalid(format!(
            "that commit message is {} bytes, above the {MAX_COMMIT_MESSAGE_BYTES} byte ceiling",
            request.message.len()
        )));
    }
    Ok(())
}

/// Turns a failed `git commit` into the error worth showing.
pub fn failure(output: &GitOutput) -> TransportError {
    let combined = format!("{} {}", output.stdout, output.stderr).to_lowercase();
    if NOTHING_TO_COMMIT.iter().any(|line| combined.contains(line)) {
        return TransportError::invalid("there is nothing staged to commit. Stage a change first.");
    }
    // Git's advice block for a missing identity is long and its first line is
    // not the useful one, so this says the actionable part itself.
    if combined.contains("please tell me who you are") || combined.contains("empty ident") {
        return TransportError::shell(
            "git does not know who you are. Set user.name and user.email, \
             then commit again.",
        );
    }
    let stderr = output.stderr.trim();
    let stdout = output.stdout.trim();
    TransportError::shell(match (stderr.is_empty(), stdout.is_empty()) {
        (false, _) => stderr.to_string(),
        (true, false) => stdout.to_string(),
        (true, true) => "the commit failed".to_string(),
    })
}

/// Parses `head_commit_argv` output: five NUL-separated fields.
pub fn parse(output: &GitOutput) -> Result<GitCommit> {
    if !output.succeeded() {
        return Err(TransportError::shell(
            "the commit was made, but git could not describe it",
        ));
    }
    let mut fields = output.stdout.trim_end_matches('\0').split('\0');
    let mut next = |what: &str| -> Result<String> {
        fields
            .next()
            .map(str::to_string)
            .ok_or_else(|| TransportError::protocol(format!("git omitted the commit {what}")))
    };

    let sha = next("sha")?;
    let short_sha = next("short sha")?;
    let summary = next("summary")?;
    let author = next("author")?;
    let seconds: u64 = next("timestamp")?
        .trim()
        .parse()
        .map_err(|_| TransportError::protocol("git reported an unreadable commit time"))?;

    if sha.is_empty() {
        return Err(TransportError::protocol(
            "git reported a commit with no sha",
        ));
    }
    Ok(GitCommit {
        sha,
        short_sha,
        summary,
        author,
        // Git counts in seconds; everything else on this interface is in
        // milliseconds, and `DirEntry::modified_ms` already set that habit.
        timestamp_ms: seconds.saturating_mul(1_000),
    })
}

#[cfg(test)]
mod tests;
