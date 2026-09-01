//! SSH agent authentication.
//!
//! The agent is how this app supports encrypted keys without ever handling a
//! passphrase: the private key never leaves the agent, and the app only ever
//! asks it to sign a challenge.
//!
//! The stream type differs per platform - a named pipe on Windows, a Unix
//! socket elsewhere - so the identity loop is generic and the two `connect`
//! wrappers are the only cfg'd code.

use russh::client::Handle;
use russh::keys::agent::client::AgentClient;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::{Result, TransportError};

use super::handler::ClientHandler;

/// Where Windows OpenSSH publishes its agent. Pageant speaks a different
/// protocol and is not attempted.
#[cfg(windows)]
const WINDOWS_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

pub async fn authenticate(handle: &mut Handle<ClientHandler>, user: &str) -> Result<bool> {
    let mut client = connect().await?;
    try_identities(handle, user, &mut client).await
}

#[cfg(windows)]
async fn connect() -> Result<AgentClient<tokio::net::windows::named_pipe::NamedPipeClient>> {
    AgentClient::connect_named_pipe(WINDOWS_AGENT_PIPE)
        .await
        .map_err(|e| {
            TransportError::protocol(format!(
                "no SSH agent to authenticate with ({e}). Start it with \
                 `Start-Service ssh-agent`, add a key with `ssh-add`, or name a key file."
            ))
        })
}

#[cfg(not(windows))]
async fn connect() -> Result<AgentClient<tokio::net::UnixStream>> {
    AgentClient::connect_env().await.map_err(|e| {
        TransportError::protocol(format!(
            "no SSH agent to authenticate with ({e}). Start one and add a key with \
             `ssh-add`, or name a key file."
        ))
    })
}

/// Offers each identity in turn and stops at the first the server accepts.
async fn try_identities<R>(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    client: &mut AgentClient<R>,
) -> Result<bool>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let identities = client.request_identities().await.map_err(|e| {
        TransportError::protocol(format!("the SSH agent refused to list its keys: {e}"))
    })?;

    if identities.is_empty() {
        return Err(TransportError::protocol(
            "the SSH agent is running but holds no keys. Add one with `ssh-add`, \
             or name a key file.",
        ));
    }

    for identity in identities {
        let accepted = handle
            .authenticate_publickey_with(user, identity, client)
            .await
            .map_err(|e| TransportError::protocol(format!("authentication failed: {e}")))?;
        if accepted {
            return Ok(true);
        }
    }
    Ok(false)
}
