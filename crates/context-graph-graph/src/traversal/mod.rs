//! Graph traversal algorithms.
//!
//! This module provides graph traversal algorithms for navigating the
//! knowledge graph with edge type filtering and NT weight modulation.
//!
//! # Algorithms
//!
//! - **BFS**: Breadth-first search for shortest paths and level-order exploration (M04-T16 ✓)
//! - **DFS**: Depth-first search (iterative, NOT recursive) for deep exploration (M04-T17 ✓)
//! - **A***: A* search with hyperbolic distance heuristic for optimal pathfinding (M04-T17a ✓)
//!
//! # Edge Filtering
//!
//! All traversals support filtering by:
//! - Edge types (Semantic, Temporal, Causal, Hierarchical)
//! - Minimum weight threshold
//! - Domain-specific modulation via NT weights
//!
//! # Components
//!
//! - BFS traversal (M04-T16 ✓)
//! - DFS traversal (M04-T17 ✓)
//! - A* traversal with hyperbolic heuristic (M04-T17a ✓)
//! - Traversal utilities (TODO: M04-T22)
//!
//! # Constitution Reference
//!
//! - edge_model.nt_weights.formula: w_eff = base * (1 + excitatory - inhibitory + 0.5*modulatory)
//!
//! # Examples
//!
//! ## Basic BFS Traversal
//!
//! ```rust,ignore
//! use context_graph_graph::traversal::{bfs_traverse, BfsParams, Domain};
//!
//! let params = BfsParams::default()
//!     .max_depth(3)
//!     .domain(Domain::Code)
//!     .min_weight(0.3);
//!
//! let result = bfs_traverse(&storage, start_node, params)?;
//! println!("Found {} nodes", result.node_count());
//! ```
//!
//! ## Shortest Path
//!
//! ```rust,ignore
//! use context_graph_graph::traversal::bfs_shortest_path;
//!
//! if let Some(path) = bfs_shortest_path(&storage, start, target, 10)? {
//!     println!("Path: {:?}", path);
//! }
//! ```
//!
//! ## A* Optimal Pathfinding
//!
//! ```rust,ignore
//! use context_graph_graph::traversal::{astar_search, AstarParams, Domain};
//!
//! let params = AstarParams::default()
//!     .domain(Domain::Code)
//!     .min_weight(0.3);
//!
//! let result = astar_search(&storage, start, goal, params)?;
//! if result.path_found {
//!     println!("Path: {:?}, cost: {}", result.path, result.total_cost);
//! }
//! ```

// M04-T16: BFS traversal with domain modulation
pub mod bfs;

// M04-T17: DFS traversal with domain modulation (iterative, NOT recursive)
pub mod dfs;

// M04-T17a: A* traversal with hyperbolic distance heuristic
pub mod astar;

// Re-export BFS public API
pub use bfs::{
    bfs_domain_neighborhood, bfs_neighborhood, bfs_shortest_path, bfs_traverse, BfsParams,
    BfsResult, Domain, EdgeType, NodeId,
};

// Re-export DFS public API (M04-T17 ✓)
pub use dfs::{
    dfs_domain_neighborhood, dfs_neighborhood, dfs_traverse, DfsIterator, DfsParams, DfsResult,
};

// Re-export A* public API (M04-T17a ✓)
pub use astar::{
    astar_bidirectional, astar_domain_path, astar_path, astar_search, AstarParams, AstarResult,
};

// M04-T17 COMPLETE: DFS implemented in dfs.rs (iterative, NOT recursive)
// M04-T17a COMPLETE: A* implemented in astar.rs with hyperbolic heuristic

// TODO: M04-T22 - Implement traversal utilities
// pub struct EdgeFilter {
//     pub edge_types: Option<Vec<EdgeType>>,
//     pub min_weight: f32,
//     pub domain: Option<Domain>,
// }
// pub fn get_modulated_weight(edge: &GraphEdge, domain: Domain) -> f32
