//! Bidirectional A* search algorithm.
//!
//! Searches from both start and goal simultaneously for improved performance.

use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::config::HyperbolicConfig;
use crate::error::{GraphError, GraphResult};
use crate::hyperbolic::PoincareBall;
use crate::storage::GraphStorage;

use super::helpers::{edge_cost, to_hyperbolic_point, uuid_to_i64};
use super::node::AstarNode;
use super::types::{AstarParams, AstarResult, NodeId};

/// A* with bidirectional search optimization.
///
/// Searches from both start and goal simultaneously, meeting in the middle.
/// Can be ~2x faster than unidirectional A* for long paths.
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `start` - Starting node ID
/// * `goal` - Goal node ID
/// * `params` - A* parameters
///
/// # Returns
/// Same as `astar_search`
pub fn astar_bidirectional(
    storage: &GraphStorage,
    start: NodeId,
    goal: NodeId,
    params: AstarParams,
) -> GraphResult<AstarResult> {
    // Handle trivial case
    if start == goal {
        return Ok(AstarResult::found(vec![start], 0.0, 1, 0));
    }

    // Initialize Poincare ball
    let config = HyperbolicConfig::default();
    let ball = PoincareBall::new(config);

    // Get hyperbolic embeddings (NO FALLBACK)
    let start_point = to_hyperbolic_point(
        storage
            .get_hyperbolic(start)?
            .ok_or(GraphError::MissingHyperbolicData(start))?,
    );
    let goal_point = to_hyperbolic_point(
        storage
            .get_hyperbolic(goal)?
            .ok_or(GraphError::MissingHyperbolicData(goal))?,
    );

    // Forward search state
    let mut forward_open: BinaryHeap<AstarNode> = BinaryHeap::new();
    let mut forward_g: HashMap<NodeId, f32> = HashMap::new();
    let mut forward_parent: HashMap<NodeId, NodeId> = HashMap::new();
    let mut forward_closed: HashSet<NodeId> = HashSet::new();

    let h_start = params.heuristic_scale * ball.distance(&start_point, &goal_point);
    forward_open.push(AstarNode::new(start, 0.0, h_start));
    forward_g.insert(start, 0.0);

    // Backward search state
    let mut backward_open: BinaryHeap<AstarNode> = BinaryHeap::new();
    let mut backward_g: HashMap<NodeId, f32> = HashMap::new();
    let mut backward_parent: HashMap<NodeId, NodeId> = HashMap::new();
    let mut backward_closed: HashSet<NodeId> = HashSet::new();

    let h_goal = params.heuristic_scale * ball.distance(&goal_point, &start_point);
    backward_open.push(AstarNode::new(goal, 0.0, h_goal));
    backward_g.insert(goal, 0.0);

    let mut best_path_cost = f32::INFINITY;
    let mut meeting_node: Option<NodeId> = None;
    let mut nodes_explored = 0;

    // Alternate between forward and backward search
    let mut forward_turn = true;

    loop {
        // Check termination
        if forward_open.is_empty() && backward_open.is_empty() {
            break;
        }

        if nodes_explored >= params.max_nodes {
            log::debug!("Bidirectional A* limit reached: {} nodes", nodes_explored);
            break;
        }

        // Choose direction
        let (open_set, g_scores, parent_map, closed_set, other_closed, other_g, target_point) =
            if forward_turn && !forward_open.is_empty() {
                (
                    &mut forward_open,
                    &mut forward_g,
                    &mut forward_parent,
                    &mut forward_closed,
                    &backward_closed,
                    &backward_g,
                    &goal_point,
                )
            } else if !backward_open.is_empty() {
                (
                    &mut backward_open,
                    &mut backward_g,
                    &mut backward_parent,
                    &mut backward_closed,
                    &forward_closed,
                    &forward_g,
                    &start_point,
                )
            } else if !forward_open.is_empty() {
                (
                    &mut forward_open,
                    &mut forward_g,
                    &mut forward_parent,
                    &mut forward_closed,
                    &backward_closed,
                    &backward_g,
                    &goal_point,
                )
            } else {
                break;
            };

        forward_turn = !forward_turn;

        // Pop from open set
        let Some(current) = open_set.pop() else {
            continue;
        };

        let current_id = current.node_id;

        // Skip if explored
        if closed_set.contains(&current_id) {
            continue;
        }

        closed_set.insert(current_id);
        nodes_explored += 1;

        // Check if other search has reached this node
        if other_closed.contains(&current_id) {
            let this_cost = *g_scores.get(&current_id).unwrap_or(&f32::INFINITY);
            let other_cost = *other_g.get(&current_id).unwrap_or(&f32::INFINITY);
            let path_cost = this_cost + other_cost;

            if path_cost < best_path_cost {
                best_path_cost = path_cost;
                meeting_node = Some(current_id);
            }
        }

        // Early termination: if best f-score > best path, we're done
        if current.f_score >= best_path_cost {
            break;
        }

        // Get current g-score
        let current_g = *g_scores.get(&current_id).unwrap_or(&f32::INFINITY);

        // Expand neighbors
        let edges = storage.get_outgoing_edges(current_id)?;

        for edge in edges {
            // Filter by edge type
            if let Some(ref allowed_types) = params.edge_types {
                if !allowed_types.contains(&edge.edge_type) {
                    continue;
                }
            }

            // Get modulated weight
            let effective_weight = edge.get_modulated_weight(params.domain);

            // Filter by minimum weight
            if effective_weight < params.min_weight {
                continue;
            }

            let neighbor_id = uuid_to_i64(&edge.target);

            if closed_set.contains(&neighbor_id) {
                continue;
            }

            let tentative_g = current_g + edge_cost(effective_weight);
            let neighbor_g = *g_scores.get(&neighbor_id).unwrap_or(&f32::INFINITY);

            if tentative_g >= neighbor_g {
                continue;
            }

            parent_map.insert(neighbor_id, current_id);
            g_scores.insert(neighbor_id, tentative_g);

            // Get heuristic
            let neighbor_point = to_hyperbolic_point(
                storage
                    .get_hyperbolic(neighbor_id)?
                    .ok_or(GraphError::MissingHyperbolicData(neighbor_id))?,
            );

            let h = params.heuristic_scale * ball.distance(&neighbor_point, target_point);
            open_set.push(AstarNode::new(neighbor_id, tentative_g, h));

            // Check if meets other search
            if let Some(&other_cost) = other_g.get(&neighbor_id) {
                let path_cost = tentative_g + other_cost;
                if path_cost < best_path_cost {
                    best_path_cost = path_cost;
                    meeting_node = Some(neighbor_id);
                }
            }
        }
    }

    // Reconstruct path if found
    if let Some(meet) = meeting_node {
        let mut forward_path = vec![meet];
        let mut node = meet;
        while let Some(&parent) = forward_parent.get(&node) {
            forward_path.push(parent);
            node = parent;
        }
        forward_path.reverse();

        let mut backward_path = Vec::new();
        node = meet;
        while let Some(&parent) = backward_parent.get(&node) {
            backward_path.push(parent);
            node = parent;
        }

        // Combine paths (forward_path ends with meet, backward_path starts after meet)
        forward_path.extend(backward_path);

        return Ok(AstarResult::found(
            forward_path,
            best_path_cost,
            nodes_explored,
            forward_open.len() + backward_open.len(),
        ));
    }

    Ok(AstarResult::no_path(nodes_explored))
}
