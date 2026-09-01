//! Argv for the calls that *change* something.
//!
//! Two differences from [`super::read`], both deliberate.
//!
//! These build `Vec<String>` rather than `Vec<&'static str>`, because paths
//! are caller values and have to be owned. Every one of them has already been
//! through [`crate::git::guard`] by the time it arrives; this file's job is to
//! place them where git reads them as paths and nothing else, which is what
//! [`PATH_SEPARATOR`] does.
//!
//! And `--no-optional-locks` is absent. These calls are *meant* to take the
//! index lock - they are writing to the index - and asking git to avoid it
//! would be asking it not to do the thing it was called for.

use crate::types::CommitRequest;

use super::PATH_SEPARATOR;

/// `git add`. An empty `paths` stages everything, which is what the
/// group-level control sends.
///
/// `--all` rather than `.` for the everything case: `.` is relative to the
/// working directory, so on a session rooted below the repository root it
/// would quietly stage only part of the tree.
pub fn stage_argv(paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        return owned(&["add", "--all"]);
    }
    with_paths(&["add"], paths)
}

/// `git reset`, and not `git restore --staged`.
///
/// `restore --staged` reads HEAD, so it fails outright on an unborn branch -
/// a repository where `git init` has run and nothing has been committed yet,
/// which is exactly when someone is most likely to stage something and change
/// their mind. `reset` works in both cases.
pub fn unstage_argv(paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        return owned(&["reset", "--quiet"]);
    }
    with_paths(&["reset", "--quiet"], paths)
}

/// `git restore --worktree`: put the file back to what is staged, or to HEAD
/// when nothing is staged for it.
///
/// **This does not delete untracked files.** `git restore` only touches paths
/// git is tracking, so discarding an untracked file leaves it alone. That is a
/// deliberate limit rather than an oversight: deleting a file git has never
/// seen is unrecoverable by any means - no commit, no stash, no reflog - and
/// the UI says so instead of offering it. See `docs/mino-workbench/git-module.md`.
pub fn discard_argv(paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        // `:/` is a pathspec meaning "everything in the repository", and
        // unlike `.` it does not depend on the working directory.
        return with_paths(&["restore", "--worktree"], &[":/".to_string()]);
    }
    with_paths(&["restore", "--worktree"], paths)
}

/// `git commit`, with the message on stdin.
///
/// `--file -` is the whole point: see the module doc on [`super`]. The caller
/// must write [`CommitRequest::trimmed`] to the child's stdin and close it, or
/// git waits forever.
///
/// `--cleanup=strip` drops trailing blank lines and comment lines, which is
/// what git would do for an interactively written message anyway.
pub fn commit_argv(request: &CommitRequest) -> Vec<String> {
    let mut argv = owned(&["commit", "--quiet", "--file", "-", "--cleanup=strip"]);
    if request.all {
        argv.push("--all".to_string());
    }
    if request.amend {
        argv.push("--amend".to_string());
    }
    argv
}

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

/// The shape every path-taking call has: options, the separator, then paths.
fn with_paths(head: &[&str], paths: &[String]) -> Vec<String> {
    let mut argv = owned(head);
    argv.push(PATH_SEPARATOR.to_string());
    argv.extend(paths.iter().cloned());
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_always_sit_behind_the_separator() {
        // A file really called `-f` is a file. Without `--` git would read it
        // as a flag, and `git add -f` means something else entirely.
        for argv in [
            stage_argv(&["-f".to_string()]),
            unstage_argv(&["-f".to_string()]),
            discard_argv(&["-f".to_string()]),
        ] {
            let separator = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
            let path = argv.iter().position(|a| a == "-f").unwrap();
            assert!(separator < path, "{argv:?}");
        }
    }

    #[test]
    fn an_empty_slice_means_everything() {
        assert_eq!(stage_argv(&[]), vec!["add", "--all"]);
        assert_eq!(unstage_argv(&[]), vec!["reset", "--quiet"]);
        // Discard-everything is still a pathspec, and one that does not depend
        // on which directory the call runs in.
        assert!(discard_argv(&[]).contains(&":/".to_string()));
    }

    #[test]
    fn the_commit_message_never_appears_in_argv() {
        let request = CommitRequest::new("Fix Bob's bug\n\nWith a body.");
        let argv = commit_argv(&request);
        assert!(
            !argv.iter().any(|arg| arg.contains("Bob")),
            "the message must travel on stdin, not in argv: {argv:?}"
        );
        assert_eq!(argv.iter().filter(|a| *a == "-").count(), 1);
    }

    #[test]
    fn all_and_amend_are_flags_not_defaults() {
        let plain = commit_argv(&CommitRequest::new("m"));
        assert!(!plain.contains(&"--all".to_string()));
        assert!(!plain.contains(&"--amend".to_string()));

        let both = commit_argv(&CommitRequest::new("m").all(true).amend(true));
        assert!(both.contains(&"--all".to_string()));
        assert!(both.contains(&"--amend".to_string()));
    }
}
