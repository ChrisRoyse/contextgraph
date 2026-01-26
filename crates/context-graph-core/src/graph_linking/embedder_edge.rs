//! K-NN graph edges per embedder.
//!
//! Each embedder maintains its own K-NN graph where each node is connected
//! to its k nearest neighbors according to that embedder's similarity metric.
//!
//! # Key Format
//!
//! Edges are stored with key format: [embedder_id: u8][source_uuid: 16 bytes] = 17 bytes fixed.
//! This enables efficient prefix scans for all neighbors of a node within an embedder.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DirectedRelation, EdgeError, EdgeResult};

/// An edge in the K-NN graph for a specific embedder.
///
/// Each embedder maintains its own graph where nodes are connected to their
/// k nearest neighbors according to that embedder's similarity metric.
///
/// # Fields
///
/// - `source`: Source node UUID
/// - `target`: Target node UUID (a neighbor of source)
/// - `embedder_id`: Which embedder this edge belongs to (0-12)
/// - `similarity`: Similarity score between source and target
/// - `direction`: For E5/E8, the direction of the relationship
///
/// # Examples
///
/// ```
/// use uuid::Uuid;
/// use context_graph_core::graph_linking::{EmbedderEdge, DirectedRelation};
///
/// // Create a semantic (E1) edge
/// let edge = EmbedderEdge::new(
///     Uuid::new_v4(),
///     Uuid::new_v4(),
///     0,  // E1 Semantic
///     0.85,
/// ).unwrap();
///
/// assert_eq!(edge.embedder_id(), 0);
/// assert_eq!(edge.direction(), DirectedRelation::Symmetric);
///
/// // Create a causal (E5) edge with direction
/// let causal_edge = EmbedderEdge::with_direction(
///     Uuid::new_v4(),
///     Uuid::new_v4(),
///     4,  // E5 Causal
///     0.75,
///     DirectedRelation::Forward,  // cause → effect
/// ).unwrap();
///
/// assert!(causal_edge.direction().is_forward());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EmbedderEdge {
    source: Uuid,
    target: Uuid,
    embedder_id: u8,
    similarity: f32,
    direction: DirectedRelation,
}

impl EmbedderEdge {
    /// Create a new embedder edge.
    ///
    /// For symmetric embedders (all except E5, E8), direction is set to Symmetric.
    /// For asymmetric embedders, use `with_direction`.
    ///
    /// # Arguments
    ///
    /// * `source` - Source node UUID
    /// * `target` - Target node UUID
    /// * `embedder_id` - Embedder index (0-12)
    /// * `similarity` - Similarity score
    ///
    /// # Errors
    ///
    /// - `InvalidEmbedderId` if embedder_id > 12
    /// - `InvalidSimilarityScore` if similarity not in [-1.0, 1.0]
    /// - `SymmetricCosineViolation` if embedder is E5/E8 (must use `with_direction`)
    pub fn new(source: Uuid, target: Uuid, embedder_id: u8, similarity: f32) -> EdgeResult<Self> {
        // Validate embedder ID
        if embedder_id > 12 {
            return Err(EdgeError::invalid_embedder_id(embedder_id));
        }

        // Check for asymmetric embedders
        if embedder_id == 4 || embedder_id == 7 {
            // E5 (Causal) or E8 (Emotional/Graph) require direction
            return Err(EdgeError::symmetric_cosine_violation(embedder_id));
        }

        // Validate similarity (cosine similarity range)
        if !(-1.0..=1.0).contains(&similarity) {
            return Err(EdgeError::InvalidSimilarityScore {
                score: similarity,
                min: -1.0,
                max: 1.0,
            });
        }

        Ok(Self {
            source,
            target,
            embedder_id,
            similarity,
            direction: DirectedRelation::Symmetric,
        })
    }

    /// Create a new embedder edge with explicit direction.
    ///
    /// Use this for asymmetric embedders (E5 Causal, E8 Graph).
    ///
    /// # Arguments
    ///
    /// * `source` - Source node UUID
    /// * `target` - Target node UUID
    /// * `embedder_id` - Embedder index (0-12)
    /// * `similarity` - Similarity score
    /// * `direction` - Relationship direction
    ///
    /// # Errors
    ///
    /// - `InvalidEmbedderId` if embedder_id > 12
    /// - `InvalidSimilarityScore` if similarity not in [-1.0, 1.0]
    pub fn with_direction(
        source: Uuid,
        target: Uuid,
        embedder_id: u8,
        similarity: f32,
        direction: DirectedRelation,
    ) -> EdgeResult<Self> {
        // Validate embedder ID
        if embedder_id > 12 {
            return Err(EdgeError::invalid_embedder_id(embedder_id));
        }

        // Validate similarity (cosine similarity range)
        if !(-1.0..=1.0).contains(&similarity) {
            return Err(EdgeError::InvalidSimilarityScore {
                score: similarity,
                min: -1.0,
                max: 1.0,
            });
        }

        Ok(Self {
            source,
            target,
            embedder_id,
            similarity,
            direction,
        })
    }

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

    /// Get the embedder ID (0-12).
    #[inline]
    pub fn embedder_id(&self) -> u8 {
        self.embedder_id
    }

    /// Get the similarity score.
    #[inline]
    pub fn similarity(&self) -> f32 {
        self.similarity
    }

    /// Get the direction of the relationship.
    #[inline]
    pub fn direction(&self) -> DirectedRelation {
        self.direction
    }

    /// Check if this edge is from an asymmetric embedder (E5 or E8).
    #[inline]
    pub fn is_asymmetric(&self) -> bool {
        self.embedder_id == 4 || self.embedder_id == 7
    }

