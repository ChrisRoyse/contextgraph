//! Embedder category classification for topic weight calculation.
//!
//! This module provides the EmbedderCategory enum which classifies each of the
//! 13 embedders into semantic roles that determine their contribution to
//! topic detection via weighted agreement.
//!
//! # Constitution Reference
//!
//! From CLAUDE.md embedder_categories section:
//! - Semantic: weight 1.0, primary topic triggers (E1, E5, E6, E7, E10, E12, E13)
//! - Temporal: weight 0.0, metadata only (E2, E3, E4)
//! - Relational: weight 0.5, supporting role (E8, E11)
//! - Structural: weight 0.5, supporting role (E9)
//!
//! # Architecture Rules
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-60: Temporal embedders MUST NOT count toward topic detection
//! - AP-61: Topic threshold MUST be weighted_agreement >= 2.5

use serde::{Deserialize, Serialize};

use crate::teleological::Embedder;

/// Category classification for embedders in topic detection.
///
/// Each category has an associated topic_weight that determines how much
/// the embedder contributes to weighted agreement calculations.
///
/// # Weighted Agreement Formula
///
/// ```text
/// weighted_agreement = Sum(topic_weight_i * is_clustered_i)
/// max_weighted_agreement = 7*1.0 + 2*0.5 + 1*0.5 = 8.5
/// topic_confidence = weighted_agreement / 8.5
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EmbedderCategory {
    /// Semantic embedders capture meaning, concepts, intent, and code.
    /// E1 (Semantic), E5 (Causal), E6 (Sparse), E7 (Code),
    /// E10 (Multimodal), E12 (LateInteraction), E13 (KeywordSplade)
    ///
    /// - topic_weight: 1.0
    /// - divergence_detection: true
    /// - count: 7 embedders
    #[default]
    Semantic,

    /// Temporal embedders capture time-based features.
    /// E2 (TemporalRecent), E3 (TemporalPeriodic), E4 (TemporalPositional)
    ///
    /// - topic_weight: 0.0 (NEVER counts toward topic detection)
    /// - divergence_detection: false
    /// - count: 3 embedders
    /// - rationale: Temporal proximity != semantic relationship
    Temporal,

    /// Relational embedders capture relationships and entity connections.
    /// E8 (Emotional), E11 (Entity)
    ///
    /// - topic_weight: 0.5
    /// - divergence_detection: false
    /// - count: 2 embedders
    Relational,

    /// Structural embedders capture form and patterns.
    /// E9 (Hdc)
    ///
    /// - topic_weight: 0.5
    /// - divergence_detection: false
    /// - count: 1 embedder
    Structural,
}

impl EmbedderCategory {
    /// Returns the topic weight for this category.
    ///
    /// Used in weighted agreement calculations for topic detection.
    /// Values are from constitution.yaml embedder_categories section.
    ///
    /// # Returns
    ///
    /// - Semantic: 1.0 (full contribution)
    /// - Temporal: 0.0 (excluded - AP-60)
    /// - Relational: 0.5 (partial contribution)
    /// - Structural: 0.5 (partial contribution)
    #[inline]
    pub const fn topic_weight(&self) -> f32 {
        match self {
            EmbedderCategory::Semantic => 1.0,
            EmbedderCategory::Temporal => 0.0,
            EmbedderCategory::Relational => 0.5,
            EmbedderCategory::Structural => 0.5,
        }
    }

    /// Returns true if this is a Semantic category embedder.
    #[inline]
    pub const fn is_semantic(&self) -> bool {
        matches!(self, EmbedderCategory::Semantic)
    }

    /// Returns true if this is a Temporal category embedder.
    #[inline]
    pub const fn is_temporal(&self) -> bool {
        matches!(self, EmbedderCategory::Temporal)
    }

    /// Returns true if this is a Relational category embedder.
    #[inline]
    pub const fn is_relational(&self) -> bool {
        matches!(self, EmbedderCategory::Relational)
    }

    /// Returns true if this is a Structural category embedder.
    #[inline]
    pub const fn is_structural(&self) -> bool {
        matches!(self, EmbedderCategory::Structural)
    }

