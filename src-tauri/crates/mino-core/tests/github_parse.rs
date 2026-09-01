//! `gh run list` and `gh run view` output into typed rows.
//!
//! The two calls the checks section makes, and the one place a run's `status`
//! and `conclusion` become the single state the UI renders.
//!
//! The pull request and issue rows are in `github_parse_rows.rs`, and what
//! happens when the JSON is not what this build expected is in
//! `github_parse_failures.rs`.

use mino_core::git::GitOutput;
use mino_core::github::call::{read, Shape};
use mino_core::types::{GitHubCheckState, GitHubResponse};

fn ok(stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

const RUNS: &str = r#"[
  {
    "databaseId": 918273,
    "workflowName": "CI",
    "displayTitle": "feat(github): the checks section",
    "headBranch": "feat/github-integration",
    "status": "completed",
    "conclusion": "failure",
    "url": "https://github.com/o/r/actions/runs/918273",
    "startedAt": "2026-08-30T09:41:12Z"
  },
  {
    "databaseId": 918272,
    "workflowName": "CI",
    "displayTitle": "still going",
    "headBranch": "feat/github-integration",
    "status": "in_progress",
    "conclusion": null,
    "url": "https://github.com/o/r/actions/runs/918272",
    "startedAt": null
  }
]"#;

#[test]
fn runs_carry_their_state_and_their_time_in_the_unit_everything_else_uses() {
    let GitHubResponse::Runs(runs) = read(Shape::Runs, &ok(RUNS)).unwrap() else {
        panic!("a runs query answers with runs");
    };
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].id, 918_273);
    assert_eq!(runs[0].workflow, "CI");
    assert_eq!(runs[0].state, GitHubCheckState::Failed);
    // Epoch milliseconds, not the ISO string gh sent.
    assert_eq!(runs[0].started_ms, Some(1_788_082_872_000));

    // A run still going has no conclusion, and that is a state rather than a
    // missing field.
    assert_eq!(runs[1].state, GitHubCheckState::Running);
    assert_eq!(runs[1].started_ms, None);
}

#[test]
fn jobs_come_out_of_the_object_gh_wraps_them_in() {
    let stdout = r#"{"jobs":[
        {"name":"build","status":"completed","conclusion":"success","url":"https://x/1"},
        {"name":"test (windows)","status":"completed","conclusion":"failure","url":"https://x/2"}
    ]}"#;
    let GitHubResponse::Jobs(jobs) = read(Shape::Jobs, &ok(stdout)).unwrap() else {
        panic!("a jobs query answers with jobs");
    };
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[1].name, "test (windows)");
    assert_eq!(jobs[1].state, GitHubCheckState::Failed);
}

#[test]
fn a_browse_answer_is_the_one_line_gh_printed() {
    let GitHubResponse::Url(url) = read(
        Shape::Url,
        &ok("https://github.com/o/r/blob/main/a.rs#L4\n"),
    )
    .unwrap() else {
        panic!("a browse query answers with a url");
    };
    assert_eq!(url, "https://github.com/o/r/blob/main/a.rs#L4");
}
