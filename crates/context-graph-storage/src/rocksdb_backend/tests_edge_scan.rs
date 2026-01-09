//! Edge scan and edge case tests for RocksDB backend.
//!
//! Tests for get_edges_from, get_edges_to, multiple edge types, and edge cases.
//! All tests use REAL data - no mocks per constitution requirements.

use tempfile::TempDir;

use super::core::RocksDbMemex;
use super::tests_edge::create_test_edge_between;
use context_graph_core::marblestone::{Domain, EdgeType};
use context_graph_core::types::GraphEdge;

// =========================================================================
// Helper Functions
// =========================================================================

fn create_temp_db() -> (TempDir, RocksDbMemex) {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let db = RocksDbMemex::open(tmp.path()).expect("Failed to open database");
    (tmp, db)
}

fn create_test_edge() -> GraphEdge {
    GraphEdge::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::Code,
    )
}

// =========================================================================
// get_edges_from Tests
// =========================================================================

#[test]
fn test_edge_crud_get_edges_from_prefix_scan() {
    println!("=== TEST: get_edges_from prefix scan ===");
    let (_tmp, db) = create_temp_db();
    let source = uuid::Uuid::new_v4();
    let target1 = uuid::Uuid::new_v4();
    let target2 = uuid::Uuid::new_v4();
    let target3 = uuid::Uuid::new_v4();

    let edge1 = create_test_edge_between(source, target1, EdgeType::Semantic);
    let edge2 = create_test_edge_between(source, target2, EdgeType::Causal);
    let edge3 = create_test_edge_between(source, target3, EdgeType::Temporal);

    db.store_edge(&edge1).expect("store1 failed");
    db.store_edge(&edge2).expect("store2 failed");
    db.store_edge(&edge3).expect("store3 failed");

    let edges = db.get_edges_from(&source).expect("get_edges_from failed");

    assert_eq!(edges.len(), 3);
    for edge in &edges {
        assert_eq!(edge.source_id, source);
    }
}

#[test]
fn test_edge_crud_get_edges_from_empty() {
    let (_tmp, db) = create_temp_db();
    let source = uuid::Uuid::new_v4();

    let edges = db.get_edges_from(&source).expect("get_edges_from failed");
    assert!(edges.is_empty());
}

#[test]
fn test_edge_crud_get_edges_from_does_not_include_other_sources() {
    let (_tmp, db) = create_temp_db();
    let source1 = uuid::Uuid::new_v4();
    let source2 = uuid::Uuid::new_v4();
    let target = uuid::Uuid::new_v4();

    let edge1 = create_test_edge_between(source1, target, EdgeType::Semantic);
    let edge2 = create_test_edge_between(source2, target, EdgeType::Semantic);

    db.store_edge(&edge1).expect("store1 failed");
    db.store_edge(&edge2).expect("store2 failed");

    let edges_from_1 = db.get_edges_from(&source1).expect("get failed");
    let edges_from_2 = db.get_edges_from(&source2).expect("get failed");

    assert_eq!(edges_from_1.len(), 1);
    assert_eq!(edges_from_2.len(), 1);
    assert_eq!(edges_from_1[0].source_id, source1);
    assert_eq!(edges_from_2[0].source_id, source2);
}

// =========================================================================
// get_edges_to Tests
// =========================================================================

#[test]
fn test_edge_crud_get_edges_to_full_scan() {
    println!("=== TEST: get_edges_to full scan ===");
    let (_tmp, db) = create_temp_db();
    let source1 = uuid::Uuid::new_v4();
    let source2 = uuid::Uuid::new_v4();
    let source3 = uuid::Uuid::new_v4();
    let target = uuid::Uuid::new_v4();

    let edge1 = create_test_edge_between(source1, target, EdgeType::Semantic);
    let edge2 = create_test_edge_between(source2, target, EdgeType::Causal);
    let edge3 = create_test_edge_between(source3, target, EdgeType::Temporal);

    db.store_edge(&edge1).expect("store1 failed");
    db.store_edge(&edge2).expect("store2 failed");
    db.store_edge(&edge3).expect("store3 failed");

    let edges = db.get_edges_to(&target).expect("get_edges_to failed");

    assert_eq!(edges.len(), 3);
    for edge in &edges {
        assert_eq!(edge.target_id, target);
    }
}

