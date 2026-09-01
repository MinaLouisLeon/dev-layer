//! Argv for listing conflicts and settling one.
//!
//! Resolving is **two git calls**, not one, and that is worth stating up
//! front because it is the only place in this crate where a single trait
//! method runs two mutating commands in sequence:
//!
//! | Resolution | What runs |
//! | --- | --- |
//! | `Ours` / `Theirs` | `git checkout --ours\|--theirs -- <path>`, then `git add -- <path>` |
//! | `Manual` | `git add -- <path>` alone |
//!
//! The checkout writes one side over the file on disk; the add is what tells
//! git the path is settled. Doing only the first would leave a file that looks
//! resolved and a commit that still refuses. Doing only the second would mark
//! a file with conflict markers still in it as resolved, which is how `<<<<<<<`
//! ends up in a release.
//!
//! Every path here has already been through [`crate::git::guard`], and every
//! one of them sits behind `--`.

use super::{GLOBAL, PATH_SEPARATOR};
use crate::types::ConflictResolution;

/// The status call `conflicts()` reads.
///
/// The same `--porcelain=v2 -z` format phase 1 already parses, asked without
/// the untracked walk. `conflicts()` only cares about `u` records, and the
/// untracked walk is most of what a status costs - so the section can be
/// re-read after every resolution without it being a call worth thinking
/// about.
pub fn conflicts_argv() -> Vec<String> {
    let mut argv = owned(GLOBAL);
    argv.extend(owned(&[
        "status",
        "--porcelain=v2",
        "-z",
        "--untracked-files=no",
        "--ignored=no",
    ]));
    argv
}

/// `git checkout --ours|--theirs -- <path>`, or `None` for a manual
/// resolution, which checks nothing out.
///
/// The `--ours`/`--theirs` words come from
/// [`ConflictResolution::checkout_flag`], so they are written down in exactly
/// one place and a caller cannot spell one.
pub fn take_side_argv(resolution: ConflictResolution, path: &str) -> Option<Vec<String>> {
    let flag = resolution.checkout_flag()?;
    let mut argv = owned(&["checkout", flag]);
    argv.push(PATH_SEPARATOR.to_string());
    argv.push(path.to_string());
    Some(argv)
}

/// `git add -- <path>`: the call that actually marks a path resolved.
///
/// It is `add` and not something conflict-specific because that *is* git's
/// interface for this - staging an unmerged path is how you tell git you have
/// settled it. Named for what it means here rather than for what it runs, so
/// the call site reads as the thing it is doing.
pub fn mark_resolved_argv(path: &str) -> Vec<String> {
    let mut argv = owned(&["add"]);
    argv.push(PATH_SEPARATOR.to_string());
    argv.push(path.to_string());
    argv
}

fn owned<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    args.iter().map(|arg| arg.as_ref().to_string()).collect()
}

#[cfg(test)]
mod tests;
