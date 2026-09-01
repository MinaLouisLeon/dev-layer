//! Argv for the two calls the probe makes. Neither takes a caller value at
//! all, which is the strongest form the rule in [`super`] takes.

use super::owned;

/// `gh auth status`.
///
/// Asked before anything else, and asked separately from `gh repo view` on
/// purpose: both fail when there are no credentials, and only this one fails
/// *for that reason*. Collapsing them would leave "log in" and "this is not a
/// GitHub repository" as the same sentence.
pub fn auth_status_argv() -> Vec<String> {
    owned(&["auth", "status"])
}

/// `gh repo view --json nameWithOwner,url,defaultBranchRef`.
///
/// The one call that answers "what repository does this remote point at". It
/// fails - and the probe reads that as [`crate::types::GitHubAvailability::Unsupported`] -
/// when the folder is not a repository, has no remote, or has one pointing at
/// a host `gh` does not serve.
///
/// Fields are named explicitly rather than taken wholesale. That is the answer
/// to `gh` changing shape between versions: a field that goes away is a
/// non-zero exit here, and a field that is added costs nothing.
pub fn repo_view_argv() -> Vec<String> {
    owned(&[
        "repo",
        "view",
        "--json",
        "nameWithOwner,url,defaultBranchRef",
    ])
}
