//! Edge CRUD tests for RocksDB backend.
//!
//! Tests for store_edge, get_edge, update_edge, delete_edge.
//! All tests use REAL data - no mocks per constitution requirements.

use tempfile::TempDir;

use super::core::RocksDbMemex;
use super::error::StorageError;
use super::helpers::format_edge_key;
use crate::column_families::cf_names;
use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use context_graph_core::types::GraphEdge;

// =========================================================================
// Helper Functions
// =========================================================================

fn create_temp_db() -> (TempDir, RocksDbMemex) {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let db = RocksDbMemex::open(tmp.path()).expect("Failed to open database");
    (tmp, db)
}

/// Create a REAL GraphEdge with all 13 fields populated.
fn create_test_edge() -> GraphEdge {
    GraphEdge::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::Code,
    )
}

/// Create a REAL GraphEdge between specific nodes.
pub(crate) fn create_test_edge_between(
    source: uuid::Uuid,
    target: uuid::Uuid,
    edge_type: EdgeType,
) -> GraphEdge {
    GraphEdge::new(source, target, edge_type, Domain::Code)
}

// =========================================================================
// store_edge Tests
// =========================================================================

#[test]
fn test_edge_crud_store_edge_basic() {
    println!("=== TEST: store_edge basic operation ===");
    let (_tmp, db) = create_temp_db();
    let edge = create_test_edge();

    let result = db.store_edge(&edge);
    assert!(result.is_ok(), "store_edge should succeed");
}

#[test]
fn test_edge_crud_store_and_get_roundtrip() {
    println!("=== TEST: store_edge + get_edge roundtrip ===");
    let (_tmp, db) = create_temp_db();
    let edge = create_test_edge();

    db.store_edge(&edge).expect("store failed");
    let retrieved = db
        .get_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .expect("get failed");

    // Verify ALL 13 fields preserved
    assert_eq!(edge.id, retrieved.id);
    assert_eq!(edge.source_id, retrieved.source_id);
    assert_eq!(edge.target_id, retrieved.target_id);
    assert_eq!(edge.edge_type, retrieved.edge_type);
    assert_eq!(edge.weight, retrieved.weight);
    assert_eq!(edge.confidence, retrieved.confidence);
    assert_eq!(edge.domain, retrieved.domain);
    assert_eq!(
        edge.neurotransmitter_weights,
        retrieved.neurotransmitter_weights
    );
    assert_eq!(edge.is_amortized_shortcut, retrieved.is_amortized_shortcut);
    assert_eq!(edge.steering_reward, retrieved.steering_reward);
    assert_eq!(edge.traversal_count, retrieved.traversal_count);
    assert_eq!(edge.created_at, retrieved.created_at);
    assert_eq!(edge.last_traversed_at, retrieved.last_traversed_at);
}

#[test]
fn test_edge_crud_store_with_marblestone_fields() {
    println!("=== TEST: store_edge preserves Marblestone fields ===");
    let (_tmp, db) = create_temp_db();

    let mut edge = create_test_edge();
    edge.weight = 0.85;
    edge.confidence = 0.95;
    edge.is_amortized_shortcut = true;
    edge.steering_reward = 0.75;
    edge.traversal_count = 42;
    edge.neurotransmitter_weights = NeurotransmitterWeights::for_domain(Domain::Medical);
    edge.record_traversal();

    db.store_edge(&edge).expect("store failed");
    let retrieved = db
        .get_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .expect("get failed");

    assert_eq!(edge.is_amortized_shortcut, retrieved.is_amortized_shortcut);
    assert_eq!(edge.steering_reward, retrieved.steering_reward);
    assert_eq!(edge.traversal_count, retrieved.traversal_count);
    assert!(retrieved.last_traversed_at.is_some());
}

// =========================================================================
// get_edge Tests
// =========================================================================

#[test]
fn test_edge_crud_get_edge_not_found() {
    let (_tmp, db) = create_temp_db();
    let fake_source = uuid::Uuid::new_v4();
    let fake_target = uuid::Uuid::new_v4();

    let result = db.get_edge(&fake_source, &fake_target, EdgeType::Semantic);

    assert!(result.is_err());
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

// =========================================================================
// update_edge Tests
// =========================================================================

#[test]
fn test_edge_crud_update_edge() {
    println!("=== TEST: update_edge ===");
    let (_tmp, db) = create_temp_db();
    let mut edge = create_test_edge();

    db.store_edge(&edge).expect("store failed");

    edge.weight = 0.999;
    edge.steering_reward = 0.5;
    db.update_edge(&edge).expect("update failed");

    let retrieved = db
        .get_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .expect("get failed");

    assert_eq!(retrieved.weight, 0.999);
    assert_eq!(retrieved.steering_reward, 0.5);
}

// =========================================================================
// delete_edge Tests
// =========================================================================

#[test]
fn test_edge_crud_delete_edge() {
    let (_tmp, db) = create_temp_db();
    let edge = create_test_edge();

    db.store_edge(&edge).expect("store failed");
    assert!(db
        .get_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .is_ok());

    db.delete_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .expect("delete failed");

    let result = db.get_edge(&edge.source_id, &edge.target_id, edge.edge_type);
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[test]
fn test_edge_crud_delete_idempotent() {
    let (_tmp, db) = create_temp_db();
    let fake_source = uuid::Uuid::new_v4();
    let fake_target = uuid::Uuid::new_v4();

    // Delete should succeed even for non-existent edge
    let result = db.delete_edge(&fake_source, &fake_target, EdgeType::Semantic);
    assert!(result.is_ok(), "Delete should be idempotent");
}

// =========================================================================
// Evidence Tests
// =========================================================================

#[test]
fn test_evidence_edge_exists_in_rocksdb() {
    let (_tmp, db) = create_temp_db();
    let edge = create_test_edge();

    db.store_edge(&edge).expect("store failed");

    let cf_edges = db.get_cf(cf_names::EDGES).unwrap();
    let key = format_edge_key(&edge.source_id, &edge.target_id, edge.edge_type);
    let value = db.db().get_cf(cf_edges, &key).expect("direct read failed");

    assert!(value.is_some(), "Edge MUST exist in edges CF");
}

#[test]
fn test_evidence_edge_key_is_33_bytes() {
    let (_tmp, db) = create_temp_db();
    let edge = create_test_edge();

    db.store_edge(&edge).expect("store failed");

    let key = format_edge_key(&edge.source_id, &edge.target_id, edge.edge_type);
    assert_eq!(key.len(), 33, "Edge key must be exactly 33 bytes");

    let cf_edges = db.get_cf(cf_names::EDGES).unwrap();
    let value = db.db().get_cf(cf_edges, &key).unwrap();
    assert!(value.is_some());
}
