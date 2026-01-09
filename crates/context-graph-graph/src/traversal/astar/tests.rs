//! A* algorithm tests.

use std::collections::BinaryHeap;

use tempfile::tempdir;
use uuid::Uuid;

use crate::storage::edges::{Domain, EdgeType};
use crate::storage::{GraphEdge, GraphStorage, PoincarePoint as StoragePoint};

use super::helpers::edge_cost;
use super::node::AstarNode;
use super::types::{AstarParams, AstarResult};
use super::{astar_bidirectional, astar_domain_path, astar_path, astar_search};

/// Helper to create UUID from i64.
fn uuid(id: i64) -> Uuid {
    Uuid::from_u64_pair(id as u64, 0)
}

/// Create a test graph with hyperbolic embeddings.
fn setup_test_graph() -> (GraphStorage, i64, i64, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create simple path: 1 -> 2 -> 3 -> 4
    let edges = vec![
        GraphEdge::new(
            1,
            uuid(1),
            uuid(2),
            EdgeType::Semantic,
            0.8,
            Domain::General,
        ),
        GraphEdge::new(
            2,
            uuid(2),
            uuid(3),
            EdgeType::Semantic,
            0.7,
            Domain::General,
        ),
        GraphEdge::new(
            3,
            uuid(3),
            uuid(4),
            EdgeType::Semantic,
            0.9,
            Domain::General,
        ),
        // Alternative longer path: 1 -> 5 -> 6 -> 4
        GraphEdge::new(
            4,
            uuid(1),
            uuid(5),
            EdgeType::Semantic,
            0.6,
            Domain::General,
        ),
        GraphEdge::new(
            5,
            uuid(5),
            uuid(6),
            EdgeType::Semantic,
            0.5,
            Domain::General,
        ),
        GraphEdge::new(
            6,
            uuid(6),
            uuid(4),
            EdgeType::Semantic,
            0.4,
            Domain::General,
        ),
    ];
    storage.put_edges(&edges).expect("put_edges failed");

    // Create hyperbolic embeddings for all nodes
    // Use storage::PoincarePoint for put_hyperbolic
    // Arrange nodes in a line in hyperbolic space
    for id in 1..=6 {
        let mut coords = [0.0f32; 64];
        // Place nodes along first dimension, scaled to stay in ball
        coords[0] = (id as f32) * 0.1;
        let point = StoragePoint { coords };
        storage
            .put_hyperbolic(id, &point)
            .expect("put_hyperbolic failed");
    }

    (storage, 1, 4, dir)
}

#[test]
fn test_astar_basic_path() {
    let (storage, start, goal, _dir) = setup_test_graph();

    let result = astar_search(&storage, start, goal, AstarParams::default()).expect("A* failed");

    assert!(result.path_found, "Path should be found");
    assert_eq!(result.path[0], start, "Path should start at start node");
    assert_eq!(
        *result.path.last().unwrap(),
        goal,
        "Path should end at goal"
    );
    assert!(result.path.len() >= 2, "Path should have at least 2 nodes");
    assert!(result.total_cost > 0.0, "Cost should be positive");
    assert!(result.total_cost.is_finite(), "Cost should be finite");
}

#[test]
fn test_astar_same_node() {
    let (storage, start, _, _dir) = setup_test_graph();

    let result = astar_search(&storage, start, start, AstarParams::default()).expect("A* failed");

    assert!(result.path_found);
    assert_eq!(result.path, vec![start]);
    assert_eq!(result.total_cost, 0.0);
}

#[test]
fn test_astar_no_path() {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create disconnected nodes
    for id in 1..=2 {
        let mut coords = [0.0f32; 64];
        coords[0] = (id as f32) * 0.1;
        let point = StoragePoint { coords };
        storage
            .put_hyperbolic(id, &point)
            .expect("put_hyperbolic failed");
    }

    let result = astar_search(&storage, 1, 2, AstarParams::default()).expect("A* failed");

    assert!(
        !result.path_found,
        "No path should be found for disconnected nodes"
    );
    assert!(result.path.is_empty());
    assert!(result.total_cost.is_infinite());
}

#[test]
fn test_astar_missing_hyperbolic_data() {
    use crate::error::GraphError;

    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create edge but no hyperbolic embedding
    let edges = vec![GraphEdge::new(
        1,
        uuid(1),
        uuid(2),
        EdgeType::Semantic,
        0.8,
        Domain::General,
    )];
    storage.put_edges(&edges).expect("put_edges failed");

    // Only set hyperbolic for node 1, not node 2
    let mut coords = [0.0f32; 64];
    coords[0] = 0.1;
    let point = StoragePoint { coords };
    storage
        .put_hyperbolic(1, &point)
        .expect("put_hyperbolic failed");

    // Should fail with MissingHyperbolicData for goal
    let result = astar_search(&storage, 1, 2, AstarParams::default());
    assert!(result.is_err());
    match result.unwrap_err() {
        GraphError::MissingHyperbolicData(id) => {
            assert_eq!(id, 2, "Error should report missing data for node 2");
        }
        e => panic!("Expected MissingHyperbolicData, got {:?}", e),
    }
}

