//! What a match is worth.
//!
//! Kept apart from [`super::fuzzy`] so that file describes *finding* a match
//! and this one describes *judging* it. Tuning ranking means editing the
//! weights below and nothing else.

/// Weights, in one place so the ratios between them are visible at a glance.
/// Tuned so that for query `main`, `src/main.rs` outranks `src/domain/in.rs`:
/// consecutive characters on a word boundary are worth far more than the same
/// characters scattered across a long path.
mod weight {
    /// Every matched character earns this much, before bonuses.
    pub const MATCH: i32 = 16;
    /// Added for each character directly following the previous match.
    pub const CONSECUTIVE: i32 = 20;
    /// First character of a path segment, e.g. the `m` of `main.rs`.
    pub const SEGMENT_START: i32 = 26;
    /// After `_`, `-`, `.` or a space, or at a lowercase-to-uppercase step.
    pub const WORD_BOUNDARY: i32 = 18;
    /// The character landed in the last segment - the filename itself, which
    /// is what people are nearly always looking for.
    pub const IN_FILENAME: i32 = 10;
    /// The candidate character is uppercase, so the run reads as a word start
    /// even without a separator in front of it.
    pub const CAPITAL: i32 = 4;
    /// Charged per unmatched character *between* matches, so a tight run beats
    /// a sprawling one. Characters before the first match are not charged:
    /// skipping a directory prefix to reach the filename is the normal case.
    pub const GAP: i32 = -3;
    /// Charged per candidate character, so between two otherwise equal matches
    /// the shorter path wins.
    pub const LENGTH: i32 = -1;
}

/// Rates a set of matched positions against the candidate they landed in.
/// `indices` must be ascending and in range - [`super::fuzzy`] guarantees both.
pub fn rate(hay: &[char], indices: &[usize]) -> i32 {
    let filename_start = filename_start(hay);
    let mut total = weight::LENGTH * hay.len() as i32;
    let mut previous: Option<usize> = None;

    for index in indices.iter().copied() {
        total += weight::MATCH;
        if index >= filename_start {
            total += weight::IN_FILENAME;
        }
        if hay[index].is_uppercase() {
            total += weight::CAPITAL;
        }
        total += boundary_bonus(hay, index);
        if let Some(before) = previous {
            total += if before + 1 == index {
                weight::CONSECUTIVE
            } else {
                weight::GAP * (index - before - 1) as i32
            };
        }
        previous = Some(index);
    }
    total
}

/// Index of the first character of the last path segment.
fn filename_start(hay: &[char]) -> usize {
    hay.iter()
        .rposition(|c| *c == '/' || *c == '\\')
        .map(|i| i + 1)
        .unwrap_or(0)
}

fn boundary_bonus(hay: &[char], index: usize) -> i32 {
    let Some(before) = index.checked_sub(1).map(|i| hay[i]) else {
        return weight::SEGMENT_START;
    };
    if before == '/' || before == '\\' {
        return weight::SEGMENT_START;
    }
    if matches!(before, '_' | '-' | '.' | ' ') {
        return weight::WORD_BOUNDARY;
    }
    if before.is_lowercase() && hay[index].is_uppercase() {
        return weight::WORD_BOUNDARY;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::super::fuzzy::score;

    fn rank(query: &str, candidate: &str) -> i32 {
        score(query, candidate).expect("expected a match").score
    }

    #[test]
    fn a_tight_match_on_the_filename_beats_a_scattered_one() {
        assert!(rank("main", "src/main.rs") > rank("main", "src/domain/in.rs"));
    }

    #[test]
    fn the_shorter_of_two_equally_good_paths_wins() {
        assert!(rank("main", "src/main.rs") > rank("main", "src/deep/nested/main.rs"));
    }

    #[test]
    fn a_filename_match_beats_the_same_run_in_a_directory_name() {
        assert!(rank("core", "src/core.rs") > rank("core", "core/src/lib.rs"));
    }

    #[test]
    fn word_starts_beat_characters_buried_mid_word() {
        assert!(rank("ftp", "FileTreePane.tsx") > rank("ftp", "shiftplan.rs"));
    }
}
