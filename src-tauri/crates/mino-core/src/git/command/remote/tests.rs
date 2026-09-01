//! The argv the three network calls are made from, and the environment
//! they run in.
//!
//! The force-push assertions are the ones worth reading twice. `--force`
//! overwrites the remote branch whatever is on it; `--force-with-lease`
//! refuses unless the remote is where this repository last saw it. The
//! difference is somebody else's commit, and the bare form must not appear
//! anywhere in this crate.

use super::*;

#[test]
fn every_remote_call_refuses_to_be_asked_a_question() {
    // The one line standing between "delegate to the system" and a pane
    // that hangs forever on a machine with no helper configured.
    assert_eq!(NO_PROMPT, &[("GIT_TERMINAL_PROMPT", "0")]);
}

#[test]
fn a_force_push_is_leased_and_never_bare() {
    let forced = push_argv(
        &PushRequest {
            force: true,
            ..PushRequest::default()
        },
        "origin",
        "main",
    );
    assert!(forced.contains(&"--force-with-lease".to_string()));
    // The bare form would overwrite a colleague's commit pushed thirty
    // seconds ago. It must not appear anywhere in this crate.
    assert!(!forced.contains(&"--force".to_string()), "{forced:?}");
}

#[test]
fn a_force_push_is_never_a_default() {
    let plain = push_argv(&PushRequest::default(), "origin", "main");
    assert!(!plain.iter().any(|arg| arg.starts_with("--force")));
    assert!(!plain.contains(&"--set-upstream".to_string()));
}

#[test]
fn the_remote_and_branch_sit_behind_the_separator() {
    // A branch really called `-n` is a branch. Without `--` git would read
    // it as a flag.
    let argv = push_argv(&PushRequest::default(), "origin", "-n");
    let separator = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
    assert_eq!(argv[separator + 1], "origin");
    assert_eq!(argv[separator + 2], "-n");
}

#[test]
fn a_pull_merges_unless_rebase_was_asked_for() {
    assert!(!pull_argv(&PullRequest::default()).contains(&"--rebase".to_string()));
    let rebased = pull_argv(&PullRequest {
        rebase: true,
        ..PullRequest::default()
    });
    assert!(rebased.contains(&"--rebase".to_string()));
}

#[test]
fn a_named_remote_is_its_own_argument() {
    assert_eq!(fetch_argv(Some("upstream")).last().unwrap(), "upstream");
    assert!(!fetch_argv(None).iter().any(|a| a == "upstream"));
    // Pruning is not optional: without it the branch picker fills with
    // branches that were merged and deleted months ago.
    assert!(fetch_argv(None).contains(&"--prune".to_string()));
}
