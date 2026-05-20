//! Core GraphEdge struct and constructors.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::types::{LLMProvenance, NodeId};

/// Type alias for edge identifiers (UUID v4).
pub type EdgeId = Uuid;

/// Type of relationship between two nodes in the graph.
///
/// Each edge type represents a distinct semantic relationship with
/// different traversal and weighting characteristics:
/// - Semantic: Similarity-based connections (weight: 0.5)
/// - Temporal: Time-ordered sequences (weight: 0.7)
/// - Causal: Cause-effect relationships (weight: 0.8)
/// - Hierarchical: Parent-child taxonomies (weight: 0.9)
/// - Contradicts: Logical contradiction relationship (weight: 0.3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Semantic similarity relationship.
    /// Default weight: 0.5 (variable based on embedding similarity).
    Semantic,

    /// Temporal sequence relationship.
    /// Default weight: 0.7 (time relationships are usually reliable).
    Temporal,

    /// Causal relationship.
    /// Default weight: 0.8 (strong evidence when established).
    Causal,

    /// Hierarchical relationship.
    /// Default weight: 0.9 (taxonomy relationships are very strong).
    Hierarchical,

    /// Contradiction relationship.
    /// Default weight: 0.3 (low base weight).
    /// This edge type is symmetric: if A contradicts B, then B contradicts A.
    Contradicts,
}

impl EdgeType {
    /// Returns a human-readable description of this edge type.
    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Semantic => "Semantic similarity - nodes share similar meaning or topic",
            Self::Temporal => "Temporal sequence - source precedes target in time",
            Self::Causal => "Causal relationship - source causes or influences target",
            Self::Hierarchical => "Hierarchical - source is parent or ancestor of target",
            Self::Contradicts => "Contradiction - source logically contradicts target",
        }
    }

    /// Returns all edge type variants as an array.
    #[inline]
    pub fn all() -> [EdgeType; 5] {
        [
            Self::Semantic,
            Self::Temporal,
            Self::Causal,
            Self::Hierarchical,
            Self::Contradicts,
        ]
    }

    /// Returns the default base weight for this edge type.
    ///
    /// These weights reflect the inherent reliability of each relationship type:
    /// - Semantic (0.5): Variable based on embedding similarity
    /// - Temporal (0.7): Time relationships are usually reliable
    /// - Causal (0.8): Strong evidence when established
    /// - Hierarchical (0.9): Taxonomy relationships are very strong
    /// - Contradicts (0.3): Low base weight
    #[inline]
    pub fn default_weight(&self) -> f32 {
        match self {
            Self::Semantic => 0.5,
            Self::Temporal => 0.7,
            Self::Causal => 0.8,
            Self::Hierarchical => 0.9,
            Self::Contradicts => 0.3,
        }
    }

    /// Returns whether this edge type represents a contradiction relationship.
    #[inline]
    pub fn is_contradiction(&self) -> bool {
        matches!(self, Self::Contradicts)
    }

    /// Returns whether this edge type is symmetric.
    ///
    /// Symmetric edges work in both directions:
    /// - Semantic: A similar to B implies B similar to A
    /// - Contradicts: A contradicts B implies B contradicts A
    ///
    /// Non-symmetric (directed) edges:
    /// - Temporal: A before B does NOT imply B before A
    /// - Causal: A causes B does NOT imply B causes A
    /// - Hierarchical: A parent of B does NOT imply B parent of A
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::Semantic | Self::Contradicts)
    }

    /// Converts this edge type to its u8 representation for storage.
    ///
    /// - Semantic: 0
    /// - Temporal: 1
    /// - Causal: 2
    /// - Hierarchical: 3
    /// - Contradicts: 4
    #[inline]
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Semantic => 0,
            Self::Temporal => 1,
            Self::Causal => 2,
            Self::Hierarchical => 3,
            Self::Contradicts => 4,
        }
    }

    /// Converts a u8 value to an EdgeType.
    ///
    /// Returns `Some(EdgeType)` if value is 0-4, `None` otherwise.
    #[inline]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Semantic),
            1 => Some(Self::Temporal),
            2 => Some(Self::Causal),
            3 => Some(Self::Hierarchical),
            4 => Some(Self::Contradicts),
            _ => None,
        }
    }
}

