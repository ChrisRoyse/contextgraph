//! Tests for metrics and statistics methods.

use crate::index::search_result::SearchResult;

#[test]
fn test_total_valid_results_multiple_queries() {
    let result = SearchResult::new(
        vec![1, 2, -1, 4, -1, 6], // 4 valid across 2 queries
        vec![0.1, 0.2, 0.0, 0.4, 0.0, 0.6],
        3,
        2,
    );

    assert_eq!(result.total_valid_results(), 4);
    assert_eq!(result.num_valid_results(0), 2); // 1, 2
    assert_eq!(result.num_valid_results(1), 2); // 4, 6
}

#[test]
fn test_is_empty() {
    let empty_result = SearchResult::new(vec![-1, -1, -1], vec![0.0, 0.0, 0.0], 3, 1);
    assert!(empty_result.is_empty());

    let non_empty = SearchResult::new(vec![1, -1, -1], vec![0.1, 0.0, 0.0], 3, 1);
    assert!(!non_empty.is_empty());
}

// ========== Min/Max Distance ==========

#[test]
fn test_min_distance() {
    let result = SearchResult::new(vec![1, 2, 3], vec![0.5, 0.1, 0.8], 3, 1);

    assert_eq!(result.min_distance(), Some(0.1));
}

#[test]
fn test_max_distance() {
    let result = SearchResult::new(vec![1, 2, 3], vec![0.5, 0.1, 0.8], 3, 1);

    assert_eq!(result.max_distance(), Some(0.8));
}

#[test]
fn test_min_max_ignores_sentinels() {
    let result = SearchResult::new(
        vec![1, -1, 3],
        vec![0.5, 999.0, 0.8], // 999.0 should be ignored
        3,
        1,
    );

    assert_eq!(result.min_distance(), Some(0.5));
    assert_eq!(result.max_distance(), Some(0.8));
}

#[test]
fn test_min_max_empty_result() {
    let result = SearchResult::new(vec![-1, -1, -1], vec![0.0, 0.0, 0.0], 3, 1);

    assert!(result.min_distance().is_none());
    assert!(result.max_distance().is_none());
}
