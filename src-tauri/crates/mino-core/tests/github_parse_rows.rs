//! `gh pr list`, `gh pr view` and `gh issue list` output into typed rows.
//!
//! Two things are asserted here and only one of them is parsing. The other is
//! that **nothing GitHub said becomes anything but text**: a title full of
//! markup arrives as a title full of markup, unchanged and un-interpreted.
//! Every title and label on this surface was written by whoever opened the
//! thing, which on a public repository is anybody at all.

use mino_core::git::GitOutput;
use mino_core::github::call::{read, Shape};
use mino_core::types::{GitHubCheckState, GitHubPrState, GitHubResponse};

fn ok(stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

#[test]
fn a_pull_request_rolls_up_both_kinds_of_check() {
    // A repository can carry check runs and older commit statuses at once. A
    // rollup that read only one half would report green for a red build.
    let stdout = r#"[{
        "number": 15,
        "title": "feat(git): branches and stash",
        "author": {"login": "MinaLouisLeon"},
        "url": "https://github.com/o/r/pull/15",
        "state": "OPEN",
        "isDraft": false,
        "headRefName": "feat/git-branches-stash",
        "baseRefName": "main",
        "updatedAt": "2026-08-30T09:41:12Z",
        "statusCheckRollup": [
            {"status": "COMPLETED", "conclusion": "SUCCESS"},
            {"state": "FAILURE"}
        ]
    }]"#;
    let GitHubResponse::PullRequests(prs) = read(Shape::PullRequests, &ok(stdout)).unwrap() else {
        panic!("a pull request list answers with pull requests");
    };
    assert_eq!(prs[0].number, 15);
    assert_eq!(prs[0].author, "MinaLouisLeon");
    assert_eq!(prs[0].state, GitHubPrState::Open);
    assert_eq!(prs[0].checks, GitHubCheckState::Failed);
    // A list carries no bodies: they are the largest field and are read one
    // at a time.
    assert!(prs[0].body.is_none());
}

#[test]
fn a_repository_with_no_checks_is_not_a_repository_whose_checks_passed() {
    let stdout = r#"[{
        "number": 1, "title": "t", "author": {"login": "a"},
        "url": "https://x", "state": "OPEN", "isDraft": true,
        "headRefName": "h", "baseRefName": "main",
        "updatedAt": "2026-08-30T09:41:12Z", "statusCheckRollup": []
    }]"#;
    let GitHubResponse::PullRequests(prs) = read(Shape::PullRequests, &ok(stdout)).unwrap() else {
        panic!("a pull request list answers with pull requests");
    };
    assert_eq!(prs[0].checks, GitHubCheckState::Unknown);
    assert!(prs[0].is_draft);
}

#[test]
fn a_deleted_author_is_an_absent_value_and_not_a_shape_change() {
    let stdout = r#"[{
        "number": 1, "title": "t", "author": null,
        "url": "https://x", "state": "MERGED", "isDraft": false,
        "headRefName": "h", "baseRefName": "main",
        "updatedAt": null, "statusCheckRollup": null
    }]"#;
    let GitHubResponse::PullRequests(prs) = read(Shape::PullRequests, &ok(stdout)).unwrap() else {
        panic!("a pull request list answers with pull requests");
    };
    assert_eq!(prs[0].author, "");
    assert_eq!(prs[0].state, GitHubPrState::Merged);
    assert_eq!(prs[0].updated_ms, None);
}

#[test]
fn issue_labels_are_names_and_the_row_survives_an_odd_one() {
    let stdout = r#"[{
        "number": 7, "title": "The tree forgets its expansion",
        "author": {"login": "someone"}, "url": "https://x",
        "state": "OPEN",
        "labels": [{"name": "bug", "color": "d73a4a"}, {"color": "ffffff"}],
        "updatedAt": "2026-08-30T09:41:12Z"
    }]"#;
    let GitHubResponse::Issues(issues) = read(Shape::Issues, &ok(stdout)).unwrap() else {
        panic!("an issues query answers with issues");
    };
    assert_eq!(issues[0].labels, vec!["bug".to_string()]);
    assert_eq!(issues[0].number, 7);
}

#[test]
fn a_title_full_of_markup_arrives_as_a_title_full_of_markup() {
    // Untrusted input, carried as text. Nothing here interprets it, and the
    // UI renders it as text - see the TypeScript suite for the other half of
    // this promise.
    let stdout = r#"[{
        "number": 1, "title": "<img src=x onerror=alert(1)> & \"quotes\"",
        "author": {"login": "a"}, "url": "https://x", "state": "OPEN",
        "labels": [], "updatedAt": null
    }]"#;
    let GitHubResponse::Issues(issues) = read(Shape::Issues, &ok(stdout)).unwrap() else {
        panic!("an issues query answers with issues");
    };
    assert_eq!(issues[0].title, "<img src=x onerror=alert(1)> & \"quotes\"");
}

#[test]
fn an_empty_list_is_a_state_and_not_a_failure() {
    let GitHubResponse::PullRequests(prs) = read(Shape::PullRequests, &ok("[]")).unwrap() else {
        panic!("a pull request list answers with pull requests");
    };
    assert!(prs.is_empty());
}