impl Default for EdgeType {
    /// Returns `EdgeType::Semantic` as the default.
    #[inline]
    fn default() -> Self {
        Self::Semantic
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Semantic => "semantic",
            Self::Temporal => "temporal",
            Self::Causal => "causal",
            Self::Hierarchical => "hierarchical",
            Self::Contradicts => "contradicts",
        };
        write!(f, "{}", s)
    }
}

/// A directed edge between two nodes in the Context Graph.
///
/// # Fields
/// - `id`: Unique edge identifier (UUID v4)
/// - `source_id`: Source node UUID
/// - `target_id`: Target node UUID
/// - `edge_type`: Relationship type (Semantic|Temporal|Causal|Hierarchical|Contradicts)
/// - `weight`: Base edge weight [0.0, 1.0]
/// - `confidence`: Confidence in validity [0.0, 1.0]
/// - `domain`: Optional knowledge domain label
/// - `is_amortized_shortcut`: True if learned during dream consolidation
/// - `steering_reward`: Steering Subsystem feedback [-1.0, 1.0]
/// - `traversal_count`: Number of times edge was traversed
/// - `created_at`: Creation timestamp
/// - `last_traversed_at`: Last traversal timestamp (None until first traversal)
///
/// # Performance
/// - Serialized size: ~200 bytes
/// - Traversal latency target: <50us
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
    pub weight: f32,

    /// Confidence in this edge's validity [0.0, 1.0].
    pub confidence: f32,

    /// Optional knowledge domain label for context-aware retrieval.
    #[serde(default)]
    pub domain: Option<String>,

    /// Whether this edge is an amortized shortcut (learned during dreams).
    pub is_amortized_shortcut: bool,

    /// Steering reward signal from the Steering Subsystem [-1.0, 1.0].
    pub steering_reward: f32,

    /// Number of times this edge has been traversed.
    pub traversal_count: u64,

    /// Timestamp when this edge was created.
    pub created_at: DateTime<Utc>,

    /// Timestamp when this edge was last traversed.
    pub last_traversed_at: Option<DateTime<Utc>>,

    /// Retired compatibility metadata from the deleted local LLM graph agent.
    /// Active ME-JEPA code must leave this as `None`; the field remains to
    /// preserve old serialized record layout.
    #[serde(default)]
    pub discovery_provenance: Option<LLMProvenance>,
}

impl GraphEdge {
    /// Creates a new edge with default values.
    ///
    /// Initializes an edge with edge-type-appropriate base weight.
    /// The edge starts with neutral confidence (0.5) and no steering reward.
    ///
    /// # Arguments
    ///
    /// * `source_id` - Source node UUID (edge originates here)
    /// * `target_id` - Target node UUID (edge points here)
    /// * `edge_type` - Type of relationship (Semantic, Temporal, Causal, Hierarchical)
    ///
    /// # Returns
    ///
    /// New GraphEdge with:
    /// - `weight` = `edge_type.default_weight()`
    /// - `confidence` = 0.5
    /// - `steering_reward` = 0.0
    /// - `traversal_count` = 0
    /// - `is_amortized_shortcut` = false
    pub fn new(source_id: NodeId, target_id: NodeId, edge_type: EdgeType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source_id,
            target_id,
            edge_type,
            weight: edge_type.default_weight(),
            confidence: 0.5,
            domain: None,
            is_amortized_shortcut: false,
            steering_reward: 0.0,
            traversal_count: 0,
            created_at: now,
            last_traversed_at: None,
            discovery_provenance: None,
        }
    }

    /// Creates a new edge with explicit weight and confidence values.
    ///
    /// Use this constructor when you have specific weight and confidence
    /// values rather than relying on edge type defaults.
    ///
    /// # Arguments
    ///
    /// * `source_id` - Source node UUID
    /// * `target_id` - Target node UUID
    /// * `edge_type` - Type of relationship
    /// * `weight` - Base edge weight (clamped to [0.0, 1.0])
    /// * `confidence` - Confidence level (clamped to [0.0, 1.0])
    pub fn with_weight(
        source_id: NodeId,
        target_id: NodeId,
        edge_type: EdgeType,
        weight: f32,
        confidence: f32,
    ) -> Self {
        let mut edge = Self::new(source_id, target_id, edge_type);
        edge.weight = weight.clamp(0.0, 1.0);
        edge.confidence = confidence.clamp(0.0, 1.0);
        edge
    }

    /// Sets the domain label on this edge (builder pattern).
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Add retired generator provenance to an existing edge.
    pub fn with_discovery_provenance(mut self, provenance: LLMProvenance) -> Self {
        self.discovery_provenance = Some(provenance);
        self
    }
}