    /// Returns all category variants.
    #[inline]
    pub const fn all() -> [EmbedderCategory; 4] {
        [
            EmbedderCategory::Semantic,
            EmbedderCategory::Temporal,
            EmbedderCategory::Relational,
            EmbedderCategory::Structural,
        ]
    }

    /// Returns count of embedders in each category: (semantic, temporal, relational, structural).
    #[inline]
    pub const fn count_by_category() -> (usize, usize, usize, usize) {
        (7, 3, 2, 1) // Total = 13
    }

    /// Returns whether embedders in this category are eligible for divergence detection.
    ///
    /// Per ARCH-10, only SEMANTIC embedders are used for divergence detection.
    /// Note: E5 (Causal) is Semantic but excluded from DIVERGENCE_SPACES per
    /// AP-77 (requires CausalDirection for meaningful scores). Use
    /// `used_for_divergence` on `Embedder` for the precise check.
    #[inline]
    pub const fn used_for_divergence_detection(&self) -> bool {
        matches!(self, EmbedderCategory::Semantic)
    }
}

impl std::fmt::Display for EmbedderCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedderCategory::Semantic => write!(f, "Semantic"),
            EmbedderCategory::Temporal => write!(f, "Temporal"),
            EmbedderCategory::Relational => write!(f, "Relational"),
            EmbedderCategory::Structural => write!(f, "Structural"),
        }
    }
}

// =============================================================================
// Category assignment functions
// =============================================================================

/// Get the category for a specific embedder.
///
/// This function maps each of the 13 embedders to their category
/// per constitution.yaml embedder_categories specification.
///
/// # Example
///
/// ```
/// use context_graph_core::embeddings::category::category_for;
/// use context_graph_core::teleological::Embedder;
///
/// assert_eq!(category_for(Embedder::Semantic).topic_weight(), 1.0);
/// assert_eq!(category_for(Embedder::TemporalRecent).topic_weight(), 0.0);
/// ```
pub fn category_for(embedder: Embedder) -> EmbedderCategory {
    match embedder {
        // Semantic category (7 embedders, weight 1.0)
        Embedder::Semantic => EmbedderCategory::Semantic,
        Embedder::Causal => EmbedderCategory::Semantic,
        Embedder::Sparse => EmbedderCategory::Semantic,
        Embedder::Code => EmbedderCategory::Semantic,
        Embedder::Multimodal => EmbedderCategory::Semantic,
        Embedder::LateInteraction => EmbedderCategory::Semantic,
        Embedder::KeywordSplade => EmbedderCategory::Semantic,

        // Temporal category (3 embedders, weight 0.0)
        Embedder::TemporalRecent => EmbedderCategory::Temporal,
        Embedder::TemporalPeriodic => EmbedderCategory::Temporal,
        Embedder::TemporalPositional => EmbedderCategory::Temporal,

        // Relational category (2 embedders, weight 0.5)
        Embedder::Emotional => EmbedderCategory::Relational,
        Embedder::Entity => EmbedderCategory::Relational,

        // Structural category (1 embedder, weight 0.5)
        Embedder::Hdc => EmbedderCategory::Structural,
    }
}

/// Calculate the maximum possible weighted agreement.
///
/// This is the sum of all topic weights across all 13 embedders:
/// - 7 semantic * 1.0 = 7.0
/// - 3 temporal * 0.0 = 0.0
/// - 2 relational * 0.5 = 1.0
/// - 1 structural * 0.5 = 0.5
/// - Total = 8.5
///
/// Used for normalizing topic confidence.
#[inline]
pub const fn max_weighted_agreement() -> f32 {
    // 7 * 1.0 + 3 * 0.0 + 2 * 0.5 + 1 * 0.5 = 7.0 + 0.0 + 1.0 + 0.5 = 8.5
    8.5
}

