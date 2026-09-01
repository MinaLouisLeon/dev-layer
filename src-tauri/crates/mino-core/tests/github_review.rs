//! Review comments into threads - #17.
//!
//! The interesting assertions are all about **anchoring**. A review comment is
//! attached to a position in a diff, not to a line in a file, and when the
//! pull request gains commits that diff stops being current. GitHub then
//! reports the comment with a null line, and the rule this suite exists to
//! hold is that such a thread is reported as outdated and given no line at
//! all. It is never quietly moved to `original_line`, which would put

use mino_core::git::GitOutput;
use mino_core::github::call::{read, Shape};
use mino_core::types::GitHubResponse;

fn ok(stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

/// Two threads: one placeable, one whose diff has moved on.
const COMMENTS: &str = r#"[
  {
    "id": 111,
    "path": "src/main.rs",
    "line": 12,
    "body": "This could be clearer.",
    "html_url": "https://github.com/o/r/pull/1#discussion_r111",
    "created_at": "2026-08-30T09:41:12Z",
    "user": { "login": "a-reviewer" },
    "in_reply_to_id": null
  },
  {
    "id": 112,
    "path": "src/main.rs",
    "line": 12,
    "body": "Agreed, will fix.",
    "html_url": "https://github.com/o/r/pull/1#discussion_r112",
    "created_at": "2026-08-30T10:00:00Z",
    "user": { "login": "the-author" },
    "in_reply_to_id": 111
  },
  {
    "id": 200,
    "path": "src/old.rs",
    "line": null,
    "body": "This whole block is wrong.",
    "html_url": "https://github.com/o/r/pull/1#discussion_r200",
    "created_at": "2026-08-29T09:00:00Z",
    "user": { "login": "a-reviewer" },
    "in_reply_to_id": null
  }
]"#;

fn threads(stdout: &str) -> Vec<mino_core::types::GitHubReviewThread> {
    let GitHubResponse::ReviewThreads(threads) = read(Shape::ReviewThreads, &ok(stdout)).unwrap()
    else {
        panic!("a review query answers with threads");
    };
    threads
}

#[test]
fn replies_join_the_thread_they_answer() {
    // And a pull request with none at all is an empty list, not a failure.
    assert!(threads("[]").is_empty());

    let threads = threads(COMMENTS);
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].id, 111);
    assert_eq!(threads[0].comments.len(), 2);
    assert_eq!(threads[0].comments[1].author, "the-author");
    // The opening comment first, then its replies in the order they were left.
    assert_eq!(threads[0].comments[0].id, 111);
}

#[test]
fn a_placeable_thread_keeps_its_line() {
    let thread = &threads(COMMENTS)[0];
    assert_eq!(thread.line, Some(12));
    assert!(!thread.outdated);
    assert!(thread.is_placeable());
}

#[test]
fn a_thread_whose_diff_has_moved_on_is_outdated_and_has_no_line() {
    // The rule this whole feature turns on. GitHub reports `line: null`, and
    // that is the outdated test - not something inferred here, and not
    // something patched over with `original_line`.
    let thread = &threads(COMMENTS)[1];
    assert_eq!(thread.id, 200);
    assert!(thread.outdated);
    assert_eq!(thread.line, None);
    assert!(!thread.is_placeable());
    // And it is still carried, because the comment stands even though its
    // position does not.
    assert_eq!(thread.comments[0].body, "This whole block is wrong.");
}

#[test]
fn a_comment_body_full_of_markup_arrives_as_text() {
    let stdout = r#"[{
        "id": 1, "path": "a.rs", "line": 1,
        "body": "<img src=x onerror=alert(1)> & <b>bold</b>",
        "html_url": "https://x", "created_at": null,
        "user": { "login": "a" }, "in_reply_to_id": null
    }]"#;
    assert_eq!(
        threads(stdout)[0].comments[0].body,
        "<img src=x onerror=alert(1)> & <b>bold</b>"
    );
}

#[test]
fn a_reply_whose_parent_is_gone_becomes_a_thread_rather_than_vanishing() {
    // GitHub can report a reply whose parent was deleted. A comment nobody can
    // see is worse than one shown alone.
    let stdout = r#"[{
        "id": 9, "path": "a.rs", "line": 3, "body": "orphan",
        "html_url": "https://x", "created_at": null,
        "user": { "login": "a" }, "in_reply_to_id": 8
    }]"#;
    let threads = threads(stdout);
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].comments[0].body, "orphan");
}

#[test]
fn malformed_review_json_is_a_typed_protocol_error_and_never_a_panic() {
    for stdout in ["not json", "{}", r#"[{"id":"not a number"}]"#] {
        let failure = read(Shape::ReviewThreads, &ok(stdout)).unwrap_err();
        assert!(
            matches!(failure, mino_core::TransportError::Protocol { .. }),
            "{stdout:?} produced {failure:?}"
        );
    }
}

#[test]
fn the_thread_a_reply_landed_in_is_found_by_the_comment_it_answered() {
    let GitHubResponse::ReviewThread(thread) =
        read(Shape::ReviewThread { comment_id: 111 }, &ok(COMMENTS)).unwrap()
    else {
        panic!("a reply answers with the thread");
    };
    assert_eq!(thread.id, 111);
    assert_eq!(thread.comments.len(), 2);
}
