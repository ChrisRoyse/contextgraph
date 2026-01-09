//! Result types for multi-embedding search.
//!
//! This module provides the result structures returned by the
//! `MultiEmbeddingQueryExecutor` trait methods.
//!
//! # Types
//!
//! - `SpaceSearchResult` - Results from a single embedding space
//! - `ScoredMatch` - A single match with similarity score
//! - `MultiEmbeddingResult` - Aggregated results across spaces
//! - `AggregatedMatch` - A result after RRF aggregation
//! - `PipelineStageTiming` - Timing breakdown for 5-stage pipeline

use std::time::Duration;
use uuid::Uuid;

/// Result from a single embedding space search.
#[derive(Clone, Debug)]
pub struct SpaceSearchResult {
    /// Space index (0-12).
    pub space_index: usize,

    /// Space name (e.g., "E1_Semantic").
    pub space_name: &'static str,

    /// Matches from this space, ranked by similarity.
    pub matches: Vec<ScoredMatch>,

    /// Search time for this space.
    pub search_time: Duration,

    /// Number of items in index for this space.
    pub index_size: usize,

    /// Whether this space search succeeded.
    pub success: bool,

    /// Error message if search failed (for graceful degradation).
    pub error: Option<String>,
}

impl SpaceSearchResult {
    /// Create a successful search result.
    pub fn success(
        space_index: usize,
        matches: Vec<ScoredMatch>,
        search_time: Duration,
        index_size: usize,
    ) -> Self {
        Self {
            space_index,
            space_name: crate::retrieval::EmbeddingSpaceMask::space_name(space_index),
            matches,
            search_time,
            index_size,
            success: true,
            error: None,
        }
    }

    /// Create a failed search result.
    pub fn failure(space_index: usize, error: String) -> Self {
        Self {
            space_index,
            space_name: crate::retrieval::EmbeddingSpaceMask::space_name(space_index),
            matches: Vec::new(),
            search_time: Duration::ZERO,
            index_size: 0,
            success: false,
            error: Some(error),
        }
    }

    /// Get ranked list of memory IDs for RRF aggregation.
    pub fn ranked_ids(&self) -> Vec<Uuid> {
        self.matches.iter().map(|m| m.memory_id).collect()
    }
}

/// A scored match from a single space.
#[derive(Clone, Debug)]
pub struct ScoredMatch {
    /// Memory/fingerprint UUID.
    pub memory_id: Uuid,

    /// Similarity score [0.0, 1.0].
    pub similarity: f32,

    /// Rank in this space's results (0-indexed).
    pub rank: usize,
}

impl ScoredMatch {
    /// Create a new scored match.
    pub fn new(memory_id: Uuid, similarity: f32, rank: usize) -> Self {
        Self {
            memory_id,
            similarity,
            rank,
        }
    }
}

/// Aggregated multi-space search result.
#[derive(Clone, Debug)]
pub struct MultiEmbeddingResult {
    /// Final ranked results after aggregation.
    pub results: Vec<AggregatedMatch>,

    /// Per-space breakdown (if include_space_breakdown=true).
    pub space_breakdown: Option<Vec<SpaceSearchResult>>,

    /// Total end-to-end search time.
    pub total_time: Duration,

    /// Number of spaces actually searched successfully.
    pub spaces_searched: usize,

    /// Number of spaces that failed (for graceful degradation tracking).
    pub spaces_failed: usize,

    /// Pipeline stage timings (if pipeline mode enabled).
    pub stage_timings: Option<PipelineStageTiming>,
}

impl MultiEmbeddingResult {
    /// Create a new multi-embedding result.
    pub fn new(
        results: Vec<AggregatedMatch>,
        total_time: Duration,
        spaces_searched: usize,
        spaces_failed: usize,
    ) -> Self {
        Self {
            results,
            space_breakdown: None,
            total_time,
            spaces_searched,
            spaces_failed,
            stage_timings: None,
        }
    }

    /// Set the space breakdown.
    pub fn with_space_breakdown(mut self, breakdown: Vec<SpaceSearchResult>) -> Self {
        self.space_breakdown = Some(breakdown);
        self
    }

    /// Set the pipeline stage timings.
    pub fn with_stage_timings(mut self, timings: PipelineStageTiming) -> Self {
        self.stage_timings = Some(timings);
        self
    }

    /// Check if query met the 60ms latency target.
    pub fn within_latency_target(&self) -> bool {
        self.total_time.as_millis() < 60
    }

    /// Get the top result if available.
    pub fn top_result(&self) -> Option<&AggregatedMatch> {
        self.results.first()
    }
}

