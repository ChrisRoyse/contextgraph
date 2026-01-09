//! Error handling tests.
//!
//! Tests error conditions including NotFound, validation failures, and edge cases.

use super::common::{create_test_node, setup_db};
use context_graph_storage::StorageError;

#[test]
fn test_not_found_error() {
    let (db, _tmp) = setup_db();
    let fake_id = uuid::Uuid::new_v4();

    println!("=== NOT FOUND ERROR TEST ===");
    println!("TRIGGER: Get non-existent node {}", fake_id);

    let result = db.get_node(&fake_id);
    println!(
        "VERIFY: NotFound={}",
        matches!(&result, Err(StorageError::NotFound { .. }))
    );
    assert!(
        matches!(result, Err(StorageError::NotFound { .. })),
        "should return NotFound error"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_validation_error_wrong_dimension() {
    let (db, _tmp) = setup_db();
    let mut node = create_test_node();
    node.embedding = vec![0.1; 100]; // Wrong dimension (should be 1536)

    println!("=== VALIDATION ERROR TEST (wrong dim) ===");
    println!("TRIGGER: Store with embedding dim={}", node.embedding.len());

    let result = db.store_node(&node);
    println!(
        "VERIFY: ValidationFailed={}",
        matches!(&result, Err(StorageError::ValidationFailed(_)))
    );
    assert!(
        matches!(result, Err(StorageError::ValidationFailed(_))),
        "should return ValidationFailed error"
    );
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_nan_importance() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: NaN IMPORTANCE ===");

    let mut node = create_test_node();
    node.importance = f32::NAN;
    println!("BEFORE: importance=NaN, TRIGGER: store_node");

    let result = db.store_node(&node);
    println!("AFTER: error={}", result.is_err());
    assert!(result.is_err(), "NaN importance should fail validation");
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_infinity_importance() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: INFINITY IMPORTANCE ===");

    let mut node = create_test_node();
    node.importance = f32::INFINITY;
    println!("BEFORE: importance=INFINITY, TRIGGER: store_node");

    let result = db.store_node(&node);
    println!("AFTER: error={}", result.is_err());
    assert!(
        result.is_err(),
        "Infinity importance should fail validation"
    );
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_out_of_bounds_importance() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: OUT OF BOUNDS IMPORTANCE ===");

    let mut node = create_test_node();
    node.importance = 1.5; // Out of [0, 1] range
    println!("BEFORE: importance=1.5 (out of [0,1]), TRIGGER: store_node");

    let result = db.store_node(&node);
    println!("AFTER: error={}", result.is_err());
    assert!(
        result.is_err(),
        "Out-of-range importance should fail validation"
    );
    println!("RESULT: PASSED");
}
