//! Shared body for the branch and stash surfaces of a transport that compiles
//! but is not built yet.
//!
//! Split from [`crate::stub_git`] the way the traits themselves are split -
//! one file per surface, so none of them grows past the project's ceiling for
//! reasons that have nothing to do with what it says.
//!
//! [`crate::unimplemented_git_transport`] expands this macro itself, so a
//! transport still writes one line and gets all three surfaces. Reaching for
//! this one directly is what a transport does when it has built the main
//! surface and not these.

/// Generates `impl GitBranchTransport` and `impl GitStashTransport` where
/// every method returns [`crate::TransportError::Unimplemented`] naming the
/// transport and the method. Replace methods one at a time as a transport is
/// built out.
#[macro_export]
macro_rules! unimplemented_git_ref_transports {
    ($ty:ty, $kind:expr) => {
        #[::async_trait::async_trait]
        impl $crate::transport::GitBranchTransport for $ty {
            async fn branches(&self) -> $crate::Result<Vec<$crate::types::GitBranch>> {
                Err($crate::TransportError::unimplemented($kind, "git_branches"))
            }

            async fn checkout(&self, _name: &str) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented($kind, "git_checkout"))
            }

            async fn create_branch(
                &self,
                _request: $crate::types::CreateBranchRequest,
            ) -> $crate::Result<$crate::types::GitBranch> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_create_branch",
                ))
            }

            async fn delete_branch(&self, _name: &str, _force: bool) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_delete_branch",
                ))
            }
        }

        #[::async_trait::async_trait]
        impl $crate::transport::GitStashTransport for $ty {
            async fn stash_list(&self) -> $crate::Result<Vec<$crate::types::GitStash>> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_stash_list",
                ))
            }

            async fn stash_push(
                &self,
                _request: $crate::types::StashRequest,
            ) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_stash_push",
                ))
            }

            async fn stash_apply(&self, _index: u32, _pop: bool) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_stash_apply",
                ))
            }

            async fn stash_drop(&self, _index: u32) -> $crate::Result<()> {
                Err($crate::TransportError::unimplemented(
                    $kind,
                    "git_stash_drop",
                ))
            }
        }
    };
}
