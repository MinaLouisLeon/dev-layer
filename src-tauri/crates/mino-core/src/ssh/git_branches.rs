//! `impl GitBranchTransport for SshTransport`.
//!
//! The mirror of `local/git_branches.rs`, over the exec channel instead of
//! `tokio::process`, and reading the *same* parsers - so a branch list and a
//! failed checkout say the same thing wherever the repository is.
//!
//! The branch name still goes through [`crate::git::refname`] and still gets a
//! `git check-ref-format` of its own before the call that changes anything.
//! That is a second round trip on a remote target, and it is worth it: it is
//! the remote git's rules the name has to satisfy, and asking the local one
//! would be answering with the wrong git.

use async_trait::async_trait;
use russh::client::Handle;

use crate::error::{Result, TransportError};
use crate::git::{self, command, refname, revision};
use crate::transport::GitBranchTransport;
use crate::types::{CreateBranchRequest, GitBranch};

use super::git_run::run_with_input;
use super::handler::ClientHandler;
use super::SshTransport;

#[async_trait]
impl GitBranchTransport for SshTransport {
    async fn branches(&self) -> Result<Vec<GitBranch>> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let output =
            run_with_input(&connected.handle, &root, &command::branches_argv(), None).await?;
        git::branches::parse(&output)
    }

    async fn checkout(&self, name: &str) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let name = checked(&connected.handle, &root, name).await?;
        let argv = command::checkout_argv(&name);
        let output = run_with_input(&connected.handle, &root, &argv, None).await?;
        if output.succeeded() {
            return Ok(());
        }
        Err(git::branches::failure(&output, &name, "checkout"))
    }

    async fn create_branch(&self, request: CreateBranchRequest) -> Result<GitBranch> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let name = checked(&connected.handle, &root, &request.name).await?;
        let request = CreateBranchRequest {
            from: revision::validate_optional(request.from.as_deref())?,
            ..request
        };

        let argv = command::create_argv(&request, &name);
        let output = run_with_input(&connected.handle, &root, &argv, None).await?;
        if !output.succeeded() {
            return Err(git::branches::failure(&output, &name, "create branch"));
        }
        // Read back rather than assumed: the tip, the upstream and whether
        // HEAD is on it are all things git decided.
        self.branches()
            .await?
            .into_iter()
            .find(|branch| branch.name == name && !branch.is_remote)
            .ok_or_else(|| {
                TransportError::shell(format!(
                    "the branch `{name}` was created, but git did not list it afterwards"
                ))
            })
    }

    async fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let name = checked(&connected.handle, &root, name).await?;
        let argv = command::delete_argv(&name, force);
        let output = run_with_input(&connected.handle, &root, &argv, None).await?;
        if output.succeeded() {
            return Ok(());
        }
        Err(git::branches::failure(&output, &name, "delete branch"))
    }
}

/// A caller's branch name, refused locally for anything readable as an option
/// and then put to the *remote* `git check-ref-format`.
async fn checked(handle: &Handle<ClientHandler>, root: &str, name: &str) -> Result<String> {
    let name = refname::precheck(name)?;
    let checked = run_with_input(handle, root, &refname::check_argv(&name), None).await?;
    if checked.succeeded() {
        return Ok(name);
    }
    Err(refname::refused(&name))
}
