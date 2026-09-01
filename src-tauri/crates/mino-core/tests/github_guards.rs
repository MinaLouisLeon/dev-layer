//! What a query refuses, and the shape it declares.
//!
//! The other half of `github_command.rs`. That file asserts each query builds
//! the argv it should; this one asserts that the values which must never reach
//! an argv do not - and that a caller cannot ask for one thing and be handed
//! another.
//!
//! Every guard runs in `plan`, on the near side of any process. Nothing below
//! reaches `gh` at all: a refusal here is a refusal before a command exists.

use mino_core::git::paths::PathStyle;
use mino_core::github::call::plan;
use mino_core::types::{GitHubQuery, IssueState, PrState, MAX_GITHUB_LIMIT};
use mino_core::TransportError;

const ROOT: &str = "/srv/app";

fn planned(query: &GitHubQuery) -> Vec<String> {
    plan(query, ROOT, PathStyle::posix())
        .expect("this query should plan")
        .argv
}

#[test]
fn a_branch_name_that_could_be_read_as_an_option_never_reaches_argv() {
    // The same guard `git checkout` uses. `-x` is a legal ref name and an
    // illegal thing to hand a command, so `check-ref-format` would accept it
    // and this is the only check that catches it.
    for branch in ["-x", "a branch", "it's", ""] {
        let refused = plan(
            &GitHubQuery::Runs {
                branch: branch.to_string(),
                limit: 5,
            },
            ROOT,
            PathStyle::posix(),
        );
        assert!(
            matches!(refused.unwrap_err(), TransportError::InvalidArgument { .. }),
            "{branch:?} should be refused"
        );
    }
}

#[test]
fn a_base_branch_gets_the_same_guard_as_any_other_branch_name() {
    let refused = plan(
        &GitHubQuery::CreatePullRequest {
            title: "Fine".to_string(),
            body: String::new(),
            base: "-x".to_string(),
            draft: false,
        },
        ROOT,
        PathStyle::posix(),
    );
    assert!(matches!(
        refused.unwrap_err(),
        TransportError::InvalidArgument { .. }
    ));
}

#[test]
fn a_pull_request_with_no_title_is_refused_before_a_command_exists() {
    for title in ["", "   ", "one\ntwo"] {
        let refused = plan(
            &GitHubQuery::CreatePullRequest {
                title: title.to_string(),
                body: String::new(),
                base: "main".to_string(),
                draft: false,
            },
            ROOT,
            PathStyle::posix(),
        );
        assert!(refused.is_err(), "{title:?} should be refused");
    }
}

#[test]
fn a_limit_is_a_number_and_is_clamped_rather_than_refused() {
    // Clamped, because a section asking for more rows than the rate limit
    // deserves is a bug worth capping - not a reason to show the reader an
    // error about a number they never typed.
    let argv = planned(&GitHubQuery::PullRequests {
        state: PrState::Open,
        limit: 10_000,
    });
    let flag = argv.iter().position(|a| a == "--limit").unwrap();
    assert_eq!(argv[flag + 1], MAX_GITHUB_LIMIT.to_string());

    // And zero is not a thing to ask gh for.
    let argv = planned(&GitHubQuery::Issues {
        state: IssueState::Open,
        limit: 0,
    });
    let flag = argv.iter().position(|a| a == "--limit").unwrap();
    assert_eq!(argv[flag + 1], "1");
}

#[test]
fn browse_refuses_a_path_outside_the_connected_root() {
    for path in [
        "/etc/passwd",
        "/srv/app/../secrets.txt",
        "/srv/other/x.rs",
        // The root itself. An operation that names it is not one this takes.
        "/srv/app",
        "",
    ] {
        let refused = plan(
            &GitHubQuery::BrowseUrl {
                path: path.to_string(),
                line: None,
                branch: None,
            },
            ROOT,
            PathStyle::posix(),
        );
        assert!(refused.is_err(), "{path:?} should be refused");
    }
}

#[test]
fn a_line_of_zero_is_a_caller_that_has_not_decided() {
    let argv = planned(&GitHubQuery::BrowseUrl {
        path: "/srv/app/src/main.rs".to_string(),
        line: Some(0),
        branch: None,
    });
    assert_eq!(argv[argv.len() - 1], "src/main.rs");
    assert!(!argv.contains(&"--branch".to_string()));
}

#[test]
fn a_browse_target_is_root_relative_and_sits_behind_the_separator() {
    let argv = planned(&GitHubQuery::BrowseUrl {
        path: "/srv/app/src/main.rs".to_string(),
        line: Some(42),
        branch: Some("main".to_string()),
    });
    assert!(argv.contains(&"--no-browser".to_string()));
    // After `--`, so a repository-relative path beginning with a dash cannot
    // be read as a flag.
    assert_eq!(argv[argv.len() - 2], "--");
    assert_eq!(argv[argv.len() - 1], "src/main.rs:42");
}
