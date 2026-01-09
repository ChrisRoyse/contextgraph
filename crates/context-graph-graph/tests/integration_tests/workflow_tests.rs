//! End-to-End Workflow Tests.
//!
//! Tests for complete workflow: storage -> index -> query -> verify.

use context_graph_graph::{
    storage::{LegacyGraphEdge, NodeId},
    Domain,
};

use crate::common::fixtures::{generate_test_edges, generate_test_nodes};
use crate::common::helpers::{
    create_test_storage, verify_entailment_cone, verify_hyperbolic_point, verify_storage_state,
    StateLog,
};

/// Test complete workflow: storage -> index -> query -> verify.
#[test]
fn test_end_to_end_workflow() {
    println!("\n=== TEST: End-to-End Workflow ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    // Step 1: Create test data
    let log1 = StateLog::new("hyperbolic_points", "0");
    let nodes = generate_test_nodes(42, 50, 1536);

    for node in &nodes {
        storage
            .put_hyperbolic(node.id, &node.point)
            .expect("Put hyperbolic failed");
    }
    log1.after("50");

    // Step 2: Create entailment cones
    let log2 = StateLog::new("entailment_cones", "0");
    for node in &nodes {
        storage
            .put_cone(node.id, &node.cone)
            .expect("Put cone failed");
    }
    log2.after("50");

    // Step 3: Create graph structure with edges
    let log3 = StateLog::new("edges", "0");
    let node_ids: Vec<NodeId> = nodes.iter().map(|n| n.id).collect();
    let edges = generate_test_edges(42, &node_ids, 3);

    for edge in &edges {
        // Map domain to u8 edge type
        let edge_type_u8 = match edge.domain {
            Domain::Code => 0,
            Domain::Legal => 1,
            Domain::Medical => 2,
            Domain::Creative => 3,
            Domain::Research => 4,
            Domain::General => 5,
        };
        storage
            .add_edge(
                edge.source_id,
                LegacyGraphEdge {
                    target: edge.target_id,
                    edge_type: edge_type_u8,
                },
            )
            .expect("Add edge failed");
    }
    log3.after(&edges.len().to_string());

    // Step 4: Verify final state
    verify_storage_state(&storage, 50, 50, 50).expect("Final state verification failed");

    // Step 5: Test query operations
    let first_node = &nodes[0];

    // Read back hyperbolic point
    let _retrieved_point = storage
        .get_hyperbolic(first_node.id)
        .expect("Get hyperbolic failed")
        .expect("Point should exist");

    verify_hyperbolic_point(&storage, first_node.id, &first_node.point, 1e-5)
        .expect("Point verification failed");

    // Read back cone
    let _retrieved_cone = storage
        .get_cone(first_node.id)
        .expect("Get cone failed")
        .expect("Cone should exist");

    verify_entailment_cone(&storage, first_node.id, &first_node.cone, 1e-5)
        .expect("Cone verification failed");

    // Step 6: Test adjacency traversal
    let adjacency = storage
        .get_adjacency(first_node.id)
        .expect("Get adjacency failed");
    assert!(!adjacency.is_empty(), "First node should have edges");
    println!(
        "  Node {} has {} outgoing edges",
        first_node.id,
        adjacency.len()
    );

    println!("=== PASSED: End-to-End Workflow ===\n");
}
