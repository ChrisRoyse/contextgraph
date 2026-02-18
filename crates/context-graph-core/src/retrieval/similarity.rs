//! Per-space similarity scores and retrieval result types.
//!
//! This module provides types for tracking similarity scores across all 13
//! embedding spaces, with category-weighted aggregation.
//!
//! # Architecture Rules
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic detection
//! - AP-61: Topic threshold MUST be weighted_agreement >= 2.5

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::embeddings::category::{category_for, max_weighted_agreement};
use crate::teleological::Embedder;

/// Number of embedding spaces.
pub const NUM_SPACES: usize = 13;

/// Similarity scores for all 13 embedding spaces.
///
/// Field names match Embedder variant names (snake_case).
/// All scores are in the range [0.0, 1.0].
///
/// # Category Weights for weighted_mean()
///
/// - Semantic (E1, E5, E6, E7, E10, E12, E13): weight 1.0
/// - Temporal (E2, E3, E4): weight 0.0 (excluded)
/// - Relational (E8, E11): weight 0.5
/// - Structural (E9): weight 0.5
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PerSpaceScores {
    /// E1: Semantic embedding similarity
    pub semantic: f32,
    /// E2: Temporal recent embedding similarity
    pub temporal_recent: f32,
    /// E3: Temporal periodic embedding similarity
    pub temporal_periodic: f32,
    /// E4: Temporal positional embedding similarity
    pub temporal_positional: f32,
    /// E5: Causal embedding similarity
    pub causal: f32,
    /// E6: Sparse lexical embedding similarity
    pub sparse: f32,
    /// E7: Code embedding similarity
    pub code: f32,
    /// E8: Graph/connectivity embedding similarity
    pub emotional: f32,
    /// E9: HDC embedding similarity
    pub hdc: f32,
    /// E10: Multimodal embedding similarity
    pub multimodal: f32,
    /// E11: Entity embedding similarity
    pub entity: f32,
    /// E12: Late interaction embedding similarity
    pub late_interaction: f32,
    /// E13: Keyword SPLADE embedding similarity
    pub keyword_splade: f32,
}

impl PerSpaceScores {
    /// Create a new PerSpaceScores with all zeros.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get score for a specific embedder.
    pub fn get_score(&self, embedder: Embedder) -> f32 {
        match embedder {
            Embedder::Semantic => self.semantic,
            Embedder::TemporalRecent => self.temporal_recent,
            Embedder::TemporalPeriodic => self.temporal_periodic,
            Embedder::TemporalPositional => self.temporal_positional,
            Embedder::Causal => self.causal,
            Embedder::Sparse => self.sparse,
            Embedder::Code => self.code,
            Embedder::Graph => self.emotional,
            Embedder::Hdc => self.hdc,
            Embedder::Multimodal => self.multimodal,
            Embedder::Entity => self.entity,
            Embedder::LateInteraction => self.late_interaction,
            Embedder::KeywordSplade => self.keyword_splade,
        }
    }

    /// Set score for a specific embedder.
    /// Score is clamped to [0.0, 1.0] range.
    pub fn set_score(&mut self, embedder: Embedder, score: f32) {
        let score = score.clamp(0.0, 1.0);
        match embedder {
            Embedder::Semantic => self.semantic = score,
            Embedder::TemporalRecent => self.temporal_recent = score,
            Embedder::TemporalPeriodic => self.temporal_periodic = score,
            Embedder::TemporalPositional => self.temporal_positional = score,
            Embedder::Causal => self.causal = score,
            Embedder::Sparse => self.sparse = score,
            Embedder::Code => self.code = score,
            Embedder::Graph => self.emotional = score,
            Embedder::Hdc => self.hdc = score,
            Embedder::Multimodal => self.multimodal = score,
            Embedder::Entity => self.entity = score,
            Embedder::LateInteraction => self.late_interaction = score,
            Embedder::KeywordSplade => self.keyword_splade = score,
        }
    }

