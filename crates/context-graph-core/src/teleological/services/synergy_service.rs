//! TASK-TELEO-007: SynergyService Implementation
//!
//! Computes cross-embedding synergies in real-time. The service manages the 13x13
//! synergy matrix, updating weights based on co-activation patterns and GWT feedback.
//!
//! # Core Responsibilities
//!
//! 1. Compute synergy values between embedding pairs
//! 2. Update synergy weights from usage patterns
//! 3. Apply task-specific synergy modulation
//! 4. Integrate with GWT consciousness feedback
//!
//! # From teleoplan.md
//!
//! "The synergy matrix captures how much embeddings 'resonate' with each other -
//! high synergy pairs should have their cross-correlations amplified."

use chrono::Utc;

use crate::teleological::{
    ProfileId, SynergyMatrix, TeleologicalProfile, CROSS_CORRELATION_COUNT, SYNERGY_DIM,
};
use crate::types::fingerprint::PurposeVector;

/// Configuration for synergy computation.
#[derive(Clone, Debug)]
pub struct SynergyConfig {
    /// Learning rate for EWMA synergy updates
    pub learning_rate: f32,
    /// Minimum synergy threshold (values below are clamped)
    pub min_synergy: f32,
    /// Maximum synergy threshold (values above are clamped)
    pub max_synergy: f32,
    /// Decay factor for unused synergy pairs
    pub decay_factor: f32,
    /// Enable GWT-aware synergy modulation
    pub gwt_modulation: bool,
}

impl Default for SynergyConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.05,
            min_synergy: 0.1,
            max_synergy: 1.0,
            decay_factor: 0.99,
            gwt_modulation: true,
        }
    }
}

/// Feedback from synergy computation for learning.
#[derive(Clone, Debug)]
pub struct SynergyFeedback {
    /// Embedding pair indices
    pub pair: (usize, usize),
    /// Whether the pair was useful (co-activated in successful retrieval)
    pub success: bool,
    /// Strength of co-activation [0.0, 1.0]
    pub activation_strength: f32,
    /// Optional GWT consciousness score at time of activation
    pub consciousness_score: Option<f32>,
}

/// Result of synergy computation.
#[derive(Clone, Debug)]
pub struct SynergyResult {
    /// Computed synergy value
    pub synergy: f32,
    /// Weighted synergy (synergy * weight)
    pub weighted_synergy: f32,
    /// Confidence based on sample count
    pub confidence: f32,
    /// Whether this is a high-synergy pair (>= 0.7)
    pub is_high_synergy: bool,
}

/// TELEO-007: Service for computing and managing cross-embedding synergies.
///
/// # Example
///
/// ```
/// use context_graph_core::teleological::services::SynergyService;
/// use context_graph_core::teleological::SynergyMatrix;
///
/// let mut service = SynergyService::new();
/// let result = service.compute_synergy(0, 4); // E1_Semantic + E5_Analogical
/// assert!(result.synergy > 0.8); // Should be strong synergy
/// ```
pub struct SynergyService {
    /// The managed synergy matrix
    matrix: SynergyMatrix,
    /// Service configuration
    config: SynergyConfig,
    /// Per-pair activation counts (for confidence computation)
    activation_counts: [[u64; SYNERGY_DIM]; SYNERGY_DIM],
    /// Active profile (affects synergy modulation)
    active_profile: Option<ProfileId>,
}

impl SynergyService {
    /// Create a new SynergyService with base synergies.
    pub fn new() -> Self {
        Self {
            matrix: SynergyMatrix::with_base_synergies(),
            config: SynergyConfig::default(),
            activation_counts: [[0u64; SYNERGY_DIM]; SYNERGY_DIM],
            active_profile: None,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: SynergyConfig) -> Self {
        Self {
            matrix: SynergyMatrix::with_base_synergies(),
            config,
            activation_counts: [[0u64; SYNERGY_DIM]; SYNERGY_DIM],
            active_profile: None,
        }
    }

    /// Create with existing matrix and configuration.
    pub fn with_matrix(matrix: SynergyMatrix, config: SynergyConfig) -> Self {
        Self {
            matrix,
            config,
            activation_counts: [[0u64; SYNERGY_DIM]; SYNERGY_DIM],
            active_profile: None,
        }
    }

    /// Compute synergy between two embeddings.
    ///
    /// # Arguments
    /// * `i` - First embedding index (0-12)
    /// * `j` - Second embedding index (0-12)
    ///
    /// # Panics
    ///
    /// Panics if indices are out of bounds (FAIL FAST).
    pub fn compute_synergy(&self, i: usize, j: usize) -> SynergyResult {
        assert!(
            i < SYNERGY_DIM && j < SYNERGY_DIM,
            "FAIL FAST: synergy indices ({}, {}) out of bounds (max {})",
            i,
            j,
            SYNERGY_DIM - 1
        );

        let synergy = self.matrix.get_synergy(i, j);
        let weighted_synergy = self.matrix.get_weighted_synergy(i, j);

        // Confidence based on activation count
        let count = self.activation_counts[i][j];
        let confidence = 1.0 - 1.0 / (1.0 + count as f32 / 10.0);

        SynergyResult {
            synergy,
            weighted_synergy,
            confidence,
            is_high_synergy: synergy >= 0.7,
        }
    }

    /// Compute all 78 synergy values for cross-correlations.
    ///
    /// Returns synergy values in flat upper-triangle order.
    pub fn compute_all_synergies(&self) -> [f32; CROSS_CORRELATION_COUNT] {
        self.matrix.to_cross_correlations()
    }

    /// Compute weighted synergies for cross-correlations.
    ///
    /// Returns synergy * weight for each pair.
    pub fn compute_weighted_synergies(&self) -> [f32; CROSS_CORRELATION_COUNT] {
        let mut result = [0.0f32; CROSS_CORRELATION_COUNT];
        let mut idx = 0;

        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                result[idx] = self.matrix.get_weighted_synergy(i, j);
                idx += 1;
            }
        }

