//! Shared body for a git surface that compiles but is not built yet.
//!
//! The sibling of [`crate::stub`], kept separate for the reason `GitTransport`
//! is separate from `Transport`: a transport whose filesystem half is built
//! and whose git half is not should be able to say so without one macro
//! answering for both. The remote-agent transport uses both today.
//!
//! Note what this does *not* do. An unbuilt git surface answers
//! `Unimplemented`; it does not answer "there is no repository here". Absence
//! and not-built-yet read the same in a quiet UI and are entirely different
//! facts, and only one of them is a bug waiting to be finished.

/// Generates `impl GitTransport` where every method returns
/// [`crate::TransportError::Unimplemented`] naming the transport and the
/// method. Replace methods one at a time as a transport is built out.
///
/// The branch, stash, remote and conflict surfaces are supertraits, so this
/// expands [`crate::unimplemented_git_ref_transports`] and
/// [`crate::unimplemented_git_remote_transports`] too: a transport that has
/// built none of git writes one line, not five.
#[macro_export]
macro_rules! unimplemented_git_transport {
    ($ty:ty, $kind:expr) => {
        $crate::unimplemented_git_ref_transports!($ty, $kind);
        $crate::unimplemented_git_remote_transports!($ty, $kind);

        #[::async_trait::async_trait]
        impl $crate::transport::GitTransport for $ty {
            async fn repository(&self) -> $crate::Result<Option<$crate::types::GitRepository>> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_repository",
                ))
            }

            async fn status(&self) -> $crate::Result<$crate::types::GitStatus> {
                Err($crate::TransportError::unimplemented($kind, "git_status"))
            }

            async fn stage(&self, _paths: &[String]) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented($kind, "git_stage"))
            }

            async fn unstage(&self, _paths: &[String]) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented($kind, "git_unstage"))
            }

            async fn discard(&self, _paths: &[String]) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented($kind, "git_discard"))
            }

            async fn commit(
                &self,
                _request: $crate::types::CommitRequest,
            ) -> $crate::Result<$crate::types::GitCommit> {
                Err($crate::TransportError::unimplemented($kind, "git_commit"))
            }

            async fn diff(
                &self,
                _request: $crate::types::DiffRequest,
            ) -> $crate::Result<$crate::types::GitDiff> {
                Err($crate::TransportError::unimplemented($kind, "git_diff"))
            }

            async fn log(
                &self,
                _request: $crate::types::LogRequest,
            ) -> $crate::Result<$crate::types::GitLog> {
                Err($crate::TransportError::unimplemented($kind, "git_log"))
            }

            async fn show(
                &self,
                _revision: &str,
            ) -> $crate::Result<$crate::types::GitCommitDetail> {
                Err($crate::TransportError::unimplemented($kind, "git_show"))
            }

            async fn commit_diff(
                &self,
                _revision: &str,
                _path: Option<&str>,
            ) -> $crate::Result<$crate::types::GitDiff> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_commit_diff",
                ))
            }

            async fn blame(&self, _path: &str) -> $crate::Result<$crate::types::GitBlame> {
                Err($crate::TransportError::unimplemented($kind, "git_blame"))
            }
        }
    };
}
