//! The two reads that are not on the trait.
//!
//! `toplevel` answers "is this a repository, and where does it start" for
//! every other call, and `ignored` serves the search walk on its own behalf.
//! Both are here rather than in `git.rs` so that file stays a list of trait
//! methods.

use crate::error::Result;
use crate::git::{self, command, paths::PathStyle};

use super::git_run::run;

/// The work tree root containing `root`, or `None` when there is not one.
pub async fn toplevel(root: &str) -> Result<Option<String>> {
    let output = run(root, &command::repository_argv()).await?;
    // `--show-toplevel` answers in forward slashes even on Windows. Putting it
    // back into the platform's own style is what lets the entry paths compare
    // against `DirEntry::path` without special cases downstream.
    Ok(git::repository_root(&output)?.map(|found| PathStyle::local().normalise(&found)))
}

/// The repository-relative paths git would not look at, for the search walk.
///
/// Not on the trait: search asks for this on its own behalf, and a failure -
/// git absent, not a repository, a call that timed out - is answered with an
/// empty list rather than an error. Losing search entirely because a folder is
/// not a repository would be a regression, so this degrades instead.
pub async fn ignored(root: &str) -> Vec<String> {
    match run(root, &command::ignored_argv()).await {
        Ok(output) => git::ignored_from(&output),
        Err(err) => {
            tracing::debug!(%err, "git could not answer for ignored paths; searching everything");
            Vec::new()
        }
    }
}
