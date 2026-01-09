//! BFS (Breadth-First Search) graph traversal with Marblestone domain modulation.
//!
//! Explores the graph level by level, applying edge type filtering and
//! NT weight modulation based on query domain.
//!
//! # Performance
//!
//! Target: <100ms for depth=6 on 10M node graph.
//! Uses VecDeque for O(1) frontier operations.
//! Uses HashSet for O(1) visited lookup.
//!
//! # Constitution Reference
//!
//! - edge_model.nt_weights.formula: Canonical modulation formula
//! - AP-009: NaN/Infinity clamped to valid range

mod traversal;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types for backwards compatibility
pub use types::{BfsParams, BfsResult, NodeId};

// Re-export edge types for convenience
pub use crate::storage::edges::{Domain, EdgeType};

// Re-export all public functions
pub use traversal::{bfs_domain_neighborhood, bfs_neighborhood, bfs_shortest_path, bfs_traverse};
