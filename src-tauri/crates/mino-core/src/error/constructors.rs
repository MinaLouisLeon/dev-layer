//! Constructors for [`TransportError`].
//!
//! Split from the enum so that file stays a description of the shape and this
//! one holds the convenience of building it.

use super::TransportError;
use crate::types::TransportKind;

impl TransportError {
    pub fn unimplemented(transport: TransportKind, feature: impl Into<String>) -> Self {
        Self::Unimplemented {
            transport,
            feature: feature.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    pub fn pty(message: impl Into<String>) -> Self {
        Self::Pty {
            message: message.into(),
        }
    }

    pub fn shell(message: impl Into<String>) -> Self {
        Self::Shell {
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidArgument {
            message: message.into(),
        }
    }

    pub fn conflict(path: &str, expected: Option<u64>, actual: Option<u64>) -> Self {
        Self::Conflict {
            path: path.to_string(),
            expected_modified_ms: expected,
            actual_modified_ms: actual,
        }
    }

    /// Used by the SSH transport for handshake, host key and authentication
    /// failures: the connection worked, the conversation over it did not.
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }

    /// Maps an OS error to the typed variant so the UI can branch on
    /// not-found vs permission-denied without string matching.
    pub fn from_io(path: &str, err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound {
                path: path.to_string(),
            },
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied {
                path: path.to_string(),
            },
            _ => Self::Io {
                message: format!("{path}: {err}"),
            },
        }
    }
}
