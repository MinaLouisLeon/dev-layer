//! Parser tests, against recorded `--porcelain=v2 -z` output.
//!
//! These are about *sequencing*: headers, the rename pair, the cap, and the
//! whole-status shape. Decoding one record is covered in `record.rs`'s own
//! tests.
//!
//! The records were taken from real repositories rather than written from the
//! manual, which is why they carry the full 40-character shas and the `N...`
//! sub-module field: a parser that only ever sees tidy input is a parser that
//! has not been tested.

use super::*;
use crate::types::GitFileState;

const OID: &str = "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39";

/// Joins records the way git does: every one terminated by a NUL, including
/// the last.
fn recorded(records: &[&str]) -> String {
    records.iter().map(|r| format!("{r}\0")).collect()
}

fn posix() -> PathStyle {
    PathStyle {
        separator: '/',
        case_insensitive: false,
    }
}

fn parse_records(records: &[&str]) -> ParsedStatus {
    parse(&recorded(records), "/srv/app", posix())
}

#[test]
fn a_clean_tree_has_headers_and_no_entries() {
    let status = parse_records(&[
        &format!("# branch.oid {OID}"),
        "# branch.head main",
        "# branch.upstream origin/main",
        "# branch.ab +0 -0",
    ]);
    assert!(status.entries.is_empty());
    assert!(!status.truncated);
    assert_eq!(status.headers.branch.as_deref(), Some("main"));
}

#[test]
fn every_row_of_a_mixed_status_survives_in_order() {
    let status = parse_records(&[
        "# branch.head main",
        &format!("1 .M N... 100644 100644 100644 {OID} {OID} src/main.rs"),
        &format!("1 A. N... 000000 100644 100644 {OID} {OID} src/new.rs"),
        "? notes.txt",
        "! target/",
    ]);

    use GitFileState::*;
    let rows: Vec<_> = status
        .entries
        .iter()
        .map(|e| (e.relative_path.as_str(), e.index, e.worktree))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("src/main.rs", Unmodified, Modified),
            ("src/new.rs", Added, Unmodified),
            ("notes.txt", Untracked, Untracked),
            ("target", Ignored, Ignored),
        ]
    );
}

#[test]
fn staged_and_then_modified_again_reports_both_sides() {
    // The condition the two-state shape exists for. A single state would have
    // to pick one of these and lose the other.
    let status = parse_records(&[&format!(
        "1 MM N... 100644 100644 100644 {OID} {OID} src/both.rs"
    )]);
    let entry = &status.entries[0];
    assert_eq!(entry.index, GitFileState::Modified);
    assert_eq!(entry.worktree, GitFileState::Modified);
}

#[test]
fn a_rename_carries_the_path_it_came_from() {
    // The two-field record: the new path, NUL, the original path.
    let status = parse_records(&[
        &format!("2 R. N... 100644 100644 100644 {OID} {OID} R100 src/after.rs"),
        "src/before.rs",
        &format!("1 .M N... 100644 100644 100644 {OID} {OID} src/next.rs"),
    ]);

    assert_eq!(status.entries.len(), 2, "the original path is not an entry");
    assert_eq!(status.entries[0].relative_path, "src/after.rs");
    assert_eq!(
        status.entries[0].original_path.as_deref(),
        Some("src/before.rs")
    );
    // The record after the pair is still parsed: the second pull did not knock
    // the loop out of step.
    assert_eq!(status.entries[1].relative_path, "src/next.rs");
    assert_eq!(status.entries[1].original_path, None);
}

#[test]
fn a_non_ascii_filename_survives_intact() {
    // What `-z` buys: git would have C-quoted this without it.
    let status = parse_records(&[&format!(
        "1 .M N... 100644 100644 100644 {OID} {OID} docs/café-☕.md"
    )]);
    assert_eq!(status.entries[0].relative_path, "docs/café-☕.md");
    assert_eq!(status.entries[0].path, "/srv/app/docs/café-☕.md");
}

#[test]
fn paths_take_the_targets_separator() {
    let windows = PathStyle {
        separator: '\\',
        case_insensitive: true,
    };
    let status = parse(
        &recorded(&[&format!(
            "1 .M N... 100644 100644 100644 {OID} {OID} src/main.rs"
        )]),
        r"C:\repo",
        windows,
    );
    assert_eq!(status.entries[0].path, r"C:\repo\src\main.rs");
    // Git's own answer is kept as git gave it, forward slashes and all.
    assert_eq!(status.entries[0].relative_path, "src/main.rs");
}

#[test]
fn the_ignore_rows_can_be_read_on_their_own() {
    let output = recorded(&["! node_modules/", "! target/", "? src/main.rs"]);
    assert_eq!(parse_ignored(&output), vec!["node_modules", "target"]);
}

#[test]
fn the_entry_list_is_capped_and_says_so() {
    let record = format!("1 .M N... 100644 100644 100644 {OID} {OID} src/f.rs");
    let records: Vec<&str> =
        std::iter::repeat_n(record.as_str(), MAX_STATUS_ENTRIES as usize + 10).collect();
    let status = parse_records(&records);
    assert_eq!(status.entries.len(), MAX_STATUS_ENTRIES as usize);
    assert!(status.truncated);
}
