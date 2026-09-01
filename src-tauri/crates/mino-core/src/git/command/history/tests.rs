//! What the history argv puts where.
//!
//! The assertions worth having are about placement rather than spelling: a
//! path that is not behind `--` can be read as a revision, and a revision that
//! is behind it cannot be read at all.

use super::*;

#[test]
fn a_diff_path_sits_behind_the_separator() {
    let argv = diff_argv(&DiffRequest::worktree(), Some("-weird.rs"));
    let sep = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
    let path = argv.iter().position(|a| a == "-weird.rs").unwrap();
    assert!(sep < path, "{argv:?}");
}

#[test]
fn a_revision_sits_in_front_of_it() {
    // The other way round from a path: `--` would make git read `main` as a
    // filename, and there is rarely a file called `main`.
    let argv = diff_argv(
        &DiffRequest::worktree().against("main"),
        Some("src/main.rs"),
    );
    let revision = argv.iter().position(|a| a == "main").unwrap();
    let sep = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
    assert!(revision < sep, "{argv:?}");
}

#[test]
fn staged_and_worktree_are_different_calls() {
    let worktree = diff_argv(&DiffRequest::worktree(), None);
    let staged = diff_argv(&DiffRequest::worktree().staged(true), None);
    assert!(!worktree.contains(&"--cached".to_string()));
    assert!(staged.contains(&"--cached".to_string()));
}

#[test]
fn every_diff_refuses_an_external_diff_tool() {
    // A user with `diff.external` set would otherwise have their own tool run
    // inside this app, and its output is not what the parser reads.
    for argv in [
        diff_argv(&DiffRequest::worktree(), None),
        commit_diff_argv("3f2a1c9", None),
    ] {
        assert!(argv.contains(&"--no-ext-diff".to_string()), "{argv:?}");
        assert!(argv.contains(&"--no-color".to_string()), "{argv:?}");
    }
}

#[test]
fn a_commit_diff_works_on_a_root_commit() {
    // `diff-tree --root`, not `diff <sha>^!`. The `^!` form degrades into
    // `diff <sha>` on a parentless commit, which compares the *working tree*
    // against it and answers nothing at all for a clean checkout.
    let argv = commit_diff_argv("3f2a1c9", None);
    assert!(argv.contains(&"diff-tree".to_string()), "{argv:?}");
    assert!(argv.contains(&"--root".to_string()), "{argv:?}");
    assert!(argv.contains(&"3f2a1c9".to_string()), "{argv:?}");
    assert!(!argv.iter().any(|a| a.contains('^')), "{argv:?}");
}

#[test]
fn log_asks_for_one_more_than_it_was_told_to() {
    // That extra row is how `truncated` is answered without a second call
    // counting the whole history. It is never returned.
    let argv = log_argv(&LogRequest::new().limit(50), None);
    assert!(argv.contains(&"--max-count=51".to_string()), "{argv:?}");

    // And the ceiling still applies to what was asked for.
    let argv = log_argv(&LogRequest::new().limit(9_000), None);
    assert!(argv.contains(&"--max-count=501".to_string()), "{argv:?}");
}

#[test]
fn skip_is_only_sent_when_there_is_something_to_skip() {
    assert!(!log_argv(&LogRequest::new(), None)
        .iter()
        .any(|a| a.starts_with("--skip")));
    assert!(log_argv(&LogRequest::new().skip(50), None).contains(&"--skip=50".to_string()));
}

#[test]
fn log_and_show_read_a_commit_the_same_way() {
    // One format constant, so the two cannot drift into disagreeing about
    // what a commit looks like.
    assert!(log_argv(&LogRequest::new(), None).contains(&COMMIT_FORMAT.to_string()));
    assert!(show_argv("3f2a1c9").contains(&COMMIT_FORMAT.to_string()));
}

#[test]
fn blame_always_names_its_path_behind_the_separator() {
    let argv = blame_argv("src/main.rs");
    let sep = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
    assert_eq!(argv.get(sep + 1).map(String::as_str), Some("src/main.rs"));
    assert!(argv.contains(&"--porcelain".to_string()));
}
