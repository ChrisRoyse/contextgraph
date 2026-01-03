---
id: "M04-T17"
title: "Implement DFS Graph Traversal"
description: |
  Implement dfs_traverse(storage, start, max_depth, max_nodes) function.
  Use iterative stack-based approach (not recursive).
  Returns Vec<NodeId> of visited nodes in DFS order.
  Handle cycles via visited set.
layer: "logic"
status: "pending"
priority: "medium"
estimated_hours: 2
sequence: 23
depends_on:
  - "M04-T13"
spec_refs:
  - "TECH-GRAPH-004 Section 7.2"
files_to_create:
  - path: "crates/context-graph-graph/src/traversal/dfs.rs"
    description: "DFS traversal implementation"
files_to_modify:
  - path: "crates/context-graph-graph/src/traversal/mod.rs"
    description: "Add dfs module"
test_file: "crates/context-graph-graph/tests/traversal_tests.rs"
---

## Context

DFS (Depth-First Search) traversal explores the graph by going as deep as possible before backtracking. Unlike BFS, it uses a stack (LIFO) instead of a queue (FIFO). The iterative implementation avoids stack overflow on deep graphs, which is critical for knowledge graphs with potentially long chains of relationships.

## Scope

### In Scope
- DfsParams configuration struct
- DfsResult output struct
- dfs_traverse() function with iterative stack
- Pre-order and post-order visit tracking
- Edge type filtering
- Cycle detection via visited set

### Out of Scope
- A* traversal (see M04-T17a)
- Domain-aware edge modulation (focus on structure)
- Topological sort (different use case)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/traversal/dfs.rs

use std::collections::HashSet;

use crate::error::{GraphError, GraphResult};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::storage::edges::EdgeType;

/// Parameters for DFS traversal
#[derive(Debug, Clone)]
pub struct DfsParams {
    /// Maximum depth to traverse (default: 100)
    pub max_depth: usize,

    /// Maximum number of nodes to visit (default: 10000)
    pub max_nodes: usize,

    /// Filter to specific edge types (None = all types)
    pub edge_types: Option<Vec<EdgeType>>,

    /// Whether to track post-order visitation
    pub track_post_order: bool,
}

impl Default for DfsParams {
    fn default() -> Self {
        Self {
            max_depth: 100,
            max_nodes: 10_000,
            edge_types: None,
            track_post_order: false,
        }
    }
}

impl DfsParams {
    /// Create params with maximum depth
    pub fn with_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
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

    /// Builder: enable post-order tracking
    pub fn with_post_order(mut self) -> Self {
        self.track_post_order = true;
        self
    }
}

/// Result of DFS traversal
#[derive(Debug, Clone)]
pub struct DfsResult {
    /// Visited node IDs in pre-order (discovery order)
    pub nodes: Vec<NodeId>,

    /// Visited node IDs in post-order (completion order)
    /// Only populated if track_post_order was true
    pub post_order: Vec<NodeId>,

    /// Starting node
    pub start_node: NodeId,

    /// Maximum depth reached
    pub max_depth_reached: usize,

    /// Whether traversal was limited by max_nodes
    pub truncated: bool,

    /// Number of back edges found (cycles)
    pub back_edges: usize,
}

impl DfsResult {
    /// Check if any nodes were found
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get total node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Check if cycles were detected
    pub fn has_cycles(&self) -> bool {
        self.back_edges > 0
    }
}

/// State for iterative DFS
#[derive(Debug)]
enum DfsState {
    /// Node to be visited (pre-order)
    Visit(NodeId, usize),  // (node, depth)
    /// Node finished processing (post-order)
    Finish(NodeId),
}