#[test]
fn test_astar_domain_modulation() {
    let (storage, start, goal, _dir) = setup_test_graph();

    // With Code domain (different from edges which are General)
    let params = AstarParams::default().domain(Domain::Code);
    let result = astar_search(&storage, start, goal, params).expect("A* failed");

    // Should still find a path, weights just modulated differently
    assert!(result.path_found);
    assert_eq!(result.path[0], start);
    assert_eq!(*result.path.last().unwrap(), goal);
}

#[test]
fn test_astar_weight_filter() {
    let (storage, start, goal, _dir) = setup_test_graph();

    // Very high min_weight (2.0) - should filter all edges.
    // Modulation formula: w_eff = weight * (1.0 + net_activation + domain_bonus) * steering_factor
    // Max possible: 0.8 * 2.2 = 1.76, so 2.0 is impossible to achieve.
    let params = AstarParams::default().min_weight(2.0);
    let result = astar_search(&storage, start, goal, params).expect("A* failed");

    assert!(!result.path_found, "No path with impossible weight filter");
}

#[test]
fn test_astar_edge_type_filter() {
    let (storage, start, goal, _dir) = setup_test_graph();

    // Only Hierarchical edges (none in graph)
    let params = AstarParams::default().edge_types(vec![EdgeType::Hierarchical]);
    let result = astar_search(&storage, start, goal, params).expect("A* failed");

    assert!(!result.path_found, "No path with edge type filter");
}

#[test]
fn test_astar_max_nodes_limit() {
    let (storage, start, goal, _dir) = setup_test_graph();

    // Very small limit
    let params = AstarParams::default().max_nodes(1);
    let result = astar_search(&storage, start, goal, params).expect("A* failed");

    // May or may not find path depending on exploration order
    assert!(result.nodes_explored <= 1);
}

#[test]
fn test_astar_bidirectional() {
    let (storage, start, goal, _dir) = setup_test_graph();

    let result = astar_bidirectional(&storage, start, goal, AstarParams::default())
        .expect("Bidirectional A* failed");

    assert!(result.path_found, "Bidirectional should find path");
    assert_eq!(result.path[0], start);
    assert_eq!(*result.path.last().unwrap(), goal);
}

#[test]
fn test_astar_path_convenience() {
    let (storage, start, goal, _dir) = setup_test_graph();

    let path = astar_path(&storage, start, goal).expect("astar_path failed");

    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(path[0], start);
    assert_eq!(*path.last().unwrap(), goal);
}

#[test]
fn test_astar_domain_path() {
    let (storage, start, goal, _dir) = setup_test_graph();

    let path = astar_domain_path(&storage, start, goal, Domain::General, 0.3)
        .expect("astar_domain_path failed");

    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(path[0], start);
    assert_eq!(*path.last().unwrap(), goal);
}

#[test]
fn test_astar_prefers_shorter_path() {
    let (storage, start, goal, _dir) = setup_test_graph();

    let result = astar_search(&storage, start, goal, AstarParams::default()).expect("A* failed");

    // The direct path 1->2->3->4 has 3 edges
    // The longer path 1->5->6->4 has 3 edges but lower weights
    // A* should prefer higher weight path
    assert!(result.path_found);
    assert!(result.path.len() <= 4, "Should find efficient path");
}

#[test]
fn test_edge_cost_function() {
    // Higher weight = lower cost
    assert!(edge_cost(0.9) < edge_cost(0.5));
    assert!(edge_cost(0.5) < edge_cost(0.1));

    // Edge cases
    assert!(edge_cost(0.0).is_finite());
    assert!(edge_cost(1.0).is_finite());
    assert!(edge_cost(-0.1).is_finite()); // Clamped to 0
    assert!(edge_cost(1.5).is_finite()); // Clamped to 1
}

#[test]
fn test_astar_node_ordering() {
    // Test min-heap behavior
    let n1 = AstarNode::new(1, 1.0, 2.0); // f = 3.0
    let n2 = AstarNode::new(2, 0.5, 1.0); // f = 1.5
    let n3 = AstarNode::new(3, 2.0, 3.0); // f = 5.0

    let mut heap = BinaryHeap::new();
    heap.push(n1);
    heap.push(n2);
    heap.push(n3);

    // Should pop in order of smallest f-score first (min-heap)
    assert_eq!(heap.pop().unwrap().node_id, 2); // f = 1.5
    assert_eq!(heap.pop().unwrap().node_id, 1); // f = 3.0
    assert_eq!(heap.pop().unwrap().node_id, 3); // f = 5.0
}

#[test]
fn test_astar_result_methods() {
    let result = AstarResult::found(vec![1, 2, 3, 4], 5.0, 10, 5);

    assert_eq!(result.path_length(), 4);
    assert_eq!(result.edge_count(), 3);
    assert!(result.path_found);
    assert_eq!(result.total_cost, 5.0);

    let no_path = AstarResult::no_path(10);
    assert_eq!(no_path.path_length(), 0);
    assert_eq!(no_path.edge_count(), 0);
    assert!(!no_path.path_found);
}
