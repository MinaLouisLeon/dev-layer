//! Argv for the call that creates a pull request, and for the one that asks
//! where a file lives on the web.
//!
//! Neither of these is a plain read, and each has its own reason for care.
//! `pr create` **writes**, so what it will make is confirmed in the UI before
//! it is called and its body never touches a command line. `browse` takes a
//! path, so the path is guarded against the connected root before it arrives -
//! see [`crate::github::browse`].

use super::owned;

/// Reads the body from standard input. The one flag in this module that
/// exists to keep a value *out* of argv.
const BODY_FROM_STDIN: &[&str] = &["--body-file", "-"];

/// Separates flags from positional arguments, exactly as `--` does for git
/// pathspecs: a file genuinely named `-n` is a file and not a flag.
const SEPARATOR: &str = "--";

/// `gh pr create --title <title> --base <base> [--draft] --body-file -`.
///
/// The head branch is deliberately not named: `gh` uses the branch that is
/// checked out, which is the one the author is looking at. Naming it here
/// would be this app deciding, from a branch value it read a moment ago, what
/// git already knows for certain now.
///
/// The **body is not in this array.** It goes to `gh` on stdin, so a
/// description containing quotes, newlines or an apostrophe is a description
/// rather than a quoting problem on a remote target. The title still travels
/// in argv and so still meets the SSH quoting rule; that limit is documented
/// in `docs/mino-workbench/github-module.md`.
pub fn create_pr_argv(title: &str, base: &str, draft: bool) -> Vec<String> {
    let mut argv = owned(&["pr", "create", "--title"]);
    argv.push(title.to_string());
    argv.push("--base".to_string());
    argv.push(base.to_string());
    if draft {
        argv.push("--draft".to_string());
    }
    argv.extend(owned(BODY_FROM_STDIN));
    argv
}

/// `gh browse --no-browser [--branch <branch>] -- <path>[:<line>]`.
///
/// `--no-browser` is the whole point: this asks `gh` where the file *is* and
/// nothing else. Opening it is the caller's decision, made in the UI through
/// the desktop opener, so a transport method called `query` never launches a
/// browser as a side effect.
///
/// `target` is built by [`crate::github::browse::target`], which has already
/// ruled the path against the connected root. It is placed after `--` so that
/// a repository-relative path beginning with a dash cannot be read as a flag.
pub fn browse_argv(target: &str, branch: Option<&str>) -> Vec<String> {
    let mut argv = owned(&["browse", "--no-browser"]);
    if let Some(branch) = branch {
        argv.push("--branch".to_string());
        argv.push(branch.to_string());
    }
    argv.push(SEPARATOR.to_string());
    argv.push(target.to_string());
    argv
}
