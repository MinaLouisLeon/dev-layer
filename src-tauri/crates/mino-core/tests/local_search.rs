//! The recursive filename search, over a real temporary tree.
//!
//! What is asserted here is the behaviour the search pane depends on: that the
//! walk descends, that it matches on a subsequence rather than a substring,
//! and that it ranks. The limits it must respect - the skip list, the result
//! ceiling, the path guard - are in `local_search_guards.rs`.

use mino_core::types::SearchQuery;
use mino_core::{LocalTransport, Transport, TransportError};

mod fixture;
use fixture::connected;

#[tokio::test]
async fn searching_before_connect_is_typed_not_connected() {
    let transport = LocalTransport::new();
    let err = transport
        .search_files(SearchQuery::new("main"))
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::NotConnected));
}

#[tokio::test]
async fn finds_a_file_nested_below_the_root() {
    let (_dir, transport) = connected().await;
    let found = transport
        .search_files(SearchQuery::new("treepane"))
        .await
        .expect("search");
    assert_eq!(
        found.hits.first().map(|h| h.relative_path.as_str()),
        Some("src/features/file-tree/TreePane.tsx")
    );
}

#[tokio::test]
async fn matches_are_a_subsequence_not_a_substring() {
    let (_dir, transport) = connected().await;
    // Initials only - the letters of "TreePane.tsx" in order, nothing more.
    let found = transport
        .search_files(SearchQuery::new("tpx"))
        .await
        .expect("search");
    assert!(found
        .hits
        .iter()
        .any(|h| h.relative_path.ends_with("TreePane.tsx")));
}

#[tokio::test]
async fn the_closer_match_ranks_first() {
    let (_dir, transport) = connected().await;
    let found = transport
        .search_files(SearchQuery::new("main"))
        .await
        .expect("search");
    assert_eq!(
        found.hits.first().map(|h| h.relative_path.as_str()),
        Some("src/main.rs")
    );
}

#[tokio::test]
async fn an_empty_query_lists_the_tree_without_ranking_it() {
    let (_dir, transport) = connected().await;
    let found = transport
        .search_files(SearchQuery::new(""))
        .await
        .expect("search");
    assert!(found.hits.iter().all(|h| h.score == 0));
    assert!(found.hits.iter().any(|h| h.relative_path == "readme.md"));
}
