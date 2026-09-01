//! Against records shaped exactly as `BRANCH_FORMAT` produces them.
//!
//! The end-to-end proof - that git actually emits this - is in
//! `tests/git_branches.rs`, which runs the binary. These are about the
//! columns: which field is which, and what happens when one of them is empty.

use super::*;

fn output(stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

/// One row with the eleven fields in `BRANCH_FORMAT` order.
fn record(fields: [&str; 11]) -> String {
    fields.join("\u{1f}")
}

fn local(name: &str, head: &str) -> String {
    record([
        head,
        &format!("refs/heads/{name}"),
        name,
        "",
        "",
        "",
        "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
        "3f2a1c9",
        "a change",
        "Test",
        "1788024729",
    ])
}

#[test]
fn the_checked_out_branch_is_the_one_git_marked() {
    let stdout = format!("{}\n{}\n", local("main", " "), local("dev", "*"));
    let branches = parse(&output(&stdout)).unwrap();

    assert_eq!(branches.len(), 2);
    assert!(!branches[0].is_head);
    assert!(branches[1].is_head);
    assert_eq!(branches[1].name, "dev");
}

#[test]
fn a_tracking_branch_reports_its_upstream_and_counts() {
    let stdout = record([
        "*",
        "refs/heads/dev",
        "dev",
        "origin/dev",
        "ahead 2, behind 1",
        "",
        "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
        "3f2a1c9",
        "a change",
        "Test",
        "1788024729",
    ]);
    let branch = parse(&output(&stdout)).unwrap().remove(0);

    assert_eq!(branch.upstream.as_deref(), Some("origin/dev"));
    assert_eq!((branch.ahead, branch.behind), (2, 1));
    // Seconds from git, milliseconds everywhere on this interface.
    assert_eq!(branch.last_commit.unwrap().timestamp_ms, 1_788_024_729_000);
}

#[test]
fn one_sided_and_absent_tracking_are_both_zeroes_not_failures() {
    assert_eq!(track("ahead 3"), (3, 0));
    assert_eq!(track("behind 4"), (0, 4));
    // `gone` is a branch whose upstream was deleted. It is still a branch.
    assert_eq!(track("gone"), (0, 0));
    assert_eq!(track(""), (0, 0));
}

#[test]
fn a_remote_ref_is_marked_by_its_full_name_not_its_short_one() {
    // The short name of `refs/remotes/origin/main` is `origin/main`, which a
    // local branch is perfectly entitled to be called.
    let stdout = record([
        " ",
        "refs/remotes/origin/main",
        "origin/main",
        "",
        "",
        "",
        "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
        "3f2a1c9",
        "a change",
        "Test",
        "1788024729",
    ]);
    let branch = parse(&output(&stdout)).unwrap().remove(0);
    assert!(branch.is_remote);
    assert_eq!(branch.name, "origin/main");
}

#[test]
fn the_remote_head_symref_is_dropped() {
    // `origin/HEAD` points at a row already in the list. Offering it would be
    // offering one branch twice under two names.
    let stdout = record([
        " ",
        "refs/remotes/origin/HEAD",
        "origin/HEAD",
        "",
        "",
        "refs/remotes/origin/main",
        "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39",
        "3f2a1c9",
        "a change",
        "Test",
        "1788024729",
    ]);
    assert!(parse(&output(&stdout)).unwrap().is_empty());
}

#[test]
fn a_row_with_no_sha_is_a_branch_with_no_commit() {
    let stdout = record([
        "*",
        "refs/heads/main",
        "main",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
    ]);
    let branch = parse(&output(&stdout)).unwrap().remove(0);
    assert_eq!(branch.name, "main");
    assert!(branch.last_commit.is_none());
}

#[test]
fn a_row_this_parser_does_not_recognise_is_dropped_not_guessed_at() {
    let stdout = "* \u{1f}refs/heads/main\u{1f}main\n";
    assert!(parse(&output(stdout)).unwrap().is_empty());
}

#[test]
fn a_failed_call_is_an_error_and_not_an_empty_list() {
    let failed = GitOutput {
        code: Some(128),
        stdout: String::new(),
        stderr: "fatal: not a git repository".to_string(),
    };
    assert!(parse(&failed).is_err());
}
