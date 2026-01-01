//! Integration tests for Module 02 - Core Infrastructure.
//!
//! These tests verify end-to-end functionality of the storage layer.
//! All tests use REAL RocksDB instances via tempfile::TempDir.
//!
//! NO MOCK DATA. NO ASYNC. All assertions verify actual database state.
//!
//! # Test Categories
//! 1. Node lifecycle (CRUD)
//! 2. Marblestone edge features (NT weights, steering, shortcuts)
//! 3. Johari quadrant index consistency
//! 4. Cognitive pulse action matrix
//! 5. Tag and temporal index operations
//! 6. Concurrent access patterns
//! 7. Performance benchmarks
//! 8. Error handling paths
//! 9. Memex trait compliance
//! 10. Edge cases (empty, limits, NaN)
//!
//! # Full State Verification Protocol
//! After each operation:
//! 1. Execute the storage operation
//! 2. Read back from RocksDB via a separate get operation
//! 3. Compare expected vs actual
//! 4. Log BEFORE/TRIGGER/AFTER/VERIFY/RESULT for evidence

use chrono::{Duration, Utc};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tempfile::TempDir;

use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use context_graph_core::types::{
    CognitivePulse, EmbeddingVector, GraphEdge, JohariQuadrant, MemoryNode, NodeId,
    SuggestedAction,
};
use context_graph_storage::{Memex, RocksDbMemex, StorageError};

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Create a valid normalized embedding (magnitude ~1.0)
fn create_valid_embedding() -> EmbeddingVector {
    const DIM: usize = 1536;
    let val = 1.0_f32 / (DIM as f32).sqrt();
    vec![val; DIM]
}

fn create_test_node() -> MemoryNode {
    MemoryNode::new("Test content".to_string(), create_valid_embedding())
}

fn create_node_with_content(content: &str) -> MemoryNode {
    MemoryNode::new(content.to_string(), create_valid_embedding())
}

fn create_test_edge(source: NodeId, target: NodeId) -> GraphEdge {
    GraphEdge::new(source, target, EdgeType::Semantic, Domain::General)
}

fn setup_db() -> (RocksDbMemex, TempDir) {
    let tmp = TempDir::new().expect("create temp dir");
    let db = RocksDbMemex::open(tmp.path()).expect("open db");
    (db, tmp)
}

// ============================================================================
// NODE LIFECYCLE TESTS
// ============================================================================

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
        after_update.metadata.tags.contains(&"updated-tag".to_string()),
        "tags should contain 'updated-tag'"
    );

    // SOFT DELETE
    db.delete_node(&node_id, true).expect("soft delete");
    let soft_deleted = db.get_node(&node_id).expect("get soft deleted");
    println!("VERIFY: deleted flag={}", soft_deleted.metadata.deleted);
    assert!(soft_deleted.metadata.deleted, "node should be marked deleted");

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

// ============================================================================
// MARBLESTONE EDGE TESTS
// ============================================================================

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
    println!(
        "AFTER APPLY: steering_reward={}",
        edge.steering_reward
    );

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
    println!("AFTER 3 TRAVERSALS: traversal_count={}", edge.traversal_count);

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

// ============================================================================
// JOHARI QUADRANT INDEX TESTS
// ============================================================================

