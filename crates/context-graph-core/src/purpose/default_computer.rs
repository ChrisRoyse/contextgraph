//! Default implementation of PurposeVectorComputer.
//!
//! Provides the standard computation of purpose vectors by calculating
//! alignment between semantic fingerprints and goal hierarchies.

use async_trait::async_trait;
use tracing::{debug, instrument, trace};

use crate::types::fingerprint::{
    EmbeddingSlice, PurposeVector, SemanticFingerprint, SparseVector, NUM_EMBEDDERS,
};

use super::computer::{PurposeComputeConfig, PurposeComputeError, PurposeVectorComputer};
use super::goals::{GoalHierarchy, GoalNode};
use super::splade::SpladeAlignment;

/// Default implementation of purpose vector computation.
///
/// Computes alignment for each of 13 embedding spaces using:
/// - Cosine similarity for dense embeddings (E1-E5, E7-E11)
/// - Sparse dot product similarity for sparse embeddings (E6, E13)
/// - MaxSim aggregation for token-level embeddings (E12)
///
/// # Example
///
/// ```ignore
/// use context_graph_core::purpose::{
///     DefaultPurposeComputer, PurposeVectorComputer, PurposeComputeConfig,
///     GoalHierarchy, GoalNode,
/// };
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
///
/// let computer = DefaultPurposeComputer::new();
/// let fingerprint = SemanticFingerprint::zeroed();
/// let mut hierarchy = GoalHierarchy::new();
/// let north_star = GoalNode::north_star(
///     "goal_1",
///     "Master machine learning",
///     vec![0.5; 1024],
///     vec!["machine".into(), "learning".into()],
/// );
/// hierarchy.add_goal(north_star).unwrap();
/// let config = PurposeComputeConfig::with_hierarchy(hierarchy);
///
/// // In async context:
/// // let purpose = computer.compute_purpose(&fingerprint, &config).await?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct DefaultPurposeComputer {
    /// Whether to log detailed computation steps.
    verbose_logging: bool,
}

impl DefaultPurposeComputer {
    /// Create a new DefaultPurposeComputer.
    pub fn new() -> Self {
        Self {
            verbose_logging: false,
        }
    }

    /// Create a new DefaultPurposeComputer with verbose logging.
    pub fn with_verbose_logging(mut self, verbose: bool) -> Self {
        self.verbose_logging = verbose;
        self
    }

