//! The path guard for the mutating calls.
//!
//! Every path handed to `stage`, `unstage`, `discard` or a path-scoped commit
//! passes through here before it can reach argv, on both transports. `discard`
//! is the reason this is a separate, explicit step rather than a check folded
//! into each call: it throws away work that exists nowhere else, and it must
//! never be reachable with a path the session has not already proved it owns.
//!
//! **Why not the existing `RootGuard`.** That guard canonicalises, which is a
//! syscall, and a syscall cannot answer for a path that is not there. Staging
//! a *deleted* file is an ordinary thing to do - it is half of what a source
//! control panel is for - so a guard built on `canonicalize` would refuse the
//! very operation the panel exists to offer. This one rules on the string, and
//! is strict about it:
//!
//! That strictness has a cost, and the local transport pays it before calling
//! here. A caller can hold a *different spelling* of a path the session plainly
//! owns - a Windows 8.3 short name, a symlinked temporary directory on macOS -
//! and a string test refuses those. `local::git_guard::resolve` canonicalises
//! what exists first, so both spellings arrive as one. Anything that does not
//! resolve, a deleted file included, reaches the rules below untouched, and
//! containment is still checked here afterwards.
//!
//! - a `..` segment is refused outright rather than resolved, so there is no
//!   traversal to reason about;
//! - the path must sit inside the session root by [`PathStyle::within`], the
//!   same test phase 1 filters status rows with;
//! - the result is root-relative with forward slashes, which git accepts on
//!   every platform and which keeps an absolute Windows path - drive letter,
//!   backslashes and all - out of a remote command line.

use crate::error::{Result, TransportError};

use super::paths::PathStyle;

/// Guards a batch. Returns the paths git should be given, root-relative.
///
/// All-or-nothing on purpose: a batch where one path is refused runs for none
/// of them. Half-applying a stage and then reporting a failure would leave the
/// index in a state nobody asked for and the UI unable to say what happened.
pub fn guard_paths(root: &str, paths: &[String], style: PathStyle) -> Result<Vec<String>> {
    paths
        .iter()
        .map(|path| guard_one(root, path, style))
        .collect()
}

fn guard_one(root: &str, path: &str, style: PathStyle) -> Result<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(TransportError::invalid("an empty path cannot be used"));
    }
    // Refused, not resolved. `a/../b` is a path this app never produces, and
    // treating it as an error is one fewer thing to be clever about.
    if trimmed
        .split(['/', '\\'])
        .any(|segment| segment == ".." || segment == ".")
    {
        return Err(TransportError::PathEscapesRoot {
            path: path.to_string(),
        });
    }
    if !style.within(root, trimmed) {
        return Err(TransportError::PathEscapesRoot {
            path: path.to_string(),
        });
    }
    let relative = relative_to(root, trimmed, style);
    if relative.is_empty() {
        // The root itself. An empty slice is how a caller says "everything";
        // naming the root is not, and letting it through would turn a
        // one-file discard into a whole-tree one.
        return Err(TransportError::invalid(
            "the connected root is not a path this operation can take",
        ));
    }
    Ok(relative)
}

/// `path` with `root` removed, forward-slashed. Both are already known to be
/// in the same style and to be root-and-descendant by the time this runs.
fn relative_to(root: &str, path: &str, style: PathStyle) -> String {
    let root = style.normalise(root).replace('\\', "/");
    let path = style.normalise(path).replace('\\', "/");
    let rest = if style.case_insensitive {
        // Slice by length rather than by the folded strings: the folded form
        // is only for comparison, and the path the caller gets back has to
        // keep its original casing.
        path.get(root.len()..).unwrap_or("")
    } else {
        path.strip_prefix(&root).unwrap_or("")
    };
    rest.trim_start_matches('/').to_string()
}

#[cfg(test)]
mod tests;
