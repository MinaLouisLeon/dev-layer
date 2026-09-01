use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which of the three transport implementations produced a value or an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum TransportKind {
    Local,
    Ssh,
    RemoteAgent,
}

/// What to connect to. One variant per transport so adding a transport is a
/// compile error everywhere it has to be handled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum ConnectionTarget {
    /// Absolute path to the folder the session is rooted at. Every local
    /// filesystem call is confined to it by the path guard.
    #[serde(rename_all = "camelCase")]
    Local { root: String },

    #[serde(rename_all = "camelCase")]
    Ssh {
        host: String,
        port: u16,
        user: String,
        /// `None` roots the session at the account's home directory, which is
        /// what a fresh connection does: the remote paths are not knowable
        /// before connecting, so the folder is chosen afterwards, from the
        /// workbench, against a real listing. Re-connecting with `Some` is
        /// how that choice is applied.
        root: Option<String>,
        /// Path to a private key on the *operating* machine. Never the key
        /// material itself: secrets are not carried through this type, are
        /// not persisted by the app, and are not logged.
        identity_path: Option<String>,
    },

    #[serde(rename_all = "camelCase")]
    RemoteAgent {
        /// `ws://127.0.0.1:8731/ws` style URL of a mino-agent daemon.
        url: String,
        root: String,
    },
}

impl ConnectionTarget {
    pub fn kind(&self) -> TransportKind {
        match self {
            Self::Local { .. } => TransportKind::Local,
            Self::Ssh { .. } => TransportKind::Ssh,
            Self::RemoteAgent { .. } => TransportKind::RemoteAgent,
        }
    }

    /// The requested root, when the caller named one. SSH may leave it unset
    /// and take the account's home directory instead.
    pub fn root(&self) -> Option<&str> {
        match self {
            Self::Local { root } | Self::RemoteAgent { root, .. } => Some(root),
            Self::Ssh { root, .. } => root.as_deref(),
        }
    }
}

/// The result of a successful `connect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct ConnectionInfo {
    pub id: String,
    pub kind: TransportKind,
    /// The canonicalised root. The UI shows this, not the requested path.
    pub root: String,
    /// Human-readable label for the title bar, e.g. `mino-workbench (local)`.
    pub label: String,
}
