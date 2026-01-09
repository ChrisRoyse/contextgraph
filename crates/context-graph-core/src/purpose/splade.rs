//! SPLADE-specific alignment information for E13.
//!
//! Provides detailed keyword alignment information for the sparse
//! SPLADE embedding space.

use serde::{Deserialize, Serialize};

/// SPLADE-specific alignment information for E13.
///
/// Stores detailed information about how a memory's SPLADE embedding
/// aligns with goal keywords, including which terms matched and coverage.
///
/// # Example
///
/// ```
/// use context_graph_core::purpose::SpladeAlignment;
///
/// let alignment = SpladeAlignment::new(
///     vec![("machine".into(), 0.8), ("learning".into(), 0.6)],
///     0.67, // 2/3 keywords matched
///     0.7,
/// );
///
/// assert!(alignment.is_significant(0.5));
/// assert!(!alignment.is_significant(0.8));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpladeAlignment {
    /// Top aligned terms with goal vocabulary.
    ///
    /// Each tuple contains (term, activation_weight) where weight
    /// indicates how strongly this term is represented in the memory.
    pub aligned_terms: Vec<(String, f32)>,

    /// Fraction of goal keywords found in memory.
    ///
    /// Range: [0.0, 1.0]
    /// 1.0 means all goal keywords are present in the memory.
    pub keyword_coverage: f32,

    /// Weighted term overlap score.
    ///
    /// Range: [0.0, 1.0]
    /// Considers both term presence and activation strength.
    pub term_overlap_score: f32,
}

impl SpladeAlignment {
    /// Create a new SPLADE alignment result.
    ///
    /// # Arguments
    ///
    /// * `aligned_terms` - Terms that match between memory and goal
    /// * `keyword_coverage` - Fraction of goal keywords present
    /// * `term_overlap_score` - Weighted overlap score
    ///
    /// The `term_overlap_score` is clamped to [0.0, 1.0].
    pub fn new(
        aligned_terms: Vec<(String, f32)>,
        keyword_coverage: f32,
        term_overlap_score: f32,
    ) -> Self {
        Self {
            aligned_terms,
            keyword_coverage: keyword_coverage.clamp(0.0, 1.0),
            term_overlap_score: term_overlap_score.clamp(0.0, 1.0),
        }
    }

    /// Check if this alignment is significant.
    ///
    /// An alignment is significant if its term overlap score meets
    /// or exceeds the given threshold.
    #[inline]
    pub fn is_significant(&self, threshold: f32) -> bool {
        self.term_overlap_score >= threshold
    }

    /// Get top N aligned terms sorted by weight.
    ///
    /// Returns references to the (term, weight) tuples, sorted by
    /// weight in descending order.
    pub fn top_terms(&self, n: usize) -> Vec<&(String, f32)> {
        let mut sorted: Vec<_> = self.aligned_terms.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }

    /// Get the number of aligned terms.
    #[inline]
    pub fn aligned_count(&self) -> usize {
        self.aligned_terms.len()
    }

    /// Check if there are any aligned terms.
    #[inline]
    pub fn has_aligned_terms(&self) -> bool {
        !self.aligned_terms.is_empty()
    }

    /// Get the total activation weight of all aligned terms.
    pub fn total_weight(&self) -> f32 {
        self.aligned_terms.iter().map(|(_, w)| w).sum()
    }

    /// Get the average activation weight of aligned terms.
    ///
    /// Returns 0.0 if there are no aligned terms.
    pub fn average_weight(&self) -> f32 {
        if self.aligned_terms.is_empty() {
            0.0
        } else {
            self.total_weight() / self.aligned_terms.len() as f32
        }
    }

