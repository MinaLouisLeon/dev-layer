//! Argv for the calls that read history: diff, log, show and blame.
//!
//! Unlike [`super::read`] these do take caller values - a path, a revision, a
//! limit - so they build `Vec<String>` and follow the same two rules the
//! mutating half does: paths are guarded by [`crate::git::guard`] before they
//! arrive, and they sit behind a `--` separator.
//!
//! A **revision** is the one caller value that is not a path. It is validated
//! by [`crate::git::revision::validate`] rather than guarded, because a sha or
//! a branch name is not a filesystem path and the path guard would refuse
//! every one of them.

use crate::types::{DiffRequest, LogRequest, DIFF_CONTEXT_LINES};

use super::{GLOBAL, PATH_SEPARATOR};

/// Fields every commit is read with, in one order, so `log` and `show` cannot
/// disagree about what a commit looks like. `%x1f` is the unit separator: a
/// commit message can contain a newline and a tab, but not this.
pub const COMMIT_FORMAT: &str = "--format=%H%x1f%h%x1f%s%x1f%an%x1f%at";

/// Options every diff shares.
///
/// `--no-ext-diff` matters: a user with `diff.external` configured would
/// otherwise have their own diff tool run inside this app, and its output is
/// not what this parser reads. `core.quotepath=false` keeps a non-ASCII path
/// from arriving as `\303\251` escapes.
fn diff_head() -> Vec<String> {
    let mut argv = owned(&["-c", "core.quotepath=false"]);
    argv.extend(GLOBAL.iter().map(|a| (*a).to_string()));
    argv.extend(owned(&[
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--find-renames",
    ]));
    argv.push(format!("--unified={DIFF_CONTEXT_LINES}"));
    argv
}

pub fn diff_argv(request: &DiffRequest, guarded_path: Option<&str>) -> Vec<String> {
    let mut argv = diff_head();
    if request.staged {
        argv.push("--cached".to_string());
    }
    if let Some(against) = &request.against {
        argv.push(against.clone());
    }
    if let Some(path) = guarded_path {
        argv.push(PATH_SEPARATOR.to_string());
        argv.push(path.to_string());
    }
    argv
}

/// The diff a single commit introduced, for the history view.
///
/// `diff-tree --root`, not `diff <sha>^!`. The `^!` form looked right and is
/// wrong on the one commit that matters most for this: a **root commit** has
/// no parent, so `^!` degrades into `diff <sha>`, which compares the working
/// tree against it and answers *nothing* for a clean checkout. `--root` diffs
/// a parentless commit against the empty tree, which is what "what did this
/// commit introduce" means.
pub fn commit_diff_argv(sha: &str, guarded_path: Option<&str>) -> Vec<String> {
    let mut argv = owned(&["-c", "core.quotepath=false"]);
    argv.extend(GLOBAL.iter().map(|a| (*a).to_string()));
    argv.extend(owned(&[
        "diff-tree",
        "--no-commit-id",
        "--patch",
        "--root",
        "--no-color",
        "--no-ext-diff",
        "--find-renames",
    ]));
    argv.push(format!("--unified={DIFF_CONTEXT_LINES}"));
    argv.push(sha.to_string());
    if let Some(path) = guarded_path {
        argv.push(PATH_SEPARATOR.to_string());
        argv.push(path.to_string());
    }
    argv
}

/// `git log`, newest first, bounded like every other walk in this codebase.
///
/// Asks for one more commit than the caller wanted. That extra row is never
/// returned - it is how `GitLog::truncated` is answered without a second call
/// counting the whole history.
pub fn log_argv(request: &LogRequest, guarded_path: Option<&str>) -> Vec<String> {
    let mut argv = GLOBAL.iter().map(|a| (*a).to_string()).collect::<Vec<_>>();
    argv.extend(owned(&["log", "-z", COMMIT_FORMAT]));
    argv.push(format!("--max-count={}", request.effective_limit() + 1));
    if request.skip > 0 {
        argv.push(format!("--skip={}", request.skip));
    }
    if let Some(path) = guarded_path {
        argv.push(PATH_SEPARATOR.to_string());
        argv.push(path.to_string());
    }
    argv
}

/// One commit and the files it touched, in a single call.
pub fn show_argv(sha: &str) -> Vec<String> {
    let mut argv = owned(&["-c", "core.quotepath=false"]);
    argv.extend(GLOBAL.iter().map(|a| (*a).to_string()));
    argv.extend(owned(&[
        "show",
        "--no-color",
        "--name-status",
        "-z",
        "--find-renames",
        COMMIT_FORMAT,
    ]));
    argv.push(sha.to_string());
    argv
}

/// `--porcelain`, not `--line-porcelain`: the former reports a commit's details
/// only the first time it appears, which on a large file is the difference
/// between a few kilobytes and a few megabytes. Re-attaching them per line is
/// [`crate::git::blame`]'s job.
pub fn blame_argv(guarded_path: &str) -> Vec<String> {
    let mut argv = GLOBAL.iter().map(|a| (*a).to_string()).collect::<Vec<_>>();
    argv.extend(owned(&["blame", "--porcelain"]));
    argv.push(PATH_SEPARATOR.to_string());
    argv.push(guarded_path.to_string());
    argv
}

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

#[cfg(test)]
mod tests;
