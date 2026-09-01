//! `impl GitStashTransport for SshTransport`.
//!
//! The mirror of `local/git_stash.rs`. One thing genuinely differs, and it is
//! documented rather than hidden: a **stash message travels in argv**, because
//! `git stash push -m` has no stdin form the way `git commit --file -` does.
//!
//! Over SSH the argv becomes a command line, and `super::command::quote`
//! refuses a value containing a single quote rather than escaping it. So a
//! stash message with an apostrophe in it is a typed error on a remote target
//! and works locally. That is the documented limit of this transport - the
//! same one that applies to a remote filename containing a quote - and the
//! error says so rather than mangling the command.

use async_trait::async_trait;

use crate::error::{Result, TransportError};
use crate::git::{self, command};
use crate::transport::GitStashTransport;
use crate::types::{GitStash, StashRequest, MAX_STASH_MESSAGE_BYTES};

use super::git_run::run_with_input;
use super::SshTransport;

#[async_trait]
impl GitStashTransport for SshTransport {
    async fn stash_list(&self) -> Result<Vec<GitStash>> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let argv = command::stash_list_argv();
        let output = run_with_input(&connected.handle, &root, &argv, None).await?;
        git::stash::parse(&output)
    }

    async fn stash_push(&self, request: StashRequest) -> Result<()> {
        validate(&request)?;
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let argv = command::stash_push_argv(request.trimmed(), request.include_untracked);
        // Not `expect`: a `stash push` that stashed nothing still **exits
        // zero**. See `git::stash::pushed`.
        let output = run_with_input(&connected.handle, &root, &argv, None).await?;
        git::stash::pushed(&output)
    }

    async fn stash_apply(&self, index: u32, pop: bool) -> Result<()> {
        let what = if pop { "stash pop" } else { "stash apply" };
        self.expect(command::stash_apply_argv(index, pop), what)
            .await
    }

    async fn stash_drop(&self, index: u32) -> Result<()> {
        self.expect(command::stash_drop_argv(index), "stash drop")
            .await
    }
}

impl SshTransport {
    /// Runs one stash call and turns a non-zero exit into the sentence worth
    /// showing - which for a conflict is "the entry is still on the stack".
    async fn expect(&self, argv: Vec<String>, what: &str) -> Result<()> {
        let connected = self.connected().await?;
        let root = connected.root.root().to_string();
        let output = run_with_input(&connected.handle, &root, &argv, None).await?;
        if output.succeeded() {
            return Ok(());
        }
        Err(git::stash::failure(&output, what))
    }
}

/// Checked before anything is sent, so an absurd message costs no round trip.
fn validate(request: &StashRequest) -> Result<()> {
    let length = request.message.as_deref().unwrap_or_default().len();
    if length > MAX_STASH_MESSAGE_BYTES {
        return Err(TransportError::invalid(format!(
            "that stash message is {length} bytes, above the \
             {MAX_STASH_MESSAGE_BYTES} byte ceiling"
        )));
    }
    Ok(())
}
