//! Core GraphEdge struct and constructors.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
use crate::types::{LLMProvenance, NodeId};

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
/// # Performance
/// - Serialized size: ~200 bytes
/// - Traversal latency target: <50μs
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

    /// Knowledge domain this edge belongs to.
    pub domain: Domain,

    /// Neurotransmitter weights for modulation.
    /// Applied via: w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)
    pub neurotransmitter_weights: NeurotransmitterWeights,

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

    /// LLM provenance metadata for how this edge was discovered (Phase 1.3).
    /// Populated by the graph discovery agent when it creates edges via LLM analysis.
    #[serde(default)]
    pub discovery_provenance: Option<LLMProvenance>,
}

impl GraphEdge {
    /// Creates a new edge with default values for the given domain.
    ///
    /// Initializes an edge with domain-appropriate neurotransmitter weights
    /// and edge-type-appropriate base weight. The edge starts with neutral
    /// confidence (0.5) and no steering reward.
    ///
    /// # Arguments
    ///
    /// * `source_id` - Source node UUID (edge originates here)
    /// * `target_id` - Target node UUID (edge points here)
    /// * `edge_type` - Type of relationship (Semantic, Temporal, Causal, Hierarchical)
    /// * `domain` - Knowledge domain for NT weight profiles
    ///
    /// # Returns
    ///
    /// New GraphEdge with:
    /// - `weight` = `edge_type.default_weight()`
    /// - `confidence` = 0.5
    /// - `neurotransmitter_weights` = domain-specific profile
    /// - `steering_reward` = 0.0
    /// - `traversal_count` = 0
    /// - `is_amortized_shortcut` = false
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    /// use context_graph_core::types::GraphEdge;
    /// use context_graph_core::marblestone::{EdgeType, Domain};
    ///
    /// let source = Uuid::new_v4();
    /// let target = Uuid::new_v4();
    ///
    /// let edge = GraphEdge::new(source, target, EdgeType::Causal, Domain::Code);
    ///
    /// assert_eq!(edge.source_id, source);
    /// assert_eq!(edge.target_id, target);
    /// assert_eq!(edge.edge_type, EdgeType::Causal);
    /// assert_eq!(edge.weight, 0.8); // Causal default
    /// assert_eq!(edge.confidence, 0.5);
    /// assert_eq!(edge.steering_reward, 0.0);
    /// ```
    pub fn new(source_id: NodeId, target_id: NodeId, edge_type: EdgeType, domain: Domain) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source_id,
            target_id,
            edge_type,
            weight: edge_type.default_weight(),
            confidence: 0.5,
            domain,
            neurotransmitter_weights: NeurotransmitterWeights::for_domain(domain),
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
    /// * `domain` - Knowledge domain
    /// * `weight` - Base edge weight (clamped to [0.0, 1.0])
    /// * `confidence` - Confidence level (clamped to [0.0, 1.0])
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    /// use context_graph_core::types::GraphEdge;
    /// use context_graph_core::marblestone::{EdgeType, Domain};
    ///
    /// let source = Uuid::new_v4();
    /// let target = Uuid::new_v4();
    ///
    /// // Create edge with custom weight and high confidence
    /// let edge = GraphEdge::with_weight(
    ///     source,
    ///     target,
    ///     EdgeType::Semantic,
    ///     Domain::Research,
    ///     0.95,  // High semantic similarity
    ///     0.85,  // High confidence
    /// );
    ///
    /// assert_eq!(edge.weight, 0.95);
    /// assert_eq!(edge.confidence, 0.85);
    ///
    /// // Values are clamped to valid range
    /// let clamped = GraphEdge::with_weight(
    ///     source,
    ///     target,
    ///     EdgeType::Semantic,
    ///     Domain::General,
    ///     1.5,   // Will be clamped to 1.0
    ///     -0.2,  // Will be clamped to 0.0
    /// );
    /// assert_eq!(clamped.weight, 1.0);
    /// assert_eq!(clamped.confidence, 0.0);
    /// ```
    pub fn with_weight(
        source_id: NodeId,
        target_id: NodeId,
        edge_type: EdgeType,
        domain: Domain,
        weight: f32,
        confidence: f32,
    ) -> Self {
        let mut edge = Self::new(source_id, target_id, edge_type, domain);
        edge.weight = weight.clamp(0.0, 1.0);
        edge.confidence = confidence.clamp(0.0, 1.0);
        edge
    }

    /// Add LLM discovery provenance to an existing edge.
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
    /// - domain: Domain::General (default)
    fn default() -> Self {
        Self::new(
            Uuid::nil(),
            Uuid::nil(),
            EdgeType::default(),
            Domain::default(),
        )
    }
}