/// Perform DFS traversal from a starting node
///
/// Uses iterative stack-based approach to avoid stack overflow on deep graphs.
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID
/// * `params` - Traversal parameters
///
/// # Returns
/// * `DfsResult` with visited nodes in DFS order
///
/// # Example
/// ```
/// let params = DfsParams::default().max_depth(10);
/// let result = dfs_traverse(&storage, start_node, params)?;
/// println!("Found {} nodes", result.node_count());
/// ```
pub fn dfs_traverse(
    storage: &GraphStorage,
    start: NodeId,
    params: DfsParams,
) -> GraphResult<DfsResult> {
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack: Vec<DfsState> = Vec::new();
    let mut result_nodes: Vec<NodeId> = Vec::new();
    let mut post_order: Vec<NodeId> = Vec::new();
    let mut max_depth_reached: usize = 0;
    let mut truncated = false;
    let mut back_edges: usize = 0;

    // Start with the initial node
    stack.push(DfsState::Visit(start, 0));

    while let Some(state) = stack.pop() {
        match state {
            DfsState::Visit(node, depth) => {
                // Check if already visited
                if visited.contains(&node) {
                    back_edges += 1;
                    continue;
                }

                // Check node limit
                if result_nodes.len() >= params.max_nodes {
                    truncated = true;
                    break;
                }

                // Mark visited and add to pre-order result
                visited.insert(node);
                result_nodes.push(node);
                max_depth_reached = max_depth_reached.max(depth);

                // Schedule post-order processing if needed
                if params.track_post_order {
                    stack.push(DfsState::Finish(node));
                }

                // Check depth limit
                if depth >= params.max_depth {
                    continue;
                }

                // Get outgoing edges and push children onto stack
                // Push in reverse order so first child is processed first
                let edges = storage.get_adjacency(node)?;

                let mut children: Vec<NodeId> = edges
                    .iter()
                    .filter(|e| {
                        if let Some(ref allowed) = params.edge_types {
                            allowed.contains(&e.edge_type)
                        } else {
                            true
                        }
                    })
                    .map(|e| e.target)
                    .collect();

                // Reverse so first edge's target is on top of stack
                children.reverse();

                for child in children {
                    stack.push(DfsState::Visit(child, depth + 1));
                }
            }
            DfsState::Finish(node) => {
                post_order.push(node);
            }
        }
    }

    Ok(DfsResult {
        nodes: result_nodes,
        post_order,
        start_node: start,
        max_depth_reached,
        truncated,
        back_edges,
    })
}

/// Simple DFS returning just node list
///
/// Convenience function for basic DFS without result metadata.
pub fn dfs_simple(
    storage: &GraphStorage,
    start: NodeId,
    max_depth: usize,
    max_nodes: usize,
) -> GraphResult<Vec<NodeId>> {
    let params = DfsParams::default()
        .max_depth(max_depth)
        .max_nodes(max_nodes);

    let result = dfs_traverse(storage, start, params)?;
    Ok(result.nodes)
}

/// Check if target is reachable from start using DFS
pub fn dfs_reachable(
    storage: &GraphStorage,
    start: NodeId,
    target: NodeId,
    max_depth: usize,
) -> GraphResult<bool> {
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack: Vec<(NodeId, usize)> = vec![(start, 0)];

    while let Some((node, depth)) = stack.pop() {
        if node == target {
            return Ok(true);
        }

        if visited.contains(&node) || depth >= max_depth {
            continue;
        }

        visited.insert(node);

        let edges = storage.get_adjacency(node)?;
        for edge in edges {
            stack.push((edge.target, depth + 1));
        }
    }

    Ok(false)
}

/// Find all paths from start to target (up to limit)
pub fn dfs_all_paths(
    storage: &GraphStorage,
    start: NodeId,
    target: NodeId,
    max_depth: usize,
    max_paths: usize,
) -> GraphResult<Vec<Vec<NodeId>>> {
    let mut paths: Vec<Vec<NodeId>> = Vec::new();
    let mut stack: Vec<(NodeId, Vec<NodeId>)> = vec![(start, vec![start])];

    while let Some((node, path)) = stack.pop() {
        if paths.len() >= max_paths {
            break;
        }

        if node == target {
            paths.push(path);
            continue;
        }

        if path.len() > max_depth {
            continue;
        }

        let edges = storage.get_adjacency(node)?;
        for edge in edges {
            // Avoid cycles within a path
            if !path.contains(&edge.target) {
                let mut new_path = path.clone();
                new_path.push(edge.target);
                stack.push((edge.target, new_path));
            }
        }
    }

    Ok(paths)
}

