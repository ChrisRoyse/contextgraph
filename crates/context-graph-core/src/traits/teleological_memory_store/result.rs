//! Search result types for teleological memory queries.

use serde::{Deserialize, Serialize};

use crate::embeddings::category::category_for;
use crate::teleological::Embedder;
use crate::types::fingerprint::TeleologicalFingerprint;

/// Temporal boost breakdown for interpretability.
///
/// Provides visibility into per-component temporal contributions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalBreakdown {
    /// E2 recency score [0.0, 1.0].
    /// Higher = more recent (based on decay function).
    pub recency_score: f32,

    /// E3 periodic score [0.0, 1.0].
    /// Higher = better hour/day pattern match.
    pub periodic_score: f32,

    /// E4 sequence score [0.0, 1.0].
    /// Higher = closer to anchor in sequence.
    pub sequence_score: f32,

    /// Combined temporal score [0.0, 1.0].
    /// Weighted combination: 50% recency + 35% sequence + 15% periodic.
    pub combined_score: f32,
}

impl Default for TemporalBreakdown {
    fn default() -> Self {
        Self {
            recency_score: 1.0,
            periodic_score: 0.5,
            sequence_score: 0.5,
            combined_score: 0.5,
        }
    }
}

/// Search result from teleological memory queries.
///
/// Contains the matched fingerprint along with scoring metadata
/// for ranking and analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalSearchResult {
    /// The matched teleological fingerprint.
    pub fingerprint: TeleologicalFingerprint,

    /// Overall similarity score [0.0, 1.0].
    /// Computed differently depending on search type.
    pub similarity: f32,

    /// Per-embedder similarity scores (13 values for E1-E13).
    /// Sparse embeddings (E6, E13) use sparse dot product.
    pub embedder_scores: [f32; 14],

    /// Stage scores from the 5-stage retrieval pipeline.
    /// [sparse_recall, semantic_ann, precision, rerank, teleological]
    pub stage_scores: [f32; 5],

    /// Original content text (if requested and available).
    ///
    /// This field is `None` when:
    /// - `include_content=false` in search options (default)
    /// - Content was never stored for this fingerprint
    /// - Backend doesn't support content storage
    ///
    /// TASK-CONTENT-004: Added for content hydration in search results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Temporal boost breakdown for interpretability.
    ///
    /// Populated when temporal boosts are applied (temporal_weight > 0).
    /// Shows per-component contributions for debugging and analysis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_breakdown: Option<TemporalBreakdown>,
}

impl TeleologicalSearchResult {
    /// Create a new search result with computed scores.
    pub fn new(
        fingerprint: TeleologicalFingerprint,
        similarity: f32,
        embedder_scores: [f32; 14],
    ) -> Self {
        Self {
            fingerprint,
            similarity,
            embedder_scores,
            stage_scores: [0.0; 5],   // Populated by pipeline stages
            content: None,            // Populated by content hydration
            temporal_breakdown: None, // Populated by apply_temporal_boosts
        }
    }

    /// Set the temporal breakdown for interpretability.
    pub fn with_temporal_breakdown(mut self, breakdown: TemporalBreakdown) -> Self {
        self.temporal_breakdown = Some(breakdown);
        self
    }

    /// Get the dominant embedder (highest weighted score).
    ///
    /// FIX: Now applies category weights from constitution:
    /// - SEMANTIC (E1, E5, E6, E7, E10, E12, E13): weight 1.0
    /// - TEMPORAL (E2, E3, E4): weight 0.0 (NEVER dominant per AP-60)
    /// - RELATIONAL (E8, E11): weight 0.5
    /// - STRUCTURAL (E9): weight 0.5
    ///
    /// This ensures temporal embedders cannot be reported as dominant,
    /// as their scores are multiplied by 0.0.
    pub fn dominant_embedder(&self) -> usize {
        self.embedder_scores
            .iter()
            .enumerate()
            .map(|(idx, &score)| {
                // Get category weight for this embedder (default to 1.0 for safety)
                let weight = Embedder::from_index(idx)
                    .map(|e| category_for(e).topic_weight())
                    .unwrap_or(1.0);
                // Temporal embedders (E2-E4) get score * 0.0 = 0.0
                (idx, score * weight)
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0) // Default to E1_Semantic if all scores are 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::SemanticFingerprint;

    #[test]
    fn test_search_result_dominant_embedder() {
        let mut scores = [0.1; 14];
        scores[5] = 0.9; // E6 (Sparse/Semantic) is dominant

        let result = TeleologicalSearchResult {
            fingerprint: TeleologicalFingerprint::new(SemanticFingerprint::zeroed(), [0u8; 32]),
            similarity: 0.8,
            embedder_scores: scores,
            stage_scores: [0.0; 5],
            content: None,
            temporal_breakdown: None,
        };

        assert_eq!(result.dominant_embedder(), 5); // E6 is semantic, should be dominant
    }

    #[test]
    fn test_dominant_embedder_excludes_temporal() {
        // E2 (temporal) has highest raw score but should NOT be dominant
        // because temporal embedders have weight 0.0 per AP-60
        let mut scores = [0.1; 14];
        scores[1] = 0.95; // E2_TemporalRecent has highest raw score
        scores[0] = 0.5; // E1_Semantic has lower raw score but weight 1.0

        let result = TeleologicalSearchResult {
            fingerprint: TeleologicalFingerprint::new(SemanticFingerprint::zeroed(), [0u8; 32]),
            similarity: 0.8,
            embedder_scores: scores,
            stage_scores: [0.0; 5],
            content: None,
            temporal_breakdown: None,
        };

        // E1 (index 0) should be dominant, NOT E2 (index 1)
        // E2 weighted score: 0.95 * 0.0 = 0.0
        // E1 weighted score: 0.5 * 1.0 = 0.5
        assert_eq!(
            result.dominant_embedder(),
            0,
            "Temporal embedder E2 should never be dominant (weight 0.0 per AP-60)"
        );
    }
}
