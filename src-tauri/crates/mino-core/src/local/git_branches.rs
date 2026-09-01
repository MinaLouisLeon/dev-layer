//! `impl GitBranchTransport for LocalTransport`.
//!
//! The same three steps every git call in this crate takes - argv from
//! [`crate::git::command`], run it, hand the output to a shared parser - with
//! one extra step in front of the mutating three: the branch name goes through
//! [`crate::git::refname`], which asks `git check-ref-format` whether git
//! itself would accept it.
//!
//! That check is a git call of its own, and it runs *before* the call that
//! changes anything. A name git would refuse never reaches `checkout` or
//! `branch`, which is what makes "an invalid branch name is refused before git
//! runs" true of the call that matters.

use async_trait::async_trait;

use crate::error::{Result, TransportError};
use crate::git::{self, command, refname, revision};
use crate::transport::GitBranchTransport;
use crate::types::{CreateBranchRequest, GitBranch};

use super::git_run::run_with_input;
use super::LocalTransport;

#[async_trait]
impl GitBranchTransport for LocalTransport {
    async fn branches(&self) -> Result<Vec<GitBranch>> {
        let root = self.guard()?.root_display();
        let output = run_with_input(&root, &command::branches_argv(), None).await?;
        git::branches::parse(&output)
    }

    async fn checkout(&self, name: &str) -> Result<()> {
        let root = self.guard()?.root_display();
        let name = checked(&root, name).await?;
        let output = run_with_input(&root, &command::checkout_argv(&name), None).await?;
        if output.succeeded() {
            return Ok(());
        }
        // Never reported as a partial success. Git either switched or it did
        // not, and the caller re-reads from truth either way.
        Err(git::branches::failure(&output, &name, "checkout"))
    }

    async fn create_branch(&self, request: CreateBranchRequest) -> Result<GitBranch> {
        let root = self.guard()?.root_display();
        let name = checked(&root, &request.name).await?;
        // A start point is a revision, not a branch name: `HEAD~2` and
        // `origin/main` are both legitimate here and neither is a ref name.
        let request = CreateBranchRequest {
            from: revision::validate_optional(request.from.as_deref())?,
            ..request
        };

        let output = run_with_input(&root, &command::create_argv(&request, &name), None).await?;
        if !output.succeeded() {
            return Err(git::branches::failure(&output, &name, "create branch"));
        }
        created(self, &name).await
    }

    async fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let root = self.guard()?.root_display();
        let name = checked(&root, name).await?;
        let output = run_with_input(&root, &command::delete_argv(&name, force), None).await?;
        if output.succeeded() {
            return Ok(());
        }
        Err(git::branches::failure(&output, &name, "delete branch"))
    }
}

/// The branch that now exists, read back rather than assumed.
///
/// A `create` that succeeded is not proof of what the branch looks like: its
/// tip, its upstream and whether HEAD is on it are all things git decided, and
/// inventing them here would be the UI showing a branch nobody read.
async fn created(transport: &LocalTransport, name: &str) -> Result<GitBranch> {
    let branches = transport.branches().await?;
    branches
        .into_iter()
        .find(|branch| branch.name == name && !branch.is_remote)
        .ok_or_else(|| {
            TransportError::shell(format!(
                "the branch `{name}` was created, but git did not list it afterwards"
            ))
        })
}

/// A caller's branch name, refused locally for anything readable as an option
/// and then put to `git check-ref-format` for git's own rules.
async fn checked(root: &str, name: &str) -> Result<String> {
    let name = refname::precheck(name)?;
    // A `check-ref-format` that could not run at all is a git problem rather
    // than a name problem, and `?` lets git's own error be the one reported.
    let checked = run_with_input(root, &refname::check_argv(&name), None).await?;
    if checked.succeeded() {
        return Ok(name);
    }
    Err(refname::refused(&name))
}
