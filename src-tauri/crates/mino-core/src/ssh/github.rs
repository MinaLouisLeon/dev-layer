//! `impl GitHubTransport for SshTransport`.
//!
//! The same three steps the local transport takes - resolve the root, run one
//! call, hand the output to [`crate::github`] - over the exec channel instead
//! of `tokio::process`. Nothing here builds an argument and nothing here
//! parses one, which is the whole point of the shared module: local and remote
//! read one parser.
//!
//! **The credential position holds at a distance, and is worth stating.** The
//! `gh` that runs is the *remote host's*, signed in as the remote account,
//! with its credential in that machine's keychain. Nothing about this
//! machine's GitHub login is involved and no token crosses the connection -
//! there is no token here to cross it.
//!
//! Remote paths are POSIX regardless of what this client runs on, so the
//! [`PathStyle`] is fixed rather than probed.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::paths::PathStyle;
use crate::github::{call, command, probe};
use crate::transport::GitHubTransport;
use crate::types::{GitHubProbe, GitHubQuery, GitHubResponse};

use super::github_run::{is_missing, run};
use super::SshTransport;

#[async_trait]
impl GitHubTransport for SshTransport {
    async fn probe(&self) -> Result<GitHubProbe> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();

        // No `which` at a distance: the only way to learn whether the remote
        // host has `gh` is to run it. Exit 127 is the shell saying it could
        // not find the command - see `github_run::NOT_FOUND`.
        let auth = run(&connected.handle, &root, &command::auth_status_argv(), None).await?;
        if is_missing(&auth) {
            return Ok(probe::absent());
        }
        if !auth.succeeded() {
            return Ok(probe::unauthenticated(&auth));
        }

        let repository = run(&connected.handle, &root, &command::repo_view_argv(), None).await?;
        if !repository.succeeded() {
            return Ok(probe::unsupported(&repository));
        }
        probe::repository(&repository)
    }

    async fn query(&self, request: GitHubQuery) -> Result<GitHubResponse> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        // Every guard runs in `plan`, on this side of the connection: a path
        // is ruled against the session root here, before anything is quoted
        // and long before anything is sent.
        let call = call::plan(&request, &root, PathStyle::posix())?;

        let output = run(&connected.handle, &root, &call.argv, call.input.as_deref()).await?;
        if !output.succeeded() {
            return Err(call::failure(&call, &request, &output));
        }
        // See the local transport: one variant answers with something other
        // than what it wants read, and `plan` is where that is decided.
        let output = match &call.follow_up {
            Some(argv) => run(&connected.handle, &root, argv, None).await?,
            None => output,
        };
        call::read(call.shape, &output)
    }
}
