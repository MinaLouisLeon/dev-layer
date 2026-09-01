//! **No secret-shaped content in an error, a result, or a log line.**
//!
//! The plan names this as phase 6's first risk, and it is the one that cannot
//! be fixed after the fact: a token in a screenshot is a token that has to be
//! rotated. So this suite walks the *outward* surface - every string a remote
//! call can produce - rather than testing `redact` again, which
//! `git::redact::tests` already does thoroughly.
//!
//! The question here is different and is the one that actually matters: **is
//! there a path from git's output to a caller that does not go through
//! `redact`?**

use mino_core::git::{redact, remote, GitOutput};
use mino_core::types::GitRemote;

/// A credential of each shape git might print one in.
const SECRETS: &[&str] = &["ghp_abc123XYZ", "hunter2", "glpat-deadbeef"];

fn output(stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        code: Some(1),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

/// The lines git actually prints with a credential in them, one per shape.
fn leaky_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for secret in SECRETS {
        lines.push(format!(
            "fatal: unable to access 'https://mina:{secret}@github.com/o/r.git/': failed"
        ));
        lines.push(format!("remote: https://{secret}@gitlab.example/o/r"));
        lines.push(format!("ssh://git:{secret}@host:22/o/r.git"));
        lines.push(format!(
            "error: failed to push some refs to 'https://x:{secret}@host/r'"
        ));
    }
    lines
}

#[test]
fn no_failure_sentence_carries_a_secret_whatever_git_said() {
    // Every branch of `remote::failure`, fed every leaky shape, on both
    // streams. If any path skipped `redact`, one of these would carry a token.
    for line in leaky_lines() {
        for out in [output(&line, ""), output("", &line)] {
            for what in ["fetch", "pull", "push", "remote"] {
                let sentence = remote::failure(&out, what).to_string();
                for secret in SECRETS {
                    assert!(
                        !sentence.contains(secret),
                        "`{what}` leaked {secret} from {line:?}: {sentence}"
                    );
                }
            }
        }
    }
}

#[test]
fn no_result_summary_carries_a_secret() {
    // `said` is what fills `GitFetchResult::summary` and its two siblings -
    // the strings a *successful* call hands back. A leak here would be worse
    // than one in an error, because nothing about a success invites suspicion.
    for line in leaky_lines() {
        for out in [output(&line, ""), output("", &line)] {
            let Some(summary) = remote::said(&out) else {
                continue;
            };
            for secret in SECRETS {
                assert!(!summary.contains(secret), "{summary}");
            }
        }
    }
}

#[test]
fn no_listed_remote_carries_a_secret() {
    // The likeliest leak of all, and the quietest: a remote configured with a
    // token in its URL is ordinary, and this list is rendered on screen.
    let listing: String = SECRETS
        .iter()
        .enumerate()
        .map(|(index, secret)| {
            format!(
                "r{index}\thttps://mina:{secret}@github.com/o/r.git (fetch)\n\
                 r{index}\thttps://mina:{secret}@github.com/o/r.git (push)\n"
            )
        })
        .collect();

    let remotes: Vec<GitRemote> = remote::parse(&output(&listing, ""));
    assert_eq!(remotes.len(), SECRETS.len());
    for entry in &remotes {
        for secret in SECRETS {
            assert!(!entry.fetch_url.contains(secret), "{:?}", entry.fetch_url);
            assert!(!entry.push_url.contains(secret), "{:?}", entry.push_url);
        }
        // Still readable: the host survives, so the reader can tell which
        // remote they are looking at.
        assert!(entry.fetch_url.contains("github.com"));
    }
}

#[test]
fn redaction_keeps_what_a_reader_needs() {
    // A guard against over-correcting. A mask that ate the host as well would
    // be safe and useless, and somebody would eventually turn it off.
    let out = redact::redact("fatal: unable to access 'https://u:p@github.com/o/r.git/'");
    assert!(out.contains("github.com/o/r.git"), "{out}");
    assert!(out.contains("unable to access"), "{out}");
}

#[test]
fn a_conventional_ssh_remote_is_still_legible() {
    // `git@github.com` is not a credential, and masking it would make every
    // ordinary SSH remote unreadable for no gain.
    let listing =
        "origin\tgit@github.com:o/r.git (fetch)\norigin\tssh://git@github.com/o/r.git (push)\n";
    let remotes = remote::parse(&output(listing, ""));
    assert_eq!(remotes[0].fetch_url, "git@github.com:o/r.git");
    assert_eq!(remotes[0].push_url, "ssh://git@github.com/o/r.git");
}
