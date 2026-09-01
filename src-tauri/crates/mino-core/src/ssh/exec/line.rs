//! One argv array into one remote command line.
//!
//! Split from the runner beside it because it is the half worth reading on its
//! own: `exec` hands a **string** to the remote login shell, so this is the
//! only place in the SSH transport where anything becomes syntax.
//!
//! Every argument is quoted, not merely joined, and
//! [`crate::ssh::command::quote`] *refuses* a value containing a single quote
//! rather than escaping it - which is the documented limit of this transport,
//! and the reason the largest free-text values travel on stdin instead.

use crate::error::{Result, TransportError};

use super::super::command::quote;

/// `cd '<cwd>' && [NAME='value' …] <program> '<arg>' '<arg>' …`.
///
/// Every argument is quoted, not just the working directory: a path with a
/// space in it would otherwise arrive at the remote program as two arguments.
/// `quote` refuses rather than escapes a value containing a single quote, so a
/// remote file whose name contains one is reported as a typed error instead of
/// being silently mishandled.
///
/// The environment prefix is how [`crate::git::command::NO_PROMPT`] reaches a
/// remote git: a POSIX shell reads `NAME=value cmd` as "run `cmd` with `NAME`
/// set". Without it, a remote push against an account with no credential
/// helper holds the channel open until the timeout, because a prompt on an
/// exec channel has nowhere at all to go.
pub(in crate::ssh) fn command_line(
    program: &str,
    cwd: &str,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<String> {
    let mut line = format!("cd {} && ", quote(cwd)?);
    for (key, value) in env {
        // A name is not quotable in this position - `'FOO'=1` is not an
        // assignment - so it is checked instead. These are constants in this
        // crate; the check is here so they stay that way.
        if !key
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(TransportError::invalid(
                "an environment name for a remote command must match ^[A-Z0-9_]+$",
            ));
        }
        line.push_str(key);
        line.push('=');
        line.push_str(&quote(value)?);
        line.push(' ');
    }
    line.push_str(program);
    for arg in argv {
        line.push(' ');
        line.push_str(&quote(arg)?);
    }
    Ok(line)
}
