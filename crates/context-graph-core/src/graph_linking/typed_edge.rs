//! Multi-relation typed edges with embedder agreement tracking.
//!
//! TypedEdge represents a relationship between two nodes that has been
//! classified into one of 8 edge types based on which embedders agree
//! that the nodes are similar.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DirectedRelation, EdgeError, EdgeResult, GraphLinkEdgeType};

/// Number of embedders in the system.
const NUM_EMBEDDERS: usize = 13;

/// A multi-relation edge between two memory nodes.
///
/// TypedEdge tracks:
/// - The edge type (based on primary embedder or multi-agreement)
/// - The weight (computed from embedder similarities)
/// - The direction (for asymmetric types like CausalChain, GraphConnected)
/// - Per-embedder similarity scores
/// - Which embedders agree the nodes are related (bitset)
///
/// # Examples
///
/// ```
/// use uuid::Uuid;
/// use context_graph_core::graph_linking::{TypedEdge, GraphLinkEdgeType, DirectedRelation};
///
/// // Create a semantic similarity edge
/// let mut scores = [0.0f32; 13];
/// scores[0] = 0.85; // E1 semantic
///
/// let edge = TypedEdge::new(
///     Uuid::new_v4(),
///     Uuid::new_v4(),
///     GraphLinkEdgeType::SemanticSimilar,
///     0.85,
///     DirectedRelation::Symmetric,
///     scores,
///     1,  // agreement_count
///     0b0000_0000_0001,  // agreeing_embedders (E1 only)
/// ).unwrap();
///
/// assert_eq!(edge.edge_type(), GraphLinkEdgeType::SemanticSimilar);
/// assert_eq!(edge.agreement_count(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedEdge {
    /// Source node UUID
    source: Uuid,
    /// Target node UUID
    target: Uuid,
    /// Edge type based on primary embedder or multi-agreement
    edge_type: GraphLinkEdgeType,
    /// Computed weight [0.0, 1.0]
    weight: f32,
    /// Direction for asymmetric edges (E5, E8)
    direction: DirectedRelation,
    /// Per-embedder similarity scores [E1, E2, ..., E13]
    embedder_scores: [f32; NUM_EMBEDDERS],
    /// Number of embedders that agree (above threshold)
    agreement_count: u8,
    /// Bitset of which embedders agree (bit 0 = E1, bit 12 = E13)
    agreeing_embedders: u16,
}

