//! Teleological retrieval result types for purpose-aware search.
//!
//! This module provides result structures for the 5-stage teleological
//! retrieval pipeline, including per-stage breakdown and teleological
//! alignment scores.
//!
//! # TASK-L008 Implementation
//!
//! Implements result structures per constitution.yaml spec:
//! - `TeleologicalRetrievalResult`: Top-level result with timing and breakdown
//! - `ScoredMemory`: Individual result with teleological scores
//! - `PipelineBreakdown`: Per-stage candidate details for debugging
//!
//! FAIL FAST: No silent fallbacks, explicit error propagation.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::constants::alignment;
use crate::types::JohariQuadrant;

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
/// - Stage 4 (Teleological): <10ms
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

    /// Get results above a minimum alignment threshold.
    ///
    /// Useful for filtering to only well-aligned results.
    pub fn results_above_alignment(&self, min_alignment: f32) -> Vec<&ScoredMemory> {
        self.results
            .iter()
            .filter(|r| r.goal_alignment >= min_alignment)
            .collect()
    }

    /// Get results in specific Johari quadrants.
    pub fn results_in_quadrants(&self, quadrants: &[JohariQuadrant]) -> Vec<&ScoredMemory> {
        self.results
            .iter()
            .filter(|r| quadrants.contains(&r.johari_quadrant))
            .collect()
    }

    /// Count misaligned results.
    pub fn misaligned_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_misaligned).count()
    }

    /// Get timing summary as human-readable string.
    pub fn timing_summary(&self) -> String {
        format!("Total: {:?} | {}", self.total_time, self.timing.summary())
    }
}

/// A scored memory from teleological retrieval.
///
/// Includes standard similarity scores plus teleological-specific
/// alignment and Johari quadrant classification.
///
/// # Score Components
///
/// - `score`: Final aggregate score after RRF fusion (0.0-1.0)
/// - `content_similarity`: Raw content similarity from Stage 3
/// - `purpose_alignment`: Purpose vector alignment from Stage 4
/// - `goal_alignment`: Goal hierarchy alignment from Stage 4
/// - `johari_quadrant`: Dominant quadrant classification
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

    /// Purpose vector alignment (Stage 4).
    ///
    /// How well memory's purpose aligns with query's purpose.
    /// From PurposeVector cosine similarity.
    pub purpose_alignment: f32,

    /// Goal hierarchy alignment (Stage 4).
    ///
    /// Composite score from GoalAlignmentCalculator.
    /// Includes North Star, Strategic, Tactical, Immediate levels.
    pub goal_alignment: f32,

    /// Dominant Johari quadrant for this memory.
    ///
    /// Determined by entropy/coherence pattern across embedding spaces.
    pub johari_quadrant: JohariQuadrant,

    /// Whether this memory is misaligned.
    ///
    /// True if any alignment score is below critical threshold (0.55).
    pub is_misaligned: bool,

    /// Number of embedding spaces where this memory appeared.
    ///
    /// Higher = more cross-space relevance.
    pub space_count: usize,
}

impl ScoredMemory {
    /// Create a new scored memory with all teleological fields.
    pub fn new(
        memory_id: Uuid,
        score: f32,
        content_similarity: f32,
        purpose_alignment: f32,
        goal_alignment: f32,
        johari_quadrant: JohariQuadrant,
        space_count: usize,
    ) -> Self {
        // Critical threshold from constitution.yaml teleological.thresholds.critical
        let is_misaligned =
            purpose_alignment < alignment::CRITICAL || goal_alignment < alignment::CRITICAL;

        Self {
            memory_id,
            score,
            content_similarity,
            purpose_alignment,
            goal_alignment,
            johari_quadrant,
            is_misaligned,
            space_count,
        }
    }

    /// Create a scored memory with explicit misalignment flag.
    pub fn with_misalignment(mut self, is_misaligned: bool) -> Self {
        self.is_misaligned = is_misaligned;
        self
    }

    /// Check if alignment is optimal (≥ alignment::OPTIMAL).
    ///
    /// Constitution: `teleological.thresholds.optimal`
    #[inline]
    pub fn is_optimal(&self) -> bool {
        self.goal_alignment >= alignment::OPTIMAL
    }

    /// Check if alignment is acceptable (≥ alignment::ACCEPTABLE).
    ///
    /// Constitution: `teleological.thresholds.acceptable`
    #[inline]
    pub fn is_acceptable(&self) -> bool {
        self.goal_alignment >= alignment::ACCEPTABLE
    }

    /// Check if alignment needs attention (between WARNING and ACCEPTABLE).
    ///
    /// Constitution: `teleological.thresholds.warning`
    #[inline]
    pub fn needs_attention(&self) -> bool {
        self.goal_alignment >= alignment::WARNING && self.goal_alignment < alignment::ACCEPTABLE
    }

