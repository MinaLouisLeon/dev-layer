//! Shared body for the remote and conflict surfaces of a transport that
//! compiles but is not built yet.
//!
//! The sibling of [`crate::stub_git_refs`], split from it for the same reason
//! that file was split from [`crate::stub_git`] - one file per surface, so
//! none of them grows past the project's ceiling for reasons that have nothing
//! to do with what it says.
//!
//! [`crate::unimplemented_git_transport`] expands this macro itself, so a
//! transport with no git at all still writes one line and gets all five
//! surfaces.
//!
//! The remote half is the one worth a note. An unbuilt `push` answers
//! `Unimplemented`; it does **not** answer "there is no remote configured",
//! and it does not fail in a way that could be read as a rejection. A caller
//! seeing this must be able to tell "this build cannot push" from "the push
//! was refused", because only one of those is something the reader can act on.

/// Generates `impl GitRemoteTransport` and `impl GitConflictTransport` where
/// every method returns [`crate::TransportError::Unimplemented`] naming the
/// transport and the method. Replace methods one at a time as a transport is
/// built out.
#[macro_export]
macro_rules! unimplemented_git_remote_transports {
    ($ty:ty, $kind:expr) => {
        #[::async_trait::async_trait]
        impl $crate::transport::GitRemoteTransport for $ty {
            async fn remotes(&self) -> $crate::Result<Vec<$crate::types::GitRemote>> {
                Err($crate::TransportError::unimplemented($kind, "git_remotes"))
            }

            async fn fetch(
                &self,
                _remote: Option<String>,
            ) -> $crate::Result<$crate::types::GitFetchResult> {
                Err($crate::TransportError::unimplemented($kind, "git_fetch"))
            }

            async fn pull(
                &self,
                _request: $crate::types::PullRequest,
            ) -> $crate::Result<$crate::types::GitPullResult> {
                Err($crate::TransportError::unimplemented($kind, "git_pull"))
            }

            async fn push(
                &self,
                _request: $crate::types::PushRequest,
            ) -> $crate::Result<$crate::types::GitPushResult> {
                Err($crate::TransportError::unimplemented($kind, "git_push"))
            }
        }

        #[::async_trait::async_trait]
        impl $crate::transport::GitConflictTransport for $ty {
            async fn conflicts(&self) -> $crate::Result<Vec<$crate::types::GitConflict>> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_conflicts",
                ))
            }

            async fn resolve(
                &self,
                _path: &str,
                _resolution: $crate::types::ConflictResolution,
            ) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented($kind, "git_resolve"))
            }
        }
    };
}
