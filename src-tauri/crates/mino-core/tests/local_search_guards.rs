//! What the search walk refuses to do.
//!
//! The behaviour of search is in `local_search.rs`; the limits it must respect
//! are here - the skip list, the result ceiling, and the path guard. These are
//! the ones where a regression is a leak rather than an inconvenience.

use mino_core::types::{ConnectionTarget, SearchQuery};
use mino_core::{LocalTransport, Transport};

mod fixture;
use fixture::connected;

#[tokio::test]
async fn skipped_directories_are_never_walked() {
    let (_dir, transport) = connected().await;
    let found = transport
        .search_files(SearchQuery::new("main.rs"))
        .await
        .expect("search");

    assert!(found.hits.iter().any(|h| h.relative_path == "src/main.rs"));
    assert!(
        !found
            .hits
            .iter()
            .any(|h| h.relative_path.contains("node_modules")),
        "node_modules must be skipped whole, not merely ranked lower"
    );
}

#[tokio::test]
async fn hidden_entries_can_be_excluded() {
    let (_dir, transport) = connected().await;

    let shown = transport
        .search_files(SearchQuery::new("hidden"))
        .await
        .expect("search");
    assert!(shown.hits.iter().any(|h| h.entry.hidden));

    let query = SearchQuery {
        include_hidden: false,
        ..SearchQuery::new("hidden")
    };
    let found = transport.search_files(query).await.expect("search");
    assert!(found.hits.is_empty());
}

#[tokio::test]
async fn directories_can_be_excluded() {
    let (_dir, transport) = connected().await;
    let query = SearchQuery {
        include_directories: false,
        ..SearchQuery::new("src")
    };
    let found = transport.search_files(query).await.expect("search");
    assert!(!found.hits.iter().any(|h| h.entry.is_dir()));
}

#[tokio::test]
async fn the_limit_is_honoured_and_reported_as_truncated() {
    let (_dir, transport) = connected().await;
    let query = SearchQuery {
        limit: Some(1),
        ..SearchQuery::new("")
    };
    let found = transport.search_files(query).await.expect("search");
    assert_eq!(found.hits.len(), 1);
    assert!(found.truncated);
    assert!(found.scanned > 1, "the walk visits more than it returns");
}

/// Nothing outside the root can appear, however the walk got there. The root
/// here is a *subdirectory* of the temp dir, so its siblings are real files
/// that a walk going upwards would otherwise find.
#[tokio::test]
async fn results_never_leave_the_connected_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("inside")).unwrap();
    std::fs::write(dir.path().join("inside/kept.txt"), "x").unwrap();
    std::fs::write(dir.path().join("outside.txt"), "x").unwrap();

    let transport = LocalTransport::new();
    let target = ConnectionTarget::Local {
        root: dir.path().join("inside").to_string_lossy().into_owned(),
    };
    let info = transport.connect(&target).await.expect("connect");

    let found = transport
        .search_files(SearchQuery::new("txt"))
        .await
        .expect("search");
    assert!(found
        .hits
        .iter()
        .all(|h| h.entry.path.starts_with(&info.root)));
    assert!(!found.hits.iter().any(|h| h.entry.name == "outside.txt"));
}
