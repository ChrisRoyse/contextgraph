//! Test modules for search_result.

mod item_tests;
mod metrics_tests;
mod query_tests;

use crate::index::search_result::SearchResult;

// ========== SearchResult Basic Tests ==========

#[test]
fn test_new_creates_valid_result() {
    let result = SearchResult::new(
        vec![1, 2, 3, 4, 5, 6],
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
        3,
        2,
    );

    assert_eq!(result.k, 3);
    assert_eq!(result.num_queries, 2);
    assert_eq!(result.ids.len(), 6);
    assert_eq!(result.distances.len(), 6);
}

#[test]
fn test_k_zero() {
    let result = SearchResult::new(Vec::new(), Vec::new(), 0, 0);

    assert!(result.is_empty());
    assert_eq!(result.total_valid_results(), 0);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_single_query_single_result() {
    let result = SearchResult::new(vec![42], vec![0.123], 1, 1);

    let q: Vec<_> = result.query_results(0).collect();
    assert_eq!(q, vec![(42, 0.123)]);
}

#[test]
fn test_default() {
    let result = SearchResult::default();
    assert!(result.is_empty());
    assert_eq!(result.k, 0);
    assert_eq!(result.num_queries, 0);
}

#[test]
fn test_len_and_k() {
    let result = SearchResult::new(
        vec![1, 2, 3, 4, 5, 6],
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
        3,
        2,
    );

    assert_eq!(result.len(), 2); // num_queries
    assert_eq!(result.k(), 3);
}

// ========== Clone and Debug ==========

#[test]
fn test_search_result_clone() {
    let result = SearchResult::new(vec![1, 2, 3], vec![0.1, 0.2, 0.3], 3, 1);
    let cloned = result.clone();

    assert_eq!(cloned.ids, result.ids);
    assert_eq!(cloned.distances, result.distances);
    assert_eq!(cloned.k, result.k);
    assert_eq!(cloned.num_queries, result.num_queries);
}

#[test]
fn test_search_result_debug() {
    let result = SearchResult::new(vec![1], vec![0.1], 1, 1);
    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("SearchResult"));
    assert!(debug_str.contains("ids"));
}
