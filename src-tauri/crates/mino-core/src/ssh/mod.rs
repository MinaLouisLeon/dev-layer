//! The SSH transport.
//!
//! Filesystem work goes over SFTP and shell work over SSH channels, which is
//! the remote counterpart of the local transport's split between `std::fs` and
//! a PTY. Both halves ride one authenticated connection.
//!
//! Three things are worth knowing before reading further:
//!
//! - **Host keys are checked.** [`handler`] verifies against `known_hosts` and
//!   refuses an unknown or changed key. There is no accept-anything mode.
//! - **No secret is ever held.** Authentication is a key file or an agent
//!   ([`session`], [`agent`]); nothing here reads a password or a passphrase,
//!   so there is nothing to leak to disk, a log or browser storage.
//! - **Listings are structured, not scraped.** `list_dir` and `stat` use SFTP
//!   rather than parsing `ls` output. That is a deliberate difference from the
//!   local transport, which prefers the Nushell channel: SFTP is always there,
//!   returns real metadata, and does not depend on `nu` being installed
//!   remotely. `run_structured` still drives Nushell, for the breadcrumb and
//!   anything else that wants typed values.

mod agent;
mod command;
mod connect;
mod exec;
mod fs;
mod git;
mod git_branches;
mod git_conflicts;
mod git_guard;
mod git_history;
mod git_remote;
mod git_run;
mod git_stash;
mod github;
mod github_run;
mod handler;
mod probe;
mod pty;
mod pty_drive;
mod pty_open;
mod read;
mod reroot;
mod roots;
mod search;
mod session;
mod structured;
mod transport_impl;
mod write;

use std::sync::Arc;

use russh::client::Handle;
use russh_sftp::client::SftpSession;
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::error::{Result, TransportError};
use crate::types::ShellProbe;

use handler::ClientHandler;
use pty::PtyRegistry;
use roots::RemoteRoot;

/// Identifies which host a live connection is to, so that changing the folder
/// can reuse it instead of authenticating again. Carries no secret: the
/// identity is a *path* to a key, never key material.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Endpoint {
    host: String,
    port: u16,
    user: String,
    identity_path: Option<String>,
}

/// Everything that exists only while connected.
struct Connected {
    handle: Handle<ClientHandler>,
    sftp: SftpSession,
    root: RemoteRoot,
    shell: ShellProbe,
    endpoint: Endpoint,
}

pub struct SshTransport {
    /// Handed to `russh::client::connect`. Built once and reused so every
    /// session negotiates with the same algorithm set.
    config: Arc<russh::client::Config>,
    state: RwLock<Option<Connected>>,
    ptys: PtyRegistry,
}

impl Default for SshTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl SshTransport {
    pub fn new() -> Self {
        Self {
            config: Arc::new(russh::client::Config::default()),
            state: RwLock::new(None),
            ptys: PtyRegistry::new(),
        }
    }

    pub fn config(&self) -> &russh::client::Config {
        &self.config
    }

    /// Borrows the live connection, or returns `NotConnected`.
    ///
    /// Every method starts with this, so "are we connected?" is asked in one
    /// place and no call site can forget it. The returned guard is held for
    /// the duration of the call, which is what keeps the session alive
    /// underneath a long transfer.
    async fn connected(&self) -> Result<RwLockReadGuard<'_, Connected>> {
        RwLockReadGuard::try_map(self.state.read().await, Option::as_ref)
            .map_err(|_| TransportError::NotConnected)
    }
}
