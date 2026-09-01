//! `gh run list` and `gh run view --json jobs` rows.

use serde_json::Value;

use crate::error::Result;
use crate::types::{GitHubCheckState, GitHubJob, GitHubRun};

use super::{array, document, instant, number, optional_text, protocol, text};

const RUNS: &str = "the workflow runs";
const JOBS: &str = "the jobs in that run";

/// Every run `gh run list` reported, in the order it reported them - which is
/// newest first, and is the order the checks section relies on to say "the
/// latest run" without sorting anything itself.
pub fn runs(stdout: &str) -> Result<Vec<GitHubRun>> {
    let document = document(stdout, RUNS)?;
    array(&document, RUNS)?.iter().map(run).collect()
}

fn run(row: &Value) -> Result<GitHubRun> {
    let status = text(row, "status", RUNS)?;
    Ok(GitHubRun {
        id: number(row, "databaseId", RUNS)?,
        workflow: text(row, "workflowName", RUNS)?,
        title: text(row, "displayTitle", RUNS)?,
        branch: text(row, "headBranch", RUNS)?,
        // Status and conclusion together, because a run in progress has no
        // conclusion and a completed one is described by nothing else.
        state: GitHubCheckState::from_run(&status, optional_text(row, "conclusion").as_deref()),
        url: text(row, "url", RUNS)?,
        started_ms: instant(row, "startedAt"),
    })
}

/// The jobs inside one run.
///
/// `gh run view --json jobs` answers with an object wrapping the array, unlike
/// the list calls, which answer with the array itself. That difference is
/// `gh`'s and is absorbed here rather than by the caller.
pub fn jobs(stdout: &str) -> Result<Vec<GitHubJob>> {
    let document = document(stdout, JOBS)?;
    let rows = document.get("jobs").ok_or_else(|| protocol(JOBS))?;
    array(rows, JOBS)?.iter().map(job).collect()
}

fn job(row: &Value) -> Result<GitHubJob> {
    let status = text(row, "status", JOBS)?;
    Ok(GitHubJob {
        name: text(row, "name", JOBS)?,
        state: GitHubCheckState::from_run(&status, optional_text(row, "conclusion").as_deref()),
        url: optional_text(row, "url"),
    })
}
