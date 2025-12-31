//! Graph edge connecting two memory nodes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::NodeId;
use crate::marblestone::EdgeType;

/// Unique identifier for graph edges
pub type EdgeId = Uuid;

/// Graph edge connecting two memory nodes.
///
/// Edges represent relationships between nodes such as semantic similarity,
/// temporal sequences, causal relationships, and more.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    /// Unique edge identifier
    pub id: EdgeId,

    /// Source node ID
    pub source: NodeId,

    /// Target node ID
    pub target: NodeId,

    /// Edge relationship type
    pub edge_type: EdgeType,

    /// Edge weight/strength [0.0, 1.0]
    pub weight: f32,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Edge metadata
    pub metadata: EdgeMetadata,
}

impl GraphEdge {
    /// Create a new edge between two nodes.
    pub fn new(source: NodeId, target: NodeId, edge_type: EdgeType, weight: f32) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            target,
            edge_type,
            weight: weight.clamp(0.0, 1.0),
            created_at: Utc::now(),
            metadata: EdgeMetadata::default(),
        }
    }
}

/// Edge metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EdgeMetadata {
    /// Confidence in relationship
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// Explanation of relationship
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_creation() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let edge = GraphEdge::new(source, target, EdgeType::Semantic, 0.8);

        assert_eq!(edge.source, source);
        assert_eq!(edge.target, target);
        assert_eq!(edge.edge_type, EdgeType::Semantic);
        assert_eq!(edge.weight, 0.8);
    }

    #[test]
    fn test_weight_clamping() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let edge1 = GraphEdge::new(source, target, EdgeType::Semantic, 1.5);
        assert_eq!(edge1.weight, 1.0);

        let edge2 = GraphEdge::new(source, target, EdgeType::Semantic, -0.5);
        assert_eq!(edge2.weight, 0.0);
    }
}
