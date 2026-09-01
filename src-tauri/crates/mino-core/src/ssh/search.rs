//! The remote recursive walk behind `Transport::search_files`.
//!
//! Breadth-first over SFTP, mirroring `local::search` level for level so a
//! search returns the same ranked answer whichever end of the connection the
//! files are on.
//!
//! SFTP rather than a remote `find`: it needs nothing installed on the host,
//! reports real metadata, and - the reason that matters most here - never puts
//! a caller value on a command line. A query typed into the search box is
//! matched in this process and is never sent to the remote shell at all.

use std::collections::VecDeque;

use russh_sftp::client::SftpSession;

use crate::error::Result;
use crate::search::{is_skipped_directory, relative_to, Collector};
use crate::types::{DirEntry, EntryKind, SearchHits, SearchQuery};

use super::fs;
use super::roots::RemoteRoot;

pub async fn search(
    sftp: &SftpSession,
    root: &RemoteRoot,
    query: &SearchQuery,
) -> Result<SearchHits> {
    let mut collector = Collector::new(query);
    let mut queue = VecDeque::from([root.root().to_string()]);

    while let Some(dir) = queue.pop_front() {
        if !collector.should_continue() {
            break;
        }
        for entry in read_children(sftp, root, &dir).await {
            if !collector.should_continue() {
                break;
            }
            let relative = relative_to(root.root(), &entry.path);
            let descend =
                matches!(entry.kind, EntryKind::Directory) && !is_skipped_directory(&entry.name);
            let path = entry.path.clone();
            collector.offer(entry, relative);
            if descend {
                queue.push_back(path);
            }
        }
    }
    Ok(collector.finish())
}

/// One remote directory level, with every failure swallowed: a directory the
/// account cannot read must not fail the whole search, only its own subtree.
///
/// Unlike `fs::list_dir` this does not re-canonicalise each level. The walk
/// starts at the guarded root and only ever moves downwards, and every child
/// is still checked with `RemoteRoot::contains` before it is accepted - so a
/// server that answered with a path outside the root gets ignored rather than
/// followed.
async fn read_children(sftp: &SftpSession, root: &RemoteRoot, dir: &str) -> Vec<DirEntry> {
    let Ok(listing) = sftp.read_dir(dir.to_string()).await else {
        tracing::debug!(path = %dir, "skipping unreadable remote directory during search");
        return Vec::new();
    };

    let mut entries = Vec::new();
    for item in listing {
        let name = item.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let child = if dir.ends_with('/') {
            format!("{dir}{name}")
        } else {
            format!("{dir}/{name}")
        };
        if !root.contains(&child) {
            continue;
        }
        entries.push(fs::entry_for(child, item.metadata()));
    }
    entries
}
