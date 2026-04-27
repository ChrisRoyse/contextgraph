//! TASK-TELEO-012: FusionEngine Implementation
//!
//! Orchestrates alignment-based multi-embedding fusion.
//! Uses 13D alignment space without dimension projection (AP-03, AP-05).
//!
//! # From teleoplan.md
//!
//! "Fusion is not just concatenation - it's the INTERPLAY between embeddings
//! that creates meaning greater than the sum of its parts."

use crate::teleological::{types::NUM_EMBEDDERS, TeleologicalProfile};

use super::synergy_service::SynergyService;

/// Configuration for fusion engine.
#[derive(Clone, Debug)]
pub struct FusionEngineConfig {
    /// Minimum confidence to accept fusion result
    pub min_confidence: f32,
    /// Apply synergy amplification
    pub apply_synergy: bool,
    /// Enable Tucker compression (expensive)
    pub enable_tucker: bool,
    /// Profile-aware fusion
    pub profile_aware: bool,
}

impl Default for FusionEngineConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            apply_synergy: true,
            enable_tucker: false,
            profile_aware: true,
        }
    }
}

/// Result of alignment-based fusion.
///
/// Operates purely on 13D alignment space without dimension projection.
///
/// # Constitution Compliance
///
/// This result type is compliant with AP-03 and AP-05:
/// - **AP-03**: No dimension projection - alignments are 13D regardless of
///   underlying embedding dimensions
/// - **AP-05**: No embedding fusion into single vector - result is 13D
///   alignment scores, not a fused embedding
#[derive(Clone, Debug)]
pub struct AlignmentFusionResult {
    /// 13D topic profile with alignment scores.
    pub topic_profile: [f32; NUM_EMBEDDERS],

    /// Per-group alignment aggregation (semantic, temporal, structural, experiential).
    pub group_alignments: [f32; 4],

    /// Overall fusion confidence [0.0, 1.0].
    pub confidence: f32,

    /// Number of active embedders (alignment > 0.1).
    pub active_embedders: usize,

    /// Dominant group index (0=Semantic, 1=Temporal, 2=Structural, 3=Experiential).
    /// None if all groups have equal alignment.
    pub dominant_group: Option<usize>,
}

impl AlignmentFusionResult {
    /// Create a new AlignmentFusionResult.
    pub fn new(
        topic_profile: [f32; NUM_EMBEDDERS],
        group_alignments: [f32; 4],
        confidence: f32,
        active_embedders: usize,
        dominant_group: Option<usize>,
    ) -> Self {
        Self {
            topic_profile,
            group_alignments,
            confidence,
            active_embedders,
            dominant_group,
        }
    }

    /// Check if fusion result indicates healthy alignment.
    ///
    /// Returns true if confidence >= 0.7 and at least 50% of embedders active.
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.confidence >= 0.7 && self.active_embedders >= 7
    }
}

/// TELEO-012: Orchestrates multi-embedding fusion.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::teleological::services::FusionEngine;
///
/// let engine = FusionEngine::new();
/// let embeddings = vec![vec![0.0f32; 1024]; 14];
/// let alignments = [0.8f32; 14];
/// let result = engine.fuse_from_alignments(&alignments);
/// ```
pub struct FusionEngine {
    config: FusionEngineConfig,
    synergy_service: SynergyService,
    active_profile: Option<TeleologicalProfile>,
}

