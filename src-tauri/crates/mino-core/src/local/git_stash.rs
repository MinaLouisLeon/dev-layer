//! `impl GitStashTransport for LocalTransport`.
//!
//! The smallest of the three surfaces, and the one with the least to decide:
//! an index is a number this crate formats itself, so the only caller value
//! that is text is the message, and it is validated for length before it goes
//! anywhere.

use async_trait::async_trait;

use crate::error::Result;
use crate::git::{self, command};
use crate::transport::GitStashTransport;
use crate::types::{GitStash, StashRequest, MAX_STASH_MESSAGE_BYTES};

use super::git_run::run_with_input;
use super::LocalTransport;

#[async_trait]
impl GitStashTransport for LocalTransport {
    async fn stash_list(&self) -> Result<Vec<GitStash>> {
        let root = self.guard()?.root_display();
        let output = run_with_input(&root, &command::stash_list_argv(), None).await?;
        git::stash::parse(&output)
    }

    async fn stash_push(&self, request: StashRequest) -> Result<()> {
        validate(&request)?;
        let root = self.guard()?.root_display();
        let argv = command::stash_push_argv(request.trimmed(), request.include_untracked);
        // Not `expect`: a `stash push` that stashed nothing still **exits
        // zero**, and reporting that as success would send the reader looking
        // for work on a stack that has no such entry.
        git::stash::pushed(&run_with_input(&root, &argv, None).await?)
    }

    async fn stash_apply(&self, index: u32, pop: bool) -> Result<()> {
        let root = self.guard()?.root_display();
        let argv = command::stash_apply_argv(index, pop);
        expect(&root, argv, if pop { "stash pop" } else { "stash apply" }).await
    }

    async fn stash_drop(&self, index: u32) -> Result<()> {
        let root = self.guard()?.root_display();
        expect(&root, command::stash_drop_argv(index), "stash drop").await
    }
}

/// Checked before git is spawned, so an absurd message costs nothing.
///
/// There is no emptiness check to make: an absent message is the ordinary
/// case, and [`StashRequest::trimmed`] already turns a message of spaces into
/// one, rather than handing git an empty `-m`.
pub(super) fn validate(request: &StashRequest) -> Result<()> {
    let length = request.message.as_deref().unwrap_or_default().len();
    if length > MAX_STASH_MESSAGE_BYTES {
        return Err(crate::error::TransportError::invalid(format!(
            "that stash message is {length} bytes, above the \
             {MAX_STASH_MESSAGE_BYTES} byte ceiling"
        )));
    }
    Ok(())
}

/// Runs one stash call and turns a non-zero exit into the sentence worth
/// showing - which for a conflict is "the entry is still on the stack".
async fn expect(root: &str, argv: Vec<String>, what: &str) -> Result<()> {
    let output = run_with_input(root, &argv, None).await?;
    if output.succeeded() {
        return Ok(());
    }
    Err(git::stash::failure(&output, what))
}
