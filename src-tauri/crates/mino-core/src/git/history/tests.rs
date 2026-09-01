//! Recorded `git log -z` and `git show --name-status -z` output.

use super::*;

const SHA: &str = "be99151be9f075a939ff822687938bd44bf7fe8c";

fn output(code: i32, stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

/// One commit record, in the order `COMMIT_FORMAT` asks for.
fn record(sha: &str, subject: &str) -> String {
    [sha, &sha[..7], subject, "A Author", "1788027833"].join("\x1f")
}

#[test]
fn a_page_of_history_parses_every_field() {
    let stdout = format!(
        "{}\0{}\0",
        record(SHA, "second subject"),
        record(SHA, "first")
    );
    let log = log_from(&output(0, &stdout, ""), &LogRequest::new()).unwrap();

    assert_eq!(log.commits.len(), 2);
    assert!(!log.truncated);
    let first = &log.commits[0];
    assert_eq!(first.sha, SHA);
    assert_eq!(first.short_sha, &SHA[..7]);
    assert_eq!(first.summary, "second subject");
    assert_eq!(first.author, "A Author");
    assert_eq!(first.timestamp_ms, 1_788_027_833_000);
}

#[test]
fn a_subject_containing_a_newline_survives() {
    // NUL separates records precisely because a message can contain anything
    // else - git forbids NUL inside a commit object outright.
    let stdout = format!("{}\0", record(SHA, "subject\nwith a newline"));
    let log = log_from(&output(0, &stdout, ""), &LogRequest::new()).unwrap();
    assert_eq!(log.commits[0].summary, "subject\nwith a newline");
}

#[test]
fn the_extra_row_answers_truncated_and_is_not_returned() {
    // The caller asked git for limit + 1. Three rows for a limit of two means
    // there is another page, and exactly two are handed back.
    let stdout: String = (0..3)
        .map(|i| format!("{}\0", record(SHA, &format!("s{i}"))))
        .collect();
    let log = log_from(&output(0, &stdout, ""), &LogRequest::new().limit(2)).unwrap();
    assert_eq!(log.commits.len(), 2);
    assert!(log.truncated);

    // And exactly the limit means there is not.
    let stdout: String = (0..2)
        .map(|i| format!("{}\0", record(SHA, &format!("s{i}"))))
        .collect();
    let log = log_from(&output(0, &stdout, ""), &LogRequest::new().limit(2)).unwrap();
    assert!(!log.truncated);
}

#[test]
fn an_unborn_branch_has_no_history_rather_than_an_error() {
    // `git log` fails on a repository with no commits. "There is nothing yet"
    // is an answer, and the history pane renders it quietly.
    let refused = output(
        128,
        "",
        "fatal: your current branch 'main' does not have any commits yet",
    );
    let log = log_from(&refused, &LogRequest::new()).unwrap();
    assert!(log.commits.is_empty());
    assert!(!log.truncated);
}

#[test]
fn a_real_log_failure_is_still_a_failure() {
    let refused = output(128, "", "fatal: not a git repository");
    assert!(log_from(&refused, &LogRequest::new()).is_err());
}

#[test]
fn a_commit_detail_lists_the_files_it_touched() {
    // Recorded shape: the commit record, NUL, a newline, then NUL-separated
    // status/path entries.
    let stdout = format!(
        "{}\0\nA\0added.txt\0M\0release notes.md\0D\0gone.rs\0",
        record(SHA, "second commit subject")
    );
    let detail = detail_from(&output(0, &stdout, "")).unwrap();

    assert_eq!(detail.commit.summary, "second commit subject");
    let files: Vec<_> = detail
        .files
        .iter()
        .map(|f| (f.relative_path.as_str(), f.state, f.old_path.as_deref()))
        .collect();
    assert_eq!(
        files,
        vec![
            ("added.txt", GitFileState::Added, None),
            ("release notes.md", GitFileState::Modified, None),
            ("gone.rs", GitFileState::Deleted, None),
        ]
    );
}

#[test]
fn a_rename_in_a_commit_consumes_two_paths() {
    // `R100` is followed by where it came from and where it went. Reading it
    // as one path would leave the loop one field out of step for every
    // following file.
    let stdout = format!(
        "{}\0\nR100\0before.txt\0after.txt\0M\0next.rs\0",
        record(SHA, "renamed")
    );
    let detail = detail_from(&output(0, &stdout, "")).unwrap();
    assert_eq!(detail.files.len(), 2);
    assert_eq!(detail.files[0].relative_path, "after.txt");
    assert_eq!(detail.files[0].old_path.as_deref(), Some("before.txt"));
    assert_eq!(detail.files[0].state, GitFileState::Renamed);
    // The file after the pair is still read correctly.
    assert_eq!(detail.files[1].relative_path, "next.rs");
}

#[test]
fn a_commit_that_touched_nothing_is_not_an_error() {
    let stdout = format!("{}\0", record(SHA, "empty commit"));
    let detail = detail_from(&output(0, &stdout, "")).unwrap();
    assert!(detail.files.is_empty());
}

#[test]
fn a_truncated_commit_record_is_reported_not_guessed_at() {
    assert!(detail_from(&output(0, "abc\0", "")).is_err());
    assert!(log_from(&output(0, "abc\0", ""), &LogRequest::new())
        .unwrap()
        .commits
        .is_empty());
}
