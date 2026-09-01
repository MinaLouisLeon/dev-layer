//! What an unbuilt remote, conflict and GitHub surface has to answer.
//!
//! Beside `unbuilt_git.rs` so neither file grows past the project's ceiling,
//! and because these three share a trap the others do not: on each of them an
//! `Unimplemented` and a *real* answer look alike in a quiet UI.
//!
//! - An unbuilt **`push`** must not fail in any way that reads as a push the
//!   remote refused. Only one of those is something the reader can act on.
//! - An unbuilt **`conflicts`** must not answer with an empty list. "No agent
//!   protocol yet" and "nothing is conflicted" both render as a missing
//!   section, and only one of them is a bug waiting to be finished.
//! - An unbuilt **`probe`** must not answer `unsupported`, for the same reason
//!   one notch further out.
#![allow(dead_code)]

use mino_core::types::TransportKind;
use mino_core::Transport;

use super::unbuilt_git::is_unimplemented;

/// The remote and conflict surfaces, on the same terms.
///
/// The point of walking these too is the same as it is for the branch and
/// stash halves, with one sharper edge: an unbuilt `push` must answer
/// `Unimplemented` and must **not** fail in any way that could be read as a
/// rejection. "This build cannot push" and "the remote refused" are different
/// facts, and only one of them is something the reader can do anything about.
pub async fn assert_every_git_remote_method_unimplemented(
    transport: &dyn Transport,
    kind: TransportKind,
) {
    let git = transport
        .git()
        .expect("an unbuilt git surface is still a surface");

    assert!(is_unimplemented(git.remotes().await.unwrap_err(), kind));
    assert!(is_unimplemented(git.fetch(None).await.unwrap_err(), kind));
    assert!(is_unimplemented(
        git.pull(mino_core::types::PullRequest::default())
            .await
            .unwrap_err(),
        kind
    ));
    assert!(is_unimplemented(
        git.push(mino_core::types::PushRequest::default())
            .await
            .unwrap_err(),
        kind
    ));
    // And this one does not answer with an empty list. "No agent protocol yet"
    // and "nothing is conflicted" render identically in a quiet panel.
    assert!(is_unimplemented(git.conflicts().await.unwrap_err(), kind));
    assert!(is_unimplemented(
        git.resolve("/srv/app/x", mino_core::types::ConflictResolution::Manual)
            .await
            .unwrap_err(),
        kind
    ));
}

/// The GitHub surface, on the same terms.
///
/// Two methods rather than nineteen, because five features share one
/// enumerated query - and the same trap applies as it does above, only more
/// sharply. An unbuilt GitHub surface must answer `Unimplemented` and must
/// **not** answer "there is no GitHub repository here": those read the same in
/// a quiet view and only one of them is a bug waiting to be finished.
pub async fn assert_every_github_method_unimplemented(
    transport: &dyn Transport,
    kind: TransportKind,
) {
    let github = transport
        .github()
        .expect("an unbuilt GitHub surface is still a surface");
    assert!(is_unimplemented(github.probe().await.unwrap_err(), kind));
    assert!(is_unimplemented(
        github
            .query(mino_core::types::GitHubQuery::PullRequests {
                state: mino_core::types::PrState::Open,
                limit: mino_core::types::DEFAULT_GITHUB_LIMIT,
            })
            .await
            .unwrap_err(),
        kind
    ));
}
