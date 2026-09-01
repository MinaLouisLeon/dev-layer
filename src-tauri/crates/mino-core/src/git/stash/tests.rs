//! Against records shaped exactly as `STASH_FORMAT` produces them. The
//! round-trip against real git is in `tests/git_stash.rs`.

use super::*;

fn output(stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

/// `%gd<US>%gs<US>%at`, NUL-terminated the way `-z` writes it.
fn record(selector: &str, subject: &str, seconds: &str) -> String {
    format!("{selector}\u{1f}{subject}\u{1f}{seconds}\0")
}

#[test]
fn a_stash_with_no_message_reports_gits_own_subject() {
    let stdout = record("stash@{0}", "WIP on main: 3f2a1c9 first", "1788024729");
    let entry = parse(&output(&stdout)).unwrap().remove(0);

    assert_eq!(entry.index, 0);
    assert_eq!(entry.branch.as_deref(), Some("main"));
    assert_eq!(entry.message, "3f2a1c9 first");
    assert_eq!(entry.timestamp_ms, 1_788_024_729_000);
}

#[test]
fn a_stash_with_a_message_reports_the_message_and_not_the_branch_again() {
    let stdout = record("stash@{1}", "On dev: half a refactor", "1788024000");
    let entry = parse(&output(&stdout)).unwrap().remove(0);

    assert_eq!(entry.index, 1);
    assert_eq!(entry.branch.as_deref(), Some("dev"));
    assert_eq!(entry.message, "half a refactor");
}

#[test]
fn a_message_containing_a_colon_survives_intact() {
    // A branch name cannot contain `:`, so the first `: ` is unambiguously
    // the boundary and everything after it is the message.
    let stdout = record("stash@{0}", "On main: fix: the parser", "1788024729");
    let entry = parse(&output(&stdout)).unwrap().remove(0);
    assert_eq!(entry.branch.as_deref(), Some("main"));
    assert_eq!(entry.message, "fix: the parser");
}

#[test]
fn the_index_is_read_from_the_selector_and_never_from_the_row_position() {
    // The two agree today. They are still different facts, and the selector
    // is the one a later `drop` is given.
    let stdout = format!(
        "{}{}",
        record("stash@{4}", "On main: newest", "1788024729"),
        record("stash@{7}", "On main: older", "1788024000"),
    );
    let entries = parse(&output(&stdout)).unwrap();
    assert_eq!(entries[0].index, 4);
    assert_eq!(entries[1].index, 7);
}

#[test]
fn an_unrecognised_subject_becomes_the_message_rather_than_a_blank_row() {
    let stdout = record("stash@{0}", "something else entirely", "1788024729");
    let entry = parse(&output(&stdout)).unwrap().remove(0);
    assert_eq!(entry.branch, None);
    assert_eq!(entry.message, "something else entirely");
}

#[test]
fn a_row_whose_selector_does_not_parse_is_dropped_not_defaulted_to_zero() {
    // Zero is a real entry, and acting on the wrong one is the mistake this
    // module is arranged to avoid.
    let stdout = record("not-a-selector", "On main: x", "1788024729");
    assert!(parse(&output(&stdout)).unwrap().is_empty());
}

#[test]
fn an_empty_stack_is_an_empty_list_and_not_an_error() {
    assert!(parse(&output("")).unwrap().is_empty());
}

#[test]
fn a_conflicting_pop_says_the_entry_is_still_there() {
    let conflicted = GitOutput {
        code: Some(1),
        stdout: "CONFLICT (content): Merge conflict in src/main.rs\n".to_string(),
        stderr: String::new(),
    };
    let message = failure(&conflicted, "stash pop").to_string();
    assert!(message.contains("still on the stack"), "{message}");
}

#[test]
fn stashing_a_clean_tree_says_so_rather_than_failing_obscurely() {
    let refused = GitOutput {
        code: Some(1),
        stdout: "No local changes to save\n".to_string(),
        stderr: String::new(),
    };
    let message = failure(&refused, "stash push").to_string();
    assert!(message.contains("nothing to stash"), "{message}");
}