    /// Iterate over all scores with their embedder.
    pub fn iter(&self) -> impl Iterator<Item = (Embedder, f32)> + '_ {
        Embedder::all().map(move |e| (e, self.get_score(e)))
    }

    /// Get the maximum score across all spaces.
    pub fn max_score(&self) -> f32 {
        self.iter().map(|(_, s)| s).fold(0.0_f32, f32::max)
    }

    /// Get the mean score across all 13 spaces (unweighted).
    pub fn mean_score(&self) -> f32 {
        let sum: f32 = self.iter().map(|(_, s)| s).sum();
        sum / NUM_SPACES as f32
    }

    /// Get category-weighted mean score.
    ///
    /// EXCLUDES temporal spaces (E2-E4) per AP-60.
    /// Uses category weights:
    /// - Semantic: 1.0
    /// - Temporal: 0.0 (excluded)
    /// - Relational: 0.5
    /// - Structural: 0.5
    ///
    /// Result is normalized by max_weighted_agreement (8.5).
    pub fn weighted_mean(&self) -> f32 {
        let mut weighted_sum = 0.0;

        for embedder in Embedder::all() {
            let weight = category_for(embedder).topic_weight();
            if weight > 0.0 {
                weighted_sum += weight * self.get_score(embedder);
            }
        }

        weighted_sum / max_weighted_agreement()
    }

    /// Convert to array for compact operations.
    /// Order matches Embedder::index(): E1=0, E2=1, ..., E13=12.
    pub fn to_array(&self) -> [f32; NUM_SPACES] {
        [
            self.semantic,
            self.temporal_recent,
            self.temporal_periodic,
            self.temporal_positional,
            self.causal,
            self.sparse,
            self.code,
            self.emotional,
            self.hdc,
            self.multimodal,
            self.entity,
            self.late_interaction,
            self.keyword_splade,
        ]
    }

    /// Create from array.
    /// Order must match Embedder::index(): E1=0, E2=1, ..., E13=12.
    pub fn from_array(arr: [f32; NUM_SPACES]) -> Self {
        Self {
            semantic: arr[0],
            temporal_recent: arr[1],
            temporal_periodic: arr[2],
            temporal_positional: arr[3],
            causal: arr[4],
            sparse: arr[5],
            code: arr[6],
            emotional: arr[7],
            hdc: arr[8],
            multimodal: arr[9],
            entity: arr[10],
            late_interaction: arr[11],
            keyword_splade: arr[12],
        }
    }

    /// Get list of spaces included in weighted calculations (weight > 0).
    pub fn included_spaces() -> Vec<Embedder> {
        Embedder::all()
            .filter(|e| category_for(*e).topic_weight() > 0.0)
            .collect()
    }
}

/// Result of similarity search for a single memory.
///
/// Contains per-space scores and aggregated relevance information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityResult {
    /// ID of the matching memory.
    pub memory_id: Uuid,
    /// Similarity scores in each embedding space.
    pub per_space_scores: PerSpaceScores,
    /// Category-weighted similarity (temporal excluded, per AP-60).
    pub weighted_similarity: f32,
    /// Computed relevance score (0.0..1.0).
    pub relevance_score: f32,
    /// Embedders where score exceeded threshold.
    pub matching_spaces: Vec<Embedder>,
    /// Embedders included in weighted calculation (weight > 0).
    pub included_spaces: Vec<Embedder>,
    /// Number of matching spaces (= matching_spaces.len()).
    pub space_count: u8,
}

impl SimilarityResult {
    /// Create a new SimilarityResult with computed weighted_similarity.
    pub fn new(memory_id: Uuid, scores: PerSpaceScores) -> Self {
        let weighted_similarity = scores.weighted_mean();
        Self {
            memory_id,
            per_space_scores: scores,
            weighted_similarity,
            relevance_score: 0.0,
            matching_spaces: Vec::new(),
            included_spaces: PerSpaceScores::included_spaces(),
            space_count: 0,
        }
    }

