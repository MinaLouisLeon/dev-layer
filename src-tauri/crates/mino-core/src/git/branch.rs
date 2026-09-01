//! The `# branch.*` headers `--porcelain=v2 --branch` puts ahead of the file
//! records.
//!
//! Four lines carry everything the header strip needs, which is why phase 1
//! gets away with a single status call: there is no second `git branch` or
//! `git rev-parse` to make them agree with.
//!
//! ```text
//! # branch.oid 3f2a1c…        or (initial) before the first commit
//! # branch.head main          or (detached)
//! # branch.upstream origin/main
//! # branch.ab +2 -1
//! ```
//!
//! Only `branch.oid` and `branch.head` are always present. A repository with
//! no upstream simply omits the last two, which is why every field below has a
//! defined value for "git did not say".

use crate::types::GitRepository;

/// Git's placeholder for an unborn branch - `git init` with no commit yet.
const INITIAL: &str = "(initial)";
/// Git's placeholder for a detached HEAD.
const DETACHED: &str = "(detached)";
/// How much of the sha the UI shows. Seven is what `git log --oneline` uses.
const SHORT_SHA_LEN: usize = 7;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchHeaders {
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

impl BranchHeaders {
    /// Takes one `# ` record, already stripped of its terminator. Anything
    /// unrecognised is ignored rather than refused: git may add headers, and a
    /// new one is not a reason to fail a status the user asked for.
    pub fn absorb(&mut self, record: &str) {
        let Some(rest) = record.strip_prefix("# ") else {
            return;
        };
        let Some((key, value)) = rest.split_once(' ') else {
            return;
        };
        match key {
            "branch.oid" if value != INITIAL => {
                self.head = Some(value.chars().take(SHORT_SHA_LEN).collect());
            }
            "branch.head" if value == DETACHED => self.detached = true,
            "branch.head" => self.branch = Some(value.to_string()),
            "branch.upstream" => self.upstream = Some(value.to_string()),
            "branch.ab" => self.absorb_ab(value),
            _ => {}
        }
    }

    /// `+2 -1`. Either half may be missing on a malformed line; a count that
    /// does not parse stays zero rather than taking the whole status down.
    fn absorb_ab(&mut self, value: &str) {
        for part in value.split_whitespace() {
            let (sign, digits) = part.split_at(1);
            let Ok(count) = digits.parse::<u32>() else {
                continue;
            };
            match sign {
                "+" => self.ahead = count,
                "-" => self.behind = count,
                _ => {}
            }
        }
    }

    /// Pairs the headers with the work tree root git reported separately.
    pub fn into_repository(self, root: String) -> GitRepository {
        GitRepository {
            root,
            branch: self.branch,
            head: self.head,
            detached: self.detached,
            upstream: self.upstream,
            ahead: self.ahead,
            behind: self.behind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absorb(records: &[&str]) -> BranchHeaders {
        let mut headers = BranchHeaders::default();
        for record in records {
            headers.absorb(record);
        }
        headers
    }

    #[test]
    fn a_tracking_branch_reports_its_counts() {
        let headers = absorb(&[
            "# branch.oid 3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
            "# branch.head dev",
            "# branch.upstream origin/dev",
            "# branch.ab +2 -1",
        ]);
        assert_eq!(headers.branch.as_deref(), Some("dev"));
        assert_eq!(headers.head.as_deref(), Some("3f2a1c9"));
        assert_eq!(headers.upstream.as_deref(), Some("origin/dev"));
        assert_eq!((headers.ahead, headers.behind), (2, 1));
        assert!(!headers.detached);
    }

    #[test]
    fn an_unborn_branch_has_a_name_but_no_head() {
        let headers = absorb(&["# branch.oid (initial)", "# branch.head main"]);
        assert_eq!(headers.branch.as_deref(), Some("main"));
        assert_eq!(headers.head, None);
        assert!(!headers.detached);
    }

    #[test]
    fn a_detached_head_has_a_sha_but_no_name() {
        let headers = absorb(&[
            "# branch.oid 3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
            "# branch.head (detached)",
        ]);
        assert_eq!(headers.branch, None);
        assert!(headers.detached);
        assert_eq!(headers.head.as_deref(), Some("3f2a1c9"));
    }

    #[test]
    fn an_unknown_header_is_ignored_not_refused() {
        let headers = absorb(&["# branch.something else", "# malformed", "not a header"]);
        assert_eq!(headers, BranchHeaders::default());
    }
}
