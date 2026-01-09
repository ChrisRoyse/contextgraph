//! TASK-TELEO-013: MultiSpaceRetriever Implementation
//!
//! Teleological-aware retrieval across multi-embedding space.
//! Combines similarity from multiple embeddings weighted by purpose alignment.
//!
//! # From teleoplan.md
//!
//! "Retrieval should leverage the FULL teleological signature -
//! not just semantic similarity, but causal, temporal, and analogical relevance too."

use crate::teleological::{GroupType, TeleologicalVector};
use crate::types::fingerprint::PurposeVector;

/// Configuration for multi-space retrieval.
#[derive(Clone, Debug)]
pub struct RetrievalConfig {
    /// Number of results to return
    pub top_k: usize,
    /// Minimum similarity threshold
    pub min_similarity: f32,
    /// Weight for purpose vector similarity
    pub purpose_weight: f32,
    /// Weight for cross-correlation similarity
    pub correlation_weight: f32,
    /// Weight for group alignment similarity
    pub group_weight: f32,
    /// Enable group filtering
    pub group_filter: Option<GroupType>,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            top_k: 10,
            min_similarity: 0.3,
            purpose_weight: 0.5,
            correlation_weight: 0.3,
            group_weight: 0.2,
            group_filter: None,
        }
    }
}

/// A retrieval result with similarity score.
#[derive(Clone, Debug)]
pub struct RetrievalResult {
    /// Index/ID of the retrieved item
    pub index: usize,
    /// Overall similarity score
    pub similarity: f32,
    /// Component-wise similarity breakdown
    pub component_similarities: ComponentSimilarities,
    /// Whether it passes the group filter (if enabled)
    pub passes_filter: bool,
}

/// Breakdown of similarity by component.
#[derive(Clone, Debug, Default)]
pub struct ComponentSimilarities {
    /// Purpose vector similarity
    pub purpose: f32,
    /// Cross-correlation similarity
    pub correlation: f32,
    /// Group alignment similarity
    pub group: f32,
}

/// TELEO-013: Retrieves items using teleological similarity.
///
/// # Example
///
/// ```
/// use context_graph_core::teleological::services::MultiSpaceRetriever;
/// use context_graph_core::teleological::TeleologicalVector;
///
/// let retriever = MultiSpaceRetriever::new();
/// let query = TeleologicalVector::default();
/// let candidates: Vec<TeleologicalVector> = vec![];
/// let results = retriever.retrieve(&query, &candidates);
/// ```
pub struct MultiSpaceRetriever {
    config: RetrievalConfig,
}

impl MultiSpaceRetriever {
    /// Create a new MultiSpaceRetriever with default configuration.
    pub fn new() -> Self {
        Self {
            config: RetrievalConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: RetrievalConfig) -> Self {
        Self { config }
    }

    /// Retrieve top-k items most similar to query.
    ///
    /// # Arguments
    /// * `query` - The query teleological vector
    /// * `candidates` - Candidate vectors to search
    pub fn retrieve(
        &self,
        query: &TeleologicalVector,
        candidates: &[TeleologicalVector],
    ) -> Vec<RetrievalResult> {
        let mut results: Vec<RetrievalResult> = candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| self.compute_similarity(idx, query, candidate))
            .filter(|r| r.similarity >= self.config.min_similarity && r.passes_filter)
            .collect();

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top-k
        results.truncate(self.config.top_k);

        results
    }

    /// Retrieve with custom weights (overrides config).
    pub fn retrieve_weighted(
        &self,
        query: &TeleologicalVector,
        candidates: &[TeleologicalVector],
        purpose_weight: f32,
        correlation_weight: f32,
        group_weight: f32,
    ) -> Vec<RetrievalResult> {
        let mut results: Vec<RetrievalResult> = candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| {
                self.compute_similarity_weighted(
                    idx,
                    query,
                    candidate,
                    purpose_weight,
                    correlation_weight,
                    group_weight,
                )
            })
            .filter(|r| r.similarity >= self.config.min_similarity && r.passes_filter)
            .collect();

        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(self.config.top_k);

