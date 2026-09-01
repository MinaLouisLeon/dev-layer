//! Host key verification.
//!
//! russh's default `check_server_key` rejects everything, and the tempting
//! shortcut is to return `Ok(true)` and move on. That would hand every session
//! to anyone who can answer on the address, so it is not done here: the key is
//! checked against the user's `known_hosts` and an unrecognised host is a
//! typed error the UI can explain.
//!
//! Trust-on-first-use is deliberately *not* implemented. Learning a key is a
//! decision for the person, not for the app, and it already has a tool:
//! `ssh-keyscan`. See `docs/mino-workbench/transport-layer-module.md`.

use russh::keys::known_hosts::check_known_hosts;

use crate::error::TransportError;

/// Why a host key was refused. Carried out of the handler because russh only
/// lets `check_server_key` answer yes or no.
///
/// The slot holding this starts as `None`, meaning the handler never ran - the
/// handshake failed before the host offered a key. That distinction matters:
/// reporting a host key problem for what is really a refused connection sends
/// the reader to the wrong file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyVerdict {
    Trusted,
    Unknown,
    Changed,
    Unreadable,
}

/// Shared between the handler and the code that reports the failure.
pub type VerdictSlot = std::sync::Arc<std::sync::Mutex<Option<HostKeyVerdict>>>;

pub struct ClientHandler {
    host: String,
    port: u16,
    verdict: VerdictSlot,
}

impl ClientHandler {
    pub fn new(host: &str, port: u16, verdict: VerdictSlot) -> Self {
        Self {
            host: host.to_string(),
            port,
            verdict,
        }
    }

    fn record(&self, verdict: HostKeyVerdict) {
        if let Ok(mut slot) = self.verdict.lock() {
            *slot = Some(verdict);
        }
    }
}

#[async_trait::async_trait]
impl russh::client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        match check_known_hosts(&self.host, self.port, server_public_key) {
            Ok(true) => {
                self.record(HostKeyVerdict::Trusted);
                Ok(true)
            }
            Ok(false) => {
                self.record(HostKeyVerdict::Unknown);
                Ok(false)
            }
            // `KeyChanged` is the one that matters: the host answered with a
            // different key than the one on record.
            Err(russh::keys::Error::KeyChanged { .. }) => {
                self.record(HostKeyVerdict::Changed);
                Ok(false)
            }
            Err(_) => {
                self.record(HostKeyVerdict::Unreadable);
                Ok(false)
            }
        }
    }
}

/// Turns a refusal into a sentence naming the fix. The fingerprint is not
/// included: printing it next to "trust this?" trains people to accept it.
pub fn verdict_error(verdict: HostKeyVerdict, host: &str, port: u16) -> TransportError {
    match verdict {
        HostKeyVerdict::Trusted => {
            TransportError::protocol("the host key was accepted but the session still failed")
        }
        HostKeyVerdict::Unknown => TransportError::protocol(format!(
            "the host key for {host}:{port} is not in your known_hosts file. \
             Add it with `ssh-keyscan -p {port} {host} >> ~/.ssh/known_hosts` \
             after checking the fingerprint with whoever runs the host."
        )),
        HostKeyVerdict::Changed => TransportError::protocol(format!(
            "the host key for {host}:{port} does not match the one recorded in \
             your known_hosts file. This is what a machine-in-the-middle looks \
             like. Do not connect until you know why it changed."
        )),
        HostKeyVerdict::Unreadable => TransportError::protocol(
            "your known_hosts file could not be read, so the host key could not \
             be verified.",
        ),
    }
}
