//! What happens when `gh` answers with something this build cannot read.
//!
//! `gh` can change the shape of its JSON between versions, so nothing in the
//! parser reaches into a document hopefully. The rule these assertions hold is
//! that a **missing structure** is a typed protocol error naming what was
//! being read - never a panic, and never a silently empty list, because an
//! empty list is a fine answer that means something else entirely.
//!
//! The counterpart rule is in `github_parse.rs`: a missing *value* - a run
//! with no start time, an author who deleted their account - is ordinary and
//! is not an error at all.

use mino_core::git::GitOutput;
use mino_core::github::call::{read, Shape};
use mino_core::TransportError;

fn ok(stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

#[test]
fn malformed_json_is_a_typed_protocol_error_and_never_a_panic() {
    let cases = [
        (Shape::Runs, "not json at all"),
        (Shape::Runs, "{}"),
        (Shape::Runs, r#"[{"databaseId": "not a number"}]"#),
        (Shape::Jobs, "[]"),
        (Shape::PullRequests, r#"[{"number": 1}]"#),
        (
            Shape::PullRequests,
            r#"[{"number":1,"title":"t","author":{"login":"a"},"url":"u","state":"WHAT","isDraft":false,"headRefName":"h","baseRefName":"b"}]"#,
        ),
        (
            Shape::Issues,
            r#"[{"number":1,"title":"t","author":{"login":"a"},"url":"u","state":"WHAT"}]"#,
        ),
        (Shape::Created, "nothing was printed"),
    ];
    for (shape, stdout) in cases {
        let failure = read(shape, &ok(stdout)).unwrap_err();
        assert!(
            matches!(failure, TransportError::Protocol { .. }),
            "{shape:?} {stdout:?} produced {failure:?}"
        );
    }
}

#[test]
fn a_protocol_error_says_what_it_was_reading_and_what_to_do() {
    let failure = read(Shape::Issues, &ok("{")).unwrap_err().to_string();
    assert!(failure.contains("the issues"), "{failure}");
    assert!(failure.contains("updating it"), "{failure}");
}
