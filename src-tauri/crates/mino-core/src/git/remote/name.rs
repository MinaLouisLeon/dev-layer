//! The one caller value a remote call takes that is not a number.
//!
//! Remote names are refs too - `refs/remotes/<name>/…` - but the shapes worth
//! refusing here are the ones that are not names at all: an empty string, and
//! anything git would read as an option. Everything else is left to git, which
//! says `'x' does not appear to be a git repository` far better than a
//! hand-rolled rule could.
//!
//! Split from the parsers beside it because it runs *before* a call and they
//! run after, and because a guard is worth finding on its own.

use crate::error::{Result, TransportError};

/// A remote name worth handing to git.
///
/// Remote names are refs too - `refs/remotes/<name>/…` - but the shapes that
/// matter here are the ones that are not names at all: an empty string, and
/// anything git would read as an option. The rest is left to git, which will
/// say `'x' does not appear to be a git repository` far better than a
/// hand-rolled rule could.
pub fn name(remote: &str) -> Result<String> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err(TransportError::invalid("a remote needs a name"));
    }
    if trimmed.starts_with('-') {
        return Err(TransportError::invalid(format!(
            "`{trimmed}` cannot be used as a remote name: it would be read as an option"
        )));
    }
    if let Some(bad) = trimmed
        .chars()
        .find(|ch| ch.is_control() || " \t'\"\\".contains(*ch))
    {
        return Err(TransportError::invalid(format!(
            "a remote name cannot contain `{}`",
            bad.escape_debug()
        )));
    }
    Ok(trimmed.to_string())
}

/// The same for an optional remote: `None` means "whichever git would use",
/// which is the right default and not a value to validate.
pub fn optional_name(remote: Option<&str>) -> Result<Option<String>> {
    remote.map(name).transpose()
}
