use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::check::GitHubCheckState;

/// One workflow run on a branch.
///
/// Every string here was written by somebody else - a workflow author, a
/// commit message, a branch name on a fork - and reaches the UI as **text**.
/// Nothing in this type is markup, a URL the app builds, or a value that goes
/// back into a command line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubRun {
    /// GitHub's own run id, and the only thing that names a run to
    /// [`super::query::GitHubQuery::RunJobs`]. A number, so no text a caller
    /// holds reaches that argv.
    #[ts(type = "number")]
    pub id: u64,
    /// The workflow's name, e.g. `CI`.
    pub workflow: String,
    /// What the run was for: the commit summary, or the pull request title.
    pub title: String,
    pub branch: String,
    pub state: GitHubCheckState,
    pub url: String,
    /// When it started, Unix epoch milliseconds - the unit every other time on
    /// this interface uses. `None` when GitHub had not recorded one yet.
    #[ts(type = "number | null")]
    pub started_ms: Option<u64>,
}

/// One job inside a run.
///
/// Read only when a run failed, and only for that run. This is the whole of
/// #14's promise beyond a red dot: the name of the job that broke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubJob {
    pub name: String,
    pub state: GitHubCheckState,
    pub url: Option<String>,
}
