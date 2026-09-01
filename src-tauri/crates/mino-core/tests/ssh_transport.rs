//! The SSH transport's contract, as far as it can be checked without a host.
//!
//! These cover the half that is pure logic and reachable offline: the
//! not-connected contract, target validation, and the guards that must fire
//! before anything touches the network. Anything needing a real server is a
//! manual case - see TC-61 onward in docs/mino-workbench/manual-testing.md.

use mino_core::types::{
    ConnectionTarget, PtySessionId, PtySize, PtySpawnSpec, ReadFileOptions, StructuredRequest,
    TransportKind,
};
use mino_core::{SshTransport, Transport, TransportError};

fn ssh_target(host: &str, user: &str) -> ConnectionTarget {
    ConnectionTarget::Ssh {
        host: host.to_string(),
        port: 22,
        user: user.to_string(),
        root: Some("/srv".to_string()),
        identity_path: None,
    }
}

#[tokio::test]
async fn it_reports_the_ssh_kind() {
    assert_eq!(SshTransport::new().kind(), TransportKind::Ssh);
}

/// Every call before `connect` is `NotConnected` - never `Unimplemented`, and
/// never a panic. This is the same contract the local transport keeps.
#[tokio::test]
async fn every_call_before_connect_is_not_connected() {
    let transport = SshTransport::new();
    let id = PtySessionId::new();
    let size = PtySize { cols: 80, rows: 24 };

    let not_connected = |err: TransportError| matches!(err, TransportError::NotConnected);

    assert!(not_connected(transport.list_dir("/").await.unwrap_err()));
    assert!(not_connected(transport.stat("/").await.unwrap_err()));
    assert!(not_connected(
        transport
            .read_file("/x", ReadFileOptions::default())
            .await
            .unwrap_err()
    ));
    assert!(not_connected(
        transport
            .open_pty(PtySpawnSpec { cwd: None, size })
            .await
            .unwrap_err()
    ));
    assert!(not_connected(
        transport.write_pty(&id, "ls").await.unwrap_err()
    ));
    assert!(not_connected(
        transport.resize_pty(&id, size).await.unwrap_err()
    ));
    assert!(not_connected(transport.close_pty(&id).await.unwrap_err()));
    assert!(not_connected(
        transport
            .run_structured(StructuredRequest::new("ls | to json"))
            .await
            .unwrap_err()
    ));
    assert!(not_connected(transport.probe_shell().await.unwrap_err()));
}

/// Disconnecting when nothing is connected is a no-op, because the window can
/// close before a connection was ever made.
#[tokio::test]
async fn disconnect_is_safe_when_idle() {
    let transport = SshTransport::new();
    assert!(transport.disconnect().await.is_ok());
    assert!(transport.disconnect().await.is_ok());
}

/// Handing the SSH transport a local target is a programming error, and it
/// says so rather than half-connecting.
#[tokio::test]
async fn it_refuses_a_target_for_another_transport() {
    let transport = SshTransport::new();
    let target = ConnectionTarget::Local {
        root: "/srv".to_string(),
    };
    let err = transport.connect(&target).await.unwrap_err();
    assert!(matches!(err, TransportError::InvalidArgument { .. }));
}

/// An empty user or host is caught before a socket is opened, so a typo in the
/// form is a message rather than a connection timeout.
#[tokio::test]
async fn it_validates_the_target_before_dialling() {
    let transport = SshTransport::new();

    let err = transport
        .connect(&ssh_target("example.invalid", "  "))
        .await;
    assert!(matches!(
        err.unwrap_err(),
        TransportError::InvalidArgument { .. }
    ));

    let err = transport.connect(&ssh_target("   ", "nu")).await;
    assert!(matches!(
        err.unwrap_err(),
        TransportError::InvalidArgument { .. }
    ));
}
