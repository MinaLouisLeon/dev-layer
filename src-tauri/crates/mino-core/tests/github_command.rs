//! What each query becomes, and the rule it has to keep.
//!
//! **The rule:** a caller value is its own argument. The assertions below check
//! that a value does *not* appear inside a joined argv - which is what would
//! happen if any of these functions had built a command line instead of an
//! array. A value that survives joining is a value the shell could parse.
//!
//! The other half of the rule is held by the type system rather than by a
//! test: a caller cannot name a subcommand at all, because `GitHubQuery` is an
//! enum and no variant of it carries program text.
//!
//! What each query *refuses* is next door, in `github_guards.rs`.

use mino_core::git::paths::PathStyle;
use mino_core::github::call::{plan, Shape};
use mino_core::types::{GitHubQuery, IssueState, PrState};

const ROOT: &str = "/srv/app";

fn planned(query: &GitHubQuery) -> Vec<String> {
    plan(query, ROOT, PathStyle::posix())
        .expect("this query should plan")
        .argv
}

/// The value of the argument following `flag`, which is the shape every caller
/// value on this interface travels in.
fn after(argv: &[String], flag: &str) -> String {
    let at = argv.iter().position(|arg| arg == flag).expect(flag);
    argv[at + 1].clone()
}

#[test]
fn a_run_list_names_the_branch_as_its_own_argument() {
    let argv = planned(&GitHubQuery::Runs {
        branch: "feat/github-integration".to_string(),
        limit: 5,
    });
    assert_eq!(&argv[..2], &["run", "list"]);
    // Behind an explicit flag, never positional, so it cannot be read as
    // anything else - and as one argument, so it cannot become two.
    assert_eq!(after(&argv, "--branch"), "feat/github-integration");
}

#[test]
fn a_run_id_and_a_pull_request_number_are_numbers_not_text() {
    assert_eq!(
        planned(&GitHubQuery::RunJobs { run_id: 918_273 }),
        vec!["run", "view", "918273", "--json", "jobs"]
    );
    let argv = planned(&GitHubQuery::PullRequest { number: 42 });
    assert_eq!(&argv[..3], &["pr", "view", "42"]);
}

#[test]
fn a_list_filter_is_an_enum_and_never_free_text() {
    for (state, word) in [
        (PrState::Open, "open"),
        (PrState::Closed, "closed"),
        (PrState::Merged, "merged"),
        (PrState::All, "all"),
    ] {
        let argv = planned(&GitHubQuery::PullRequests { state, limit: 20 });
        assert_eq!(after(&argv, "--state"), word);
    }
    let issues = GitHubQuery::Issues {
        state: IssueState::Closed,
        limit: 20,
    };
    assert_eq!(after(&planned(&issues), "--state"), "closed");
}

#[test]
fn every_read_names_its_json_fields_and_the_shape_of_its_own_answer() {
    // Naming the fields is the whole mitigation for gh changing shape between
    // versions: a field this build needs and gh no longer has is a non-zero
    // exit, and a field gh added that this build does not name costs nothing.
    //
    // The shape is carried rather than inferred afterwards, so reading an
    // answer never involves guessing which question produced it.
    let reads = [
        (
            GitHubQuery::Runs {
                branch: "main".to_string(),
                limit: 1,
            },
            Shape::Runs,
        ),
        (GitHubQuery::RunJobs { run_id: 1 }, Shape::Jobs),
        (
            GitHubQuery::PullRequests {
                state: PrState::Open,
                limit: 1,
            },
            Shape::PullRequests,
        ),
        (GitHubQuery::PullRequest { number: 1 }, Shape::PullRequest),
        (
            GitHubQuery::Issues {
                state: IssueState::Open,
                limit: 1,
            },
            Shape::Issues,
        ),
    ];
    for (query, shape) in reads {
        let call = plan(&query, ROOT, PathStyle::posix()).unwrap();
        assert!(call.argv.contains(&"--json".to_string()), "{query:?}");
        assert_eq!(call.shape, shape, "{query:?}");
        // A read sends nothing on stdin. Only the one call that writes does.
        assert!(call.input.is_none(), "{query:?}");
    }
}

#[test]
fn a_title_is_one_argument_and_a_body_is_not_an_argument_at_all() {
    let query = GitHubQuery::CreatePullRequest {
        title: "Bring the checks in; it's overdue".to_string(),
        body: "A body with 'quotes', a ; and\nnewlines.".to_string(),
        base: "main".to_string(),
        draft: true,
    };
    let call = plan(&query, ROOT, PathStyle::posix()).unwrap();

    // The title survives as exactly one argument, semicolon and all.
    let title = after(&call.argv, "--title");
    assert_eq!(title, "Bring the checks in; it's overdue");
    // And the body is nowhere in the argument list. It travels on stdin, which
    // is the whole reason a description may contain an apostrophe.
    assert!(!call.argv.join(" ").contains("newlines"), "{:?}", call.argv);
    assert_eq!(
        call.input.as_deref(),
        Some("A body with 'quotes', a ; and\nnewlines.")
    );
    assert!(call.argv.contains(&"--draft".to_string()));
    assert_eq!(call.shape, Shape::Created);
}

#[test]
fn a_draft_is_a_flag_and_never_a_default() {
    let published = GitHubQuery::CreatePullRequest {
        title: "Ready".to_string(),
        body: String::new(),
        base: "main".to_string(),
        draft: false,
    };
    assert!(!planned(&published).contains(&"--draft".to_string()));
}