/// Timing breakdown for 5-stage pipeline.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PipelineStageTiming {
    /// Stage 1: SPLADE sparse retrieval.
    pub stage1_splade: Duration,

    /// Stage 2: Matryoshka 128D dense filter.
    pub stage2_matryoshka: Duration,

    /// Stage 3: Full 13-space HNSW search.
    pub stage3_full_hnsw: Duration,

    /// Stage 4: Teleological alignment filter.
    pub stage4_teleological: Duration,

    /// Stage 5: Late interaction reranking.
    pub stage5_late_interaction: Duration,

    /// Candidates after each stage.
    pub candidates_per_stage: [usize; 5],
}

impl PipelineStageTiming {
    /// Create new timing with all stages.
    pub fn new(
        stage1: Duration,
        stage2: Duration,
        stage3: Duration,
        stage4: Duration,
        stage5: Duration,
        candidates: [usize; 5],
    ) -> Self {
        Self {
            stage1_splade: stage1,
            stage2_matryoshka: stage2,
            stage3_full_hnsw: stage3,
            stage4_teleological: stage4,
            stage5_late_interaction: stage5,
            candidates_per_stage: candidates,
        }
    }

    /// Check if all stages met their latency targets (constitution.yaml).
    pub fn all_stages_within_target(&self) -> bool {
        self.stage1_splade.as_millis() < 5
            && self.stage2_matryoshka.as_millis() < 10
            && self.stage3_full_hnsw.as_millis() < 20
            && self.stage4_teleological.as_millis() < 10
            && self.stage5_late_interaction.as_millis() < 15
    }

    /// Total pipeline time.
    pub fn total(&self) -> Duration {
        self.stage1_splade
            + self.stage2_matryoshka
            + self.stage3_full_hnsw
            + self.stage4_teleological
            + self.stage5_late_interaction
    }

    /// Get a summary of each stage's timing.
    pub fn summary(&self) -> String {
        format!(
            "S1:{:?} S2:{:?} S3:{:?} S4:{:?} S5:{:?} Total:{:?}",
            self.stage1_splade,
            self.stage2_matryoshka,
            self.stage3_full_hnsw,
            self.stage4_teleological,
            self.stage5_late_interaction,
            self.total()
        )
    }
}

/// A result aggregated across multiple embedding spaces.
#[derive(Clone, Debug)]
pub struct AggregatedMatch {
    /// Memory/fingerprint UUID.
    pub memory_id: Uuid,

    /// Aggregated score (RRF or weighted average).
    pub aggregate_score: f32,

    /// Number of spaces this memory appeared in.
    pub space_count: usize,

    /// Per-space scores (space_index, similarity, rank).
    pub space_contributions: Vec<SpaceContribution>,

    /// Purpose alignment score (from Stage 4 if available).
    pub purpose_alignment: Option<f32>,
}

impl AggregatedMatch {
    /// Create a new aggregated match.
    pub fn new(memory_id: Uuid, aggregate_score: f32, space_count: usize) -> Self {
        Self {
            memory_id,
            aggregate_score,
            space_count,
            space_contributions: Vec::new(),
            purpose_alignment: None,
        }
    }

    /// Add a space contribution.
    pub fn add_contribution(&mut self, contribution: SpaceContribution) {
        self.space_contributions.push(contribution);
    }

    /// Set the purpose alignment score.
    pub fn with_purpose_alignment(mut self, alignment: f32) -> Self {
        self.purpose_alignment = Some(alignment);
        self
    }
}

/// Contribution from a single space to the aggregated score.
#[derive(Clone, Debug)]
pub struct SpaceContribution {
    /// Space index (0-12).
    pub space_index: usize,

    /// Similarity in this space.
    pub similarity: f32,

    /// Rank in this space's results.
    pub rank: usize,

    /// RRF contribution: 1/(k + rank + 1).
    pub rrf_contribution: f32,
}

impl SpaceContribution {
    /// Create a new space contribution.
    pub fn new(space_index: usize, similarity: f32, rank: usize, rrf_k: f32) -> Self {
        let rrf_contribution = 1.0 / (rrf_k + (rank as f32) + 1.0);
        Self {
            space_index,
            similarity,
            rank,
            rrf_contribution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scored_match() {
        let id = Uuid::new_v4();
        let m = ScoredMatch::new(id, 0.85, 3);

        assert_eq!(m.memory_id, id);
        assert!((m.similarity - 0.85).abs() < 0.001);
        assert_eq!(m.rank, 3);

        println!("[VERIFIED] ScoredMatch creation");
    }

    #[test]
    fn test_space_search_result_success() {
        let id = Uuid::new_v4();
        let matches = vec![ScoredMatch::new(id, 0.9, 0)];
        let result = SpaceSearchResult::success(0, matches, Duration::from_millis(5), 1000);

        assert_eq!(result.space_index, 0);
        assert_eq!(result.space_name, "E1_Semantic");
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.matches.len(), 1);

        println!("[VERIFIED] SpaceSearchResult::success");
    }

    #[test]
    fn test_space_search_result_failure() {
        let result = SpaceSearchResult::failure(5, "Index corrupted".to_string());

        assert_eq!(result.space_index, 5);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.matches.is_empty());

        println!("[VERIFIED] SpaceSearchResult::failure");
    }

