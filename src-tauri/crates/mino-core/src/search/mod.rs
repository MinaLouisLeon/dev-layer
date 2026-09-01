//! Filename search: the matcher, and the budget a walk runs inside.
//!
//! The walking itself is per-transport - `std::fs` locally, SFTP over SSH -
//! but everything a walk *decides* is here, so the two cannot drift into
//! ranking differently, skipping different directories or disagreeing about
//! when a result set is truncated.

pub mod fuzzy;
pub mod ignore;
mod scoring;

use std::time::{Duration, Instant};

use crate::types::{
    DirEntry, EntryKind, SearchHit, SearchHits, SearchQuery, MAX_SCANNED_ENTRIES,
    SEARCH_TIMEOUT_MS, SKIPPED_DIRECTORIES,
};

pub use ignore::IgnoreSet;

/// True for a directory a walk must not descend into. Matched on the
/// directory's own name, so `node_modules` is skipped at any depth.
pub fn is_skipped_directory(name: &str) -> bool {
    SKIPPED_DIRECTORIES.contains(&name)
}

/// Turns an absolute path into the root-relative, forward-slashed form that is
/// matched and displayed. Falls back to the whole path when it somehow sits
/// outside the root - the guards make that unreachable, but a wrong string is
/// a better outcome here than a panic.
pub fn relative_to(root: &str, path: &str) -> String {
    let normalised = path.replace('\\', "/");
    let base = root.replace('\\', "/");
    let trimmed = normalised
        .strip_prefix(&base)
        .map(|rest| rest.trim_start_matches('/'))
        .unwrap_or(&normalised);
    trimmed.to_string()
}

/// Gathers matches under a query's limits and a wall-clock deadline.
///
/// A walk offers every entry it visits and asks, after each one, whether to
/// keep going. All the stopping conditions live here rather than in the walk,
/// so "when does a search give up" has one answer.
pub struct Collector {
    query: String,
    limit: usize,
    include_hidden: bool,
    include_directories: bool,
    hits: Vec<SearchHit>,
    scanned: u32,
    truncated: bool,
    deadline: Instant,
    /// What git said it would not look at. Empty unless a transport supplied
    /// one, which is what makes ignoring an addition to the skip list rather
    /// than a replacement for it.
    ignores: IgnoreSet,
}

impl Collector {
    pub fn new(query: &SearchQuery) -> Self {
        Self {
            query: query.query.clone(),
            limit: query.effective_limit(),
            include_hidden: query.include_hidden,
            include_directories: query.include_directories,
            hits: Vec::new(),
            scanned: 0,
            truncated: false,
            deadline: Instant::now() + Duration::from_millis(SEARCH_TIMEOUT_MS),
            ignores: IgnoreSet::default(),
        }
    }

    /// Adds git's ignore rules to this walk.
    ///
    /// A builder rather than a constructor argument, so a transport that has
    /// no git - or whose git call failed - simply does not call it and keeps
    /// today's behaviour exactly.
    pub fn with_ignores(mut self, ignores: IgnoreSet) -> Self {
        self.ignores = ignores;
        self
    }

    /// True for a path the walk should neither offer nor descend into. Asked
    /// by the walk for directories, and applied by [`Collector::offer`] for
    /// everything else, so the two cannot disagree.
    pub fn is_ignored(&self, relative: &str) -> bool {
        self.ignores.contains(relative)
    }

    /// Offers one visited entry. `relative` is its root-relative path, which
    /// is what the query is matched against.
    pub fn offer(&mut self, entry: DirEntry, relative: String) {
        self.scanned += 1;
        if self.is_ignored(&relative) {
            return;
        }
        if entry.hidden && !self.include_hidden {
            return;
        }
        if matches!(entry.kind, EntryKind::Directory) && !self.include_directories {
            return;
        }
        let Some(matched) = fuzzy::score(&self.query, &relative) else {
            return;
        };
        self.hits.push(SearchHit {
            entry,
            relative_path: relative,
            score: matched.score,
            match_indices: matched.indices,
        });
    }

    /// False once the walk has spent its budget. Checked between entries, so a
    /// walk stops promptly without a cancellation channel.
    pub fn should_continue(&mut self) -> bool {
        if self.scanned >= MAX_SCANNED_ENTRIES || Instant::now() >= self.deadline {
            self.truncated = true;
            return false;
        }
        true
    }

    /// Ranks what was found and cuts it to the limit.
    ///
    /// Ties break on the shorter path, then alphabetically, so an unchanged
    /// tree always produces the same order - a list that reshuffles between
    /// identical searches is worse than one that ranks imperfectly.
    pub fn finish(mut self) -> SearchHits {
        self.hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.relative_path.len().cmp(&b.relative_path.len()))
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        let truncated = self.truncated || self.hits.len() > self.limit;
        self.hits.truncate(self.limit);
        SearchHits {
            hits: self.hits,
            truncated,
            scanned: self.scanned,
        }
    }
}
