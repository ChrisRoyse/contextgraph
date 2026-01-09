//! A* types and parameters.
//!
//! Contains the core types for A* traversal configuration and results.

use crate::storage::edges::{Domain, EdgeType};

/// Node ID type for A* (i64 for storage compatibility).
pub type NodeId = i64;

/// Parameters for A* traversal.
#[derive(Debug, Clone)]
pub struct AstarParams {
    /// Domain for NT weight modulation.
    pub domain: Domain,

    /// Minimum edge weight threshold (after modulation).
    pub min_weight: f32,

    /// Filter to specific edge types (None = all types).
    pub edge_types: Option<Vec<EdgeType>>,

    /// Maximum nodes to explore before giving up (default: 100000).
    pub max_nodes: usize,

    /// Heuristic scale factor (default: 0.1).
    /// Smaller values = more admissible but slower.
    /// Must be <= 1.0 for optimality guarantee.
    pub heuristic_scale: f32,
}

impl Default for AstarParams {
    fn default() -> Self {
        Self {
            domain: Domain::General,
            min_weight: 0.0,
            edge_types: None,
            max_nodes: 100_000,
            heuristic_scale: 0.1, // Conservative for admissibility
        }
    }
}

impl AstarParams {
    /// Builder: set domain for NT weight modulation.
    #[must_use]
    pub fn domain(mut self, domain: Domain) -> Self {
        self.domain = domain;
        self
    }

    /// Builder: set minimum weight threshold.
    #[must_use]
    pub fn min_weight(mut self, weight: f32) -> Self {
        self.min_weight = weight;
        self
    }

    /// Builder: set edge types filter.
    #[must_use]
    pub fn edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    /// Builder: set maximum nodes to explore.
    #[must_use]
    pub fn max_nodes(mut self, max: usize) -> Self {
        self.max_nodes = max;
        self
    }

    /// Builder: set heuristic scale factor.
    #[must_use]
    pub fn heuristic_scale(mut self, scale: f32) -> Self {
        self.heuristic_scale = scale;
        self
    }
}

/// Result of A* pathfinding.
#[derive(Debug, Clone)]
pub struct AstarResult {
    /// Path from start to goal (empty if no path found).
    pub path: Vec<NodeId>,

    /// Total path cost (f(goal) = g(goal) since h(goal) = 0).
    pub total_cost: f32,

    /// Number of nodes explored.
    pub nodes_explored: usize,

    /// Number of nodes in open set when terminated.
    pub open_set_size: usize,

    /// Whether path was found.
    pub path_found: bool,
}

impl AstarResult {
    /// Create empty result for no path found.
    #[must_use]
    pub fn no_path(nodes_explored: usize) -> Self {
        Self {
            path: Vec::new(),
            total_cost: f32::INFINITY,
            nodes_explored,
            open_set_size: 0,
            path_found: false,
        }
    }

    /// Create result with found path.
    #[must_use]
    pub fn found(
        path: Vec<NodeId>,
        total_cost: f32,
        nodes_explored: usize,
        open_set_size: usize,
    ) -> Self {
        Self {
            path,
            total_cost,
            nodes_explored,
            open_set_size,
            path_found: true,
        }
    }

    /// Get path length (number of nodes).
    #[must_use]
    pub fn path_length(&self) -> usize {
        self.path.len()
    }

    /// Get number of edges in path.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        if self.path.is_empty() {
            0
        } else {
            self.path.len() - 1
        }
    }
}