    #[test]
    fn test_space_search_result_ranked_ids() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let matches = vec![ScoredMatch::new(id1, 0.9, 0), ScoredMatch::new(id2, 0.8, 1)];
        let result = SpaceSearchResult::success(0, matches, Duration::from_millis(5), 1000);

        let ranked = result.ranked_ids();
        assert_eq!(ranked, vec![id1, id2]);

        println!("[VERIFIED] ranked_ids preserves order");
    }

    #[test]
    fn test_multi_embedding_result() {
        let id = Uuid::new_v4();
        let matches = vec![AggregatedMatch::new(id, 0.05, 3)];
        let result = MultiEmbeddingResult::new(matches, Duration::from_millis(45), 13, 0);

        assert!(result.within_latency_target());
        assert_eq!(result.spaces_searched, 13);
        assert_eq!(result.spaces_failed, 0);
        assert!(result.top_result().is_some());

        println!("[VERIFIED] MultiEmbeddingResult creation");
    }

    #[test]
    fn test_multi_embedding_result_latency_exceeded() {
        let result = MultiEmbeddingResult::new(Vec::new(), Duration::from_millis(75), 13, 0);

        assert!(!result.within_latency_target());

        println!("[VERIFIED] within_latency_target returns false for >60ms");
    }

    #[test]
    fn test_pipeline_stage_timing() {
        let timing = PipelineStageTiming::new(
            Duration::from_millis(4),
            Duration::from_millis(8),
            Duration::from_millis(18),
            Duration::from_millis(9),
            Duration::from_millis(12),
            [1000, 200, 100, 50, 20],
        );

        assert!(timing.all_stages_within_target());
        assert_eq!(timing.total().as_millis(), 51);

        println!("[VERIFIED] PipelineStageTiming all within target");
    }

    #[test]
    fn test_pipeline_stage_timing_exceeded() {
        let timing = PipelineStageTiming::new(
            Duration::from_millis(6), // Exceeds 5ms target
            Duration::from_millis(8),
            Duration::from_millis(18),
            Duration::from_millis(9),
            Duration::from_millis(12),
            [1000, 200, 100, 50, 20],
        );

        assert!(!timing.all_stages_within_target());

        println!("[VERIFIED] all_stages_within_target false when stage1 > 5ms");
    }

    #[test]
    fn test_aggregated_match() {
        let id = Uuid::new_v4();
        let mut m = AggregatedMatch::new(id, 0.05, 3);
        m.add_contribution(SpaceContribution::new(0, 0.9, 0, 60.0));
        m.add_contribution(SpaceContribution::new(1, 0.85, 1, 60.0));

        assert_eq!(m.space_contributions.len(), 2);
        assert!(m.purpose_alignment.is_none());

        let m_with_alignment = m.with_purpose_alignment(0.75);
        assert_eq!(m_with_alignment.purpose_alignment, Some(0.75));

        println!("[VERIFIED] AggregatedMatch creation and modification");
    }

    #[test]
    fn test_space_contribution_rrf() {
        let contrib = SpaceContribution::new(0, 0.9, 0, 60.0);

        // RRF for rank 0: 1/(60+1) = 1/61
        let expected = 1.0 / 61.0;
        assert!((contrib.rrf_contribution - expected).abs() < 0.0001);

        let contrib2 = SpaceContribution::new(0, 0.8, 5, 60.0);
        // RRF for rank 5: 1/(60+6) = 1/66
        let expected2 = 1.0 / 66.0;
        assert!((contrib2.rrf_contribution - expected2).abs() < 0.0001);

        println!("[VERIFIED] SpaceContribution computes correct RRF");
    }

    #[test]
    fn test_pipeline_timing_summary() {
        let timing = PipelineStageTiming::new(
            Duration::from_millis(4),
            Duration::from_millis(8),
            Duration::from_millis(18),
            Duration::from_millis(9),
            Duration::from_millis(12),
            [1000, 200, 100, 50, 20],
        );

        let summary = timing.summary();
        assert!(summary.contains("S1:"));
        assert!(summary.contains("Total:"));

        println!("[VERIFIED] summary() produces readable output: {}", summary);
    }
}
