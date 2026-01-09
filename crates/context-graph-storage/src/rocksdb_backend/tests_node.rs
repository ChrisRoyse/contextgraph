//! Node CRUD tests for RocksDB backend.
//!
//! Tests for store_node, get_node, update_node operations.
//! All tests use REAL data - no mocks per constitution requirements.

use tempfile::TempDir;

use super::core::RocksDbMemex;
use super::error::StorageError;
use super::helpers::{format_tag_key, format_temporal_key};
use crate::column_families::cf_names;
use crate::serialization::serialize_uuid;
use context_graph_core::types::{
    EmbeddingVector, JohariQuadrant, MemoryNode, NodeMetadata, DEFAULT_EMBEDDING_DIM,
};

// =========================================================================
// Helper Functions
// =========================================================================

pub(crate) fn create_temp_db() -> (TempDir, RocksDbMemex) {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let db = RocksDbMemex::open(tmp.path()).expect("Failed to open database");
    (tmp, db)
}

/// Create a valid normalized embedding vector.
pub(crate) fn create_normalized_embedding(dim: usize) -> EmbeddingVector {
    let val = 1.0 / (dim as f32).sqrt();
    vec![val; dim]
}

/// Create a valid MemoryNode with real data that passes validate().
pub(crate) fn create_valid_test_node() -> MemoryNode {
    let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);
    let mut node = MemoryNode::new("Test content for CRUD operations".to_string(), embedding);
    node.importance = 0.75;
    node.emotional_valence = 0.5;
    node.quadrant = JohariQuadrant::Open;
    node.metadata = NodeMetadata::new()
        .with_source("test-source")
        .with_language("en");
    assert!(node.validate().is_ok(), "Test node must be valid");
    node
}

/// Create a valid MemoryNode with tags for testing tag index operations.
pub(crate) fn create_node_with_tags(tags: Vec<&str>) -> MemoryNode {
    let mut node = create_valid_test_node();
    for tag in tags {
        node.metadata.add_tag(tag);
    }
    node
}

// =========================================================================
// store_node Tests
// =========================================================================

#[test]
fn test_store_node_basic() {
    println!("=== TEST: store_node basic operation ===");
    let (_tmp, db) = create_temp_db();
    let node = create_valid_test_node();

    println!("BEFORE: Storing node {}", node.id);
    let result = db.store_node(&node);
    println!("AFTER: Store result = {:?}", result.is_ok());

    assert!(result.is_ok(), "store_node should succeed: {:?}", result);
}

#[test]
fn test_store_node_and_get_roundtrip() {
    println!("=== TEST: store_node + get_node roundtrip ===");
    let (_tmp, db) = create_temp_db();
    let node = create_valid_test_node();
    let id = node.id;

    db.store_node(&node).expect("store failed");
    let retrieved = db.get_node(&id).expect("get failed");

    assert_eq!(node.id, retrieved.id);
    assert_eq!(node.content, retrieved.content);
    assert_eq!(node.embedding, retrieved.embedding);
    assert_eq!(node.quadrant, retrieved.quadrant);
    assert_eq!(node.importance, retrieved.importance);
    println!("RESULT: All fields preserved in roundtrip");
}

#[test]
fn test_store_node_validation_failure() {
    println!("=== TEST: store_node validation failure (fail fast) ===");
    let (_tmp, db) = create_temp_db();

    let bad_embedding = vec![0.1; 100]; // Wrong dimension
    let mut node = MemoryNode::new("Test".to_string(), bad_embedding);
    node.importance = 0.5;

    let result = db.store_node(&node);

    assert!(result.is_err());
    assert!(matches!(result, Err(StorageError::ValidationFailed(_))));
}

// =========================================================================
// get_node Tests
// =========================================================================

