use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why the GitHub surface has something to show, or has not.
///
/// Four answers, and they are four different facts. Only the last one is a
/// reason to make a network call, and none of the other three is an *error*:
/// a machine without `gh`, an account that has not logged in and a checkout
/// whose remote is not GitHub are all ordinary conditions that the pane
/// renders as one sentence and then goes quiet about.
///
/// This is the same shape [`crate::types::GitRepository`] is asked for
/// through `repository()`: one cheap call on mount, remembered for the
/// session, rather than every surface discovering the same absence for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum GitHubAvailability {
    /// `gh` is not installed, or is not on the target's PATH.
    Absent,
    /// `gh` is there and has no credentials. The app cannot offer to fix
    /// this - `gh auth login` is interactive and owns its own keychain entry -
    /// so the only correct thing to say is which command to run.
    Unauthenticated,
    /// `gh` is logged in, but this folder is not a GitHub checkout: no
    /// repository, no remote, or a remote pointing at GitLab or Bitbucket.
    /// Quiet absence, not an error.
    Unsupported,
    /// A GitHub repository, reachable, with credentials. The only state in
    /// which a section may query anything.
    Ready,
}

/// The repository the remote points at, as `gh` resolved it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubRepository {
    /// `owner/name`, exactly as `gh` printed it. Rendered as text like every
    /// other value that came from the network.
    pub name_with_owner: String,
    /// The repository's own web address. The one URL the app did not build.
    pub url: String,
    /// The branch a new pull request targets unless the author picks another.
    pub default_branch: Option<String>,
}

/// What one probe found. Cheap enough to call on mount, and called again only
/// when the session changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct GitHubProbe {
    pub availability: GitHubAvailability,
    /// Filled only when `availability` is [`GitHubAvailability::Ready`].
    pub repository: Option<GitHubRepository>,
    /// `gh`'s own sentence, when it had one worth reading. Untrusted text like
    /// everything else `gh` prints, so it is rendered and never interpreted.
    pub detail: Option<String>,
}

impl GitHubProbe {
    /// The three quiet states, each carrying whatever `gh` said about itself.
    pub fn quiet(availability: GitHubAvailability, detail: Option<String>) -> Self {
        Self {
            availability,
            repository: None,
            detail,
        }
    }

    pub fn ready(repository: GitHubRepository) -> Self {
        Self {
            availability: GitHubAvailability::Ready,
            repository: Some(repository),
            detail: None,
        }
    }

    /// True when a query may be sent. Asked by the transport before it runs
    /// one, so a section that forgot to check cannot reach the network.
    pub fn is_ready(&self) -> bool {
        matches!(self.availability, GitHubAvailability::Ready)
    }
}
