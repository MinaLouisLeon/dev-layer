//! Opening the connection: TCP, host key check, then authentication.
//!
//! Two authentication methods, and deliberately no third:
//!
//! - a private key file named by `identity_path`, and
//! - a running SSH agent (see [`super::agent`]).
//!
//! There is no password and no passphrase parameter anywhere in this module,
//! because `ConnectionTarget::Ssh` carries no secret and nothing here may
//! write one to disk or a log. An encrypted key is therefore not decrypted by
//! the app; that is what the agent is for, and the error says so.

use std::sync::{Arc, Mutex};

use russh::client::Handle;
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::load_secret_key;

use crate::error::{Result, TransportError};

/// Ceiling on the TCP connect and handshake. Long enough for a slow link,
/// short enough that a typo does not look like a hang.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

use super::agent;
use super::handler::{verdict_error, ClientHandler, HostKeyVerdict, VerdictSlot};

pub async fn open(
    config: Arc<russh::client::Config>,
    host: &str,
    port: u16,
    user: &str,
    identity_path: Option<&str>,
) -> Result<Handle<ClientHandler>> {
    if user.trim().is_empty() {
        return Err(TransportError::invalid("an SSH user name is required"));
    }
    if host.trim().is_empty() {
        return Err(TransportError::invalid("an SSH host is required"));
    }

    // `None` until the handler runs, so a failure before the host offers a key
    // is reported as what it is rather than as a host key problem.
    let verdict: VerdictSlot = Arc::new(Mutex::new(None));
    let handler = ClientHandler::new(host, port, Arc::clone(&verdict));

    // Without a ceiling of our own, an unroutable address sits on the OS TCP
    // timeout - twenty seconds of an unexplained spinner. This fails in a
    // predictable time with a typed error instead.
    let dial = russh::client::connect(config, (host, port), handler);
    let mut handle = tokio::time::timeout(CONNECT_TIMEOUT, dial)
        .await
        .map_err(|_| TransportError::Timeout {
            operation: format!("connecting to {host}:{port}"),
            ms: CONNECT_TIMEOUT.as_millis() as u64,
        })?
        .map_err(|e| connect_error(e, &verdict, host, port))?;

    let authenticated = match identity_path {
        Some(path) => with_key(&mut handle, user, path).await?,
        None => agent::authenticate(&mut handle, user).await?,
    };

    if !authenticated {
        return Err(TransportError::protocol(match identity_path {
            Some(path) => format!("{host} rejected the key at {path} for user {user}."),
            None => format!(
                "{host} rejected every identity the SSH agent offered for user {user}. \
                 Add the right key with `ssh-add`, or name a key file."
            ),
        }));
    }

    Ok(handle)
}

/// A failed handshake is usually the host key, but russh reports that as a
/// generic error - so the verdict the handler recorded is what gets reported.
///
/// `None` means the handler never ran: the connection was refused, or the
/// address never answered. That is a network failure, not a host key failure,
/// and saying otherwise sends the reader to the wrong file.
fn connect_error(
    err: russh::Error,
    verdict: &VerdictSlot,
    host: &str,
    port: u16,
) -> TransportError {
    let recorded = verdict.lock().map(|v| *v).unwrap_or(None);
    match recorded {
        None | Some(HostKeyVerdict::Trusted) => {
            TransportError::protocol(format!("could not reach {host}:{port}: {err}"))
        }
        Some(other) => verdict_error(other, host, port),
    }
}

async fn with_key(handle: &mut Handle<ClientHandler>, user: &str, path: &str) -> Result<bool> {
    // `None` is the passphrase, and it stays `None`. A key this cannot open is
    // an encrypted key, and `key_error` points at the agent rather than asking
    // for a secret the app has nowhere safe to keep.
    let key = load_secret_key(path, None).map_err(|e| key_error(path, e))?;
    let with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None)
        .map_err(|e| TransportError::protocol(format!("{path}: {e}")))?;
    handle
        .authenticate_publickey(user, with_alg)
        .await
        .map_err(|e| TransportError::protocol(format!("authentication failed: {e}")))
}

fn key_error(path: &str, err: russh::keys::Error) -> TransportError {
    let text = err.to_string();
    let encrypted = text.contains("passphrase")
        || text.contains("Encrypted")
        || text.contains("decrypt")
        || text.contains("crypto error");
    if encrypted {
        return TransportError::protocol(format!(
            "the key at {path} is encrypted, and this app never asks for a passphrase. \
             Load it into your SSH agent with `ssh-add {path}` and connect without \
             naming a key file."
        ));
    }
    TransportError::protocol(format!("could not read the key at {path}: {text}"))
}