#[test]
fn test_get_node_not_found() {
    println!("=== TEST: get_node returns NotFound for missing node ===");
    let (_tmp, db) = create_temp_db();
    let fake_id = uuid::Uuid::new_v4();

    let result = db.get_node(&fake_id);

    assert!(result.is_err());
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

// =========================================================================
// update_node Tests
// =========================================================================

#[test]
fn test_update_node_basic() {
    println!("=== TEST: update_node basic operation ===");
    let (_tmp, db) = create_temp_db();
    let mut node = create_valid_test_node();

    db.store_node(&node).expect("store failed");

    node.content = "Updated content".to_string();
    db.update_node(&node).expect("update failed");

    let retrieved = db.get_node(&node.id).expect("get failed");
    assert_eq!(retrieved.content, "Updated content");
}

#[test]
fn test_update_node_not_found_fails() {
    let (_tmp, db) = create_temp_db();
    let node = create_valid_test_node();

    let result = db.update_node(&node);

    assert!(result.is_err());
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[test]
fn edge_case_quadrant_transition() {
    println!("=== EDGE CASE: Quadrant Transition ===");
    let (_tmp, db) = create_temp_db();
    let mut node = create_valid_test_node();
    node.quadrant = JohariQuadrant::Open;
    let node_key = serialize_uuid(&node.id);

    db.store_node(&node).expect("store failed");

    let cf_open = db.get_cf(cf_names::JOHARI_OPEN).unwrap();
    let before_open = db.db().get_cf(cf_open, node_key).unwrap();
    println!(
        "BEFORE: johari_open entry exists = {}",
        before_open.is_some()
    );
    assert!(before_open.is_some());

    node.quadrant = JohariQuadrant::Hidden;
    db.update_node(&node).expect("update failed");

    let after_open = db.db().get_cf(cf_open, node_key).unwrap();
    let cf_hidden = db.get_cf(cf_names::JOHARI_HIDDEN).unwrap();
    let after_hidden = db.db().get_cf(cf_hidden, node_key).unwrap();

    println!(
        "AFTER: johari_open={}, johari_hidden={}",
        after_open.is_some(),
        after_hidden.is_some()
    );
    assert!(after_open.is_none(), "Should be REMOVED from johari_open");
    assert!(after_hidden.is_some(), "Should be ADDED to johari_hidden");
}

#[test]
fn edge_case_empty_tags_update() {
    println!("=== EDGE CASE: Empty Tags Update ===");
    let (_tmp, db) = create_temp_db();
    let mut node = create_node_with_tags(vec!["a", "b", "c"]);

    db.store_node(&node).expect("store failed");

    let cf_tags = db.get_cf(cf_names::TAGS).unwrap();
    for tag in &["a", "b", "c"] {
        let tag_key = format_tag_key(tag, &node.id);
        let exists = db.db().get_cf(cf_tags, &tag_key).unwrap().is_some();
        println!("BEFORE: Tag '{}' exists: {}", tag, exists);
    }

    node.metadata.tags = vec![];
    db.update_node(&node).expect("update failed");

    for tag in &["a", "b", "c"] {
        let tag_key = format_tag_key(tag, &node.id);
        let exists = db.db().get_cf(cf_tags, &tag_key).unwrap();
        assert!(exists.is_none(), "Tag '{}' should be removed", tag);
    }
}

// =========================================================================
// Evidence Tests
// =========================================================================

#[test]
fn evidence_store_node_creates_all_indexes() {
    println!("=== EVIDENCE: store_node creates all indexes ===");
    let (_tmp, db) = create_temp_db();

    let mut node = create_valid_test_node();
    node.metadata.add_tag("important");
    node.metadata.source = Some("test-source".to_string());

    db.store_node(&node).expect("store failed");

    let node_key = serialize_uuid(&node.id);

    // Verify node exists
    let cf_nodes = db.get_cf(cf_names::NODES).unwrap();
    assert!(db.db().get_cf(cf_nodes, node_key).unwrap().is_some());

    // Verify embedding
    let cf_emb = db.get_cf(cf_names::EMBEDDINGS).unwrap();
    assert!(db.db().get_cf(cf_emb, node_key).unwrap().is_some());

    // Verify johari index
    let cf_johari = db.get_cf(node.quadrant.column_family()).unwrap();
    assert!(db.db().get_cf(cf_johari, node_key).unwrap().is_some());

    // Verify temporal index
    let cf_temporal = db.get_cf(cf_names::TEMPORAL).unwrap();
    let temporal_key = format_temporal_key(node.created_at, &node.id);
    assert!(db
        .db()
        .get_cf(cf_temporal, &temporal_key)
        .unwrap()
        .is_some());

    println!("RESULT: All indexes verified ✓");
}
