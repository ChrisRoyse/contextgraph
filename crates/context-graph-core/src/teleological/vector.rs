//! TeleologicalVector: Fused multi-embedding representation.
//!
//! From teleoplan.md: The teleological vector is **not a concatenation** - it is
//! a **fusion** where each dimension gains meaning from its relationship to all
//! 12 other perspectives.

use serde::{Deserialize, Serialize};

use super::groups::GroupAlignments;
use super::synergy_matrix::CROSS_CORRELATION_COUNT;
use super::types::{ProfileId, TuckerCore, NUM_EMBEDDERS};
use crate::types::fingerprint::PurposeVector;

/// TeleologicalVector: Complete fused representation of a memory's teleological signature.
///
/// Combines:
/// - Purpose Vector (13D alignment to North Star)
/// - Cross-correlations (78 unique embedding pair interactions)
/// - Group alignments (6D hierarchical aggregation)
/// - Optional Tucker core for compressed tensor representation
///
/// From teleoplan.md: "TELEOLOGICAL VECTOR = 13 perspectives x 1024 dimensions"
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeleologicalVector {
    /// 13D purpose vector: alignment of each embedding to North Star
    pub purpose_vector: PurposeVector,

    /// 78 cross-correlations: synergy-weighted interactions between embedding pairs
    /// Order: (0,1), (0,2), ..., (0,12), (1,2), ..., (11,12)
    /// Uses Vec<f32> for serde compatibility (arrays > 32 elements not supported by default)
    pub cross_correlations: Vec<f32>,

    /// 6D group alignments from hierarchical aggregation
    pub group_alignments: GroupAlignments,

    /// Optional Tucker decomposition for compressed tensor representation
    /// Only populated when full tensor decomposition is computed
    pub tucker_core: Option<TuckerCore>,

    /// Profile ID that generated this vector (for task-specific weighting)
    pub profile_id: Option<ProfileId>,

    /// Confidence score for this teleological representation [0.0, 1.0]
    /// Higher = more reliable fusion, lower = sparse/conflicting embeddings
    pub confidence: f32,
}

impl TeleologicalVector {
    /// Create a new TeleologicalVector from a purpose vector.
    ///
    /// Cross-correlations and group alignments are initialized to zero.
    /// Use `with_correlations()` or fusion methods to populate.
    pub fn new(purpose_vector: PurposeVector) -> Self {
        Self {
            purpose_vector,
            cross_correlations: vec![0.0; CROSS_CORRELATION_COUNT],
            group_alignments: GroupAlignments::default(),
            tucker_core: None,
            profile_id: None,
            confidence: 1.0,
        }
    }

