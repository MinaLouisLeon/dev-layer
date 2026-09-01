//! The local recursive walk behind `Transport::search_files`.
//!
//! Breadth-first from the connected root, which means shallow matches - the
//! ones people usually want - are found before the walk goes deep, and a
//! truncated search still returns the most useful part of the tree.
//!
//! Two rules the walk never bends. Symlinked directories are not descended
//! into, because a link pointing at an ancestor turns a walk into an infinite
//! one, and the skip list in `types::search` is applied by directory name at
//! every level. Containment needs no per-entry check: the walk only ever
//! starts at the guarded root and only ever moves downwards, so nothing it
//! reaches can be outside.
//!
//! Inside a repository the walk additionally honours `.gitignore`, by asking
//! git once per search which paths it would not look at. That is an *addition*
//! to the skip list, never a replacement: a folder with no repository, or a
//! machine with no git, searches exactly as it did before, because the ignore
//! set comes back empty and an empty set ignores nothing.

use std::collections::VecDeque;
use std::path::PathBuf;

use crate::error::{Result, TransportError};
use crate::search::{is_skipped_directory, relative_to, Collector, IgnoreSet};
use crate::types::{EntryKind, SearchHits, SearchQuery};

use super::fs;
use super::roots::RootGuard;

/// Runs the walk on a blocking thread. Directory traversal is synchronous
/// filesystem work and a large tree would otherwise stall the async runtime
/// for as long as it takes.
pub async fn search(guard: RootGuard, query: SearchQuery) -> Result<SearchHits> {
    // Asked before the walk moves to a blocking thread, because it is one
    // short async process call and the walk itself must stay synchronous.
    // `ignored` never fails: it answers with an empty list instead.
    let ignores = IgnoreSet::new(super::git_read::ignored(&guard.root_display()).await);
    tokio::task::spawn_blocking(move || walk(&guard, &query, ignores))
        .await
        .map_err(|e| TransportError::io(format!("the search task failed: {e}")))?
}

fn walk(guard: &RootGuard, query: &SearchQuery, ignores: IgnoreSet) -> Result<SearchHits> {
    let mut collector = Collector::new(query).with_ignores(ignores);
    let root = guard.root().to_path_buf();
    let root_display = guard.root_display();

    let mut queue = VecDeque::from([root]);
    while let Some(dir) = queue.pop_front() {
        if !collector.should_continue() {
            break;
        }
        for entry in read_children(&dir) {
            if !collector.should_continue() {
                break;
            }
            // `fs::entry_from` has already put the path through
            // `display_path`, so it is the string the UI will show.
            let path = entry.path.clone();
            let relative = relative_to(&root_display, &path);
            let descend = matches!(entry.kind, EntryKind::Directory)
                && !is_skipped_directory(&entry.name)
                && !collector.is_ignored(&relative);
            collector.offer(entry, relative);
            if descend {
                queue.push_back(PathBuf::from(&path));
            }
        }
    }
    Ok(collector.finish())
}

/// One directory level, with every failure swallowed.
///
/// An unreadable directory in the middle of a tree must not fail the search -
/// the answer for the rest of the tree is still worth having. This differs
/// from `fs::list_dir`, where a directory the user asked for by name failing
/// to open is exactly what they need to be told about.
fn read_children(dir: &std::path::Path) -> Vec<crate::types::DirEntry> {
    let Ok(reader) = std::fs::read_dir(dir) else {
        tracing::debug!(path = %dir.display(), "skipping unreadable directory during search");
        return Vec::new();
    };

    let mut entries = Vec::new();
    for item in reader.flatten() {
        let child = item.path();
        // symlink_metadata, so a link is reported as a link and never
        // followed: the queue below only takes real directories, which is what
        // stops a link to an ancestor becoming an endless walk.
        let Ok(meta) = std::fs::symlink_metadata(&child) else {
            continue;
        };
        entries.push(fs::entry_from(&child, &meta));
    }
    entries
}
