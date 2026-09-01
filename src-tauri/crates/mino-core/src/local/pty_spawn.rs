//! Spawning one PTY and pumping its output into a channel.
//!
//! `portable-pty` is a blocking API, so the read loop lives on a dedicated OS
//! thread and hands chunks to async callers over an mpsc channel. The thread
//! ends when the pty reaches EOF, which happens when the master is dropped in
//! `close`, so no thread outlives its session.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty};
use tokio::sync::mpsc;

use crate::error::{Result, TransportError};
use crate::types::{PtyEvent, PtyExit, PtySize};

/// Output chunks buffered before the reader thread blocks. Deep enough that a
/// burst of terminal output (`cat` on a large file) does not stall the shell.
const CHANNEL_CAPACITY: usize = 512;
const READ_BUFFER_BYTES: usize = 8192;
/// How long the waiter polls for an exit status after EOF before giving up.
const EXIT_POLL_ATTEMPTS: u32 = 100;
const EXIT_POLL_INTERVAL_MS: u64 = 50;
/// Matches what the SSH transport requests, so a shell behaves the same on
/// either transport.
const TERM_NAME: &str = "xterm-256color";

pub type SharedChild = Arc<Mutex<Box<dyn Child + Send + Sync>>>;

pub struct SpawnedPty {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: SharedChild,
    pub events: mpsc::Receiver<PtyEvent>,
}

/// What the child is told about the terminal it is attached to.
///
/// A child inherits this process's environment, and that is the wrong default
/// for a terminal emulator: whatever launched the app decides whether `nu`,
/// `claude` or `git` print in colour. Two cases bite in practice - a parent
/// with no `TERM` at all (normal on Windows) and a parent carrying
/// `NO_COLOR`, which is honoured by most modern CLI tools and turns the pane
/// monochrome for reasons the user cannot see.
///
/// So the pty declares its own terminal, exactly as any other emulator does.
/// xterm.js renders 256 colours and true colour, so it is honest to say so.
fn apply_terminal_env(command: &mut CommandBuilder) {
    command.env("TERM", TERM_NAME);
    command.env("COLORTERM", "truecolor");
    // Inherited opt-outs. The pane is colour-capable regardless of how the
    // app itself was started, so these are cleared rather than passed on.
    command.env_remove("NO_COLOR");
    command.env_remove("CLICOLOR_FORCE");
}

pub fn spawn(program: &str, cwd: &str, size: PtySize) -> Result<SpawnedPty> {
    let size = size.sanitised();
    let pair = native_pty_system()
        .openpty(portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| TransportError::pty(format!("could not open a pty: {e}")))?;

    let mut command = CommandBuilder::new(program);
    command.cwd(cwd);
    apply_terminal_env(&mut command);

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| TransportError::pty(format!("could not start {program}: {e}")))?;
    // Dropped immediately: holding the slave open would keep the pty alive
    // after the child exits and the reader would never see EOF.
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| TransportError::pty(format!("could not read from the pty: {e}")))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| TransportError::pty(format!("could not write to the pty: {e}")))?;

    let child: SharedChild = Arc::new(Mutex::new(child));
    let (tx, events) = mpsc::channel(CHANNEL_CAPACITY);
    let waiter = Arc::clone(&child);
    std::thread::Builder::new()
        .name("mino-pty-reader".to_string())
        .spawn(move || pump(reader, waiter, tx))
        .map_err(|e| TransportError::pty(format!("could not start the pty reader: {e}")))?;

    Ok(SpawnedPty {
        master: pair.master,
        writer,
        child,
        events,
    })
}

fn pump(mut reader: Box<dyn Read + Send>, child: SharedChild, tx: mpsc::Sender<PtyEvent>) {
    let mut buffer = [0u8; READ_BUFFER_BYTES];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let chunk = String::from_utf8_lossy(&buffer[..read]).into_owned();
                if tx.blocking_send(PtyEvent::Output(chunk)).is_err() {
                    // Receiver gone: the session was closed. Stop quietly.
                    return;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => {
                let _ = tx.blocking_send(PtyEvent::Error(err.to_string()));
                break;
            }
        }
    }
    let _ = tx.blocking_send(PtyEvent::Exit(wait_for_exit(&child)));
}

fn wait_for_exit(child: &SharedChild) -> PtyExit {
    for _ in 0..EXIT_POLL_ATTEMPTS {
        let status = child
            .lock()
            .ok()
            .and_then(|mut c| c.try_wait().ok().flatten());
        if let Some(status) = status {
            return PtyExit {
                code: Some(status.exit_code() as i32),
                success: status.success(),
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(EXIT_POLL_INTERVAL_MS));
    }
    PtyExit {
        code: None,
        success: false,
    }
}
