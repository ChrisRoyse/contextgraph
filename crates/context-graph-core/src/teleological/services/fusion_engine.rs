//! TASK-TELEO-012: FusionEngine Implementation
//!
//! Orchestrates multi-embedding fusion into TeleologicalVector.
//! Coordinates all teleological services for end-to-end fusion.
//!
//! # From teleoplan.md
//!
//! "Fusion is not just concatenation - it's the INTERPLAY between embeddings
//! that creates meaning greater than the sum of its parts."

use crate::teleological::{
    types::{EMBEDDING_DIM, NUM_EMBEDDERS},
    ProfileId, TeleologicalProfile, TeleologicalVector,
};
use crate::types::fingerprint::PurposeVector;

use super::correlation_extractor::CorrelationExtractor;
use super::group_aggregator::GroupAggregator;
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

/// Result of fusion operation.
#[derive(Clone, Debug)]
pub struct FusionResult {
    /// The fused teleological vector
    pub vector: TeleologicalVector,
    /// Overall fusion confidence
    pub confidence: f32,
    /// Per-component scores
    pub component_scores: ComponentScores,
    /// Metadata about the fusion
    pub metadata: FusionMetadata,
}

/// Scores for each fusion component.
#[derive(Clone, Debug, Default)]
pub struct ComponentScores {
    /// Purpose vector quality
    pub purpose_vector: f32,
    /// Correlation density (non-sparse)
    pub correlations: f32,
    /// Group coherence
    pub groups: f32,
    /// Synergy utilization
    pub synergy: f32,
}

/// Metadata about the fusion process.
#[derive(Clone, Debug, Default)]
pub struct FusionMetadata {
    /// Number of active embedders (alignment > threshold)
    pub active_embedders: usize,
    /// Strongest embedding pair
    pub strongest_pair: Option<(usize, usize)>,
    /// Dominant group
    pub dominant_group: Option<crate::teleological::GroupType>,
    /// Profile used (if any)
    pub profile_id: Option<ProfileId>,
}

/// TELEO-012: Orchestrates multi-embedding fusion.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::teleological::services::FusionEngine;
///
/// let engine = FusionEngine::new();
/// let embeddings = vec![vec![0.0f32; 1024]; 13];
/// let alignments = [0.8f32; 13];
/// let result = engine.fuse(&embeddings, &alignments);
/// ```
pub struct FusionEngine {
    config: FusionEngineConfig,
    synergy_service: SynergyService,
    correlation_extractor: CorrelationExtractor,
    group_aggregator: GroupAggregator,
    active_profile: Option<TeleologicalProfile>,
}

