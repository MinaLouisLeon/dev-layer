//! The task that owns one remote PTY channel.
//!
//! Split from [`super::pty`] only to keep both files readable; the reasoning
//! for the design is written down there.
//!
//! `Channel::wait` is `mpsc::Receiver::recv` underneath, so losing the race in
//! `select!` cannot drop a message - which is what makes it safe to await
//! commands and channel traffic in the same loop.

use russh::client::Msg;
use russh::{Channel, ChannelMsg};
use tokio::sync::mpsc;

use crate::types::{PtyEvent, PtyExit};

use super::pty::Command;

pub(super) async fn drive(
    mut channel: Channel<Msg>,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<PtyEvent>,
) {
    let mut code: Option<u32> = None;
    loop {
        tokio::select! {
            message = channel.wait() => {
                if !handle_message(message, &events, &mut code).await {
                    break;
                }
            }
            command = commands.recv() => {
                if !handle_command(command, &channel).await {
                    break;
                }
            }
        }
    }

    let _ = channel.close().await;
    let code = code.map(|c| c as i32);
    let _ = events
        .send(PtyEvent::Exit(PtyExit {
            code,
            success: code == Some(0),
        }))
        .await;
}

/// Returns false when the session is over.
async fn handle_message(
    message: Option<ChannelMsg>,
    events: &mpsc::Sender<PtyEvent>,
    code: &mut Option<u32>,
) -> bool {
    match message {
        // Extended data type 1 is stderr. A terminal shows it inline, exactly
        // as a local shell would.
        Some(ChannelMsg::Data { ref data })
        | Some(ChannelMsg::ExtendedData { ref data, ext: 1 }) => {
            let text = String::from_utf8_lossy(data).into_owned();
            events.send(PtyEvent::Output(text)).await.is_ok()
        }
        Some(ChannelMsg::ExitStatus { exit_status }) => {
            *code = Some(exit_status);
            true
        }
        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => false,
        Some(_) => true,
    }
}

/// Returns false when the session should end.
async fn handle_command(command: Option<Command>, channel: &Channel<Msg>) -> bool {
    match command {
        Some(Command::Write(data)) => channel.data(data.as_bytes()).await.is_ok(),
        Some(Command::Resize(size)) => {
            // A refused resize is not worth ending the session over: the
            // remote simply keeps the old geometry.
            let _ = channel
                .window_change(u32::from(size.cols), u32::from(size.rows), 0, 0)
                .await;
            true
        }
        // Both arms mean nobody is holding the session open any more.
        Some(Command::Close) | None => false,
    }
}
