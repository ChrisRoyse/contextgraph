//! Teleological retrieval result types for multi-embedding search.
//!
//! This module provides result structures for the multi-embedding
//! retrieval pipeline, including per-stage breakdown.
//!
//! # TASK-L008 Implementation
//!
//! Implements result structures per constitution.yaml spec:
//! - `TeleologicalRetrievalResult`: Top-level result with timing and breakdown
//! - `ScoredMemory`: Individual result with scores
//! - `PipelineBreakdown`: Per-stage candidate details for debugging
//!
//! FAIL FAST: No silent fallbacks, explicit error propagation.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::PipelineStageTiming;

/// Result from teleological retrieval pipeline.
///
/// Contains final ranked results plus timing breakdown and optional
/// per-stage details for debugging.
///
/// # Latency Requirements (constitution.yaml)
///
/// - Total pipeline: <60ms @ 1M memories
/// - Stage 1 (SPLADE): <5ms
/// - Stage 2 (Matryoshka): <10ms
/// - Stage 3 (Full HNSW): <20ms
/// - Stage 4 (Score filter): <10ms
/// - Stage 5 (Late Interaction): <15ms
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeleologicalRetrievalResult {
    /// Final ranked results after all pipeline stages.
    ///
    /// Ordered by aggregate score (highest first).
    pub results: Vec<ScoredMemory>,

    /// Timing breakdown for each pipeline stage.
    pub timing: PipelineStageTiming,

    /// Total end-to-end time.
    pub total_time: Duration,

    /// Number of embedding spaces successfully searched.
    pub spaces_searched: usize,

    /// Number of embedding spaces that failed (graceful degradation).
    pub spaces_failed: usize,

    /// Per-stage breakdown (if include_breakdown=true in query).
    ///
    /// Useful for debugging and performance analysis.
    pub breakdown: Option<PipelineBreakdown>,
}

impl TeleologicalRetrievalResult {
    /// Create a new teleological retrieval result.
    pub fn new(
        results: Vec<ScoredMemory>,
        timing: PipelineStageTiming,
        total_time: Duration,
        spaces_searched: usize,
        spaces_failed: usize,
    ) -> Self {
        Self {
            results,
            timing,
            total_time,
            spaces_searched,
            spaces_failed,
            breakdown: None,
        }
    }

    /// Add per-stage breakdown.
    pub fn with_breakdown(mut self, breakdown: PipelineBreakdown) -> Self {
        self.breakdown = Some(breakdown);
        self
    }

    /// Check if the pipeline met the <60ms latency target.
    #[inline]
    pub fn within_latency_target(&self) -> bool {
        self.total_time.as_millis() < 60
    }

    /// Check if all stages met their individual latency targets.
    #[inline]
    pub fn all_stages_within_target(&self) -> bool {
        self.timing.all_stages_within_target()
    }

    /// Get the top result if available.
    pub fn top_result(&self) -> Option<&ScoredMemory> {
        self.results.first()
    }

    /// Get results count.
    #[inline]
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Check if results are empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get timing summary as human-readable string.
    pub fn timing_summary(&self) -> String {
        format!("Total: {:?} | {}", self.total_time, self.timing.summary())
    }
}

/// A scored memory from teleological retrieval.
///
/// Includes standard similarity scores from multi-embedding search.
///
/// # Score Components
///
/// - `score`: Final aggregate score after RRF fusion (0.0-1.0)
/// - `content_similarity`: Raw content similarity from Stage 3
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoredMemory {
    /// Memory/fingerprint UUID.
    pub memory_id: Uuid,

    /// Final aggregate score after RRF fusion.
    ///
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d)) where k=60
    pub score: f32,

    /// Raw content similarity (Stage 3).
    ///
    /// Average cosine similarity across the 13 embedding spaces.
    pub content_similarity: f32,

    /// Number of embedding spaces where this memory appeared.
    ///
    /// Higher = more cross-space relevance.
    pub space_count: usize,
}

impl ScoredMemory {
    /// Create a new scored memory.
    pub fn new(memory_id: Uuid, score: f32, content_similarity: f32, space_count: usize) -> Self {
        Self {
            memory_id,
            score,
            content_similarity,
            space_count,
        }
    }
}

/// Per-stage breakdown for debugging and analysis.
///
/// Contains candidate IDs and counts at each stage to understand
/// filtering behavior.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PipelineBreakdown {
    /// Stage 1: SPLADE sparse retrieval candidates.
    pub stage1_candidates: Vec<Uuid>,

    /// Stage 2: Matryoshka 128D filtering candidates.
    pub stage2_candidates: Vec<Uuid>,

    /// Stage 3: Full HNSW multi-space candidates.
    pub stage3_candidates: Vec<Uuid>,

    /// Stage 4: Score-filtered candidates.
    pub stage4_candidates: Vec<Uuid>,

    /// Stage 5: Late interaction final candidates.
    pub stage5_candidates: Vec<Uuid>,
}

impl PipelineBreakdown {
    /// Create an empty breakdown.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set Stage 1 candidates.
    pub fn with_stage1(mut self, candidates: Vec<Uuid>) -> Self {
        self.stage1_candidates = candidates;
        self
    }