        result
    }

    /// Apply feedback to update synergy values.
    ///
    /// Uses EWMA (Exponential Weighted Moving Average) to update synergies
    /// based on whether embedding pairs were useful in retrieval.
    pub fn apply_feedback(&mut self, feedback: &SynergyFeedback) {
        let (i, j) = feedback.pair;

        assert!(
            i < SYNERGY_DIM && j < SYNERGY_DIM,
            "FAIL FAST: feedback pair ({}, {}) out of bounds",
            i,
            j
        );
        assert!(
            i != j,
            "FAIL FAST: feedback pair cannot be same index ({}, {})",
            i,
            j
        );

        // Update activation count
        self.activation_counts[i][j] = self.activation_counts[i][j].saturating_add(1);
        self.activation_counts[j][i] = self.activation_counts[i][j];

        // Compute target synergy based on feedback
        let target = if feedback.success {
            // Increase synergy for successful co-activations
            let base = self.matrix.get_synergy(i, j);
            let boost = feedback.activation_strength * 0.1;

            // GWT modulation: high consciousness = stronger learning
            let gwt_factor = feedback.consciousness_score.map_or(1.0, |c| 0.8 + 0.4 * c);

            (base + boost * gwt_factor).min(self.config.max_synergy)
        } else {
            // Slight decrease for unsuccessful co-activations
            let base = self.matrix.get_synergy(i, j);
            (base * 0.98).max(self.config.min_synergy)
        };

        // EWMA update (only for off-diagonal)
        if i != j {
            let current = self.matrix.get_synergy(i, j);
            let updated =
                current * (1.0 - self.config.learning_rate) + target * self.config.learning_rate;

            // Clamp to valid range
            let clamped = updated.clamp(self.config.min_synergy, self.config.max_synergy);

            self.matrix.set_synergy(i, j, clamped);
        }

        // Increment sample count
        self.matrix.sample_count = self.matrix.sample_count.saturating_add(1);
        self.matrix.computed_at = Utc::now();
    }

    /// Apply profile-specific synergy modulation.
    ///
    /// Modifies synergy weights based on task requirements.
    pub fn apply_profile_modulation(&mut self, profile: &TeleologicalProfile) {
        let weights = profile.get_all_weights();

        // For each pair, weight synergy by product of embedding weights
        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                let pair_weight = (weights[i] * weights[j]).sqrt();
                let current_weight = self.matrix.get_weight(i, j);

                // Blend with profile weight
                let new_weight = current_weight * 0.7 + pair_weight * 0.3;
                self.matrix.set_weight(i, j, new_weight);
            }
        }

        self.active_profile = Some(ProfileId::new(profile.id.as_str()));
    }

    /// Apply decay to all synergies (for unused pairs).
    ///
    /// Should be called periodically to prevent stale synergies.
    pub fn apply_decay(&mut self) {
        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                let current = self.matrix.get_synergy(i, j);

                // Decay towards base synergy
                let base = SynergyMatrix::BASE_SYNERGIES[i][j];
                let decayed =
                    current * self.config.decay_factor + base * (1.0 - self.config.decay_factor);

                self.matrix.set_synergy(i, j, decayed);
            }
        }
    }

    /// Get synergies modulated by purpose vector alignment.
    ///
    /// Higher alignment = stronger synergy contribution.
    pub fn get_aligned_synergies(
        &self,
        purpose_vector: &PurposeVector,
    ) -> [f32; CROSS_CORRELATION_COUNT] {
        let alignments = purpose_vector.alignments;
        let mut result = [0.0f32; CROSS_CORRELATION_COUNT];
        let mut idx = 0;

        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                let synergy = self.matrix.get_synergy(i, j);
                let alignment_factor = (alignments[i] * alignments[j]).sqrt();

                // Synergy amplified by alignment
                result[idx] = synergy * (0.5 + 0.5 * alignment_factor);
                idx += 1;
            }
        }

        result
    }

    /// Get high-synergy pairs (synergy >= threshold).
    pub fn get_high_synergy_pairs(&self, threshold: f32) -> Vec<(usize, usize, f32)> {
        let mut pairs = Vec::new();

        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                let synergy = self.matrix.get_synergy(i, j);
                if synergy >= threshold {
                    pairs.push((i, j, synergy));
                }
            }
        }

        // Sort by synergy descending
        pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        pairs
    }

    /// Get the managed synergy matrix.
    pub fn matrix(&self) -> &SynergyMatrix {
        &self.matrix
    }

    /// Get mutable reference to the matrix.
    pub fn matrix_mut(&mut self) -> &mut SynergyMatrix {
        &mut self.matrix
    }

    /// Get current configuration.
    pub fn config(&self) -> &SynergyConfig {
        &self.config
    }

    /// Get activation count for a pair.
    pub fn get_activation_count(&self, i: usize, j: usize) -> u64 {
        assert!(
            i < SYNERGY_DIM && j < SYNERGY_DIM,
            "FAIL FAST: activation count indices ({}, {}) out of bounds",
            i,
            j
        );
        self.activation_counts[i][j]
    }

    /// Get total sample count.
    pub fn total_samples(&self) -> u64 {
        self.matrix.sample_count
    }

    /// Get active profile ID.
    pub fn active_profile(&self) -> Option<&ProfileId> {
        self.active_profile.as_ref()
    }

    /// Validate matrix invariants.
    ///
    /// Returns `Ok(())` if matrix is valid, or detailed error describing what failed.
    pub fn validate(&self) -> crate::teleological::ComparisonValidationResult<()> {
        self.matrix.validate()
    }

    /// Check if service matrix is valid (returns bool for simple checks).
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.matrix.is_valid()
    }

    /// Assert service is valid, panicking with detailed error on failure.
    pub fn assert_valid(&self) {
        self.matrix.assert_valid();
    }
}

