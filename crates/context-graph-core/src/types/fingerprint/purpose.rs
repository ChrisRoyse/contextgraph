//! Purpose Vector types for teleological alignment tracking.
//!
//! From constitution.yaml: Purpose Vector PV = [A(E1,V), ..., A(E13,V)]
//! where A(Ei, V) = cos(θ) between embedder i and North Star goal V.

use serde::{Deserialize, Serialize};

// Re-export NUM_EMBEDDERS from semantic.rs for backwards compatibility
pub use super::semantic::NUM_EMBEDDERS;

// Import alignment thresholds from centralized constants
use crate::config::constants::alignment;

/// Alignment threshold categories from Royse 2026 research.
///
/// From constitution.yaml:
/// - Optimal: θ ≥ 0.75
/// - Acceptable: θ ∈ [0.70, 0.75)
/// - Warning: θ ∈ [0.55, 0.70)
/// - Critical: θ < 0.55
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlignmentThreshold {
    /// θ ≥ 0.75 - Strong alignment with North Star goal
    Optimal,
    /// θ ∈ [0.70, 0.75) - Acceptable alignment, monitor for drift
    Acceptable,
    /// θ ∈ [0.55, 0.70) - Alignment degrading, intervention recommended
    Warning,
    /// θ < 0.55 - Critical misalignment, immediate action required
    Critical,
}

impl AlignmentThreshold {
    /// Classify an alignment value into a threshold category.
    ///
    /// # Arguments
    /// * `theta` - Alignment value (cosine similarity to North Star), expected range [-1.0, 1.0]
    ///
    /// # Returns
    /// The appropriate threshold category based on Royse 2026 thresholds.
    ///
    /// # Example
    /// ```
    /// use context_graph_core::types::fingerprint::AlignmentThreshold;
    ///
    /// assert_eq!(AlignmentThreshold::classify(0.80), AlignmentThreshold::Optimal);
    /// assert_eq!(AlignmentThreshold::classify(0.72), AlignmentThreshold::Acceptable);
    /// assert_eq!(AlignmentThreshold::classify(0.60), AlignmentThreshold::Warning);
    /// assert_eq!(AlignmentThreshold::classify(0.40), AlignmentThreshold::Critical);
    /// ```
    #[inline]
    pub fn classify(theta: f32) -> Self {
        // Use centralized constants from config::constants::alignment
        // per Constitution AP-003: "Magic numbers → define constants"
        if theta >= alignment::OPTIMAL {
            Self::Optimal
        } else if theta >= alignment::ACCEPTABLE {
            Self::Acceptable
        } else if theta >= alignment::WARNING {
            Self::Warning
        } else {
            Self::Critical
        }
    }

    /// Check if this threshold indicates misalignment requiring action.
    ///
    /// Warning and Critical thresholds are considered misaligned.
    #[inline]
    pub fn is_misaligned(&self) -> bool {
        matches!(self, Self::Warning | Self::Critical)
    }

    /// Check if this threshold is critical (requires immediate action).
    #[inline]
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical)
    }

    /// Get the minimum theta value for this threshold.
    ///
    /// Uses centralized constants from `config::constants::alignment`
    /// per Constitution AP-003: "Magic numbers → define constants"
    #[inline]
    pub fn min_theta(&self) -> f32 {
        match self {
            Self::Optimal => alignment::OPTIMAL,
            Self::Acceptable => alignment::ACCEPTABLE,
            Self::Warning => alignment::WARNING,
            Self::Critical => f32::NEG_INFINITY,
        }
    }
}

impl std::fmt::Display for AlignmentThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Optimal => write!(f, "Optimal (θ ≥ 0.75)"),
            Self::Acceptable => write!(f, "Acceptable (0.70 ≤ θ < 0.75)"),
            Self::Warning => write!(f, "Warning (0.55 ≤ θ < 0.70)"),
            Self::Critical => write!(f, "Critical (θ < 0.55)"),
        }
    }
}

