//! Core A* search algorithm implementation.
//!
//! Contains the main `astar_search` function.

use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::config::HyperbolicConfig;
use crate::error::{GraphError, GraphResult};
use crate::hyperbolic::PoincareBall;
use crate::storage::GraphStorage;

use super::helpers::{edge_cost, to_hyperbolic_point, uuid_to_i64};
use super::node::AstarNode;
use super::types::{AstarParams, AstarResult, NodeId};

/// Perform A* pathfinding from start to goal.
///
/// Uses hyperbolic distance in Poincare ball as heuristic.
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID
/// * `goal` - Goal node ID
/// * `params` - A* parameters
///
/// # Returns
/// * `Ok(AstarResult)` - Pathfinding result
/// * `Err(GraphError::MissingHyperbolicData)` - Node missing hyperbolic embedding
/// * `Err(GraphError::*)` - Storage error
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_graph::traversal::astar::{astar_search, AstarParams, Domain};
///
/// let params = AstarParams::default()
///     .domain(Domain::Code)
///     .min_weight(0.3);
///
/// let result = astar_search(&storage, start_node, goal_node, params)?;
/// if result.path_found {
///     println!("Path: {:?}, cost: {}", result.path, result.total_cost);
/// }
/// ```
pub fn astar_search(
    storage: &GraphStorage,
    start: NodeId,
    goal: NodeId,
    params: AstarParams,
) -> GraphResult<AstarResult> {
    // Handle trivial case
    if start == goal {
        return Ok(AstarResult::found(vec![start], 0.0, 1, 0));
    }

    // Initialize Poincare ball for hyperbolic distance
    let config = HyperbolicConfig::default();
    let ball = PoincareBall::new(config);

    // Get hyperbolic embedding for goal (required for heuristic)
    // NO FALLBACK - fail fast per AP-001
    let goal_point = to_hyperbolic_point(
        storage
            .get_hyperbolic(goal)?
            .ok_or(GraphError::MissingHyperbolicData(goal))?,
    );

    // Get start point
    let start_point = to_hyperbolic_point(
        storage
            .get_hyperbolic(start)?
            .ok_or(GraphError::MissingHyperbolicData(start))?,
    );

    // Initial heuristic
    let h_start = params.heuristic_scale * ball.distance(&start_point, &goal_point);

    // Open set (priority queue)
    let mut open_set: BinaryHeap<AstarNode> = BinaryHeap::new();
    open_set.push(AstarNode::new(start, 0.0, h_start));

    // Track best g-score for each node
    let mut g_scores: HashMap<NodeId, f32> = HashMap::new();
    g_scores.insert(start, 0.0);

    // Track parent for path reconstruction
    let mut came_from: HashMap<NodeId, NodeId> = HashMap::new();

    // Closed set (already explored)
    let mut closed_set: HashSet<NodeId> = HashSet::new();

    let mut nodes_explored = 0;

    while let Some(current) = open_set.pop() {
        let current_id = current.node_id;

        // Check if we've reached the goal
        if current_id == goal {
            // Reconstruct path
            let mut path = vec![goal];
            let mut node = goal;
            while let Some(&parent) = came_from.get(&node) {
                path.push(parent);
                node = parent;
            }
            path.reverse();

            return Ok(AstarResult::found(
                path,
                current.g_score,
                nodes_explored,
                open_set.len(),
            ));
        }

        // Skip if already explored
        if closed_set.contains(&current_id) {
            continue;
        }

        // Mark as explored
        closed_set.insert(current_id);
        nodes_explored += 1;

        // Check exploration limit
        if nodes_explored >= params.max_nodes {
            log::debug!("A* exploration limit reached: {} nodes", nodes_explored);
            return Ok(AstarResult::no_path(nodes_explored));
        }

        // Get current g-score
        let current_g = *g_scores.get(&current_id).unwrap_or(&f32::INFINITY);

        // Expand neighbors
        let edges = storage.get_outgoing_edges(current_id)?;

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

            // Convert UUID to i64
            let neighbor_id = uuid_to_i64(&edge.target);

            // Skip if already explored
            if closed_set.contains(&neighbor_id) {
                continue;
            }

            // Calculate tentative g-score
            let tentative_g = current_g + edge_cost(effective_weight);

            // Check if this is a better path
            let neighbor_g = *g_scores.get(&neighbor_id).unwrap_or(&f32::INFINITY);
            if tentative_g >= neighbor_g {
                continue; // Not a better path
            }

            // Update path
            came_from.insert(neighbor_id, current_id);
            g_scores.insert(neighbor_id, tentative_g);

            // Calculate heuristic for neighbor
            // Get hyperbolic embedding (NO FALLBACK)
            let neighbor_point = to_hyperbolic_point(
                storage
                    .get_hyperbolic(neighbor_id)?
                    .ok_or(GraphError::MissingHyperbolicData(neighbor_id))?,
            );

            let h = params.heuristic_scale * ball.distance(&neighbor_point, &goal_point);

            // Add to open set
            open_set.push(AstarNode::new(neighbor_id, tentative_g, h));
        }
    }

    // No path found
    Ok(AstarResult::no_path(nodes_explored))
}