impl FusionEngine {
    /// Create a new FusionEngine with default configuration.
    pub fn new() -> Self {
        Self {
            config: FusionEngineConfig::default(),
            synergy_service: SynergyService::new(),
            active_profile: None,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: FusionEngineConfig) -> Self {
        Self {
            config,
            synergy_service: SynergyService::new(),
            active_profile: None,
        }
    }

    /// Set the active profile for fusion.
    pub fn set_profile(&mut self, profile: TeleologicalProfile) {
        self.active_profile = Some(profile);
    }

    /// Clear the active profile.
    pub fn clear_profile(&mut self) {
        self.active_profile = None;
    }

    /// Get synergy service reference.
    pub fn synergy_service(&self) -> &SynergyService {
        &self.synergy_service
    }

    /// Get mutable synergy service.
    pub fn synergy_service_mut(&mut self) -> &mut SynergyService {
        &mut self.synergy_service
    }

    /// Get configuration.
    pub fn config(&self) -> &FusionEngineConfig {
        &self.config
    }

    /// Get active profile.
    pub fn active_profile(&self) -> Option<&TeleologicalProfile> {
        self.active_profile.as_ref()
    }

    /// Perform alignment-based fusion without dimension projection.
    ///
    /// This is the constitution-compliant fusion method that operates on
    /// topic profile alignments rather than raw embeddings.
    ///
    /// # Constitution Compliance
    ///
    /// - **AP-03**: No dimension projection - alignments are 13D regardless of
    ///   underlying embedding dimensions
    /// - **AP-05**: No embedding fusion into single vector - result is 13D
    ///   alignment scores, not a fused embedding
    ///
    /// # Arguments
    /// * `alignments` - 13D topic profile alignments (computed per-embedder in native space)
    ///
    /// # Returns
    /// `AlignmentFusionResult` containing:
    /// - topic_profile: The 13D alignment scores
    /// - group_alignments: 4D group aggregation (Semantic, Temporal, Structural, Experiential)
    /// - confidence: Overall fusion confidence
    /// - active_embedders: Count of embedders with alignment > 0.1
    /// - dominant_group: Index of highest-scoring group
    ///
    /// # Panics
    ///
    /// Panics (FAIL FAST) if any alignment is outside [0.0, 1.0] range.
    pub fn fuse_from_alignments(&self, alignments: &[f32; NUM_EMBEDDERS]) -> AlignmentFusionResult {
        // Validate alignments are in valid range [0.0, 1.0]
        for (i, &a) in alignments.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(&a),
                "FAIL FAST: Alignment {} has value {}, expected [0.0, 1.0]. \
                 See TECH-FUSION-001 for valid alignment computation.",
                i,
                a
            );
        }

        // Step 1: Compute topic profile score
        let tp_score = alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32;

        // Step 2: Aggregate to groups
        // Groups: Semantic (E1-E3), Temporal (E4-E6), Structural (E7-E9), Experiential (E10-E13)
        let group_alignments = [
            (alignments[0] + alignments[1] + alignments[2]) / 3.0, // Semantic
            (alignments[3] + alignments[4] + alignments[5]) / 3.0, // Temporal
            (alignments[6] + alignments[7] + alignments[8]) / 3.0, // Structural
            (alignments[9] + alignments[10] + alignments[11] + alignments[12]) / 4.0, // Experiential
        ];
        let group_score = group_alignments.iter().sum::<f32>() / 4.0;

        // Step 3: Compute confidence
        // Weighted: TP (50%), Groups (50%)
        let active_embedders = alignments.iter().filter(|&&a| a > 0.1).count();
        let coverage = active_embedders as f32 / NUM_EMBEDDERS as f32;
        let confidence = (tp_score * 0.5 + group_score * 0.5) * coverage;

        // Step 4: Determine dominant group
        let dominant_group = group_alignments
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i);

        AlignmentFusionResult::new(
            *alignments,
            group_alignments,
            confidence,
            active_embedders,
            dominant_group,
        )
    }
}