impl Default for GraphEdge {
    /// Creates a default edge with nil UUIDs.
    ///
    /// Uses:
    /// - source_id/target_id: Uuid::nil()
    /// - edge_type: EdgeType::Semantic (default)
    fn default() -> Self {
        Self::new(Uuid::nil(), Uuid::nil(), EdgeType::default())
    }
}

#[cfg(test)]
mod edge_type_tests {
    use super::*;

    #[test]
    fn test_edge_type_as_u8() {
        assert_eq!(EdgeType::Semantic.as_u8(), 0);
        assert_eq!(EdgeType::Temporal.as_u8(), 1);
        assert_eq!(EdgeType::Causal.as_u8(), 2);
        assert_eq!(EdgeType::Hierarchical.as_u8(), 3);
        assert_eq!(EdgeType::Contradicts.as_u8(), 4);
    }

    #[test]
    fn test_edge_type_from_u8_valid() {
        assert_eq!(EdgeType::from_u8(0), Some(EdgeType::Semantic));
        assert_eq!(EdgeType::from_u8(1), Some(EdgeType::Temporal));
        assert_eq!(EdgeType::from_u8(2), Some(EdgeType::Causal));
        assert_eq!(EdgeType::from_u8(3), Some(EdgeType::Hierarchical));
        assert_eq!(EdgeType::from_u8(4), Some(EdgeType::Contradicts));
    }

    #[test]
    fn test_edge_type_from_u8_invalid() {
        assert_eq!(EdgeType::from_u8(5), None);
        assert_eq!(EdgeType::from_u8(255), None);
    }

    #[test]
    fn test_edge_type_roundtrip() {
        for edge_type in EdgeType::all() {
            let u8_val = edge_type.as_u8();
            let recovered = EdgeType::from_u8(u8_val).expect("valid u8 should convert");
            assert_eq!(recovered, edge_type);
        }
    }

    #[test]
    fn test_default_weight() {
        assert_eq!(EdgeType::Semantic.default_weight(), 0.5);
        assert_eq!(EdgeType::Temporal.default_weight(), 0.7);
        assert_eq!(EdgeType::Causal.default_weight(), 0.8);
        assert_eq!(EdgeType::Hierarchical.default_weight(), 0.9);
        assert_eq!(EdgeType::Contradicts.default_weight(), 0.3);
    }

    #[test]
    fn test_display() {
        assert_eq!(EdgeType::Semantic.to_string(), "semantic");
        assert_eq!(EdgeType::Temporal.to_string(), "temporal");
        assert_eq!(EdgeType::Causal.to_string(), "causal");
        assert_eq!(EdgeType::Hierarchical.to_string(), "hierarchical");
        assert_eq!(EdgeType::Contradicts.to_string(), "contradicts");
    }

    #[test]
    fn test_default() {
        assert_eq!(EdgeType::default(), EdgeType::Semantic);
    }

    #[test]
    fn test_all() {
        let all = EdgeType::all();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&EdgeType::Semantic));
        assert!(all.contains(&EdgeType::Temporal));
        assert!(all.contains(&EdgeType::Causal));
        assert!(all.contains(&EdgeType::Hierarchical));
        assert!(all.contains(&EdgeType::Contradicts));
    }

    #[test]
    fn test_contradicts_is_contradiction() {
        assert!(EdgeType::Contradicts.is_contradiction());
        assert!(!EdgeType::Semantic.is_contradiction());
        assert!(!EdgeType::Temporal.is_contradiction());
        assert!(!EdgeType::Causal.is_contradiction());
        assert!(!EdgeType::Hierarchical.is_contradiction());
    }

    #[test]
    fn test_is_symmetric() {
        assert!(EdgeType::Semantic.is_symmetric());
        assert!(EdgeType::Contradicts.is_symmetric());
        assert!(!EdgeType::Temporal.is_symmetric());
        assert!(!EdgeType::Causal.is_symmetric());
        assert!(!EdgeType::Hierarchical.is_symmetric());
    }

    #[test]
    fn test_contradicts_has_low_weight() {
        assert!(EdgeType::Contradicts.default_weight() < EdgeType::Semantic.default_weight());
    }

    #[test]
    fn test_contradicts_serde() {
        let json = serde_json::to_string(&EdgeType::Contradicts).unwrap();
        assert_eq!(json, r#""contradicts""#);
        let restored: EdgeType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, EdgeType::Contradicts);
    }
}
