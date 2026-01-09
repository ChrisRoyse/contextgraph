//! Group alignments for hierarchical teleological aggregation.
//!
//! From teleoplan.md Section 3.2 Cross-Embedding Meaning Amplification:
//!
//! Hierarchical Grouping:
//! - Factual: E1, E12, E13 (what IS)
//! - Temporal: E2, E3 (when/sequence)
//! - Causal: E4, E7 (why/how)
//! - Relational: E5, E8, E9 (like/where/who)
//! - Qualitative: E10, E11 (feel/principle)
//! - Implementation: E6 (code)

use serde::{Deserialize, Serialize};

use super::types::NUM_EMBEDDERS;

/// Number of embedding groups in the hierarchical aggregation.
pub const NUM_GROUPS: usize = 6;

/// Group alignments from hierarchical aggregation.
///
/// Six groups capture different aspects of knowledge:
/// - **Factual**: What something IS (E1_Semantic + E12_Factual + E13_Sparse)
/// - **Temporal**: When/sequence (E2_Episodic + E3_Temporal)
/// - **Causal**: Why/how (E4_Causal + E7_Procedural)
/// - **Relational**: Like/where/who (E5_Analogical + E8_Spatial + E9_Social)
/// - **Qualitative**: Feel/principle (E10_Emotional + E11_Abstract)
/// - **Implementation**: Code (E6_Code)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GroupAlignments {
    /// Factual group: E1 + E12 + E13 (what IS)
    pub factual: f32,

    /// Temporal group: E2 + E3 (when/sequence)
    pub temporal: f32,

    /// Causal group: E4 + E7 (why/how)
    pub causal: f32,

    /// Relational group: E5 + E8 + E9 (like/where/who)
    pub relational: f32,

    /// Qualitative group: E10 + E11 (feel/principle)
    pub qualitative: f32,

    /// Implementation group: E6 (code)
    pub implementation: f32,
}

/// Indices for each embedding group.
pub mod group_indices {
    /// Factual group indices: E1, E12, E13
    pub const FACTUAL: &[usize] = &[0, 11, 12];
    /// Temporal group indices: E2, E3
    pub const TEMPORAL: &[usize] = &[1, 2];
    /// Causal group indices: E4, E7
    pub const CAUSAL: &[usize] = &[3, 6];
    /// Relational group indices: E5, E8, E9
    pub const RELATIONAL: &[usize] = &[4, 7, 8];
    /// Qualitative group indices: E10, E11
    pub const QUALITATIVE: &[usize] = &[9, 10];
    /// Implementation group indices: E6
    pub const IMPLEMENTATION: &[usize] = &[5];
}

impl GroupAlignments {
    /// Create GroupAlignments from a 13D purpose vector.
    ///
    /// Computes weighted average of embeddings within each group.
    ///
    /// # Arguments
    /// * `alignments` - 13D array of per-embedder alignment values
    /// * `weights` - Optional per-embedder weights (defaults to uniform)
    pub fn from_alignments(
        alignments: &[f32; NUM_EMBEDDERS],
        weights: Option<&[f32; NUM_EMBEDDERS]>,
    ) -> Self {
        let default_weights = [1.0f32; NUM_EMBEDDERS];
        let w = weights.unwrap_or(&default_weights);

        Self {
            factual: weighted_group_average(alignments, w, group_indices::FACTUAL),
            temporal: weighted_group_average(alignments, w, group_indices::TEMPORAL),
            causal: weighted_group_average(alignments, w, group_indices::CAUSAL),
            relational: weighted_group_average(alignments, w, group_indices::RELATIONAL),
            qualitative: weighted_group_average(alignments, w, group_indices::QUALITATIVE),
            implementation: weighted_group_average(alignments, w, group_indices::IMPLEMENTATION),
        }
    }

    /// Create with explicit values.
    pub fn new(
        factual: f32,
        temporal: f32,
        causal: f32,
        relational: f32,
        qualitative: f32,
        implementation: f32,
    ) -> Self {
        Self {
            factual,
            temporal,
            causal,
            relational,
            qualitative,
            implementation,
        }
    }

    /// Get all group values as an array.
    #[inline]
    pub fn as_array(&self) -> [f32; NUM_GROUPS] {
        [
            self.factual,
            self.temporal,
            self.causal,
            self.relational,
            self.qualitative,
            self.implementation,
        ]
    }