    /// Create with all components specified.
    ///
    /// # Panics
    ///
    /// Panics if cross_correlations length is not exactly CROSS_CORRELATION_COUNT (78).
    pub fn with_all(
        purpose_vector: PurposeVector,
        cross_correlations: Vec<f32>,
        group_alignments: GroupAlignments,
        confidence: f32,
    ) -> Self {
        assert!(
            cross_correlations.len() == CROSS_CORRELATION_COUNT,
            "FAIL FAST: cross_correlations must have exactly {} elements, got {}",
            CROSS_CORRELATION_COUNT,
            cross_correlations.len()
        );
        Self {
            purpose_vector,
            cross_correlations,
            group_alignments,
            tucker_core: None,
            profile_id: None,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Get cross-correlation value for embedding pair (i, j).
    ///
    /// # Arguments
    /// * `i` - First embedding index (0-12)
    /// * `j` - Second embedding index (0-12), must be different from i
    ///
    /// # Panics
    ///
    /// Panics if indices out of bounds or i == j (FAIL FAST).
    #[inline]
    pub fn get_correlation(&self, i: usize, j: usize) -> f32 {
        assert!(
            i < NUM_EMBEDDERS && j < NUM_EMBEDDERS,
            "FAIL FAST: correlation indices ({}, {}) out of bounds (max {})",
            i,
            j,
            NUM_EMBEDDERS - 1
        );
        assert!(
            i != j,
            "FAIL FAST: correlation indices must be different, got ({}, {})",
            i,
            j
        );

        // Ensure i < j for canonical ordering
        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let flat_idx = super::synergy_matrix::SynergyMatrix::indices_to_flat(lo, hi);
        self.cross_correlations[flat_idx]
    }

    /// Set cross-correlation value for embedding pair (i, j).
    ///
    /// Automatically handles ordering (stores same value for both (i,j) and (j,i)).
    ///
    /// # Panics
    ///
    /// Panics if indices out of bounds or i == j (FAIL FAST).
    #[inline]
    pub fn set_correlation(&mut self, i: usize, j: usize, value: f32) {
        assert!(
            i < NUM_EMBEDDERS && j < NUM_EMBEDDERS,
            "FAIL FAST: correlation indices ({}, {}) out of bounds (max {})",
            i,
            j,
            NUM_EMBEDDERS - 1
        );
        assert!(
            i != j,
            "FAIL FAST: correlation indices must be different, got ({}, {})",
            i,
            j
        );

        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let flat_idx = super::synergy_matrix::SynergyMatrix::indices_to_flat(lo, hi);
        self.cross_correlations[flat_idx] = value;
    }

    /// Compute overall alignment score.
    ///
    /// Weighted combination of:
    /// - Purpose vector aggregate (40%)
    /// - Group alignment average (30%)
    /// - Cross-correlation average (20%)
    /// - Confidence (10%)
    pub fn overall_alignment(&self) -> f32 {
        let pv_score = self.purpose_vector.aggregate_alignment();
        let group_score = self.group_alignments.average();
        let corr_score = self.average_correlation();

        0.4 * pv_score + 0.3 * group_score + 0.2 * corr_score + 0.1 * self.confidence
    }

    /// Average of all cross-correlations.
    pub fn average_correlation(&self) -> f32 {
        let sum: f32 = self.cross_correlations.iter().sum();
        sum / CROSS_CORRELATION_COUNT as f32
    }

    /// Compute cosine similarity between two TeleologicalVectors.
    ///
    /// Uses normalized combination of purpose vector and cross-correlations.
    pub fn similarity(&self, other: &Self) -> f32 {
        // Purpose vector similarity (weight: 0.6)
        let pv_sim = self.purpose_vector.similarity(&other.purpose_vector);

        // Cross-correlation similarity (weight: 0.3)
        let mut corr_dot = 0.0f32;
        let mut corr_norm_a = 0.0f32;
        let mut corr_norm_b = 0.0f32;

        for i in 0..CROSS_CORRELATION_COUNT {
            corr_dot += self.cross_correlations[i] * other.cross_correlations[i];
            corr_norm_a += self.cross_correlations[i] * self.cross_correlations[i];
            corr_norm_b += other.cross_correlations[i] * other.cross_correlations[i];
        }

        let corr_sim = if corr_norm_a > f32::EPSILON && corr_norm_b > f32::EPSILON {
            corr_dot / (corr_norm_a.sqrt() * corr_norm_b.sqrt())
        } else {
            0.0
        };

        // Group alignment similarity (weight: 0.1)
        let group_sim = self.group_alignments.similarity(&other.group_alignments);

        0.6 * pv_sim + 0.3 * corr_sim + 0.1 * group_sim
    }

    /// Set the profile ID.
    pub fn with_profile(mut self, profile_id: ProfileId) -> Self {
        self.profile_id = Some(profile_id);
        self
    }

    /// Set the Tucker core.
    pub fn with_tucker_core(mut self, tucker_core: TuckerCore) -> Self {
        self.tucker_core = Some(tucker_core);
        self
    }

    /// Check if this vector has Tucker decomposition available.
    pub fn has_tucker_core(&self) -> bool {
        self.tucker_core.is_some()
    }

    /// Get number of non-zero cross-correlations (sparsity indicator).
    pub fn nonzero_correlations(&self) -> usize {
        self.cross_correlations
            .iter()
            .filter(|&&v| v.abs() > f32::EPSILON)
            .count()
    }

    /// Correlation density (proportion of non-zero correlations).
    pub fn correlation_density(&self) -> f32 {
        self.nonzero_correlations() as f32 / CROSS_CORRELATION_COUNT as f32
    }
}

impl Default for TeleologicalVector {
    fn default() -> Self {
        Self::new(PurposeVector::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_purpose_vector(values: [f32; NUM_EMBEDDERS]) -> PurposeVector {
        PurposeVector::new(values)
    }

    #[test]
    fn test_teleological_vector_new() {
        let pv = make_purpose_vector([0.8; NUM_EMBEDDERS]);
        let tv = TeleologicalVector::new(pv.clone());

        assert_eq!(tv.purpose_vector.alignments, pv.alignments);
        assert_eq!(tv.cross_correlations.len(), CROSS_CORRELATION_COUNT);
        assert!(tv.tucker_core.is_none());
        assert!(tv.profile_id.is_none());
        assert!((tv.confidence - 1.0).abs() < f32::EPSILON);

        println!("[PASS] TeleologicalVector::new creates valid structure");
    }

    #[test]
    fn test_teleological_vector_with_all() {
        let pv = make_purpose_vector([0.7; NUM_EMBEDDERS]);
        let cross = vec![0.5; CROSS_CORRELATION_COUNT];
        let groups = GroupAlignments {
            factual: 0.8,
            temporal: 0.7,
            causal: 0.6,
            relational: 0.5,
            qualitative: 0.4,
            implementation: 0.9,
        };

        let tv = TeleologicalVector::with_all(pv, cross.clone(), groups.clone(), 0.85);

        assert_eq!(tv.cross_correlations, cross);
        assert!((tv.group_alignments.factual - 0.8).abs() < f32::EPSILON);
        assert!((tv.confidence - 0.85).abs() < f32::EPSILON);

        println!("[PASS] TeleologicalVector::with_all populates all fields");
    }

    #[test]
    fn test_teleological_vector_confidence_clamping() {
        let pv = make_purpose_vector([0.5; NUM_EMBEDDERS]);
        let cross = vec![0.0; CROSS_CORRELATION_COUNT];
        let groups = GroupAlignments::default();

        let tv_high = TeleologicalVector::with_all(pv.clone(), cross.clone(), groups.clone(), 1.5);
        assert!((tv_high.confidence - 1.0).abs() < f32::EPSILON);

        let tv_low = TeleologicalVector::with_all(pv, cross, groups, -0.5);
        assert!((tv_low.confidence - 0.0).abs() < f32::EPSILON);

        println!("[PASS] Confidence clamped to [0.0, 1.0]");
    }

    #[test]
    fn test_teleological_vector_get_correlation() {
        let mut tv = TeleologicalVector::default();

        // Set via flat index
        let flat_idx = super::super::synergy_matrix::SynergyMatrix::indices_to_flat(2, 5);
        tv.cross_correlations[flat_idx] = 0.75;

        // Get via (i, j)
        assert!((tv.get_correlation(2, 5) - 0.75).abs() < f32::EPSILON);
        // Also works with reversed order
        assert!((tv.get_correlation(5, 2) - 0.75).abs() < f32::EPSILON);

        println!("[PASS] get_correlation works bidirectionally");
    }

    #[test]
    fn test_teleological_vector_set_correlation() {
        let mut tv = TeleologicalVector::default();

        tv.set_correlation(3, 8, 0.65);

        assert!((tv.get_correlation(3, 8) - 0.65).abs() < f32::EPSILON);
        assert!((tv.get_correlation(8, 3) - 0.65).abs() < f32::EPSILON);

        println!("[PASS] set_correlation maintains symmetry");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_teleological_vector_get_correlation_same_index() {
        let tv = TeleologicalVector::default();
        let _ = tv.get_correlation(5, 5);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_teleological_vector_get_correlation_out_of_bounds() {
        let tv = TeleologicalVector::default();
        let _ = tv.get_correlation(13, 0);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_teleological_vector_set_correlation_same_index() {
        let mut tv = TeleologicalVector::default();
        tv.set_correlation(7, 7, 0.5);
    }

    #[test]
    fn test_teleological_vector_overall_alignment() {
        let pv = make_purpose_vector([0.8; NUM_EMBEDDERS]);
        let mut tv = TeleologicalVector::new(pv);

        // Set cross-correlations to 0.6
        for i in 0..CROSS_CORRELATION_COUNT {
            tv.cross_correlations[i] = 0.6;
        }

        // Set group alignments
        tv.group_alignments = GroupAlignments {
            factual: 0.7,
            temporal: 0.7,
            causal: 0.7,
            relational: 0.7,
            qualitative: 0.7,
            implementation: 0.7,
        };

        tv.confidence = 0.9;

        let overall = tv.overall_alignment();

        // Expected: 0.4 * 0.8 + 0.3 * 0.7 + 0.2 * 0.6 + 0.1 * 0.9 = 0.32 + 0.21 + 0.12 + 0.09 = 0.74
        assert!((overall - 0.74).abs() < 0.01);

        println!("[PASS] overall_alignment = {overall:.4}");
    }

    #[test]
    fn test_teleological_vector_average_correlation() {
        let mut tv = TeleologicalVector::default();

        // Set all correlations to 0.5
        for i in 0..CROSS_CORRELATION_COUNT {
            tv.cross_correlations[i] = 0.5;
        }

        assert!((tv.average_correlation() - 0.5).abs() < f32::EPSILON);

        println!("[PASS] average_correlation works correctly");
    }

    #[test]
    fn test_teleological_vector_similarity_identical() {
        let pv = make_purpose_vector([0.75; NUM_EMBEDDERS]);
        let mut tv = TeleologicalVector::new(pv);

        // Set some non-zero cross-correlations to avoid zero-norm edge case
        for i in 0..CROSS_CORRELATION_COUNT {
            tv.cross_correlations[i] = 0.5;
        }

        tv.group_alignments = GroupAlignments {
            factual: 0.8,
            temporal: 0.7,
            causal: 0.6,
            relational: 0.5,
            qualitative: 0.4,
            implementation: 0.9,
        };

        let sim = tv.similarity(&tv);
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Self-similarity should be ~1.0, got {}",
            sim
        );

        println!("[PASS] Identical vectors have similarity ~1.0");
    }

    #[test]
    fn test_teleological_vector_similarity_different() {
        let tv1 = TeleologicalVector::new(make_purpose_vector([0.9; NUM_EMBEDDERS]));
        let tv2 = TeleologicalVector::new(make_purpose_vector([0.1; NUM_EMBEDDERS]));

        let sim = tv1.similarity(&tv2);
        assert!(
            sim < 0.9,
            "Different vectors should have lower similarity, got {}",
            sim
        );

        println!("[PASS] Different vectors have lower similarity: {sim:.4}");
    }

    #[test]
    fn test_teleological_vector_with_profile() {
        let tv = TeleologicalVector::default().with_profile(ProfileId::new("test_profile"));

        assert!(tv.profile_id.is_some());
        assert_eq!(tv.profile_id.as_ref().unwrap().as_str(), "test_profile");

        println!("[PASS] with_profile sets profile ID");
    }

    #[test]
    fn test_teleological_vector_with_tucker_core() {
        let tucker = TuckerCore::new((2, 2, 4));
        let tv = TeleologicalVector::default().with_tucker_core(tucker);

        assert!(tv.has_tucker_core());

        println!("[PASS] with_tucker_core sets Tucker decomposition");
    }

    #[test]
    fn test_teleological_vector_nonzero_correlations() {
        let mut tv = TeleologicalVector::default();

        assert_eq!(tv.nonzero_correlations(), 0);

        tv.set_correlation(0, 1, 0.5);
        tv.set_correlation(2, 3, 0.3);
        tv.set_correlation(5, 10, 0.7);

        assert_eq!(tv.nonzero_correlations(), 3);

        println!("[PASS] nonzero_correlations counts correctly");
    }

    #[test]
    fn test_teleological_vector_correlation_density() {
        let mut tv = TeleologicalVector::default();

        assert!((tv.correlation_density() - 0.0).abs() < f32::EPSILON);

        // Set all correlations
        for i in 0..CROSS_CORRELATION_COUNT {
            tv.cross_correlations[i] = 0.5;
        }

        assert!((tv.correlation_density() - 1.0).abs() < f32::EPSILON);

        println!("[PASS] correlation_density works correctly");
    }

    #[test]
    fn test_teleological_vector_default() {
        let tv = TeleologicalVector::default();

        assert_eq!(tv.purpose_vector.alignments, [0.0; NUM_EMBEDDERS]);
        assert_eq!(tv.cross_correlations, vec![0.0; CROSS_CORRELATION_COUNT]);
        assert!(tv.tucker_core.is_none());

        println!("[PASS] TeleologicalVector::default creates zeroed structure");
    }

    #[test]
    fn test_teleological_vector_serialization() {
        let mut tv = TeleologicalVector::new(make_purpose_vector([0.7; NUM_EMBEDDERS]));
        tv.set_correlation(1, 4, 0.8);
        tv.confidence = 0.95;

        let json = serde_json::to_string(&tv).unwrap();
        let deserialized: TeleologicalVector = serde_json::from_str(&json).unwrap();

        assert!((tv.confidence - deserialized.confidence).abs() < f32::EPSILON);
        assert!(
            (tv.get_correlation(1, 4) - deserialized.get_correlation(1, 4)).abs() < f32::EPSILON
        );

        println!("[PASS] Serialization roundtrip works");
    }
}
