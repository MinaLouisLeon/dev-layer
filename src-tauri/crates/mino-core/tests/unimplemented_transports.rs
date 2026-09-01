//! Proves the interface holds against a transport that is declared but not
//! built: the remote-agent transport compiles, answers every method, and never
//! panics.
//!
//! The git and GitHub halves of the walk are in `fixture::unbuilt_git`,
//! because `GitTransport` now has three surfaces on it and enumerating all of
//! them - plus the two GitHub methods beside them - is the largest thing here
//! without being what this file is about.
//!
//! The SSH transport used to be asserted here too. It is implemented now, so
//! its contract moved to `ssh_transport.rs` - an unbuilt method there would be
//! a regression, not the expected answer.

use mino_core::types::{
    ConnectionTarget, PtySessionId, PtySize, PtySpawnSpec, ReadFileOptions, SearchQuery,
    StructuredRequest, TransportKind,
};
use mino_core::{RemoteAgentTransport, Transport};

mod fixture;

use fixture::unbuilt_git::is_unimplemented;

async fn assert_every_method_unimplemented(
    transport: &dyn Transport,
    kind: TransportKind,
    target: ConnectionTarget,
) {
    let id = PtySessionId::new();
    let size = PtySize { cols: 80, rows: 24 };

    assert!(is_unimplemented(
        transport.connect(&target).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.disconnect().await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.list_dir("/").await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.stat("/").await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport
            .search_files(SearchQuery::new("main"))
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport
            .read_file("/x", ReadFileOptions::default())
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport
            .open_pty(PtySpawnSpec { cwd: None, size })
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.write_pty(&id, "ls").await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.resize_pty(&id, size).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.close_pty(&id).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport
            .run_structured(StructuredRequest::new("ls | to json"))
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        transport.probe_shell().await.unwrap_err(),
        kind
    ));

    fixture::unbuilt_git::assert_every_git_method_unimplemented(transport, kind).await;
    fixture::unbuilt_remote::assert_every_git_remote_method_unimplemented(transport, kind).await;
    fixture::unbuilt_remote::assert_every_github_method_unimplemented(transport, kind).await;
}

#[tokio::test]
async fn remote_agent_transport_answers_every_method() {
    let transport = RemoteAgentTransport::new();
    assert_eq!(transport.kind(), TransportKind::RemoteAgent);
    assert!(transport.url().is_none());
    let target = ConnectionTarget::RemoteAgent {
        url: "ws://127.0.0.1:8731/ws".to_string(),
        root: "/srv".to_string(),
    };
    assert_every_method_unimplemented(&transport, TransportKind::RemoteAgent, target).await;
}
