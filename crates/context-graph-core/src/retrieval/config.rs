//! Threshold and weight configurations for similarity and divergence detection.
//!
//! This module provides static configurations for the 13-embedding space retrieval system:
//! - `PerSpaceThresholds`: Per-embedder threshold values for high/low detection
//! - `SpaceWeights`: Category-based weights for weighted similarity calculation
//! - `SimilarityThresholds`: Container for high (relevance) and low (divergence) thresholds
//!
//! # Architecture Rules
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) weight = 0.0 (excluded from weighted calculations)
//! - AP-61: Topic threshold MUST be weighted_agreement >= 2.5
//! - AP-10: No NaN/Infinity in similarity scores (all values clamped)
//!
//! # Category Weights (from constitution)
//!
//! - Semantic (E1, E5, E6, E7, E10, E12, E13): weight 1.0
//! - Temporal (E2, E3, E4): weight 0.0 (excluded)
//! - Relational (E8, E11): weight 0.5
//! - Structural (E9): weight 0.5
//! - MAX_WEIGHTED_AGREEMENT = 7*1.0 + 2*0.5 + 1*0.5 = 8.5

use serde::{Deserialize, Serialize};

use crate::teleological::Embedder;

/// Lookback duration for divergence detection (2 hours in seconds).
///
/// Memories older than this are not considered for divergence detection.
pub const RECENT_LOOKBACK_SECS: u64 = 2 * 60 * 60;

/// Maximum number of recent memories to check for divergence.
///
/// Limits computation when many memories exist in the lookback window.
pub const MAX_RECENT_MEMORIES: usize = 50;

/// Category-based weights for topic similarity calculation.
///
/// Temporal spaces (E2-E4) are excluded with weight 0.0.
/// Order matches Embedder::index(): E1=0, E2=1, ..., E13=12.
///
/// From TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 548-562.
pub const SPACE_WEIGHTS: [f32; 13] = [
    1.0, // E1: Semantic - core meaning
    0.0, // E2: Temporal Recent (excluded)
    0.0, // E3: Temporal Periodic (excluded)
    0.0, // E4: Temporal Positional (excluded)
    1.0, // E5: Causal - cause/effect
    1.0, // E6: Sparse - sparse terms
    1.0, // E7: Code - code structure
    0.5, // E8: Graph (Relational)
    0.5, // E9: Hdc (Structural)
    1.0, // E10: Multimodal
    0.5, // E11: Entity (Relational)
    1.0, // E12: LateInteraction
    1.0, // E13: KeywordSplade
];

/// Total weight sum for normalization: 7*1.0 + 2*0.5 + 1*0.5 = 8.5
pub const TOTAL_WEIGHT: f32 = 8.5;

/// Per-space threshold values for all 13 embedding spaces.
///
/// Field names MUST match `PerSpaceScores` field names exactly.
/// All threshold values are in the range [0.0, 1.0].
///
/// # Usage
///
/// - High thresholds: Score above = highly relevant
/// - Low thresholds: Score below = divergent (topic drift)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerSpaceThresholds {
    /// E1: Semantic embedding threshold
    pub semantic: f32,
    /// E2: Temporal recent embedding threshold
    pub temporal_recent: f32,
    /// E3: Temporal periodic embedding threshold
    pub temporal_periodic: f32,
    /// E4: Temporal positional embedding threshold
    pub temporal_positional: f32,
    /// E5: Causal embedding threshold
    pub causal: f32,
    /// E6: Sparse lexical embedding threshold
    pub sparse: f32,
    /// E7: Code embedding threshold
    pub code: f32,
    /// E8: Graph/connectivity embedding threshold
    pub emotional: f32,
    /// E9: HDC embedding threshold
    pub hdc: f32,
    /// E10: Multimodal embedding threshold
    pub multimodal: f32,
    /// E11: Entity embedding threshold
    pub entity: f32,
    /// E12: Late interaction embedding threshold
    pub late_interaction: f32,
    /// E13: Keyword SPLADE embedding threshold
    pub keyword_splade: f32,
}

