//! Graph edge connecting two memory nodes with Marblestone architecture support.
//!
//! This module provides the GraphEdge struct which represents directed relationships
//! between MemoryNodes in the Context Graph. It implements the Marblestone architecture
//! features for neurotransmitter-based weight modulation, amortized shortcuts, and
//! steering rewards.
//!
//! # Constitution Reference
//! - edge_model: Full edge specification
//! - edge_model.nt_weights: Neurotransmitter modulation formula
//! - edge_model.amortized: Shortcut learning criteria
//! - edge_model.steering_reward: [-1,1] reward signal

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::NodeId;
use crate::marblestone::{Domain, EdgeType, NeurotransmitterWeights};

/// Type alias for edge identifiers (UUID v4).
pub type EdgeId = Uuid;

/// A directed edge between two nodes in the Context Graph.
///
/// Implements Marblestone architecture features:
/// - Neurotransmitter-based weight modulation
/// - Amortized shortcuts (learned during dream consolidation)
/// - Steering rewards for reinforcement learning
///
/// # Fields
/// All 13 fields per PRD Section 4.2 and constitution.yaml edge_model:
/// - `id`: Unique edge identifier (UUID v4)
/// - `source_id`: Source node UUID
/// - `target_id`: Target node UUID
/// - `edge_type`: Relationship type (Semantic|Temporal|Causal|Hierarchical)
/// - `weight`: Base edge weight [0.0, 1.0]
/// - `confidence`: Confidence in validity [0.0, 1.0]
/// - `domain`: Knowledge domain for context-aware retrieval
/// - `neurotransmitter_weights`: NT modulation weights
/// - `is_amortized_shortcut`: True if learned during dream consolidation
/// - `steering_reward`: Steering Subsystem feedback [-1.0, 1.0]
/// - `traversal_count`: Number of times edge was traversed
/// - `created_at`: Creation timestamp
/// - `last_traversed_at`: Last traversal timestamp (None until first traversal)
///
/// # Performance Characteristics
/// - Serialized size: ~200 bytes
/// - Traversal latency target: <50μs
///
/// # Example
/// ```rust
/// use context_graph_core::types::GraphEdge;
/// use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
/// use uuid::Uuid;
///
/// let source = Uuid::new_v4();
/// let target = Uuid::new_v4();
///
/// // Create edge with all Marblestone fields
/// let edge = GraphEdge {
///     id: Uuid::new_v4(),
///     source_id: source,
///     target_id: target,
///     edge_type: EdgeType::Causal,
///     weight: 0.8,
///     confidence: 0.9,
///     domain: Domain::Code,
///     neurotransmitter_weights: NeurotransmitterWeights::for_domain(Domain::Code),
///     is_amortized_shortcut: false,
///     steering_reward: 0.0,
///     traversal_count: 0,
///     created_at: chrono::Utc::now(),
///     last_traversed_at: None,
/// };
///
/// assert!(edge.weight >= 0.0 && edge.weight <= 1.0);
/// assert!(edge.confidence >= 0.0 && edge.confidence <= 1.0);
/// assert!(edge.steering_reward >= -1.0 && edge.steering_reward <= 1.0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Unique identifier for this edge (UUID v4).
    pub id: EdgeId,

    /// Source node ID (edge starts here).
    pub source_id: NodeId,

    /// Target node ID (edge ends here).
    pub target_id: NodeId,

    /// Type of relationship this edge represents.
    pub edge_type: EdgeType,

    /// Base weight of the edge [0.0, 1.0].
    /// Higher weight = stronger connection.
    pub weight: f32,

    /// Confidence in this edge's validity [0.0, 1.0].
    /// Higher confidence = more reliable relationship.
    pub confidence: f32,

    /// Knowledge domain this edge belongs to.
    /// Used for context-aware retrieval weighting.
    pub domain: Domain,

    /// Neurotransmitter weights for modulation.
    /// Applied via: w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)
    pub neurotransmitter_weights: NeurotransmitterWeights,

    /// Whether this edge is an amortized shortcut (learned during dreams).
    /// Shortcuts are created when 3+ hop paths are traversed ≥5 times.
    pub is_amortized_shortcut: bool,

    /// Steering reward signal from the Steering Subsystem [-1.0, 1.0].
    /// Positive = reinforce, Negative = discourage, Zero = neutral.
    pub steering_reward: f32,

    /// Number of times this edge has been traversed.
    /// Used for amortized shortcut detection.
    pub traversal_count: u64,

    /// Timestamp when this edge was created.
    pub created_at: DateTime<Utc>,

    /// Timestamp when this edge was last traversed.
    /// None until the first traversal occurs.
    pub last_traversed_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // =========================================================================
    // Struct Field Existence Tests
    // =========================================================================

    #[test]
    fn test_graph_edge_has_all_13_fields() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // This test verifies all 13 fields compile and are accessible
        let edge = GraphEdge {
            id: Uuid::new_v4(),
            source_id: source,
            target_id: target,
            edge_type: EdgeType::Semantic,
            weight: 0.5,
            confidence: 0.8,
            domain: Domain::General,
            neurotransmitter_weights: NeurotransmitterWeights::default(),
            is_amortized_shortcut: false,
            steering_reward: 0.0,
            traversal_count: 0,
            created_at: Utc::now(),
            last_traversed_at: None,
        };

        // Verify all fields are accessible
        let _id: EdgeId = edge.id;
        let _src: NodeId = edge.source_id;
        let _tgt: NodeId = edge.target_id;
        let _et: EdgeType = edge.edge_type;
        let _w: f32 = edge.weight;
        let _c: f32 = edge.confidence;
        let _d: Domain = edge.domain;
        let _nt: NeurotransmitterWeights = edge.neurotransmitter_weights;
        let _short: bool = edge.is_amortized_shortcut;
        let _sr: f32 = edge.steering_reward;
        let _tc: u64 = edge.traversal_count;
        let _ca: DateTime<Utc> = edge.created_at;
        let _lt: Option<DateTime<Utc>> = edge.last_traversed_at;
    }

    #[test]
    fn test_edge_id_is_uuid() {
        let edge_id: EdgeId = Uuid::new_v4();
        assert_eq!(edge_id.get_version_num(), 4);
    }

    // =========================================================================
    // Field Type Tests
    // =========================================================================

    #[test]
    fn test_source_id_is_node_id() {
        let source: NodeId = Uuid::new_v4();
        let edge = create_test_edge();
        let _: NodeId = edge.source_id;
        assert_ne!(source, edge.source_id); // Just verifying type compatibility
    }

    #[test]
    fn test_target_id_is_node_id() {
        let edge = create_test_edge();
        let _: NodeId = edge.target_id;
    }

    #[test]
    fn test_edge_type_uses_marblestone_enum() {
        let edge = create_test_edge();
        // Verify it's the Marblestone EdgeType (has default_weight method)
        let _weight = edge.edge_type.default_weight();
    }

    #[test]
    fn test_domain_uses_marblestone_enum() {
        let edge = create_test_edge();
        // Verify it's the Marblestone Domain (has description method)
        let _desc = edge.domain.description();
    }

    #[test]
    fn test_nt_weights_uses_marblestone_struct() {
        let edge = create_test_edge();
        // Verify it's the Marblestone NeurotransmitterWeights
        let _eff = edge.neurotransmitter_weights.compute_effective_weight(0.5);
    }

    // =========================================================================
    // Serde Serialization Tests
    // =========================================================================

    #[test]
    fn test_serde_roundtrip() {
        let edge = create_test_edge();
        let json = serde_json::to_string(&edge).expect("serialize failed");
        let restored: GraphEdge = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(edge, restored);
    }

    #[test]
    fn test_serde_json_contains_all_fields() {
        let edge = create_test_edge();
        let json = serde_json::to_string(&edge).unwrap();

        // Verify all field names appear in JSON
        assert!(json.contains("\"id\""), "JSON missing id field");
        assert!(json.contains("\"source_id\""), "JSON missing source_id field");
        assert!(json.contains("\"target_id\""), "JSON missing target_id field");
        assert!(json.contains("\"edge_type\""), "JSON missing edge_type field");
        assert!(json.contains("\"weight\""), "JSON missing weight field");
        assert!(json.contains("\"confidence\""), "JSON missing confidence field");
        assert!(json.contains("\"domain\""), "JSON missing domain field");
        assert!(json.contains("\"neurotransmitter_weights\""), "JSON missing neurotransmitter_weights field");
        assert!(json.contains("\"is_amortized_shortcut\""), "JSON missing is_amortized_shortcut field");
        assert!(json.contains("\"steering_reward\""), "JSON missing steering_reward field");
        assert!(json.contains("\"traversal_count\""), "JSON missing traversal_count field");
        assert!(json.contains("\"created_at\""), "JSON missing created_at field");
        assert!(json.contains("\"last_traversed_at\""), "JSON missing last_traversed_at field");
    }

    #[test]
    fn test_serde_with_last_traversed_at_some() {
        let mut edge = create_test_edge();
        edge.last_traversed_at = Some(Utc::now());

        let json = serde_json::to_string(&edge).unwrap();
        let restored: GraphEdge = serde_json::from_str(&json).unwrap();

        assert!(restored.last_traversed_at.is_some());
    }

    #[test]
    fn test_serde_with_last_traversed_at_none() {
        let edge = create_test_edge();
        assert!(edge.last_traversed_at.is_none());

        let json = serde_json::to_string(&edge).unwrap();
        let restored: GraphEdge = serde_json::from_str(&json).unwrap();

        assert!(restored.last_traversed_at.is_none());
    }

    #[test]
    fn test_serde_edge_type_snake_case() {
        let mut edge = create_test_edge();
        edge.edge_type = EdgeType::Hierarchical;

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"hierarchical\""), "EdgeType should serialize to snake_case");
    }

    #[test]
    fn test_serde_domain_snake_case() {
        let mut edge = create_test_edge();
        edge.domain = Domain::Medical;

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"medical\""), "Domain should serialize to snake_case");
    }

    // =========================================================================
    // Derive Trait Tests
    // =========================================================================

    #[test]
    fn test_debug_format() {
        let edge = create_test_edge();
        let debug = format!("{:?}", edge);
        assert!(debug.contains("GraphEdge"));
        assert!(debug.contains("source_id"));
        assert!(debug.contains("target_id"));
    }

    #[test]
    fn test_clone() {
        let edge = create_test_edge();
        let cloned = edge.clone();
        assert_eq!(edge, cloned);
    }

    #[test]
    fn test_partial_eq() {
        let edge1 = create_test_edge();
        let edge2 = edge1.clone();
        assert_eq!(edge1, edge2);

        let mut edge3 = edge1.clone();
        edge3.weight = 0.9;
        assert_ne!(edge1, edge3);
    }

    // =========================================================================
    // Field Value Range Tests
    // =========================================================================

    #[test]
    fn test_weight_boundary_zero() {
        let mut edge = create_test_edge();
        edge.weight = 0.0;
        assert_eq!(edge.weight, 0.0);
    }

    #[test]
    fn test_weight_boundary_one() {
        let mut edge = create_test_edge();
        edge.weight = 1.0;
        assert_eq!(edge.weight, 1.0);
    }

    #[test]
    fn test_confidence_boundary_zero() {
        let mut edge = create_test_edge();
        edge.confidence = 0.0;
        assert_eq!(edge.confidence, 0.0);
    }

    #[test]
    fn test_confidence_boundary_one() {
        let mut edge = create_test_edge();
        edge.confidence = 1.0;
        assert_eq!(edge.confidence, 1.0);
    }

    #[test]
    fn test_steering_reward_boundary_negative_one() {
        let mut edge = create_test_edge();
        edge.steering_reward = -1.0;
        assert_eq!(edge.steering_reward, -1.0);
    }

    #[test]
    fn test_steering_reward_boundary_positive_one() {
        let mut edge = create_test_edge();
        edge.steering_reward = 1.0;
        assert_eq!(edge.steering_reward, 1.0);
    }

    #[test]
    fn test_steering_reward_zero_is_neutral() {
        let edge = create_test_edge();
        assert_eq!(edge.steering_reward, 0.0);
    }

    #[test]
    fn test_traversal_count_starts_at_zero() {
        let edge = create_test_edge();
        assert_eq!(edge.traversal_count, 0);
    }

    #[test]
    fn test_is_amortized_shortcut_defaults_false() {
        let edge = create_test_edge();
        assert!(!edge.is_amortized_shortcut);
    }

    #[test]
    fn test_is_amortized_shortcut_can_be_true() {
        let mut edge = create_test_edge();
        edge.is_amortized_shortcut = true;
        assert!(edge.is_amortized_shortcut);
    }

    // =========================================================================
    // All EdgeType Variants Test
    // =========================================================================

    #[test]
    fn test_all_edge_types_work() {
        for edge_type in EdgeType::all() {
            let mut edge = create_test_edge();
            edge.edge_type = edge_type;

            let json = serde_json::to_string(&edge).unwrap();
            let restored: GraphEdge = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.edge_type, edge_type);
        }
    }

    // =========================================================================
    // All Domain Variants Test
    // =========================================================================

    #[test]
    fn test_all_domains_work() {
        for domain in Domain::all() {
            let mut edge = create_test_edge();
            edge.domain = domain;
            edge.neurotransmitter_weights = NeurotransmitterWeights::for_domain(domain);

            let json = serde_json::to_string(&edge).unwrap();
            let restored: GraphEdge = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.domain, domain);
        }
    }

    // =========================================================================
    // Timestamp Tests
    // =========================================================================

    #[test]
    fn test_created_at_is_required() {
        let edge = create_test_edge();
        let _: DateTime<Utc> = edge.created_at;
    }

    #[test]
    fn test_timestamps_preserved_through_serde() {
        let edge = create_test_edge();
        let original_created = edge.created_at;

        let json = serde_json::to_string(&edge).unwrap();
        let restored: GraphEdge = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.created_at, original_created);
    }

    // =========================================================================
    // UUID Tests
    // =========================================================================

    #[test]
    fn test_id_is_v4_uuid() {
        let edge = create_test_edge();
        assert_eq!(edge.id.get_version_num(), 4);
    }

    #[test]
    fn test_source_and_target_are_different() {
        let edge = create_test_edge();
        assert_ne!(edge.source_id, edge.target_id, "Source and target should be different UUIDs");
    }

    // =========================================================================
    // Helper Function
    // =========================================================================

    fn create_test_edge() -> GraphEdge {
        GraphEdge {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            edge_type: EdgeType::Semantic,
            weight: 0.5,
            confidence: 0.8,
            domain: Domain::General,
            neurotransmitter_weights: NeurotransmitterWeights::default(),
            is_amortized_shortcut: false,
            steering_reward: 0.0,
            traversal_count: 0,
            created_at: Utc::now(),
            last_traversed_at: None,
        }
    }
}
