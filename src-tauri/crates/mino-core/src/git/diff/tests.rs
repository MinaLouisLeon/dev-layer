//! Parser tests against recorded `git diff` output.
//!
//! Every patch below was captured from a real repository rather than written
//! from the manual - which is how the trailing tab after a path with a space,
//! and the fact that a pure rename has no `---`/`+++` lines at all, came to be
//! handled here instead of being found later.

use super::*;
use crate::types::GitDiffLineKind;

/// The ordinary case: one file, one hunk, two edits.
const MODIFIED: &str = "\
diff --git a/keep.txt b/keep.txt
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
fn a_modified_file_keeps_its_line_numbers_on_both_sides() {
    let diff = parse(MODIFIED);
    assert_eq!(diff.files.len(), 1);
    assert!(!diff.truncated);

    let file = &diff.files[0];
    assert_eq!(file.relative_path, "keep.txt");
    assert!(!file.binary);
    assert_eq!(file.old_path, None);

    let hunk = &file.hunks[0];
    assert_eq!((hunk.old_start, hunk.old_lines), (1, 5));
    assert_eq!((hunk.new_start, hunk.new_lines), (1, 5));

    let numbered: Vec<_> = hunk
        .lines
        .iter()
        .map(|l| (l.kind, l.content.as_str(), l.old_line, l.new_line))
        .collect();
    use GitDiffLineKind::*;
    assert_eq!(
        numbered,
        vec![
            (Context, "one", Some(1), Some(1)),
            // A removed line has no new-side number, and an added one has no
            // old-side number. That is the whole reason both are optional.
            (Removed, "two", Some(2), None),
            (Added, "TWO", None, Some(2)),
            (Context, "three", Some(3), Some(3)),
            (Context, "four", Some(4), Some(4)),
            (Removed, "five", Some(5), None),
            (Added, "FIVE", None, Some(5)),
        ]
    );
}

#[test]
fn a_new_file_has_no_old_side() {
    let diff = parse(
        "\
diff --git a/added.txt b/added.txt
new file mode 100644
index 0000000..858e580
--- /dev/null
+++ b/added.txt
@@ -0,0 +1,2 @@
+brand new
+second line
",
    );
    let file = &diff.files[0];
    assert_eq!(file.relative_path, "added.txt");
    assert!(file.hunks[0].lines.iter().all(|l| l.old_line.is_none()));
    assert_eq!(file.hunks[0].lines[1].new_line, Some(2));
}

#[test]
fn a_deleted_file_is_named_from_its_old_side() {
    // `+++` is `/dev/null` here, so the name has to come from `---`.
    let diff = parse(
        "\
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 858e580..0000000
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-it was here
",
    );
    assert_eq!(diff.files[0].relative_path, "gone.txt");
    assert_eq!(diff.files[0].hunks[0].lines[0].old_line, Some(1));
}

#[test]
fn no_newline_at_end_of_file_marks_the_line_above_it() {
    let diff = parse(
        "\
diff --git a/tail.txt b/tail.txt
index 69db55d..acc92b8 100644
--- a/tail.txt
+++ b/tail.txt
@@ -1 +1 @@
-no trailing newline
\\ No newline at end of file
+no trailing newline CHANGED
\\ No newline at end of file
",
    );
    let lines = &diff.files[0].hunks[0].lines;
    // Two content lines, not four: the marker is metadata on the line above.
    assert_eq!(lines.len(), 2);
    assert!(lines[0].no_newline);
    assert!(lines[1].no_newline);
    assert_eq!(lines[1].content, "no trailing newline CHANGED");
}

#[test]
fn an_empty_diff_is_an_empty_answer_not_a_failure() {
    let diff = parse("");
    assert!(diff.is_empty());
    assert!(!diff.truncated);
}