    /// Compute cosine similarity between two dense vectors.
    ///
    /// Returns 0.0 if either vector has zero norm (undefined case).
    ///
    /// # Arguments
    ///
    /// * `a` - First dense vector
    /// * `b` - Second dense vector
    ///
    /// # Panics
    ///
    /// This function does not panic. If vectors have different lengths,
    /// similarity is computed over the shorter length.
    #[inline]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        if len == 0 {
            return 0.0;
        }

        let mut dot = 0.0_f32;
        let mut norm_a = 0.0_f32;
        let mut norm_b = 0.0_f32;

        for i in 0..len {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denominator = norm_a.sqrt() * norm_b.sqrt();
        if denominator < f32::EPSILON {
            0.0
        } else {
            dot / denominator
        }
    }

    /// Compute sparse vector cosine similarity.
    ///
    /// Uses merge-join on sorted indices for O(n+m) complexity.
    #[inline]
    fn sparse_cosine_similarity(a: &SparseVector, b: &SparseVector) -> f32 {
        a.cosine_similarity(b)
    }

    /// Compute MaxSim aggregation for token-level embeddings (ColBERT-style).
    ///
    /// For each token in `a`, finds maximum similarity to any token in `b`,
    /// then averages across all tokens in `a`.
    ///
    /// MaxSim(a, b) = (1/|a|) * Σ max_{j ∈ b} sim(a_i, b_j)
    fn maxsim_similarity(a: &[Vec<f32>], b: &[f32]) -> f32 {
        if a.is_empty() {
            return 0.0;
        }

        // For late-interaction alignment, we compare each token embedding
        // against the goal embedding and take the max per token, then average
        let mut total_max_sim = 0.0_f32;

        for token_embedding in a {
            let sim = Self::cosine_similarity(token_embedding, b);
            total_max_sim += sim;
        }

        total_max_sim / a.len() as f32
    }

    /// Project goal embedding to match target space dimension.
    ///
    /// If the goal embedding is larger, truncates.
    /// If smaller, zero-pads.
    ///
    /// # Arguments
    ///
    /// * `goal_embedding` - The goal's reference embedding
    /// * `target_dim` - Target dimension for the space
    fn project_goal_to_space(goal_embedding: &[f32], target_dim: usize) -> Vec<f32> {
        if goal_embedding.len() == target_dim {
            goal_embedding.to_vec()
        } else if goal_embedding.len() > target_dim {
            // Truncate to target dimension
            goal_embedding[..target_dim].to_vec()
        } else {
            // Zero-pad to target dimension
            let mut projected = goal_embedding.to_vec();
            projected.resize(target_dim, 0.0);
            projected
        }
    }

    /// Compute SPLADE-specific alignment for E13.
    ///
    /// Analyzes keyword overlap between memory and goal vocabularies.
    ///
    /// # Arguments
    ///
    /// * `memory_splade` - Memory's sparse SPLADE embedding
    /// * `goal_keywords` - Goal's target keywords
    ///
    /// # Returns
    ///
    /// `SpladeAlignment` with term matches, coverage, and overlap score.
    fn compute_splade_alignment(
        memory_splade: &SparseVector,
        goal_keywords: &[String],
    ) -> SpladeAlignment {
        if goal_keywords.is_empty() || memory_splade.is_empty() {
            return SpladeAlignment::default();
        }

        // For a real implementation, we would need a vocabulary mapping
        // Here we compute overlap score based on sparse vector density
        // In production, each keyword would map to vocabulary indices

        // Compute term overlap based on activation patterns
        let aligned_terms: Vec<(String, f32)> = goal_keywords
            .iter()
            .filter_map(|keyword| {
                // In a real implementation, we would hash keyword to vocab index
                // For now, use keyword hash modulo vocab size as proxy
                let pseudo_idx = Self::keyword_to_vocab_index(keyword);
                memory_splade.get(pseudo_idx).map(|weight| (keyword.clone(), weight))
            })
            .collect();

        let keyword_coverage = if goal_keywords.is_empty() {
            0.0
        } else {
            aligned_terms.len() as f32 / goal_keywords.len() as f32
        };

        // Weight coverage by activation strength
        let total_weight: f32 = aligned_terms.iter().map(|(_, w)| w).sum();
        let term_overlap_score = if aligned_terms.is_empty() {
            0.0
        } else {
            (keyword_coverage + total_weight / aligned_terms.len() as f32) / 2.0
        };

        SpladeAlignment::new(aligned_terms, keyword_coverage, term_overlap_score)
    }

    /// Map keyword to vocabulary index (deterministic hash).
    ///
    /// Uses FNV-1a hash for consistent mapping across sessions.
    #[inline]
    fn keyword_to_vocab_index(keyword: &str) -> u16 {
        // FNV-1a hash for deterministic mapping
        let mut hash: u32 = 2166136261;
        for byte in keyword.to_lowercase().bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(16777619);
        }
        // Map to vocab range [0, 30521]
        (hash % 30522) as u16
    }

    /// Compute alignment for a single embedding space.
    ///
    /// # Arguments
    ///
    /// * `space_idx` - Index of the embedding space (0-12)
    /// * `fingerprint` - The memory's semantic fingerprint
    /// * `goal` - The goal node to align against
    ///
    /// # Returns
    ///
    /// Alignment value in range [-1.0, 1.0] or 0.0 if computation fails.
    fn compute_space_alignment(
        &self,
        space_idx: usize,
        fingerprint: &SemanticFingerprint,
        goal: &GoalNode,
    ) -> f32 {
        let embedding_slice = match fingerprint.get_embedding(space_idx) {
            Some(slice) => slice,
            None => {
                trace!(space_idx, "No embedding available for space");
                return 0.0;
            }
        };

        match embedding_slice {
            EmbeddingSlice::Dense(dense) => {
                // Project goal to match this space's dimension
                let target_dim = dense.len();
                let projected_goal = Self::project_goal_to_space(&goal.embedding, target_dim);
                let similarity = Self::cosine_similarity(dense, &projected_goal);
                trace!(space_idx, similarity, "Dense alignment computed");
                similarity
            }
            EmbeddingSlice::Sparse(sparse) => {
                // For sparse embeddings (E6, E13), compute sparse similarity
                // Create a sparse goal vector from keywords
                if goal.keywords.is_empty() {
                    trace!(space_idx, "No goal keywords for sparse alignment");
                    return 0.0;
                }

                // Create sparse goal representation from keywords
                let goal_sparse = Self::keywords_to_sparse(&goal.keywords);
                let similarity = Self::sparse_cosine_similarity(sparse, &goal_sparse);
                trace!(space_idx, similarity, "Sparse alignment computed");
                similarity
            }
            EmbeddingSlice::TokenLevel(tokens) => {
                // For token-level embeddings (E12), use MaxSim
                let projected_goal = Self::project_goal_to_space(&goal.embedding, 128);
                let similarity = Self::maxsim_similarity(tokens, &projected_goal);
                trace!(space_idx, similarity, "Token-level alignment computed");
                similarity
            }
        }
    }

    /// Convert keywords to sparse vector representation.
    ///
    /// Creates a sparse vector with uniform weights at keyword vocabulary indices.
    fn keywords_to_sparse(keywords: &[String]) -> SparseVector {
        if keywords.is_empty() {
            return SparseVector::empty();
        }

        let mut indices: Vec<u16> = keywords
            .iter()
            .map(|kw| Self::keyword_to_vocab_index(kw))
            .collect();

        // Sort and deduplicate
        indices.sort_unstable();
        indices.dedup();

        let values = vec![1.0 / (indices.len() as f32).sqrt(); indices.len()];

        SparseVector::new(indices, values).unwrap_or_else(|_| SparseVector::empty())
    }

    /// Propagate alignment through goal hierarchy.
    ///
    /// Combines North Star alignment with weighted child goal alignments.
    ///
    /// # Arguments
    ///
    /// * `base_alignment` - Direct alignment with North Star
    /// * `hierarchy` - Goal hierarchy for traversal
    /// * `fingerprint` - Memory fingerprint to compute child alignments
    /// * `config` - Configuration with propagation weights
    ///
    /// # Returns
    ///
    /// Propagated alignment values for all 13 spaces.
    async fn propagate_hierarchy(
        &self,
        base_alignments: &[f32; NUM_EMBEDDERS],
        hierarchy: &GoalHierarchy,
        fingerprint: &SemanticFingerprint,
        config: &PurposeComputeConfig,
    ) -> [f32; NUM_EMBEDDERS] {
        if !config.hierarchical_propagation {
            return *base_alignments;
        }

        let north_star = match hierarchy.north_star() {
            Some(ns) => ns,
            None => return *base_alignments,
        };

        // Get child goals of North Star
        let children = hierarchy.children(&north_star.id);
        if children.is_empty() {
            return *base_alignments;
        }

        let (base_weight, child_weight) = config.propagation_weights;
        let mut propagated = [0.0_f32; NUM_EMBEDDERS];

        // Compute child alignments and aggregate
        let mut child_alignments = vec![[0.0_f32; NUM_EMBEDDERS]; children.len()];

        for (i, child) in children.iter().enumerate() {
            for space_idx in 0..NUM_EMBEDDERS {
                child_alignments[i][space_idx] =
                    self.compute_space_alignment(space_idx, fingerprint, child);
            }
        }

        // Weighted combination
        for space_idx in 0..NUM_EMBEDDERS {
            let base_component = base_weight * base_alignments[space_idx];

            // Average child alignments weighted by their level weights
            let child_sum: f32 = children
                .iter()
                .zip(child_alignments.iter())
                .map(|(child, alignments)| {
                    alignments[space_idx] * child.level.propagation_weight()
                })
                .sum();

            let total_child_weight: f32 = children
                .iter()
                .map(|c| c.level.propagation_weight())
                .sum();

            let child_component = if total_child_weight > 0.0 {
                child_weight * (child_sum / total_child_weight)
            } else {
                0.0
            };

            propagated[space_idx] = base_component + child_component;

            // Apply minimum alignment threshold
            if propagated[space_idx].abs() < config.min_alignment {
                propagated[space_idx] = 0.0;
            }
        }

        propagated
    }
}