    /// Create with full computed fields.
    pub fn with_relevance(
        memory_id: Uuid,
        scores: PerSpaceScores,
        relevance_score: f32,
        matching_spaces: Vec<Embedder>,
    ) -> Self {
        let space_count = matching_spaces.len() as u8;
        let weighted_similarity = scores.weighted_mean();
        Self {
            memory_id,
            per_space_scores: scores,
            weighted_similarity,
            relevance_score: relevance_score.clamp(0.0, 1.0),
            matching_spaces,
            included_spaces: PerSpaceScores::included_spaces(),
            space_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_space_scores_default() {
        let scores = PerSpaceScores::new();
        assert_eq!(scores.semantic, 0.0);
        assert_eq!(scores.code, 0.0);
        assert_eq!(scores.max_score(), 0.0);
        println!("[PASS] Default PerSpaceScores has all zeros");
    }

    #[test]
    fn test_get_set_score() {
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.85);
        scores.set_score(Embedder::Code, 0.92);

        assert_eq!(scores.get_score(Embedder::Semantic), 0.85);
        assert_eq!(scores.get_score(Embedder::Code), 0.92);
        assert_eq!(scores.max_score(), 0.92);
        println!("[PASS] get_score/set_score work correctly");
    }

    #[test]
    fn test_score_clamping() {
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 1.5); // Should clamp to 1.0
        scores.set_score(Embedder::TemporalRecent, -0.5); // Should clamp to 0.0

        assert_eq!(scores.get_score(Embedder::Semantic), 1.0);
        assert_eq!(scores.get_score(Embedder::TemporalRecent), 0.0);
        println!("[PASS] Score clamping enforces [0.0, 1.0]");
    }

    #[test]
    fn test_iterator() {
        let scores = PerSpaceScores::new();
        let count = scores.iter().count();
        assert_eq!(count, 13);
        println!("[PASS] Iterator visits all 13 spaces");
    }

    #[test]
    fn test_iterator_order() {
        let mut scores = PerSpaceScores::new();
        scores.semantic = 0.1;
        scores.temporal_recent = 0.2;
        scores.keyword_splade = 0.13;

        let collected: Vec<_> = scores.iter().collect();
        assert_eq!(collected[0], (Embedder::Semantic, 0.1));
        assert_eq!(collected[1], (Embedder::TemporalRecent, 0.2));
        assert_eq!(collected[12], (Embedder::KeywordSplade, 0.13));
        println!("[PASS] Iterator order matches Embedder::index()");
    }

    #[test]
    fn test_array_conversion() {
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.5);
        scores.set_score(Embedder::Code, 0.7);

        let arr = scores.to_array();
        assert_eq!(arr[0], 0.5); // E1 at index 0
        assert_eq!(arr[6], 0.7); // E7 at index 6

