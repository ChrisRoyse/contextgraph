//! Node delete and lifecycle tests for RocksDB backend.
//!
//! Tests for delete_node (soft/hard), edge cases, and full lifecycle.
//! All tests use REAL data - no mocks per constitution requirements.

use super::error::StorageError;
use super::tests_node::{create_temp_db, create_valid_test_node};
use crate::column_families::cf_names;
use crate::serialization::serialize_uuid;

// =========================================================================
// delete_node Tests
// =========================================================================

#[test]
fn test_delete_node_not_found_fails() {
    let (_tmp, db) = create_temp_db();
    let fake_id = uuid::Uuid::new_v4();

    let result = db.delete_node(&fake_id, false);

    assert!(result.is_err());
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[test]
fn test_delete_node_soft_delete() {
    println!("=== TEST: delete_node soft delete (SEC-06) ===");
    let (_tmp, db) = create_temp_db();
    let node = create_valid_test_node();
    let id = node.id;

    db.store_node(&node).expect("store failed");
    db.delete_node(&id, true).expect("soft delete failed");

    let retrieved = db
        .get_node(&id)
        .expect("get should succeed after soft delete");
    assert!(retrieved.metadata.deleted);
    assert!(retrieved.metadata.deleted_at.is_some());
}

#[test]
fn test_delete_node_hard_delete() {
    println!("=== TEST: delete_node hard delete ===");
    let (_tmp, db) = create_temp_db();
    let mut node = create_valid_test_node();
    node.metadata.add_tag("test-tag");
    node.metadata.source = Some("test-source".to_string());
    let id = node.id;
    let node_key = serialize_uuid(&id);

    db.store_node(&node).expect("store failed");

    let cf_nodes = db.get_cf(cf_names::NODES).unwrap();
    assert!(db.db().get_cf(cf_nodes, node_key).unwrap().is_some());

    db.delete_node(&id, false).expect("hard delete failed");

    assert!(db.db().get_cf(cf_nodes, node_key).unwrap().is_none());
    let result = db.get_node(&id);
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

// =========================================================================
// Edge Case Tests
// =========================================================================

#[test]
fn edge_case_unicode_content() {
    println!("=== EDGE CASE: Unicode Content ===");
    let (_tmp, db) = create_temp_db();

    let mut node = create_valid_test_node();
    node.content = "日本語 🎉 émojis λ α β γ δ ε ζ".to_string();

    db.store_node(&node).expect("store failed");
    let retrieved = db.get_node(&node.id).expect("get failed");

    assert_eq!(node.content, retrieved.content);
    println!("RESULT: Unicode content preserved");
}

// =========================================================================
// Performance Sanity Tests
// =========================================================================

#[test]
fn test_store_get_performance_sanity() {
    println!("=== PERFORMANCE: store_node + get_node timing ===");
    let (_tmp, db) = create_temp_db();
    let node = create_valid_test_node();

    // Warm up
    db.store_node(&node).unwrap();
    db.get_node(&node.id).unwrap();
    db.delete_node(&node.id, false).unwrap();

    // Time store
    let node2 = create_valid_test_node();
    let start = std::time::Instant::now();
    db.store_node(&node2).unwrap();
    let store_time = start.elapsed();

    // Time get
    let start = std::time::Instant::now();
    let _ = db.get_node(&node2.id).unwrap();
    let get_time = start.elapsed();

    println!("  store_node: {:?}", store_time);
    println!("  get_node: {:?}", get_time);

    assert!(store_time.as_millis() < 100);
    assert!(get_time.as_millis() < 100);
}

// =========================================================================
// Full Lifecycle Test
// =========================================================================

#[test]
fn test_full_lifecycle() {
    println!("=== TEST: Full node lifecycle ===");
    let (_tmp, db) = create_temp_db();
    let mut node = create_valid_test_node();
    let id = node.id;

    // Create
    db.store_node(&node).expect("store failed");
    let stored = db.get_node(&id).expect("get failed");
    assert_eq!(stored.content, node.content);

    // Update
    node.content = "Updated content".to_string();
    db.update_node(&node).expect("update failed");
    let updated = db.get_node(&id).expect("get failed");
    assert_eq!(updated.content, "Updated content");

    // Soft delete
    db.delete_node(&id, true).expect("soft delete failed");
    let soft_deleted = db.get_node(&id).expect("get failed");
    assert!(soft_deleted.metadata.deleted);

    println!("RESULT: Full lifecycle completed ✓");
}
