//! What the one query that writes refuses, and what it reads back.

use super::*;

#[test]
fn an_ordinary_request_survives_and_comes_back_trimmed() {
    let (title, base) = validate("  Add the GitHub view  ", "A body.", "main").unwrap();
    assert_eq!(title, "Add the GitHub view");
    assert_eq!(base, "main");
}

#[test]
fn a_title_of_spaces_is_not_a_title() {
    // `gh` would happily create a pull request called "   ".
    assert!(validate("   ", "body", "main").is_err());
    assert!(validate("", "body", "main").is_err());
}

#[test]
fn a_newline_in_a_title_is_refused_and_names_the_description() {
    let refusal = validate("one\ntwo", "", "main").unwrap_err().to_string();
    assert!(refusal.contains("description"), "{refusal}");
}

#[test]
fn the_ceilings_are_enforced_on_both_halves() {
    assert!(validate(&"t".repeat(MAX_PR_TITLE_BYTES + 1), "", "main").is_err());
    assert!(validate("t", &"b".repeat(MAX_PR_BODY_BYTES + 1), "main").is_err());
}

#[test]
fn the_base_gets_the_branch_guard_and_not_a_length_check() {
    // `-x` is a legal ref name and an illegal thing to hand a command, which
    // is exactly what `refname::precheck` exists to catch.
    assert!(validate("t", "", "-x").is_err());
    assert!(validate("t", "", "a branch").is_err());
    assert!(validate("t", "", "release/2.0").is_ok());
}

#[test]
fn a_body_may_contain_anything_at_all() {
    // It travels on stdin, so quotes, newlines and apostrophes are content.
    let body = "It's a fix.\n\n```rust\nlet x = \"y\";\n```\n";
    assert!(validate("Fix it", body, "main").is_ok());
}

#[test]
fn the_url_is_read_from_the_last_address_gh_printed() {
    let stdout = "\nCreating pull request for feat into main\n\nhttps://github.com/o/r/pull/42\n";
    let (url, number) = parse(stdout).unwrap();
    assert_eq!(url, "https://github.com/o/r/pull/42");
    assert_eq!(number, Some(42));
}

#[test]
fn an_address_with_no_number_still_answers_with_the_address() {
    let (url, number) = parse("https://github.com/o/r/pull/new/feat").unwrap();
    assert_eq!(url, "https://github.com/o/r/pull/new/feat");
    assert_eq!(number, None);
}

#[test]
fn output_with_no_address_is_a_protocol_error_and_never_a_panic() {
    assert!(parse("").is_err());
    assert!(parse("something went sideways").is_err());
}