/// Purpose Vector: 13D alignment signature to North Star goal.
///
/// From constitution.yaml: `PV = [A(E1,V), A(E2,V), ..., A(E13,V)]`
/// where `A(Ei, V) = cos(θ)` between embedder i and North Star goal V.
///
/// Each element is the cosine similarity between that embedder's representation
/// and the North Star goal vector, measuring how well that semantic dimension
/// aligns with the system's ultimate purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurposeVector {
    /// Alignment values for each of 13 embedders. Range: [-1.0, 1.0]
    /// Index mapping:
    /// - 0: E1 OpenAI text-embedding-3-large
    /// - 1: E2 Voyage-3-large
    /// - 2: E3 Cohere embed-v4
    /// - 3: E4 Gemini text-005
    /// - 4: E5 Jina embeddings v3
    /// - 5: E6 SPLADE v3
    /// - 6: E7 BGE-M3
    /// - 7: E8 GTE-Qwen2
    /// - 8: E9 Arctic-embed-L
    /// - 9: E10 Nomic-embed
    /// - 10: E11 mxbai-large
    /// - 11: E12 ModernBERT
    /// - 12: E13 SPLADE v3 (sparse NNZ count)
    pub alignments: [f32; NUM_EMBEDDERS],

    /// Index of the embedder with highest alignment (0-12).
    pub dominant_embedder: u8,

    /// Coherence score: standard deviation inverse of alignments.
    /// High coherence = all embedders agree on alignment direction.
    /// Range: [0.0, 1.0] where 1.0 = perfect agreement
    pub coherence: f32,

    /// Stability score: inverse of alignment variance over time.
    /// High stability = alignment doesn't fluctuate between accesses.
    /// Range: [0.0, 1.0] where 1.0 = perfectly stable
    pub stability: f32,
}

impl PurposeVector {
    /// Create a new PurposeVector from alignment values.
    ///
    /// Automatically computes dominant_embedder and coherence.
    /// Stability starts at 1.0 (no history to measure variance).
    ///
    /// # Arguments
    /// * `alignments` - Array of 13 alignment values (cosine similarities)
    ///
    /// # Panics
    /// Does not panic. Invalid alignment values are accepted but may produce
    /// unexpected results in downstream computations.
    pub fn new(alignments: [f32; NUM_EMBEDDERS]) -> Self {
        let dominant_embedder = Self::compute_dominant(&alignments);
        let coherence = Self::compute_coherence(&alignments);

        Self {
            alignments,
            dominant_embedder,
            coherence,
            stability: 1.0, // No history yet
        }
    }

    /// Compute the aggregate (mean) alignment across all embedders.
    ///
    /// This is the primary measure of overall goal alignment.
    #[inline]
    pub fn aggregate_alignment(&self) -> f32 {
        let sum: f32 = self.alignments.iter().sum();
        sum / NUM_EMBEDDERS as f32
    }

    /// Get the threshold status based on aggregate alignment.
    #[inline]
    pub fn threshold_status(&self) -> AlignmentThreshold {
        AlignmentThreshold::classify(self.aggregate_alignment())
    }

    /// Find the index of the dominant (highest alignment) embedder.
    #[inline]
    pub fn find_dominant(&self) -> u8 {
        self.dominant_embedder
    }

    /// Compute cosine similarity between two PurposeVectors.
    ///
    /// Measures how similar the alignment profiles are between two memories.
    /// Used for "find memories serving the same purpose" queries.
    ///
    /// # Returns
    /// Cosine similarity in range [-1.0, 1.0]
    pub fn similarity(&self, other: &Self) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..NUM_EMBEDDERS {
            dot += self.alignments[i] * other.alignments[i];
            norm_a += self.alignments[i] * self.alignments[i];
            norm_b += other.alignments[i] * other.alignments[i];
        }

