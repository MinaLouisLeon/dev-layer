//! Turning "this file, this line" into something `gh browse` can be given.
//!
//! The path is the reason this is a module rather than a `format!`. A browse
//! request names a file, and a file the session does not own must not be
//! nameable - not because the URL would leak anything by itself, but because
//! "which paths may this app act on" is answered in exactly one way in this
//! codebase, and a second way would be one more to get wrong.
//!
//! So it reuses [`crate::git::guard`]: the same guard `stage`, `discard` and
//! `blame` pass their paths through. A `..` segment is refused outright, the
//! path must sit inside the session root, and what comes back is
//! root-relative with forward slashes - which is also exactly the shape
//! `gh browse` wants.

use crate::error::{Result, TransportError};
use crate::git::guard::guard_paths;
use crate::git::paths::PathStyle;
use crate::git::refname;

/// The positional argument for `gh browse`: `<path>` or `<path>:<line>`.
///
/// The line is a `u32` formatted here, so the only caller *text* in the result
/// is the path, and the path has just been ruled on. Line zero is dropped
/// rather than sent: editors count from one, `gh` counts from one, and a zero
/// is a caller that has not decided.
pub fn target(root: &str, path: &str, line: Option<u32>, style: PathStyle) -> Result<String> {
    let relative = guard_paths(root, std::slice::from_ref(&path.to_string()), style)?.remove(0);
    Ok(match line {
        Some(line) if line > 0 => format!("{relative}:{line}"),
        _ => relative,
    })
}

/// The branch to link to, checked the way every other branch name in this
/// codebase is checked.
///
/// [`crate::git::refname::precheck`] is the local half of the branch guard: it
/// refuses an empty name, an absurd length, a leading dash, and anything the
/// remote quoting rule would have to refuse later anyway. `gh browse` is not
/// `git checkout`, but a value that reaches a command line gets the same
/// treatment wherever it is going.
pub fn branch(name: Option<&str>) -> Result<Option<String>> {
    match name {
        Some(name) => refname::precheck(name).map(Some),
        None => Ok(None),
    }
}

/// What a failed `gh browse` should say.
///
/// `gh` is terse here - a file it cannot place produces very little - so the
/// sentence names the likely reason rather than leaving the reader with an
/// empty notice.
pub fn refused(path: &str) -> TransportError {
    TransportError::invalid(format!(
        "GitHub could not place `{path}`. A file that has never been pushed has no address on \
         the web yet."
    ))
}
