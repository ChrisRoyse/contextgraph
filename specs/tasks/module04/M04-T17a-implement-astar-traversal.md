---
id: "M04-T17a"
title: "Implement A* Hyperbolic Traversal"
description: |
  Implement astar_traverse(storage, start, goal, heuristic) function.
  Use hyperbolic distance as admissible heuristic for efficient path finding.
  The heuristic uses Poincare ball distance to goal as lower bound.
  Returns Option<Vec<NodeId>> with optimal path if found.
layer: "logic"
status: "pending"
priority: "medium"
estimated_hours: 3
sequence: 24
depends_on:
  - "M04-T17"
spec_refs:
  - "TECH-GRAPH-004 Section 7"
  - "REQ-KG-061"
files_to_create:
  - path: "crates/context-graph-graph/src/traversal/astar.rs"
    description: "A* traversal with hyperbolic heuristic"
files_to_modify:
  - path: "crates/context-graph-graph/src/traversal/mod.rs"
    description: "Add astar module"
test_file: "crates/context-graph-graph/tests/traversal_tests.rs"
---

## Context

A* (A-star) is an informed search algorithm that uses a heuristic to guide exploration toward the goal. By using Poincare ball distance as the heuristic, we leverage the hyperbolic geometry of the knowledge graph's embeddings for efficient path finding. The heuristic is admissible (never overestimates) because hyperbolic distance provides a lower bound on actual path length.

## Scope

### In Scope
- A* algorithm implementation with priority queue
- Hyperbolic distance heuristic using PoincarePoint
- AstarParams configuration
- AstarResult with path and cost
- Fallback to BFS if hyperbolic data unavailable

### Out of Scope
- CUDA-accelerated heuristic (see M04-T23)
- Multi-goal A*
- Bidirectional A*

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/traversal/astar.rs

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

use crate::error::{GraphError, GraphResult};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::storage::edges::EdgeType;
use crate::hyperbolic::{PoincarePoint, PoincareBall, HyperbolicConfig};

/// Parameters for A* traversal
#[derive(Debug, Clone)]
pub struct AstarParams {
    /// Maximum nodes to expand (default: 100000)
    pub max_expansions: usize,

    /// Maximum path length (default: 100)
    pub max_path_length: usize,

    /// Filter to specific edge types (None = all types)
    pub edge_types: Option<Vec<EdgeType>>,

    /// Whether to use hyperbolic heuristic (falls back to Dijkstra if false)
    pub use_hyperbolic_heuristic: bool,

    /// Hyperbolic configuration for distance calculations
    pub hyperbolic_config: HyperbolicConfig,

    /// Weight of heuristic vs actual cost (epsilon for weighted A*)
    /// 1.0 = standard A*, >1.0 = faster but suboptimal
    pub epsilon: f32,
}

impl Default for AstarParams {
    fn default() -> Self {
        Self {
            max_expansions: 100_000,
            max_path_length: 100,
            edge_types: None,
            use_hyperbolic_heuristic: true,
            hyperbolic_config: HyperbolicConfig::default(),
            epsilon: 1.0,
        }
    }
}

impl AstarParams {
    /// Builder: set max expansions
    pub fn max_expansions(mut self, n: usize) -> Self {
        self.max_expansions = n;
        self
    }

    /// Builder: set max path length
    pub fn max_path_length(mut self, n: usize) -> Self {
        self.max_path_length = n;
        self
    }

    /// Builder: set edge types filter
    pub fn edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    /// Builder: disable hyperbolic heuristic (use Dijkstra)
    pub fn without_heuristic(mut self) -> Self {
        self.use_hyperbolic_heuristic = false;
        self
    }

    /// Builder: set epsilon for weighted A*
    pub fn epsilon(mut self, e: f32) -> Self {
        self.epsilon = e.max(1.0);
        self
    }
}

/// Result of A* traversal
#[derive(Debug, Clone)]
pub struct AstarResult {
    /// Path from start to goal (inclusive), None if no path
    pub path: Option<Vec<NodeId>>,

    /// Total cost of the path
    pub total_cost: f32,

    /// Number of nodes expanded
    pub nodes_expanded: usize,

    /// Number of nodes in open set at termination
    pub open_set_size: usize,

    /// Whether search was truncated by limits
    pub truncated: bool,
}

impl AstarResult {
    /// Check if a path was found
    pub fn found(&self) -> bool {
        self.path.is_some()
    }

    /// Get path length (number of edges)
    pub fn path_length(&self) -> usize {
        self.path.as_ref().map(|p| p.len().saturating_sub(1)).unwrap_or(0)
    }
}

