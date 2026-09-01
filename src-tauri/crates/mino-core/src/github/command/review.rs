//! Argv for the two review-comment calls, and the one place this module
//! reaches for `gh api`.
//!
//! Every other call in [`super`] is a `gh` subcommand with `--json`.
//! Line-anchored review comments are not: no `--json` field on `gh pr view`
//! carries them, because they are not a property of the pull request so much
//! as of its diff. So these two go through `gh api`, which is `gh`'s own
//! escape hatch to the REST endpoints it has not wrapped.
//!
//! That is a wider door than the rest of this module opens, and it is narrowed
//! in three ways:
//!
//! - **The path is fixed program text.** `{owner}` and `{repo}` are
//!   placeholders `gh` substitutes from the checkout it is standing in, so
//!   these calls cannot be pointed at another repository - not by a caller,
//!   and not by anything that came back from a previous call.
//! - **The only caller values are numbers.** A pull request number and a
//!   comment id, both formatted here from integers, so no caller text reaches
//!   a path at all.
//! - **The reply body travels on stdin**, as JSON built by `serde_json` - so a
//!   body containing quotes, newlines or an apostrophe is a body rather than a
//!   quoting problem, and nothing has to be escaped by hand.

use super::owned;

/// Reads the request body as JSON from standard input.
///
/// The counterpart of `--body-file -` for `pr create`: the one flag in this
/// module whose job is to keep a value *out* of argv.
const BODY_FROM_STDIN: &[&str] = &["--input", "-"];

/// `gh api repos/{owner}/{repo}/pulls/<number>/comments --paginate`.
///
/// `--paginate` because a long-running pull request can carry more comments
/// than one page, and a review pane that silently showed the first thirty
/// would be a review pane that hides objections.
pub fn review_comments_argv(number: u32) -> Vec<String> {
    let mut argv = owned(&["api", "--paginate"]);
    argv.push(format!("repos/{{owner}}/{{repo}}/pulls/{number}/comments"));
    argv
}

/// `gh api --method POST repos/{owner}/{repo}/pulls/comments/<id>/replies
/// --input -`.
///
/// GitHub's endpoint for replying to a review comment takes the comment being
/// replied to rather than the thread, which is why
/// [`crate::types::GitHubReviewThread::id`] is the id of the comment that
/// started the thread.
pub fn reply_argv(comment_id: u64) -> Vec<String> {
    let mut argv = owned(&["api", "--method", "POST"]);
    argv.push(format!(
        "repos/{{owner}}/{{repo}}/pulls/comments/{comment_id}/replies"
    ));
    argv.extend(owned(BODY_FROM_STDIN));
    argv
}

/// One review thread, read back after replying to it.
///
/// `gh api` answers a reply with the new comment alone, and the caller wants
/// the thread. Rather than appending the answer to a list it already holds -
/// which would be the UI inventing a thread nobody read - the thread is asked
/// for again.
pub fn thread_argv(number: u32) -> Vec<String> {
    review_comments_argv(number)
}

/// The JSON body a reply sends on stdin.
///
/// Built by `serde_json` rather than by formatting a string, so a body
/// containing a quote or a newline is encoded rather than breaking the
/// document - the same reason the pull request body never goes near argv.
pub fn reply_body(body: &str) -> String {
    serde_json::json!({ "body": body }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_is_a_placeholder_gh_fills_in_and_never_a_caller_value() {
        // The narrowest part of the widest door in this module: these calls
        // cannot be pointed at another repository.
        let argv = review_comments_argv(42);
        let path = argv.last().unwrap();
        assert_eq!(path, "repos/{owner}/{repo}/pulls/42/comments");
        assert!(argv.contains(&"--paginate".to_string()));
    }

    #[test]
    fn a_comment_id_is_a_number_and_never_text() {
        let argv = reply_argv(918_273);
        assert!(argv
            .iter()
            .any(|arg| arg == "repos/{owner}/{repo}/pulls/comments/918273/replies"));
        assert!(argv.contains(&"POST".to_string()));
    }

    #[test]
    fn a_reply_body_is_json_on_stdin_and_not_an_argument() {
        let argv = reply_argv(1);
        assert_eq!(&argv[argv.len() - 2..], &["--input", "-"]);

        // Encoded rather than formatted, so a quote or a newline is content.
        let body = reply_body("It's \"fine\".\nReally.");
        assert_eq!(body, r#"{"body":"It's \"fine\".\nReally."}"#);
        assert!(!argv.iter().any(|arg| arg.contains("fine")));
    }
}
