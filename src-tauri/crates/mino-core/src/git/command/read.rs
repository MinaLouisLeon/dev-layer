//! Argv for the calls that only *read*.
//!
//! Nothing here takes a caller value at all, which is the strongest form the
//! injection rule takes: there is no path, no branch name and no message for
//! anything to be spliced into. The mutating half is in [`super::write`],
//! where paths do appear and are guarded before they arrive.

use super::GLOBAL;

/// `git rev-parse --show-toplevel`: the work tree root, or exit 128 when the
/// directory is not inside a repository. That failure is the *answer* to
/// "is this a repository", not an error - see [`super::not_a_repository`].
pub fn repository_argv() -> Vec<&'static str> {
    let mut argv = GLOBAL.to_vec();
    argv.extend_from_slice(&["rev-parse", "--show-toplevel"]);
    argv
}

/// The one status call the whole phase is served by.
///
/// - `--porcelain=v2` is a documented, versioned format, not scraped human
///   text, and unlike v1 it carries the branch, the upstream and the
///   ahead/behind counts in the same pass.
/// - `-z` makes records NUL-terminated, which is the only way a filename
///   containing a newline or a quote survives intact.
/// - `--untracked-files=all` lists files inside an untracked directory
///   individually, because the tree decorates files and not just folders.
/// - `--ignored=matching` reports a directory that matches an ignore pattern
///   as one row instead of recursing into it, which is what keeps
///   `node_modules` from becoming forty thousand rows.
pub fn status_argv() -> Vec<&'static str> {
    let mut argv = GLOBAL.to_vec();
    argv.extend_from_slice(&[
        "status",
        "--porcelain=v2",
        "-z",
        "--branch",
        "--untracked-files=all",
        "--ignored=matching",
    ]);
    argv
}

/// The headers alone: branch, head, upstream and the ahead/behind counts.
///
/// `repository()` is asked on every connect, before anything is rendered, and
/// it does not need the file rows. Skipping the untracked walk turns a call
/// that can take a second on a large checkout into one that returns
/// immediately - and the headers come from a real status, so they cannot
/// disagree with the one the tree reads.
pub fn branch_argv() -> Vec<&'static str> {
    let mut argv = GLOBAL.to_vec();
    argv.extend_from_slice(&[
        "status",
        "--porcelain=v2",
        "-z",
        "--branch",
        "--untracked-files=no",
        "--ignored=no",
    ]);
    argv
}

/// The cheap half of a status, for the search walk's ignore predicate.
///
/// Search does not care what changed, only what git would not look at, so it
/// skips the branch headers and asks for `normal` rather than `all`, which
/// collapses each untracked directory into one row instead of listing its
/// contents. That is most of the cost of a status, and search runs this on
/// every query.
///
/// `--untracked-files=no` is not an option here even though it would be
/// cheaper still: git refuses that combination outright ("Unsupported
/// combination of ignored and untracked-files arguments"), because working out
/// what is ignored *is* the untracked walk.
pub fn ignored_argv() -> Vec<&'static str> {
    let mut argv = GLOBAL.to_vec();
    argv.extend_from_slice(&[
        "status",
        "--porcelain=v2",
        "-z",
        "--untracked-files=normal",
        "--ignored=matching",
    ]);
    argv
}

/// The commit at HEAD, in one NUL-separated line.
///
/// Run straight after a successful `commit`, rather than scraping the sha out
/// of git's own commit output: that output is human text that changes between
/// versions, and `--format` is a documented interface.
pub fn head_commit_argv() -> Vec<&'static str> {
    let mut argv = GLOBAL.to_vec();
    argv.extend_from_slice(&["log", "-1", "-z", "--format=%H%x00%h%x00%s%x00%an%x00%at"]);
    argv
}

/// `git --version`, the probe. Cheap, and it answers the only question worth
/// asking up front: is there a usable git here at all.
pub fn version_argv() -> Vec<&'static str> {
    vec!["--version"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_options_precede_the_subcommand() {
        let argv = status_argv();
        let lock = argv.iter().position(|a| *a == "--no-optional-locks");
        let status = argv.iter().position(|a| *a == "status");
        assert!(lock < status);
    }

    #[test]
    fn no_function_here_takes_a_caller_value() {
        // The point of the file, asserted by its own signatures: every one of
        // these is callable with no arguments, so there is nothing for a path
        // or a branch name to be spliced into.
        for argv in [
            repository_argv(),
            status_argv(),
            branch_argv(),
            ignored_argv(),
            head_commit_argv(),
            version_argv(),
        ] {
            assert!(!argv.is_empty());
        }
    }
}
