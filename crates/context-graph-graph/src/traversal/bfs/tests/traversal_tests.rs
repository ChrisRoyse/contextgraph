//! Tests for BFS traversal algorithms.

use tempfile::tempdir;
use uuid::Uuid;

use crate::storage::edges::{Domain, EdgeType};
use crate::storage::{GraphEdge, GraphStorage};
use crate::traversal::bfs::{bfs_neighborhood, bfs_shortest_path, bfs_traverse, BfsParams};

/// Create test graph and return (storage, start_node_id, tempdir).
/// TempDir must be kept alive for the duration of the test.
fn setup_test_graph() -> (GraphStorage, i64, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create a simple tree structure:
    //     1
    //    / \
    //   2   3
    //  /|   |\
    // 4 5   6 7

    // Use Uuid::from_u64_pair to create consistent UUIDs from i64 node IDs
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
fn test_bfs_basic_traversal() {
    let (storage, start, _dir) = setup_test_graph();

    let result = bfs_traverse(&storage, start, BfsParams::default()).expect("BFS failed");

    // Should find all 7 nodes
    assert_eq!(
        result.node_count(),
        7,
        "Expected 7 nodes, got {}",
        result.node_count()
    );
    assert_eq!(result.nodes[0], 1, "Start node should be first");

    // Verify depth counts
    assert_eq!(result.nodes_at_depth(0), 1, "Depth 0: 1 node");
    assert_eq!(result.nodes_at_depth(1), 2, "Depth 1: 2 nodes");
    assert_eq!(result.nodes_at_depth(2), 4, "Depth 2: 4 nodes");

    assert!(!result.truncated);
    assert_eq!(result.max_depth_reached, 2);
}

#[test]
fn test_bfs_max_depth_limit() {
    let (storage, start, _dir) = setup_test_graph();

    let result =
        bfs_traverse(&storage, start, BfsParams::default().max_depth(1)).expect("BFS failed");

    // Should find only depth 0 and 1: nodes 1, 2, 3
    assert_eq!(result.node_count(), 3);
    assert_eq!(result.max_depth_reached, 1);
}

#[test]
fn test_bfs_max_nodes_limit() {
    let (storage, start, _dir) = setup_test_graph();

    let result =
        bfs_traverse(&storage, start, BfsParams::default().max_nodes(3)).expect("BFS failed");

    assert_eq!(result.node_count(), 3);
    assert!(result.truncated);
}

#[test]
fn test_bfs_edge_type_filter() {
    let (storage, start, _dir) = setup_test_graph();

    // Only follow Semantic edges (not Hierarchical)
    let result = bfs_traverse(
        &storage,
        start,
        BfsParams::default().edge_types(vec![EdgeType::Semantic]),
    )
    .expect("BFS failed");

    // Nodes 6 and 7 are only reachable via Hierarchical edges
    // So we should find: 1, 2, 3, 4, 5 = 5 nodes
    assert_eq!(result.node_count(), 5);
}

#[test]
fn test_bfs_domain_modulation() {
    let (storage, start, _dir) = setup_test_graph();

    // With Code domain, edges from node 3 (Domain::Code) get bonus
    let result = bfs_traverse(
        &storage,
        start,
        BfsParams::default().domain(Domain::Code).min_weight(0.5),
    )
    .expect("BFS failed");

    // All edges should pass 0.5 threshold
    assert!(result.node_count() >= 1);

    // Verify modulated weights are accessible
    for edge in &result.edges {
        let w = edge.get_modulated_weight(Domain::Code);
        assert!(w >= 0.5, "Edge weight {} should be >= 0.5", w);
    }
}

#[test]
fn test_bfs_shortest_path_found() {
    let (storage, _, _dir) = setup_test_graph();

    let path = bfs_shortest_path(&storage, 1, 7, 10).expect("BFS failed");

    assert!(path.is_some());
    let path = path.unwrap();

    assert_eq!(path[0], 1, "Path should start at node 1");
    assert_eq!(*path.last().unwrap(), 7, "Path should end at node 7");
    assert_eq!(path.len(), 3, "Path should be: 1 -> 3 -> 7");
}

#[test]
fn test_bfs_shortest_path_not_found() {
    let (storage, _, _dir) = setup_test_graph();

    // Node 99 doesn't exist
    let path = bfs_shortest_path(&storage, 1, 99, 10).expect("BFS failed");

    assert!(path.is_none());
}

#[test]
fn test_bfs_neighborhood() {
    let (storage, start, _dir) = setup_test_graph();

    let neighbors = bfs_neighborhood(&storage, start, 1).expect("BFS failed");

    // Distance 1: nodes 1, 2, 3
    assert_eq!(neighbors.len(), 3);
    assert!(neighbors.contains(&1));
    assert!(neighbors.contains(&2));
    assert!(neighbors.contains(&3));
}

#[test]
fn test_bfs_empty_graph() {
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    let result = bfs_traverse(&storage, 1, BfsParams::default()).expect("BFS failed");

    // Should return just the start node (even if it has no edges)
    assert_eq!(result.node_count(), 1);
    assert_eq!(result.nodes[0], 1);
    assert_eq!(result.edge_count(), 0);
}

#[test]
fn test_bfs_min_weight_filter() {
    let (storage, start, _dir) = setup_test_graph();

    // Set high min_weight to filter most edges
    let result =
        bfs_traverse(&storage, start, BfsParams::default().min_weight(0.9)).expect("BFS failed");

    // Most edges have weight 0.7-0.8, should be filtered
    // Only start node should be returned
    assert!(result.node_count() < 7);
}

#[test]
fn test_bfs_cyclic_graph() {
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

    let result = bfs_traverse(&storage, 1, BfsParams::default()).expect("BFS failed");

    // Should visit each node exactly once, no infinite loop
    assert_eq!(result.node_count(), 3);
    assert!(result.nodes.contains(&1));
    assert!(result.nodes.contains(&2));
    assert!(result.nodes.contains(&3));
}
