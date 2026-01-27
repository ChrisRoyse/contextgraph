//! Validation tests for single embedder search.
//!
//! Tests FAIL FAST behavior on invalid inputs.

use std::sync::Arc;

use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexRegistry};
use crate::teleological::search::error::SearchError;

use crate::teleological::search::single::search::SingleEmbedderSearch;

fn create_test_search() -> SingleEmbedderSearch {
    let registry = Arc::new(EmbedderIndexRegistry::new());
    SingleEmbedderSearch::new(registry)
}

// ========== FAIL FAST VALIDATION TESTS ==========

#[test]
fn test_unsupported_embedder_e6() {
    println!("=== TEST: E6Sparse returns UnsupportedEmbedder error ===");
    println!("BEFORE: Attempting search on E6Sparse");

    let search = create_test_search();
    let query = vec![1.0f32; 100]; // Dimension doesn't matter

    let result = search.search(EmbedderIndex::E6Sparse, &query, 10, None);

    println!("AFTER: result = {:?}", result);
    assert!(result.is_err());

    match result.unwrap_err() {
        SearchError::UnsupportedEmbedder { embedder } => {
            assert_eq!(embedder, EmbedderIndex::E6Sparse);
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_unsupported_embedder_e12() {
    println!("=== TEST: E12LateInteraction returns UnsupportedEmbedder error ===");

    let search = create_test_search();
    let query = vec![1.0f32; 128];

    let result = search.search(EmbedderIndex::E12LateInteraction, &query, 10, None);

    match result.unwrap_err() {
        SearchError::UnsupportedEmbedder { embedder } => {
            assert_eq!(embedder, EmbedderIndex::E12LateInteraction);
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_unsupported_embedder_e13() {
    println!("=== TEST: E13Splade returns UnsupportedEmbedder error ===");

    let search = create_test_search();
    let query = vec![1.0f32; 100];

    let result = search.search(EmbedderIndex::E13Splade, &query, 10, None);

    match result.unwrap_err() {
        SearchError::UnsupportedEmbedder { embedder } => {
            assert_eq!(embedder, EmbedderIndex::E13Splade);
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_dimension_mismatch() {
    println!("=== TEST: Wrong dimension returns DimensionMismatch error ===");
    println!("BEFORE: E1Semantic expects 1024D, providing 512D");

    let search = create_test_search();
    let query = vec![1.0f32; 512]; // Wrong: E1 expects 1024

    let result = search.search(EmbedderIndex::E1Semantic, &query, 10, None);

    println!("AFTER: result = {:?}", result);
    assert!(result.is_err());

    match result.unwrap_err() {
        SearchError::DimensionMismatch {
            embedder,
            expected,
            actual,
        } => {
            assert_eq!(embedder, EmbedderIndex::E1Semantic);
            assert_eq!(expected, 1024);
            assert_eq!(actual, 512);
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_empty_query() {
    println!("=== TEST: Empty query returns EmptyQuery error ===");

    let search = create_test_search();
    let query: Vec<f32> = vec![];

    let result = search.search(EmbedderIndex::E1Semantic, &query, 10, None);

    match result.unwrap_err() {
        SearchError::EmptyQuery { embedder } => {
            assert_eq!(embedder, EmbedderIndex::E1Semantic);
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_nan_in_query() {
    println!("=== TEST: NaN in query returns InvalidVector error ===");

    let search = create_test_search();
    let mut query = vec![1.0f32; 1024];
    query[100] = f32::NAN;

    let result = search.search(EmbedderIndex::E8Graph, &query, 10, None);

    match result.unwrap_err() {
        SearchError::InvalidVector { embedder, message } => {
            assert_eq!(embedder, EmbedderIndex::E8Graph);
            assert!(message.contains("Non-finite"));
            assert!(message.contains("100"));
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_infinity_in_query() {
    println!("=== TEST: Infinity in query returns InvalidVector error ===");

    let search = create_test_search();
    let mut query = vec![1.0f32; 1024];
    query[0] = f32::INFINITY;

    let result = search.search(EmbedderIndex::E8Graph, &query, 10, None);

    match result.unwrap_err() {
        SearchError::InvalidVector { embedder, message } => {
            assert_eq!(embedder, EmbedderIndex::E8Graph);
            assert!(message.contains("Non-finite"));
            assert!(message.contains("inf"));
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}

#[test]
fn test_neg_infinity_in_query() {
    println!("=== TEST: Negative infinity in query returns InvalidVector error ===");

    let search = create_test_search();
    let mut query = vec![1.0f32; 1024];
    query[50] = f32::NEG_INFINITY;

    let result = search.search(EmbedderIndex::E8Graph, &query, 10, None);

    match result.unwrap_err() {
        SearchError::InvalidVector { embedder, .. } => {
            assert_eq!(embedder, EmbedderIndex::E8Graph);
        }
        e => panic!("Wrong error type: {:?}", e),
    }

    println!("RESULT: PASS");
}
