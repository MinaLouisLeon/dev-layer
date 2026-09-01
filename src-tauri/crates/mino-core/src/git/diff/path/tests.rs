//! Every one of these was taken from real `git diff` output.

use super::*;

#[test]
fn an_ordinary_path_loses_only_its_prefix() {
    assert_eq!(from_header("a/src/main.rs").as_deref(), Some("src/main.rs"));
    assert_eq!(from_header("b/src/main.rs").as_deref(), Some("src/main.rs"));
}

#[test]
fn a_path_with_a_space_arrives_with_a_trailing_tab() {
    // The case that makes `diff --git a/x y b/x y` unparseable and these lines
    // parseable: git marks the end of the name.
    assert_eq!(
        from_header("a/release notes.md\t").as_deref(),
        Some("release notes.md")
    );
    assert_eq!(
        from_header("b/docs/two words.txt\t").as_deref(),
        Some("docs/two words.txt")
    );
}

#[test]
fn dev_null_is_an_absence_not_a_name() {
    assert_eq!(from_header("/dev/null"), None);
}

#[test]
fn a_file_really_called_a_slash_something_keeps_its_name() {
    // Git still prefixes it, so exactly one `a/` comes off.
    assert_eq!(from_header("a/a/nested.rs").as_deref(), Some("a/nested.rs"));
}

#[test]
fn the_diff_git_pair_is_split_where_the_halves_match() {
    // The fallback for a binary file and a mode-only change, which have no
    // ---/+++ lines at all. A space in the name makes this ambiguous, and
    // equality of the two halves is what resolves it.
    assert_eq!(
        from_pair("a/src/main.rs b/src/main.rs").as_deref(),
        Some("src/main.rs")
    );
    assert_eq!(
        from_pair("a/release notes.md b/release notes.md").as_deref(),
        Some("release notes.md")
    );
    // A rename has unequal halves, so the new side is taken. `rename to`
    // corrects it afterwards anyway.
    assert_eq!(
        from_pair("a/before.txt b/after.txt").as_deref(),
        Some("after.txt")
    );
    assert_eq!(from_pair("nonsense"), None);
}

#[test]
fn a_rename_line_carries_the_path_on_its_own() {
    // The only source of a path for a pure rename, which has no ---/+++ lines.
    assert_eq!(
        from_rename("src/before.rs").as_deref(),
        Some("src/before.rs")
    );
    assert_eq!(from_rename("").as_deref(), None);
}

#[test]
fn c_quoting_is_undone() {
    assert_eq!(unquote(r#""a\tb""#), "a\tb");
    assert_eq!(unquote(r#""say \"hi\"""#), r#"say "hi""#);
    // Git escapes a real backslash as two, so the quoted value carries two.
    assert_eq!(unquote("\"back\\\\slash\""), "back\\slash");
    // And an escape this does not know keeps the character rather than the
    // backslash, which is what git means by it.
    assert_eq!(unquote(r#""a\sb""#), "asb");
}

#[test]
fn octal_escapes_decode_as_bytes_not_characters() {
    // `é` is two bytes in UTF-8, and git escapes each one separately.
    assert_eq!(unquote(r#""caf\303\251.md""#), "café.md");
    assert_eq!(
        from_header(r#""b/caf\303\251.md""#).as_deref(),
        Some("café.md")
    );
}

#[test]
fn an_unquoted_value_is_left_alone() {
    // The common case: every call passes `core.quotepath=false`, so a
    // non-ASCII name usually arrives raw.
    assert_eq!(unquote("café.md"), "café.md");
    assert_eq!(from_header("a/café.md").as_deref(), Some("café.md"));
}
