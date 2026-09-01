//! Remote PTY sessions.
//!
//! One SSH channel per session: `request_pty`, then a shell, then a task that
//! drains the channel into the same `PtyEvent` stream the local transport
//! produces. The UI cannot tell the two apart, which is the point of having
//! one transport interface.
//!
//! A russh `Channel` cannot be split - reading needs `&mut self` and writing
//! needs `&self` - so each session is owned outright by one task, and writes,
//! resizes and closes reach it as commands. `Channel::wait` is an mpsc receive
//! and therefore cancel-safe, which is what makes the `select!` below sound.
//!
//! Teardown is the part that matters: `close` ends one session and `close_all`
//! ends every one, so no remote shell outlives the window.

use std::collections::HashMap;
use std::sync::Mutex;

use russh::client::Handle;
use tokio::sync::mpsc;

use crate::error::{Result, TransportError};
use crate::types::{PtyEvent, PtySessionId, PtySize};

use super::handler::ClientHandler;
use super::pty_drive::drive;

/// Matches the local transport's depth, so a chatty remote applies back
/// pressure at the same point.
const CHANNEL_DEPTH: usize = 512;
const COMMAND_DEPTH: usize = 32;
const TERM: &str = "xterm-256color";

pub(super) enum Command {
    Write(String),
    Resize(PtySize),
    Close,
}

#[derive(Default)]
pub struct PtyRegistry {
    sessions: Mutex<HashMap<String, mpsc::Sender<Command>>>,
}

impl PtyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn open(
        &self,
        handle: &Handle<ClientHandler>,
        id: &PtySessionId,
        size: PtySize,
        program: Option<&str>,
    ) -> Result<mpsc::Receiver<PtyEvent>> {
        let size = size.sanitised();
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| TransportError::pty(format!("could not open a session: {e}")))?;

        channel
            .request_pty(
                true,
                TERM,
                u32::from(size.cols),
                u32::from(size.rows),
                0,
                0,
                &[],
            )
            .await
            .map_err(|e| TransportError::pty(format!("the host refused a pty: {e}")))?;

        match program {
            // A named program still runs under the pty, so the terminal
            // behaves the same whether or not nu was found.
            Some(command) => channel.exec(true, command.as_bytes()).await,
            None => channel.request_shell(true).await,
        }
        .map_err(|e| TransportError::pty(format!("could not start the remote shell: {e}")))?;

        let (events_tx, events_rx) = mpsc::channel(CHANNEL_DEPTH);
        let (commands_tx, commands_rx) = mpsc::channel(COMMAND_DEPTH);
        tokio::spawn(drive(channel, commands_rx, events_tx));

        self.table()?.insert(id.0.clone(), commands_tx);
        Ok(events_rx)
    }

    pub async fn write(&self, id: &PtySessionId, data: &str) -> Result<()> {
        self.send(id, Command::Write(data.to_string())).await
    }

    pub async fn resize(&self, id: &PtySessionId, size: PtySize) -> Result<()> {
        self.send(id, Command::Resize(size.sanitised())).await
    }

    pub async fn close(&self, id: &PtySessionId) -> Result<()> {
        let sender = self.table()?.remove(&id.0);
        match sender {
            Some(tx) => {
                let _ = tx.send(Command::Close).await;
                Ok(())
            }
            None => Err(TransportError::PtyNotFound { id: id.0.clone() }),
        }
    }

    /// Best effort by design: `disconnect` must succeed even if a session has
    /// already gone away with the connection.
    pub async fn close_all(&self) {
        let sessions: Vec<_> = match self.table() {
            Ok(mut table) => table.drain().map(|(_, tx)| tx).collect(),
            Err(_) => return,
        };
        for tx in sessions {
            let _ = tx.send(Command::Close).await;
        }
    }

    async fn send(&self, id: &PtySessionId, command: Command) -> Result<()> {
        let sender = self
            .table()?
            .get(&id.0)
            .cloned()
            .ok_or_else(|| TransportError::PtyNotFound { id: id.0.clone() })?;
        sender
            .send(command)
            .await
            .map_err(|_| TransportError::PtyNotFound { id: id.0.clone() })
    }

    fn table(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, mpsc::Sender<Command>>>> {
        self.sessions
            .lock()
            .map_err(|_| TransportError::pty("the session table was poisoned"))
    }
}
