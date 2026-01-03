---
id: "M04-T16"
title: "Implement BFS Graph Traversal"
description: |
  Implement bfs_traverse(storage, start, params) function.
  BfsParams: max_depth (6), max_nodes (10000), edge_types (Option<Vec>), domain_filter.
  Returns BfsResult with nodes, edges, depth_counts.
  Use VecDeque for frontier, HashSet for visited tracking.
  Performance: <100ms for depth=6 on 10M node graph.
layer: "logic"
status: "pending"
priority: "high"
estimated_hours: 3
sequence: 22
depends_on:
  - "M04-T13"
  - "M04-T15"
spec_refs:
  - "TECH-GRAPH-004 Section 7.1"
  - "REQ-KG-061"
files_to_create:
  - path: "crates/context-graph-graph/src/traversal/bfs.rs"
    description: "BFS traversal implementation"
files_to_modify:
  - path: "crates/context-graph-graph/src/traversal/mod.rs"
    description: "Add bfs module and re-exports"
test_file: "crates/context-graph-graph/tests/traversal_tests.rs"
---

## Context

BFS (Breadth-First Search) traversal explores the graph level by level, visiting all neighbors before moving to the next depth level. This is essential for finding shortest paths and exploring node neighborhoods. The domain-aware version filters edges based on Marblestone domain matching, enabling focused exploration in specific knowledge domains.

## Scope

### In Scope
- BfsParams configuration struct
- BfsResult output struct
- bfs_traverse() function with domain filtering
- Edge type filtering
- Max depth and max nodes limits
- Visited set to prevent cycles

### Out of Scope
- DFS traversal (see M04-T17)
- A* traversal (see M04-T17a)
- Weighted shortest path algorithms

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/traversal/bfs.rs

use std::collections::{VecDeque, HashSet, HashMap};

use crate::error::{GraphError, GraphResult};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::storage::edges::{GraphEdge, EdgeType, Domain};

/// Parameters for BFS traversal
#[derive(Debug, Clone)]
pub struct BfsParams {
    /// Maximum depth to traverse (default: 6)
    pub max_depth: usize,

    /// Maximum number of nodes to visit (default: 10000)
    pub max_nodes: usize,

    /// Filter to specific edge types (None = all types)
    pub edge_types: Option<Vec<EdgeType>>,

    /// Domain filter for edge weighting (None = no domain preference)
    pub domain_filter: Option<Domain>,

    /// Minimum edge weight threshold (after modulation)
    pub min_weight: f32,

    /// Whether to include edge data in results
    pub include_edges: bool,

    /// Whether to record traversal on edges
    pub record_traversal: bool,
}

impl Default for BfsParams {
    fn default() -> Self {
        Self {
            max_depth: 6,
            max_nodes: 10_000,
            edge_types: None,
            domain_filter: None,
            min_weight: 0.0,
            include_edges: true,
            record_traversal: false,
        }
    }
}

impl BfsParams {
    /// Create params with maximum depth
    pub fn with_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            ..Default::default()
        }
    }

    /// Create params with domain filter
    pub fn for_domain(domain: Domain) -> Self {
        Self {
            domain_filter: Some(domain),
            ..Default::default()
        }
    }

    /// Builder: set max depth
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Builder: set max nodes
    pub fn max_nodes(mut self, nodes: usize) -> Self {
        self.max_nodes = nodes;
        self
    }

    /// Builder: set edge types filter
    pub fn edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    /// Builder: set domain filter
    pub fn domain(mut self, domain: Domain) -> Self {
        self.domain_filter = Some(domain);
        self
    }

    /// Builder: set minimum weight threshold
    pub fn min_weight(mut self, weight: f32) -> Self {
        self.min_weight = weight;
        self
    }
}

/// Result of BFS traversal
#[derive(Debug, Clone)]
pub struct BfsResult {
    /// Visited node IDs in BFS order
    pub nodes: Vec<NodeId>,

    /// Traversed edges (if include_edges was true)
    pub edges: Vec<GraphEdge>,

    /// Number of nodes found at each depth level
    pub depth_counts: HashMap<usize, usize>,

    /// Starting node
    pub start_node: NodeId,

    /// Actual depth reached
    pub max_depth_reached: usize,

    /// Whether traversal was limited by max_nodes
    pub truncated: bool,
}

impl BfsResult {
    /// Check if any nodes were found
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get total node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total edge count
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get nodes at a specific depth
    pub fn nodes_at_depth(&self, depth: usize) -> usize {
        *self.depth_counts.get(&depth).unwrap_or(&0)
    }
}

