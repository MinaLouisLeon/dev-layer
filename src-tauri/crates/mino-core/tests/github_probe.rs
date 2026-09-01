//! The probe's four answers, and the fact that three of them are not errors.
//!
//! Every state is reachable without a network, a `gh` install or a login,
//! because the deciding is done by pure functions over what `gh` printed. The
//! transports contribute the process and nothing else.

use mino_core::git::GitOutput;
use mino_core::github::probe;
use mino_core::types::GitHubAvailability;

fn output(code: i32, stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

#[test]
fn a_machine_without_gh_is_told_where_to_get_it_and_nothing_else() {
    let probe = probe::absent();
    assert_eq!(probe.availability, GitHubAvailability::Absent);
    assert!(!probe.is_ready());
    assert!(probe.repository.is_none());
    let sentence = probe.detail.unwrap();
    assert!(sentence.contains("gh"), "{sentence}");
    assert!(sentence.contains("cli.github.com"), "{sentence}");
}

#[test]
fn an_account_that_is_not_signed_in_is_told_the_command_to_run() {
    // The app cannot log anybody in - the handshake is interactive and the
    // credential belongs to gh's keychain entry - so naming the command is
    // the only correct thing to say.
    let probe = probe::unauthenticated(&output(1, "", "You are not logged into any GitHub hosts."));
    assert_eq!(probe.availability, GitHubAvailability::Unauthenticated);
    let sentence = probe.detail.unwrap();
    assert!(sentence.contains("gh auth login"), "{sentence}");
    // gh's own words are carried through as text.
    assert!(
        sentence.contains("not logged into any GitHub hosts"),
        "{sentence}"
    );
}

#[test]
fn a_non_github_remote_is_unsupported_rather_than_a_failure() {
    let probe = probe::unsupported(&output(
        1,
        "",
        "none of the git remotes configured for this repository point to a known GitHub host",
    ));
    assert_eq!(probe.availability, GitHubAvailability::Unsupported);
    assert!(!probe.is_ready());
    let sentence = probe.detail.unwrap();
    assert!(
        sentence.contains("Only remotes pointing at GitHub"),
        "{sentence}"
    );
}

#[test]
fn a_folder_that_is_not_a_repository_reads_the_same_way() {
    // Three causes, one state: no repository, no remote, or a remote pointing
    // somewhere else. The reader's next move is the same for all three, and
    // gh's own sentence says which it was.
    let probe = probe::unsupported(&output(1, "", "not a git repository"));
    assert_eq!(probe.availability, GitHubAvailability::Unsupported);
}

#[test]
fn a_ready_probe_carries_the_repository_the_remote_points_at() {
    let json = r#"{
        "nameWithOwner": "MinaLouisLeon/mino-terminal",
        "url": "https://github.com/MinaLouisLeon/mino-terminal",
        "defaultBranchRef": { "name": "main" }
    }"#;
    let probe = probe::repository(&output(0, json, "")).unwrap();
    assert!(probe.is_ready());
    let repository = probe.repository.unwrap();
    assert_eq!(repository.name_with_owner, "MinaLouisLeon/mino-terminal");
    assert_eq!(
        repository.url,
        "https://github.com/MinaLouisLeon/mino-terminal"
    );
    assert_eq!(repository.default_branch.as_deref(), Some("main"));
    assert!(probe.detail.is_none());
}

#[test]
fn a_repository_with_no_commits_yet_has_no_default_branch_and_is_still_ready() {
    let json = r#"{"nameWithOwner":"o/r","url":"https://github.com/o/r","defaultBranchRef":null}"#;
    let probe = probe::repository(&output(0, json, "")).unwrap();
    assert!(probe.is_ready());
    assert!(probe.repository.unwrap().default_branch.is_none());
}

#[test]
fn malformed_json_from_gh_is_a_typed_protocol_error_and_never_a_panic() {
    for stdout in ["", "{", "null", r#"{"url":"x"}"#] {
        let failure = probe::repository(&output(0, stdout, "")).unwrap_err();
        assert!(
            matches!(failure, mino_core::TransportError::Protocol { .. }),
            "{stdout:?} produced {failure:?}"
        );
    }
}

#[test]
fn a_silent_failure_still_produces_a_sentence() {
    // gh is terse on some failures. An empty notice helps nobody.
    let probe = probe::unauthenticated(&output(1, "", ""));
    assert!(probe.detail.unwrap().contains("gh auth login"));
}
