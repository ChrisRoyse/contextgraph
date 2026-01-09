//! Marblestone edge tests - NT weights, steering, shortcuts, and traversal.
//!
//! Tests the advanced edge features from the Marblestone specification.

use super::common::{create_test_edge, create_test_node, setup_db};
use context_graph_core::marblestone::{Domain, EdgeType};
use context_graph_core::types::GraphEdge;

#[test]
fn test_edge_with_neurotransmitter_weights() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");

    let edge = GraphEdge::new(node1.id, node2.id, EdgeType::Causal, Domain::Code);
    println!("=== NT WEIGHTS TEST ===");
    println!(
        "BEFORE: NT={:?}, domain={:?}",
        edge.neurotransmitter_weights, edge.domain
    );

    db.store_edge(&edge).expect("store edge");

    // Read back from RocksDB
    let retrieved = db
        .get_edge(&node1.id, &node2.id, EdgeType::Causal)
        .expect("get edge");

    println!("AFTER: NT={:?}", retrieved.neurotransmitter_weights);
    assert_eq!(
        retrieved.neurotransmitter_weights.excitatory,
        edge.neurotransmitter_weights.excitatory
    );
    assert_eq!(
        retrieved.neurotransmitter_weights.inhibitory,
        edge.neurotransmitter_weights.inhibitory
    );
    assert_eq!(
        retrieved.neurotransmitter_weights.modulatory,
        edge.neurotransmitter_weights.modulatory
    );
    assert_eq!(retrieved.domain, Domain::Code);
    println!("RESULT: PASSED");
}

#[test]
fn test_edge_steering_reward_persistence() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");

    let mut edge = create_test_edge(node1.id, node2.id);
    println!("=== STEERING REWARD TEST ===");
    println!(
        "BEFORE: steering_reward={}, is_amortized_shortcut={}",
        edge.steering_reward, edge.is_amortized_shortcut
    );

    // Apply steering reward
    edge.apply_steering_reward(0.5);
    println!("AFTER APPLY: steering_reward={}", edge.steering_reward);

    db.store_edge(&edge).expect("store");

    // Read back from RocksDB - the source of truth
    let retrieved = db
        .get_edge(&node1.id, &node2.id, EdgeType::Semantic)
        .expect("get");

    println!(
        "VERIFY: retrieved steering_reward={}",
        retrieved.steering_reward
    );
    assert!(
        (retrieved.steering_reward - edge.steering_reward).abs() < 0.001,
        "steering_reward mismatch: {} vs {}",
        retrieved.steering_reward,
        edge.steering_reward
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_amortized_shortcut_edge() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");

    let mut edge = create_test_edge(node1.id, node2.id);
    println!("=== AMORTIZED SHORTCUT TEST ===");
    println!(
        "BEFORE: is_amortized_shortcut={}",
        edge.is_amortized_shortcut
    );

    // Mark as shortcut (simulating dream consolidation)
    edge.mark_as_shortcut();
    println!(
        "AFTER MARK: is_amortized_shortcut={}",
        edge.is_amortized_shortcut
    );

    db.store_edge(&edge).expect("store");

    // Read back from RocksDB
    let retrieved = db
        .get_edge(&node1.id, &node2.id, EdgeType::Semantic)
        .expect("get");

    println!(
        "VERIFY: is_amortized_shortcut={}",
        retrieved.is_amortized_shortcut
    );
    assert!(
        retrieved.is_amortized_shortcut,
        "edge should be marked as amortized shortcut"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_edge_traversal_tracking() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    db.store_node(&node1).expect("store node1");
    db.store_node(&node2).expect("store node2");

    let mut edge = create_test_edge(node1.id, node2.id);
    println!("=== EDGE TRAVERSAL TRACKING TEST ===");
    println!("BEFORE: traversal_count={}", edge.traversal_count);

    // Record multiple traversals
    edge.record_traversal();
    edge.record_traversal();
    edge.record_traversal();
    println!(
        "AFTER 3 TRAVERSALS: traversal_count={}",
        edge.traversal_count
    );

    db.store_edge(&edge).expect("store");

    // Read back from RocksDB
    let retrieved = db
        .get_edge(&node1.id, &node2.id, EdgeType::Semantic)
        .expect("get");

    println!("VERIFY: traversal_count={}", retrieved.traversal_count);
    assert_eq!(retrieved.traversal_count, 3, "should have 3 traversals");
    assert!(
        retrieved.last_traversed_at.is_some(),
        "last_traversed_at should be set"
    );
    println!("RESULT: PASSED");
}
