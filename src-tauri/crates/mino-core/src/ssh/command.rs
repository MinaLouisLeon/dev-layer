//! Building the one remote command line, and refusing to build a dangerous one.
//!
//! `exec` hands a string to the remote login shell, which parses it. That is
//! the injection surface, and the rule for this file is simple: the string is
//! assembled from fixed program text only. Caller values travel over stdin as
//! JSON (see [`super::structured`]), never through here.
//!
//! One caller value does reach this file - the working directory, because `cd`
//! takes a path. It is single-quoted, and [`quote`] refuses rather than
//! escapes anything it cannot quote safely.

use crate::error::{Result, TransportError};
use crate::types::StructuredRequest;

/// Reads the JSON on stdin and binds it as `$env.MINO_*` before the pipeline
/// runs, so a pipeline written for the local transport works unchanged.
const PREAMBLE: &str = "$in | from json | load-env; ";
const REQUIRED_SUFFIX: &str = "to json";

pub fn command_line(nu: &str, pipeline: &str, cwd: Option<&str>) -> Result<String> {
    let script = format!("{PREAMBLE}{pipeline}");
    let mut line = String::new();
    if let Some(dir) = cwd {
        line.push_str(&format!("cd {} && ", quote(dir)?));
    }
    line.push_str(&format!(
        "{} --stdin --no-config-file -c {}",
        quote(nu)?,
        quote(&script)?
    ));
    Ok(line)
}

/// The interactive shell's launch line: change into the session root, then
/// replace this shell with the target one so the process tree stays flat and
/// closing the channel kills exactly one process.
///
/// Both values are quoted. Neither is free text - `program` comes from the
/// remote probe and `cwd` has already been through the root guard - but they
/// are still caller-influenced, so they are treated as data.
pub fn command_line_shell(program: &str, cwd: &str) -> Result<String> {
    Ok(format!("cd {} && exec {}", quote(cwd)?, quote(program)?))
}

/// POSIX single-quoting. A single quote cannot appear inside single quotes, so
/// rather than splice quotes together this refuses the value: everything that
/// reaches here is a path or fixed program text, and getting the escaping
/// subtly wrong is a worse outcome than refusing an exotic filename.
pub fn quote(value: &str) -> Result<String> {
    if value.contains('\'') || value.contains('\0') {
        return Err(TransportError::invalid(
            "a remote path containing a quote or a null byte cannot be used",
        ));
    }
    Ok(format!("'{value}'"))
}

pub fn validate(request: &StructuredRequest, nu: &str) -> Result<()> {
    if !request.pipeline.trim_end().ends_with(REQUIRED_SUFFIX) {
        return Err(TransportError::invalid(
            "a structured pipeline must end in `to json`",
        ));
    }
    // The pipeline is program text, so this can only fire on a bug in this
    // crate - but it fires before anything is sent, not after.
    if request.pipeline.contains('\'') {
        return Err(TransportError::invalid(
            "a structured pipeline may not contain a single quote",
        ));
    }
    if nu.contains('\'') {
        return Err(TransportError::invalid("the remote nu path is not usable"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_values_never_reach_the_command_line() {
        let line = command_line("nu", "ls | to json", None).unwrap();
        assert!(line.contains("--stdin"));
        assert!(line.contains("from json | load-env"));
        // Nothing but program text: no parameter value appears anywhere.
        assert!(!line.contains("MINO_PATH"));
    }

    #[test]
    fn a_quote_in_a_path_is_refused_not_escaped() {
        assert!(quote("/srv/it's").is_err());
        assert!(quote("/srv/app").is_ok());
        assert!(command_line("nu", "ls | to json", Some("/srv/it's")).is_err());
    }

    #[test]
    fn the_pipeline_must_still_end_in_to_json() {
        let bad = StructuredRequest::new("ls | first");
        assert!(validate(&bad, "nu").is_err());
        let good = StructuredRequest::new("ls | to json");
        assert!(validate(&good, "nu").is_ok());
    }

    #[test]
    fn parameter_names_are_restricted() {
        let request = StructuredRequest::new("ls | to json").param("BAD-KEY", "x");
        assert!(validate(&request, "nu").is_err());
    }
}