        let denominator = (norm_a.sqrt()) * (norm_b.sqrt());
        if denominator < f32::EPSILON {
            0.0
        } else {
            dot / denominator
        }
    }

    /// Update stability based on comparison with previous state.
    ///
    /// # Arguments
    /// * `previous` - The previous PurposeVector to compare against
    /// * `decay` - Exponential decay factor for stability (default 0.9)
    pub fn update_stability(&mut self, previous: &Self, decay: f32) {
        let delta = self.similarity(previous);
        // High similarity = high stability, use exponential moving average
        self.stability = decay * self.stability + (1.0 - decay) * delta.abs();
    }

    /// Compute dominant embedder index from alignments.
    fn compute_dominant(alignments: &[f32; NUM_EMBEDDERS]) -> u8 {
        let mut max_idx = 0u8;
        let mut max_val = alignments[0];

        for (i, &val) in alignments.iter().enumerate().skip(1) {
            if val > max_val {
                max_val = val;
                max_idx = i as u8;
            }
        }
        max_idx
    }

    /// Compute coherence from alignments using inverse standard deviation.
    fn compute_coherence(alignments: &[f32; NUM_EMBEDDERS]) -> f32 {
        let mean: f32 = alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32;
        let variance: f32 =
            alignments.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / NUM_EMBEDDERS as f32;
        let std_dev = variance.sqrt();

        // Inverse stddev normalized to [0, 1]
        // When std_dev = 0, coherence = 1.0 (perfect agreement)
        // As std_dev increases, coherence decreases toward 0
        1.0 / (1.0 + std_dev)
    }
}