    /// Find a specific term in the aligned terms.
    ///
    /// Returns the weight if found, None otherwise.
    pub fn get_term_weight(&self, term: &str) -> Option<f32> {
        let term_lower = term.to_lowercase();
        self.aligned_terms
            .iter()
            .find(|(t, _)| t.to_lowercase() == term_lower)
            .map(|(_, w)| *w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splade_alignment_creation() {
        let aligned = SpladeAlignment::new(
            vec![("machine".into(), 0.8), ("learning".into(), 0.6)],
            0.5,
            0.7,
        );

        assert_eq!(aligned.aligned_terms.len(), 2);
        assert_eq!(aligned.keyword_coverage, 0.5);
        assert_eq!(aligned.term_overlap_score, 0.7);
        println!("[VERIFIED] SpladeAlignment creation works correctly");
    }

    #[test]
    fn test_splade_alignment_clamping() {
        let aligned = SpladeAlignment::new(vec![], 1.5, 2.0);
        assert_eq!(aligned.keyword_coverage, 1.0);
        assert_eq!(aligned.term_overlap_score, 1.0);

        let aligned2 = SpladeAlignment::new(vec![], -0.5, -1.0);
        assert_eq!(aligned2.keyword_coverage, 0.0);
        assert_eq!(aligned2.term_overlap_score, 0.0);
        println!("[VERIFIED] SpladeAlignment clamps values to [0.0, 1.0]");
    }

    #[test]
    fn test_splade_alignment_significance() {
        let aligned = SpladeAlignment::new(vec![("test".into(), 0.5)], 0.3, 0.6);

        assert!(aligned.is_significant(0.5));
        assert!(aligned.is_significant(0.6));
        assert!(!aligned.is_significant(0.7));
        assert!(!aligned.is_significant(1.0));
        println!("[VERIFIED] SpladeAlignment significance check works");
    }

    #[test]
    fn test_splade_alignment_top_terms() {
        let aligned = SpladeAlignment::new(
            vec![("c".into(), 0.3), ("a".into(), 0.9), ("b".into(), 0.6)],
            0.5,
            0.6,
        );

        let top2 = aligned.top_terms(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].0, "a");
        assert_eq!(top2[0].1, 0.9);
        assert_eq!(top2[1].0, "b");
        assert_eq!(top2[1].1, 0.6);
        println!("[VERIFIED] SpladeAlignment top_terms sorts correctly");
    }

    #[test]
    fn test_splade_alignment_top_terms_more_than_available() {
        let aligned = SpladeAlignment::new(vec![("only".into(), 0.5)], 0.5, 0.5);

        let top5 = aligned.top_terms(5);
        assert_eq!(top5.len(), 1);
        println!("[VERIFIED] top_terms handles n > available correctly");
    }

    #[test]
    fn test_splade_alignment_default() {
        let aligned = SpladeAlignment::default();
        assert!(aligned.aligned_terms.is_empty());
        assert_eq!(aligned.keyword_coverage, 0.0);
        assert_eq!(aligned.term_overlap_score, 0.0);
        println!("[VERIFIED] SpladeAlignment default is empty");
    }

    #[test]
    fn test_splade_alignment_helper_methods() {
        let aligned = SpladeAlignment::new(vec![("a".into(), 0.4), ("b".into(), 0.6)], 0.5, 0.5);

        assert_eq!(aligned.aligned_count(), 2);
        assert!(aligned.has_aligned_terms());
        assert!((aligned.total_weight() - 1.0).abs() < 0.001);
        assert!((aligned.average_weight() - 0.5).abs() < 0.001);
        println!("[VERIFIED] SpladeAlignment helper methods work correctly");
    }

    #[test]
    fn test_splade_alignment_get_term_weight() {
        let aligned = SpladeAlignment::new(
            vec![("Machine".into(), 0.8), ("Learning".into(), 0.6)],
            0.5,
            0.7,
        );

        // Case-insensitive lookup
        assert_eq!(aligned.get_term_weight("machine"), Some(0.8));
        assert_eq!(aligned.get_term_weight("LEARNING"), Some(0.6));
        assert_eq!(aligned.get_term_weight("missing"), None);
        println!("[VERIFIED] get_term_weight does case-insensitive lookup");
    }

    #[test]
    fn test_splade_alignment_empty_average() {
        let aligned = SpladeAlignment::default();
        assert_eq!(aligned.average_weight(), 0.0);
        println!("[VERIFIED] average_weight returns 0.0 for empty alignment");
    }

    #[test]
    fn test_splade_alignment_serialization() {
        let aligned = SpladeAlignment::new(vec![("test".into(), 0.5)], 0.6, 0.7);

        let json = serde_json::to_string(&aligned).unwrap();
        let restored: SpladeAlignment = serde_json::from_str(&json).unwrap();

        assert_eq!(aligned, restored);
        println!("[VERIFIED] SpladeAlignment serialization roundtrip works");
    }
}
