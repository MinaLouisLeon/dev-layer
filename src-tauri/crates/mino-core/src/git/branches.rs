//! Reading `git branch --format`, and reading a refusal to change branches.
//!
//! The sibling of [`super::commit`]: one thing git tells us, parsed in one
//! place so local and SSH cannot disagree about it.
//!
//! The refusals matter as much as the rows, and they are in [`failure`]:
//! `checkout` fails for three entirely different reasons - the branch is not
//! there, the working tree would be overwritten, the name is already taken -
//! and passing git's raw stderr through would make all three look like the
//! same shell error.

use crate::error::{Result, TransportError};
use crate::types::{GitBranch, GitCommit};

use super::GitOutput;

mod failure;

pub use failure::failure;

/// The separator `BRANCH_FORMAT` puts between fields. A ref name and an author
/// name cannot contain a control character, so nothing in a record can be
/// mistaken for it.
const UNIT: char = '\u{1f}';
const FIELDS: usize = 11;

const REMOTE_PREFIX: &str = "refs/remotes/";

/// Every branch git listed, in git's own order - local first, then remote,
/// each alphabetically. Not re-sorted here: the picker groups them, and a
/// second ordering would be a second thing to keep in agreement.
pub fn parse(output: &GitOutput) -> Result<Vec<GitBranch>> {
    if !output.succeeded() {
        return Err(TransportError::shell(output.message("branch")));
    }
    Ok(output.stdout.lines().filter_map(row).collect())
}

/// One row, or `None` when it is not a branch worth offering.
fn row(line: &str) -> Option<GitBranch> {
    let fields: Vec<&str> = line.split(UNIT).collect();
    // A short row is a git that formats differently from the one this was
    // written against. Dropping the row costs one entry; guessing at its
    // columns would put an author name where an upstream belongs.
    if fields.len() < FIELDS {
        return None;
    }
    // `origin/HEAD` is a symbolic ref pointing at a row already in this list.
    // Offering it would be offering one branch twice under two names.
    if !fields[5].is_empty() {
        return None;
    }
    let refname = fields[1];
    let (ahead, behind) = track(fields[4]);
    Some(GitBranch {
        name: fields[2].to_string(),
        is_head: fields[0].trim() == "*",
        is_remote: refname.starts_with(REMOTE_PREFIX),
        upstream: non_empty(fields[3]),
        ahead,
        behind,
        last_commit: commit(&fields[6..]),
    })
}

/// `%(upstream:track,nobracket)`: `ahead 2, behind 1`, `ahead 2`, `gone`, or
/// empty. Anything unrecognised counts as zero rather than failing the row -
/// a branch whose tracking is unreadable is still a branch you can check out.
fn track(value: &str) -> (u32, u32) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in value.split(',') {
        let mut words = part.split_whitespace();
        let (Some(word), Some(count)) = (words.next(), words.next()) else {
            continue;
        };
        let Ok(count) = count.parse::<u32>() else {
            continue;
        };
        match word {
            "ahead" => ahead = count,
            "behind" => behind = count,
            _ => {}
        }
    }
    (ahead, behind)
}

/// The tip commit, from the five fields after the tracking ones. `None` when
/// git had no sha for it, which is what an unborn branch produces.
fn commit(fields: &[&str]) -> Option<GitCommit> {
    let sha = non_empty(fields.first()?)?;
    Some(GitCommit {
        sha,
        short_sha: fields.get(1)?.to_string(),
        summary: fields.get(2)?.to_string(),
        author: fields.get(3)?.to_string(),
        // Seconds from git, milliseconds on this interface. An unparseable
        // date costs the timestamp, not the branch.
        timestamp_ms: fields
            .get(4)?
            .trim()
            .parse::<u64>()
            .unwrap_or(0)
            .saturating_mul(1_000),
    })
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests;
