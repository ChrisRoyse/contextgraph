//! Core DFS traversal implementation.
//!
//! ITERATIVE depth-first search (NO recursion) to avoid stack overflow.
//!
//! # Performance
//!
//! Target: No stack overflow on 100,000+ node graphs.
//! Uses Vec<(NodeId, usize)> as explicit stack.
//! Uses HashSet for O(1) visited lookup.
//!
//! # Constitution Reference
//!
//! - edge_model.nt_weights.formula: Canonical modulation formula
//! - AP-009: NaN/Infinity clamped to valid range

use std::collections::HashSet;

use crate::error::GraphResult;
use crate::storage::GraphStorage;

use super::iterator::uuid_to_i64;
use super::result::DfsResult;
use super::types::{DfsParams, Domain, NodeId};

/// Perform ITERATIVE DFS traversal from a starting node.
///
/// # IMPORTANT: This is an ITERATIVE implementation using explicit stack.
/// NO RECURSION is used to avoid stack overflow on large graphs.
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID (i64)
/// * `params` - Traversal parameters
///
/// # Returns
/// * `Ok(DfsResult)` - Traversal results
/// * `Err(GraphError)` - Storage access failed or node not found
///
/// # Algorithm
/// Uses a Vec<(NodeId, usize)> as explicit stack for depth-first traversal.
/// Visits nodes in pre-order (node visited before its children).
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_graph::traversal::dfs::{dfs_traverse, DfsParams, Domain};
///
/// let params = DfsParams::default()
///     .max_depth(5)
///     .domain(Domain::Code)
///     .min_weight(0.3);
///
/// let result = dfs_traverse(&storage, start_node, params)?;
/// println!("Visited {} nodes, max depth: {}",
///     result.node_count(), result.max_depth_reached());
/// ```
pub fn dfs_traverse(
    storage: &GraphStorage,
    start: NodeId,
    params: DfsParams,
) -> GraphResult<DfsResult> {
    let mut result = DfsResult::new();
    let mut visited: HashSet<NodeId> = HashSet::new();

    // ITERATIVE: Use Vec as explicit stack (NOT recursion)
    // Each entry is (node_id, depth)
    let mut stack: Vec<(NodeId, usize)> = vec![(start, 0)];

    // Initialize parent for start node
    result.parents.insert(start, None);

    while let Some((current, depth)) = stack.pop() {
        // Skip if already visited (handles cycles)
        if visited.contains(&current) {
            continue;
        }

        // Check max nodes limit BEFORE processing
        if let Some(max) = params.max_nodes {
            if visited.len() >= max {
                log::debug!("DFS truncated at {} nodes (limit: {})", visited.len(), max);
                break;
            }
        }

        // Mark as visited and record in result
        visited.insert(current);
        result.visited_order.push(current);
        result.depths.insert(current, depth);

        // Don't expand if at max depth
        if let Some(max_depth) = params.max_depth {
            if depth >= max_depth {
                continue;
            }
        }

        // CORRECT API: get_outgoing_edges NOT get_adjacency
        let edges = storage.get_outgoing_edges(current)?;

        // Collect valid neighbors to push to stack
        let mut neighbors: Vec<(NodeId, f32)> = Vec::new();

        for edge in edges {
            // Filter by edge type if specified
            if let Some(ref allowed_types) = params.edge_types {
                if !allowed_types.contains(&edge.edge_type) {
                    continue;
                }
            }

            // Get NT-modulated weight
            let effective_weight = edge.get_modulated_weight(params.domain);

            // Filter by minimum weight
            if effective_weight < params.min_weight {
                continue;
            }

            // CRITICAL: Convert UUID to i64
            let neighbor_id = uuid_to_i64(&edge.target);

            // Skip if already visited
            if visited.contains(&neighbor_id) {
                continue;
            }

            neighbors.push((neighbor_id, effective_weight));

            // Record parent if not already set
            result.parents.entry(neighbor_id).or_insert(Some(current));

            // Record edge traversal
            result
                .edges_traversed
                .push((current, neighbor_id, effective_weight));
        }

        // Push neighbors to stack in REVERSE order for correct DFS order
        // (so first neighbor is processed first when popped)
        for (neighbor_id, _) in neighbors.into_iter().rev() {
            stack.push((neighbor_id, depth + 1));
        }
    }

    log::debug!(
        "DFS complete: {} nodes, max_depth={}",
        result.visited_order.len(),
        result.max_depth_reached()
    );

    Ok(result)
}

/// Get all nodes within a given depth from start using DFS.
///
/// Convenience wrapper around dfs_traverse.
pub fn dfs_neighborhood(
    storage: &GraphStorage,
    center: NodeId,
    max_depth: usize,
) -> GraphResult<Vec<NodeId>> {
    let params = DfsParams::default().max_depth(max_depth);
    let result = dfs_traverse(storage, center, params)?;
    Ok(result.visited_order)
}

/// Get nodes within depth, filtered by domain and minimum weight.
///
/// Returns only nodes reachable via edges with weight >= min_weight
/// after domain modulation.
pub fn dfs_domain_neighborhood(
    storage: &GraphStorage,
    center: NodeId,
    max_depth: usize,
    domain: Domain,
    min_weight: f32,
) -> GraphResult<Vec<NodeId>> {
    let params = DfsParams::default()
        .max_depth(max_depth)
        .domain(domain)
        .min_weight(min_weight);
    let result = dfs_traverse(storage, center, params)?;
    Ok(result.visited_order)
}
