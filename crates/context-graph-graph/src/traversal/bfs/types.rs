//! BFS types and data structures.
//!
//! Contains parameter and result types for BFS traversal operations.

use std::collections::HashMap;

use uuid::Uuid;

use crate::storage::GraphEdge;

// Re-export edge types for convenience
pub use crate::storage::edges::{Domain, EdgeType};

/// Node ID type for BFS (i64 for storage compatibility).
pub type NodeId = i64;

/// Parameters for BFS traversal.
///
/// Controls depth limits, node limits, and filtering behavior.
#[derive(Debug, Clone)]
pub struct BfsParams {
    /// Maximum depth to traverse (default: 6).
    /// Depth 0 is the start node.
    pub max_depth: usize,

    /// Maximum number of nodes to visit (default: 10000).
    /// Prevents runaway traversal on dense graphs.
    pub max_nodes: usize,

    /// Filter to specific edge types (None = all types).
    pub edge_types: Option<Vec<EdgeType>>,

    /// Domain filter for edge weighting (None = no domain preference).
    /// When set, uses `get_modulated_weight(domain)` instead of base weight.
    pub domain_filter: Option<Domain>,

    /// Minimum edge weight threshold (after modulation).
    /// Edges below this weight are not traversed.
    pub min_weight: f32,

    /// Whether to include edge data in results.
    pub include_edges: bool,

    /// Whether to record traversal on edges (updates steering_reward).
    /// Only set true if you will persist the updated edges.
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
    /// Create params with specific max depth.
    #[must_use]
    pub fn with_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            ..Default::default()
        }
    }

    /// Create params for specific domain.
    #[must_use]
    pub fn for_domain(domain: Domain) -> Self {
        Self {
            domain_filter: Some(domain),
            ..Default::default()
        }
    }

    /// Builder: set max depth.
    #[must_use]
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Builder: set max nodes.
    #[must_use]
    pub fn max_nodes(mut self, nodes: usize) -> Self {
        self.max_nodes = nodes;
        self
    }

    /// Builder: set edge types filter.
    #[must_use]
    pub fn edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    /// Builder: set domain filter.
    #[must_use]
    pub fn domain(mut self, domain: Domain) -> Self {
        self.domain_filter = Some(domain);
        self
    }

    /// Builder: set minimum weight threshold.
    #[must_use]
    pub fn min_weight(mut self, weight: f32) -> Self {
        self.min_weight = weight;
        self
    }

    /// Builder: set whether to include edges in results.
    #[must_use]
    pub fn include_edges(mut self, include: bool) -> Self {
        self.include_edges = include;
        self
    }
}

/// Result of BFS traversal.
#[derive(Debug, Clone)]
pub struct BfsResult {
    /// Visited node IDs in BFS order (i64).
    pub nodes: Vec<NodeId>,

    /// Traversed edges (if include_edges was true).
    pub edges: Vec<GraphEdge>,

    /// Number of nodes found at each depth level.
    pub depth_counts: HashMap<usize, usize>,

    /// Starting node ID.
    pub start_node: NodeId,

    /// Actual maximum depth reached.
    pub max_depth_reached: usize,

    /// Whether traversal was limited by max_nodes.
    pub truncated: bool,
}

impl BfsResult {
    /// Check if no nodes were found (shouldn't happen - start node always included).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get total node count.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total edge count.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get count of nodes at specific depth.
    #[must_use]
    pub fn nodes_at_depth(&self, depth: usize) -> usize {
        *self.depth_counts.get(&depth).unwrap_or(&0)
    }
}

/// Convert UUID to i64 for storage key operations.
///
/// This reverses `Uuid::from_u64_pair(id as u64, 0)` used in storage.
/// from_u64_pair stores values in big-endian order in the UUID bytes.
#[inline]
pub(crate) fn uuid_to_i64(uuid: &Uuid) -> i64 {
    let bytes = uuid.as_bytes();
    // from_u64_pair uses big-endian byte order
    i64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}
