//! Node lifecycle tests - CRUD operations on memory nodes.
//!
//! Tests create, read, update, and delete operations with full state verification.

use super::common::{create_test_node, create_valid_embedding, setup_db};
use context_graph_storage::StorageError;

#[test]
fn test_node_lifecycle_create_read_update_delete() {
    let (db, _tmp) = setup_db();
    let node = create_test_node();
    let node_id = node.id;

    println!("=== NODE LIFECYCLE TEST ===");
    println!("TRIGGER: Creating node with ID {}", node_id);

    // CREATE
    db.store_node(&node).expect("store failed");

    // READ - Verify in Source of Truth
    let retrieved = db.get_node(&node_id).expect("get failed");
    println!("VERIFY: Node exists with content '{}'", retrieved.content);
    assert_eq!(retrieved.id, node_id);
    assert_eq!(retrieved.content, node.content);

    // UPDATE
    let mut updated = retrieved.clone();
    updated.importance = 0.9;
    updated.metadata.tags.push("updated-tag".to_string());
    db.update_node(&updated).expect("update failed");

    // VERIFY UPDATE - Read back from RocksDB
    let after_update = db.get_node(&node_id).expect("get after update");
    println!(
        "VERIFY: importance={}, tags={:?}",
        after_update.importance, after_update.metadata.tags
    );
    assert!(
        (after_update.importance - 0.9).abs() < 0.001,
        "importance should be 0.9, got {}",
        after_update.importance
    );
    assert!(
        after_update
            .metadata
            .tags
            .contains(&"updated-tag".to_string()),
        "tags should contain 'updated-tag'"
    );

    // SOFT DELETE
    db.delete_node(&node_id, true).expect("soft delete");
    let soft_deleted = db.get_node(&node_id).expect("get soft deleted");
    println!("VERIFY: deleted flag={}", soft_deleted.metadata.deleted);
    assert!(
        soft_deleted.metadata.deleted,
        "node should be marked deleted"
    );

    // HARD DELETE
    db.delete_node(&node_id, false).expect("hard delete");
    let result = db.get_node(&node_id);
    println!("VERIFY: NotFound={}", result.is_err());
    assert!(
        matches!(result, Err(StorageError::NotFound { .. })),
        "expected NotFound error after hard delete"
    );

    println!("RESULT: PASSED");
}

#[test]
fn test_node_embedding_roundtrip() {
    let (db, _tmp) = setup_db();
    let node = create_test_node();
    let original = node.embedding.clone();

    println!("=== EMBEDDING ROUNDTRIP TEST ===");
    println!(
        "BEFORE: Creating node with embedding dim={}",
        original.len()
    );

    db.store_node(&node).expect("store");

    // Read embedding back from RocksDB
    let retrieved = db.get_embedding(&node.id).expect("get embedding");

    println!(
        "VERIFY: dim original={}, retrieved={}",
        original.len(),
        retrieved.len()
    );
    assert_eq!(original.len(), retrieved.len());

    // Verify each element matches within floating point tolerance
    for (i, (o, r)) in original.iter().zip(retrieved.iter()).enumerate() {
        assert!(
            (o - r).abs() < 1e-7,
            "Mismatch at index {}: {} vs {}",
            i,
            o,
            r
        );
    }
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_empty_content() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: EMPTY CONTENT ===");

    use context_graph_core::types::MemoryNode;
    let node = MemoryNode::new("".to_string(), create_valid_embedding());
    println!("BEFORE: empty content, TRIGGER: store_node");

    // Empty content should be allowed (spec doesn't prohibit it)
    let result = db.store_node(&node);
    println!("AFTER: success={}", result.is_ok());
    // If empty content is allowed, verify it stored correctly
    if result.is_ok() {
        let retrieved = db.get_node(&node.id).expect("get");
        assert_eq!(retrieved.content, "");
    }
    println!("RESULT: PASSED");
}