        results
    }

    /// Retrieve items that match a specific group profile.
    pub fn retrieve_by_group(
        &self,
        query: &TeleologicalVector,
        candidates: &[TeleologicalVector],
        group: GroupType,
    ) -> Vec<RetrievalResult> {
        let mut config = self.config.clone();
        config.group_filter = Some(group);

        let retriever = Self::with_config(config);
        retriever.retrieve(query, candidates)
    }

    /// Compute similarity between query and candidate.
    fn compute_similarity(
        &self,
        index: usize,
        query: &TeleologicalVector,
        candidate: &TeleologicalVector,
    ) -> RetrievalResult {
        self.compute_similarity_weighted(
            index,
            query,
            candidate,
            self.config.purpose_weight,
            self.config.correlation_weight,
            self.config.group_weight,
        )
    }

    /// Compute similarity with custom weights.
    fn compute_similarity_weighted(
        &self,
        index: usize,
        query: &TeleologicalVector,
        candidate: &TeleologicalVector,
        purpose_weight: f32,
        correlation_weight: f32,
        group_weight: f32,
    ) -> RetrievalResult {
        // Purpose vector similarity
        let purpose_sim = query.purpose_vector.similarity(&candidate.purpose_vector);

        // Cross-correlation similarity
        let corr_sim =
            Self::correlation_similarity(&query.cross_correlations, &candidate.cross_correlations);

        // Group alignment similarity
        let group_sim = query
            .group_alignments
            .similarity(&candidate.group_alignments);

        // Weighted combination
        let total_weight = purpose_weight + correlation_weight + group_weight;
        let similarity = if total_weight > f32::EPSILON {
            (purpose_weight * purpose_sim
                + correlation_weight * corr_sim
                + group_weight * group_sim)
                / total_weight
        } else {
            0.0
        };

        // Check group filter
        let passes_filter = self.check_group_filter(candidate);

        RetrievalResult {
            index,
            similarity,
            component_similarities: ComponentSimilarities {
                purpose: purpose_sim,
                correlation: corr_sim,
                group: group_sim,
            },
            passes_filter,
        }
    }

    /// Compute cosine similarity between correlation vectors.
    fn correlation_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..a.len() {
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

    /// Check if candidate passes group filter.
    fn check_group_filter(&self, candidate: &TeleologicalVector) -> bool {
        match self.config.group_filter {
            None => true,
            Some(required_group) => {
                // Candidate passes if the required group is dominant or strong
                let dominant = candidate.group_alignments.dominant_group();
                if dominant == required_group {
                    return true;
                }

                // Also pass if the required group has alignment > 0.6
                candidate.group_alignments.get(required_group) > 0.6
            }
        }
    }

    /// Find k-nearest neighbors using only purpose vectors (fast path).
    pub fn retrieve_by_purpose(
        &self,
        query: &PurposeVector,
        candidates: &[PurposeVector],
    ) -> Vec<(usize, f32)> {
        let mut results: Vec<(usize, f32)> = candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| (idx, query.similarity(c)))
            .filter(|(_, sim)| *sim >= self.config.min_similarity)
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results.truncate(self.config.top_k);
        results
    }

    /// Get configuration.
    pub fn config(&self) -> &RetrievalConfig {
        &self.config
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: RetrievalConfig) {
        self.config = config;
    }
}

