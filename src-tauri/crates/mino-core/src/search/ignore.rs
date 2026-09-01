//! The optional ignore predicate a walk runs under.
//!
//! Built from what git said it would not look at, so a search of a repository
//! stops offering build output the person searching does not mean. It sits
//! beside [`crate::types::SKIPPED_DIRECTORIES`] rather than replacing it: the
//! hard-coded list is what a folder with no git, and no repository, still
//! gets.
//!
//! **Degrading is the point.** An empty set ignores nothing, and every failure
//! upstream - git absent, not a repository, a call that timed out - produces
//! exactly that. Losing search because a folder is not a checkout would be a
//! regression, and this is the shape that makes it impossible.

use std::collections::HashSet;

/// Repository-relative paths git reported as ignored, as prefixes.
///
/// A wholly-ignored directory arrives from `--ignored=matching` as one row, so
/// this holds `node_modules` rather than its forty thousand children, and
/// [`IgnoreSet::contains`] answers for everything beneath it.
#[derive(Debug, Clone, Default)]
pub struct IgnoreSet {
    paths: HashSet<String>,
    /// Longest path in the set, in separator-delimited segments. A candidate
    /// deeper than this cannot be matched by any prefix, so most lookups stop
    /// after one comparison rather than climbing the whole path.
    depth: usize,
}

impl IgnoreSet {
    pub fn new(paths: impl IntoIterator<Item = String>) -> Self {
        let paths: HashSet<String> = paths
            .into_iter()
            .map(|path| path.trim_matches('/').to_string())
            .filter(|path| !path.is_empty())
            .collect();
        let depth = paths
            .iter()
            .map(|path| path.split('/').count())
            .max()
            .unwrap_or(0);
        Self { paths, depth }
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// True when `relative` is an ignored path or sits inside one. `relative`
    /// is root-relative with forward slashes - the form
    /// [`crate::search::relative_to`] produces and git speaks.
    pub fn contains(&self, relative: &str) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        let candidate = relative.trim_matches('/');
        if candidate.is_empty() {
            return false;
        }
        // Downwards from the shortest prefix: `node_modules/react/index.js` is
        // ignored because `node_modules` is, and the first segment is usually
        // the one that answers. Nothing deeper than the longest path in the
        // set can match, so a file forty levels down still costs `depth`
        // lookups and not forty.
        let mut end = 0;
        for (segment_index, segment) in candidate.split('/').take(self.depth).enumerate() {
            end += if segment_index == 0 {
                segment.len()
            } else {
                segment.len() + 1
            };
            if self.paths.contains(&candidate[..end]) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_covers_everything_beneath_it() {
        let set = IgnoreSet::new(["node_modules".to_string(), "apps/ui/dist".to_string()]);
        assert!(set.contains("node_modules"));
        assert!(set.contains("node_modules/react/index.js"));
        assert!(set.contains("apps/ui/dist/assets/main.js"));
        assert!(!set.contains("apps/ui/src/main.tsx"));
        // A shared prefix is not containment: `node_modules_old` is its own
        // directory and git said nothing about it.
        assert!(!set.contains("node_modules_old/x"));
    }

    #[test]
    fn an_empty_set_ignores_nothing() {
        let set = IgnoreSet::default();
        assert!(set.is_empty());
        assert!(!set.contains("node_modules/react/index.js"));
    }

    #[test]
    fn trailing_slashes_from_git_do_not_survive() {
        let set = IgnoreSet::new(["target/".to_string(), "/".to_string(), String::new()]);
        assert!(set.contains("target/debug/build"));
        assert!(!set.contains("src/main.rs"));
    }
}
