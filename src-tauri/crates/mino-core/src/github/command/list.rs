//! Argv for the five calls that only read.
//!
//! Every one of them names its `--json` fields explicitly. That is not
//! tidiness: it is the whole mitigation for `gh` changing shape between
//! versions. A field this build needs and `gh` no longer has is a non-zero
//! exit with `gh`'s own sentence, and a field `gh` added that this build does
//! not name costs nothing at all.

use super::{issue_state_word, limit, owned, state_word};
use crate::types::{IssueState, PrState};

/// Fields every run row is read with.
pub const RUN_FIELDS: &str =
    "databaseId,workflowName,displayTitle,headBranch,status,conclusion,url,startedAt";

/// Fields every pull request row is read with. `statusCheckRollup` is the
/// expensive one and the reason the section exists: without it a list of pull
/// requests says nothing about whether any of them is safe to merge.
pub const PR_FIELDS: &str =
    "number,title,author,url,state,isDraft,headRefName,baseRefName,updatedAt,statusCheckRollup";

/// The same, plus the body. Asked for one pull request at a time - see
/// [`crate::types::GitHubPullRequest::body`].
pub const PR_DETAIL_FIELDS: &str =
    "number,title,author,url,state,isDraft,headRefName,baseRefName,updatedAt,statusCheckRollup,body";

pub const ISSUE_FIELDS: &str = "number,title,author,url,state,labels,updatedAt";

/// `gh run list --branch <branch> --limit <n> --json …`.
///
/// The branch is a caller value and travels as its own argv element behind an
/// explicit flag. It has already been through [`crate::git::refname::precheck`]
/// by the time it arrives - the same guard a checkout uses - so a name that
/// could be read as an option never gets this far.
pub fn runs_argv(branch: &str, requested: u32) -> Vec<String> {
    let mut argv = owned(&["run", "list", "--branch"]);
    argv.push(branch.to_string());
    argv.push("--limit".to_string());
    argv.push(limit(requested));
    argv.extend(owned(&["--json", RUN_FIELDS]));
    argv
}

/// `gh run view <id> --json jobs`.
///
/// The id is a `u64` formatted here, so no caller text reaches this argv at
/// all - the same property [`crate::git::command::selector`] gives a stash
/// index.
pub fn run_jobs_argv(run_id: u64) -> Vec<String> {
    vec![
        "run".to_string(),
        "view".to_string(),
        run_id.to_string(),
        "--json".to_string(),
        "jobs".to_string(),
    ]
}

/// `gh pr list --state <word> --limit <n> --json …`.
pub fn pull_requests_argv(state: PrState, requested: u32) -> Vec<String> {
    let mut argv = owned(&["pr", "list", "--state", state_word(state), "--limit"]);
    argv.push(limit(requested));
    argv.extend(owned(&["--json", PR_FIELDS]));
    argv
}

/// `gh pr view <number> --json …`. A number, so nothing a caller typed is here.
pub fn pull_request_argv(number: u32) -> Vec<String> {
    vec![
        "pr".to_string(),
        "view".to_string(),
        number.to_string(),
        "--json".to_string(),
        PR_DETAIL_FIELDS.to_string(),
    ]
}

/// `gh issue list --state <word> --limit <n> --json …`.
pub fn issues_argv(state: IssueState, requested: u32) -> Vec<String> {
    let mut argv = owned(&[
        "issue",
        "list",
        "--state",
        issue_state_word(state),
        "--limit",
    ]);
    argv.push(limit(requested));
    argv.extend(owned(&["--json", ISSUE_FIELDS]));
    argv
}
