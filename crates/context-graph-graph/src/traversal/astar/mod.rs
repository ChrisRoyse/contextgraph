//! A* graph traversal with hyperbolic distance heuristic.
//!
//! Optimal pathfinding in the knowledge graph using A* algorithm with
//! Poincare ball hyperbolic distance as the heuristic function.
//!
//! # Algorithm
//!
//! A* combines uniform-cost search with a heuristic:
//! - f(n) = g(n) + h(n)
//! - g(n) = actual cost from start to n
//! - h(n) = heuristic estimate from n to goal
//!
//! # Hyperbolic Heuristic
//!
//! Uses `PoincareBall.distance()` scaled by 0.1 to ensure admissibility.
//! The heuristic must underestimate actual path cost to guarantee optimality.
//!
//! # Edge Cost
//!
//! Edge cost = 1.0 / (effective_weight + 0.001)
//! Higher weights = lower cost = preferred paths.
//!
//! # Performance
//!
//! Target: <50ms for single-source-single-target on 10M node graph.
//! Uses BinaryHeap for O(log n) priority queue operations.
//!
//! # Constitution Reference
//!
//! - edge_model.nt_weights.formula: w_eff = base * (1 + excitatory - inhibitory + 0.5*modulatory)
//! - AP-001: Never unwrap() - returns MissingHyperbolicData if embeddings missing
//! - AP-009: NaN/Infinity clamped to valid range

mod algorithm;
mod bidirectional;
mod helpers;
mod node;
mod types;

#[cfg(test)]
mod tests;

// Re-export public API
pub use algorithm::astar_search;
pub use bidirectional::astar_bidirectional;
pub use types::{AstarParams, AstarResult, NodeId};

// Re-export edge types for convenience
pub use crate::storage::edges::{Domain, EdgeType};

/// Convenience function: find optimal path with default parameters.
pub fn astar_path(
    storage: &crate::storage::GraphStorage,
    start: NodeId,
    goal: NodeId,
) -> crate::error::GraphResult<Option<Vec<NodeId>>> {
    let result = astar_search(storage, start, goal, AstarParams::default())?;
    if result.path_found {
        Ok(Some(result.path))
    } else {
        Ok(None)
    }
}

/// Find optimal path with domain modulation.
pub fn astar_domain_path(
    storage: &crate::storage::GraphStorage,
    start: NodeId,
    goal: NodeId,
    domain: Domain,
    min_weight: f32,
) -> crate::error::GraphResult<Option<Vec<NodeId>>> {
    let params = AstarParams::default().domain(domain).min_weight(min_weight);
    let result = astar_search(storage, start, goal, params)?;
    if result.path_found {
        Ok(Some(result.path))
    } else {
        Ok(None)
    }
}
