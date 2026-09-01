//! Shared body for a GitHub surface that compiles but is not built yet.
//!
//! The third of its kind, after [`crate::stub`] and [`crate::stub_git`], and
//! kept separate for the same reason those two are: a transport whose
//! filesystem half is built and whose GitHub half is not should be able to say
//! so without one macro answering for all three.
//!
//! Note what this does *not* do, because it is the same trap `stub_git`
//! documents and it is worse here. An unbuilt GitHub surface answers
//! `Unimplemented`; it does **not** answer "there is no GitHub repository
//! here". Those read the same in a quiet UI - both leave the view saying one
//! sentence and nothing else - and they are entirely different facts. Only one
//! of them is a bug waiting to be finished, and a probe that quietly reported
//! `Unsupported` would hide it for good.

/// Generates `impl GitHubTransport` where both methods return
/// [`crate::TransportError::Unimplemented`] naming the transport and the
/// method. Replace them as a transport is built out.
#[macro_export]
macro_rules! unimplemented_github_transport {
    ($ty:ty, $kind:expr) => {
        #[::async_trait::async_trait]
        impl $crate::transport::GitHubTransport for $ty {
            async fn probe(&self) -> $crate::Result<$crate::types::GitHubProbe> {
                Err($crate::TransportError::unimplemented($kind, "github_probe"))
            }

            async fn query(
                &self,
                _request: $crate::types::GitHubQuery,
            ) -> $crate::Result<$crate::types::GitHubResponse> {
                Err($crate::TransportError::unimplemented($kind, "github_query"))
            }
        }
    };
}
