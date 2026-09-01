use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub enum ShellKind {
    Nu,
    /// The platform default used when `nu` is not on PATH.
    Fallback,
}

/// Result of looking for `nu` on the target. Cheap enough to call on mount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct ShellProbe {
    pub nu_available: bool,
    /// Absolute path to `nu`, when found.
    pub nu_path: Option<String>,
    /// Program spawned when `nu` is absent, e.g. `/bin/zsh`, `powershell.exe`.
    pub fallback_program: String,
    pub fallback_label: String,
}

/// A non-interactive Nushell call.
///
/// SECURITY: `pipeline` is fixed program text supplied by the app, and caller
/// values go in `params` only. Each param is bound as an environment variable
/// named `MINO_<KEY>` and referenced from the pipeline as `$env.MINO_<KEY>`,
/// so a path containing `; rm -rf /` is data, never syntax. Keys are validated
/// against `^[A-Z0-9_]+$` before the process is spawned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct StructuredRequest {
    /// Must end in `to json`; the transport rejects anything else so the
    /// channel always returns parseable structured data.
    pub pipeline: String,
    #[ts(type = "Record<string, string>")]
    pub params: BTreeMap<String, String>,
    pub cwd: Option<String>,
    /// Wall-clock ceiling. `None` uses the transport default (10s).
    #[ts(type = "number | null")]
    pub timeout_ms: Option<u64>,
}

impl StructuredRequest {
    pub fn new(pipeline: impl Into<String>) -> Self {
        Self {
            pipeline: pipeline.into(),
            params: BTreeMap::new(),
            cwd: None,
            timeout_ms: None,
        }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

/// Parsed output of a structured call. `value` is whatever `to json`
/// produced, already parsed — callers never see raw terminal text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "generated/")]
pub struct StructuredOutput {
    #[ts(type = "unknown")]
    pub value: serde_json::Value,
    /// Anything Nushell wrote to stderr. Surfaced in the error notice; empty
    /// on success.
    pub stderr: String,
}
