//! The one function phase 6 leans on hardest.
//!
//! Every assertion here is really the same assertion: a secret that went in
//! does not come out. The cases are the shapes git actually prints.

use super::*;

#[test]
fn a_token_in_a_remote_url_does_not_survive() {
    let line = "fatal: unable to access 'https://mina:ghp_abc123XYZ@github.com/o/r.git/'";
    let out = redact(line);
    assert!(!out.contains("ghp_abc123XYZ"), "{out}");
    assert!(!out.contains("mina:"), "{out}");
    assert_eq!(
        out,
        "fatal: unable to access 'https://***@github.com/o/r.git/'"
    );
}

#[test]
fn a_bare_token_userinfo_over_https_is_a_credential() {
    // `https://<token>@host` is exactly how a personal access token is written
    // into a remote, so a userinfo with no colon still masks over HTTP(S).
    let out = redact("remote: https://ghp_abc123XYZ@github.com/o/r");
    assert!(!out.contains("ghp_abc123XYZ"), "{out}");
    assert_eq!(out, "remote: https://***@github.com/o/r");
}

#[test]
fn the_conventional_ssh_login_is_left_alone() {
    // `git@github.com` is not a secret, and masking it would make every
    // ordinary SSH remote unreadable for no gain.
    for line in [
        "ssh://git@github.com/o/r.git",
        "git@github.com:o/r.git",
        "remote: ssh://git@host:22/o/r",
    ] {
        assert_eq!(redact(line), line, "{line}");
    }
}

#[test]
fn an_ssh_url_carrying_a_password_still_masks() {
    let out = redact("ssh://git:hunter2@host/o/r");
    assert!(!out.contains("hunter2"), "{out}");
    assert_eq!(out, "ssh://***@host/o/r");
}

#[test]
fn several_urls_in_one_line_are_all_masked() {
    let out = redact("from https://a:1@x.test/r to https://b:2@y.test/r");
    assert!(!out.contains(":1@"), "{out}");
    assert!(!out.contains(":2@"), "{out}");
    assert_eq!(out, "from https://***@x.test/r to https://***@y.test/r");
}

#[test]
fn an_at_sign_in_a_path_is_not_userinfo() {
    // The authority ends at the first `/`. A `@` past that belongs to a path.
    let line = "https://github.com/o/r/blob/main/@types/index.d.ts";
    assert_eq!(redact(line), line);
}

#[test]
fn ordinary_git_output_passes_through_unchanged() {
    for line in [
        "Everything up-to-date",
        "   3f2a1c9..8ce8c26  main -> main",
        "! [rejected]        main -> main (non-fast-forward)",
        "",
    ] {
        assert_eq!(redact(line), line, "{line}");
    }
}

#[test]
fn it_is_idempotent() {
    // Calling it twice must not mask the mask, because a value can pass
    // through more than one layer on its way to a message.
    let once = redact("https://u:p@host/r");
    assert_eq!(redact(&once), once);
}

#[test]
fn a_very_long_output_is_cut_rather_than_carried_whole() {
    // A push prints a hundred progress lines. Nothing downstream renders more
    // than a sentence, and an unbounded string crosses an IPC boundary.
    let long = "x".repeat(MAX_SUMMARY_BYTES * 2);
    let out = redact(&long);
    assert!(out.len() <= MAX_SUMMARY_BYTES + 4, "{}", out.len());
    assert!(out.ends_with('…'));
}

#[test]
fn multibyte_text_is_cut_on_a_character_boundary() {
    // A naive slice here would panic, and panicking while building an error
    // message is the worst possible place to do it.
    let long = "é".repeat(MAX_SUMMARY_BYTES);
    let out = redact(&long);
    assert!(out.ends_with('…'));
}

#[test]
fn a_summary_of_nothing_is_nothing() {
    assert_eq!(summary("   \n  "), None);
    assert_eq!(summary(""), None);
    assert_eq!(summary("  done  "), Some("done".to_string()));
}

#[test]
fn a_summary_is_redacted_like_everything_else() {
    let out = summary("pushed to https://u:secret@host/r").unwrap();
    assert!(!out.contains("secret"), "{out}");
}