impl Default for FusionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleological::NUM_EMBEDDERS;

    #[test]
    fn test_fusion_engine_new() {
        let engine = FusionEngine::new();
        assert!(engine.config().apply_synergy);
        assert!(engine.config().profile_aware);
    }

    #[test]
    fn test_set_clear_profile() {
        let mut engine = FusionEngine::new();

        assert!(engine.active_profile().is_none());

        let profile = TeleologicalProfile::code_implementation();
        engine.set_profile(profile);
        assert!(engine.active_profile().is_some());

        engine.clear_profile();
        assert!(engine.active_profile().is_none());
    }

    #[test]
    fn test_synergy_service_access() {
        let mut engine = FusionEngine::new();

        let _ = engine.synergy_service().total_samples();
        let _ = engine.synergy_service_mut().total_samples();
    }

    #[test]
    fn test_alignment_fusion_result() {
        // Test instantiation with healthy values
        let topic_profile = [0.8f32; NUM_EMBEDDERS];
        let group_alignments = [0.9, 0.7, 0.6, 0.8];
        let result = AlignmentFusionResult::new(
            topic_profile,
            group_alignments,
            0.85,    // confidence
            10,      // active_embedders (10 out of 13)
            Some(0), // dominant_group: Semantic
        );

        assert!(
            result.is_healthy(),
            "Should be healthy with confidence=0.85 and 10 active embedders"
        );
        assert_eq!(result.topic_profile.len(), NUM_EMBEDDERS);
        assert_eq!(result.group_alignments.len(), 4);
        assert_eq!(result.confidence, 0.85);
        assert_eq!(result.active_embedders, 10);
        assert_eq!(result.dominant_group, Some(0));

        println!("[PASS] AlignmentFusionResult instantiation and is_healthy()");
    }

    #[test]
    fn test_alignment_fusion_result_unhealthy() {
        // Test unhealthy case: low confidence
        let low_confidence = AlignmentFusionResult::new(
            [0.5f32; NUM_EMBEDDERS],
            [0.5, 0.5, 0.5, 0.5],
            0.5, // below 0.7 threshold
            10,
            None,
        );
        assert!(
            !low_confidence.is_healthy(),
            "Low confidence should not be healthy"
        );

        // Test unhealthy case: too few active embedders
        let few_active = AlignmentFusionResult::new(
            [0.5f32; NUM_EMBEDDERS],
            [0.5, 0.5, 0.5, 0.5],
            0.8, // good confidence
            5,   // below 7 threshold (50% of 13)
            None,
        );
        assert!(
            !few_active.is_healthy(),
            "Too few active embedders should not be healthy"
        );

        println!("[PASS] AlignmentFusionResult unhealthy edge cases");
    }

    #[test]
    fn test_alignment_fusion_result_clone_debug() {
        let result = AlignmentFusionResult::new(
            [0.5f32; NUM_EMBEDDERS],
            [0.6, 0.7, 0.8, 0.9],
            0.75,
            8,
            Some(3), // Experiential dominant
        );

        // Test Clone
        let cloned = result.clone();
        assert_eq!(cloned.confidence, result.confidence);
        assert_eq!(cloned.active_embedders, result.active_embedders);

        // Test Debug
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("AlignmentFusionResult"));
        assert!(debug_str.contains("confidence"));

        println!("[PASS] AlignmentFusionResult Clone and Debug traits");
    }

    // ============================================================================
    // TASK-LOGIC-001: Tests for fuse_from_alignments() returning AlignmentFusionResult
    // ============================================================================

    #[test]
    fn test_fuse_from_alignments_valid_input() {
        let engine = FusionEngine::new();
        let alignments = [0.8f32; NUM_EMBEDDERS];

        let result = engine.fuse_from_alignments(&alignments);

        // Verify topic_profile matches input alignments
        assert_eq!(result.topic_profile, alignments);

        // Verify group_alignments are computed correctly
        // All alignments are 0.8, so all groups should be 0.8
        for group in &result.group_alignments {
            assert!(
                (group - 0.8).abs() < 0.001,
                "Group alignment should be ~0.8"
            );
        }

        // Verify active_embedders count (all are above 0.1)
        assert_eq!(result.active_embedders, NUM_EMBEDDERS);

        // Verify confidence is positive
        assert!(result.confidence > 0.0);
        assert!(result.confidence <= 1.0);

        // Verify dominant_group is set
        assert!(result.dominant_group.is_some());

        println!("[PASS] fuse_from_alignments produces valid AlignmentFusionResult");
    }

    #[test]
    fn test_fuse_from_alignments_group_aggregation() {
        let engine = FusionEngine::new();

        // Create alignments with distinct group patterns
        // Semantic (E1-E3): high
        // Temporal (E4-E6): medium
        // Structural (E7-E9): low
        // Experiential (E10-E13): medium-high
        let alignments = [
            0.9, 0.9, 0.9, // Semantic: should average to 0.9
            0.5, 0.5, 0.5, // Temporal: should average to 0.5
            0.2, 0.2, 0.2, // Structural: should average to 0.2
            0.7, 0.7, 0.7, 0.7, // Experiential: should average to 0.7
            0.8, // E14 BGE-M3 Dense (14th embedder)
        ];

        let result = engine.fuse_from_alignments(&alignments);

        // Verify group aggregation
        assert!(
            (result.group_alignments[0] - 0.9).abs() < 0.001,
            "Semantic should be 0.9"
        );
        assert!(
            (result.group_alignments[1] - 0.5).abs() < 0.001,
            "Temporal should be 0.5"
        );
        assert!(
            (result.group_alignments[2] - 0.2).abs() < 0.001,
            "Structural should be 0.2"
        );
        assert!(
            (result.group_alignments[3] - 0.7).abs() < 0.001,
            "Experiential should be 0.7"
        );

        // Semantic should be dominant
        assert_eq!(
            result.dominant_group,
            Some(0),
            "Semantic group should be dominant"
        );

        println!("[PASS] fuse_from_alignments correctly aggregates groups");
    }

    #[test]
    fn test_fuse_from_alignments_active_embedders_count() {
        let engine = FusionEngine::new();

        // Only 5 embedders active (above 0.1 threshold)
        let mut alignments = [0.0f32; NUM_EMBEDDERS];
        alignments[0] = 0.5;
        alignments[3] = 0.6;
        alignments[6] = 0.7;
        alignments[9] = 0.8;
        alignments[12] = 0.9;

        let result = engine.fuse_from_alignments(&alignments);

        assert_eq!(result.active_embedders, 5, "Should have 5 active embedders");

        println!("[PASS] fuse_from_alignments correctly counts active embedders");
    }

    #[test]
    fn test_fuse_from_alignments_boundary_values() {
        let engine = FusionEngine::new();

        // Test boundary values: exactly 0.0 and 1.0
        let mut alignments = [0.0f32; NUM_EMBEDDERS];
        alignments[0] = 1.0; // Maximum valid
        alignments[1] = 0.0; // Minimum valid

        let result = engine.fuse_from_alignments(&alignments);

        // Should not panic, should return valid result
        assert!(result.confidence >= 0.0);
        assert!(result.confidence <= 1.0);

        println!("[PASS] fuse_from_alignments handles boundary values [0.0, 1.0]");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_fuse_from_alignments_panics_on_above_one() {
        let engine = FusionEngine::new();

        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        alignments[5] = 1.5; // Invalid: above 1.0

        let _ = engine.fuse_from_alignments(&alignments);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_fuse_from_alignments_panics_on_negative() {
        let engine = FusionEngine::new();

        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        alignments[3] = -0.1; // Invalid: negative

        let _ = engine.fuse_from_alignments(&alignments);
    }

    #[test]
    fn test_fuse_from_alignments_confidence_formula() {
        let engine = FusionEngine::new();

        // All alignments at 0.8, all active
        let alignments = [0.8f32; NUM_EMBEDDERS];

        let result = engine.fuse_from_alignments(&alignments);

        // Expected:
        // pv_score = 0.8 (average of all 0.8s)
        // group_score = 0.8 (average of all groups which are all 0.8)
        // coverage = 13/13 = 1.0 (all active)
        // confidence = (0.8 * 0.5 + 0.8 * 0.5) * 1.0 = 0.8
        assert!(
            (result.confidence - 0.8).abs() < 0.001,
            "Expected confidence ~0.8, got {}",
            result.confidence
        );

        println!("[PASS] fuse_from_alignments confidence formula is correct");
    }

    #[test]
    fn test_fuse_from_alignments_returns_alignment_fusion_result_type() {
        let engine = FusionEngine::new();
        let alignments = [0.5f32; NUM_EMBEDDERS];

        // This test verifies the return type at compile time
        let result: AlignmentFusionResult = engine.fuse_from_alignments(&alignments);

        // Verify it has all expected fields
        let _ = result.topic_profile;
        let _ = result.group_alignments;
        let _ = result.confidence;
        let _ = result.active_embedders;
        let _ = result.dominant_group;

        println!("[PASS] fuse_from_alignments returns AlignmentFusionResult type");
    }
}
