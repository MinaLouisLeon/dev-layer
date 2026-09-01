//! What the two review calls become.
//!
//! Split from the parsing beside it because it is the other half of the same
//! feature and a different question: that file asks what GitHub said, and this
//! one asks what was sent.
//!
//! Two things are narrowed here, and both matter because `gh api` is the
//! widest door this crate opens. The **path** is fixed program text with
//! `{owner}` and `{repo}` placeholders `gh` fills in from the checkout, so
//! these calls cannot be pointed at another repository. And the reply **body**
//! is JSON on stdin, so a reply containing a quote or a newline is a reply.

use mino_core::git::paths::PathStyle;
use mino_core::github::call::{plan, Shape};
use mino_core::types::GitHubQuery;

#[test]
fn reading_review_comments_names_a_fixed_path_and_a_number() {
    // The widest door this module opens is `gh api`, and this is what narrows
    // it: `{owner}` and `{repo}` are placeholders gh fills in from the
    // checkout, so the call cannot be pointed at another repository.
    let call = plan(
        &GitHubQuery::ReviewComments { number: 42 },
        "/srv/app",
        PathStyle::posix(),
    )
    .unwrap();
    assert_eq!(call.argv[0], "api");
    assert!(call
        .argv
        .contains(&"repos/{owner}/{repo}/pulls/42/comments".to_string()));
    assert!(call.input.is_none());
    assert!(call.follow_up.is_none());
}

#[test]
fn a_reply_sends_its_body_on_stdin_and_reads_the_thread_back() {
    let call = plan(
        &GitHubQuery::ReplyToReviewComment {
            number: 42,
            comment_id: 111,
            body: "It's fine by me.\nReally.".to_string(),
        },
        "/srv/app",
        PathStyle::posix(),
    )
    .unwrap();

    // The body is nowhere in the argument list - so an apostrophe, a quote or
    // a newline is content rather than a quoting problem.
    assert!(!call.argv.join(" ").contains("Really"), "{:?}", call.argv);
    assert_eq!(
        call.input.as_deref(),
        Some(r#"{"body":"It's fine by me.\nReally."}"#)
    );
    // And the thread is read back rather than assembled from the one comment
    // gh hands back.
    assert!(call.follow_up.is_some());
    assert_eq!(call.shape, Shape::ReviewThread { comment_id: 111 });
}
