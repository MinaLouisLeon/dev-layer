//! Recorded `git blame --porcelain` output.
//!
//! The recording below is the real thing, including the detail that shapes the
//! whole parser: the second line of a commit has no header block at all.

use super::*;

const FIRST: &str = "6651ceeb58b5ca5f97526e1796fe82a0ce36d796";
const SECOND: &str = "1fefad39f457446981982ab364a3e28f17b8a437";

const RECORDED: &str = "\
6651ceeb58b5ca5f97526e1796fe82a0ce36d796 1 1 2
author A Author
author-mail <t@e.invalid>
author-time 1788027873
author-tz +0300
committer A Author
committer-mail <t@e.invalid>
committer-time 1788027873
committer-tz +0300
summary first subject
boundary
filename f.txt
\talpha
6651ceeb58b5ca5f97526e1796fe82a0ce36d796 2 2
\tbeta
1fefad39f457446981982ab364a3e28f17b8a437 3 3 2
author B Author
author-mail <t@e.invalid>
author-time 1788027999
author-tz +0300
committer B Author
committer-mail <t@e.invalid>
committer-time 1788027999
committer-tz +0300
summary second subject
previous 6651ceeb58b5ca5f97526e1796fe82a0ce36d796 f.txt
filename f.txt
\tgamma
1fefad39f457446981982ab364a3e28f17b8a437 4 4
\tdelta
";

fn output(code: i32, stdout: &str) -> GitOutput {
    GitOutput {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

#[test]
fn every_line_is_attributed_to_the_commit_that_introduced_it() {
    let blame = parse(&output(0, RECORDED), "f.txt").unwrap();
    assert_eq!(blame.relative_path, "f.txt");
    assert!(!blame.truncated);

    let rows: Vec<_> = blame
        .lines
        .iter()
        .map(|l| (l.line, l.sha.as_str(), l.author.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (1, FIRST, "A Author"),
            // The line whose header block git omitted. Getting this one right
            // is the whole point of keeping a map.
            (2, FIRST, "A Author"),
            (3, SECOND, "B Author"),
            (4, SECOND, "B Author"),
        ]
    );
}

#[test]
fn a_repeated_commit_keeps_its_time_and_summary_too() {
    let blame = parse(&output(0, RECORDED), "f.txt").unwrap();
    assert_eq!(blame.lines[1].timestamp_ms, 1_788_027_873_000);
    assert_eq!(blame.lines[1].summary, "first subject");
    assert_eq!(blame.lines[3].timestamp_ms, 1_788_027_999_000);
    assert_eq!(blame.lines[3].summary, "second subject");
}

#[test]
fn the_short_sha_is_the_gutters_width() {
    let blame = parse(&output(0, RECORDED), "f.txt").unwrap();
    assert_eq!(blame.lines[0].short_sha, &FIRST[..BLAME_SHA_LENGTH]);
    assert!(blame.lines[0].sha.starts_with(&blame.lines[0].short_sha));
}

#[test]
fn line_numbers_come_from_the_final_file_not_the_original() {
    // The header is `<sha> <line in the original> <line in the final file>`.
    // A gutter that used the first number would misplace every moved line.
    let moved = "\
6651ceeb58b5ca5f97526e1796fe82a0ce36d796 40 7 1
author A Author
author-time 1788027873
summary moved
filename f.txt
\tmoved line
";
    let blame = parse(&output(0, moved), "f.txt").unwrap();
    assert_eq!(blame.lines[0].line, 7);
}

#[test]
fn content_that_looks_like_a_header_is_still_content() {
    // A file whose own text begins with a sha. The tab prefix is what makes
    // content unambiguous, and it is checked first.
    let tricky = format!(
        "{FIRST} 1 1 1\nauthor A Author\nauthor-time 1\nsummary s\nfilename f.txt\n\t{FIRST} 9 9 9\n"
    );
    let blame = parse(&output(0, &tricky), "f.txt").unwrap();
    assert_eq!(blame.lines.len(), 1);
    assert_eq!(blame.lines[0].line, 1);
}

#[test]
fn an_enormous_file_is_cut_and_says_so() {
    let mut recorded = String::new();
    for line in 1..=MAX_BLAME_LINES + 10 {
        recorded.push_str(&format!(
            "{FIRST} {line} {line} 1\nauthor A\nauthor-time 1\nsummary s\nfilename f\n\tx\n"
        ));
    }
    let blame = parse(&output(0, &recorded), "f.txt").unwrap();
    assert!(blame.truncated);
    assert_eq!(blame.lines.len(), MAX_BLAME_LINES as usize);
}

#[test]
fn a_failed_blame_is_reported() {
    let refused = GitOutput {
        code: Some(128),
        stdout: String::new(),
        stderr: "fatal: no such path".to_string(),
    };
    assert!(parse(&refused, "missing.rs").is_err());
}
