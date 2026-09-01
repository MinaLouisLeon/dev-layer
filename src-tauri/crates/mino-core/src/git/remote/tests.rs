//! What git said about a remote, and what this crate makes of it.

use super::*;
use crate::types::{GitConflictKind, GitPullOutcome, GitPushOutcome};
use crate::TransportError;

fn out(stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

fn conflict() -> GitConflict {
    GitConflict {
        path: "/root/a.txt".to_string(),
        relative_path: "a.txt".to_string(),
        kind: GitConflictKind::BothModified,
    }
}

#[test]
fn a_remote_name_that_could_be_read_as_an_option_is_refused() {
    for bad in ["-x", "", "   ", "a remote", "it's"] {
        assert!(name(bad).is_err(), "{bad:?} should be refused");
    }
    assert_eq!(name("  origin  ").unwrap(), "origin");
    assert_eq!(optional_name(None).unwrap(), None);
}

#[test]
fn remotes_keep_both_urls_because_git_lets_them_differ() {
    let listing = "origin\thttps://github.com/o/r.git (fetch)\n\
                   origin\tssh://git@github.com/o/r.git (push)\n\
                   upstream\thttps://github.com/up/r.git (fetch)\n\
                   upstream\thttps://github.com/up/r.git (push)\n";
    let remotes = parse(&out(listing, ""));
    assert_eq!(remotes.len(), 2);
    assert_eq!(remotes[0].name, "origin");
    assert_eq!(remotes[0].fetch_url, "https://github.com/o/r.git");
    assert_eq!(remotes[0].push_url, "ssh://git@github.com/o/r.git");
}

#[test]
fn a_token_in_a_configured_remote_never_reaches_the_type() {
    // An ordinary thing to find in somebody's .git/config, and the reason
    // this parser redacts at the boundary rather than trusting callers to.
    let listing = "origin\thttps://mina:ghp_secret@github.com/o/r.git (fetch)\n\
                   origin\thttps://mina:ghp_secret@github.com/o/r.git (push)\n";
    let remotes = parse(&out(listing, ""));
    assert_eq!(remotes[0].fetch_url, "https://***@github.com/o/r.git");
    assert!(!remotes[0].push_url.contains("ghp_secret"));
}

#[test]
fn a_url_containing_a_space_survives_whole() {
    // A local path is a legitimate remote, and plenty of people have one under
    // a home directory with a space in its name. Splitting on whitespace cuts
    // those in half, and the half that survives is not a URL.
    let path = "C:/Users/Mina Louis/repos/origin.git";
    let listing = format!(
        "origin	{path} (fetch)
origin	{path} (push)
"
    );
    let remotes = parse(&out(&listing, ""));
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].fetch_url, path);
    assert_eq!(remotes[0].push_url, path);
}

#[test]
fn a_malformed_row_is_skipped_rather_than_breaking_the_list() {
    let remotes = parse(&out("origin\n\norigin\thttps://x.test/r (fetch)\n", ""));
    assert_eq!(remotes.len(), 1);
}

#[test]
fn a_pull_reports_which_of_the_four_things_happened() {
    assert_eq!(
        pull_outcome(&out("Already up to date.\n", ""), &[]),
        GitPullOutcome::AlreadyUpToDate
    );
    assert_eq!(
        pull_outcome(&out("", "Updating 3f2a1c9..8ce8c26\nFast-forward\n"), &[]),
        GitPullOutcome::FastForwarded
    );
    assert_eq!(
        pull_outcome(
            &out("", "Successfully rebased and updated refs/heads/main."),
            &[]
        ),
        GitPullOutcome::Rebased
    );
    assert_eq!(
        pull_outcome(&out("", "Merge made by the 'ort' strategy."), &[]),
        GitPullOutcome::Merged
    );
}

#[test]
fn a_conflicted_tree_decides_the_outcome_whatever_git_printed() {
    // A pull that hit a conflict exits non-zero and says several things; the
    // tree is the fact, and it is the one the caller already had to read.
    let outcome = pull_outcome(&out("", "Automatic merge failed"), &[conflict()]);
    assert_eq!(outcome, GitPullOutcome::Conflicted);
}

#[test]
fn an_unrecognised_summary_is_a_merge_and_not_a_guess() {
    // Something came down and the tree changed. Saying "merged" is honest;
    // claiming a fast-forward would not be.
    assert_eq!(
        pull_outcome(&out("", "some future wording"), &[]),
        GitPullOutcome::Merged
    );
}

#[test]
fn a_push_that_changed_nothing_says_so() {
    // `=` is the porcelain flag for "up to date". The human summary on stderr
    // differs between versions and locales; this does not.
    let porcelain =
        "To github.com:o/r.git\n=\trefs/heads/main:refs/heads/main\t[up to date]\nDone\n";
    assert_eq!(
        push_outcome(&out(porcelain, "")),
        GitPushOutcome::AlreadyUpToDate
    );
}

#[test]
fn a_push_that_moved_a_ref_says_so() {
    let porcelain =
        "To github.com:o/r.git\n \trefs/heads/main:refs/heads/main\t3f2a1c9..8ce8c26\nDone\n";
    assert_eq!(push_outcome(&out(porcelain, "")), GitPushOutcome::Pushed);
}

#[test]
fn a_missing_credential_names_what_to_configure() {
    // The failure D3 makes possible, and the one the reader can act on.
    let failure = failure(
        &out(
            "",
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
        ),
        "push",
    )
    .to_string();
    assert!(failure.contains("credential helper"), "{failure}");
    assert!(failure.contains("SSH agent"), "{failure}");
    // And it does not just repeat git's line, which is where a URL would be.
    assert!(!failure.contains("github.com"), "{failure}");
}

#[test]
fn a_rejected_push_says_nothing_was_pushed_and_what_to_do() {
    let failure = failure(
        &out(
            "!\trefs/heads/main:refs/heads/main\t[rejected] (non-fast-forward)\n",
            "error: failed to push some refs",
        ),
        "push",
    )
    .to_string();
    assert!(failure.contains("Fetch and pull first"), "{failure}");
    assert!(failure.contains("Nothing was pushed"), "{failure}");
}

#[test]
fn a_refused_lease_explains_what_it_protected() {
    let failure = failure(&out("", "! [rejected] main -> main (stale info)"), "push").to_string();
    assert!(failure.contains("work you have not seen"), "{failure}");
}

#[test]
fn an_unrecognised_failure_carries_gits_words_redacted() {
    let failure = failure(
        &out(
            "",
            "fatal: unable to access 'https://u:tok@host/r': timed out",
        ),
        "fetch",
    )
    .to_string();
    assert!(failure.contains("timed out"), "{failure}");
    assert!(!failure.contains("tok"), "{failure}");
}

#[test]
fn a_dirty_tree_is_refused_and_points_at_the_stash() {
    // Never stashed on the reader's behalf: a stash they did not make is a
    // stash they will not think to look for.
    let refusal = dirty().to_string();
    assert!(refusal.contains("Stash section"), "{refusal}");
    assert!(matches!(dirty(), TransportError::InvalidArgument { .. }));
}

#[test]
fn a_detached_head_has_nothing_to_push() {
    assert!(detached().to_string().contains("no branch checked out"));
}
