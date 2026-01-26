//! Configurable edge detection thresholds.
//!
//! Each edge type has a configurable similarity threshold that determines
//! when two nodes are considered connected by that edge type.

use serde::{Deserialize, Serialize};

use super::GraphLinkEdgeType;

/// Edge detection thresholds for each edge type.
///
/// # Examples
///
/// ```
/// use context_graph_core::graph_linking::{EdgeThresholds, GraphLinkEdgeType, DEFAULT_THRESHOLDS};
///
/// // Use default thresholds
/// let thresholds = DEFAULT_THRESHOLDS;
/// assert_eq!(thresholds.get(GraphLinkEdgeType::SemanticSimilar), 0.75);
///
/// // Create custom thresholds
/// let custom = EdgeThresholds::builder()
///     .semantic_similar(0.80)
///     .code_related(0.65)
///     .build();
/// assert_eq!(custom.get(GraphLinkEdgeType::SemanticSimilar), 0.80);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeThresholds {
    /// E1 semantic similarity threshold (default: 0.75)
    pub semantic_similar: f32,
    /// E7 code similarity threshold (default: 0.70)
    pub code_related: f32,
    /// E11 entity similarity threshold (default: 0.65)
    pub entity_shared: f32,
    /// E5 causal similarity threshold (default: 0.60)
    pub causal_chain: f32,
    /// E8 graph connectivity threshold (default: 0.60)
    pub graph_connected: f32,
    /// E10 intent similarity threshold (default: 0.70)
    pub intent_aligned: f32,
    /// E6/E13 keyword overlap threshold (default: 0.50)
    pub keyword_overlap: f32,
    /// Multi-agreement threshold (default: 0.60)
    pub multi_agreement: f32,
    /// Minimum embedders required for multi_agreement (default: 3)
    pub multi_agreement_min_embedders: u8,
}

impl EdgeThresholds {
    /// Create a new builder for custom thresholds.
    pub fn builder() -> EdgeThresholdsBuilder {
        EdgeThresholdsBuilder::new()
    }

    /// Get the threshold for a specific edge type.
    pub fn get(&self, edge_type: GraphLinkEdgeType) -> f32 {
        match edge_type {
            GraphLinkEdgeType::SemanticSimilar => self.semantic_similar,
            GraphLinkEdgeType::CodeRelated => self.code_related,
            GraphLinkEdgeType::EntityShared => self.entity_shared,
            GraphLinkEdgeType::CausalChain => self.causal_chain,
            GraphLinkEdgeType::GraphConnected => self.graph_connected,
            GraphLinkEdgeType::IntentAligned => self.intent_aligned,
            GraphLinkEdgeType::KeywordOverlap => self.keyword_overlap,
            GraphLinkEdgeType::MultiAgreement => self.multi_agreement,
        }
    }

    /// Check if a similarity score exceeds the threshold for an edge type.
    #[inline]
    pub fn exceeds(&self, edge_type: GraphLinkEdgeType, similarity: f32) -> bool {
        similarity >= self.get(edge_type)
    }

    /// Get all thresholds as an array indexed by edge type.
    pub fn as_array(&self) -> [f32; 8] {
        [
            self.semantic_similar,
            self.code_related,
            self.entity_shared,
            self.causal_chain,
            self.graph_connected,
            self.intent_aligned,
            self.keyword_overlap,
            self.multi_agreement,
        ]
    }
}

impl Default for EdgeThresholds {
    fn default() -> Self {
        DEFAULT_THRESHOLDS
    }
}

/// Default edge detection thresholds.
///
/// These are tuned for the 13-embedder system:
/// - Higher thresholds for broad embedders (E1 semantic)
/// - Lower thresholds for specialized embedders (E6/E13 keyword)
pub const DEFAULT_THRESHOLDS: EdgeThresholds = EdgeThresholds {
    semantic_similar: 0.75,       // E1 - broad, need higher threshold
    code_related: 0.70,           // E7 - code-specific
    entity_shared: 0.65,          // E11 - entity matching
    causal_chain: 0.60,           // E5 - causal chains (asymmetric)
    graph_connected: 0.60,        // E8 - graph structure (asymmetric)
    intent_aligned: 0.70,         // E10 - intent matching
    keyword_overlap: 0.50,        // E6/E13 - sparse similarity scores differently
    multi_agreement: 0.60,        // Multiple agree = strong signal
    multi_agreement_min_embedders: 3, // Need 3+ embedders to agree
};

/// Builder for custom edge thresholds.
#[derive(Debug, Clone)]
pub struct EdgeThresholdsBuilder {
    thresholds: EdgeThresholds,
}

impl EdgeThresholdsBuilder {
    /// Create a new builder starting from default thresholds.
    pub fn new() -> Self {
        Self {
            thresholds: DEFAULT_THRESHOLDS,
        }
    }

    /// Set the semantic_similar (E1) threshold.
    pub fn semantic_similar(mut self, threshold: f32) -> Self {
        self.thresholds.semantic_similar = threshold;
        self
    }

    /// Set the code_related (E7) threshold.
    pub fn code_related(mut self, threshold: f32) -> Self {
        self.thresholds.code_related = threshold;
        self
    }

