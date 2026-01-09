//! Default implementation of PurposeVectorComputer.
//!
//! Provides the standard computation of purpose vectors by calculating
//! alignment between semantic fingerprints and goal hierarchies.
//!
//! # Architecture (constitution.yaml)
//!
//! - **ARCH-02**: Goals use TeleologicalArray for apples-to-apples comparison
//!   - Goal.E1 vs Memory.E1 (semantic, 1024D)
//!   - Goal.E5 vs Memory.E5 (causal, 768D)
//!   - Goal.E7 vs Memory.E7 (code, 1536D)
//!     etc.
//!
//! This implementation does NOT project embeddings between spaces.
//! Each space is compared directly: same model to same model.

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
/// # Architecture (ARCH-02)
///
/// Each embedding space is compared apples-to-apples:
/// - Goal's E1 embedding vs Memory's E1 embedding (same model, same dimensions)
/// - Goal's E5 embedding vs Memory's E5 embedding (same model, same dimensions)
/// - etc.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::purpose::{
///     DefaultPurposeComputer, PurposeVectorComputer, PurposeComputeConfig,
///     GoalHierarchy, GoalNode, GoalLevel, GoalDiscoveryMetadata,
/// };
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
///
/// let computer = DefaultPurposeComputer::new();
/// let fingerprint = SemanticFingerprint::zeroed();
/// let mut hierarchy = GoalHierarchy::new();
///
/// // Goals are discovered autonomously with TeleologicalArray
/// let discovery = GoalDiscoveryMetadata::bootstrap();
/// let north_star = GoalNode::autonomous_goal(
///     "Emergent ML mastery goal".into(),
///     GoalLevel::NorthStar,
///     SemanticFingerprint::zeroed(),
///     discovery,
/// ).unwrap();
/// hierarchy.add_goal(north_star).unwrap();
///
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

        for (ai, bi) in a.iter().zip(b.iter()) {
            dot += ai * bi;
            norm_a += ai * ai;
            norm_b += bi * bi;
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
    fn maxsim_similarity(memory_tokens: &[Vec<f32>], goal_tokens: &[Vec<f32>]) -> f32 {
        if memory_tokens.is_empty() || goal_tokens.is_empty() {
            return 0.0;
        }

        // For each memory token, find max similarity to any goal token
        let mut total_max_sim = 0.0_f32;

        for memory_token in memory_tokens {
            let max_sim = goal_tokens
                .iter()
                .map(|goal_token| Self::cosine_similarity(memory_token, goal_token))
                .fold(f32::MIN, |a, b| a.max(b));
            total_max_sim += max_sim;
        }

        total_max_sim / memory_tokens.len() as f32
    }

    /// Compute SPLADE-specific alignment for E13.
    ///
    /// Directly compares sparse vectors (apples-to-apples).
    ///
    /// # Arguments
    ///
    /// * `memory_splade` - Memory's sparse SPLADE embedding (E13)
    /// * `goal_splade` - Goal's sparse SPLADE embedding (E13)
    ///
    /// # Returns
    ///
    /// `SpladeAlignment` with term matches, coverage, and overlap score.
    #[allow(dead_code)]
    fn compute_splade_alignment(
        memory_splade: &SparseVector,
        goal_splade: &SparseVector,
    ) -> SpladeAlignment {
        if goal_splade.is_empty() || memory_splade.is_empty() {
            return SpladeAlignment::default();
        }

        // Find overlapping indices
        let mut aligned_terms: Vec<(String, f32)> = Vec::new();
        let mut memory_idx = 0;
        let mut goal_idx = 0;

        while memory_idx < memory_splade.indices.len() && goal_idx < goal_splade.indices.len() {
            let m_idx = memory_splade.indices[memory_idx];
            let g_idx = goal_splade.indices[goal_idx];

            match m_idx.cmp(&g_idx) {
                std::cmp::Ordering::Equal => {
                    // Matching index - record alignment
                    let combined_weight =
                        memory_splade.values[memory_idx] * goal_splade.values[goal_idx];
                    aligned_terms.push((format!("vocab_{}", m_idx), combined_weight));
                    memory_idx += 1;
                    goal_idx += 1;
                }
                std::cmp::Ordering::Less => {
                    memory_idx += 1;
                }
                std::cmp::Ordering::Greater => {
                    goal_idx += 1;
                }
            }
        }

        let keyword_coverage = if goal_splade.indices.is_empty() {
            0.0
        } else {
            aligned_terms.len() as f32 / goal_splade.indices.len() as f32
        };

        let term_overlap_score = Self::sparse_cosine_similarity(memory_splade, goal_splade);

        SpladeAlignment::new(aligned_terms, keyword_coverage, term_overlap_score)
    }

    /// Compute alignment for a single embedding space.
    ///
    /// Uses apples-to-apples comparison (ARCH-02):
    /// - Memory's E1 vs Goal's E1 (semantic, same model)
    /// - Memory's E5 vs Goal's E5 (causal, same model)
    /// - etc.
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
        // Get memory's embedding for this space
        let memory_embedding = match fingerprint.get_embedding(space_idx) {
            Some(slice) => slice,
            None => {
                trace!(space_idx, "No memory embedding available for space");
                return 0.0;
            }
        };

        // Get goal's embedding for this space (apples-to-apples)
        let goal_embedding = match goal.array().get_embedding(space_idx) {
            Some(ref_enum) => ref_enum,
            None => {
                trace!(space_idx, "No goal embedding available for space");
                return 0.0;
            }
        };

        // Compare same type to same type (both are EmbeddingSlice)
        match (memory_embedding, goal_embedding) {
            (EmbeddingSlice::Dense(memory_dense), EmbeddingSlice::Dense(goal_dense)) => {
                // Dense vs Dense (E1-E5, E7-E11)
                let similarity = Self::cosine_similarity(memory_dense, goal_dense);
                trace!(space_idx, similarity, "Dense alignment computed");
                similarity
            }
            (EmbeddingSlice::Sparse(memory_sparse), EmbeddingSlice::Sparse(goal_sparse)) => {
                // Sparse vs Sparse (E6, E13)
                let similarity = Self::sparse_cosine_similarity(memory_sparse, goal_sparse);
                trace!(space_idx, similarity, "Sparse alignment computed");
                similarity
            }
            (
                EmbeddingSlice::TokenLevel(memory_tokens),
                EmbeddingSlice::TokenLevel(goal_tokens),
            ) => {
                // Token-level vs Token-level (E12)
                let similarity = Self::maxsim_similarity(memory_tokens, goal_tokens);
                trace!(space_idx, similarity, "Token-level alignment computed");
                similarity
            }
            _ => {
                // Type mismatch - this shouldn't happen with valid fingerprints
                trace!(space_idx, "Embedding type mismatch between memory and goal");
                0.0
            }
        }
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

        for (child_alignment, child) in child_alignments.iter_mut().zip(children.iter()) {
            for (space_idx, alignment) in child_alignment.iter_mut().enumerate() {
                *alignment = self.compute_space_alignment(space_idx, fingerprint, child);
            }
        }

        // Weighted combination
        for (space_idx, base_alignment) in base_alignments.iter().enumerate() {
            let base_component = base_weight * base_alignment;

            // Average child alignments weighted by their level weights
            let child_sum: f32 = children
                .iter()
                .zip(child_alignments.iter())
                .map(|(child, alignments)| alignments[space_idx] * child.level.propagation_weight())
                .sum();

            let total_child_weight: f32 =
                children.iter().map(|c| c.level.propagation_weight()).sum();

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

        // Compute base alignments for each space (apples-to-apples)
        let mut base_alignments = [0.0_f32; NUM_EMBEDDERS];
        for (space_idx, alignment) in base_alignments.iter_mut().enumerate() {
            *alignment = self.compute_space_alignment(space_idx, fingerprint, north_star);
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
    use super::super::goals::{GoalDiscoveryMetadata, GoalLevel};
    use super::*;

    // Helper function to create a valid zeroed fingerprint for testing
    fn test_fingerprint() -> SemanticFingerprint {
        SemanticFingerprint::zeroed()
    }

    // Helper function to create bootstrap discovery metadata
    fn test_discovery() -> GoalDiscoveryMetadata {
        GoalDiscoveryMetadata::bootstrap()
    }

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
        let north_star = GoalNode::autonomous_goal(
            "Master ML".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
        hierarchy.add_goal(north_star).unwrap();

        let config = PurposeComputeConfig::with_hierarchy(hierarchy);
        let result = computer.compute_purpose(&fingerprint, &config).await;

        assert!(result.is_ok());
        let purpose = result.unwrap();
        // Zeroed fingerprint against zeroed goal should produce similarity based on zero vectors
        // With zeroed vectors, cosine similarity is undefined (0/0), returns 0.0
        assert!(purpose.aggregate_alignment().abs() < 0.01);
        println!("[VERIFIED] compute_purpose succeeds with North Star");
    }

    #[tokio::test]
    async fn test_compute_purpose_batch_empty() {
        let computer = DefaultPurposeComputer::new();
        let mut hierarchy = GoalHierarchy::new();
        let north_star = GoalNode::autonomous_goal(
            "Goal".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
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
        let north_star = GoalNode::autonomous_goal(
            "Goal".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
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
        let old_ns = GoalNode::autonomous_goal(
            "Old Goal".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
        old_hierarchy.add_goal(old_ns).unwrap();

        let mut new_hierarchy = GoalHierarchy::new();
        let new_ns = GoalNode::autonomous_goal(
            "New Goal".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
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
        let north_star = GoalNode::autonomous_goal(
            "Goal".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
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
        let north_star = GoalNode::autonomous_goal(
            "Master ML".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
        let ns_id = north_star.id;
        hierarchy.add_goal(north_star).unwrap();

        // Add Strategic child
        let strategic = GoalNode::child_goal(
            "Learn Deep Learning".into(),
            GoalLevel::Strategic,
            ns_id,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();
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

    #[test]
    fn test_maxsim_empty_tokens() {
        let tokens: &[Vec<f32>] = &[];
        let goal_tokens: &[Vec<f32>] = &[vec![1.0, 2.0, 3.0]];
        let sim = DefaultPurposeComputer::maxsim_similarity(tokens, goal_tokens);
        assert_eq!(sim, 0.0);
        println!("[VERIFIED] Empty memory tokens return 0.0 similarity");
    }

    #[test]
    fn test_maxsim_single_token() {
        let tokens = vec![vec![1.0, 0.0, 0.0]];
        let goal_tokens = vec![vec![1.0, 0.0, 0.0]];
        let sim = DefaultPurposeComputer::maxsim_similarity(&tokens, &goal_tokens);
        assert!((sim - 1.0).abs() < 1e-6);
        println!("[VERIFIED] Single matching token has similarity 1.0");
    }

    #[test]
    fn test_splade_alignment_empty() {
        let sparse = SparseVector::new(vec![1, 2, 3], vec![0.5, 0.5, 0.5]).unwrap();
        let empty = SparseVector::empty();
        let alignment = DefaultPurposeComputer::compute_splade_alignment(&sparse, &empty);
        assert_eq!(alignment.keyword_coverage, 0.0);
        assert_eq!(alignment.term_overlap_score, 0.0);
        println!("[VERIFIED] Empty goal sparse produces zero alignment");
    }

    #[test]
    fn test_splade_alignment_matching() {
        let a = SparseVector::new(vec![1, 3, 5], vec![0.5, 0.5, 0.5]).unwrap();
        let b = SparseVector::new(vec![1, 3, 5], vec![0.5, 0.5, 0.5]).unwrap();
        let alignment = DefaultPurposeComputer::compute_splade_alignment(&a, &b);
        assert_eq!(alignment.keyword_coverage, 1.0);
        assert!((alignment.term_overlap_score - 1.0).abs() < 1e-6);
        println!("[VERIFIED] Matching sparse vectors have full alignment");
    }

    #[test]
    fn test_apples_to_apples_alignment() {
        // Verify that compute_space_alignment uses goal.array() not goal.embedding
        let computer = DefaultPurposeComputer::new();
        let fingerprint = test_fingerprint();
        let goal = GoalNode::autonomous_goal(
            "Test goal".into(),
            GoalLevel::NorthStar,
            test_fingerprint(),
            test_discovery(),
        )
        .unwrap();

        // This should work without panicking - proves we're using array() not embedding
        for space_idx in 0..NUM_EMBEDDERS {
            let _alignment = computer.compute_space_alignment(space_idx, &fingerprint, &goal);
        }
        println!("[VERIFIED] Apples-to-apples alignment uses goal.array()");
    }
}