#[test]
fn test_edge_crud_get_edges_to_empty() {
    let (_tmp, db) = create_temp_db();
    let target = uuid::Uuid::new_v4();

    let edges = db.get_edges_to(&target).expect("get_edges_to failed");
    assert!(edges.is_empty());
}

// =========================================================================
// Multiple Edge Types Tests
// =========================================================================

#[test]
fn test_edge_crud_multiple_edge_types_same_nodes() {
    println!("=== TEST: Multiple edge types between same nodes ===");
    let (_tmp, db) = create_temp_db();
    let source = uuid::Uuid::new_v4();
    let target = uuid::Uuid::new_v4();

    for edge_type in EdgeType::all() {
        let edge = create_test_edge_between(source, target, edge_type);
        db.store_edge(&edge).expect("store failed");
    }

    for edge_type in EdgeType::all() {
        let edge = db
            .get_edge(&source, &target, edge_type)
            .expect("get failed");
        assert_eq!(edge.edge_type, edge_type);
    }

    let edges = db.get_edges_from(&source).expect("get_edges_from failed");
    // EdgeType::all() returns 5 types: Semantic, Temporal, Causal, Entailment, Contradiction
    assert_eq!(edges.len(), 5);
}

// =========================================================================
// Edge Case Tests
// =========================================================================

#[test]
fn test_edge_case_extreme_weight_values() {
    println!("=== EDGE CASE: Extreme weight values ===");
    let (_tmp, db) = create_temp_db();

    let mut edge = create_test_edge();
    edge.weight = 0.0;
    edge.confidence = 1.0;
    edge.steering_reward = -1.0;

    db.store_edge(&edge).expect("store failed");
    let retrieved = db
        .get_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .expect("get failed");

    assert_eq!(retrieved.weight, 0.0);
    assert_eq!(retrieved.confidence, 1.0);
    assert_eq!(retrieved.steering_reward, -1.0);
}

#[test]
fn test_edge_case_all_edge_types() {
    let (_tmp, db) = create_temp_db();

    for edge_type in EdgeType::all() {
        let edge = GraphEdge::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            edge_type,
            Domain::General,
        );
        db.store_edge(&edge).expect("store failed");
        let retrieved = db
            .get_edge(&edge.source_id, &edge.target_id, edge_type)
            .expect("get failed");
        assert_eq!(retrieved.edge_type, edge_type);
    }
}

#[test]
fn test_edge_case_all_domain_types() {
    let (_tmp, db) = create_temp_db();

    for domain in Domain::all() {
        let edge = GraphEdge::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            EdgeType::Semantic,
            domain,
        );
        db.store_edge(&edge).expect("store failed");
        let retrieved = db
            .get_edge(&edge.source_id, &edge.target_id, EdgeType::Semantic)
            .expect("get failed");
        assert_eq!(retrieved.domain, domain);
    }
}

// =========================================================================
// Performance Sanity Tests
// =========================================================================

#[test]
fn test_edge_crud_performance_sanity() {
    println!("=== PERFORMANCE: Edge CRUD timing ===");
    let (_tmp, db) = create_temp_db();
    let edge = create_test_edge();

    // Warm up
    db.store_edge(&edge).unwrap();
    let _ = db
        .get_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .unwrap();
    db.delete_edge(&edge.source_id, &edge.target_id, edge.edge_type)
        .unwrap();

    let edge2 = create_test_edge();
    let start = std::time::Instant::now();
    db.store_edge(&edge2).unwrap();
    let store_time = start.elapsed();

    let start = std::time::Instant::now();
    let _ = db
        .get_edge(&edge2.source_id, &edge2.target_id, edge2.edge_type)
        .unwrap();
    let get_time = start.elapsed();

    println!("  store_edge: {:?}", store_time);
    println!("  get_edge: {:?}", get_time);

    assert!(store_time.as_millis() < 100);
    assert!(get_time.as_millis() < 100);
}
