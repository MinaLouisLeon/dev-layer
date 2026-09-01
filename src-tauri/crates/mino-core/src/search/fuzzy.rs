//! Subsequence matching, the way a quick-open box behaves.
//!
//! `ftp` matches `FileTreePane.tsx`; `srcmain` matches `src/main.rs`. Every
//! query character must appear, in order, somewhere in the candidate - but not
//! adjacently, which is what separates this from a substring search.
//!
//! It lives in the core rather than in the UI for one reason: every transport
//! must rank the same set of files the same way, or the search pane would
//! reorder itself when a session moved from local to SSH. The weights are in
//! [`super::scoring`]; this file decides *where* the match lands.
//!
//! Every alignment is tried, rather than the usual two-pass shortcut of
//! scanning forward for the earliest end and then backward to tighten. That
//! shortcut is cheaper and wrong in the case that matters most: searching
//! `main` in `src/domain/main.rs` it locks onto the `main` inside `domain`,
//! and the filename is never offered to the scorer at all. So each occurrence
//! of the query's first character is tried as a start and the best-scoring
//! alignment wins - bounded by (occurrences of that character) x (candidate
//! length), which is cheap on something as short as a path.

use super::scoring;

/// A scored match: how good it is, and where it landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    /// Character (not byte) indices into the candidate, ascending.
    pub indices: Vec<u32>,
}

/// Scores `query` against `candidate`, or `None` when the characters of
/// `query` do not appear in it in order.
///
/// An empty query matches everything with a score of zero, which is what keeps
/// "list the tree" and "show me matches" on one code path.
pub fn score(query: &str, candidate: &str) -> Option<Match> {
    let needle: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if needle.is_empty() {
        return Some(Match {
            score: 0,
            indices: Vec::new(),
        });
    }
    let hay: Vec<char> = candidate.chars().collect();
    if needle.len() > hay.len() {
        return None;
    }
    let lower: Vec<char> = hay
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();

    let mut best: Option<Match> = None;
    for start in 0..lower.len() {
        if lower[start] != needle[0] {
            continue;
        }
        // Once the remainder cannot be matched from here it cannot be matched
        // from anywhere later either, so this ends the search rather than
        // skipping to the next occurrence.
        let Some(indices) = take_from(&needle, &lower, start) else {
            break;
        };
        let rated = scoring::rate(&hay, &indices);
        if best.as_ref().is_none_or(|found| rated > found.score) {
            best = Some(Match {
                score: rated,
                indices: indices.iter().map(|i| *i as u32).collect(),
            });
        }
    }
    best
}

/// Pins the first query character at `start` and takes the first available
/// match for each character after it. `None` when the candidate runs out
/// before the query does.
fn take_from(needle: &[char], lower: &[char], start: usize) -> Option<Vec<usize>> {
    let mut indices = Vec::with_capacity(needle.len());
    indices.push(start);
    let mut at = start + 1;
    for want in &needle[1..] {
        let found = lower[at..].iter().position(|c| c == want)?;
        at += found + 1;
        indices.push(at - 1);
    }
    Some(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indices(query: &str, candidate: &str) -> Option<Vec<u32>> {
        score(query, candidate).map(|m| m.indices)
    }

    #[test]
    fn initials_match_a_camel_case_name() {
        assert_eq!(indices("ftp", "FileTreePane.tsx"), Some(vec![0, 4, 8]));
    }

    #[test]
    fn a_query_that_is_not_a_subsequence_does_not_match() {
        assert!(score("zzz", "src/main.rs").is_none());
        // Right characters, wrong order.
        assert!(score("niam", "main.rs").is_none());
    }

    /// The case the two-pass shortcut gets wrong: `main` appears in a
    /// directory name before it appears in the filename, and the filename is
    /// the one a person means.
    #[test]
    fn the_filename_is_matched_over_an_earlier_directory_name() {
        assert_eq!(
            indices("main", "src/domain/main.rs"),
            Some(vec![11, 12, 13, 14])
        );
    }

    #[test]
    fn a_tight_run_is_preferred_to_an_earlier_scattered_one() {
        // `ab` is scattered at 0 and 5, and adjacent at 8.
        assert_eq!(indices("ab", "a....b..ab"), Some(vec![8, 9]));
    }

    #[test]
    fn an_empty_query_matches_everything_neutrally() {
        let matched = score("", "src/main.rs").unwrap();
        assert_eq!(matched.score, 0);
        assert!(matched.indices.is_empty());
    }

    #[test]
    fn matching_is_case_insensitive_in_both_directions() {
        assert!(score("MAIN", "src/main.rs").is_some());
        assert!(score("main", "src/MAIN.RS").is_some());
    }

    #[test]
    fn whitespace_in_a_query_is_ignored_rather_than_matched() {
        // Someone typing "file tree" means both words, not a literal space.
        assert!(score("file tree", "FileTreePane.tsx").is_some());
    }
}