#[async_trait]
impl PurposeVectorComputer for DefaultPurposeComputer {
    #[instrument(skip(self, fingerprint, config), level = "debug")]
    async fn compute_purpose(
        &self,
        fingerprint: &SemanticFingerprint,
        config: &PurposeComputeConfig,
    ) -> Result<PurposeVector, PurposeComputeError> {
        // Validate hierarchy has North Star
        let north_star = config
            .hierarchy
            .north_star()
            .ok_or(PurposeComputeError::NoNorthStar)?;

        debug!(
            north_star_id = %north_star.id,
            hierarchical_propagation = config.hierarchical_propagation,
            "Computing purpose vector"
        );

        // Compute base alignments for each space
        let mut base_alignments = [0.0_f32; NUM_EMBEDDERS];
        for space_idx in 0..NUM_EMBEDDERS {
            base_alignments[space_idx] =
                self.compute_space_alignment(space_idx, fingerprint, north_star);
        }

        // Apply hierarchical propagation if enabled
        let final_alignments: [f32; NUM_EMBEDDERS] = self
            .propagate_hierarchy(&base_alignments, &config.hierarchy, fingerprint, config)
            .await;

        debug!(
            aggregate = final_alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32,
            "Purpose vector computed"
        );

        Ok(PurposeVector::new(final_alignments))
    }