/// Node state for priority queue
#[derive(Debug, Clone)]
struct AstarNode {
    node_id: NodeId,
    /// g(n): cost from start to this node
    g_cost: f32,
    /// f(n) = g(n) + h(n): estimated total cost
    f_cost: f32,
}

impl PartialEq for AstarNode {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl Eq for AstarNode {}

impl PartialOrd for AstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.f_cost.partial_cmp(&self.f_cost)
            .unwrap_or(Ordering::Equal)
    }
}

/// Perform A* traversal from start to goal
///
/// Uses hyperbolic distance as heuristic when available.
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID
/// * `goal` - Goal node ID
/// * `params` - Traversal parameters
///
/// # Returns
/// * `AstarResult` with optimal path if found
///
/// # Example
/// ```
/// let result = astar_traverse(&storage, start, goal, AstarParams::default())?;
/// if let Some(path) = result.path {
///     println!("Found path of length {}", path.len());
/// }
/// ```
pub fn astar_traverse(
    storage: &GraphStorage,
    start: NodeId,
    goal: NodeId,
    params: AstarParams,
) -> GraphResult<AstarResult> {
    // Early return if start == goal
    if start == goal {
        return Ok(AstarResult {
            path: Some(vec![start]),
            total_cost: 0.0,
            nodes_expanded: 0,
            open_set_size: 0,
            truncated: false,
        });
    }

    // Get goal's hyperbolic position for heuristic
    let goal_point = if params.use_hyperbolic_heuristic {
        storage.get_hyperbolic(goal)?
    } else {
        None
    };

    // Initialize data structures
    let mut open_set: BinaryHeap<AstarNode> = BinaryHeap::new();
    let mut g_costs: HashMap<NodeId, f32> = HashMap::new();
    let mut came_from: HashMap<NodeId, NodeId> = HashMap::new();
    let mut closed_set: HashSet<NodeId> = HashSet::new();
    let mut nodes_expanded: usize = 0;

    // Start node
    let start_h = compute_heuristic(storage, start, &goal_point, &params)?;
    open_set.push(AstarNode {
        node_id: start,
        g_cost: 0.0,
        f_cost: params.epsilon * start_h,
    });
    g_costs.insert(start, 0.0);

    while let Some(current) = open_set.pop() {
        // Check limits
        if nodes_expanded >= params.max_expansions {
            return Ok(AstarResult {
                path: None,
                total_cost: f32::INFINITY,
                nodes_expanded,
                open_set_size: open_set.len(),
                truncated: true,
            });
        }

        // Goal reached?
        if current.node_id == goal {
            let path = reconstruct_path(&came_from, goal);

            if path.len() > params.max_path_length {
                return Ok(AstarResult {
                    path: None,
                    total_cost: f32::INFINITY,
                    nodes_expanded,
                    open_set_size: open_set.len(),
                    truncated: true,
                });
            }

            return Ok(AstarResult {
                path: Some(path),
                total_cost: current.g_cost,
                nodes_expanded,
                open_set_size: open_set.len(),
                truncated: false,
            });
        }

        // Skip if already in closed set
        if closed_set.contains(&current.node_id) {
            continue;
        }

        closed_set.insert(current.node_id);
        nodes_expanded += 1;

        // Expand neighbors
        let edges = storage.get_adjacency(current.node_id)?;

        for edge in edges {
            // Filter by edge type
            if let Some(ref allowed) = params.edge_types {
                if !allowed.contains(&edge.edge_type) {
                    continue;
                }
            }

            // Skip if in closed set
            if closed_set.contains(&edge.target) {
                continue;
            }

            // Calculate tentative g cost
            // Edge cost = 1.0 - edge.weight (higher weight = lower cost)
            let edge_cost = 1.0 - edge.weight;
            let tentative_g = current.g_cost + edge_cost;

            // Check if this is a better path
            let existing_g = g_costs.get(&edge.target).copied().unwrap_or(f32::INFINITY);

            if tentative_g < existing_g {
                // This is a better path
                came_from.insert(edge.target, current.node_id);
                g_costs.insert(edge.target, tentative_g);

                let h = compute_heuristic(storage, edge.target, &goal_point, &params)?;
                let f_cost = tentative_g + params.epsilon * h;

                open_set.push(AstarNode {
                    node_id: edge.target,
                    g_cost: tentative_g,
                    f_cost,
                });
            }
        }
    }

    // No path found
    Ok(AstarResult {
        path: None,
        total_cost: f32::INFINITY,
        nodes_expanded,
        open_set_size: 0,
        truncated: false,
    })
}