impl PerSpaceThresholds {
    /// Get threshold for a specific embedder.
    #[inline]
    pub fn get_threshold(&self, embedder: Embedder) -> f32 {
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

    /// Set threshold for a specific embedder.
    ///
    /// Threshold is clamped to [0.0, 1.0] range per AP-10.
    pub fn set_threshold(&mut self, embedder: Embedder, threshold: f32) {
        let threshold = threshold.clamp(0.0, 1.0);
        match embedder {
            Embedder::Semantic => self.semantic = threshold,
            Embedder::TemporalRecent => self.temporal_recent = threshold,
            Embedder::TemporalPeriodic => self.temporal_periodic = threshold,
            Embedder::TemporalPositional => self.temporal_positional = threshold,
            Embedder::Causal => self.causal = threshold,
            Embedder::Sparse => self.sparse = threshold,
            Embedder::Code => self.code = threshold,
            Embedder::Graph => self.emotional = threshold,
            Embedder::Hdc => self.hdc = threshold,
            Embedder::Multimodal => self.multimodal = threshold,
            Embedder::Entity => self.entity = threshold,
            Embedder::LateInteraction => self.late_interaction = threshold,
            Embedder::KeywordSplade => self.keyword_splade = threshold,
        }
    }

    /// Convert to array for compact operations.
    ///
    /// Order matches Embedder::index(): E1=0, E2=1, ..., E13=12.
    pub fn to_array(&self) -> [f32; 13] {
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
    ///
    /// Order must match Embedder::index(): E1=0, E2=1, ..., E13=12.
    pub fn from_array(arr: [f32; 13]) -> Self {
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

    /// Iterate over all thresholds with their embedder.
    pub fn iter(&self) -> impl Iterator<Item = (Embedder, f32)> + '_ {
        Embedder::all().map(move |e| (e, self.get_threshold(e)))
    }
}

impl From<[f32; 13]> for PerSpaceThresholds {
    fn from(arr: [f32; 13]) -> Self {
        Self::from_array(arr)
    }
}

/// Category-based weights for similarity calculation.
///
/// Weights determine how much each embedding space contributes to
/// weighted similarity calculations. Temporal spaces (E2-E4) have
/// weight 0.0 and are excluded from weighted calculations per AP-60.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceWeights {
    weights: [f32; 13],
}

impl SpaceWeights {
    /// Create new SpaceWeights from an array.
    ///
    /// Order must match Embedder::index(): E1=0, E2=1, ..., E13=12.
    pub fn new(weights: [f32; 13]) -> Self {
        Self { weights }
    }

    /// Get weight for a specific embedder.
    #[inline]
    pub fn get_weight(&self, embedder: Embedder) -> f32 {
        self.weights[embedder.index()]
    }

    /// Set weight for a specific embedder.
    ///
    /// Weight is clamped to >= 0.0.
    pub fn set_weight(&mut self, embedder: Embedder, weight: f32) {
        self.weights[embedder.index()] = weight.max(0.0);
    }

    /// Get the sum of all weights.
    pub fn sum(&self) -> f32 {
        self.weights.iter().sum()
    }

    /// Normalize weights so they sum to 13.0 (one per space).
    ///
    /// If sum is 0.0, weights remain unchanged to avoid division by zero.
    pub fn normalize(&mut self) {
        let sum = self.sum();
        if sum > 0.0 {
            let factor = 13.0 / sum;
            for w in &mut self.weights {
                *w *= factor;
            }
        }
    }

    /// Create a normalized copy of these weights.
    pub fn normalized(&self) -> Self {
        let mut result = self.clone();
        result.normalize();
        result
    }

    /// Get the weights as a slice.
    pub fn as_slice(&self) -> &[f32; 13] {
        &self.weights
    }

    /// Get the weights as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [f32; 13] {
        &mut self.weights
    }

    /// Iterate over all weights with their embedder.
    pub fn iter(&self) -> impl Iterator<Item = (Embedder, f32)> + '_ {
        Embedder::all().map(move |e| (e, self.get_weight(e)))
    }
}

impl Default for SpaceWeights {
    fn default() -> Self {
        default_weights()
    }
}

impl From<[f32; 13]> for SpaceWeights {
    fn from(weights: [f32; 13]) -> Self {
        Self::new(weights)
    }
}

/// Container for both high and low thresholds.
///
/// Combines high thresholds (for relevance detection) and low thresholds
/// (for divergence detection) into a single configuration object.
///
/// # Invariant
///
/// For every embedder, `high > low` must hold. This is verified in debug builds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimilarityThresholds {
    /// High thresholds for relevance detection.
    pub high: PerSpaceThresholds,
    /// Low thresholds for divergence detection.
    pub low: PerSpaceThresholds,
}

