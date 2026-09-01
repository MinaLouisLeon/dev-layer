//! Terminal sessions.
//!
//! The PTY itself is `portable-pty` (wezterm's), which wraps ConPTY properly
//! on Windows — pseudo-console setup is fiddly, security-sensitive, and
//! thoroughly solved already.
//!
//! Each session owns two threads: one draining the PTY into `terminal::output`
//! events, one waiting on the child so an exited shell removes its own session.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{Error, Result};

pub const OUTPUT_EVENT: &str = "terminal::output";
pub const CLOSED_EVENT: &str = "terminal::closed";

/// PTY reads are chunked; 8 KiB keeps `cargo build` output smooth without
/// flooding the event bridge.
const READ_CHUNK: usize = 8192;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutput {
    pub id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalClosed {
    pub id: String,
    pub exit_code: u32,
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default)]
pub struct TerminalSessions {
    sessions: Mutex<HashMap<String, Session>>,
    next_id: AtomicU64,
}

impl TerminalSessions {
    pub fn open(
        self: &std::sync::Arc<Self>,
        app: &AppHandle,
        cols: u16,
        rows: u16,
        shell: Option<String>,
        cwd: Option<PathBuf>,
    ) -> Result<String> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Platform(format!("openpty: {e}")))?;

        let program = shell.unwrap_or_else(default_shell);
        let mut command = CommandBuilder::new(&program);
        if let Some(dir) = cwd.filter(|d| d.is_dir()) {
            command.cwd(dir);
        }

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| Error::Platform(format!("spawn {program}: {e}")))?;
        // The slave handle must be dropped or the reader never sees EOF when
        // the shell exits.
        drop(pair.slave);

        let killer = child.clone_killer();
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| Error::Platform(format!("pty reader: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| Error::Platform(format!("pty writer: {e}")))?;

        let id = format!("term-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        self.sessions.lock().insert(
            id.clone(),
            Session {
                master: pair.master,
                writer,
                killer,
            },
        );

        spawn_reader(app.clone(), id.clone(), reader);

        // Waiter: an exited shell tears down its own session.
        let waiter_app = app.clone();
        let waiter_id = id.clone();
        let waiter_sessions = std::sync::Arc::clone(self);
        std::thread::Builder::new()
            .name(format!("dev-layer/{id}-wait"))
            .spawn(move || {
                let status = child.wait().map(|s| s.exit_code()).unwrap_or(1);
                waiter_sessions.sessions.lock().remove(&waiter_id);
                let _ = waiter_app.emit(
                    CLOSED_EVENT,
                    TerminalClosed {
                        id: waiter_id,
                        exit_code: status,
                    },
                );
            })
            .map_err(|e| Error::Platform(e.to_string()))?;

        tracing::info!(id, program, "terminal opened");
        Ok(id)
    }

    pub fn write(&self, id: &str, data: &str) -> Result<()> {
        let mut sessions = self.sessions.lock();
        let session = sessions
            .get_mut(id)
            .ok_or_else(|| Error::Platform(format!("no session {id}")))?;
        session
            .writer
            .write_all(data.as_bytes())
            .and_then(|()| session.writer.flush())
            .map_err(|e| Error::Platform(format!("write: {e}")))
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let sessions = self.sessions.lock();
        let session = sessions
            .get(id)
            .ok_or_else(|| Error::Platform(format!("no session {id}")))?;
        session
            .master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Platform(format!("resize: {e}")))
    }

    pub fn close(&self, id: &str) {
        if let Some(mut session) = self.sessions.lock().remove(id) {
            let _ = session.killer.kill();
            tracing::info!(id, "terminal closed");
        }
    }

    /// Registered with `safety`: a crash must not leave orphaned shells behind.
    pub fn close_all(&self) {
        let sessions = std::mem::take(&mut *self.sessions.lock());
        if sessions.is_empty() {
            return;
        }
        tracing::info!(count = sessions.len(), "killing terminal sessions");
        for (_, mut session) in sessions {
            let _ = session.killer.kill();
        }
    }

    pub fn ids(&self) -> Vec<String> {
        self.sessions.lock().keys().cloned().collect()
    }
}

