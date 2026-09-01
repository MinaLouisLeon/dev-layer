//! The single error type crossing the transport boundary.
//!
//! The variants are here; the constructors that build them are in
//! [`constructors`], which keeps this file to the shape of the error.
//!
//! `TransportError` is `Serialize`, so a Tauri command, an agent HTTP response
//! and an agent WebSocket frame all return the exact same shape, and the
//! TypeScript client narrows on one discriminant (`kind`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::TransportKind;

pub type Result<T> = std::result::Result<T, TransportError>;

#[derive(Debug, Clone, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum TransportError {
    /// A transport exists and compiles but the operation is not built yet.
    /// Returned by the SSH and remote-agent transports for every method in
    /// phase 1. Never a panic: `todo!()`/`unimplemented!()` are banned here.
    #[error("{transport:?} transport does not implement {feature} yet")]
    #[serde(rename_all = "camelCase")]
    Unimplemented {
        feature: String,
        transport: TransportKind,
    },

    #[error("no active connection; call connect first")]
    NotConnected,

    #[error("path not found: {path}")]
    NotFound { path: String },

    #[error("permission denied: {path}")]
    PermissionDenied { path: String },

    /// The requested path resolved outside the connected root. Raised by the
    /// path guard, not by the OS.
    #[error("path escapes the connected root: {path}")]
    PathEscapesRoot { path: String },

    #[error("{path} is {size} bytes, above the {limit} byte ceiling")]
    #[serde(rename_all = "camelCase")]
    TooLarge {
        path: String,
        #[ts(type = "number")]
        size: u64,
        #[ts(type = "number")]
        limit: u64,
    },

    #[error("{path} looks like a binary file")]
    #[serde(rename_all = "camelCase")]
    BinaryFile {
        path: String,
        #[ts(type = "number")]
        size: u64,
    },

    #[error("pty session {id} is not open")]
    PtyNotFound { id: String },

    #[error("pty error: {message}")]
    Pty { message: String },

    #[error("shell error: {message}")]
    Shell { message: String },

    #[error("i/o error: {message}")]
    Io { message: String },

    #[error("protocol error: {message}")]
    Protocol { message: String },

    #[error("{operation} timed out after {ms}ms")]
    Timeout {
        operation: String,
        #[ts(type = "number")]
        ms: u64,
    },

    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    /// The file changed between being opened and being saved. Raised instead
    /// of overwriting, so an edit made elsewhere is never silently lost.
    #[error("{path} changed on disk since it was opened")]
    #[serde(rename_all = "camelCase")]
    Conflict {
        path: String,
        /// Modification time the editor loaded, and the one found now.
        #[ts(type = "number | null")]
        expected_modified_ms: Option<u64>,
        #[ts(type = "number | null")]
        actual_modified_ms: Option<u64>,
    },
}

mod constructors;
