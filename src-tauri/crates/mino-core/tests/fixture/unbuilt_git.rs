//! What an unbuilt git surface has to answer, in one place.
//!
//! Beside the fixtures rather than inside `unimplemented_transports.rs`,
//! because `GitTransport` now has three surfaces on it and walking all of them
//! is the largest thing in that file without being what the file is about.
//!
//! The point of walking *every* method: the four extra surfaces are
//! supertraits reached through the same object, and the one that was forgotten
//! would panic the first time a picker opened rather than answering.
//!
//! The remote, conflict and GitHub walks are in `unbuilt_remote.rs`, which
//! also carries the two sentences those three need that these do not.

use mino_core::types::TransportKind;
use mino_core::{Transport, TransportError};

pub fn is_unimplemented(err: TransportError, expected: TransportKind) -> bool {
    matches!(err, TransportError::Unimplemented { transport, .. } if transport == expected)
}

pub async fn assert_every_git_method_unimplemented(transport: &dyn Transport, kind: TransportKind) {
    // The git surface is present rather than absent: `None` would read as
    // "this target has no git", which is a different fact from "not written
    // yet" and would send the reader to the wrong file.
    let git = transport
        .git()
        .expect("an unbuilt git surface is still a surface");
    assert!(is_unimplemented(git.repository().await.unwrap_err(), kind));
    assert!(is_unimplemented(git.status().await.unwrap_err(), kind));

    let paths = ["/srv/app/x".to_string()];
    assert!(is_unimplemented(git.stage(&paths).await.unwrap_err(), kind));
    assert!(is_unimplemented(
        git.unstage(&paths).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.discard(&paths).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.commit(mino_core::types::CommitRequest::new("m"))
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.diff(mino_core::types::DiffRequest::worktree())
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.log(mino_core::types::LogRequest::new())
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(git.show("HEAD").await.unwrap_err(), kind));
    assert!(is_unimplemented(
        git.commit_diff("HEAD", None).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.blame("/srv/app/x").await.unwrap_err(),
        kind
    ));

    // The branch and stash surfaces are supertraits, reached through the same
    // object. An unbuilt transport has to answer for those too, or the one
    // that was forgotten would panic the first time a picker opened.
    assert!(is_unimplemented(git.branches().await.unwrap_err(), kind));
    assert!(is_unimplemented(
        git.checkout("main").await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.create_branch(mino_core::types::CreateBranchRequest::new("feat"))
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.delete_branch("feat", false).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(git.stash_list().await.unwrap_err(), kind));
    assert!(is_unimplemented(
        git.stash_push(mino_core::types::StashRequest::new())
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.stash_apply(0, false).await.unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(git.stash_drop(0).await.unwrap_err(), kind));
}