impl Default for PurposeVector {
    fn default() -> Self {
        Self::new([0.0; NUM_EMBEDDERS])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== AlignmentThreshold Tests =====

    #[test]
    fn test_alignment_threshold_classify_optimal() {
        // Boundary: exactly 0.75
        assert_eq!(
            AlignmentThreshold::classify(0.75),
            AlignmentThreshold::Optimal
        );
        // Above boundary
        assert_eq!(
            AlignmentThreshold::classify(0.80),
            AlignmentThreshold::Optimal
        );
        assert_eq!(
            AlignmentThreshold::classify(0.99),
            AlignmentThreshold::Optimal
        );
        assert_eq!(
            AlignmentThreshold::classify(1.0),
            AlignmentThreshold::Optimal
        );

        println!("[PASS] Optimal threshold: θ >= 0.75 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_classify_acceptable() {
        // Boundary: exactly 0.70
        assert_eq!(
            AlignmentThreshold::classify(0.70),
            AlignmentThreshold::Acceptable
        );
        // In range
        assert_eq!(
            AlignmentThreshold::classify(0.72),
            AlignmentThreshold::Acceptable
        );
        // Just below upper boundary
        assert_eq!(
            AlignmentThreshold::classify(0.749),
            AlignmentThreshold::Acceptable
        );

        println!("[PASS] Acceptable threshold: 0.70 <= θ < 0.75 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_classify_warning() {
        // Boundary: exactly 0.55
        assert_eq!(
            AlignmentThreshold::classify(0.55),
            AlignmentThreshold::Warning
        );
        // In range
        assert_eq!(
            AlignmentThreshold::classify(0.60),
            AlignmentThreshold::Warning
        );
        // Just below upper boundary
        assert_eq!(
            AlignmentThreshold::classify(0.699),
            AlignmentThreshold::Warning
        );

        println!("[PASS] Warning threshold: 0.55 <= θ < 0.70 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_classify_critical() {
        // Below 0.55
        assert_eq!(
            AlignmentThreshold::classify(0.54),
            AlignmentThreshold::Critical
        );
        assert_eq!(
            AlignmentThreshold::classify(0.40),
            AlignmentThreshold::Critical
        );
        assert_eq!(
            AlignmentThreshold::classify(0.0),
            AlignmentThreshold::Critical
        );
        // Negative values
        assert_eq!(
            AlignmentThreshold::classify(-0.5),
            AlignmentThreshold::Critical
        );

        println!("[PASS] Critical threshold: θ < 0.55 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_is_misaligned() {
        assert!(!AlignmentThreshold::Optimal.is_misaligned());
        assert!(!AlignmentThreshold::Acceptable.is_misaligned());
        assert!(AlignmentThreshold::Warning.is_misaligned());
        assert!(AlignmentThreshold::Critical.is_misaligned());

        println!("[PASS] is_misaligned returns true for Warning and Critical only");
    }

    #[test]
    fn test_alignment_threshold_is_critical() {
        assert!(!AlignmentThreshold::Optimal.is_critical());
        assert!(!AlignmentThreshold::Acceptable.is_critical());
        assert!(!AlignmentThreshold::Warning.is_critical());
        assert!(AlignmentThreshold::Critical.is_critical());

        println!("[PASS] is_critical returns true for Critical only");
    }

    // ===== PurposeVector Tests =====

    #[test]
    fn test_purpose_vector_new() {
        let alignments = [
            0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76,
        ];
        let pv = PurposeVector::new(alignments);

        assert_eq!(pv.alignments, alignments);
        assert_eq!(pv.dominant_embedder, 2); // Index of 0.9
        assert!(pv.coherence > 0.0 && pv.coherence <= 1.0);
        assert_eq!(pv.stability, 1.0); // Initial stability

        println!("[PASS] PurposeVector::new correctly initializes all fields");
        println!("  - alignments: {:?}", pv.alignments);
        println!(
            "  - dominant_embedder: {} (value: {})",
            pv.dominant_embedder, alignments[pv.dominant_embedder as usize]
        );
        println!("  - coherence: {:.4}", pv.coherence);
    }

    #[test]
    fn test_purpose_vector_aggregate_alignment() {
        // All same value = easy to verify mean
        let uniform = PurposeVector::new([0.75; NUM_EMBEDDERS]);
        assert!((uniform.aggregate_alignment() - 0.75).abs() < f32::EPSILON);

        // Known sum
        let alignments = [
            0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.5, 0.5, 0.55,
        ];
        let expected_mean = alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32;
        let pv = PurposeVector::new(alignments);
        assert!((pv.aggregate_alignment() - expected_mean).abs() < f32::EPSILON);

        println!("[PASS] aggregate_alignment returns correct mean");
        println!(
            "  - Uniform [0.75; 13] mean: {:.4}",
            uniform.aggregate_alignment()
        );
        println!(
            "  - Variable array mean: {:.4} (expected: {:.4})",
            pv.aggregate_alignment(),
            expected_mean
        );
    }

    #[test]
    fn test_purpose_vector_threshold_status() {
        // Optimal aggregate
        let optimal = PurposeVector::new([0.8; NUM_EMBEDDERS]);
        assert_eq!(optimal.threshold_status(), AlignmentThreshold::Optimal);

        // Critical aggregate
        let critical = PurposeVector::new([0.3; NUM_EMBEDDERS]);
        assert_eq!(critical.threshold_status(), AlignmentThreshold::Critical);

        println!("[PASS] threshold_status correctly classifies aggregate alignment");
    }

    #[test]
    fn test_purpose_vector_find_dominant() {
        // Clear dominant
        let mut alignments = [0.5; NUM_EMBEDDERS];
        alignments[7] = 0.95; // E8 is dominant
        let pv = PurposeVector::new(alignments);
        assert_eq!(pv.find_dominant(), 7);

        // First value is dominant (ties go to first)
        let tie = PurposeVector::new([0.9; NUM_EMBEDDERS]);
        assert_eq!(tie.find_dominant(), 0);

        println!("[PASS] find_dominant returns index of highest alignment");
    }

    #[test]
    fn test_purpose_vector_similarity_identical() {
        let pv = PurposeVector::new([
            0.7, 0.8, 0.6, 0.9, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76,
        ]);
        let similarity = pv.similarity(&pv);
        assert!((similarity - 1.0).abs() < 1e-6);

        println!("[PASS] Identical vectors have similarity 1.0");
    }

    #[test]
    fn test_purpose_vector_similarity_orthogonal() {
        // Opposing alignment patterns (13 elements with alternating pattern)
        let pv1 = PurposeVector::new([
            1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0,
        ]);
        let pv2 = PurposeVector::new([
            0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
        ]);
        let similarity = pv1.similarity(&pv2);
        assert!(similarity.abs() < 1e-6); // Orthogonal = 0

        println!("[PASS] Orthogonal vectors have similarity 0.0");
    }

    #[test]
    fn test_purpose_vector_similarity_opposite() {
        let pv1 = PurposeVector::new([0.5; NUM_EMBEDDERS]);
        let pv2 = PurposeVector::new([-0.5; NUM_EMBEDDERS]);
        let similarity = pv1.similarity(&pv2);
        assert!((similarity - (-1.0)).abs() < 1e-6);

        println!("[PASS] Opposite vectors have similarity -1.0");
    }

    #[test]
    fn test_purpose_vector_coherence_uniform() {
        // All same = perfect coherence
        let uniform = PurposeVector::new([0.8; NUM_EMBEDDERS]);
        assert!((uniform.coherence - 1.0).abs() < 1e-6);

        println!("[PASS] Uniform alignments have coherence 1.0 (no variance)");
    }

    #[test]
    fn test_purpose_vector_coherence_varied() {
        // High variance = lower coherence (13 elements with alternating pattern)
        let varied = PurposeVector::new([
            0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1,
        ]);
        assert!(varied.coherence < 1.0);
        assert!(varied.coherence > 0.0);

        println!("[PASS] Varied alignments have coherence < 1.0");
        println!("  - High variance coherence: {:.4}", varied.coherence);
    }

    #[test]
    fn test_purpose_vector_update_stability() {
        let mut pv1 = PurposeVector::new([0.8; NUM_EMBEDDERS]);
        let pv2 = PurposeVector::new([0.8; NUM_EMBEDDERS]); // Identical

        let initial_stability = pv1.stability;
        pv1.update_stability(&pv2, 0.9);

        // Identical vectors = high similarity = stability should remain high
        assert!(pv1.stability > 0.9);

        println!("[PASS] update_stability correctly updates based on similarity");
        println!(
            "  - Initial: {:.4}, After: {:.4}",
            initial_stability, pv1.stability
        );
    }

    #[test]
    fn test_purpose_vector_default() {
        let pv = PurposeVector::default();
        assert_eq!(pv.alignments, [0.0; NUM_EMBEDDERS]);
        assert_eq!(pv.dominant_embedder, 0);

        println!("[PASS] Default PurposeVector has zero alignments");
    }

    // ===== Edge Cases =====

    #[test]
    fn test_purpose_vector_zero_vector() {
        let zero = PurposeVector::new([0.0; NUM_EMBEDDERS]);
        assert_eq!(zero.aggregate_alignment(), 0.0);
        assert_eq!(zero.threshold_status(), AlignmentThreshold::Critical);

        // Similarity with zero vector
        let nonzero = PurposeVector::new([0.5; NUM_EMBEDDERS]);
        let similarity = zero.similarity(&nonzero);
        assert_eq!(similarity, 0.0); // Zero norm = 0 similarity

        println!("[PASS] Zero vector edge case handled correctly");
    }

    #[test]
    fn test_alignment_threshold_boundary_values() {
        // Test exact boundaries with epsilon tolerance
        assert_eq!(
            AlignmentThreshold::classify(0.75 - f32::EPSILON),
            AlignmentThreshold::Acceptable
        );
        assert_eq!(
            AlignmentThreshold::classify(0.70 - f32::EPSILON),
            AlignmentThreshold::Warning
        );
        assert_eq!(
            AlignmentThreshold::classify(0.55 - f32::EPSILON),
            AlignmentThreshold::Critical
        );

        println!("[PASS] Boundary values classify correctly with epsilon tolerance");
    }
}
