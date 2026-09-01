//! How an argv array becomes one remote command line.
//!
//! Every argument is quoted, so a path containing a space stays one argument
//! and a path containing a semicolon stays a path. A single quote is refused
//! rather than escaped, which is the documented limit of this transport.
//!
//! The function under test moved to `ssh/exec.rs` when `gh` arrived and needed
//! the same command line built for a different program. These tests stayed
//! here, and gained the `gh` cases below: one line builder means these
//! assertions hold for both binaries, which is exactly why it was moved.

use crate::git::command;
use crate::github::command as gh;
use crate::ssh::exec::command_line;

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// The command line as it is built for git, which is every caller before
/// phase 5.
fn git_line(cwd: &str, argv: &[String]) -> crate::Result<String> {
    command_line(command::GIT_PROGRAM, cwd, argv, &[])
}

#[test]
fn every_argument_is_quoted_not_just_the_directory() {
    let line = git_line("/srv/app", &owned(&command::status_argv())).unwrap();
    assert!(line.starts_with("cd '/srv/app' && git "));
    assert!(line.contains("'--porcelain=v2'"));
}

#[test]
fn a_quote_in_the_remote_path_is_refused_not_escaped() {
    assert!(git_line("/srv/it's", &owned(&command::status_argv())).is_err());
}

#[test]
fn a_path_with_a_space_stays_one_argument() {
    let line = git_line(
        "/srv/app",
        &command::stage_argv(&["release notes.md".to_string()]),
    )
    .unwrap();
    assert!(line.ends_with("'--' 'release notes.md'"), "{line}");
}

#[test]
fn a_shell_metacharacter_in_a_path_is_inert() {
    // Quoted, so the remote shell reads it as a filename. The path guard
    // has already refused anything outside the root; this is the second
    // line of defence, not the first.
    let line = git_line(
        "/srv/app",
        &command::stage_argv(&["; rm -rf /".to_string()]),
    )
    .unwrap();
    assert!(line.ends_with("'--' '; rm -rf /'"), "{line}");
}

#[test]
fn the_same_rules_hold_for_gh() {
    let line = command_line(
        crate::github::GH_PROGRAM,
        "/srv/app",
        &gh::create_pr_argv("a title with spaces", "main", true),
        &[],
    )
    .unwrap();
    assert!(line.starts_with("cd '/srv/app' && gh "), "{line}");
    assert!(line.contains("'a title with spaces'"), "{line}");
    assert!(line.contains("'--draft'"), "{line}");
}

#[test]
fn a_pull_request_title_with_an_apostrophe_is_refused_on_this_transport() {
    // The documented limit, and the reason the *body* travels on stdin: a
    // description with an apostrophe in it must never be a refusal, and a
    // title is short enough that rewording one is a reasonable thing to ask.
    assert!(command_line(
        crate::github::GH_PROGRAM,
        "/srv/app",
        &gh::create_pr_argv("it's broken", "main", false),
        &[],
    )
    .is_err());
}

#[test]
fn a_browse_target_stays_one_argument_behind_the_separator() {
    let line = command_line(
        crate::github::GH_PROGRAM,
        "/srv/app",
        &gh::browse_argv("src/some file.rs:42", None),
        &[],
    )
    .unwrap();
    assert!(line.ends_with("'--' 'src/some file.rs:42'"), "{line}");
}

#[test]
fn an_environment_pair_is_set_in_front_of_the_program() {
    // How `GIT_TERMINAL_PROMPT=0` reaches a remote git: a POSIX shell reads
    // `NAME=value cmd` as "run cmd with NAME set". Without it a remote push
    // against an unconfigured account holds the channel open until the
    // timeout, because a prompt on an exec channel has nowhere to go.
    let line = command_line(
        command::GIT_PROGRAM,
        "/srv/app",
        &owned(&["fetch"]),
        crate::git::command::NO_PROMPT,
    )
    .unwrap();
    assert_eq!(line, "cd '/srv/app' && GIT_TERMINAL_PROMPT='0' git 'fetch'");
}

#[test]
fn an_environment_name_that_is_not_a_name_is_refused() {
    // A name cannot be quoted in that position - `'FOO'=1` is not an
    // assignment - so it is checked instead.
    assert!(command_line(
        command::GIT_PROGRAM,
        "/srv/app",
        &owned(&["fetch"]),
        &[("not a name", "0")],
    )
    .is_err());
}
