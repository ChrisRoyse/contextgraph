//! A* priority queue node.
//!
//! Implements ordering for BinaryHeap min-heap behavior.

use std::cmp::Ordering;

use super::types::NodeId;

/// Node in A* open set priority queue.
///
/// Uses Reverse ordering so BinaryHeap (max-heap) becomes min-heap.
#[derive(Debug, Clone)]
pub(crate) struct AstarNode {
    /// Node ID.
    pub node_id: NodeId,
    /// f(n) = g(n) + h(n).
    pub f_score: f32,
    /// g(n) = cost from start to this node.
    pub g_score: f32,
}

impl AstarNode {
    /// Create a new A* node with computed f-score.
    pub fn new(node_id: NodeId, g_score: f32, h_score: f32) -> Self {
        Self {
            node_id,
            f_score: g_score + h_score,
            g_score,
        }
    }
}

// Ordering for min-heap (smaller f_score = higher priority)
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
        // Handle NaN by treating it as greater than everything
        match (self.f_score.is_nan(), other.f_score.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less, // NaN goes to bottom of heap
            (false, true) => Ordering::Greater,
            (false, false) => {
                // Reverse for min-heap
                other
                    .f_score
                    .partial_cmp(&self.f_score)
                    .unwrap_or(Ordering::Equal)
            }
        }
    }
}
