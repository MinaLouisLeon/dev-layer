//! Establishing a session: authenticate, open SFTP, pin the root, probe.
//!
//! Split out of `transport_impl` because it is the one method with a sequence
//! worth reading on its own. The order matters and is not arbitrary:
//! authenticate before anything is opened, canonicalise the root before it is
//! trusted, and prove it is a directory before anything can list it.

use std::sync::Arc;

use russh::client::Handle;
use russh_sftp::client::SftpSession;

use crate::error::{Result, TransportError};
use crate::types::{ConnectionInfo, ConnectionTarget, TransportKind};

use super::handler::ClientHandler;
use super::roots::{base_name, RemoteRoot};
use super::{fs, probe, session, Connected, Endpoint};

/// What `SFTP realpath` resolves to the account's home directory. Used when no
/// root was named, which is every fresh connection.
const REMOTE_HOME: &str = ".";

pub(super) async fn establish(
    config: Arc<russh::client::Config>,
    target: &ConnectionTarget,
) -> Result<(ConnectionInfo, Connected)> {
    let ConnectionTarget::Ssh {
        host,
        port,
        user,
        root,
        identity_path,
    } = target
    else {
        return Err(TransportError::invalid(
            "the SSH transport was handed a target for another transport",
        ));
    };

    let handle = session::open(config, host, *port, user, identity_path.as_deref()).await?;
    let sftp = open_sftp(&handle).await?;
    let (guard, _) = pin_root(&sftp, root.as_deref()).await?;
    let shell = probe::probe(&handle).await?;
    let info = ConnectionInfo {
        id: uuid::Uuid::new_v4().to_string(),
        kind: TransportKind::Ssh,
        root: guard.root().to_string(),
        label: format!("{} ({user}@{host})", base_name(guard.root())),
    };

    Ok((
        info,
        Connected {
            handle,
            sftp,
            root: guard,
            shell,
            endpoint: Endpoint {
                host: host.clone(),
                port: *port,
                user: user.clone(),
                identity_path: identity_path.clone(),
            },
        },
    ))
}

/// Resolves a requested root and proves it is a directory.
///
/// `None` means "wherever this account lands", which SFTP answers with
/// `realpath(".")`. Splitting this out is what lets a folder change re-pin the
/// root without opening a second connection.
pub(super) async fn pin_root(
    sftp: &SftpSession,
    requested: Option<&str>,
) -> Result<(RemoteRoot, String)> {
    let asked = requested.unwrap_or(REMOTE_HOME);
    // `realpath` is what turns `.`, `~/src` or a relative path into the
    // absolute one every later containment check is made against.
    let canonical = sftp
        .canonicalize(asked.to_string())
        .await
        .map_err(|e| fs::map_error(asked, e))?;
    let guard = RemoteRoot::new(&canonical)?;

    let meta = sftp
        .metadata(guard.root().to_string())
        .await
        .map_err(|e| fs::map_error(asked, e))?;
    if !meta.is_dir() {
        return Err(TransportError::invalid(format!(
            "{asked} is not a directory"
        )));
    }
    let root = guard.root().to_string();
    Ok((guard, root))
}

/// Opens the SFTP subsystem on its own channel, kept for the connection's life
/// so that every later file call is one round trip rather than a new channel.
async fn open_sftp(handle: &Handle<ClientHandler>) -> Result<SftpSession> {
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| TransportError::protocol(format!("could not open a session: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| TransportError::protocol(format!("the host refused SFTP: {e}")))?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| TransportError::protocol(format!("could not start SFTP: {e}")))
}
