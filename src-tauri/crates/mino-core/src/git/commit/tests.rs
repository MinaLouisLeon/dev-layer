//! Reading a commit, and reading git's refusal to make one.
//!
//! The refusal cases matter more than the parse: git reports "nothing to
//! commit" as a *failure* with the explanation on stdout, and passing that
//! through raw would show the user a status listing where a sentence belongs.

use super::*;

fn output(code: i32, stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

#[test]
fn an_empty_message_is_refused_before_git_is_spawned() {
    assert!(validate(&CommitRequest::new("   \n  ")).is_err());
    assert!(validate(&CommitRequest::new("real message")).is_ok());
}

#[test]
fn nothing_staged_reads_as_a_sentence_not_as_gits_status_output() {
    // Git puts this on stdout, and exits 1.
    let refused = output(
        1,
        "On branch main\nnothing to commit, working tree clean\n",
        "",
    );
    let error = failure(&refused);
    assert!(
        matches!(error, TransportError::InvalidArgument { ref message }
            if message.contains("nothing staged")),
        "{error:?}"
    );
}

#[test]
fn a_missing_identity_says_what_to_set() {
    let refused = output(
        128,
        "",
        "*** Please tell me who you are.\n\nRun\n\n  git config ...",
    );
    assert!(
        matches!(failure(&refused), TransportError::Shell { ref message }
            if message.contains("user.email"))
    );
}

#[test]
fn a_real_failure_keeps_gits_own_words() {
    let refused = output(1, "", "error: could not lock config file");
    assert!(
        matches!(failure(&refused), TransportError::Shell { ref message }
            if message.contains("could not lock"))
    );
}

#[test]
fn a_commit_line_parses_into_its_five_fields() {
    // Joined rather than written as one literal: a `\0` immediately before
    // a digit reads as an octal escape, which is a trap for the next
    // person even though Rust does not have octal escapes.
    let line = [
        "3f2a1c9d8e7b",
        "9f2a1c9",
        "Fix Bob's bug",
        "Mina",
        "1788024729",
    ]
    .join("\0");
    let commit = parse(&output(0, &line, "")).unwrap();
    assert_eq!(commit.sha, "3f2a1c9d8e7b");
    assert_eq!(commit.short_sha, "9f2a1c9");
    assert_eq!(commit.summary, "Fix Bob's bug");
    assert_eq!(commit.author, "Mina");
    assert_eq!(commit.timestamp_ms, 1_788_024_729_000);
}

#[test]
fn a_truncated_commit_line_is_reported_not_guessed_at() {
    assert!(parse(&output(0, "3f2a1c9\0short\0", "")).is_err());
    assert!(parse(&output(0, &["", "", "", "", "notanumber"].join("\0"), "")).is_err());
}
