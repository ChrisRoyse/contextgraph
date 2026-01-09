//! Basic tests for DFS traversal.

use std::collections::{HashMap, HashSet};
use tempfile::tempdir;
use uuid::Uuid;

use crate::storage::{GraphEdge, GraphStorage};
use crate::traversal::dfs::{
    dfs_domain_neighborhood, dfs_neighborhood, dfs_traverse, DfsIterator, DfsParams, Domain,
    EdgeType, NodeId,
};

/// Create test graph and return (storage, start_node_id, tempdir).
pub fn setup_test_graph() -> (GraphStorage, NodeId, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create a simple tree structure:
    //     1
    //    / \
    //   2   3
    //  /|   |\
    // 4 5   6 7

    let uuid = |id: i64| Uuid::from_u64_pair(id as u64, 0);

    let edges = vec![
        // From node 1
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
            uuid(1),
            uuid(3),
            EdgeType::Semantic,
            0.8,
            Domain::General,
        ),
        // From node 2
        GraphEdge::new(
            3,
            uuid(2),
            uuid(4),
            EdgeType::Semantic,
            0.7,
            Domain::General,
        ),
        GraphEdge::new(
            4,
            uuid(2),
            uuid(5),
            EdgeType::Semantic,
            0.7,
            Domain::General,
        ),
        // From node 3
        GraphEdge::new(
            5,
            uuid(3),
            uuid(6),
            EdgeType::Hierarchical,
            0.7,
            Domain::Code,
        ),
        GraphEdge::new(
            6,
            uuid(3),
            uuid(7),
            EdgeType::Hierarchical,
            0.7,
            Domain::Code,
        ),
    ];

    storage.put_edges(&edges).expect("put_edges failed");

    (storage, 1, dir)
}

#[test]
fn test_dfs_basic_traversal() {
    let (storage, start, _dir) = setup_test_graph();

    let result = dfs_traverse(&storage, start, DfsParams::default()).expect("DFS failed");

    // Should find all 7 nodes
    assert_eq!(
        result.node_count(),
        7,
        "Expected 7 nodes, got {}",
        result.node_count()
    );
    assert_eq!(result.visited_order[0], 1, "Start node should be first");

    // DFS visits nodes in pre-order (depth-first)
    // Verify all nodes are present
    let visited_set: HashSet<_> = result.visited_order.iter().copied().collect();
    for i in 1..=7 {
        assert!(visited_set.contains(&i), "Node {} should be visited", i);
    }

    // Verify max depth
    assert_eq!(result.max_depth_reached(), 2);
}

#[test]
fn test_dfs_depth_limit() {
    let (storage, start, _dir) = setup_test_graph();

    let result =
        dfs_traverse(&storage, start, DfsParams::default().max_depth(1)).expect("DFS failed");

    // Should find only depth 0 and 1: nodes 1, 2, 3
    assert_eq!(result.node_count(), 3);
    assert_eq!(result.max_depth_reached(), 1);
}

#[test]
fn test_dfs_cycle_handling() {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create cycle: 1 -> 2 -> 3 -> 1
    let uuid = |id: i64| Uuid::from_u64_pair(id as u64, 0);
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
            0.8,
            Domain::General,
        ),
        GraphEdge::new(
            3,
            uuid(3),
            uuid(1),
            EdgeType::Semantic,
            0.8,
            Domain::General,
        ),
    ];
    storage.put_edges(&edges).expect("put_edges failed");

    let result = dfs_traverse(&storage, 1, DfsParams::default()).expect("DFS failed");

    // Should visit each node exactly once, no infinite loop
    assert_eq!(result.node_count(), 3);

    // Verify each node visited exactly once
    let mut counts: HashMap<NodeId, usize> = HashMap::new();
    for &node in &result.visited_order {
        *counts.entry(node).or_insert(0) += 1;
    }
    for i in 1..=3 {
        assert_eq!(
            counts.get(&i),
            Some(&1),
            "Node {} should be visited exactly once",
            i
        );
    }
}

