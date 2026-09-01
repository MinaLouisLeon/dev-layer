//! `impl GitHubTransport for LocalTransport`.
//!
//! Dispatch only, in the same spirit as `git.rs`: resolve the root, run one
//! `gh` call, hand the output to [`crate::github`] to interpret. Nothing here
//! builds an argument and nothing here parses one, which is what lets this
//! file and `ssh/github.rs` be the same twenty lines twice over rather than
//! two implementations that can disagree.
//!
//! Every call runs with the *session* root as its working directory, exactly
//! as git does. That is how `gh` works out which repository is being asked
//! about, and it is why no repository name is ever passed as an argument.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::paths::PathStyle;
use crate::github::{self, call, probe};
use crate::transport::GitHubTransport;
use crate::types::{GitHubProbe, GitHubQuery, GitHubResponse};

use super::github_run::run;
use super::LocalTransport;

#[async_trait]
impl GitHubTransport for LocalTransport {
    async fn probe(&self) -> Result<GitHubProbe> {
        let root = self.guard()?.root_display();
        // Cheapest first, and no process at all for the commonest absence.
        if github::find_gh().is_none() {
            return Ok(probe::absent());
        }

        // Asked before `repo view` because both fail without credentials and
        // only this one fails *for that reason*. See `github::probe`.
        let auth = run(&root, &github::command::auth_status_argv(), None).await?;
        if !auth.succeeded() {
            return Ok(probe::unauthenticated(&auth));
        }

        let repository = run(&root, &github::command::repo_view_argv(), None).await?;
        if !repository.succeeded() {
            return Ok(probe::unsupported(&repository));
        }
        probe::repository(&repository)
    }

    async fn query(&self, request: GitHubQuery) -> Result<GitHubResponse> {
        let root = self.guard()?.root_display();
        // Every guard runs in `plan`: the path guard for a browse, the branch
        // guard for a run list, and the validation in front of the one call
        // that writes. Nothing below this line can reach `gh` unruled.
        let call = call::plan(&request, &root, PathStyle::local())?;

        let output = run(&root, &call.argv, call.input.as_deref()).await?;
        if !output.succeeded() {
            return Err(call::failure(&call, &request, &output));
        }
        // One variant answers with something other than what it wants read -
        // a reply hands back the new comment, and the caller wants the
        // thread. `plan` decides that; this runs it.
        let output = match &call.follow_up {
            Some(argv) => run(&root, argv, None).await?,
            None => output,
        };
        call::read(call.shape, &output)
    }
}
