//! Decoding one record: the state pair, where each shape keeps its path, and
//! what happens to a record that makes no sense.
//!
//! Sequencing records - headers, the rename pair, the cap - is covered in
//! `porcelain/tests.rs`.

use super::*;

const OID: &str = "3f2a1c9d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39";

fn posix() -> PathStyle {
    PathStyle {
        separator: '/',
        case_insensitive: false,
    }
}

fn decode(record: &str) -> GitEntry {
    entry_from(record, None, "/srv/app", posix()).expect("a decodable record")
}

#[test]
fn the_two_sides_come_from_the_xy_pair() {
    let entry = decode(&format!(
        "1 MD N... 100644 100644 100644 {OID} {OID} src/main.rs"
    ));
    assert_eq!(entry.index, GitFileState::Modified);
    assert_eq!(entry.worktree, GitFileState::Deleted);
    assert_eq!(entry.relative_path, "src/main.rs");
}

#[test]
fn a_dot_is_a_clean_side_and_so_is_an_unknown_code() {
    assert_eq!(state('.'), GitFileState::Unmodified);
    assert_eq!(state('X'), GitFileState::Unmodified);
    assert_eq!(
        states(".M"),
        (GitFileState::Unmodified, GitFileState::Modified)
    );
}

#[test]
fn each_record_type_finds_its_path_at_the_right_field() {
    assert_eq!(
        decode(&format!(
            "2 R. N... 100644 100644 100644 {OID} {OID} R100 src/after.rs"
        ))
        .relative_path,
        "src/after.rs"
    );
    assert_eq!(
        decode(&format!(
            "u UU N... 100644 100644 100644 100644 {OID} {OID} {OID} src/merge.rs"
        ))
        .relative_path,
        "src/merge.rs"
    );
    assert_eq!(decode("? notes.txt").relative_path, "notes.txt");
    assert_eq!(decode("! target/").relative_path, "target");
}

#[test]
fn a_path_with_spaces_is_taken_whole() {
    let entry = decode(&format!(
        "1 .M N... 100644 100644 100644 {OID} {OID} docs/release notes.md"
    ));
    assert_eq!(entry.relative_path, "docs/release notes.md");
    assert_eq!(entry.path, "/srv/app/docs/release notes.md");
}

#[test]
fn a_malformed_or_unknown_record_is_dropped_not_guessed_at() {
    assert!(entry_from("x nonsense", None, "/srv/app", posix()).is_none());
    assert!(entry_from("1 .M", None, "/srv/app", posix()).is_none());
    assert!(entry_from("? /", None, "/srv/app", posix()).is_none());
}