    /// Get alignment threshold classification.
    ///
    /// Uses thresholds from constitution.yaml teleological.thresholds.
    pub fn alignment_threshold(&self) -> AlignmentLevel {
        if self.goal_alignment >= alignment::OPTIMAL {
            AlignmentLevel::Optimal
        } else if self.goal_alignment >= alignment::ACCEPTABLE {
            AlignmentLevel::Acceptable
        } else if self.goal_alignment >= alignment::WARNING {
            AlignmentLevel::Warning
        } else {
            AlignmentLevel::Critical
        }
    }
}

/// Alignment level classification.
///
/// Thresholds from constitution.yaml teleological.thresholds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignmentLevel {
    /// θ ≥ OPTIMAL (0.75) - Excellent alignment
    Optimal,
    /// θ ∈ [ACCEPTABLE, OPTIMAL) - Good alignment
    Acceptable,
    /// θ ∈ [WARNING, ACCEPTABLE) - Needs improvement
    Warning,
    /// θ < CRITICAL (0.55) - Critical misalignment
    Critical,
}

impl AlignmentLevel {
    /// Get the minimum threshold for this level.
    ///
    /// Uses constants from `crate::config::constants::alignment`.
    pub fn min_threshold(self) -> f32 {
        match self {
            AlignmentLevel::Optimal => alignment::OPTIMAL,
            AlignmentLevel::Acceptable => alignment::ACCEPTABLE,
            AlignmentLevel::Warning => alignment::WARNING,
            AlignmentLevel::Critical => 0.0,
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

    /// Stage 4: Teleological filtering candidates.
    pub stage4_candidates: Vec<Uuid>,

    /// Stage 5: Late interaction final candidates.
    pub stage5_candidates: Vec<Uuid>,

    /// Number filtered out at Stage 4 due to alignment.
    pub stage4_filtered_count: usize,

    /// Average alignment of filtered candidates (for debugging).
    pub stage4_filtered_avg_alignment: f32,

    /// Number filtered out due to Johari quadrant.
    pub johari_filtered_count: usize,
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

    /// Set Stage 4 candidates and filtering info.
    pub fn with_stage4(
        mut self,
        candidates: Vec<Uuid>,
        filtered_count: usize,
        filtered_avg_alignment: f32,
    ) -> Self {
        self.stage4_candidates = candidates;
        self.stage4_filtered_count = filtered_count;
        self.stage4_filtered_avg_alignment = filtered_avg_alignment;
        self
    }

    /// Set Stage 5 candidates.
    pub fn with_stage5(mut self, candidates: Vec<Uuid>) -> Self {
        self.stage5_candidates = candidates;
        self
    }

    /// Set Johari filtering count.
    pub fn with_johari_filtered(mut self, count: usize) -> Self {
        self.johari_filtered_count = count;
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
            "S1:{} → S2:{} → S3:{} → S4:{} (filtered:{}) → S5:{}",
            self.stage1_candidates.len(),
            self.stage2_candidates.len(),
            self.stage3_candidates.len(),
            self.stage4_candidates.len(),
            self.stage4_filtered_count,
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
        let memory = ScoredMemory::new(id, 0.85, 0.90, 0.80, 0.75, JohariQuadrant::Open, 8);

        assert_eq!(memory.memory_id, id);
        assert!((memory.score - 0.85).abs() < f32::EPSILON);
        assert!((memory.content_similarity - 0.90).abs() < f32::EPSILON);
        assert!((memory.purpose_alignment - 0.80).abs() < f32::EPSILON);
        assert!((memory.goal_alignment - 0.75).abs() < f32::EPSILON);
        assert_eq!(memory.johari_quadrant, JohariQuadrant::Open);
        assert!(!memory.is_misaligned);
        assert_eq!(memory.space_count, 8);

        println!("[VERIFIED] ScoredMemory creation with all teleological fields");
    }

    #[test]
    fn test_scored_memory_misalignment_detection() {
        let id = Uuid::new_v4();

        // Below critical threshold (0.55)
        let misaligned = ScoredMemory::new(
            id,
            0.85,
            0.90,
            0.50, // Below 0.55
            0.60,
            JohariQuadrant::Open,
            8,
        );
        assert!(misaligned.is_misaligned);

        let misaligned2 = ScoredMemory::new(
            id,
            0.85,
            0.90,
            0.60,
            0.40, // Below 0.55
            JohariQuadrant::Open,
            8,
        );
        assert!(misaligned2.is_misaligned);

        // Above threshold
        let aligned = ScoredMemory::new(id, 0.85, 0.90, 0.60, 0.60, JohariQuadrant::Open, 8);
        assert!(!aligned.is_misaligned);

        println!("BEFORE: purpose_alignment=0.50, goal_alignment=0.40");
        println!(
            "AFTER: is_misaligned={}, is_misaligned2={}",
            misaligned.is_misaligned, misaligned2.is_misaligned
        );
        println!("[VERIFIED] Misalignment detection uses CRITICAL_THRESHOLD=0.55");
    }

    #[test]
    fn test_alignment_level_classification() {
        let id = Uuid::new_v4();

        let optimal = ScoredMemory::new(id, 0.9, 0.9, 0.9, 0.80, JohariQuadrant::Open, 8);
        assert_eq!(optimal.alignment_threshold(), AlignmentLevel::Optimal);
        assert!(optimal.is_optimal());

        let acceptable = ScoredMemory::new(id, 0.9, 0.9, 0.9, 0.72, JohariQuadrant::Open, 8);
        assert_eq!(acceptable.alignment_threshold(), AlignmentLevel::Acceptable);
        assert!(acceptable.is_acceptable());
        assert!(!acceptable.is_optimal());

        let warning = ScoredMemory::new(id, 0.9, 0.9, 0.9, 0.60, JohariQuadrant::Open, 8);
        assert_eq!(warning.alignment_threshold(), AlignmentLevel::Warning);
        assert!(warning.needs_attention());

        let critical = ScoredMemory::new(id, 0.9, 0.9, 0.9, 0.40, JohariQuadrant::Open, 8);
        assert_eq!(critical.alignment_threshold(), AlignmentLevel::Critical);
        assert!(critical.is_misaligned);

        println!("[VERIFIED] AlignmentLevel thresholds: Optimal≥0.75, Acceptable≥0.70, Warning≥0.55, Critical<0.55");
    }

    #[test]
    fn test_teleological_result_creation() {
        let id = Uuid::new_v4();
        let memory = ScoredMemory::new(id, 0.85, 0.90, 0.80, 0.75, JohariQuadrant::Open, 8);

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
            13,
            0,
        );

        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
        assert!(result.within_latency_target());
        assert!(result.all_stages_within_target());
        assert_eq!(result.spaces_searched, 13);
        assert_eq!(result.spaces_failed, 0);

        println!("[VERIFIED] TeleologicalRetrievalResult creation and latency checks");
    }

    #[test]
    fn test_result_filtering() {
        let results = vec![
            ScoredMemory::new(Uuid::new_v4(), 0.9, 0.9, 0.9, 0.80, JohariQuadrant::Open, 8),
            ScoredMemory::new(
                Uuid::new_v4(),
                0.8,
                0.8,
                0.8,
                0.65,
                JohariQuadrant::Blind,
                6,
            ),
            ScoredMemory::new(
                Uuid::new_v4(),
                0.7,
                0.7,
                0.5,
                0.40,
                JohariQuadrant::Hidden,
                4,
            ),
        ];

        let timing = PipelineStageTiming::default();
        let result = TeleologicalRetrievalResult::new(
            results,
            timing,
            std::time::Duration::from_millis(50),
            13,
            0,
        );

        // Filter by alignment
        let above_70 = result.results_above_alignment(0.70);
        assert_eq!(above_70.len(), 1);

        // Filter by quadrant
        let blind_hidden =
            result.results_in_quadrants(&[JohariQuadrant::Blind, JohariQuadrant::Hidden]);
        assert_eq!(blind_hidden.len(), 2);

        // Misaligned count
        assert_eq!(result.misaligned_count(), 1);

        println!("[VERIFIED] Result filtering by alignment and quadrant works");
    }

    #[test]
    fn test_pipeline_breakdown() {
        let ids: Vec<Uuid> = (0..100).map(|_| Uuid::new_v4()).collect();

        let breakdown = PipelineBreakdown::new()
            .with_stage1(ids[0..100].to_vec())
            .with_stage2(ids[0..50].to_vec())
            .with_stage3(ids[0..25].to_vec())
            .with_stage4(ids[0..15].to_vec(), 10, 0.48)
            .with_stage5(ids[0..10].to_vec());

        assert_eq!(breakdown.stage1_candidates.len(), 100);
        assert_eq!(breakdown.stage4_filtered_count, 10);
        assert!((breakdown.stage4_filtered_avg_alignment - 0.48).abs() < f32::EPSILON);

        let ratio = breakdown.reduction_ratio();
        assert!((ratio - 0.10).abs() < 0.001);

        let summary = breakdown.funnel_summary();
        assert!(summary.contains("S1:100"));
        assert!(summary.contains("filtered:10"));

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

    #[test]
    fn test_alignment_level_min_threshold() {
        assert!((AlignmentLevel::Optimal.min_threshold() - 0.75).abs() < f32::EPSILON);
        assert!((AlignmentLevel::Acceptable.min_threshold() - 0.70).abs() < f32::EPSILON);
        assert!((AlignmentLevel::Warning.min_threshold() - 0.55).abs() < f32::EPSILON);
        assert!((AlignmentLevel::Critical.min_threshold() - 0.0).abs() < f32::EPSILON);

        println!("[VERIFIED] AlignmentLevel min_threshold matches constitution.yaml");
    }
}
