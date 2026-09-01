//! What a remote call said, decided in one place.
//!
//! Three jobs, and the third is the one phase 6 exists to get right.
//!
//! 1. **Reading `git remote -v`** into [`crate::types::GitRemote`], with every
//!    URL redacted on the way through.
//! 2. **Naming the outcome.** A pull that fast-forwarded, a pull that merged,
//!    a pull that left conflict markers and a pull that had nothing to do are
//!    four different situations with four different next moves. Reporting them
//!    as one boolean would put the reader back to comparing two lists.
//! 3. **Turning a failure into a sentence somebody can act on**, without
//!    letting a credential through - which is [`failure`], next door. Every
//!    string that reaches an error there goes through [`super::redact`] first,
//!    not because a particular line is suspected but because the ones that
//!    carry secrets are exactly the ones written by somebody else's git.

use crate::types::{GitConflict, GitPullOutcome, GitPushOutcome, GitRemote};

use super::redact::{redact, summary};
use super::GitOutput;

mod failure;
mod name;

pub use failure::{detached, dirty, failure};
pub use name::{name, optional_name};

/// `git remote --verbose` rows.
///
/// ```text
/// origin<TAB>https://github.com/o/r.git (fetch)
/// origin<TAB>https://github.com/o/r.git (push)
/// ```
///
/// Each remote appears twice, and git allows the two URLs to differ - so both
/// are kept rather than one being assumed to stand for the other.
///
/// **Split on the tab, not on whitespace.** A URL may contain spaces: a local
/// path is a legitimate remote, and plenty of people have one under a home
/// directory with a space in its name. Splitting on whitespace cuts those in
/// half, and the half that survives is not a URL.
pub fn parse(output: &GitOutput) -> Vec<GitRemote> {
    let mut remotes: Vec<GitRemote> = Vec::new();

    for line in output.stdout.lines() {
        let Some((name, rest)) = line.split_once('\t') else {
            continue;
        };
        // The trailing `(fetch)` or `(push)` is git's, not part of the URL.
        let (url, kind) = match rest.rsplit_once(' ') {
            Some((url, kind)) if kind.starts_with('(') => (url, kind),
            _ => (rest, ""),
        };
        if url.is_empty() {
            continue;
        }
        // Redacted here, at the only place a remote URL enters the app. A
        // remote configured with a token in it is ordinary, and this is what
        // keeps it off the screen.
        let url = redact(url);
        let push = kind.contains("push");

        match remotes.iter_mut().find(|existing| existing.name == name) {
            Some(existing) if push => existing.push_url = url,
            Some(existing) => existing.fetch_url = url,
            None => remotes.push(GitRemote {
                name: name.to_string(),
                fetch_url: url.clone(),
                push_url: url,
            }),
        }
    }
    remotes
}

/// What a successful pull did, read from git's own summary.
///
/// The words below are git's, and they are the stable half of its output: the
/// per-file lines change with every version, and these four have not. An
/// unrecognised summary answers [`GitPullOutcome::Merged`], which is the
/// honest answer - something came down and the tree changed - rather than a
/// guess at which of the other three it was.
///
/// `conflicts` is passed in rather than re-derived, because a pull that hit a
/// conflict exits **non-zero**: the caller has already had to look at the tree
/// to know whether it is a conflict or a failure, and asking twice would let
/// the two answers disagree.
pub fn pull_outcome(output: &GitOutput, conflicts: &[GitConflict]) -> GitPullOutcome {
    if !conflicts.is_empty() {
        return GitPullOutcome::Conflicted;
    }
    let combined = format!("{}\n{}", output.stdout, output.stderr).to_lowercase();
    if combined.contains("already up to date") || combined.contains("already up-to-date") {
        return GitPullOutcome::AlreadyUpToDate;
    }
    if combined.contains("fast-forward") {
        return GitPullOutcome::FastForwarded;
    }
    if combined.contains("rebasing") || combined.contains("successfully rebased") {
        return GitPullOutcome::Rebased;
    }
    GitPullOutcome::Merged
}

/// What a successful push did, from `git push --porcelain`.
///
/// `--porcelain` is why this is readable at all. Its per-ref lines start with
/// a flag character - `=` for up to date, a space for a normal update, `*` for
/// a new ref - and that is a documented interface, unlike the human summary on
/// stderr which differs between versions and locales.
pub fn push_outcome(output: &GitOutput) -> GitPushOutcome {
    let up_to_date = output
        .stdout
        .lines()
        .filter(|line| line.contains('\t'))
        .all(|line| line.starts_with('='));
    if up_to_date && output.stdout.contains('\t') {
        return GitPushOutcome::AlreadyUpToDate;
    }
    GitPushOutcome::Pushed
}

/// Git's own words about what it just did, redacted and trimmed.
pub fn said(output: &GitOutput) -> Option<String> {
    // stderr first: git puts its progress and its summary there, and stdout
    // is the porcelain the parser above has already read.
    summary(&output.stderr).or_else(|| summary(&output.stdout))
}

#[cfg(test)]
mod tests;