impl Default for MultiSpaceRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleological::{GroupAlignments, CROSS_CORRELATION_COUNT, NUM_EMBEDDERS};

    fn make_teleological_vector(alignment: f32) -> TeleologicalVector {
        let pv = PurposeVector::new([alignment; NUM_EMBEDDERS]);
        TeleologicalVector::with_all(
            pv,
            vec![alignment; CROSS_CORRELATION_COUNT],
            GroupAlignments::new(
                alignment, alignment, alignment, alignment, alignment, alignment,
            ),
            1.0,
        )
    }

    #[test]
    fn test_multi_space_retriever_new() {
        let retriever = MultiSpaceRetriever::new();
        assert_eq!(retriever.config().top_k, 10);

        println!("[PASS] MultiSpaceRetriever::new creates default config");
    }

    #[test]
    fn test_retrieve_empty_candidates() {
        let retriever = MultiSpaceRetriever::new();
        let query = make_teleological_vector(0.8);
        let candidates: Vec<TeleologicalVector> = vec![];

        let results = retriever.retrieve(&query, &candidates);
        assert!(results.is_empty());

        println!("[PASS] retrieve handles empty candidates");
    }

    #[test]
    fn test_retrieve_finds_similar() {
        let retriever = MultiSpaceRetriever::new();
        let query = make_teleological_vector(0.8);

        let candidates = vec![
            make_teleological_vector(0.8), // Very similar
            make_teleological_vector(0.7), // Similar
            make_teleological_vector(0.2), // Different
        ];

        let results = retriever.retrieve(&query, &candidates);

        // Should find similar items
        assert!(!results.is_empty());

        // Most similar should be first
        assert!(results[0].similarity > 0.8);

        println!("[PASS] retrieve finds similar items");
    }

    #[test]
    fn test_retrieve_respects_top_k() {
        let retriever = MultiSpaceRetriever::with_config(RetrievalConfig {
            top_k: 2,
            min_similarity: 0.0, // Accept all
            ..Default::default()
        });

        let query = make_teleological_vector(0.5);
        let candidates: Vec<TeleologicalVector> = (0..10)
            .map(|i| make_teleological_vector(i as f32 / 10.0))
            .collect();

        let results = retriever.retrieve(&query, &candidates);

        assert_eq!(results.len(), 2);

        println!("[PASS] retrieve respects top_k");
    }

    #[test]
    fn test_retrieve_respects_min_similarity() {
        let retriever = MultiSpaceRetriever::with_config(RetrievalConfig {
            min_similarity: 0.9,
            ..Default::default()
        });

        let query = make_teleological_vector(0.9);
        let candidates = vec![
            make_teleological_vector(0.9), // High
            make_teleological_vector(0.5), // Low - filtered
            make_teleological_vector(0.1), // Very low - filtered
        ];

        let results = retriever.retrieve(&query, &candidates);

        // Only high similarity should pass
        assert!(!results.is_empty());
        for r in &results {
            assert!(r.similarity >= 0.9);
        }

        println!("[PASS] retrieve filters by min_similarity");
    }

    #[test]
    fn test_component_similarities() {
        let retriever = MultiSpaceRetriever::new();
        let query = make_teleological_vector(0.7);
        let candidates = vec![make_teleological_vector(0.7)];

        let results = retriever.retrieve(&query, &candidates);

        assert!(!results.is_empty());
        let r = &results[0];

        // Identical vectors should have high component similarities
        assert!(r.component_similarities.purpose > 0.9);
        assert!(r.component_similarities.group > 0.9);

        println!("[PASS] Component similarities computed");
    }

    #[test]
    fn test_retrieve_weighted() {
        let retriever = MultiSpaceRetriever::new();
        let query = make_teleological_vector(0.8);
        let candidates = vec![make_teleological_vector(0.7)];

        // All weight on purpose
        let purpose_results = retriever.retrieve_weighted(&query, &candidates, 1.0, 0.0, 0.0);

        // All weight on groups
        let group_results = retriever.retrieve_weighted(&query, &candidates, 0.0, 0.0, 1.0);

        // Both should find the candidate (same values in this case)
        assert!(!purpose_results.is_empty());
        assert!(!group_results.is_empty());

        println!("[PASS] retrieve_weighted applies custom weights");
    }

    #[test]
    fn test_retrieve_by_group() {
        let retriever = MultiSpaceRetriever::new();
        let query = make_teleological_vector(0.8);

        // Make candidates with different dominant groups
        let mut code_candidate = make_teleological_vector(0.5);
        code_candidate.group_alignments.implementation = 0.95;

        let candidates = vec![
            make_teleological_vector(0.8), // Uniform
            code_candidate,                // Implementation-dominant
        ];

        let impl_results =
            retriever.retrieve_by_group(&query, &candidates, GroupType::Implementation);

        // Should find the implementation-dominant one
        assert!(!impl_results.is_empty());

        println!("[PASS] retrieve_by_group filters by group");
    }

    #[test]
    fn test_retrieve_by_purpose() {
        let retriever = MultiSpaceRetriever::new();
        let query = PurposeVector::new([0.8f32; NUM_EMBEDDERS]);

        let candidates = vec![
            PurposeVector::new([0.8f32; NUM_EMBEDDERS]),
            PurposeVector::new([0.2f32; NUM_EMBEDDERS]),
        ];

        let results = retriever.retrieve_by_purpose(&query, &candidates);

        assert!(!results.is_empty());
        // Most similar (0.8) should be first
        assert!(results[0].1 > 0.9);

        println!("[PASS] retrieve_by_purpose fast path works");
    }

    #[test]
    fn test_results_sorted_by_similarity() {
        let retriever = MultiSpaceRetriever::with_config(RetrievalConfig {
            min_similarity: 0.0,
            ..Default::default()
        });

        let query = make_teleological_vector(0.5);
        let candidates: Vec<TeleologicalVector> = (1..=5)
            .map(|i| make_teleological_vector(i as f32 / 10.0))
            .collect();

        let results = retriever.retrieve(&query, &candidates);

        // Should be sorted descending by similarity
        for i in 1..results.len() {
            assert!(
                results[i - 1].similarity >= results[i].similarity,
                "Results not sorted: {} < {}",
                results[i - 1].similarity,
                results[i].similarity
            );
        }

        println!("[PASS] Results sorted by similarity descending");
    }
}