    /// Set Stage 2 candidates.
    pub fn with_stage2(mut self, candidates: Vec<Uuid>) -> Self {
        self.stage2_candidates = candidates;
        self
    }

    /// Set Stage 3 candidates.
    pub fn with_stage3(mut self, candidates: Vec<Uuid>) -> Self {
        self.stage3_candidates = candidates;
        self
    }

    /// Set Stage 4 candidates.
    pub fn with_stage4(mut self, candidates: Vec<Uuid>) -> Self {
        self.stage4_candidates = candidates;
        self
    }

    /// Set Stage 5 candidates.
    pub fn with_stage5(mut self, candidates: Vec<Uuid>) -> Self {
        self.stage5_candidates = candidates;
        self
    }

    /// Get candidate reduction ratio (Stage 1 to Stage 5).
    pub fn reduction_ratio(&self) -> f32 {
        if self.stage1_candidates.is_empty() {
            return 0.0;
        }
        self.stage5_candidates.len() as f32 / self.stage1_candidates.len() as f32
    }

    /// Get funnel summary string.
    pub fn funnel_summary(&self) -> String {
        format!(
            "S1:{} → S2:{} → S3:{} → S4:{} → S5:{}",
            self.stage1_candidates.len(),
            self.stage2_candidates.len(),
            self.stage3_candidates.len(),
            self.stage4_candidates.len(),
            self.stage5_candidates.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scored_memory_creation() {
        let id = Uuid::new_v4();
        let memory = ScoredMemory::new(id, 0.85, 0.90, 8);

        assert_eq!(memory.memory_id, id);
        assert!((memory.score - 0.85).abs() < f32::EPSILON);
        assert!((memory.content_similarity - 0.90).abs() < f32::EPSILON);
        assert_eq!(memory.space_count, 8);

        println!("[VERIFIED] ScoredMemory creation with all fields");
    }

    #[test]
    fn test_teleological_result_creation() {
        let id = Uuid::new_v4();
        let memory = ScoredMemory::new(id, 0.85, 0.90, 8);

        let timing = PipelineStageTiming::new(
            std::time::Duration::from_millis(4),
            std::time::Duration::from_millis(8),
            std::time::Duration::from_millis(18),
            std::time::Duration::from_millis(9),
            std::time::Duration::from_millis(12),
            [1000, 200, 100, 50, 20],
        );

        let result = TeleologicalRetrievalResult::new(
            vec![memory],
            timing,
            std::time::Duration::from_millis(55),
            14,
            0,
        );

        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
        assert!(result.within_latency_target());
        assert!(result.all_stages_within_target());
        assert_eq!(result.spaces_searched, 14);
        assert_eq!(result.spaces_failed, 0);

        println!("[VERIFIED] TeleologicalRetrievalResult creation and latency checks");
    }

    #[test]
    fn test_pipeline_breakdown() {
        let ids: Vec<Uuid> = (0..100).map(|_| Uuid::new_v4()).collect();

        let breakdown = PipelineBreakdown::new()
            .with_stage1(ids[0..100].to_vec())
            .with_stage2(ids[0..50].to_vec())
            .with_stage3(ids[0..25].to_vec())
            .with_stage4(ids[0..15].to_vec())
            .with_stage5(ids[0..10].to_vec());

        assert_eq!(breakdown.stage1_candidates.len(), 100);

        let ratio = breakdown.reduction_ratio();
        assert!((ratio - 0.10).abs() < 0.001);

        let summary = breakdown.funnel_summary();
        assert!(summary.contains("S1:100"));

        println!("BEFORE: 100 candidates");
        println!("AFTER: {}", summary);
        println!("[VERIFIED] PipelineBreakdown tracks funnel correctly");
    }

    #[test]
    fn test_latency_target_exceeded() {
        let timing = PipelineStageTiming::new(
            std::time::Duration::from_millis(6), // Exceeds 5ms
            std::time::Duration::from_millis(8),
            std::time::Duration::from_millis(18),
            std::time::Duration::from_millis(9),
            std::time::Duration::from_millis(12),
            [1000, 200, 100, 50, 20],
        );

        let result = TeleologicalRetrievalResult::new(
            Vec::new(),
            timing,
            std::time::Duration::from_millis(65), // Exceeds 60ms
            13,
            0,
        );

        assert!(!result.within_latency_target());
        assert!(!result.all_stages_within_target());

        println!("[VERIFIED] Latency target checks fail when thresholds exceeded");
    }

    #[test]
    fn test_timing_summary() {
        let timing = PipelineStageTiming::new(
            std::time::Duration::from_millis(4),
            std::time::Duration::from_millis(8),
            std::time::Duration::from_millis(18),
            std::time::Duration::from_millis(9),
            std::time::Duration::from_millis(12),
            [1000, 200, 100, 50, 20],
        );

        let result = TeleologicalRetrievalResult::new(
            Vec::new(),
            timing,
            std::time::Duration::from_millis(55),
            13,
            0,
        );

        let summary = result.timing_summary();
        assert!(summary.contains("Total:"));
        assert!(summary.contains("S1:"));
        assert!(summary.contains("S4:"));

        println!("[VERIFIED] timing_summary produces: {}", summary);
    }
}