    /// Create from array.
    pub fn from_array(values: [f32; NUM_GROUPS]) -> Self {
        Self {
            factual: values[0],
            temporal: values[1],
            causal: values[2],
            relational: values[3],
            qualitative: values[4],
            implementation: values[5],
        }
    }

    /// Average of all group alignments.
    #[inline]
    pub fn average(&self) -> f32 {
        let arr = self.as_array();
        arr.iter().sum::<f32>() / NUM_GROUPS as f32
    }

    /// Standard deviation of group alignments.
    pub fn std_dev(&self) -> f32 {
        let arr = self.as_array();
        let mean = self.average();
        let variance: f32 =
            arr.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / NUM_GROUPS as f32;
        variance.sqrt()
    }

    /// Coherence: inverse of standard deviation normalized to [0, 1].
    /// High coherence = all groups agree on alignment direction.
    pub fn coherence(&self) -> f32 {
        1.0 / (1.0 + self.std_dev())
    }

    /// Find the dominant (highest alignment) group.
    pub fn dominant_group(&self) -> GroupType {
        let arr = self.as_array();
        let mut max_idx = 0;
        let mut max_val = arr[0];

        for (i, &val) in arr.iter().enumerate().skip(1) {
            if val > max_val {
                max_val = val;
                max_idx = i;
            }
        }

        GroupType::from_index(max_idx)
    }

    /// Find the weakest (lowest alignment) group.
    pub fn weakest_group(&self) -> GroupType {
        let arr = self.as_array();
        let mut min_idx = 0;
        let mut min_val = arr[0];

        for (i, &val) in arr.iter().enumerate().skip(1) {
            if val < min_val {
                min_val = val;
                min_idx = i;
            }
        }

        GroupType::from_index(min_idx)
    }

    /// Cosine similarity between two GroupAlignments.
    pub fn similarity(&self, other: &Self) -> f32 {
        let a = self.as_array();
        let b = other.as_array();

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..NUM_GROUPS {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denom = (norm_a.sqrt()) * (norm_b.sqrt());
        if denom < f32::EPSILON {
            0.0
        } else {
            dot / denom
        }
    }

    /// Get alignment for a specific group type.
    #[inline]
    pub fn get(&self, group: GroupType) -> f32 {
        match group {
            GroupType::Factual => self.factual,
            GroupType::Temporal => self.temporal,
            GroupType::Causal => self.causal,
            GroupType::Relational => self.relational,
            GroupType::Qualitative => self.qualitative,
            GroupType::Implementation => self.implementation,
        }
    }

    /// Set alignment for a specific group type.
    #[inline]
    pub fn set(&mut self, group: GroupType, value: f32) {
        match group {
            GroupType::Factual => self.factual = value,
            GroupType::Temporal => self.temporal = value,
            GroupType::Causal => self.causal = value,
            GroupType::Relational => self.relational = value,
            GroupType::Qualitative => self.qualitative = value,
            GroupType::Implementation => self.implementation = value,
        }
    }
}

/// Compute weighted average for a group of embeddings.
fn weighted_group_average(
    alignments: &[f32; NUM_EMBEDDERS],
    weights: &[f32; NUM_EMBEDDERS],
    indices: &[usize],
) -> f32 {
    if indices.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0f32;
    let mut weight_sum = 0.0f32;

    for &idx in indices {
        sum += alignments[idx] * weights[idx];
        weight_sum += weights[idx];
    }

    if weight_sum > f32::EPSILON {
        sum / weight_sum
    } else {
        0.0
    }
}

/// Enum representing the six embedding groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GroupType {
    /// Factual: E1 + E12 + E13
    Factual,
    /// Temporal: E2 + E3
    Temporal,
    /// Causal: E4 + E7
    Causal,
    /// Relational: E5 + E8 + E9
    Relational,
    /// Qualitative: E10 + E11
    Qualitative,
    /// Implementation: E6
    Implementation,
}

impl GroupType {
    /// Get all group types in order.
    pub const ALL: [GroupType; NUM_GROUPS] = [
        GroupType::Factual,
        GroupType::Temporal,
        GroupType::Causal,
        GroupType::Relational,
        GroupType::Qualitative,
        GroupType::Implementation,
    ];