#[test]
fn test_dfs_edge_type_filter() {
    let (storage, start, _dir) = setup_test_graph();

    // Only follow Semantic edges (not Hierarchical)
    let result = dfs_traverse(
        &storage,
        start,
        DfsParams::default().edge_types(vec![EdgeType::Semantic]),
    )
    .expect("DFS failed");

    // Nodes 6 and 7 are only reachable via Hierarchical edges
    // So we should find: 1, 2, 3, 4, 5 = 5 nodes
    assert_eq!(result.node_count(), 5);

    let visited_set: HashSet<_> = result.visited_order.iter().copied().collect();
    assert!(!visited_set.contains(&6), "Node 6 should NOT be visited");
    assert!(!visited_set.contains(&7), "Node 7 should NOT be visited");
}

#[test]
fn test_dfs_weight_threshold() {
    let (storage, start, _dir) = setup_test_graph();

    // Set very high min_weight to filter all edges
    let result = dfs_traverse(
        &storage,
        start,
        DfsParams::default().min_weight(2.0), // Impossible threshold
    )
    .expect("DFS failed");

    // All edges should be filtered, only start node remains
    assert_eq!(
        result.node_count(),
        1,
        "Only start node with impossible threshold"
    );
    assert_eq!(result.visited_order[0], start);
}

#[test]
fn test_dfs_path_reconstruction() {
    let (storage, start, _dir) = setup_test_graph();

    let result = dfs_traverse(&storage, start, DfsParams::default()).expect("DFS failed");

    // Test path to node 4 (should be 1 -> 2 -> 4)
    let path = result.path_to(4);
    assert!(path.is_some(), "Path to node 4 should exist");
    let path = path.unwrap();
    assert_eq!(path[0], 1, "Path should start at 1");
    assert_eq!(*path.last().unwrap(), 4, "Path should end at 4");

    // Test path to nonexistent node
    let no_path = result.path_to(99);
    assert!(no_path.is_none(), "Path to nonexistent node should be None");
}

#[test]
fn test_dfs_empty_graph() {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    let result = dfs_traverse(&storage, 1, DfsParams::default()).expect("DFS failed");

    // Should return just the start node (even if it has no edges)
    assert_eq!(result.node_count(), 1);
    assert_eq!(result.visited_order[0], 1);
    assert!(result.edges_traversed.is_empty());
}

#[test]
fn test_dfs_max_nodes_limit() {
    let (storage, start, _dir) = setup_test_graph();

    let result =
        dfs_traverse(&storage, start, DfsParams::default().max_nodes(3)).expect("DFS failed");

    assert_eq!(result.node_count(), 3, "Should stop at max_nodes limit");
}

#[test]
fn test_dfs_iterator() {
    let (storage, start, _dir) = setup_test_graph();

    let iter = DfsIterator::new(&storage, start, DfsParams::default());

    let collected: Vec<_> = iter.map(|r| r.expect("Iterator error")).collect();

    // Should yield all 7 nodes
    assert_eq!(collected.len(), 7);
    assert_eq!(collected[0].0, 1, "First node should be start");

    // Verify all nodes present
    let node_ids: HashSet<_> = collected.iter().map(|(id, _)| *id).collect();
    for i in 1..=7 {
        assert!(
            node_ids.contains(&i),
            "Node {} should be in iterator output",
            i
        );
    }
}

#[test]
fn test_dfs_neighborhood() {
    let (storage, start, _dir) = setup_test_graph();

    let neighbors = dfs_neighborhood(&storage, start, 1).expect("DFS failed");

    // Distance 1: start + immediate neighbors (2 and 3)
    assert!(neighbors.contains(&1));
    assert!(neighbors.contains(&2));
    assert!(neighbors.contains(&3));
    assert_eq!(neighbors.len(), 3);
}

#[test]
fn test_dfs_domain_neighborhood() {
    let (storage, start, _dir) = setup_test_graph();

    let neighbors =
        dfs_domain_neighborhood(&storage, start, 2, Domain::Code, 0.5).expect("DFS failed");

    // Should include nodes reachable with modulated weights >= 0.5
    assert!(neighbors.contains(&1));
    assert!(!neighbors.is_empty());
}