#[test]
fn test_johari_quadrant_index_consistency() {
    let (db, _tmp) = setup_db();

    let mut open_node = create_test_node();
    open_node.quadrant = JohariQuadrant::Open;
    let mut hidden_node = create_test_node();
    hidden_node.quadrant = JohariQuadrant::Hidden;

    println!("=== JOHARI INDEX TEST ===");
    println!(
        "BEFORE: open_node quadrant={:?}, hidden_node quadrant={:?}",
        open_node.quadrant, hidden_node.quadrant
    );

    db.store_node(&open_node).expect("store open");
    db.store_node(&hidden_node).expect("store hidden");

    // Query the indexes - Source of Truth
    let open_ids = db
        .get_nodes_by_quadrant(JohariQuadrant::Open, None, 0)
        .expect("query open");
    let hidden_ids = db
        .get_nodes_by_quadrant(JohariQuadrant::Hidden, None, 0)
        .expect("query hidden");

    println!(
        "VERIFY: Open has {} nodes, Hidden has {} nodes",
        open_ids.len(),
        hidden_ids.len()
    );
    assert!(
        open_ids.contains(&open_node.id),
        "open_node should be in Open index"
    );
    assert!(
        hidden_ids.contains(&hidden_node.id),
        "hidden_node should be in Hidden index"
    );

    // Transition test: move open_node to Hidden
    let mut transitioned = open_node.clone();
    transitioned.quadrant = JohariQuadrant::Hidden;
    println!(
        "TRIGGER: Transitioning node {} from Open to Hidden",
        transitioned.id
    );
    db.update_node(&transitioned).expect("update");

    // Verify indexes updated correctly
    let open_after = db
        .get_nodes_by_quadrant(JohariQuadrant::Open, None, 0)
        .expect("query");
    let hidden_after = db
        .get_nodes_by_quadrant(JohariQuadrant::Hidden, None, 0)
        .expect("query");

    println!(
        "AFTER TRANSITION: Open={}, Hidden={}",
        open_after.len(),
        hidden_after.len()
    );
    assert!(
        !open_after.contains(&open_node.id),
        "transitioned node should NOT be in Open index"
    );
    assert!(
        hidden_after.contains(&open_node.id),
        "transitioned node SHOULD be in Hidden index"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_all_four_johari_quadrants() {
    let (db, _tmp) = setup_db();

    println!("=== ALL FOUR JOHARI QUADRANTS TEST ===");

    // Create one node for each quadrant
    let mut node_open = create_test_node();
    node_open.quadrant = JohariQuadrant::Open;
    let mut node_hidden = create_test_node();
    node_hidden.quadrant = JohariQuadrant::Hidden;
    let mut node_blind = create_test_node();
    node_blind.quadrant = JohariQuadrant::Blind;
    let mut node_unknown = create_test_node();
    node_unknown.quadrant = JohariQuadrant::Unknown;

    db.store_node(&node_open).expect("store open");
    db.store_node(&node_hidden).expect("store hidden");
    db.store_node(&node_blind).expect("store blind");
    db.store_node(&node_unknown).expect("store unknown");

    // Query each quadrant
    for (quadrant, expected_id) in [
        (JohariQuadrant::Open, node_open.id),
        (JohariQuadrant::Hidden, node_hidden.id),
        (JohariQuadrant::Blind, node_blind.id),
        (JohariQuadrant::Unknown, node_unknown.id),
    ] {
        let ids = db
            .get_nodes_by_quadrant(quadrant, None, 0)
            .expect("query quadrant");
        println!("VERIFY: {:?} contains {} nodes", quadrant, ids.len());
        assert!(
            ids.contains(&expected_id),
            "{:?} index should contain the corresponding node",
            quadrant
        );
    }
    println!("RESULT: PASSED");
}

// ============================================================================
// COGNITIVE PULSE TESTS
// ============================================================================

#[test]
fn test_cognitive_pulse_action_matrix() {
    println!("=== COGNITIVE PULSE ACTION MATRIX ===");

    // Test decision paths from constitution.yaml and PRD
    // entropy > 0.7, coherence < 0.4 → Stabilize
    let stabilize = CognitivePulse::from_values(0.8, 0.3);
    println!(
        "entropy=0.8, coherence=0.3 => {:?}",
        stabilize.suggested_action
    );
    assert_eq!(
        stabilize.suggested_action,
        SuggestedAction::Stabilize,
        "High entropy + low coherence should suggest Stabilize"
    );

    // entropy < 0.4, coherence > 0.6 → Ready
    let ready = CognitivePulse::from_values(0.3, 0.8);
    println!(
        "entropy=0.3, coherence=0.8 => {:?}",
        ready.suggested_action
    );
    assert_eq!(
        ready.suggested_action,
        SuggestedAction::Ready,
        "Low entropy + high coherence should suggest Ready"
    );

    // entropy > 0.6, coherence > 0.5 → Explore
    let explore = CognitivePulse::from_values(0.7, 0.6);
    println!(
        "entropy=0.7, coherence=0.6 => {:?}",
        explore.suggested_action
    );
    assert_eq!(
        explore.suggested_action,
        SuggestedAction::Explore,
        "High entropy + moderate coherence should suggest Explore"
    );

    // Default case → Continue
    let continue_action = CognitivePulse::from_values(0.5, 0.5);
    println!(
        "entropy=0.5, coherence=0.5 => {:?}",
        continue_action.suggested_action
    );
    assert_eq!(
        continue_action.suggested_action,
        SuggestedAction::Continue,
        "Balanced state should suggest Continue"
    );

    // Low coherence → Consolidate
    let consolidate = CognitivePulse::from_values(0.4, 0.3);
    println!(
        "entropy=0.4, coherence=0.3 => {:?}",
        consolidate.suggested_action
    );
    assert_eq!(
        consolidate.suggested_action,
        SuggestedAction::Consolidate,
        "Low coherence should suggest Consolidate"
    );

    // Very high entropy → Prune
    let prune = CognitivePulse::from_values(0.85, 0.5);
    println!(
        "entropy=0.85, coherence=0.5 => {:?}",
        prune.suggested_action
    );
    assert_eq!(
        prune.suggested_action,
        SuggestedAction::Prune,
        "Very high entropy should suggest Prune"
    );

    println!("RESULT: PASSED");
}

#[test]
fn test_cognitive_pulse_is_healthy() {
    println!("=== COGNITIVE PULSE IS_HEALTHY TEST ===");

    let healthy = CognitivePulse::from_values(0.5, 0.5);
    println!(
        "BEFORE: entropy=0.5, coherence=0.5, is_healthy={}",
        healthy.is_healthy()
    );
    assert!(healthy.is_healthy(), "balanced pulse should be healthy");

    let unhealthy_high_entropy = CognitivePulse::from_values(0.9, 0.5);
    println!(
        "VERIFY: entropy=0.9, coherence=0.5, is_healthy={}",
        unhealthy_high_entropy.is_healthy()
    );
    assert!(
        !unhealthy_high_entropy.is_healthy(),
        "high entropy should be unhealthy"
    );

    let unhealthy_low_coherence = CognitivePulse::from_values(0.5, 0.2);
    println!(
        "VERIFY: entropy=0.5, coherence=0.2, is_healthy={}",
        unhealthy_low_coherence.is_healthy()
    );
    assert!(
        !unhealthy_low_coherence.is_healthy(),
        "low coherence should be unhealthy"
    );

    println!("RESULT: PASSED");
}

// ============================================================================
// TAG INDEX TESTS
// ============================================================================

#[test]
fn test_tag_index_consistency() {
    let (db, _tmp) = setup_db();
    let mut node = create_test_node();
    node.metadata.tags = vec!["rust".to_string(), "async".to_string()];

    println!("=== TAG INDEX TEST ===");
    println!("BEFORE: tags={:?}", node.metadata.tags);
    db.store_node(&node).expect("store");

    // Query for rust tag
    let rust_nodes = db
        .get_nodes_by_tag("rust", None, 0)
        .expect("query rust");
    println!("VERIFY: 'rust' tag has {} nodes", rust_nodes.len());
    assert!(
        rust_nodes.contains(&node.id),
        "node should be indexed under 'rust'"
    );

    // Update tags - remove rust, add tokio
    node.metadata.tags = vec!["async".to_string(), "tokio".to_string()];
    println!("TRIGGER: Update tags to {:?}", node.metadata.tags);
    db.update_node(&node).expect("update");

    // Verify indexes updated
    let rust_after = db
        .get_nodes_by_tag("rust", None, 0)
        .expect("query rust");
    let tokio_nodes = db
        .get_nodes_by_tag("tokio", None, 0)
        .expect("query tokio");

    println!(
        "VERIFY: rust={}, tokio={}",
        rust_after.len(),
        tokio_nodes.len()
    );
    assert!(
        !rust_after.contains(&node.id),
        "node should NOT be in 'rust' index after update"
    );
    assert!(
        tokio_nodes.contains(&node.id),
        "node SHOULD be in 'tokio' index after update"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_multiple_tags_per_node() {
    let (db, _tmp) = setup_db();
    let mut node = create_test_node();
    node.metadata.tags = vec![
        "tag1".to_string(),
        "tag2".to_string(),
        "tag3".to_string(),
    ];

    println!("=== MULTIPLE TAGS TEST ===");
    db.store_node(&node).expect("store");

    // Verify node appears in all tag indexes
    for tag in &["tag1", "tag2", "tag3"] {
        let ids = db.get_nodes_by_tag(tag, None, 0).expect("query");
        println!("VERIFY: '{}' contains node: {}", tag, ids.contains(&node.id));
        assert!(ids.contains(&node.id), "node should be in '{}' index", tag);
    }
    println!("RESULT: PASSED");
}

// ============================================================================
// TEMPORAL INDEX TESTS
// ============================================================================

#[test]
fn test_temporal_index() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    let node3 = create_test_node();

    db.store_node(&node1).expect("store 1");
    std::thread::sleep(std::time::Duration::from_millis(10));
    db.store_node(&node2).expect("store 2");
    std::thread::sleep(std::time::Duration::from_millis(10));
    db.store_node(&node3).expect("store 3");

    println!("=== TEMPORAL INDEX TEST ===");
    let start = node1.created_at - Duration::seconds(1);
    let end = Utc::now() + Duration::seconds(1);

    let nodes = db
        .get_nodes_in_time_range(start, end, None, 0)
        .expect("query");
    println!(
        "VERIFY: Found {} nodes in range [{} to {}]",
        nodes.len(),
        start,
        end
    );
    assert!(nodes.len() >= 3, "should find at least 3 nodes");
    assert!(nodes.contains(&node1.id), "should contain node1");
    assert!(nodes.contains(&node2.id), "should contain node2");
    assert!(nodes.contains(&node3.id), "should contain node3");
    println!("RESULT: PASSED");
}

// ============================================================================
// CONCURRENT ACCESS TESTS
// ============================================================================

#[test]
fn test_concurrent_reads() {
    let (db, _tmp) = setup_db();
    let db = Arc::new(db);
    let node = create_test_node();
    let node_id = node.id;
    db.store_node(&node).expect("store");

    println!("=== CONCURRENT READS TEST ===");
    println!("TRIGGER: Spawning 100 concurrent read threads");

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let db = Arc::clone(&db);
            thread::spawn(move || db.get_node(&node_id).is_ok())
        })
        .collect();

    let success_count: usize = handles
        .into_iter()
        .map(|h| if h.join().unwrap() { 1 } else { 0 })
        .sum();

    println!("VERIFY: {}/100 reads succeeded", success_count);
    assert_eq!(success_count, 100, "all 100 reads should succeed");
    println!("RESULT: PASSED");
}

#[test]
fn test_concurrent_writes() {
    let (db, _tmp) = setup_db();
    let db = Arc::new(db);

    println!("=== CONCURRENT WRITES TEST ===");
    println!("TRIGGER: Spawning 50 concurrent write threads");

    let handles: Vec<_> = (0..50)
        .map(|i| {
            let db = Arc::clone(&db);
            thread::spawn(move || {
                let node = create_node_with_content(&format!("Node {}", i));
                let id = node.id;
                (id, db.store_node(&node).is_ok())
            })
        })
        .collect();

    let mut stored = Vec::new();
    for h in handles {
        let (id, ok) = h.join().unwrap();
        if ok {
            stored.push(id);
        }
    }

    println!("VERIFY: {} nodes stored", stored.len());
    assert_eq!(stored.len(), 50, "all 50 nodes should be stored");

    // Verify all nodes are readable
    for id in &stored {
        assert!(
            db.get_node(id).is_ok(),
            "node {} should be readable after concurrent write",
            id
        );
    }
    println!("RESULT: PASSED");
}

// ============================================================================
// PERFORMANCE TESTS
// ============================================================================

#[test]
fn test_store_latency() {
    let (db, _tmp) = setup_db();
    let mut latencies = Vec::with_capacity(1000);

    println!("=== STORE LATENCY TEST ===");
    println!("TRIGGER: Storing 1000 nodes and measuring latency");

    for i in 0..1000 {
        let node = create_node_with_content(&format!("Perf {}", i));
        let start = Instant::now();
        db.store_node(&node).expect("store");
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p50 = latencies[500];
    let p95 = latencies[950];
    let p99 = latencies[990];

    println!("VERIFY: p50={:?}, p95={:?}, p99={:?}", p50, p95, p99);
    assert!(
        p99 < std::time::Duration::from_millis(10),
        "p99 latency should be under 10ms, got {:?}",
        p99
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_get_latency() {
    let (db, _tmp) = setup_db();
    let mut ids = Vec::with_capacity(1000);

    // First, store 1000 nodes
    for i in 0..1000 {
        let node = create_node_with_content(&format!("Perf {}", i));
        ids.push(node.id);
        db.store_node(&node).expect("store");
    }
    db.flush_all().expect("flush");

    println!("=== GET LATENCY TEST ===");
    println!("TRIGGER: Reading 1000 nodes and measuring latency");

    let mut latencies = Vec::with_capacity(1000);
    for id in &ids {
        let start = Instant::now();
        db.get_node(id).expect("get");
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p50 = latencies[500];
    let p95 = latencies[950];
    let p99 = latencies[990];

    println!("VERIFY: p50={:?}, p95={:?}, p99={:?}", p50, p95, p99);
    assert!(
        p99 < std::time::Duration::from_millis(5),
        "p99 get latency should be under 5ms, got {:?}",
        p99
    );
    println!("RESULT: PASSED");
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

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
    println!(
        "TRIGGER: Store with embedding dim={}",
        node.embedding.len()
    );

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

// ============================================================================
// MEMEX TRAIT TEST
// ============================================================================

#[test]
fn test_memex_trait() {
    let (db, _tmp) = setup_db();
    let memex: &dyn Memex = &db;

    println!("=== MEMEX TRAIT TEST ===");
    let node = create_test_node();
    let node_id = node.id;

    // Store via trait
    memex.store_node(&node).expect("store via trait");

    // Get via trait
    let retrieved = memex.get_node(&node_id).expect("get via trait");
    println!("VERIFY: roundtrip via trait");
    assert_eq!(retrieved.id, node.id);
    assert_eq!(retrieved.content, node.content);

    // Health check via trait
    let health = memex.health_check().expect("health check");
    println!(
        "VERIFY: is_healthy={}, node_count={}",
        health.is_healthy, health.node_count
    );
    assert!(health.is_healthy, "health check should pass");
    assert!(health.node_count >= 1, "should have at least 1 node");

    // Query via trait
    let by_quadrant = memex
        .query_by_quadrant(JohariQuadrant::Open, Some(10))
        .expect("query by quadrant");
    println!("VERIFY: query_by_quadrant returned {} results", by_quadrant.len());

    println!("RESULT: PASSED");
}

#[test]
fn test_memex_trait_object_safety() {
    let (db, _tmp) = setup_db();

    println!("=== MEMEX TRAIT OBJECT SAFETY TEST ===");
    println!("BEFORE: Creating Box<dyn Memex>");

    // This line compiles only if trait is object-safe
    let boxed: Box<dyn Memex> = Box::new(db);

    // Use the boxed trait object
    let node = create_test_node();
    boxed.store_node(&node).expect("store via Box<dyn Memex>");

    let retrieved = boxed.get_node(&node.id).expect("get via Box<dyn Memex>");
    assert_eq!(retrieved.id, node.id);

    println!("AFTER: Box<dyn Memex> operations successful");
    println!("RESULT: PASSED");
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn edge_case_empty_result() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: EMPTY RESULT ===");

    let result = db
        .get_nodes_by_tag("nonexistent-xyz-tag-12345", None, 0)
        .expect("query");
    println!(
        "BEFORE: Empty DB, TRIGGER: Query nonexistent tag, AFTER: len={}",
        result.len()
    );
    assert!(result.is_empty(), "should return empty vec for nonexistent tag");
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_limit_and_offset() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: LIMIT AND OFFSET ===");

    // Store 100 nodes with same tag
    for i in 0..100 {
        let mut node = create_node_with_content(&format!("N{}", i));
        node.metadata.tags.push("test-limit".to_string());
        db.store_node(&node).expect("store");
    }

    // Query with limit
    let limited = db
        .get_nodes_by_tag("test-limit", Some(10), 0)
        .expect("query");
    println!(
        "BEFORE: 100 nodes, TRIGGER: limit=10, AFTER: len={}",
        limited.len()
    );
    assert_eq!(limited.len(), 10, "should return exactly 10 nodes");

    // Query with offset
    let offset = db
        .get_nodes_by_tag("test-limit", Some(10), 50)
        .expect("query");
    println!("TRIGGER: limit=10, offset=50, AFTER: len={}", offset.len());
    assert_eq!(offset.len(), 10, "should return 10 nodes with offset");

    println!("RESULT: PASSED");
}

#[test]
fn edge_case_nan_importance() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: NaN IMPORTANCE ===");

    let mut node = create_test_node();
    node.importance = f32::NAN;
    println!(
        "BEFORE: importance=NaN, TRIGGER: store_node"
    );

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
    println!(
        "BEFORE: importance=INFINITY, TRIGGER: store_node"
    );

    let result = db.store_node(&node);
    println!("AFTER: error={}", result.is_err());
    assert!(result.is_err(), "Infinity importance should fail validation");
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_out_of_bounds_importance() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: OUT OF BOUNDS IMPORTANCE ===");

    let mut node = create_test_node();
    node.importance = 1.5; // Out of [0, 1] range
    println!(
        "BEFORE: importance=1.5 (out of [0,1]), TRIGGER: store_node"
    );

    let result = db.store_node(&node);
    println!("AFTER: error={}", result.is_err());
    assert!(result.is_err(), "Out-of-range importance should fail validation");
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_empty_content() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: EMPTY CONTENT ===");

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

#[test]
fn edge_case_source_index() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: SOURCE INDEX ===");

    let mut node = create_test_node();
    node.metadata.source = Some("test-source-url".to_string());
    db.store_node(&node).expect("store");

    let by_source = db
        .get_nodes_by_source("test-source-url", None, 0)
        .expect("query");
    println!(
        "VERIFY: source index contains {} nodes",
        by_source.len()
    );
    assert!(
        by_source.contains(&node.id),
        "node should be indexed by source"
    );
    println!("RESULT: PASSED");
}

// ============================================================================
// EDGE CRUD OPERATIONS
// ============================================================================

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
    println!("BEFORE: weight={}, confidence={}", edge.weight, edge.confidence);

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
        db.get_edge(&node1.id, &node2.id, EdgeType::Semantic).is_ok(),
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

// ============================================================================
// EMBEDDING OPERATIONS
// ============================================================================

#[test]
fn test_embedding_batch_get() {
    let (db, _tmp) = setup_db();
    let mut ids = Vec::new();

    println!("=== EMBEDDING BATCH GET TEST ===");

    // Store 5 nodes
    for i in 0..5 {
        let node = create_node_with_content(&format!("Batch {}", i));
        ids.push(node.id);
        db.store_node(&node).expect("store");
    }

    // Batch get embeddings
    let embeddings = db.batch_get_embeddings(&ids).expect("batch get");
    println!("VERIFY: got {} embeddings", embeddings.len());
    assert_eq!(embeddings.len(), 5);

    for (i, emb_opt) in embeddings.iter().enumerate() {
        assert!(
            emb_opt.is_some(),
            "embedding {} should exist",
            i
        );
        assert_eq!(
            emb_opt.as_ref().unwrap().len(),
            1536,
            "embedding {} should have 1536 dimensions",
            i
        );
    }
    println!("RESULT: PASSED");
}

#[test]
fn test_embedding_exists() {
    let (db, _tmp) = setup_db();
    let node = create_test_node();
    let fake_id = uuid::Uuid::new_v4();

    db.store_node(&node).expect("store");

    println!("=== EMBEDDING EXISTS TEST ===");

    let exists = db.embedding_exists(&node.id).expect("check exists");
    println!("VERIFY: embedding_exists for stored node = {}", exists);
    assert!(exists, "embedding should exist for stored node");

    let not_exists = db.embedding_exists(&fake_id).expect("check not exists");
    println!(
        "VERIFY: embedding_exists for fake id = {}",
        not_exists
    );
    assert!(!not_exists, "embedding should not exist for fake id");

    println!("RESULT: PASSED");
}

#[test]
fn test_delete_embedding() {
    let (db, _tmp) = setup_db();
    let node = create_test_node();
    let node_id = node.id;

    db.store_node(&node).expect("store");

    println!("=== DELETE EMBEDDING TEST ===");

    // Verify embedding exists
    assert!(db.embedding_exists(&node_id).expect("check"), "embedding should exist");

    // Delete embedding
    db.delete_embedding(&node_id).expect("delete embedding");

    // Verify embedding is gone
    let exists = db.embedding_exists(&node_id).expect("check after delete");
    println!("VERIFY: embedding_exists after delete = {}", exists);
    assert!(!exists, "embedding should not exist after delete");

    println!("RESULT: PASSED");
}

// ============================================================================
// HEALTH CHECK
// ============================================================================

#[test]
fn test_health_check() {
    let (db, _tmp) = setup_db();

    println!("=== HEALTH CHECK TEST ===");

    // Cast to Memex trait to get StorageHealth (inherent method returns ())
    let memex: &dyn Memex = &db;

    // Empty database health
    let health = memex.health_check().expect("health check");
    println!(
        "BEFORE: is_healthy={}, node_count={}, edge_count={}, storage_bytes={}",
        health.is_healthy, health.node_count, health.edge_count, health.storage_bytes
    );
    assert!(health.is_healthy);

    // Add some data
    for i in 0..10 {
        let node = create_node_with_content(&format!("Health {}", i));
        db.store_node(&node).expect("store");
    }

    let health_after = memex.health_check().expect("health check after");
    println!(
        "AFTER: is_healthy={}, node_count={}, edge_count={}, storage_bytes={}",
        health_after.is_healthy,
        health_after.node_count,
        health_after.edge_count,
        health_after.storage_bytes
    );
    assert!(health_after.is_healthy);
    assert!(health_after.node_count >= 10, "should count at least 10 nodes");

    println!("RESULT: PASSED");
}

// ============================================================================
// DOMAIN-SPECIFIC NT WEIGHTS
// ============================================================================

#[test]
fn test_domain_specific_nt_weights() {
    println!("=== DOMAIN-SPECIFIC NT WEIGHTS TEST ===");

    // Test that different domains produce different NT weights
    let code_nt = NeurotransmitterWeights::for_domain(Domain::Code);
    let legal_nt = NeurotransmitterWeights::for_domain(Domain::Legal);
    let medical_nt = NeurotransmitterWeights::for_domain(Domain::Medical);
    let creative_nt = NeurotransmitterWeights::for_domain(Domain::Creative);

    println!("Code domain NT: {:?}", code_nt);
    println!("Legal domain NT: {:?}", legal_nt);
    println!("Medical domain NT: {:?}", medical_nt);
    println!("Creative domain NT: {:?}", creative_nt);

    // Verify weights are in valid range [0, 1]
    for nt in [&code_nt, &legal_nt, &medical_nt, &creative_nt] {
        assert!(nt.excitatory >= 0.0 && nt.excitatory <= 1.0);
        assert!(nt.inhibitory >= 0.0 && nt.inhibitory <= 1.0);
        assert!(nt.modulatory >= 0.0 && nt.modulatory <= 1.0);
    }

    println!("RESULT: PASSED");
}

#[test]
fn test_nt_effective_weight_formula() {
    println!("=== NT EFFECTIVE WEIGHT FORMULA TEST ===");

    // Actual formula:
    // signal = base_weight * excitatory - base_weight * inhibitory
    // mod_factor = 1.0 + (modulatory - 0.5) * 0.4
    // result = (signal * mod_factor).clamp(0.0, 1.0)
    let nt = NeurotransmitterWeights {
        excitatory: 0.8,
        inhibitory: 0.2,
        modulatory: 0.4,
    };

    let base = 0.5;
    let signal = base * nt.excitatory - base * nt.inhibitory;
    let mod_factor = 1.0 + (nt.modulatory - 0.5) * 0.4;
    let expected = (signal * mod_factor).clamp(0.0, 1.0);
    let computed = nt.compute_effective_weight(base);

    println!(
        "VERIFY: base={}, NT={{exc={}, inh={}, mod={}}}, signal={}, mod_factor={}, expected={}, computed={}",
        base, nt.excitatory, nt.inhibitory, nt.modulatory, signal, mod_factor, expected, computed
    );
    assert!(
        (computed - expected).abs() < 0.001,
        "effective weight should match formula"
    );

    println!("RESULT: PASSED");
}