/// The topic detection threshold per ARCH-09.
///
/// A cluster must have weighted_agreement >= 2.5 to be considered a topic.
#[inline]
pub const fn topic_threshold() -> f32 {
    2.5
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_weights() {
        assert_eq!(EmbedderCategory::Semantic.topic_weight(), 1.0);
        assert_eq!(EmbedderCategory::Temporal.topic_weight(), 0.0);
        assert_eq!(EmbedderCategory::Relational.topic_weight(), 0.5);
        assert_eq!(EmbedderCategory::Structural.topic_weight(), 0.5);
        println!("[PASS] All topic_weight values match constitution");
    }

    #[test]
    fn test_is_semantic() {
        assert!(EmbedderCategory::Semantic.is_semantic());
        assert!(!EmbedderCategory::Temporal.is_semantic());
        assert!(!EmbedderCategory::Relational.is_semantic());
        assert!(!EmbedderCategory::Structural.is_semantic());
        println!("[PASS] is_semantic() returns true only for Semantic");
    }

    #[test]
    fn test_is_temporal() {
        assert!(!EmbedderCategory::Semantic.is_temporal());
        assert!(EmbedderCategory::Temporal.is_temporal());
        assert!(!EmbedderCategory::Relational.is_temporal());
        assert!(!EmbedderCategory::Structural.is_temporal());
        println!("[PASS] is_temporal() returns true only for Temporal");
    }

    #[test]
    fn test_is_relational() {
        assert!(!EmbedderCategory::Semantic.is_relational());
        assert!(!EmbedderCategory::Temporal.is_relational());
        assert!(EmbedderCategory::Relational.is_relational());
        assert!(!EmbedderCategory::Structural.is_relational());
        println!("[PASS] is_relational() returns true only for Relational");
    }

    #[test]
    fn test_is_structural() {
        assert!(!EmbedderCategory::Semantic.is_structural());
        assert!(!EmbedderCategory::Temporal.is_structural());
        assert!(!EmbedderCategory::Relational.is_structural());
        assert!(EmbedderCategory::Structural.is_structural());
        println!("[PASS] is_structural() returns true only for Structural");
    }

    #[test]
    fn test_all_categories() {
        let all = EmbedderCategory::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&EmbedderCategory::Semantic));
        assert!(all.contains(&EmbedderCategory::Temporal));
        assert!(all.contains(&EmbedderCategory::Relational));
        assert!(all.contains(&EmbedderCategory::Structural));
        println!("[PASS] all() returns all 4 categories");
    }

    #[test]
    fn test_max_weighted_agreement() {
        // 7 * 1.0 + 3 * 0.0 + 2 * 0.5 + 1 * 0.5 = 8.5
        assert!((max_weighted_agreement() - 8.5).abs() < f32::EPSILON);
        println!("[PASS] max_weighted_agreement() = 8.5");
    }

    #[test]
    fn test_topic_threshold() {
        assert!((topic_threshold() - 2.5).abs() < f32::EPSILON);
        println!("[PASS] topic_threshold() = 2.5 (per ARCH-09)");
    }

    #[test]
    fn test_category_for_semantic_embedders() {
        // 7 semantic embedders
        assert_eq!(category_for(Embedder::Semantic), EmbedderCategory::Semantic);
        assert_eq!(category_for(Embedder::Causal), EmbedderCategory::Semantic);
        assert_eq!(category_for(Embedder::Sparse), EmbedderCategory::Semantic);
        assert_eq!(category_for(Embedder::Code), EmbedderCategory::Semantic);
        assert_eq!(category_for(Embedder::Multimodal), EmbedderCategory::Semantic);
        assert_eq!(category_for(Embedder::LateInteraction), EmbedderCategory::Semantic);
        assert_eq!(category_for(Embedder::KeywordSplade), EmbedderCategory::Semantic);
        println!("[PASS] All 7 semantic embedders correctly categorized");
    }

    #[test]
    fn test_category_for_temporal_embedders() {
        // 3 temporal embedders
        assert_eq!(category_for(Embedder::TemporalRecent), EmbedderCategory::Temporal);
        assert_eq!(category_for(Embedder::TemporalPeriodic), EmbedderCategory::Temporal);
        assert_eq!(category_for(Embedder::TemporalPositional), EmbedderCategory::Temporal);
        println!("[PASS] All 3 temporal embedders correctly categorized");
    }

    #[test]
    fn test_category_for_relational_embedders() {
        // 2 relational embedders
        assert_eq!(category_for(Embedder::Emotional), EmbedderCategory::Relational);
        assert_eq!(category_for(Embedder::Entity), EmbedderCategory::Relational);
        println!("[PASS] All 2 relational embedders correctly categorized");
    }

    #[test]
    fn test_category_for_structural_embedders() {
        // 1 structural embedder
        assert_eq!(category_for(Embedder::Hdc), EmbedderCategory::Structural);
        println!("[PASS] Structural embedder (E9 HDC) correctly categorized");
    }

    #[test]
    fn test_all_embedders_have_category() {
        // Verify all 13 embedders are covered
        for embedder in Embedder::all() {
            let cat = category_for(embedder);
            // Just verify it doesn't panic and returns a valid category
            assert!(EmbedderCategory::all().contains(&cat));
        }
        assert_eq!(Embedder::all().count(), 13);
        println!("[PASS] All 13 embedders have valid category assignments");
    }

    #[test]
    fn test_embedder_count_by_category() {
        let (semantic, temporal, relational, structural) = EmbedderCategory::count_by_category();
        assert_eq!(semantic, 7);
        assert_eq!(temporal, 3);
        assert_eq!(relational, 2);
        assert_eq!(structural, 1);
        assert_eq!(semantic + temporal + relational + structural, 13);
        println!("[PASS] Category counts: semantic=7, temporal=3, relational=2, structural=1");
    }

    #[test]
    fn test_divergence_detection_semantic_only() {
        // ARCH-10: Only semantic embedders used for divergence detection
        assert!(EmbedderCategory::Semantic.used_for_divergence_detection());
        assert!(!EmbedderCategory::Temporal.used_for_divergence_detection());
        assert!(!EmbedderCategory::Relational.used_for_divergence_detection());
        assert!(!EmbedderCategory::Structural.used_for_divergence_detection());
        println!("[PASS] Divergence detection uses SEMANTIC only (per ARCH-10)");
    }

    #[test]
    fn test_ap60_temporal_excluded() {
        // AP-60: Temporal embedders MUST NOT count toward topic detection
        for embedder in [Embedder::TemporalRecent, Embedder::TemporalPeriodic, Embedder::TemporalPositional] {
            let cat = category_for(embedder);
            assert_eq!(cat.topic_weight(), 0.0, "Temporal embedder {:?} has non-zero weight", embedder);
        }
        println!("[PASS] AP-60 verified: temporal embedders have weight 0.0");
    }

    #[test]
    fn test_topic_examples_from_constitution() {
        // From constitution.yaml topic_detection.examples:

        // "3 semantic spaces agreeing = 3.0 -> TOPIC"
        let three_semantic = 3.0 * EmbedderCategory::Semantic.topic_weight();
        assert!(three_semantic >= topic_threshold());

        // "2 semantic + 1 relational = 2.5 -> TOPIC"
        let two_sem_one_rel: f32 = 2.0 * 1.0 + 1.0 * 0.5;
        assert!((two_sem_one_rel - 2.5).abs() < f32::EPSILON);
        assert!(two_sem_one_rel >= topic_threshold());

        // "2 semantic spaces only = 2.0 -> NOT TOPIC"
        let two_semantic = 2.0 * 1.0;
        assert!(two_semantic < topic_threshold());

        // "5 temporal spaces = 0.0 -> NOT TOPIC"
        let five_temporal = 5.0 * EmbedderCategory::Temporal.topic_weight();
        assert_eq!(five_temporal, 0.0);
        assert!(five_temporal < topic_threshold());

        // "1 semantic + 3 relational = 2.5 -> TOPIC"
        let one_sem_three_rel: f32 = 1.0 * 1.0 + 3.0 * 0.5;
        assert!((one_sem_three_rel - 2.5).abs() < f32::EPSILON);
        assert!(one_sem_three_rel >= topic_threshold());

        println!("[PASS] All constitution topic examples verified");
    }

    #[test]
    fn test_serialization_roundtrip() {
        for cat in EmbedderCategory::all() {
            let json = serde_json::to_string(&cat).expect("serialize");
            let restored: EmbedderCategory = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cat, restored);
        }
        println!("[PASS] Serialization roundtrip works for all categories");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", EmbedderCategory::Semantic), "Semantic");
        assert_eq!(format!("{}", EmbedderCategory::Temporal), "Temporal");
        assert_eq!(format!("{}", EmbedderCategory::Relational), "Relational");
        assert_eq!(format!("{}", EmbedderCategory::Structural), "Structural");
        println!("[PASS] Display trait works correctly");
    }
}
