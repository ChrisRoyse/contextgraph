//! Edge CRUD operations and graph traversal tests.
//!
//! Tests edge update, delete, and from/to queries.

use super::common::{create_test_edge, create_test_node, setup_db};
use context_graph_core::marblestone::{Domain, EdgeType};
use context_graph_core::types::GraphEdge;
use context_graph_storage::StorageError;

#[test]
fn test_edge_update() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");

    let mut edge = create_test_edge(node1.id, node2.id);
    db.store_edge(&edge).expect("store edge");

    println!("=== EDGE UPDATE TEST ===");
    println!(
        "BEFORE: weight={}, confidence={}",
        edge.weight, edge.confidence
    );

    // Update edge
    edge.confidence = 0.9;
    edge.apply_steering_reward(0.3);
    db.update_edge(&edge).expect("update edge");

    // Verify update in RocksDB
    let retrieved = db
        .get_edge(&node1.id, &node2.id, EdgeType::Semantic)
        .expect("get");

    println!(
        "AFTER: confidence={}, steering_reward={}",
        retrieved.confidence, retrieved.steering_reward
    );
    assert!(
        (retrieved.confidence - 0.9).abs() < 0.001,
        "confidence should be 0.9"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_edge_delete() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");

    let edge = create_test_edge(node1.id, node2.id);
    db.store_edge(&edge).expect("store edge");

    println!("=== EDGE DELETE TEST ===");

    // Verify edge exists
    assert!(
        db.get_edge(&node1.id, &node2.id, EdgeType::Semantic)
            .is_ok(),
        "edge should exist"
    );

    // Delete edge
    db.delete_edge(&node1.id, &node2.id, EdgeType::Semantic)
        .expect("delete edge");

    // Verify edge is gone
    let result = db.get_edge(&node1.id, &node2.id, EdgeType::Semantic);
    println!("VERIFY: NotFound after delete={}", result.is_err());
    assert!(
        matches!(result, Err(StorageError::NotFound { .. })),
        "edge should not exist after delete"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_get_edges_from_and_to() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    let node3 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");
    db.store_node(&node3).expect("store node3");

    // Create edges: node1 -> node2, node1 -> node3, node3 -> node2
    let edge1 = GraphEdge::new(node1.id, node2.id, EdgeType::Semantic, Domain::General);
    let edge2 = GraphEdge::new(node1.id, node3.id, EdgeType::Causal, Domain::Code);
    let edge3 = GraphEdge::new(node3.id, node2.id, EdgeType::Temporal, Domain::Research);

    db.store_edge(&edge1).expect("store edge1");
    db.store_edge(&edge2).expect("store edge2");
    db.store_edge(&edge3).expect("store edge3");

    println!("=== GET EDGES FROM/TO TEST ===");

    // Get edges from node1
    let from_node1 = db.get_edges_from(&node1.id).expect("get edges from node1");
    println!("VERIFY: edges from node1 = {}", from_node1.len());
    assert_eq!(from_node1.len(), 2, "node1 should have 2 outgoing edges");

    // Get edges to node2
    let to_node2 = db.get_edges_to(&node2.id).expect("get edges to node2");
    println!("VERIFY: edges to node2 = {}", to_node2.len());
    assert_eq!(to_node2.len(), 2, "node2 should have 2 incoming edges");

    println!("RESULT: PASSED");
}

#[test]
fn test_edge_not_found() {
    let (db, _tmp) = setup_db();
    let fake_source = uuid::Uuid::new_v4();
    let fake_target = uuid::Uuid::new_v4();

    println!("=== EDGE NOT FOUND TEST ===");
    println!(
        "TRIGGER: Get non-existent edge {} -> {}",
        fake_source, fake_target
    );

    let result = db.get_edge(&fake_source, &fake_target, EdgeType::Semantic);
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