        let recovered = PerSpaceScores::from_array(arr);
        assert_eq!(recovered.semantic, 0.5);
        assert_eq!(recovered.code, 0.7);
        println!("[PASS] Array conversion roundtrip works");
    }

    #[test]
    fn test_weighted_mean_excludes_temporal() {
        let mut scores = PerSpaceScores::new();
        // Set all semantic spaces to 1.0
        scores.semantic = 1.0;
        scores.causal = 1.0;
        scores.sparse = 1.0;
        scores.code = 1.0;
        scores.multimodal = 1.0;
        scores.late_interaction = 1.0;
        scores.keyword_splade = 1.0;
        // Set relational to 1.0
        scores.emotional = 1.0;
        scores.entity = 1.0;
        // Set structural to 1.0
        scores.hdc = 1.0;
        // Set temporal to 1.0 (should be excluded)
        scores.temporal_recent = 1.0;
        scores.temporal_periodic = 1.0;
        scores.temporal_positional = 1.0;

        let weighted = scores.weighted_mean();
        // (7*1.0 + 2*0.5 + 1*0.5) / 8.5 = 8.5 / 8.5 = 1.0
        assert!((weighted - 1.0).abs() < 1e-6);
        println!("[PASS] weighted_mean = 1.0 when all weighted spaces are 1.0");
    }

    #[test]
    fn test_weighted_mean_temporal_has_no_effect() {
        let mut scores = PerSpaceScores::new();
        // Only set temporal spaces to 1.0
        scores.temporal_recent = 1.0;
        scores.temporal_periodic = 1.0;
        scores.temporal_positional = 1.0;

        let weighted = scores.weighted_mean();
        // Temporal has weight 0.0, so result should be 0.0
        assert_eq!(weighted, 0.0);
        println!("[PASS] AP-60 verified: temporal spaces excluded from weighted_mean");
    }

    #[test]
    fn test_included_spaces() {
        let included = PerSpaceScores::included_spaces();
        // Should be 10 spaces (13 - 3 temporal)
        assert_eq!(included.len(), 10);
        // Should NOT contain temporal
        assert!(!included.contains(&Embedder::TemporalRecent));
        assert!(!included.contains(&Embedder::TemporalPeriodic));
        assert!(!included.contains(&Embedder::TemporalPositional));
        // Should contain semantic
        assert!(included.contains(&Embedder::Semantic));
        assert!(included.contains(&Embedder::Code));
        println!("[PASS] included_spaces returns 10 non-temporal spaces");
    }

    #[test]
    fn test_similarity_result() {
        let id = Uuid::new_v4();
        let mut scores = PerSpaceScores::new();
        scores.semantic = 0.8;
        scores.code = 0.9;

        let result = SimilarityResult::with_relevance(
            id,
            scores,
            0.75,
            vec![Embedder::Semantic, Embedder::Code],
        );

        assert_eq!(result.memory_id, id);
        assert_eq!(result.relevance_score, 0.75);
        assert_eq!(result.space_count, 2);
        assert_eq!(result.matching_spaces.len(), 2);
        assert_eq!(result.included_spaces.len(), 10);
        println!("[PASS] SimilarityResult construction works");
    }

    #[test]
    fn test_serialization_roundtrip_json() {
        let mut scores = PerSpaceScores::new();
        scores.semantic = 0.85;
        scores.code = 0.92;

        let json = serde_json::to_string(&scores).expect("serialize");
        let recovered: PerSpaceScores = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(scores, recovered);
        println!("[PASS] JSON serialization roundtrip works");
    }

    #[test]
    fn test_similarity_result_serialization() {
        let id = Uuid::new_v4();
        let scores = PerSpaceScores::new();
        let result = SimilarityResult::new(id, scores);

        let json = serde_json::to_string(&result).expect("serialize");
        let recovered: SimilarityResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(recovered.memory_id, id);
        println!("[PASS] SimilarityResult JSON roundtrip works");
    }

    #[test]
    fn test_mean_score() {
        let mut scores = PerSpaceScores::new();
        // Set all to 0.5
        for embedder in Embedder::all() {
            scores.set_score(embedder, 0.5);
        }

        let mean = scores.mean_score();
        assert!((mean - 0.5).abs() < 1e-6);
        println!("[PASS] mean_score computes correctly");
    }

    #[test]
    fn test_relevance_score_clamping() {
        let id = Uuid::new_v4();
        let scores = PerSpaceScores::new();

        // Test clamping above 1.0
        let result = SimilarityResult::with_relevance(id, scores.clone(), 1.5, vec![]);
        assert_eq!(result.relevance_score, 1.0);

        // Test clamping below 0.0
        let result2 = SimilarityResult::with_relevance(id, scores, -0.5, vec![]);
        assert_eq!(result2.relevance_score, 0.0);
        println!("[PASS] Relevance score is clamped to [0.0, 1.0]");
    }

    #[test]
    fn test_weighted_mean_partial_scores() {
        let mut scores = PerSpaceScores::new();
        // Set 3 semantic spaces to 1.0
        scores.semantic = 1.0;
        scores.causal = 1.0;
        scores.code = 1.0;
        // Rest are 0.0

        let weighted = scores.weighted_mean();
        // (3 * 1.0) / 8.5 = 3.0 / 8.5 ≈ 0.353
        assert!((weighted - 3.0 / 8.5).abs() < 1e-6);
        println!("[PASS] weighted_mean computes correctly for partial scores: {:.6}", weighted);
    }

    #[test]
    fn test_similarity_result_new() {
        let id = Uuid::new_v4();
        let mut scores = PerSpaceScores::new();
        scores.semantic = 0.5;

        let result = SimilarityResult::new(id, scores);

        assert_eq!(result.memory_id, id);
        assert_eq!(result.relevance_score, 0.0);
        assert_eq!(result.space_count, 0);
        assert!(result.matching_spaces.is_empty());
        assert_eq!(result.included_spaces.len(), 10);
        // weighted_similarity should be computed
        assert!(result.weighted_similarity > 0.0);
        println!("[PASS] SimilarityResult::new() computes weighted_similarity");
    }
}
