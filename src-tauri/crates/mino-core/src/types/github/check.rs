use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a run, a job or a pull request's checks are getting on.
///
/// Seven states rather than the two dozen words GitHub uses across `status`,
/// `conclusion` and `statusCheckRollup`. That narrowing is deliberate and it
/// is where the version risk is answered: `gh` may add a conclusion next year,
/// and a vocabulary copied verbatim would either grow a variant the UI does
/// not render or fail to parse a run that is otherwise fine.
///
/// [`Unknown`](Self::Unknown) is the catch-all, and it is a *state*, not a
/// parse failure. A run whose conclusion this build has never heard of is
/// still a run worth listing with its title and its link; refusing the whole
/// list over one unrecognised word would be the worse answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitHubCheckState {
    /// Queued, waiting, requested: accepted and not started.
    Pending,
    /// In progress right now. The one state worth looking at again shortly -
    /// though only when the reader asks, never on a timer.
    Running,
    Passed,
    Failed,
    Cancelled,
    /// Skipped, or neutral: it ran and decided it had nothing to say.
    Skipped,
    /// No checks at all, or a word this build does not recognise.
    Unknown,
}

impl GitHubCheckState {
    /// `gh`'s `status` and `conclusion` pair, mapped onto one state.
    ///
    /// A completed run is described by its conclusion; anything else is
    /// described by its status, because a run still going has no conclusion
    /// to read. Both are matched case-insensitively: the REST and GraphQL
    /// halves of GitHub disagree about capitalisation, and `gh` passes
    /// whichever it used straight through.
    pub fn from_run(status: &str, conclusion: Option<&str>) -> Self {
        match status.to_ascii_lowercase().as_str() {
            "completed" => Self::from_conclusion(conclusion.unwrap_or_default()),
            "in_progress" | "inprogress" => Self::Running,
            "queued" | "waiting" | "pending" | "requested" => Self::Pending,
            _ => Self::Unknown,
        }
    }

    /// One word from `conclusion`, or the `state` of an older commit status.
    ///
    /// Both vocabularies are read here rather than in two functions, because a
    /// repository can carry both kinds of check at once and a reader does not
    /// care which mechanism reported the failure.
    pub fn from_conclusion(word: &str) -> Self {
        match word.to_ascii_lowercase().as_str() {
            "success" => Self::Passed,
            // `startup_failure`, `timed_out` and a commit status `error` are
            // failures the reader has to act on in exactly the same way, so
            // they read the same here.
            "failure" | "error" | "timed_out" | "timedout" | "startup_failure"
            | "action_required" => Self::Failed,
            "cancelled" | "canceled" | "stale" => Self::Cancelled,
            "skipped" | "neutral" => Self::Skipped,
            "pending" | "expected" => Self::Pending,
            _ => Self::Unknown,
        }
    }

    /// The state a set of checks adds up to.
    ///
    /// One failure decides it, then anything still running, then anything
    /// pending. That order is the useful one: a reader wants to know whether
    /// this is broken before they want to know whether it has finished.
    pub fn rollup(states: impl IntoIterator<Item = Self>) -> Self {
        let mut seen_running = false;
        let mut seen_pending = false;
        let mut seen_passed = false;
        for state in states {
            match state {
                Self::Failed => return Self::Failed,
                Self::Running => seen_running = true,
                Self::Pending => seen_pending = true,
                Self::Passed => seen_passed = true,
                Self::Cancelled | Self::Skipped | Self::Unknown => {}
            }
        }
        if seen_running {
            Self::Running
        } else if seen_pending {
            Self::Pending
        } else if seen_passed {
            Self::Passed
        } else {
            Self::Unknown
        }
    }
}