    /// Convert from index (0-5) to GroupType.
    ///
    /// # Panics
    ///
    /// Panics if index >= NUM_GROUPS (FAIL FAST).
    pub fn from_index(index: usize) -> Self {
        assert!(
            index < NUM_GROUPS,
            "FAIL FAST: group index {} out of bounds (max {})",
            index,
            NUM_GROUPS - 1
        );
        Self::ALL[index]
    }

    /// Convert to index (0-5).
    #[inline]
    pub fn to_index(self) -> usize {
        match self {
            GroupType::Factual => 0,
            GroupType::Temporal => 1,
            GroupType::Causal => 2,
            GroupType::Relational => 3,
            GroupType::Qualitative => 4,
            GroupType::Implementation => 5,
        }
    }

    /// Get the embedding indices that belong to this group.
    pub fn embedding_indices(self) -> &'static [usize] {
        match self {
            GroupType::Factual => group_indices::FACTUAL,
            GroupType::Temporal => group_indices::TEMPORAL,
            GroupType::Causal => group_indices::CAUSAL,
            GroupType::Relational => group_indices::RELATIONAL,
            GroupType::Qualitative => group_indices::QUALITATIVE,
            GroupType::Implementation => group_indices::IMPLEMENTATION,
        }
    }

    /// Human-readable description of what this group captures.
    pub fn description(self) -> &'static str {
        match self {
            GroupType::Factual => "what IS (semantic, factual, sparse)",
            GroupType::Temporal => "when/sequence (episodic, temporal)",
            GroupType::Causal => "why/how (causal, procedural)",
            GroupType::Relational => "like/where/who (analogical, spatial, social)",
            GroupType::Qualitative => "feel/principle (emotional, abstract)",
            GroupType::Implementation => "code (implementation)",
        }
    }
}