fn spawn_reader(app: AppHandle, id: String, mut reader: Box<dyn Read + Send>) {
    std::thread::Builder::new()
        .name(format!("dev-layer/{id}-read"))
        .spawn(move || {
            let mut buffer = [0u8; READ_CHUNK];
            let mut pending: Vec<u8> = Vec::new();

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // shell exited
                    Ok(count) => {
                        let text = decode_stream(&mut pending, &buffer[..count]);
                        if !text.is_empty() {
                            let _ = app.emit(
                                OUTPUT_EVENT,
                                TerminalOutput {
                                    id: id.clone(),
                                    data: text,
                                },
                            );
                        }
                    }
                    Err(e) => {
                        tracing::debug!(id, error = %e, "pty read ended");
                        break;
                    }
                }
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| tracing::error!(error = %e, "could not start pty reader"));
}

/// Decodes a byte chunk as UTF-8, carrying an incomplete trailing sequence over
/// to the next chunk.
///
/// PTY reads split wherever the buffer fills, which lands mid-character often
/// enough to matter — `from_utf8_lossy` per chunk would sprinkle replacement
/// characters through any non-ASCII output.
fn decode_stream(pending: &mut Vec<u8>, chunk: &[u8]) -> String {
    pending.extend_from_slice(chunk);
    let mut out = String::new();

    loop {
        match std::str::from_utf8(pending) {
            Ok(text) => {
                out.push_str(text);
                pending.clear();
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                out.push_str(&String::from_utf8_lossy(&pending[..valid]));

                match error.error_len() {
                    // Genuinely invalid bytes: emit a replacement and keep
                    // decoding, so good text after them is not held hostage.
                    Some(bad) => {
                        out.push('\u{fffd}');
                        pending.drain(..valid + bad);
                    }
                    // Truncated sequence: keep the tail for the next chunk. A
                    // UTF-8 character is at most 4 bytes, so a longer remainder
                    // is garbage rather than a partial character.
                    None => {
                        pending.drain(..valid);
                        if pending.len() > 4 {
                            pending.clear();
                        }
                        break;
                    }
                }
            }
        }
    }
    out
}

/// PowerShell 7 if installed, then Windows PowerShell, then whatever `COMSPEC`
/// says — matching what a developer would expect their terminal to open.
fn default_shell() -> String {
    #[cfg(windows)]
    {
        for candidate in ["pwsh.exe", "powershell.exe"] {
            if on_path(candidate) {
                return candidate.to_string();
            }
        }
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

#[cfg(windows)]
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::decode_stream;

    #[test]
    fn passes_ascii_straight_through() {
        let mut pending = Vec::new();
        assert_eq!(
            decode_stream(&mut pending, b"cargo build\r\n"),
            "cargo build\r\n"
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn carries_a_split_character_across_chunks() {
        // The box-drawing character cargo uses in progress bars, split in two.
        let full = "▔ done".as_bytes();
        let (head, tail) = full.split_at(2);

        let mut pending = Vec::new();
        assert_eq!(decode_stream(&mut pending, head), "");
        assert_eq!(pending.len(), 2, "incomplete sequence must be held");
        assert_eq!(decode_stream(&mut pending, tail), "▔ done");
        assert!(pending.is_empty());
    }

    #[test]
    fn emits_the_valid_prefix_immediately() {
        let mut pending = Vec::new();
        let mut chunk = b"ok ".to_vec();
        chunk.extend_from_slice(&"▔".as_bytes()[..1]);
        assert_eq!(decode_stream(&mut pending, &chunk), "ok ");
    }

    #[test]
    fn drops_invalid_bytes_instead_of_stalling() {
        let mut pending = Vec::new();
        assert_eq!(
            decode_stream(&mut pending, &[0xff, b'h', b'i']),
            "\u{fffd}hi"
        );
        assert!(pending.is_empty());
    }
}