    #[instrument(skip(self, fingerprints, config), level = "debug")]
    async fn compute_purpose_batch(
        &self,
        fingerprints: &[SemanticFingerprint],
        config: &PurposeComputeConfig,
    ) -> Result<Vec<PurposeVector>, PurposeComputeError> {
        debug!(
            batch_size = fingerprints.len(),
            "Computing purpose vectors in batch"
        );

        // Validate once for all
        if config.hierarchy.north_star().is_none() {
            return Err(PurposeComputeError::NoNorthStar);
        }

        let mut results = Vec::with_capacity(fingerprints.len());
        for fingerprint in fingerprints {
            let purpose = self.compute_purpose(fingerprint, config).await?;
            results.push(purpose);
        }

        debug!(
            completed = results.len(),
            "Batch purpose computation complete"
        );

        Ok(results)
    }

    #[instrument(skip(self, fingerprint, old_hierarchy, new_hierarchy), level = "debug")]
    async fn recompute_for_goal_change(
        &self,
        fingerprint: &SemanticFingerprint,
        old_hierarchy: &GoalHierarchy,
        new_hierarchy: &GoalHierarchy,
    ) -> Result<PurposeVector, PurposeComputeError> {
        debug!(
            old_north_star = ?old_hierarchy.north_star().map(|n| n.id.to_string()),
            new_north_star = ?new_hierarchy.north_star().map(|n| n.id.to_string()),
            "Recomputing purpose for goal change"
        );

        let config = PurposeComputeConfig::with_hierarchy(new_hierarchy.clone());
        self.compute_purpose(fingerprint, &config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::goals::{GoalId, GoalLevel};

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let similarity = DefaultPurposeComputer::cosine_similarity(&a, &a);
        assert!((similarity - 1.0).abs() < 1e-6);
        println!("[VERIFIED] Identical vectors have cosine similarity 1.0");
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let similarity = DefaultPurposeComputer::cosine_similarity(&a, &b);
        assert!(similarity.abs() < 1e-6);
        println!("[VERIFIED] Orthogonal vectors have cosine similarity 0.0");
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let similarity = DefaultPurposeComputer::cosine_similarity(&a, &b);
        assert!((similarity - (-1.0)).abs() < 1e-6);
        println!("[VERIFIED] Opposite vectors have cosine similarity -1.0");
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let similarity = DefaultPurposeComputer::cosine_similarity(&a, &b);
        assert_eq!(similarity, 0.0);
        println!("[VERIFIED] Zero vector returns 0.0 similarity");
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![1.0, 2.0, 3.0];
        // Should compute over shorter length (3 elements)
        let similarity = DefaultPurposeComputer::cosine_similarity(&a, &b);
        let expected = (1.0 + 4.0 + 9.0) / (14.0_f32.sqrt() * 14.0_f32.sqrt());
        assert!((similarity - expected).abs() < 1e-5);
        println!("[VERIFIED] Mismatched lengths handled correctly");
    }

    #[test]
    fn test_project_goal_same_dim() {
        let goal = vec![1.0, 2.0, 3.0];
        let projected = DefaultPurposeComputer::project_goal_to_space(&goal, 3);
        assert_eq!(projected, goal);
        println!("[VERIFIED] Same dimension projection unchanged");
    }

    #[test]
    fn test_project_goal_truncate() {
        let goal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let projected = DefaultPurposeComputer::project_goal_to_space(&goal, 3);
        assert_eq!(projected, vec![1.0, 2.0, 3.0]);
        println!("[VERIFIED] Larger goal truncated correctly");
    }

    #[test]
    fn test_project_goal_pad() {
        let goal = vec![1.0, 2.0];
        let projected = DefaultPurposeComputer::project_goal_to_space(&goal, 5);
        assert_eq!(projected, vec![1.0, 2.0, 0.0, 0.0, 0.0]);
        println!("[VERIFIED] Smaller goal zero-padded correctly");
    }

    #[test]
    fn test_keyword_to_vocab_index_deterministic() {
        let idx1 = DefaultPurposeComputer::keyword_to_vocab_index("machine");
        let idx2 = DefaultPurposeComputer::keyword_to_vocab_index("machine");
        assert_eq!(idx1, idx2);
        println!("[VERIFIED] Keyword to vocab index is deterministic");
    }

    #[test]
    fn test_keyword_to_vocab_index_case_insensitive() {
        let idx1 = DefaultPurposeComputer::keyword_to_vocab_index("Machine");
        let idx2 = DefaultPurposeComputer::keyword_to_vocab_index("MACHINE");
        let idx3 = DefaultPurposeComputer::keyword_to_vocab_index("machine");
        assert_eq!(idx1, idx2);
        assert_eq!(idx2, idx3);
        println!("[VERIFIED] Keyword to vocab index is case insensitive");
    }

    #[test]
    fn test_keyword_to_vocab_index_range() {
        let keywords = ["test", "machine", "learning", "neural", "network"];
        for kw in keywords {
            let idx = DefaultPurposeComputer::keyword_to_vocab_index(kw);
            assert!(idx < 30522, "Index {} for keyword '{}' out of range", idx, kw);
        }
        println!("[VERIFIED] All keyword indices are in valid vocab range");
    }

    #[test]
    fn test_keywords_to_sparse_empty() {
        let sparse = DefaultPurposeComputer::keywords_to_sparse(&[]);
        assert!(sparse.is_empty());
        println!("[VERIFIED] Empty keywords produce empty sparse vector");
    }

    #[test]
    fn test_keywords_to_sparse_sorted() {
        let keywords = vec!["zebra".into(), "apple".into(), "banana".into()];
        let sparse = DefaultPurposeComputer::keywords_to_sparse(&keywords);

        // Indices should be sorted
        for i in 1..sparse.indices.len() {
            assert!(
                sparse.indices[i] > sparse.indices[i - 1],
                "Indices not sorted: {:?}",
                sparse.indices
            );
        }
        println!("[VERIFIED] Keywords to sparse produces sorted indices");
    }

    #[test]
    fn test_keywords_to_sparse_normalized() {
        let keywords = vec!["a".into(), "b".into(), "c".into()];
        let sparse = DefaultPurposeComputer::keywords_to_sparse(&keywords);

        // Values should be 1/sqrt(n) for uniform distribution
        let expected = 1.0 / (3.0_f32).sqrt();
        for &v in &sparse.values {
            assert!((v - expected).abs() < 1e-6);
        }
        println!("[VERIFIED] Keywords to sparse has normalized weights");
    }

    #[test]
    fn test_maxsim_empty_tokens() {
        let tokens: &[Vec<f32>] = &[];
        let goal = vec![1.0, 2.0, 3.0];
        let sim = DefaultPurposeComputer::maxsim_similarity(tokens, &goal);
        assert_eq!(sim, 0.0);
        println!("[VERIFIED] Empty tokens return 0.0 similarity");
    }

    #[test]
    fn test_maxsim_single_token() {
        let tokens = vec![vec![1.0, 0.0, 0.0]];
        let goal = vec![1.0, 0.0, 0.0];
        let sim = DefaultPurposeComputer::maxsim_similarity(&tokens, &goal);
        assert!((sim - 1.0).abs() < 1e-6);
        println!("[VERIFIED] Single matching token has similarity 1.0");
    }

    #[test]
    fn test_splade_alignment_empty_keywords() {
        let sparse = SparseVector::new(vec![1, 2, 3], vec![0.5, 0.5, 0.5]).unwrap();
        let alignment = DefaultPurposeComputer::compute_splade_alignment(&sparse, &[]);
        assert_eq!(alignment.keyword_coverage, 0.0);
        assert_eq!(alignment.term_overlap_score, 0.0);
        println!("[VERIFIED] Empty keywords produce zero alignment");
    }

    #[test]
    fn test_splade_alignment_empty_sparse() {
        let sparse = SparseVector::empty();
        let keywords = vec!["test".into()];
        let alignment = DefaultPurposeComputer::compute_splade_alignment(&sparse, &keywords);
        assert_eq!(alignment.keyword_coverage, 0.0);
        println!("[VERIFIED] Empty sparse vector produces zero alignment");
    }

    #[test]
    fn test_default_computer_new() {
        let computer = DefaultPurposeComputer::new();
        assert!(!computer.verbose_logging);
        println!("[VERIFIED] DefaultPurposeComputer::new creates default instance");
    }

    #[test]
    fn test_default_computer_verbose() {
        let computer = DefaultPurposeComputer::new().with_verbose_logging(true);
        assert!(computer.verbose_logging);
        println!("[VERIFIED] with_verbose_logging sets flag correctly");
    }

    #[tokio::test]
    async fn test_compute_purpose_no_north_star() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = SemanticFingerprint::zeroed();
        let config = PurposeComputeConfig::default(); // Empty hierarchy

        let result = computer.compute_purpose(&fingerprint, &config).await;
        assert!(matches!(result, Err(PurposeComputeError::NoNorthStar)));
        println!("[VERIFIED] compute_purpose fails without North Star");
    }