impl SimilarityThresholds {
    /// Create new SimilarityThresholds.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if any high threshold is not greater than
    /// the corresponding low threshold (invariant violation).
    pub fn new(high: PerSpaceThresholds, low: PerSpaceThresholds) -> Self {
        // Debug-only invariant check
        #[cfg(debug_assertions)]
        for embedder in Embedder::all() {
            let h = high.get_threshold(embedder);
            let l = low.get_threshold(embedder);
            debug_assert!(
                h > l,
                "Invariant violation: high {} must be > low {} for {:?}",
                h,
                l,
                embedder
            );
        }

        Self { high, low }
    }

    /// Check if a score exceeds the high threshold for an embedder.
    #[inline]
    pub fn is_high(&self, embedder: Embedder, score: f32) -> bool {
        score >= self.high.get_threshold(embedder)
    }

    /// Check if a score is below the low threshold for an embedder.
    #[inline]
    pub fn is_low(&self, embedder: Embedder, score: f32) -> bool {
        score < self.low.get_threshold(embedder)
    }

    /// Check if a score is in the middle range (between low and high).
    #[inline]
    pub fn is_middle(&self, embedder: Embedder, score: f32) -> bool {
        let low = self.low.get_threshold(embedder);
        let high = self.high.get_threshold(embedder);
        score >= low && score < high
    }
}

impl Default for SimilarityThresholds {
    fn default() -> Self {
        Self::new(high_thresholds(), low_thresholds())
    }
}

// =============================================================================
// Static Constructor Functions
// =============================================================================

/// High thresholds for relevance detection.
///
/// EXACT values from TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 489-502.
/// Scores at or above these thresholds indicate high relevance.
pub fn high_thresholds() -> PerSpaceThresholds {
    PerSpaceThresholds {
        semantic: 0.75,
        temporal_recent: 0.70,
        temporal_periodic: 0.70,
        temporal_positional: 0.70,
        causal: 0.70,
        sparse: 0.60,
        code: 0.80,
        emotional: 0.70,
        hdc: 0.70,
        multimodal: 0.70,
        entity: 0.70,
        late_interaction: 0.70,
        keyword_splade: 0.60,
    }
}

/// Low thresholds for divergence detection.
///
/// EXACT values from TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 505-518.
/// Scores below these thresholds indicate topic divergence.
pub fn low_thresholds() -> PerSpaceThresholds {
    PerSpaceThresholds {
        semantic: 0.30,
        temporal_recent: 0.30,
        temporal_periodic: 0.30,
        temporal_positional: 0.30,
        causal: 0.25,
        sparse: 0.20,
        code: 0.35,
        emotional: 0.30,
        hdc: 0.30,
        multimodal: 0.30,
        entity: 0.30,
        late_interaction: 0.30,
        keyword_splade: 0.20,
    }
}