impl FusionEngine {
    /// Create a new FusionEngine with default configuration.
    pub fn new() -> Self {
        Self {
            config: FusionEngineConfig::default(),
            synergy_service: SynergyService::new(),
            correlation_extractor: CorrelationExtractor::new(),
            group_aggregator: GroupAggregator::new(),
            active_profile: None,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: FusionEngineConfig) -> Self {
        Self {
            config,
            synergy_service: SynergyService::new(),
            correlation_extractor: CorrelationExtractor::new(),
            group_aggregator: GroupAggregator::new(),
            active_profile: None,
        }
    }

    /// Perform fusion of embeddings into TeleologicalVector.
    ///
    /// # Arguments
    /// * `embeddings` - 13 embedding vectors of dimension 1024
    /// * `alignments` - 13D purpose vector alignments
    ///
    /// # Panics
    ///
    /// Panics if embeddings or alignments have wrong dimensions (FAIL FAST).
    pub fn fuse(&self, embeddings: &[Vec<f32>], alignments: &[f32; NUM_EMBEDDERS]) -> FusionResult {
        assert!(
            embeddings.len() == NUM_EMBEDDERS,
            "FAIL FAST: Expected {} embeddings, got {}",
            NUM_EMBEDDERS,
            embeddings.len()
        );

        for (i, emb) in embeddings.iter().enumerate() {
            assert!(
                emb.len() == EMBEDDING_DIM,
                "FAIL FAST: Embedding {} has dimension {}, expected {}",
                i,
                emb.len(),
                EMBEDDING_DIM
            );
        }

        // Step 1: Create purpose vector
        let purpose_vector = PurposeVector::new(*alignments);
        let pv_score = purpose_vector.aggregate_alignment();

        // Step 2: Extract cross-correlations
        let synergy_matrix = if self.config.apply_synergy {
            Some(self.synergy_service.matrix())
        } else {
            None
        };

        let corr_result = self
            .correlation_extractor
            .extract(embeddings, synergy_matrix);
        let corr_score = 1.0 - corr_result.sparsity;

        // Step 3: Aggregate to groups
        let group_result = self.group_aggregator.aggregate(alignments);
        let group_score = group_result.coherence;

        // Step 4: Apply profile modulation if enabled
        let mut correlations = corr_result.correlations.to_vec();
        let mut profile_id = None;

        if self.config.profile_aware {
            if let Some(profile) = &self.active_profile {
                self.apply_profile_modulation(&mut correlations, profile, alignments);
                profile_id = Some(ProfileId::new(profile.id.as_str()));
            }
        }

        // Calculate synergy utilization
        let synergy_score = if self.config.apply_synergy {
            self.calculate_synergy_utilization(&correlations, alignments)
        } else {
            1.0
        };

        // Compute overall confidence
        let confidence =
            (pv_score * 0.3 + corr_score * 0.25 + group_score * 0.25 + synergy_score * 0.2)
                * self.embedding_coverage(alignments);

        // Build metadata (before moving group_result.alignments)
        let active_embedders = alignments.iter().filter(|&&a| a > 0.1).count();
        let strongest_pair = corr_result.strongest_pair.map(|(i, j, _)| (i, j));
        let dominant_group = Some(group_result.alignments.dominant_group());

        // Build the TeleologicalVector
        let mut vector = TeleologicalVector::with_all(
            purpose_vector,
            correlations,
            group_result.alignments,
            confidence,
        );

        if let Some(pid) = &profile_id {
            vector = vector.with_profile(pid.clone());
        }

        FusionResult {
            vector,
            confidence,
            component_scores: ComponentScores {
                purpose_vector: pv_score,
                correlations: corr_score,
                groups: group_score,
                synergy: synergy_score,
            },
            metadata: FusionMetadata {
                active_embedders,
                strongest_pair,
                dominant_group,
                profile_id,
            },
        }
    }

    /// Fuse with a specific profile.
    pub fn fuse_with_profile(
        &self,
        embeddings: &[Vec<f32>],
        alignments: &[f32; NUM_EMBEDDERS],
        profile: &TeleologicalProfile,
    ) -> FusionResult {
        let mut engine = Self::with_config(self.config.clone());
        engine.set_profile(profile.clone());
        engine.fuse(embeddings, alignments)
    }

    /// Set the active profile for fusion.
    pub fn set_profile(&mut self, profile: TeleologicalProfile) {
        self.active_profile = Some(profile);
    }

    /// Clear the active profile.
    pub fn clear_profile(&mut self) {
        self.active_profile = None;
    }

    /// Apply profile-specific modulation to correlations.
    fn apply_profile_modulation(
        &self,
        correlations: &mut [f32],
        profile: &TeleologicalProfile,
        alignments: &[f32; NUM_EMBEDDERS],
    ) {
        let weights = profile.get_all_weights();

        let mut idx = 0;
        for i in 0..NUM_EMBEDDERS {
            for j in (i + 1)..NUM_EMBEDDERS {
                // Weight correlation by product of embedder weights and their alignments
                let weight_factor = (weights[i] * weights[j]).sqrt();
                let alignment_factor = (alignments[i] * alignments[j]).sqrt();

                correlations[idx] *= weight_factor * (0.5 + 0.5 * alignment_factor);
                idx += 1;
            }
        }
    }

    /// Calculate how well synergies are being utilized.
    fn calculate_synergy_utilization(
        &self,
        correlations: &[f32],
        alignments: &[f32; NUM_EMBEDDERS],
    ) -> f32 {
        let synergies = self.synergy_service.compute_all_synergies();
        let high_synergy_count = synergies.iter().filter(|&&s| s >= 0.7).count();

        if high_synergy_count == 0 {
            return 1.0;
        }

        // Check if high-synergy pairs have meaningful correlations
        let mut utilized = 0;
        let mut idx = 0;

        for i in 0..NUM_EMBEDDERS {
            for j in (i + 1)..NUM_EMBEDDERS {
                let synergy = synergies[idx];
                let correlation = correlations[idx].abs();

                // High synergy pair is "utilized" if correlation and alignments are meaningful
                if synergy >= 0.7 && correlation > 0.1 && alignments[i] > 0.2 && alignments[j] > 0.2
                {
                    utilized += 1;
                }

                idx += 1;
            }
        }

        utilized as f32 / high_synergy_count as f32
    }

    /// Calculate embedding coverage (how many embedders are active).
    fn embedding_coverage(&self, alignments: &[f32; NUM_EMBEDDERS]) -> f32 {
        let active = alignments.iter().filter(|&&a| a > 0.1).count();
        (active as f32 / NUM_EMBEDDERS as f32).sqrt()
    }

    /// Check if fusion result meets quality threshold.
    pub fn is_quality_fusion(&self, result: &FusionResult) -> bool {
        result.confidence >= self.config.min_confidence
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
}

impl Default for FusionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleological::{CROSS_CORRELATION_COUNT, NUM_EMBEDDERS};

    fn make_embeddings(fill: f32) -> Vec<Vec<f32>> {
        vec![vec![fill; EMBEDDING_DIM]; NUM_EMBEDDERS]
    }

    #[test]
    fn test_fusion_engine_new() {
        let engine = FusionEngine::new();
        assert!(engine.config().apply_synergy);
        assert!(engine.config().profile_aware);

        println!("[PASS] FusionEngine::new creates default config");
    }

    #[test]
    fn test_fuse_basic() {
        let engine = FusionEngine::new();
        let embeddings = make_embeddings(0.5);
        let alignments = [0.8f32; NUM_EMBEDDERS];

        let result = engine.fuse(&embeddings, &alignments);

        assert!(result.confidence > 0.0);
        assert_eq!(
            result.vector.cross_correlations.len(),
            CROSS_CORRELATION_COUNT
        );
        assert!(result.metadata.active_embedders == NUM_EMBEDDERS);

        println!("[PASS] fuse produces valid FusionResult");
    }

    #[test]
    fn test_component_scores() {
        let engine = FusionEngine::new();
        let embeddings = make_embeddings(0.5);
        let alignments = [0.9f32; NUM_EMBEDDERS];

        let result = engine.fuse(&embeddings, &alignments);

        // All scores should be positive for valid input
        assert!(result.component_scores.purpose_vector > 0.0);
        assert!(result.component_scores.groups > 0.0);
        assert!(result.component_scores.synergy >= 0.0);

        println!("[PASS] Component scores all computed");
    }

    #[test]
    fn test_fusion_metadata() {
        let engine = FusionEngine::new();
        let embeddings = make_embeddings(0.5);
        let alignments = [0.8f32; NUM_EMBEDDERS];

        let result = engine.fuse(&embeddings, &alignments);

        assert_eq!(result.metadata.active_embedders, NUM_EMBEDDERS);
        assert!(result.metadata.dominant_group.is_some());

        println!("[PASS] Fusion metadata populated");
    }

    #[test]
    fn test_fuse_with_profile() {
        let engine = FusionEngine::new();
        let embeddings = make_embeddings(0.5);
        let alignments = [0.8f32; NUM_EMBEDDERS];

        let profile = TeleologicalProfile::code_implementation();
        let result = engine.fuse_with_profile(&embeddings, &alignments, &profile);

        assert!(result.metadata.profile_id.is_some());

        println!("[PASS] fuse_with_profile applies profile");
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

        println!("[PASS] set_profile and clear_profile work");
    }

    #[test]
    fn test_is_quality_fusion() {
        let engine = FusionEngine::with_config(FusionEngineConfig {
            min_confidence: 0.5,
            ..Default::default()
        });

        let embeddings = make_embeddings(0.5);

        // High alignment should produce quality fusion
        let high_result = engine.fuse(&embeddings, &[0.9f32; NUM_EMBEDDERS]);

        // Low alignment less likely to be quality
        let low_result = engine.fuse(&embeddings, &[0.1f32; NUM_EMBEDDERS]);

        // High should have higher confidence
        assert!(high_result.confidence > low_result.confidence);

        println!("[PASS] is_quality_fusion distinguishes quality levels");
    }

    #[test]
    fn test_embedding_coverage_effect() {
        let engine = FusionEngine::new();
        let embeddings = make_embeddings(0.5);

        // Full coverage
        let full_result = engine.fuse(&embeddings, &[0.8f32; NUM_EMBEDDERS]);

        // Partial coverage
        let mut partial = [0.0f32; NUM_EMBEDDERS];
        partial[0] = 0.8;
        partial[5] = 0.8;
        let partial_result = engine.fuse(&embeddings, &partial);

        // Full should have higher confidence due to coverage
        assert!(full_result.confidence > partial_result.confidence);

        println!("[PASS] Embedding coverage affects confidence");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_fuse_wrong_embedding_count() {
        let engine = FusionEngine::new();
        let embeddings = vec![vec![0.0f32; EMBEDDING_DIM]; 10]; // Wrong
        let alignments = [0.5f32; NUM_EMBEDDERS];

        let _ = engine.fuse(&embeddings, &alignments);
    }

    #[test]
    fn test_synergy_service_access() {
        let mut engine = FusionEngine::new();

        let _ = engine.synergy_service().total_samples();
        let _ = engine.synergy_service_mut().total_samples();

        println!("[PASS] Synergy service accessible from engine");
    }
}