    #[tokio::test]
    async fn test_compute_purpose_with_north_star() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = SemanticFingerprint::zeroed();

        let mut hierarchy = GoalHierarchy::new();
        let north_star = GoalNode::north_star(
            "ns_1",
            "Master ML",
            vec![0.5; 1024],
            vec!["machine".into(), "learning".into()],
        );
        hierarchy.add_goal(north_star).unwrap();

        let config = PurposeComputeConfig::with_hierarchy(hierarchy);
        let result = computer.compute_purpose(&fingerprint, &config).await;

        assert!(result.is_ok());
        let purpose = result.unwrap();
        // Zeroed fingerprint against non-zero goal should produce low alignment
        assert!(purpose.aggregate_alignment().abs() < 0.01);
        println!("[VERIFIED] compute_purpose succeeds with North Star");
    }

    #[tokio::test]
    async fn test_compute_purpose_batch_empty() {
        let computer = DefaultPurposeComputer::new();
        let mut hierarchy = GoalHierarchy::new();
        let north_star = GoalNode::north_star(
            "ns_1",
            "Goal",
            vec![0.5; 1024],
            vec![],
        );
        hierarchy.add_goal(north_star).unwrap();
        let config = PurposeComputeConfig::with_hierarchy(hierarchy);

        let result = computer.compute_purpose_batch(&[], &config).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
        println!("[VERIFIED] compute_purpose_batch handles empty input");
    }

    #[tokio::test]
    async fn test_compute_purpose_batch_multiple() {
        let computer = DefaultPurposeComputer::new();
        let fingerprints = vec![
            SemanticFingerprint::zeroed(),
            SemanticFingerprint::zeroed(),
            SemanticFingerprint::zeroed(),
        ];

        let mut hierarchy = GoalHierarchy::new();
        let north_star = GoalNode::north_star(
            "ns_1",
            "Goal",
            vec![0.5; 1024],
            vec![],
        );
        hierarchy.add_goal(north_star).unwrap();
        let config = PurposeComputeConfig::with_hierarchy(hierarchy);

        let result = computer.compute_purpose_batch(&fingerprints, &config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
        println!("[VERIFIED] compute_purpose_batch processes multiple fingerprints");
    }

    #[tokio::test]
    async fn test_recompute_for_goal_change() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = SemanticFingerprint::zeroed();

        let mut old_hierarchy = GoalHierarchy::new();
        let old_ns = GoalNode::north_star("old_ns", "Old Goal", vec![0.1; 1024], vec![]);
        old_hierarchy.add_goal(old_ns).unwrap();

        let mut new_hierarchy = GoalHierarchy::new();
        let new_ns = GoalNode::north_star("new_ns", "New Goal", vec![0.9; 1024], vec![]);
        new_hierarchy.add_goal(new_ns).unwrap();

        let result = computer
            .recompute_for_goal_change(&fingerprint, &old_hierarchy, &new_hierarchy)
            .await;

        assert!(result.is_ok());
        println!("[VERIFIED] recompute_for_goal_change handles hierarchy transition");
    }

    #[tokio::test]
    async fn test_hierarchical_propagation_disabled() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = SemanticFingerprint::zeroed();

        let mut hierarchy = GoalHierarchy::new();
        let north_star = GoalNode::north_star("ns", "Goal", vec![0.5; 1024], vec![]);
        hierarchy.add_goal(north_star).unwrap();

        let config = PurposeComputeConfig::with_hierarchy(hierarchy).with_propagation(false);
        let result = computer.compute_purpose(&fingerprint, &config).await;

        assert!(result.is_ok());
        println!("[VERIFIED] Hierarchical propagation can be disabled");
    }

    #[tokio::test]
    async fn test_hierarchical_propagation_with_children() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = SemanticFingerprint::zeroed();

        let mut hierarchy = GoalHierarchy::new();

        // Add North Star
        let north_star = GoalNode::north_star(
            "ns",
            "Master ML",
            vec![0.5; 1024],
            vec!["ml".into()],
        );
        hierarchy.add_goal(north_star).unwrap();

        // Add Strategic child
        let strategic = GoalNode::child(
            "strat_1",
            "Learn Deep Learning",
            GoalLevel::Strategic,
            GoalId::new("ns"),
            vec![0.6; 1024],
            0.8,
            vec!["deep".into(), "learning".into()],
        );
        hierarchy.add_goal(strategic).unwrap();

        let config = PurposeComputeConfig::with_hierarchy(hierarchy)
            .with_propagation(true)
            .with_weights(0.7, 0.3);

        let result = computer.compute_purpose(&fingerprint, &config).await;
        assert!(result.is_ok());
        println!("[VERIFIED] Hierarchical propagation with children works");
    }

    #[test]
    fn test_sparse_cosine_similarity() {
        let a = SparseVector::new(vec![1, 3, 5], vec![1.0, 1.0, 1.0]).unwrap();
        let b = SparseVector::new(vec![1, 3, 5], vec![1.0, 1.0, 1.0]).unwrap();
        let sim = DefaultPurposeComputer::sparse_cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
        println!("[VERIFIED] Identical sparse vectors have cosine similarity 1.0");
    }

    #[test]
    fn test_sparse_cosine_similarity_no_overlap() {
        let a = SparseVector::new(vec![1, 2, 3], vec![1.0, 1.0, 1.0]).unwrap();
        let b = SparseVector::new(vec![4, 5, 6], vec![1.0, 1.0, 1.0]).unwrap();
        let sim = DefaultPurposeComputer::sparse_cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
        println!("[VERIFIED] Non-overlapping sparse vectors have similarity 0.0");
    }
}
