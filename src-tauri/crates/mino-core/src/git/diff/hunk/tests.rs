//! Hunk headers, line numbering, and the cap.
//!
//! The file-level shapes - renames, binaries, deletions - are in
//! `diff/tests.rs`. What is here is where each line lands, which is the part a
//! renderer would otherwise have to work out for itself.

use crate::git::diff::parse;
use crate::types::MAX_DIFF_LINES;

/// The ordinary case, repeated here so these tests read on their own.
const MODIFIED: &str = "diff --git a/keep.txt b/keep.txt
index b2f931a..820620b 100644
--- a/keep.txt
+++ b/keep.txt
@@ -1,5 +1,5 @@
 one
-two
+TWO
 three
 four
-five
+FIVE
";

#[test]
fn a_hunk_header_carries_gits_context_note() {
    let patch = MODIFIED.replace("@@ -1,5 +1,5 @@", "@@ -1,5 +1,5 @@ fn main()");
    let diff = parse(&patch);
    assert_eq!(diff.files[0].hunks[0].header, "fn main()");
    // And the ranges still parsed around it.
    assert_eq!(diff.files[0].hunks[0].new_lines, 5);
}

#[test]
fn a_one_line_range_omits_its_count() {
    // `@@ -1 +1 @@` rather than `@@ -1,1 +1,1 @@`. Both are real output.
    let diff = parse(
        "diff --git a/x b/x
--- a/x
+++ b/x
@@ -7 +7 @@
-a
+b
",
    );
    let hunk = &diff.files[0].hunks[0];
    assert_eq!((hunk.old_start, hunk.old_lines), (7, 1));
    assert_eq!(hunk.lines[0].old_line, Some(7));
}

#[test]
fn several_files_and_several_hunks_stay_separate() {
    let patch = format!(
        "{MODIFIED}diff --git a/other.txt b/other.txt
index 1..2 100644
--- a/other.txt
+++ b/other.txt
@@ -10,2 +10,3 @@
 ten
+eleven
 twelve
@@ -40,1 +41,1 @@
-forty
+FORTY
"
    );
    let diff = parse(&patch);
    assert_eq!(diff.files.len(), 2);
    assert_eq!(diff.files[1].hunks.len(), 2);
    // The second hunk starts where its header said, not where the first ended.
    assert_eq!(diff.files[1].hunks[1].lines[0].old_line, Some(40));
    assert_eq!(diff.files[1].hunks[1].lines[1].new_line, Some(41));
}

#[test]
fn an_enormous_diff_is_cut_and_says_so() {
    let mut patch = String::from(
        "diff --git a/big.txt b/big.txt
--- a/big.txt
+++ b/big.txt
",
    );
    patch.push_str(&format!(
        "@@ -1,{0} +1,{0} @@
",
        MAX_DIFF_LINES + 10
    ));
    for index in 0..MAX_DIFF_LINES + 10 {
        patch.push_str(&format!(
            "+line {index}
"
        ));
    }
    let diff = parse(&patch);
    assert!(diff.truncated);
    assert_eq!(diff.files[0].hunks[0].lines.len(), MAX_DIFF_LINES as usize);
}