/// Perform BFS traversal from a starting node
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID
/// * `params` - Traversal parameters
///
/// # Returns
/// * `BfsResult` with visited nodes and edges
///
/// # Performance
/// * Target: <100ms for depth=6 on 10M node graph
///
/// # Example
/// ```
/// let params = BfsParams::default()
///     .max_depth(3)
///     .domain(Domain::Code);
///
/// let result = bfs_traverse(&storage, start_node, params)?;
/// println!("Found {} nodes", result.node_count());
/// ```
pub fn bfs_traverse(
    storage: &GraphStorage,
    start: NodeId,
    params: BfsParams,
) -> GraphResult<BfsResult> {
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut frontier: VecDeque<(NodeId, usize)> = VecDeque::new();
    let mut result_nodes: Vec<NodeId> = Vec::new();
    let mut result_edges: Vec<GraphEdge> = Vec::new();
    let mut depth_counts: HashMap<usize, usize> = HashMap::new();
    let mut max_depth_reached: usize = 0;
    let mut truncated = false;

    // Start with the initial node
    frontier.push_back((start, 0));
    visited.insert(start);

    while let Some((current_node, depth)) = frontier.pop_front() {
        // Check node limit
        if result_nodes.len() >= params.max_nodes {
            truncated = true;
            break;
        }

        // Add to results
        result_nodes.push(current_node);
        *depth_counts.entry(depth).or_insert(0) += 1;
        max_depth_reached = max_depth_reached.max(depth);

        // Check depth limit
        if depth >= params.max_depth {
            continue;
        }

        // Get outgoing edges
        let edges = storage.get_adjacency(current_node)?;

        for edge in edges {
            // Filter by edge type
            if let Some(ref allowed_types) = params.edge_types {
                if !allowed_types.contains(&edge.edge_type) {
                    continue;
                }
            }

            // Filter by minimum weight (with domain modulation if specified)
            let effective_weight = if let Some(domain) = params.domain_filter {
                edge.get_modulated_weight(domain)
            } else {
                edge.weight
            };

            if effective_weight < params.min_weight {
                continue;
            }

            // Check if target already visited
            if visited.contains(&edge.target) {
                continue;
            }

            // Add to frontier
            visited.insert(edge.target);
            frontier.push_back((edge.target, depth + 1));

            // Collect edge if requested
            if params.include_edges {
                result_edges.push(edge);
            }
        }
    }

    Ok(BfsResult {
        nodes: result_nodes,
        edges: result_edges,
        depth_counts,
        start_node: start,
        max_depth_reached,
        truncated,
    })
}

/// Perform BFS to find shortest path between two nodes
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID
/// * `target` - Target node ID
/// * `max_depth` - Maximum depth to search
///
/// # Returns
/// * `Some(Vec<NodeId>)` - Path from start to target (inclusive)
/// * `None` - If no path found within max_depth
pub fn bfs_shortest_path(
    storage: &GraphStorage,
    start: NodeId,
    target: NodeId,
    max_depth: usize,
) -> GraphResult<Option<Vec<NodeId>>> {
    if start == target {
        return Ok(Some(vec![start]));
    }

    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut frontier: VecDeque<(NodeId, usize)> = VecDeque::new();
    let mut parent: HashMap<NodeId, NodeId> = HashMap::new();

    frontier.push_back((start, 0));
    visited.insert(start);

    while let Some((current_node, depth)) = frontier.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let edges = storage.get_adjacency(current_node)?;

        for edge in edges {
            if visited.contains(&edge.target) {
                continue;
            }

            parent.insert(edge.target, current_node);
            visited.insert(edge.target);

            if edge.target == target {
                // Reconstruct path
                let mut path = vec![target];
                let mut current = target;

                while let Some(&prev) = parent.get(&current) {
                    path.push(prev);
                    current = prev;
                }

                path.reverse();
                return Ok(Some(path));
            }

            frontier.push_back((edge.target, depth + 1));
        }
    }

    Ok(None)
}

/// Get all nodes within a given distance from start
pub fn bfs_neighborhood(
    storage: &GraphStorage,
    start: NodeId,
    max_distance: usize,
) -> GraphResult<Vec<NodeId>> {
    let params = BfsParams::with_depth(max_distance)
        .include_edges(false);

    let result = bfs_traverse(storage, start, params)?;
    Ok(result.nodes)
}