/// Compute heuristic value (hyperbolic distance to goal)
fn compute_heuristic(
    storage: &GraphStorage,
    node: NodeId,
    goal_point: &Option<PoincarePoint>,
    params: &AstarParams,
) -> GraphResult<f32> {
    if !params.use_hyperbolic_heuristic {
        return Ok(0.0);  // Dijkstra: no heuristic
    }

    let goal_point = match goal_point {
        Some(p) => p,
        None => return Ok(0.0),  // No goal embedding, use Dijkstra
    };

    let node_point = match storage.get_hyperbolic(node)? {
        Some(p) => p,
        None => return Ok(0.0),  // No node embedding
    };

    // Compute Poincare ball distance
    let ball = PoincareBall::new(&params.hyperbolic_config);
    let distance = ball.distance(&node_point, goal_point);

    // Scale distance to be comparable with edge costs [0, 1]
    // Hyperbolic distance can be large, normalize by dividing by typical max
    let normalized = distance / 10.0;  // Assume max meaningful distance ~10

    Ok(normalized.min(1.0))  // Cap at 1.0
}

/// Reconstruct path from came_from map
fn reconstruct_path(came_from: &HashMap<NodeId, NodeId>, goal: NodeId) -> Vec<NodeId> {
    let mut path = vec![goal];
    let mut current = goal;

    while let Some(&prev) = came_from.get(&current) {
        path.push(prev);
        current = prev;
    }

    path.reverse();
    path
}

/// Simplified A* without hyperbolic heuristic (Dijkstra's algorithm)
pub fn dijkstra_shortest_path(
    storage: &GraphStorage,
    start: NodeId,
    goal: NodeId,
    max_expansions: usize,
) -> GraphResult<Option<Vec<NodeId>>> {
    let params = AstarParams::default()
        .without_heuristic()
        .max_expansions(max_expansions);

    let result = astar_traverse(storage, start, goal, params)?;
    Ok(result.path)
}

/// A* with custom heuristic function
pub fn astar_with_heuristic<H>(
    storage: &GraphStorage,
    start: NodeId,
    goal: NodeId,
    heuristic: H,
    max_expansions: usize,
) -> GraphResult<AstarResult>
where
    H: Fn(NodeId) -> f32,
{
    // Early return if start == goal
    if start == goal {
        return Ok(AstarResult {
            path: Some(vec![start]),
            total_cost: 0.0,
            nodes_expanded: 0,
            open_set_size: 0,
            truncated: false,
        });
    }

    let mut open_set: BinaryHeap<AstarNode> = BinaryHeap::new();
    let mut g_costs: HashMap<NodeId, f32> = HashMap::new();
    let mut came_from: HashMap<NodeId, NodeId> = HashMap::new();
    let mut closed_set: HashSet<NodeId> = HashSet::new();
    let mut nodes_expanded: usize = 0;

    open_set.push(AstarNode {
        node_id: start,
        g_cost: 0.0,
        f_cost: heuristic(start),
    });
    g_costs.insert(start, 0.0);

    while let Some(current) = open_set.pop() {
        if nodes_expanded >= max_expansions {
            return Ok(AstarResult {
                path: None,
                total_cost: f32::INFINITY,
                nodes_expanded,
                open_set_size: open_set.len(),
                truncated: true,
            });
        }

        if current.node_id == goal {
            return Ok(AstarResult {
                path: Some(reconstruct_path(&came_from, goal)),
                total_cost: current.g_cost,
                nodes_expanded,
                open_set_size: open_set.len(),
                truncated: false,
            });
        }

        if closed_set.contains(&current.node_id) {
            continue;
        }

        closed_set.insert(current.node_id);
        nodes_expanded += 1;

        let edges = storage.get_adjacency(current.node_id)?;

        for edge in edges {
            if closed_set.contains(&edge.target) {
                continue;
            }

            let edge_cost = 1.0 - edge.weight;
            let tentative_g = current.g_cost + edge_cost;
            let existing_g = g_costs.get(&edge.target).copied().unwrap_or(f32::INFINITY);

            if tentative_g < existing_g {
                came_from.insert(edge.target, current.node_id);
                g_costs.insert(edge.target, tentative_g);

                open_set.push(AstarNode {
                    node_id: edge.target,
                    g_cost: tentative_g,
                    f_cost: tentative_g + heuristic(edge.target),
                });
            }
        }
    }

    Ok(AstarResult {
        path: None,
        total_cost: f32::INFINITY,
        nodes_expanded,
        open_set_size: 0,
        truncated: false,
    })
}
```

### Constraints
- Uses hyperbolic distance as admissible heuristic
- Priority queue (BinaryHeap) for efficient node selection
- Heuristic never overestimates (admissible)
- Falls back to Dijkstra if hyperbolic data unavailable
- Edge cost = 1.0 - edge.weight (higher weight = lower cost)

### Acceptance Criteria
- [ ] astar_traverse() finds optimal path using A*
- [ ] Uses hyperbolic distance heuristic (admissible)
- [ ] Returns None if no path exists
- [ ] Returns Some(path) with shortest path
- [ ] Uses priority queue for frontier
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
astar(storage, start, goal, params):
    if start == goal:
        return [start]

    goal_point = storage.get_hyperbolic(goal)  // for heuristic

    open_set = PriorityQueue()  // min-heap by f_cost
    g_costs = {start: 0}
    came_from = {}
    closed_set = {}

    open_set.push(start, f=h(start))

    while open_set not empty:
        current = open_set.pop_min()

        if current == goal:
            return reconstruct_path(came_from, goal)

        if current in closed_set:
            continue

        closed_set.add(current)

        for edge in storage.get_adjacency(current):
            if edge.target in closed_set:
                continue

            edge_cost = 1.0 - edge.weight
            tentative_g = g_costs[current] + edge_cost

            if tentative_g < g_costs.get(edge.target, inf):
                came_from[edge.target] = current
                g_costs[edge.target] = tentative_g
                f = tentative_g + epsilon * h(edge.target, goal_point)
                open_set.push(edge.target, f)

    return None  // no path
```