/// Detect cycles reachable from start
pub fn dfs_find_cycles(
    storage: &GraphStorage,
    start: NodeId,
    max_depth: usize,
) -> GraphResult<Vec<Vec<NodeId>>> {
    let mut cycles: Vec<Vec<NodeId>> = Vec::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack: Vec<(NodeId, Vec<NodeId>)> = vec![(start, vec![start])];

    while let Some((node, path)) = stack.pop() {
        if path.len() > max_depth + 1 {
            continue;
        }

        let edges = storage.get_adjacency(node)?;
        for edge in edges {
            if edge.target == start && path.len() > 1 {
                // Found a cycle back to start
                let mut cycle = path.clone();
                cycle.push(edge.target);
                cycles.push(cycle);
            } else if !visited.contains(&edge.target) && !path.contains(&edge.target) {
                let mut new_path = path.clone();
                new_path.push(edge.target);
                stack.push((edge.target, new_path));
            }
        }

        visited.insert(node);
    }

    Ok(cycles)
}
```

### Constraints
- MUST use iterative stack, NOT recursion
- Visited set prevents infinite loops on cycles
- Stack contains state enum for pre/post-order tracking
- Children pushed in reverse order for correct DFS order
- No stack overflow on deep graphs

### Acceptance Criteria
- [ ] dfs_traverse() visits nodes in depth-first order
- [ ] Uses iterative stack, not recursion
- [ ] Respects max_depth and max_nodes
- [ ] No stack overflow on deep graphs
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
dfs_traverse(storage, start, params):
    visited = {}
    stack = [Visit(start, 0)]
    pre_order = []
    post_order = []

    while stack not empty:
        state = stack.pop()

        match state:
            Visit(node, depth):
                if node in visited:
                    back_edges += 1
                    continue

                if len(pre_order) >= max_nodes:
                    truncated = true
                    break

                visited.add(node)
                pre_order.append(node)

                if track_post_order:
                    stack.push(Finish(node))

                if depth >= max_depth:
                    continue

                children = storage.get_adjacency(node)
                    .filter(edge_types)
                    .map(|e| e.target)
                    .reverse()  // so first is on top

                for child in children:
                    stack.push(Visit(child, depth + 1))

            Finish(node):
                post_order.append(node)

    return DfsResult
```

### Edge Cases
- Start node not in graph: Return result with just start
- No outgoing edges: Return result with just start
- Cyclic graph: Visited set prevents revisiting
- Very deep graph: Iterative approach prevents stack overflow
- Self-loop: Detected as back edge

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph dfs
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] DFS visits in correct order (depth-first)
- [ ] Deep graphs don't cause stack overflow
- [ ] Cycles are handled correctly
- [ ] Post-order is correct if requested

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_graph() -> (GraphStorage, NodeId) {
        // Same as BFS test
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

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
    fn test_dfs_basic() {
        let (storage, start) = setup_test_graph();

        let result = dfs_traverse(&storage, start, DfsParams::default()).unwrap();

        // Should find all 7 nodes
        assert_eq!(result.nodes.len(), 7);
        assert_eq!(result.nodes[0], 1);  // Start first
    }

    #[test]
    fn test_dfs_order() {
        let (storage, start) = setup_test_graph();

        let result = dfs_traverse(&storage, start, DfsParams::default()).unwrap();

        // DFS should go deep first
        // Order: 1 -> 2 -> 4 -> 5 -> 3 -> 6 -> 7
        // (exact order depends on edge ordering)
        assert_eq!(result.nodes[0], 1);

        // After 1, should go to first child
        assert!(result.nodes[1] == 2 || result.nodes[1] == 3);
    }

    #[test]
    fn test_dfs_max_depth() {
        let (storage, start) = setup_test_graph();

        let result = dfs_traverse(&storage, start,
            DfsParams::default().max_depth(1)
        ).unwrap();

        // Depth 0: 1, Depth 1: 2 or 3
        assert!(result.nodes.len() <= 3);
    }

    #[test]
    fn test_dfs_post_order() {
        let (storage, start) = setup_test_graph();

        let result = dfs_traverse(&storage, start,
            DfsParams::default().with_post_order()
        ).unwrap();

        // Post-order should have all nodes
        assert_eq!(result.post_order.len(), 7);

        // Start node should be last in post-order
        assert_eq!(*result.post_order.last().unwrap(), 1);
    }

    #[test]
    fn test_dfs_reachable() {
        let (storage, _) = setup_test_graph();

        assert!(dfs_reachable(&storage, 1, 7, 10).unwrap());
        assert!(!dfs_reachable(&storage, 1, 99, 10).unwrap());
    }

    #[test]
    fn test_dfs_cycle_detection() {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        // Create cycle: 1 -> 2 -> 3 -> 1
        storage.put_adjacency(1, &[GraphEdge::semantic(1, 1, 2, 0.8)]).unwrap();
        storage.put_adjacency(2, &[GraphEdge::semantic(2, 2, 3, 0.8)]).unwrap();
        storage.put_adjacency(3, &[GraphEdge::semantic(3, 3, 1, 0.8)]).unwrap();

        let result = dfs_traverse(&storage, 1, DfsParams::default()).unwrap();

        assert!(result.has_cycles());
        assert!(result.back_edges > 0);
    }
}
```