// Helper impl for BfsParams
impl BfsParams {
    fn include_edges(mut self, include: bool) -> Self {
        self.include_edges = include;
        self
    }
}
```

### Constraints
- Use VecDeque for O(1) front/back operations
- Use HashSet for O(1) visited lookup
- Visited set prevents infinite loops on cycles
- max_depth limits exploration depth (default 6)
- max_nodes limits total visited nodes (default 10000)
- Domain filter applies Marblestone modulation to weights
- Performance: <100ms for depth=6 on 10M node graph

### Acceptance Criteria
- [ ] bfs_traverse() visits nodes level by level
- [ ] Respects max_depth and max_nodes limits
- [ ] edge_types filter restricts which edges to follow
- [ ] domain_filter applies Marblestone domain matching
- [ ] No infinite loops on cyclic graphs (visited set)
- [ ] depth_counts tracks nodes found at each depth
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
bfs_traverse(storage, start, params):
    visited = {start}
    frontier = [(start, 0)]  // (node, depth) queue
    result_nodes = []
    depth_counts = {}

    while frontier not empty:
        (node, depth) = frontier.pop_front()

        if len(result_nodes) >= max_nodes:
            break (truncated)

        result_nodes.append(node)
        depth_counts[depth] += 1

        if depth >= max_depth:
            continue  // don't expand

        for edge in storage.get_adjacency(node):
            if edge_types and edge.type not in edge_types:
                continue
            if domain_filter:
                weight = edge.get_modulated_weight(domain)
            else:
                weight = edge.weight
            if weight < min_weight:
                continue
            if edge.target in visited:
                continue

            visited.add(edge.target)
            frontier.append((edge.target, depth + 1))

    return BfsResult
```

### Edge Cases
- Start node not in graph: Return result with just start node
- No outgoing edges: Return result with just start node
- All edges filtered: Return result with just start node
- Cyclic graph: Visited set prevents revisiting
- Large graph: max_nodes truncation

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph bfs
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] BFS visits in correct order (level by level)
- [ ] Depth limits are respected
- [ ] Edge type filtering works
- [ ] Domain modulation affects traversal

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_graph() -> (GraphStorage, NodeId) {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        // Create a simple tree:
        //     1
        //    / \
        //   2   3
        //  /|   |\
        // 4 5   6 7

        storage.put_adjacency(1, &[
            GraphEdge::semantic(1, 1, 2, 0.8),
            GraphEdge::semantic(2, 1, 3, 0.8),
        ]).unwrap();

        storage.put_adjacency(2, &[
            GraphEdge::semantic(3, 2, 4, 0.7),
            GraphEdge::semantic(4, 2, 5, 0.7),
        ]).unwrap();

        storage.put_adjacency(3, &[
            GraphEdge::semantic(5, 3, 6, 0.7),
            GraphEdge::semantic(6, 3, 7, 0.7),
        ]).unwrap();

        (storage, 1)
    }

    #[test]
    fn test_bfs_basic() {
        let (storage, start) = setup_test_graph();

        let result = bfs_traverse(&storage, start, BfsParams::default()).unwrap();

        // Should find all 7 nodes
        assert_eq!(result.nodes.len(), 7);
        assert_eq!(result.nodes[0], 1);  // Start first

        // Depth 0: node 1
        // Depth 1: nodes 2, 3
        // Depth 2: nodes 4, 5, 6, 7
        assert_eq!(result.nodes_at_depth(0), 1);
        assert_eq!(result.nodes_at_depth(1), 2);
        assert_eq!(result.nodes_at_depth(2), 4);
    }

    #[test]
    fn test_bfs_max_depth() {
        let (storage, start) = setup_test_graph();

        let result = bfs_traverse(&storage, start,
            BfsParams::default().max_depth(1)
        ).unwrap();

        // Should find nodes at depth 0 and 1 only
        assert_eq!(result.nodes.len(), 3);  // 1, 2, 3
        assert_eq!(result.max_depth_reached, 1);
    }

    #[test]
    fn test_bfs_max_nodes() {
        let (storage, start) = setup_test_graph();

        let result = bfs_traverse(&storage, start,
            BfsParams::default().max_nodes(3)
        ).unwrap();

        assert_eq!(result.nodes.len(), 3);
        assert!(result.truncated);
    }

    #[test]
    fn test_bfs_shortest_path() {
        let (storage, _) = setup_test_graph();

        let path = bfs_shortest_path(&storage, 1, 7, 10).unwrap();
        assert!(path.is_some());

        let path = path.unwrap();
        assert_eq!(path[0], 1);  // Start
        assert_eq!(path.last(), Some(&7));  // End
        assert_eq!(path.len(), 3);  // 1 -> 3 -> 7
    }

    #[test]
    fn test_bfs_no_path() {
        let (storage, _) = setup_test_graph();

        // Node 99 doesn't exist
        let path = bfs_shortest_path(&storage, 1, 99, 10).unwrap();
        assert!(path.is_none());
    }
}
```