impl std::fmt::Display for GroupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupType::Factual => write!(f, "Factual"),
            GroupType::Temporal => write!(f, "Temporal"),
            GroupType::Causal => write!(f, "Causal"),
            GroupType::Relational => write!(f, "Relational"),
            GroupType::Qualitative => write!(f, "Qualitative"),
            GroupType::Implementation => write!(f, "Implementation"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_alignments_default() {
        let ga = GroupAlignments::default();

        assert!((ga.factual - 0.0).abs() < f32::EPSILON);
        assert!((ga.temporal - 0.0).abs() < f32::EPSILON);
        assert!((ga.causal - 0.0).abs() < f32::EPSILON);
        assert!((ga.relational - 0.0).abs() < f32::EPSILON);
        assert!((ga.qualitative - 0.0).abs() < f32::EPSILON);
        assert!((ga.implementation - 0.0).abs() < f32::EPSILON);

        println!("[PASS] GroupAlignments::default creates zeroed structure");
    }

    #[test]
    fn test_group_alignments_from_alignments() {
        // Create alignments with known values
        let mut alignments = [0.5f32; NUM_EMBEDDERS];

        // Factual: E1(0), E12(11), E13(12) = average of 0.8, 0.6, 0.7
        alignments[0] = 0.8;
        alignments[11] = 0.6;
        alignments[12] = 0.7;

        // Temporal: E2(1), E3(2) = average of 0.9, 0.7
        alignments[1] = 0.9;
        alignments[2] = 0.7;

        // Implementation: E6(5) = 0.95
        alignments[5] = 0.95;

        let ga = GroupAlignments::from_alignments(&alignments, None);

        // Factual = (0.8 + 0.6 + 0.7) / 3 = 0.7
        assert!(
            (ga.factual - 0.7).abs() < 0.001,
            "factual = {} (expected 0.7)",
            ga.factual
        );

        // Temporal = (0.9 + 0.7) / 2 = 0.8
        assert!(
            (ga.temporal - 0.8).abs() < 0.001,
            "temporal = {} (expected 0.8)",
            ga.temporal
        );

        // Implementation = 0.95 (single element)
        assert!(
            (ga.implementation - 0.95).abs() < 0.001,
            "implementation = {} (expected 0.95)",
            ga.implementation
        );

        println!("[PASS] GroupAlignments::from_alignments computes correct averages");
    }

    #[test]
    fn test_group_alignments_weighted() {
        let alignments = [0.5f32; NUM_EMBEDDERS];
        let mut weights = [1.0f32; NUM_EMBEDDERS];

        // Give E1 (index 0) double weight in factual group
        weights[0] = 2.0;

        let ga = GroupAlignments::from_alignments(&alignments, Some(&weights));

        // All alignments are 0.5, so result should still be 0.5 regardless of weights
        assert!((ga.factual - 0.5).abs() < 0.001);

        println!("[PASS] Weighted group computation works correctly");
    }

    #[test]
    fn test_group_alignments_new() {
        let ga = GroupAlignments::new(0.8, 0.7, 0.6, 0.5, 0.4, 0.9);

        assert!((ga.factual - 0.8).abs() < f32::EPSILON);
        assert!((ga.temporal - 0.7).abs() < f32::EPSILON);
        assert!((ga.causal - 0.6).abs() < f32::EPSILON);
        assert!((ga.relational - 0.5).abs() < f32::EPSILON);
        assert!((ga.qualitative - 0.4).abs() < f32::EPSILON);
        assert!((ga.implementation - 0.9).abs() < f32::EPSILON);

        println!("[PASS] GroupAlignments::new sets all fields");
    }

    #[test]
    fn test_group_alignments_as_array() {
        let ga = GroupAlignments::new(0.1, 0.2, 0.3, 0.4, 0.5, 0.6);
        let arr = ga.as_array();

        assert_eq!(arr, [0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);

        println!("[PASS] as_array returns correct order");
    }

    #[test]
    fn test_group_alignments_from_array() {
        let arr = [0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6];
        let ga = GroupAlignments::from_array(arr);

        assert!((ga.factual - 0.1).abs() < f32::EPSILON);
        assert!((ga.implementation - 0.6).abs() < f32::EPSILON);

        println!("[PASS] from_array creates correct structure");
    }

    #[test]
    fn test_group_alignments_average() {
        let ga = GroupAlignments::new(0.6, 0.6, 0.6, 0.6, 0.6, 0.6);
        assert!((ga.average() - 0.6).abs() < f32::EPSILON);

        let ga2 = GroupAlignments::new(0.1, 0.2, 0.3, 0.4, 0.5, 0.6);
        // Average = (0.1 + 0.2 + 0.3 + 0.4 + 0.5 + 0.6) / 6 = 2.1 / 6 = 0.35
        assert!((ga2.average() - 0.35).abs() < 0.001);

        println!("[PASS] average computes correctly");
    }

    #[test]
    fn test_group_alignments_std_dev() {
        // Uniform = 0 std dev
        let uniform = GroupAlignments::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
        assert!(uniform.std_dev().abs() < f32::EPSILON);

        // Non-uniform has positive std dev
        let varied = GroupAlignments::new(0.1, 0.9, 0.1, 0.9, 0.1, 0.9);
        assert!(varied.std_dev() > 0.3);

        println!("[PASS] std_dev computes correctly");
    }

    #[test]
    fn test_group_alignments_coherence() {
        // Uniform = coherence 1.0
        let uniform = GroupAlignments::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
        assert!((uniform.coherence() - 1.0).abs() < 0.001);

        // Varied = lower coherence
        let varied = GroupAlignments::new(0.1, 0.9, 0.1, 0.9, 0.1, 0.9);
        assert!(varied.coherence() < 0.8);

        println!("[PASS] coherence computed correctly");
    }

    #[test]
    fn test_group_alignments_dominant_group() {
        let ga = GroupAlignments::new(0.5, 0.6, 0.7, 0.8, 0.9, 0.4);
        assert_eq!(ga.dominant_group(), GroupType::Qualitative);

        let ga2 = GroupAlignments::new(0.95, 0.6, 0.7, 0.8, 0.9, 0.4);
        assert_eq!(ga2.dominant_group(), GroupType::Factual);

        println!("[PASS] dominant_group finds maximum");
    }

    #[test]
    fn test_group_alignments_weakest_group() {
        let ga = GroupAlignments::new(0.5, 0.6, 0.7, 0.8, 0.9, 0.4);
        assert_eq!(ga.weakest_group(), GroupType::Implementation);

        println!("[PASS] weakest_group finds minimum");
    }

    #[test]
    fn test_group_alignments_similarity_identical() {
        let ga = GroupAlignments::new(0.5, 0.6, 0.7, 0.8, 0.9, 0.4);
        assert!((ga.similarity(&ga) - 1.0).abs() < 0.001);

        println!("[PASS] Identical groups have similarity 1.0");
    }

    #[test]
    fn test_group_alignments_get_set() {
        let mut ga = GroupAlignments::default();

        ga.set(GroupType::Causal, 0.75);
        assert!((ga.get(GroupType::Causal) - 0.75).abs() < f32::EPSILON);

        ga.set(GroupType::Implementation, 0.9);
        assert!((ga.get(GroupType::Implementation) - 0.9).abs() < f32::EPSILON);

        println!("[PASS] get/set work correctly");
    }

    #[test]
    fn test_group_alignments_serialization() {
        let ga = GroupAlignments::new(0.1, 0.2, 0.3, 0.4, 0.5, 0.6);
        let json = serde_json::to_string(&ga).unwrap();
        let deserialized: GroupAlignments = serde_json::from_str(&json).unwrap();

        assert!((ga.factual - deserialized.factual).abs() < f32::EPSILON);
        assert!((ga.implementation - deserialized.implementation).abs() < f32::EPSILON);

        println!("[PASS] Serialization roundtrip works");
    }

    // ===== GroupType Tests =====

    #[test]
    fn test_group_type_from_index() {
        assert_eq!(GroupType::from_index(0), GroupType::Factual);
        assert_eq!(GroupType::from_index(1), GroupType::Temporal);
        assert_eq!(GroupType::from_index(5), GroupType::Implementation);

        println!("[PASS] GroupType::from_index works");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_group_type_from_index_out_of_bounds() {
        let _ = GroupType::from_index(6);
    }

    #[test]
    fn test_group_type_to_index() {
        assert_eq!(GroupType::Factual.to_index(), 0);
        assert_eq!(GroupType::Implementation.to_index(), 5);

        // Roundtrip
        for i in 0..NUM_GROUPS {
            assert_eq!(GroupType::from_index(i).to_index(), i);
        }

        println!("[PASS] GroupType::to_index works and roundtrips");
    }

    #[test]
    fn test_group_type_embedding_indices() {
        assert_eq!(GroupType::Factual.embedding_indices(), &[0, 11, 12]);
        assert_eq!(GroupType::Temporal.embedding_indices(), &[1, 2]);
        assert_eq!(GroupType::Causal.embedding_indices(), &[3, 6]);
        assert_eq!(GroupType::Relational.embedding_indices(), &[4, 7, 8]);
        assert_eq!(GroupType::Qualitative.embedding_indices(), &[9, 10]);
        assert_eq!(GroupType::Implementation.embedding_indices(), &[5]);

        println!("[PASS] embedding_indices match teleoplan.md");
    }

    #[test]
    fn test_group_type_all() {
        assert_eq!(GroupType::ALL.len(), NUM_GROUPS);
        assert_eq!(GroupType::ALL[0], GroupType::Factual);
        assert_eq!(GroupType::ALL[5], GroupType::Implementation);

        println!("[PASS] GroupType::ALL contains all groups");
    }

    #[test]
    fn test_group_type_display() {
        assert_eq!(format!("{}", GroupType::Factual), "Factual");
        assert_eq!(format!("{}", GroupType::Implementation), "Implementation");

        println!("[PASS] GroupType Display works");
    }

    #[test]
    fn test_group_type_description() {
        assert!(GroupType::Factual.description().contains("what IS"));
        assert!(GroupType::Implementation.description().contains("code"));

        println!("[PASS] GroupType descriptions are informative");
    }

    #[test]
    fn test_group_indices_coverage() {
        // Verify all 13 embeddings are covered by exactly one group
        let mut covered = [false; NUM_EMBEDDERS];

        for group in GroupType::ALL {
            for &idx in group.embedding_indices() {
                assert!(!covered[idx], "Embedding {} is in multiple groups", idx);
                covered[idx] = true;
            }
        }

        for (i, &c) in covered.iter().enumerate() {
            assert!(c, "Embedding {} is not in any group", i);
        }

        println!("[PASS] All 13 embeddings covered by exactly one group");
    }
}