impl TypedEdge {
    /// Create a new typed edge.
    ///
    /// # Arguments
    ///
    /// * `source` - Source node UUID
    /// * `target` - Target node UUID
    /// * `edge_type` - The classified edge type
    /// * `weight` - Computed weight [0.0, 1.0]
    /// * `direction` - Direction for asymmetric types
    /// * `embedder_scores` - Per-embedder similarity scores
    /// * `agreement_count` - Number of agreeing embedders
    /// * `agreeing_embedders` - Bitset of agreeing embedders
    ///
    /// # Errors
    ///
    /// - `InvalidSimilarityScore` if weight not in [0.0, 1.0]
    /// - `DirectionRequired` if edge_type is asymmetric but direction is Symmetric
    /// - `AgreementCountMismatch` if count doesn't match bitset popcount
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: Uuid,
        target: Uuid,
        edge_type: GraphLinkEdgeType,
        weight: f32,
        direction: DirectedRelation,
        embedder_scores: [f32; NUM_EMBEDDERS],
        agreement_count: u8,
        agreeing_embedders: u16,
    ) -> EdgeResult<Self> {
        // Validate weight
        if !(0.0..=1.0).contains(&weight) {
            return Err(EdgeError::InvalidSimilarityScore {
                score: weight,
                min: 0.0,
                max: 1.0,
            });
        }

        // Validate direction for asymmetric types
        if edge_type.is_asymmetric() && direction.is_symmetric() {
            return Err(EdgeError::DirectionRequired { edge_type });
        }

        // Validate agreement count matches bitset
        let popcount = agreeing_embedders.count_ones() as u8;
        if agreement_count != popcount {
            return Err(EdgeError::AgreementCountMismatch {
                count: agreement_count,
                popcount,
            });
        }

        Ok(Self {
            source,
            target,
            edge_type,
            weight,
            direction,
            embedder_scores,
            agreement_count,
            agreeing_embedders,
        })
    }

    /// Create a typed edge from embedder scores, auto-detecting edge type.
    ///
    /// This analyzes the embedder scores to determine the appropriate edge type
    /// based on which embedders have high similarity.
    ///
    /// # Arguments
    ///
    /// * `source` - Source node UUID
    /// * `target` - Target node UUID
    /// * `embedder_scores` - Per-embedder similarity scores
    /// * `thresholds` - Per-embedder similarity thresholds
    /// * `direction` - Direction for asymmetric embedders (E5, E8)
    pub fn from_scores(
        source: Uuid,
        target: Uuid,
        embedder_scores: [f32; NUM_EMBEDDERS],
        thresholds: &[f32; NUM_EMBEDDERS],
        direction: DirectedRelation,
    ) -> EdgeResult<Self> {
        // Calculate which embedders agree (score >= threshold)
        // Exclude temporal embedders (E2=1, E3=2, E4=3) per AP-60
        let mut agreeing_embedders: u16 = 0;
        let mut agreement_count: u8 = 0;

        for (i, (&score, &threshold)) in embedder_scores.iter().zip(thresholds.iter()).enumerate() {
            // Skip temporal embedders per AP-60
            if matches!(i, 1 | 2 | 3) {
                continue;
            }

            if score >= threshold {
                agreeing_embedders |= 1 << i;
                agreement_count += 1;
            }
        }

        // Determine edge type based on agreement pattern
        let edge_type = Self::detect_edge_type(agreeing_embedders, agreement_count);

        // Calculate weight based on edge type
        let weight = Self::calculate_weight(&embedder_scores, edge_type, agreeing_embedders);

        // Validate direction for asymmetric types
        let final_direction = if edge_type.is_asymmetric() {
            if direction.is_symmetric() {
                // Default to forward for asymmetric if not specified
                DirectedRelation::Forward
            } else {
                direction
            }
        } else {
            DirectedRelation::Symmetric
        };

        Ok(Self {
            source,
            target,
            edge_type,
            weight,
            direction: final_direction,
            embedder_scores,
            agreement_count,
            agreeing_embedders,
        })
    }

    /// Detect edge type from agreeing embedders.
    fn detect_edge_type(agreeing_embedders: u16, agreement_count: u8) -> GraphLinkEdgeType {
        // If 3+ embedders agree, it's multi-agreement
        if agreement_count >= 3 {
            return GraphLinkEdgeType::MultiAgreement;
        }

        // Check for specific embedder patterns
        // Priority order: specialized embedders first

        // E7 Code (bit 6)
        if (agreeing_embedders & (1 << 6)) != 0 {
            return GraphLinkEdgeType::CodeRelated;
        }

        // E11 Entity (bit 10)
        if (agreeing_embedders & (1 << 10)) != 0 {
            return GraphLinkEdgeType::EntityShared;
        }

        // E5 Causal (bit 4) - asymmetric
        if (agreeing_embedders & (1 << 4)) != 0 {
            return GraphLinkEdgeType::CausalChain;
        }

        // E8 Graph (bit 7) - asymmetric
        if (agreeing_embedders & (1 << 7)) != 0 {
            return GraphLinkEdgeType::GraphConnected;
        }

        // E10 Intent (bit 9)
        if (agreeing_embedders & (1 << 9)) != 0 {
            return GraphLinkEdgeType::IntentAligned;
        }

        // E6 Sparse or E13 SPLADE (bits 5, 12)
        if (agreeing_embedders & ((1 << 5) | (1 << 12))) != 0 {
            return GraphLinkEdgeType::KeywordOverlap;
        }

        // Default to E1 semantic (bit 0)
        GraphLinkEdgeType::SemanticSimilar
    }

    /// Calculate edge weight based on type and scores.
    fn calculate_weight(
        scores: &[f32; NUM_EMBEDDERS],
        edge_type: GraphLinkEdgeType,
        agreeing_embedders: u16,
    ) -> f32 {
        match edge_type {
            GraphLinkEdgeType::MultiAgreement => {
                // Average of all agreeing embedder scores
                let mut sum = 0.0;
                let mut count = 0;
                for i in 0..NUM_EMBEDDERS {
                    if (agreeing_embedders & (1 << i)) != 0 {
                        sum += scores[i];
                        count += 1;
                    }
                }
                if count > 0 {
                    sum / count as f32
                } else {
                    0.0
                }
            }
            _ => {
                // Use primary embedder score
                if let Some(idx) = edge_type.primary_embedder_index() {
                    scores[idx].max(0.0).min(1.0)
                } else {
                    0.0
                }
            }
        }
    }

    // ========== Getters ==========

    /// Get the source node UUID.
    #[inline]
    pub fn source(&self) -> Uuid {
        self.source
    }

    /// Get the target node UUID.
    #[inline]
    pub fn target(&self) -> Uuid {
        self.target
    }

    /// Get the edge type.
    #[inline]
    pub fn edge_type(&self) -> GraphLinkEdgeType {
        self.edge_type
    }

    /// Get the edge weight.
    #[inline]
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// Get the direction.
    #[inline]
    pub fn direction(&self) -> DirectedRelation {
        self.direction
    }

    /// Get the per-embedder similarity scores.
    #[inline]
    pub fn embedder_scores(&self) -> &[f32; NUM_EMBEDDERS] {
        &self.embedder_scores
    }

    /// Get the agreement count.
    #[inline]
    pub fn agreement_count(&self) -> u8 {
        self.agreement_count
    }

    /// Get the agreeing embedders bitset.
    #[inline]
    pub fn agreeing_embedders(&self) -> u16 {
        self.agreeing_embedders
    }

    /// Check if a specific embedder agrees.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Embedder index (0-12)
    #[inline]
    pub fn embedder_agrees(&self, embedder_id: u8) -> bool {
        if embedder_id > 12 {
            return false;
        }
        (self.agreeing_embedders & (1 << embedder_id)) != 0
    }

    /// Get the similarity score for a specific embedder.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Embedder index (0-12)
    pub fn embedder_score(&self, embedder_id: u8) -> Option<f32> {
        if embedder_id > 12 {
            return None;
        }
        Some(self.embedder_scores[embedder_id as usize])
    }

    /// Check if this edge is from an asymmetric type.
    #[inline]
    pub fn is_asymmetric(&self) -> bool {
        self.edge_type.is_asymmetric()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_scores() -> [f32; NUM_EMBEDDERS] {
        [0.0; NUM_EMBEDDERS]
    }

    fn default_thresholds() -> [f32; NUM_EMBEDDERS] {
        [0.5; NUM_EMBEDDERS]
    }

    #[test]
    fn test_new_semantic_edge() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85; // E1

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::SemanticSimilar,
            0.85,
            DirectedRelation::Symmetric,
            scores,
            1,
            0b0000_0000_0001, // E1 only
        )
        .unwrap();

        assert_eq!(edge.source(), source);
        assert_eq!(edge.target(), target);
        assert_eq!(edge.edge_type(), GraphLinkEdgeType::SemanticSimilar);
        assert_eq!(edge.weight(), 0.85);
        assert!(edge.direction().is_symmetric());
        assert_eq!(edge.agreement_count(), 1);
        assert!(edge.embedder_agrees(0)); // E1
        assert!(!edge.embedder_agrees(1)); // E2
    }

    #[test]
    fn test_new_asymmetric_edge() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[4] = 0.75; // E5 Causal

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::CausalChain,
            0.75,
            DirectedRelation::Forward,
            scores,
            1,
            0b0000_0001_0000, // E5 only
        )
        .unwrap();

        assert_eq!(edge.edge_type(), GraphLinkEdgeType::CausalChain);
        assert!(edge.is_asymmetric());
        assert!(edge.direction().is_forward());
    }

    #[test]
    fn test_new_rejects_asymmetric_without_direction() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let scores = default_scores();

        let result = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::CausalChain,
            0.75,
            DirectedRelation::Symmetric, // Invalid for CausalChain
            scores,
            1,
            0b0000_0001_0000,
        );

        assert!(matches!(result, Err(EdgeError::DirectionRequired { .. })));
    }

    #[test]
    fn test_new_rejects_invalid_weight() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let scores = default_scores();

        let result = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::SemanticSimilar,
            1.5, // Invalid
            DirectedRelation::Symmetric,
            scores,
            0,
            0,
        );

        assert!(matches!(result, Err(EdgeError::InvalidSimilarityScore { .. })));
    }

    #[test]
    fn test_new_rejects_agreement_mismatch() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let scores = default_scores();

        let result = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::SemanticSimilar,
            0.85,
            DirectedRelation::Symmetric,
            scores,
            3,    // Claims 3 agreeing
            0b01, // But only 1 bit set
        );

        assert!(matches!(result, Err(EdgeError::AgreementCountMismatch { .. })));
    }

    #[test]
    fn test_from_scores_semantic() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85; // E1 semantic

        let thresholds = default_thresholds();

        let edge = TypedEdge::from_scores(source, target, scores, &thresholds, DirectedRelation::Symmetric)
            .unwrap();

        assert_eq!(edge.edge_type(), GraphLinkEdgeType::SemanticSimilar);
        assert!(edge.embedder_agrees(0));
    }

    #[test]
    fn test_from_scores_code_priority() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85; // E1 semantic
        scores[6] = 0.75; // E7 code

        let thresholds = default_thresholds();

        let edge = TypedEdge::from_scores(source, target, scores, &thresholds, DirectedRelation::Symmetric)
            .unwrap();

        // E7 code should take priority over E1 semantic
        assert_eq!(edge.edge_type(), GraphLinkEdgeType::CodeRelated);
    }

    #[test]
    fn test_from_scores_multi_agreement() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85; // E1 semantic
        scores[5] = 0.75; // E6 sparse
        scores[9] = 0.80; // E10 intent

        let thresholds = default_thresholds();

        let edge = TypedEdge::from_scores(source, target, scores, &thresholds, DirectedRelation::Symmetric)
            .unwrap();

        assert_eq!(edge.edge_type(), GraphLinkEdgeType::MultiAgreement);
        assert_eq!(edge.agreement_count(), 3);
    }

    #[test]
    fn test_from_scores_excludes_temporal() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85; // E1 semantic
        scores[1] = 0.90; // E2 temporal recent - should be ignored
        scores[2] = 0.90; // E3 temporal periodic - should be ignored
        scores[3] = 0.90; // E4 temporal positional - should be ignored

        let thresholds = default_thresholds();

        let edge = TypedEdge::from_scores(source, target, scores, &thresholds, DirectedRelation::Symmetric)
            .unwrap();

        // Should only count E1, not temporal embedders
        assert_eq!(edge.agreement_count(), 1);
        assert!(edge.embedder_agrees(0)); // E1
        assert!(!edge.embedder_agrees(1)); // E2 excluded
        assert!(!edge.embedder_agrees(2)); // E3 excluded
        assert!(!edge.embedder_agrees(3)); // E4 excluded
    }

    #[test]
    fn test_embedder_score() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85;
        scores[6] = 0.75;

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::SemanticSimilar,
            0.85,
            DirectedRelation::Symmetric,
            scores,
            2,
            0b0100_0000_0001,
        )
        .unwrap();

        assert_eq!(edge.embedder_score(0), Some(0.85));
        assert_eq!(edge.embedder_score(6), Some(0.75));
        assert_eq!(edge.embedder_score(13), None); // Invalid
    }

    #[test]
    fn test_serde_roundtrip() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = default_scores();
        scores[0] = 0.85;

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::SemanticSimilar,
            0.85,
            DirectedRelation::Symmetric,
            scores,
            1,
            0b0000_0000_0001,
        )
        .unwrap();

        let json = serde_json::to_string(&edge).unwrap();
        let recovered: TypedEdge = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered.source(), edge.source());
        assert_eq!(recovered.target(), edge.target());
        assert_eq!(recovered.edge_type(), edge.edge_type());
        assert_eq!(recovered.weight(), edge.weight());
    }
}