    /// Check if this edge is from a temporal embedder (E2, E3, E4).
    ///
    /// Per AP-60, temporal embedders should NOT be used for edge type detection.
    #[inline]
    pub fn is_temporal(&self) -> bool {
        matches!(self.embedder_id, 1 | 2 | 3)
    }

    /// Get the adjusted similarity with direction modifier applied.
    ///
    /// For asymmetric embedders:
    /// - Forward: 1.2x boost
    /// - Backward: 0.8x dampening
    /// - Symmetric: 1.0x (no change)
    #[inline]
    pub fn adjusted_similarity(&self) -> f32 {
        self.similarity * self.direction.similarity_modifier()
    }

    /// Create the reverse edge (swap source and target, reverse direction).
    pub fn reverse(&self) -> Self {
        Self {
            source: self.target,
            target: self.source,
            embedder_id: self.embedder_id,
            similarity: self.similarity,
            direction: self.direction.reverse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_symmetric_edge() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let edge = EmbedderEdge::new(source, target, 0, 0.85).unwrap();

        assert_eq!(edge.source(), source);
        assert_eq!(edge.target(), target);
        assert_eq!(edge.embedder_id(), 0);
        assert_eq!(edge.similarity(), 0.85);
        assert_eq!(edge.direction(), DirectedRelation::Symmetric);
        assert!(!edge.is_asymmetric());
    }

    #[test]
    fn test_new_rejects_asymmetric_embedders() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // E5 Causal - should require direction
        let result = EmbedderEdge::new(source, target, 4, 0.75);
        assert!(matches!(
            result,
            Err(EdgeError::SymmetricCosineViolation { embedder_id: 4, .. })
        ));

        // E8 Graph - should require direction
        let result = EmbedderEdge::new(source, target, 7, 0.75);
        assert!(matches!(
            result,
            Err(EdgeError::SymmetricCosineViolation { embedder_id: 7, .. })
        ));
    }

    #[test]
    fn test_with_direction_asymmetric() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // E5 Causal with forward direction (cause → effect)
        let edge = EmbedderEdge::with_direction(source, target, 4, 0.75, DirectedRelation::Forward)
            .unwrap();

        assert_eq!(edge.embedder_id(), 4);
        assert!(edge.is_asymmetric());
        assert!(edge.direction().is_forward());
    }

    #[test]
    fn test_invalid_embedder_id() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let result = EmbedderEdge::new(source, target, 13, 0.75);
        assert!(matches!(
            result,
            Err(EdgeError::InvalidEmbedderId { embedder_id: 13 })
        ));

        let result = EmbedderEdge::new(source, target, 255, 0.75);
        assert!(matches!(
            result,
            Err(EdgeError::InvalidEmbedderId { embedder_id: 255 })
        ));
    }

    #[test]
    fn test_invalid_similarity_score() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let result = EmbedderEdge::new(source, target, 0, 1.5);
        assert!(matches!(result, Err(EdgeError::InvalidSimilarityScore { .. })));

        let result = EmbedderEdge::new(source, target, 0, -1.5);
        assert!(matches!(result, Err(EdgeError::InvalidSimilarityScore { .. })));

        // Valid edge cases
        assert!(EmbedderEdge::new(source, target, 0, 1.0).is_ok());
        assert!(EmbedderEdge::new(source, target, 0, -1.0).is_ok());
        assert!(EmbedderEdge::new(source, target, 0, 0.0).is_ok());
    }

    #[test]
    fn test_is_temporal() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // E2, E3, E4 are temporal
        for id in [1, 2, 3] {
            let edge = EmbedderEdge::new(source, target, id, 0.5).unwrap();
            assert!(edge.is_temporal(), "E{} should be temporal", id + 1);
        }

        // Others are not temporal
        for id in [0, 5, 6, 8, 9, 10, 11, 12] {
            let edge = EmbedderEdge::new(source, target, id, 0.5).unwrap();
            assert!(!edge.is_temporal(), "E{} should not be temporal", id + 1);
        }
    }

    #[test]
    fn test_adjusted_similarity() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // Symmetric edge - no adjustment
        let edge = EmbedderEdge::new(source, target, 0, 0.8).unwrap();
        assert_eq!(edge.adjusted_similarity(), 0.8);

        // Forward (cause→effect) - 1.2x boost
        let edge =
            EmbedderEdge::with_direction(source, target, 4, 0.8, DirectedRelation::Forward).unwrap();
        assert!((edge.adjusted_similarity() - 0.96).abs() < 1e-6);

        // Backward (effect→cause) - 0.8x dampening
        let edge =
            EmbedderEdge::with_direction(source, target, 4, 0.8, DirectedRelation::Backward).unwrap();
        assert!((edge.adjusted_similarity() - 0.64).abs() < 1e-6);
    }

    #[test]
    fn test_reverse() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let edge =
            EmbedderEdge::with_direction(source, target, 4, 0.75, DirectedRelation::Forward).unwrap();
        let reversed = edge.reverse();

        assert_eq!(reversed.source(), target);
        assert_eq!(reversed.target(), source);
        assert_eq!(reversed.embedder_id(), 4);
        assert_eq!(reversed.similarity(), 0.75);
        assert_eq!(reversed.direction(), DirectedRelation::Backward);
    }

    #[test]
    fn test_serde_roundtrip() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let edge =
            EmbedderEdge::with_direction(source, target, 4, 0.75, DirectedRelation::Forward).unwrap();

        let json = serde_json::to_string(&edge).unwrap();
        let recovered: EmbedderEdge = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered.source(), edge.source());
        assert_eq!(recovered.target(), edge.target());
        assert_eq!(recovered.embedder_id(), edge.embedder_id());
        assert_eq!(recovered.similarity(), edge.similarity());
        assert_eq!(recovered.direction(), edge.direction());
    }
}
