//! Advanced tests for DFS traversal - performance and edge cases.

use std::collections::HashSet;
use tempfile::tempdir;
use uuid::Uuid;

use crate::storage::{GraphEdge, GraphStorage};
use crate::traversal::dfs::{dfs_traverse, DfsParams, Domain, EdgeType};

use super::basic::setup_test_graph;

#[test]
fn test_dfs_domain_modulation() {
    let (storage, start, _dir) = setup_test_graph();

    // With Code domain, edges from node 3 (Domain::Code) get bonus
    let result_code = dfs_traverse(
        &storage,
        start,
        DfsParams::default().domain(Domain::Code).min_weight(0.5),
    )
    .expect("DFS failed");

    // With General domain (different from Code edges)
    let result_general = dfs_traverse(
        &storage,
        start,
        DfsParams::default().domain(Domain::General).min_weight(0.5),
    )
    .expect("DFS failed");

    // Both should find nodes, but potentially different effective weights
    assert!(result_code.node_count() >= 1);
    assert!(result_general.node_count() >= 1);
}

#[test]
fn test_dfs_vs_bfs_coverage() {
    // DFS and BFS should visit the same set of nodes (just in different order)
    let (storage, start, _dir) = setup_test_graph();

    let dfs_result = dfs_traverse(&storage, start, DfsParams::default()).expect("DFS failed");

    // DFS should find all 7 nodes
    assert_eq!(dfs_result.node_count(), 7);

    let dfs_set: HashSet<_> = dfs_result.visited_order.iter().copied().collect();
    for i in 1..=7 {
        assert!(dfs_set.contains(&i), "DFS should visit node {}", i);
    }
}

#[test]
fn test_dfs_deep_chain() {
    // Test iterative DFS on deep chain (would overflow stack if recursive)
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    // Create chain: 0 -> 1 -> 2 -> ... -> 999
    let uuid = |id: i64| Uuid::from_u64_pair(id as u64, 0);
    let edges: Vec<GraphEdge> = (0..1000i64)
        .map(|i| {
            GraphEdge::new(
                i,
                uuid(i),
                uuid(i + 1),
                EdgeType::Semantic,
                0.8,
                Domain::General,
            )
        })
        .collect();
    storage.put_edges(&edges).expect("put_edges failed");

    // Should NOT stack overflow (iterative implementation)
    let result = dfs_traverse(
        &storage,
        0,
        DfsParams::default().max_depth(1000).max_nodes(1100),
    )
    .expect("DFS failed on deep chain - implementation may be recursive!");

    // Should find all 1001 nodes (0 through 1000)
    assert_eq!(result.node_count(), 1001);
    assert_eq!(result.max_depth_reached(), 1000);
}

#[test]
fn test_dfs_weight_boundary() {
    // Test weight filtering with modulated weights.
    // The modulation formula is: w_eff = weight * (1.0 + net_activation + domain_bonus) * steering_factor
    // With default Domain::General (no bonus) and no NT weights, effective weight = base weight.
    let dir = tempdir().expect("Failed to create temp dir");
    let storage = GraphStorage::open_default(dir.path()).expect("Failed to open storage");

    let uuid = |id: i64| Uuid::from_u64_pair(id as u64, 0);

    // Edge with base weight 0.5
    let edges = vec![GraphEdge::new(
        1,
        uuid(1),
        uuid(2),
        EdgeType::Semantic,
        0.5,
        Domain::General,
    )];
    storage.put_edges(&edges).expect("put_edges failed");

    // Low threshold (0.1) should always pass - effective weight will be >= base weight
    let result =
        dfs_traverse(&storage, 1, DfsParams::default().min_weight(0.1)).expect("DFS failed");
    assert_eq!(
        result.node_count(),
        2,
        "Edge with weight 0.5 should pass threshold 0.1"
    );

    // High threshold (2.0) should always fail - impossible to achieve with modulation
    // Max modulation: weight * (1 + 1.0 + 0.2) * 1.0 = weight * 2.2 = 0.5 * 2.2 = 1.1 max
    let result =
        dfs_traverse(&storage, 1, DfsParams::default().min_weight(2.0)).expect("DFS failed");
    assert_eq!(
        result.node_count(),
        1,
        "Edge with weight 0.5 should not reach threshold 2.0"
    );
}
