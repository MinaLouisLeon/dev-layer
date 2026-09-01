//! The non-interactive Nushell channel.
//!
//! Runs a fixed pipeline with `nu --no-config-file -c`, binds caller values as
//! environment variables and parses the `to json` output. This is what the
//! tree and the breadcrumb consume; terminal text is never scraped.

use std::collections::BTreeMap;
use std::process::Stdio;

use tokio::process::Command;

use crate::error::{Result, TransportError};
use crate::local::no_window::hide_console;
use crate::types::{StructuredOutput, StructuredRequest};

pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;
pub const MAX_TIMEOUT_MS: u64 = 60_000;
/// Every bound value becomes `$env.MINO_<KEY>` inside the pipeline.
pub const PARAM_PREFIX: &str = "MINO_";
const REQUIRED_SUFFIX: &str = "to json";

pub async fn run(nu_path: &str, request: &StructuredRequest) -> Result<StructuredOutput> {
    validate(request)?;
    let timeout_ms = request
        .timeout_ms
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .min(MAX_TIMEOUT_MS);

    let mut command = Command::new(nu_path);
    // The tree asks this once per folder it reads, so without the flag one
    // expanded directory is one console window - see `no_window`.
    hide_console(&mut command);
    // The pipeline is a single argument, never a shell line, and caller
    // values are bound as env vars below - so they cannot become syntax.
    command
        .arg("--no-config-file")
        .arg("-c")
        .arg(&request.pipeline)
        .envs(bound_env(&request.params))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Dropping the future on timeout must not leave nu running.
        .kill_on_drop(true);

    if let Some(cwd) = &request.cwd {
        command.current_dir(cwd);
    }

    let child = command
        .spawn()
        .map_err(|e| TransportError::shell(format!("could not start nu: {e}")))?;

    let output = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| TransportError::Timeout {
        operation: "run_structured".to_string(),
        ms: timeout_ms,
    })?
    .map_err(|e| TransportError::shell(e.to_string()))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(TransportError::shell(if stderr.is_empty() {
            "the nushell command failed".to_string()
        } else {
            stderr
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = serde_json::from_str(stdout.trim()).map_err(|e| TransportError::Protocol {
        message: format!("nushell returned output that is not json: {e}"),
    })?;

    Ok(StructuredOutput { value, stderr })
}

fn bound_env(params: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    params
        .iter()
        .map(|(key, value)| (format!("{PARAM_PREFIX}{key}"), value.clone()))
        .collect()
}

fn validate(request: &StructuredRequest) -> Result<()> {
    if !request.pipeline.trim_end().ends_with(REQUIRED_SUFFIX) {
        return Err(TransportError::invalid(
            "a structured pipeline must end in `to json`",
        ));
    }
    for key in request.params.keys() {
        let valid = !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !valid {
            return Err(TransportError::invalid(format!(
                "parameter name `{key}` must match ^[A-Z0-9_]+$"
            )));
        }
    }
    Ok(())
}
