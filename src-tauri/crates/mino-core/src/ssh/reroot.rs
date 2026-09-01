//! Changing the working folder on a connection that is already open.
//!
//! Picking a directory in the workbench is a re-connect to the same host. A
//! fresh handshake for that would re-authenticate - and ask the SSH agent
//! again - to do what is really a re-pin of the root, so this reuses the live
//! connection whenever the endpoint has not changed.

use crate::error::Result;
use crate::types::{ConnectionInfo, ConnectionTarget, TransportKind};

use super::connect::pin_root;
use super::roots::base_name;
use super::{Endpoint, SshTransport};

impl SshTransport {
    /// Re-pins the root on the live connection when `target` names the same
    /// host, user and identity as the current session.
    ///
    /// Returns `None` when there is nothing to reuse - not connected, or a
    /// different endpoint - and the caller falls back to a full connect.
    ///
    /// PTY sessions are deliberately left alone: the shells keep running with
    /// their own working directories, exactly as they would if someone typed
    /// `cd` in them. Only the pane's root moves.
    pub(super) async fn reroot(&self, target: &ConnectionTarget) -> Result<Option<ConnectionInfo>> {
        let ConnectionTarget::Ssh {
            host,
            port,
            user,
            root,
            identity_path,
        } = target
        else {
            return Ok(None);
        };
        let wanted = Endpoint {
            host: host.clone(),
            port: *port,
            user: user.clone(),
            identity_path: identity_path.clone(),
        };

        let mut guard = self.state.write().await;
        let Some(connected) = guard.as_mut() else {
            return Ok(None);
        };
        if connected.endpoint != wanted {
            return Ok(None);
        }

        let (pinned, resolved) = pin_root(&connected.sftp, root.as_deref()).await?;
        connected.root = pinned;
        Ok(Some(ConnectionInfo {
            id: uuid::Uuid::new_v4().to_string(),
            kind: TransportKind::Ssh,
            root: resolved.clone(),
            label: format!("{} ({user}@{host})", base_name(&resolved)),
        }))
    }
}