/// Category-based weights from constitution.
///
/// EXACT values from TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 548-562.
/// Sum = 8.5 (before normalization).
///
/// - Semantic (E1, E5, E6, E7, E10, E12, E13): weight 1.0
/// - Temporal (E2, E3, E4): weight 0.0 (excluded per AP-60)
/// - Relational (E8, E11): weight 0.5
/// - Structural (E9): weight 0.5
pub fn default_weights() -> SpaceWeights {
    SpaceWeights::new(SPACE_WEIGHTS)
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // HIGH_THRESHOLDS Tests
    // =========================================================================

    #[test]
    fn test_high_thresholds_exact_values() {
        let h = high_thresholds();
        assert_eq!(h.semantic, 0.75);
        assert_eq!(h.temporal_recent, 0.70);
        assert_eq!(h.temporal_periodic, 0.70);
        assert_eq!(h.temporal_positional, 0.70);
        assert_eq!(h.causal, 0.70);
        assert_eq!(h.sparse, 0.60);
        assert_eq!(h.code, 0.80);
        assert_eq!(h.emotional, 0.70);
        assert_eq!(h.hdc, 0.70);
        assert_eq!(h.multimodal, 0.70);
        assert_eq!(h.entity, 0.70);
        assert_eq!(h.late_interaction, 0.70);
        assert_eq!(h.keyword_splade, 0.60);
        println!("[PASS] HIGH_THRESHOLDS match spec exactly (all 13 values verified)");
    }

    // =========================================================================
    // LOW_THRESHOLDS Tests
    // =========================================================================

    #[test]
    fn test_low_thresholds_exact_values() {
        let l = low_thresholds();
        assert_eq!(l.semantic, 0.30);
        assert_eq!(l.temporal_recent, 0.30);
        assert_eq!(l.temporal_periodic, 0.30);
        assert_eq!(l.temporal_positional, 0.30);
        assert_eq!(l.causal, 0.25);
        assert_eq!(l.sparse, 0.20);
        assert_eq!(l.code, 0.35);
        assert_eq!(l.emotional, 0.30);
        assert_eq!(l.hdc, 0.30);
        assert_eq!(l.multimodal, 0.30);
        assert_eq!(l.entity, 0.30);
        assert_eq!(l.late_interaction, 0.30);
        assert_eq!(l.keyword_splade, 0.20);
        println!("[PASS] LOW_THRESHOLDS match spec exactly (all 13 values verified)");
    }

    // =========================================================================
    // High vs Low Invariant Tests
    // =========================================================================

    #[test]
    fn test_all_high_greater_than_low() {
        let high = high_thresholds();
        let low = low_thresholds();

        for embedder in Embedder::all() {
            let h = high.get_threshold(embedder);
            let l = low.get_threshold(embedder);
            assert!(
                h > l,
                "{:?}: high {} must be > low {}",
                embedder,
                h,
                l
            );
            println!("[PASS] {:?}: high {:.2} > low {:.2}", embedder, h, l);
        }
    }

    // =========================================================================
    // SPACE_WEIGHTS Tests
    // =========================================================================

    #[test]
    fn test_space_weights_constant_values() {
        assert_eq!(SPACE_WEIGHTS[0], 1.0);  // E1 Semantic
        assert_eq!(SPACE_WEIGHTS[1], 0.0);  // E2 Temporal Recent (excluded)
        assert_eq!(SPACE_WEIGHTS[2], 0.0);  // E3 Temporal Periodic (excluded)
        assert_eq!(SPACE_WEIGHTS[3], 0.0);  // E4 Temporal Positional (excluded)
        assert_eq!(SPACE_WEIGHTS[4], 1.0);  // E5 Causal
        assert_eq!(SPACE_WEIGHTS[5], 1.0);  // E6 Sparse
        assert_eq!(SPACE_WEIGHTS[6], 1.0);  // E7 Code
        assert_eq!(SPACE_WEIGHTS[7], 0.5);  // E8 Graph (Relational)
        assert_eq!(SPACE_WEIGHTS[8], 0.5);  // E9 Hdc (Structural)
        assert_eq!(SPACE_WEIGHTS[9], 1.0);  // E10 Multimodal
        assert_eq!(SPACE_WEIGHTS[10], 0.5); // E11 Entity (Relational)
        assert_eq!(SPACE_WEIGHTS[11], 1.0); // E12 LateInteraction
        assert_eq!(SPACE_WEIGHTS[12], 1.0); // E13 KeywordSplade
        println!("[PASS] SPACE_WEIGHTS constant matches spec exactly");
    }

    #[test]
    fn test_weights_sum_before_normalization() {
        let weights = default_weights();
        let sum = weights.sum();
        assert!(
            (sum - 8.5).abs() < 0.001,
            "Expected 8.5, got {}",
            sum
        );
        println!("[PASS] default_weights().sum() = {} (expected 8.5)", sum);
    }

    #[test]
    fn test_total_weight_constant() {
        assert!((TOTAL_WEIGHT - 8.5).abs() < f32::EPSILON);
        println!("[PASS] TOTAL_WEIGHT constant = 8.5");
    }

    #[test]
    fn test_weights_normalization() {
        let mut weights = default_weights();
        weights.normalize();
        let sum = weights.sum();
        assert!(
            (sum - 13.0).abs() < 0.001,
            "Expected 13.0, got {}",
            sum
        );
        println!("[PASS] normalized weights sum = {} (expected 13.0)", sum);
    }

    #[test]
    fn test_weights_normalization_preserves_zeros() {
        let mut weights = default_weights();
        weights.normalize();

        // Temporal weights should remain 0.0 after normalization
        assert_eq!(weights.get_weight(Embedder::TemporalRecent), 0.0);
        assert_eq!(weights.get_weight(Embedder::TemporalPeriodic), 0.0);
        assert_eq!(weights.get_weight(Embedder::TemporalPositional), 0.0);
        println!("[PASS] Temporal weights remain 0.0 after normalization (AP-60)");
    }

    #[test]
    fn test_weights_normalized_method() {
        let weights = default_weights();
        let normalized = weights.normalized();

        // Original unchanged
        assert!((weights.sum() - 8.5).abs() < 0.001);
        // Normalized is 13.0
        assert!((normalized.sum() - 13.0).abs() < 0.001);
        println!("[PASS] normalized() creates normalized copy without modifying original");
    }

    // =========================================================================
    // PerSpaceThresholds get/set Tests
    // =========================================================================

    #[test]
    fn test_threshold_get_all_embedders() {
        let t = high_thresholds();
        for embedder in Embedder::all() {
            let value = t.get_threshold(embedder);
            assert!(value >= 0.0 && value <= 1.0, "{:?} threshold out of range", embedder);
        }
        println!("[PASS] get_threshold works for all 13 embedders");
    }

    #[test]
    fn test_threshold_set_clamping_high() {
        let mut t = high_thresholds();
        t.set_threshold(Embedder::Semantic, 1.5);
        assert_eq!(t.get_threshold(Embedder::Semantic), 1.0);
        println!("[PASS] set_threshold clamps 1.5 -> 1.0");
    }

    #[test]
    fn test_threshold_set_clamping_low() {
        let mut t = high_thresholds();
        t.set_threshold(Embedder::Code, -0.3);
        assert_eq!(t.get_threshold(Embedder::Code), 0.0);
        println!("[PASS] set_threshold clamps -0.3 -> 0.0");
    }

    #[test]
    fn test_threshold_set_valid_value() {
        let mut t = high_thresholds();
        t.set_threshold(Embedder::Causal, 0.65);
        assert_eq!(t.get_threshold(Embedder::Causal), 0.65);
        println!("[PASS] set_threshold accepts valid value 0.65");
    }

    // =========================================================================
    // SpaceWeights get/set Tests
    // =========================================================================

    #[test]
    fn test_weight_get_all_embedders() {
        let w = default_weights();
        for embedder in Embedder::all() {
            let value = w.get_weight(embedder);
            assert!(value >= 0.0, "{:?} weight negative", embedder);
        }
        println!("[PASS] get_weight works for all 13 embedders");
    }

    #[test]
    fn test_weight_set_clamping_negative() {
        let mut w = default_weights();
        w.set_weight(Embedder::Semantic, -0.5);
        assert_eq!(w.get_weight(Embedder::Semantic), 0.0);
        println!("[PASS] set_weight clamps -0.5 -> 0.0");
    }

    #[test]
    fn test_weight_set_valid_value() {
        let mut w = default_weights();
        w.set_weight(Embedder::Semantic, 2.0);
        assert_eq!(w.get_weight(Embedder::Semantic), 2.0);
        println!("[PASS] set_weight accepts valid positive value");
    }

    // =========================================================================
    // Array Conversion Tests
    // =========================================================================

    #[test]
    fn test_threshold_array_roundtrip() {
        let t = high_thresholds();
        let arr = t.to_array();
        let recovered = PerSpaceThresholds::from_array(arr);
        assert_eq!(t, recovered);
        println!("[PASS] PerSpaceThresholds array roundtrip");
    }

    #[test]
    fn test_threshold_array_order() {
        let t = high_thresholds();
        let arr = t.to_array();
        assert_eq!(arr[0], t.semantic);     // E1
        assert_eq!(arr[6], t.code);         // E7
        assert_eq!(arr[12], t.keyword_splade); // E13
        println!("[PASS] PerSpaceThresholds array order matches Embedder::index()");
    }

    #[test]
    fn test_threshold_from_array_trait() {
        let arr = [0.5; 13];
        let t: PerSpaceThresholds = arr.into();
        assert_eq!(t.semantic, 0.5);
        assert_eq!(t.code, 0.5);
        println!("[PASS] PerSpaceThresholds From<[f32; 13]> works");
    }

    #[test]
    fn test_weights_from_array_trait() {
        let arr = [1.0; 13];
        let w: SpaceWeights = arr.into();
        assert_eq!(w.get_weight(Embedder::Semantic), 1.0);
        assert_eq!(w.sum(), 13.0);
        println!("[PASS] SpaceWeights From<[f32; 13]> works");
    }

    // =========================================================================
    // SimilarityThresholds Tests
    // =========================================================================

    #[test]
    fn test_similarity_thresholds_default() {
        let st = SimilarityThresholds::default();
        assert_eq!(st.high, high_thresholds());
        assert_eq!(st.low, low_thresholds());
        println!("[PASS] SimilarityThresholds::default() uses spec values");
    }

    #[test]
    fn test_similarity_thresholds_is_high() {
        let st = SimilarityThresholds::default();
        assert!(st.is_high(Embedder::Semantic, 0.80));
        assert!(!st.is_high(Embedder::Semantic, 0.70));
        println!("[PASS] is_high() correctly identifies high scores");
    }

    #[test]
    fn test_similarity_thresholds_is_low() {
        let st = SimilarityThresholds::default();
        assert!(st.is_low(Embedder::Semantic, 0.25));
        assert!(!st.is_low(Embedder::Semantic, 0.35));
        println!("[PASS] is_low() correctly identifies low scores");
    }

    #[test]
    fn test_similarity_thresholds_is_middle() {
        let st = SimilarityThresholds::default();
        // E1: low=0.30, high=0.75
        assert!(st.is_middle(Embedder::Semantic, 0.50));
        assert!(!st.is_middle(Embedder::Semantic, 0.25)); // Below low
        assert!(!st.is_middle(Embedder::Semantic, 0.80)); // At/above high
        println!("[PASS] is_middle() correctly identifies middle scores");
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_per_space_thresholds_serialization() {
        let t = high_thresholds();
        let json = serde_json::to_string(&t).expect("serialize");
        let recovered: PerSpaceThresholds = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(t, recovered);
        println!("[PASS] PerSpaceThresholds JSON roundtrip");
    }

    #[test]
    fn test_space_weights_serialization() {
        let w = default_weights();
        let json = serde_json::to_string(&w).expect("serialize");
        let recovered: SpaceWeights = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(w, recovered);
        println!("[PASS] SpaceWeights JSON roundtrip");
    }

    #[test]
    fn test_similarity_thresholds_serialization() {
        let st = SimilarityThresholds::default();
        let json = serde_json::to_string(&st).expect("serialize");
        let recovered: SimilarityThresholds = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(st, recovered);
        println!("[PASS] SimilarityThresholds JSON roundtrip");
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_recent_lookback_secs() {
        assert_eq!(RECENT_LOOKBACK_SECS, 7200); // 2 hours in seconds
        println!("[PASS] RECENT_LOOKBACK_SECS = 7200 (2 hours)");
    }

    #[test]
    fn test_max_recent_memories() {
        assert_eq!(MAX_RECENT_MEMORIES, 50);
        println!("[PASS] MAX_RECENT_MEMORIES = 50");
    }

    // =========================================================================
    // Iterator Tests
    // =========================================================================

    #[test]
    fn test_threshold_iterator() {
        let t = high_thresholds();
        let count = t.iter().count();
        assert_eq!(count, 13);
        println!("[PASS] PerSpaceThresholds::iter() visits all 13 spaces");
    }

    #[test]
    fn test_weight_iterator() {
        let w = default_weights();
        let count = w.iter().count();
        assert_eq!(count, 13);
        println!("[PASS] SpaceWeights::iter() visits all 13 spaces");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_normalization_with_all_zeros() {
        let mut weights = SpaceWeights::new([0.0; 13]);
        weights.normalize(); // Should not panic (sum = 0 case)
        assert_eq!(weights.sum(), 0.0);
        println!("[PASS] normalize() handles all-zero weights safely");
    }

    #[test]
    fn test_as_slice() {
        let w = default_weights();
        let slice = w.as_slice();
        assert_eq!(slice.len(), 13);
        assert_eq!(slice[0], 1.0); // E1
        println!("[PASS] as_slice() returns correct slice");
    }

    // =========================================================================
    // Constitution Compliance Tests
    // =========================================================================

    #[test]
    fn test_ap60_temporal_excluded() {
        let w = default_weights();
        assert_eq!(w.get_weight(Embedder::TemporalRecent), 0.0);
        assert_eq!(w.get_weight(Embedder::TemporalPeriodic), 0.0);
        assert_eq!(w.get_weight(Embedder::TemporalPositional), 0.0);
        println!("[PASS] AP-60 verified: temporal embedders have weight 0.0");
    }

    #[test]
    fn test_category_weights_match_constitution() {
        use crate::embeddings::category::category_for;

        let w = default_weights();
        for embedder in Embedder::all() {
            let weight = w.get_weight(embedder);
            let expected = category_for(embedder).topic_weight();
            assert!(
                (weight - expected).abs() < f32::EPSILON,
                "{:?}: weight {} != category topic_weight {}",
                embedder,
                weight,
                expected
            );
        }
        println!("[PASS] All weights match EmbedderCategory::topic_weight()");
    }
}
