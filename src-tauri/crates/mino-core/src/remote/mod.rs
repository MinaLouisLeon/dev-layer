//! The remote-agent transport - scaffolded, not built.
//!
//! Talks to a `mino-agent` daemon over WebSocket. Every method answers
//! `TransportError::Unimplemented { transport: RemoteAgent }`.
//!
//! Phase 2 plan: one request frame per trait method, one response frame per
//! result, and `PtyEvent` pushed unsolicited on the same socket. The frame
//! schema is written down once, in `crates/mino-agent/src/protocol.rs`, so
//! both ends are built against one definition.
//!
//! Blocking open task: the agent has no authentication yet, which is why it
//! binds to loopback only. This transport must not be pointed at a
//! non-loopback agent until the token handshake exists. See
//! docs/mino-workbench/endpoints.md.

use crate::types::TransportKind;
use crate::{unimplemented_git_transport, unimplemented_github_transport, unimplemented_transport};

pub struct RemoteAgentTransport {
    url: std::sync::RwLock<Option<String>>,
}

impl Default for RemoteAgentTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteAgentTransport {
    pub fn new() -> Self {
        Self {
            url: std::sync::RwLock::new(None),
        }
    }

    /// Phase 2 entry point: opens the WebSocket to the agent.
    ///
    /// Not called yet - `connect` returns `Unimplemented` - but written
    /// against the real client so the dependency and the call shape are
    /// settled rather than guessed at later.
    #[allow(dead_code)]
    async fn dial(url: &str) -> crate::Result<()> {
        let (stream, _response) = tokio_tungstenite::connect_async(url).await.map_err(|e| {
            crate::TransportError::Protocol {
                message: format!("could not reach the agent at {url}: {e}"),
            }
        })?;
        drop(stream);
        Ok(())
    }

    /// The agent URL a future `connect` stores.
    pub fn url(&self) -> Option<String> {
        self.url.read().ok().and_then(|url| url.clone())
    }
}

unimplemented_transport!(RemoteAgentTransport, TransportKind::RemoteAgent);
unimplemented_git_transport!(RemoteAgentTransport, TransportKind::RemoteAgent);
unimplemented_github_transport!(RemoteAgentTransport, TransportKind::RemoteAgent);
