//! The live PTY session registry.
//!
//! Sessions are addressed only by the opaque id the transport minted, and
//! every session is killed on `close`/`close_all` — including on
//! `disconnect` — so closing the app leaves no orphaned child process.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

use portable_pty::MasterPty;

use crate::error::{Result, TransportError};
use crate::types::{PtySession, PtySessionId, PtySize, PtyStream, ShellKind};

use super::pty_spawn::{spawn, SharedChild};

struct LiveSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: SharedChild,
}

/// Everything a caller must decide before a shell is spawned.
pub struct SpawnRequest {
    pub program: String,
    pub shell: ShellKind,
    pub cwd: String,
    pub size: PtySize,
    pub fell_back: bool,
}

#[derive(Default)]
pub struct PtyRegistry {
    sessions: Mutex<HashMap<PtySessionId, LiveSession>>,
}

impl PtyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&self, request: SpawnRequest) -> Result<PtyStream> {
        let size = request.size.sanitised();
        let spawned = spawn(&request.program, &request.cwd, size)?;
        let id = PtySessionId::new();

        self.lock()?.insert(
            id.clone(),
            LiveSession {
                master: spawned.master,
                writer: spawned.writer,
                child: spawned.child,
            },
        );

        Ok(PtyStream {
            session: PtySession {
                id,
                program: request.program,
                shell: request.shell,
                cwd: request.cwd,
                size,
                fell_back: request.fell_back,
            },
            events: spawned.events,
        })
    }

    pub fn write(&self, id: &PtySessionId, data: &str) -> Result<()> {
        let mut sessions = self.lock()?;
        let session = sessions.get_mut(id).ok_or_else(|| not_found(id))?;
        session
            .writer
            .write_all(data.as_bytes())
            .and_then(|_| session.writer.flush())
            .map_err(|e| TransportError::pty(e.to_string()))
    }

    pub fn resize(&self, id: &PtySessionId, size: PtySize) -> Result<()> {
        let size = size.sanitised();
        let sessions = self.lock()?;
        let session = sessions.get(id).ok_or_else(|| not_found(id))?;
        session
            .master
            .resize(portable_pty::PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TransportError::pty(format!("could not resize the terminal: {e}")))
    }

    pub fn close(&self, id: &PtySessionId) -> Result<()> {
        let session = self.lock()?.remove(id).ok_or_else(|| not_found(id))?;
        terminate(session);
        Ok(())
    }

    /// Closes every session. Errors are swallowed on purpose: teardown must
    /// not leave sessions behind because one child was already gone.
    pub fn close_all(&self) {
        let drained: Vec<LiveSession> = match self.lock() {
            Ok(mut sessions) => sessions.drain().map(|(_, session)| session).collect(),
            Err(_) => return,
        };
        for session in drained {
            terminate(session);
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<PtySessionId, LiveSession>>> {
        self.sessions
            .lock()
            .map_err(|_| TransportError::pty("the pty registry lock was poisoned"))
    }
}

impl Drop for PtyRegistry {
    fn drop(&mut self) {
        self.close_all();
    }
}

fn terminate(session: LiveSession) {
    if let Ok(mut child) = session.child.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Dropping the master closes the pty, which ends the reader thread.
    drop(session.writer);
    drop(session.master);
}

fn not_found(id: &PtySessionId) -> TransportError {
    TransportError::PtyNotFound { id: id.to_string() }
}