### Heuristic Function
```
h(node, goal_point):
    node_point = storage.get_hyperbolic(node)
    if node_point is None or goal_point is None:
        return 0  // fallback to Dijkstra

    distance = poincare_ball.distance(node_point, goal_point)
    return min(distance / 10.0, 1.0)  // normalize
```

### Edge Cases
- Start == goal: Return single-node path
- No path exists: Return None
- Missing hyperbolic data: Fall back to Dijkstra (h=0)
- Very long path: Truncate at max_path_length
- Too many expansions: Return truncated result

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph astar
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] A* finds same path as BFS shortest path
- [ ] Hyperbolic heuristic reduces node expansions
- [ ] Path is actually optimal
- [ ] Priority queue orders correctly

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_graph_with_hyperbolic() -> (GraphStorage, NodeId, NodeId) {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        // Create graph and add hyperbolic coordinates
        storage.put_adjacency(1, &[
            GraphEdge::semantic(1, 1, 2, 0.8),
            GraphEdge::semantic(2, 1, 3, 0.5),  // longer path
        ]).unwrap();

        storage.put_adjacency(2, &[
            GraphEdge::semantic(3, 2, 4, 0.9),
        ]).unwrap();

        storage.put_adjacency(3, &[
            GraphEdge::semantic(4, 3, 4, 0.6),
        ]).unwrap();

        // Add hyperbolic coordinates (simplified)
        storage.put_hyperbolic(1, &PoincarePoint::origin()).unwrap();
        storage.put_hyperbolic(4, &PoincarePoint::origin()).unwrap();

        (storage, 1, 4)  // start, goal
    }

    #[test]
    fn test_astar_basic() {
        let (storage, start, goal) = setup_test_graph_with_hyperbolic();

        let result = astar_traverse(&storage, start, goal, AstarParams::default()).unwrap();

        assert!(result.found());
        let path = result.path.unwrap();
        assert_eq!(path[0], start);
        assert_eq!(*path.last().unwrap(), goal);
    }

    #[test]
    fn test_astar_same_node() {
        let (storage, start, _) = setup_test_graph_with_hyperbolic();

        let result = astar_traverse(&storage, start, start, AstarParams::default()).unwrap();

        assert!(result.found());
        assert_eq!(result.path.unwrap(), vec![start]);
        assert_eq!(result.nodes_expanded, 0);
    }

    #[test]
    fn test_astar_no_path() {
        let (storage, start, _) = setup_test_graph_with_hyperbolic();

        let result = astar_traverse(&storage, start, 999, AstarParams::default()).unwrap();

        assert!(!result.found());
        assert!(result.path.is_none());
    }

    #[test]
    fn test_astar_vs_dijkstra() {
        let (storage, start, goal) = setup_test_graph_with_hyperbolic();

        let astar_result = astar_traverse(&storage, start, goal, AstarParams::default()).unwrap();
        let dijkstra_result = dijkstra_shortest_path(&storage, start, goal, 100000).unwrap();

        // Both should find same path (optimal)
        assert_eq!(astar_result.path, dijkstra_result);

        // A* should expand fewer nodes (with good heuristic)
        // This depends on the graph structure and heuristic quality
    }
}
```
