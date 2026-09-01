//! The three file shapes whose name does not come from a `+++` line.
//!
//! A pure rename has no `---`/`+++` lines at all, a binary file has none
//! either, and a path containing a space arrives with a trailing tab. Each was
//! found by running real git, and each would have produced a nameless entry -
//! which this parser drops - if it had not been.

use crate::git::diff::parse;

#[test]
fn a_pure_rename_has_no_hunks_and_still_names_both_paths() {
    // Recorded from a real `git mv` with no edit. There are no ---/+++ lines
    // at all, which is why the rename lines are read.
    let diff = parse(
        "\
diff --git a/before.txt b/after.txt
similarity index 100%
rename from before.txt
rename to after.txt
",
    );
    let file = &diff.files[0];
    assert_eq!(file.relative_path, "after.txt");
    assert_eq!(file.old_path.as_deref(), Some("before.txt"));
    assert!(file.hunks.is_empty());
}

#[test]
fn a_binary_file_says_so_instead_of_carrying_megabytes() {
    let diff = parse(
        "\
diff --git a/logo.bin b/logo.bin
index c94be36..1d1cf9c 100644
Binary files a/logo.bin and b/logo.bin differ
",
    );
    assert!(diff.files[0].binary);
    assert!(diff.files[0].hunks.is_empty());
    assert_eq!(diff.files[0].relative_path, "logo.bin");
}

#[test]
fn a_path_with_a_space_survives_the_trailing_tab() {
    let diff = parse(
        "\
diff --git a/release notes.md b/release notes.md
index 814f4a4..879de50 100644
--- a/release notes.md\t
+++ b/release notes.md\t
@@ -1,2 +1,2 @@
 one
-two
+TWO
",
    );
    assert_eq!(diff.files[0].relative_path, "release notes.md");
}