    /// Set the entity_shared (E11) threshold.
    pub fn entity_shared(mut self, threshold: f32) -> Self {
        self.thresholds.entity_shared = threshold;
        self
    }

    /// Set the causal_chain (E5) threshold.
    pub fn causal_chain(mut self, threshold: f32) -> Self {
        self.thresholds.causal_chain = threshold;
        self
    }

    /// Set the graph_connected (E8) threshold.
    pub fn graph_connected(mut self, threshold: f32) -> Self {
        self.thresholds.graph_connected = threshold;
        self
    }

    /// Set the intent_aligned (E10) threshold.
    pub fn intent_aligned(mut self, threshold: f32) -> Self {
        self.thresholds.intent_aligned = threshold;
        self
    }

    /// Set the keyword_overlap (E6/E13) threshold.
    pub fn keyword_overlap(mut self, threshold: f32) -> Self {
        self.thresholds.keyword_overlap = threshold;
        self
    }

    /// Set the multi_agreement threshold.
    pub fn multi_agreement(mut self, threshold: f32) -> Self {
        self.thresholds.multi_agreement = threshold;
        self
    }

    /// Set the minimum embedders required for multi_agreement.
    pub fn multi_agreement_min_embedders(mut self, min: u8) -> Self {
        self.thresholds.multi_agreement_min_embedders = min;
        self
    }

    /// Build the EdgeThresholds.
    pub fn build(self) -> EdgeThresholds {
        self.thresholds
    }
}

impl Default for EdgeThresholdsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_thresholds() {
        let thresholds = EdgeThresholds::default();
        assert_eq!(thresholds.semantic_similar, 0.75);
        assert_eq!(thresholds.code_related, 0.70);
        assert_eq!(thresholds.entity_shared, 0.65);
        assert_eq!(thresholds.causal_chain, 0.60);
        assert_eq!(thresholds.graph_connected, 0.60);
        assert_eq!(thresholds.intent_aligned, 0.70);
        assert_eq!(thresholds.keyword_overlap, 0.50);
        assert_eq!(thresholds.multi_agreement, 0.60);
        assert_eq!(thresholds.multi_agreement_min_embedders, 3);
    }

    #[test]
    fn test_get_by_edge_type() {
        let thresholds = DEFAULT_THRESHOLDS;

        assert_eq!(thresholds.get(GraphLinkEdgeType::SemanticSimilar), 0.75);
        assert_eq!(thresholds.get(GraphLinkEdgeType::CodeRelated), 0.70);
        assert_eq!(thresholds.get(GraphLinkEdgeType::EntityShared), 0.65);
        assert_eq!(thresholds.get(GraphLinkEdgeType::CausalChain), 0.60);
        assert_eq!(thresholds.get(GraphLinkEdgeType::GraphConnected), 0.60);
        assert_eq!(thresholds.get(GraphLinkEdgeType::IntentAligned), 0.70);
        assert_eq!(thresholds.get(GraphLinkEdgeType::KeywordOverlap), 0.50);
        assert_eq!(thresholds.get(GraphLinkEdgeType::MultiAgreement), 0.60);
    }

    #[test]
    fn test_exceeds() {
        let thresholds = DEFAULT_THRESHOLDS;

        // Above threshold
        assert!(thresholds.exceeds(GraphLinkEdgeType::SemanticSimilar, 0.80));
        // At threshold
        assert!(thresholds.exceeds(GraphLinkEdgeType::SemanticSimilar, 0.75));
        // Below threshold
        assert!(!thresholds.exceeds(GraphLinkEdgeType::SemanticSimilar, 0.70));
    }

    #[test]
    fn test_builder() {
        let thresholds = EdgeThresholds::builder()
            .semantic_similar(0.80)
            .code_related(0.65)
            .multi_agreement_min_embedders(4)
            .build();

        assert_eq!(thresholds.semantic_similar, 0.80);
        assert_eq!(thresholds.code_related, 0.65);
        assert_eq!(thresholds.multi_agreement_min_embedders, 4);
        // Other values should be defaults
        assert_eq!(thresholds.entity_shared, 0.65);
    }

    #[test]
    fn test_as_array() {
        let thresholds = DEFAULT_THRESHOLDS;
        let arr = thresholds.as_array();

        assert_eq!(arr.len(), 8);
        assert_eq!(arr[0], 0.75); // semantic_similar
        assert_eq!(arr[1], 0.70); // code_related
        assert_eq!(arr[6], 0.50); // keyword_overlap
    }

    #[test]
    fn test_thresholds_in_valid_range() {
        let thresholds = DEFAULT_THRESHOLDS;
        for edge_type in GraphLinkEdgeType::all() {
            let t = thresholds.get(edge_type);
            assert!(
                (0.0..=1.0).contains(&t),
                "{:?} threshold {} out of range",
                edge_type,
                t
            );
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let thresholds = EdgeThresholds::builder()
            .semantic_similar(0.85)
            .build();

        let json = serde_json::to_string(&thresholds).unwrap();
        let recovered: EdgeThresholds = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered, thresholds);
    }
}
