//! Query result types for single and multi-space retrieval.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::constants::RRF_K;

/// Result from per-embedder index search (single space).
///
/// Used in Stage 3 before RRF fusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderQueryResult {
    /// Fingerprint UUID.
    pub id: Uuid,

    /// Embedder index (0-12).
    pub embedder_idx: u8,

    /// Similarity score [0.0, 1.0] for cosine, [-1.0, 1.0] for dot product.
    pub similarity: f32,

    /// Distance (metric-specific). For cosine: 1 - similarity.
    pub distance: f32,

    /// Rank in this embedder's result list (0-indexed).
    pub rank: usize,
}

impl EmbedderQueryResult {
    /// Create from similarity score.
    #[must_use]
    pub fn from_similarity(id: Uuid, embedder_idx: u8, similarity: f32, rank: usize) -> Self {
        Self {
            id,
            embedder_idx,
            similarity,
            distance: 1.0 - similarity.clamp(-1.0, 1.0),
            rank,
        }
    }

    /// Compute RRF contribution for this result.
    /// Formula: 1 / (k + rank + 1) where k = 60 (1-indexed, consistent with core fusion)
    #[must_use]
    pub fn rrf_contribution(&self) -> f32 {
        1.0 / (RRF_K + self.rank as f32 + 1.0)
    }
}

/// Aggregated result from multi-space retrieval (after RRF fusion).
///
/// This is the final result type after Stage 3 multi-space reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSpaceQueryResult {
    /// Fingerprint UUID.
    pub id: Uuid,

    /// Per-embedder similarities (13 values).
    /// NaN if embedder wasn't searched (e.g., sparse-only query).
    pub embedder_similarities: [f32; 13],

    /// RRF fused score from multi-space retrieval.
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d) + 1) where k=60 (1-indexed)
    pub rrf_score: f32,

    /// Weighted average similarity (alternative to RRF).
    /// Uses Constitution-defined weights per query type.
    pub weighted_similarity: f32,

    /// Number of embedders that contributed to this result.
    /// Less than 13 if some embedders weren't searched.
    pub embedder_count: usize,
}

impl MultiSpaceQueryResult {
    /// Create from individual embedder results.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    /// * `results` - Per-embedder query results
    ///
    /// # Panics
    /// Panics if results is empty.
    #[must_use]
    pub fn from_embedder_results(id: Uuid, results: &[EmbedderQueryResult]) -> Self {
        if results.is_empty() {
            panic!(
                "AGGREGATION ERROR: Cannot create MultiSpaceQueryResult from empty results. \
                 Fingerprint ID: {}. This indicates query execution bug.",
                id
            );
        }

        let mut embedder_similarities = [f32::NAN; 13];
        let mut rrf_score = 0.0f32;
        let mut weighted_sum = 0.0f32;
        let mut weight_total = 0.0f32;

        for result in results {
            let idx = result.embedder_idx as usize;
            if idx < 13 {
                embedder_similarities[idx] = result.similarity;
                rrf_score += result.rrf_contribution();
                weighted_sum += result.similarity;
                weight_total += 1.0;
            }
        }

        let weighted_similarity = if weight_total > 0.0 {
            weighted_sum / weight_total
        } else {
            0.0
        };

        Self {
            id,
            embedder_similarities,
            rrf_score,
            weighted_similarity,
            embedder_count: results.len(),
        }
    }
}