impl Default for SynergyService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synergy_service_new() {
        let service = SynergyService::new();

        // Should start with base synergies
        assert!(service.matrix().get_synergy(0, 4) > 0.8); // E1 + E5 = strong
        assert_eq!(service.total_samples(), 0);

        println!("[PASS] SynergyService::new creates service with base synergies");
    }

    #[test]
    fn test_compute_synergy() {
        let service = SynergyService::new();

        let result = service.compute_synergy(0, 4);

        assert!(result.synergy > 0.8);
        assert!(result.is_high_synergy);
        assert!((result.confidence - 0.0).abs() < 0.1); // No activations yet

        println!("[PASS] compute_synergy returns correct result");
    }

    #[test]
    fn test_compute_all_synergies() {
        let service = SynergyService::new();

        let synergies = service.compute_all_synergies();

        assert_eq!(synergies.len(), CROSS_CORRELATION_COUNT);
        assert!(synergies[0] > 0.0); // All base synergies are positive

        println!("[PASS] compute_all_synergies returns 78 values");
    }

    #[test]
    fn test_apply_feedback_positive() {
        let mut service = SynergyService::new();

        let original = service.compute_synergy(1, 3).synergy;

        let feedback = SynergyFeedback {
            pair: (1, 3),
            success: true,
            activation_strength: 0.8,
            consciousness_score: Some(0.9),
        };

        service.apply_feedback(&feedback);

        let updated = service.compute_synergy(1, 3).synergy;
        assert!(
            updated >= original,
            "Positive feedback should increase synergy"
        );
        assert_eq!(service.get_activation_count(1, 3), 1);

        println!("[PASS] apply_feedback increases synergy for positive feedback");
    }

    #[test]
    fn test_apply_feedback_negative() {
        let mut service = SynergyService::new();

        let original = service.compute_synergy(2, 5).synergy;

        let feedback = SynergyFeedback {
            pair: (2, 5),
            success: false,
            activation_strength: 0.3,
            consciousness_score: None,
        };

        service.apply_feedback(&feedback);

        let updated = service.compute_synergy(2, 5).synergy;
        assert!(
            updated <= original,
            "Negative feedback should decrease synergy"
        );

        println!("[PASS] apply_feedback decreases synergy for negative feedback");
    }

    #[test]
    fn test_apply_profile_modulation() {
        let mut service = SynergyService::new();

        let profile = TeleologicalProfile::code_implementation();
        service.apply_profile_modulation(&profile);

        assert!(service.active_profile().is_some());

        println!("[PASS] apply_profile_modulation sets active profile");
    }

    #[test]
    fn test_get_high_synergy_pairs() {
        let service = SynergyService::new();

        let pairs = service.get_high_synergy_pairs(0.9);

        // Should include (0, 4) = E1_Semantic + E5_Analogical
        assert!(pairs.iter().any(|p| p.0 == 0 && p.1 == 4));

        // Should be sorted by synergy descending
        for window in pairs.windows(2) {
            assert!(window[0].2 >= window[1].2);
        }

        println!("[PASS] get_high_synergy_pairs returns sorted high-synergy pairs");
    }

    #[test]
    fn test_get_aligned_synergies() {
        let service = SynergyService::new();

        // High alignment purpose vector
        let high_pv = PurposeVector::new([0.9; 13]);
        let aligned = service.get_aligned_synergies(&high_pv);

        // Low alignment purpose vector
        let low_pv = PurposeVector::new([0.1; 13]);
        let low_aligned = service.get_aligned_synergies(&low_pv);

        // High alignment should produce higher values
        let high_sum: f32 = aligned.iter().sum();
        let low_sum: f32 = low_aligned.iter().sum();
        assert!(high_sum > low_sum);

        println!("[PASS] get_aligned_synergies scales by alignment");
    }

    #[test]
    fn test_apply_decay() {
        let mut service = SynergyService::new();

        // Boost a synergy artificially
        service.matrix_mut().set_synergy(2, 8, 0.95);
        let base = SynergyMatrix::BASE_SYNERGIES[2][8];

        // Apply decay multiple times
        // With decay_factor=0.99: value = base + (initial - base) * 0.99^n
        // After 200 iterations: 0.6 + 0.35 * 0.99^200 ≈ 0.6 + 0.35 * 0.134 ≈ 0.647
        // This should be within 0.1 of base (0.6)
        for _ in 0..200 {
            service.apply_decay();
        }

        // Should decay towards base synergy
        let decayed = service.compute_synergy(2, 8).synergy;

        // With 200 iterations at decay_factor=0.99, value should be within 0.1 of base
        assert!(
            (decayed - base).abs() < 0.1,
            "Decayed {} should be within 0.1 of base {}. After 200 iterations with decay_factor=0.99, expected ~{}",
            decayed, base, base + 0.35 * 0.99_f32.powi(200)
        );

        println!("[PASS] apply_decay moves synergies towards base values");
        println!(
            "  - Decayed: {:.4}, Base: {:.4}, Diff: {:.4}",
            decayed,
            base,
            (decayed - base).abs()
        );
    }

    #[test]
    fn test_confidence_increases_with_activations() {
        let mut service = SynergyService::new();

        let initial_conf = service.compute_synergy(3, 7).confidence;

        // Apply multiple feedbacks
        for _ in 0..20 {
            let feedback = SynergyFeedback {
                pair: (3, 7),
                success: true,
                activation_strength: 0.5,
                consciousness_score: None,
            };
            service.apply_feedback(&feedback);
        }

        let final_conf = service.compute_synergy(3, 7).confidence;

        assert!(final_conf > initial_conf);

        println!("[PASS] Confidence increases with activation count");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_compute_synergy_out_of_bounds() {
        let service = SynergyService::new();
        let _ = service.compute_synergy(13, 0);
    }

    #[test]
    fn test_validate() {
        let service = SynergyService::new();
        assert!(service.validate().is_ok(), "New service should be valid");
        assert!(service.is_valid(), "is_valid() should return true");

        println!("[PASS] validate passes for valid service");
    }

    #[test]
    fn test_synergy_bounds_maintained() {
        let mut service = SynergyService::new();

        // Apply many positive feedbacks
        for _ in 0..100 {
            let feedback = SynergyFeedback {
                pair: (4, 9),
                success: true,
                activation_strength: 1.0,
                consciousness_score: Some(1.0),
            };
            service.apply_feedback(&feedback);
        }

        let synergy = service.compute_synergy(4, 9).synergy;
        assert!(synergy <= 1.0, "Synergy should not exceed 1.0");

        // Apply many negative feedbacks
        for _ in 0..200 {
            let feedback = SynergyFeedback {
                pair: (4, 9),
                success: false,
                activation_strength: 0.1,
                consciousness_score: None,
            };
            service.apply_feedback(&feedback);
        }

        let synergy = service.compute_synergy(4, 9).synergy;
        assert!(synergy >= 0.1, "Synergy should not go below min_synergy");

        println!("[PASS] Synergy values stay within configured bounds");
    }
}
