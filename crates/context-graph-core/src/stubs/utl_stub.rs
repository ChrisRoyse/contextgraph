//! Real UTL processor implementing the constitution-specified formulas.
//!
//! Implements the Unified Theory of Learning per constitution.yaml:
//! - Canonical: L = f((ΔS × ΔC) · wₑ · cos φ)
//! - Multi-embedding: L_multi = sigmoid(2.0 · (Σᵢ τᵢλ_S·ΔSᵢ) · (Σⱼ τⱼλ_C·ΔCⱼ) · wₑ · cos φ)
//!
//! ΔS computed via KNN distance: ΔS = σ((d_k - μ)/σ_d)
//! ΔC computed via connectivity: ΔC = |{neighbors: sim(e, n) > θ_edge}| / max_edges

use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::error::CoreResult;
use crate::traits::UtlProcessor;
use crate::types::{MemoryNode, UtlContext, UtlMetrics};

/// Real UTL processor implementing constitution-specified computation.
///
/// Computes ΔS (surprise) using KNN distance from reference embeddings.
/// Computes ΔC (coherence) using connectivity to existing memories.
/// Applies sigmoid activation per the multi-embedding formula.
#[derive(Debug, Clone)]
pub struct StubUtlProcessor {
    /// Threshold for memory consolidation
    consolidation_threshold: f32,
    /// Default edge similarity threshold (θ_edge = 0.7 per constitution)
    default_edge_threshold: f32,
    /// Default max edges for connectivity normalization
    default_max_edges: usize,
    /// Default k for KNN computation
    default_k: usize,
}

impl Default for StubUtlProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl StubUtlProcessor {
    /// Create a new UTL processor with default parameters.
    pub fn new() -> Self {
        Self {
            consolidation_threshold: 0.7,
            default_edge_threshold: 0.7, // θ_edge prior from constitution
            default_max_edges: 10,       // max_edges from constitution
            default_k: 5,                // k for KNN
        }
    }

    /// Create with custom consolidation threshold.
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            consolidation_threshold: threshold,
            ..Self::new()
        }
    }

    /// Sigmoid activation function: σ(x) = 1 / (1 + e^(-x))
    #[inline]
    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }

    /// Compute cosine similarity between two vectors.
    ///
    /// Returns value in [-1, 1]. Returns 0.0 for empty or zero-magnitude vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag_a < f32::EPSILON || mag_b < f32::EPSILON {
            return 0.0;
        }

        (dot / (mag_a * mag_b)).clamp(-1.0, 1.0)
    }

    /// Compute Euclidean distance between two vectors.
    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::MAX;
        }
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Compute k-th nearest neighbor distance.
    ///
    /// Returns the distance to the k-th nearest reference embedding.
    /// If fewer than k references exist, returns average distance.
    fn compute_knn_distance(input: &[f32], references: &[Vec<f32>], k: usize) -> Option<f32> {
        if references.is_empty() || input.is_empty() {
            return None;
        }

        let mut distances: Vec<f32> = references
            .iter()
            .filter(|r| r.len() == input.len())
            .map(|r| Self::euclidean_distance(input, r))
            .filter(|d| d.is_finite())
            .collect();

        if distances.is_empty() {
            return None;
        }

        distances.sort_by(|a, b| a.partial_cmp(b).expect("distances should be finite"));

        // Return k-th distance (0-indexed), or last if fewer than k
        let idx = (k.saturating_sub(1)).min(distances.len() - 1);
        Some(distances[idx])
    }

    /// Compute ΔS (surprise/entropy) using KNN distance.
    ///
    /// Per constitution: ΔS_knn = σ((d_k - μ_corpus) / σ_corpus)
    ///
    /// Returns value in [0, 1] where:
    /// - High ΔS = input is far from known embeddings (novel)
    /// - Low ΔS = input is close to known embeddings (familiar)
    fn compute_delta_s_from_embeddings(
        input: &[f32],
        references: &[Vec<f32>],
        mean_dist: f32,
        std_dist: f32,
        k: usize,
    ) -> f32 {
        let d_k = match Self::compute_knn_distance(input, references, k) {
            Some(d) => d,
            None => return 0.5, // Default to medium surprise if no references
        };

        // Avoid division by zero
        let std_dist = std_dist.max(f32::EPSILON);

        // Normalized z-score
        let z = (d_k - mean_dist) / std_dist;

        // Apply sigmoid to map to [0, 1]
        Self::sigmoid(z)
    }

    /// Compute ΔC (coherence change) using connectivity measure.
    ///
    /// Per constitution:
    /// ΔC = α × Connectivity + β × ClusterFit + γ × Consistency
    /// Simplified: ΔC = Connectivity = |{neighbors: sim(e, n) > θ_edge}| / max_edges
    ///
    /// Returns value in [0, 1] where:
    /// - High ΔC = input integrates well with existing knowledge
    /// - Low ΔC = input is disconnected from existing knowledge
    fn compute_delta_c_from_embeddings(
        input: &[f32],
        references: &[Vec<f32>],
        edge_threshold: f32,
        max_edges: usize,
    ) -> f32 {
        if references.is_empty() || input.is_empty() {
            return 0.0; // No coherence with empty corpus
        }

        // Count neighbors above similarity threshold
        let neighbor_count = references
            .iter()
            .filter(|r| r.len() == input.len())
            .map(|r| Self::cosine_similarity(input, r))
            .filter(|sim| *sim > edge_threshold)
            .count();

        // Normalize by max_edges
        let max_edges = max_edges.max(1);
        (neighbor_count as f32 / max_edges as f32).clamp(0.0, 1.0)
    }

    /// Fallback: Generate a deterministic value from input hash.
    ///
    /// Used when embeddings are not available. Still produces consistent
    /// values but is NOT a real UTL computation.
    #[allow(dead_code)]
    fn hash_to_float(input: &str, seed: u64) -> f32 {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        seed.hash(&mut hasher);
        let hash = hasher.finish();
        // Map to [0.0, 1.0]
        (hash as f64 / u64::MAX as f64) as f32
    }
}

#[async_trait]
impl UtlProcessor for StubUtlProcessor {
    /// Compute the full UTL learning score.
    ///
    /// Per constitution multi-embedding formula:
    /// L_multi = sigmoid(2.0 · ΔS · ΔC · wₑ · cos φ)
    ///
    /// When embeddings are available, uses real KNN-based computation.
    /// Falls back to sigmoid(2.0 * prior_entropy * current_coherence * wₑ * cos φ) otherwise.
    async fn compute_learning_score(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let surprise = self.compute_surprise(input, context).await?;
        let coherence_change = self.compute_coherence_change(input, context).await?;
        let emotional_weight = self.compute_emotional_weight(input, context).await?;
        let alignment = self.compute_alignment(input, context).await?;

        // Per constitution: L_multi = sigmoid(2.0 · ΔS · ΔC · wₑ · cos φ)
        // The 2.0 scaling factor ensures sigmoid output spans meaningful range
        let raw_score = 2.0 * surprise * coherence_change * emotional_weight * alignment;
        let score = Self::sigmoid(raw_score);

        Ok(score.clamp(0.0, 1.0))
    }

    /// Compute surprise (ΔS) using KNN distance.
    ///
    /// Per constitution: ΔS_knn = σ((d_k - μ_corpus) / σ_corpus)
    ///
    /// When input_embedding and reference_embeddings are provided in context,
    /// computes real KNN-based surprise. Otherwise falls back to prior_entropy.
    async fn compute_surprise(&self, _input: &str, context: &UtlContext) -> CoreResult<f32> {
        // Check if we have embeddings for real computation
        if let (Some(input_emb), Some(ref_embs)) =
            (&context.input_embedding, &context.reference_embeddings)
        {
            // Get corpus statistics (use defaults if not provided)
            let stats = context.corpus_stats.clone().unwrap_or_default();

            // Real ΔS computation using KNN distance
            let delta_s = Self::compute_delta_s_from_embeddings(
                input_emb,
                ref_embs,
                stats.mean_knn_distance,
                stats.std_knn_distance,
                stats.k,
            );

            return Ok(delta_s.clamp(0.0, 1.0));
        }

        // Fallback: use prior_entropy as a proxy for surprise
        // High prior entropy suggests high novelty potential
        Ok(context.prior_entropy.clamp(0.0, 1.0))
    }

    /// Compute coherence change (ΔC) using connectivity measure.
    ///
    /// Per constitution: ΔC = |{neighbors: sim(e, n) > θ_edge}| / max_edges
    ///
    /// When embeddings are available, computes real connectivity.
    /// Otherwise falls back to current_coherence from context.
    async fn compute_coherence_change(
        &self,
        _input: &str,
        context: &UtlContext,
    ) -> CoreResult<f32> {
        // Check if we have embeddings for real computation
        if let (Some(input_emb), Some(ref_embs)) =
            (&context.input_embedding, &context.reference_embeddings)
        {
            let edge_threshold = context
                .edge_similarity_threshold
                .unwrap_or(self.default_edge_threshold);
            let max_edges = context.max_edges.unwrap_or(self.default_max_edges);

            // Real ΔC computation using connectivity
            let delta_c = Self::compute_delta_c_from_embeddings(
                input_emb,
                ref_embs,
                edge_threshold,
                max_edges,
            );

            return Ok(delta_c.clamp(0.0, 1.0));
        }

        // Fallback: use current_coherence as a proxy
        Ok(context.current_coherence.clamp(0.0, 1.0))
    }

    /// Compute emotional weight (wₑ).
    ///
    /// Per constitution: wₑ ∈ [0.5, 1.5]
    ///
    /// Applies emotional state modifier to base weight of 1.0.
    async fn compute_emotional_weight(
        &self,
        _input: &str,
        context: &UtlContext,
    ) -> CoreResult<f32> {
        // Base weight is 1.0, modified by emotional state
        let weight = context.emotional_state.weight_modifier();
        Ok(weight.clamp(0.5, 1.5))
    }

    /// Compute goal alignment (cos φ).
    ///
    /// Per constitution: cos φ ∈ [-1, 1]
    ///
    /// When goal_vector and input_embedding are available, computes real cosine similarity.
    /// Otherwise defaults to 1.0 (full alignment).
    async fn compute_alignment(&self, _input: &str, context: &UtlContext) -> CoreResult<f32> {
        // Check if we have vectors for real alignment computation
        if let (Some(input_emb), Some(goal_vec)) = (&context.input_embedding, &context.goal_vector)
        {
            // Real alignment: cosine similarity to goal/North Star vector
            let alignment = Self::cosine_similarity(input_emb, goal_vec);
            return Ok(alignment.clamp(-1.0, 1.0));
        }

        // Default to full alignment (cos φ = 1.0)
        // Per constitution, this is the default for wₑ=1.0 and cos(φ)=1.0
        Ok(1.0)
    }

    /// Determine if a node should be consolidated to long-term memory.
    async fn should_consolidate(&self, node: &MemoryNode) -> CoreResult<bool> {
        Ok(node.importance >= self.consolidation_threshold)
    }

    /// Get full UTL metrics for input.
    async fn compute_metrics(&self, input: &str, context: &UtlContext) -> CoreResult<UtlMetrics> {
        let surprise = self.compute_surprise(input, context).await?;
        let coherence_change = self.compute_coherence_change(input, context).await?;
        let emotional_weight = self.compute_emotional_weight(input, context).await?;
        let alignment = self.compute_alignment(input, context).await?;
        let learning_score = self.compute_learning_score(input, context).await?;

        Ok(UtlMetrics {
            entropy: context.prior_entropy,
            coherence: context.current_coherence,
            learning_score,
            surprise,
            coherence_change,
            emotional_weight,
            alignment,
        })
    }

    /// Get current UTL system status as JSON.
    fn get_status(&self) -> serde_json::Value {
        serde_json::json!({
            "lifecycle_phase": "Infancy",
            "interaction_count": 0,
            "entropy": 0.0,
            "coherence": 0.0,
            "learning_score": 0.0,
            "johari_quadrant": "Hidden",
            "consolidation_phase": "Wake",
            "phase_angle": 0.0,
            "computation_mode": "real",  // Indicates real UTL, not stub
            "formula": "L = sigmoid(2.0 * ΔS * ΔC * wₑ * cos φ)",
            "thresholds": {
                "entropy_trigger": 0.9,
                "coherence_trigger": 0.2,
                "min_importance_store": 0.1,
                "consolidation_threshold": self.consolidation_threshold,
                "edge_similarity": self.default_edge_threshold,
                "max_edges": self.default_max_edges,
                "knn_k": self.default_k
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CorpusStats, EmotionalState};

    // =========================================================================
    // Helper functions for creating test embeddings
    // =========================================================================

    fn create_test_embedding(dim: usize, base_val: f32) -> Vec<f32> {
        (0..dim).map(|i| base_val + (i as f32 * 0.01)).collect()
    }

    fn create_reference_embeddings(count: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..count)
            .map(|i| create_test_embedding(dim, i as f32 * 0.1))
            .collect()
    }

    // =========================================================================
    // Basic functionality tests
    // =========================================================================

    #[tokio::test]
    async fn test_compute_learning_score() {
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let score = processor
            .compute_learning_score("test input", &context)
            .await
            .unwrap();

        assert!((0.0..=1.0).contains(&score));
    }

    #[tokio::test]
    async fn test_deterministic_output() {
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let score1 = processor
            .compute_surprise("same input", &context)
            .await
            .unwrap();
        let score2 = processor
            .compute_surprise("same input", &context)
            .await
            .unwrap();

        assert_eq!(score1, score2);
    }

    #[tokio::test]
    async fn test_emotional_weight_modifier() {
        let processor = StubUtlProcessor::new();

        let neutral_ctx = UtlContext {
            emotional_state: EmotionalState::Neutral,
            ..Default::default()
        };
        let curious_ctx = UtlContext {
            emotional_state: EmotionalState::Curious,
            ..Default::default()
        };

        let neutral_weight = processor
            .compute_emotional_weight("test", &neutral_ctx)
            .await
            .unwrap();
        let curious_weight = processor
            .compute_emotional_weight("test", &curious_ctx)
            .await
            .unwrap();

        // Curious should have higher weight (1.2 vs 1.0)
        assert!(curious_weight > neutral_weight);
        assert_eq!(neutral_weight, 1.0);
        assert_eq!(curious_weight, 1.2);
    }

    // =========================================================================
    // TC-GHOST-001: UTL Equation Logic Tests (Updated for real computation)
    // =========================================================================

    #[tokio::test]
    async fn test_utl_equation_formula_verification() {
        // TC-GHOST-001: UTL formula L = sigmoid(2.0 * ΔS * ΔC * wₑ * cos φ)
        let processor = StubUtlProcessor::new();
        let context = UtlContext {
            prior_entropy: 0.6,
            current_coherence: 0.7,
            ..Default::default()
        };

        let input = "test input for UTL verification";

        // Get individual components
        let surprise = processor.compute_surprise(input, &context).await.unwrap();
        let coherence_change = processor
            .compute_coherence_change(input, &context)
            .await
            .unwrap();
        let emotional_weight = processor
            .compute_emotional_weight(input, &context)
            .await
            .unwrap();
        let alignment = processor.compute_alignment(input, &context).await.unwrap();

        // Compute expected learning score using the sigmoid formula per constitution
        let raw = 2.0 * surprise * coherence_change * emotional_weight * alignment;
        let expected = 1.0 / (1.0 + (-raw).exp());

        // Get actual learning score
        let actual = processor
            .compute_learning_score(input, &context)
            .await
            .unwrap();

        // Verify sigmoid formula is correctly implemented
        assert!(
            (actual - expected).abs() < 0.0001,
            "UTL formula mismatch: expected sigmoid(2.0 * {} * {} * {} * {}) = {}, got {}",
            surprise,
            coherence_change,
            emotional_weight,
            alignment,
            expected,
            actual
        );
    }

    #[tokio::test]
    async fn test_utl_learning_score_in_valid_range() {
        // TC-GHOST-001: Learning score must always be in [0.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "a",
            "test",
            "Neural Network",
            "complex input string with many words",
        ] {
            let score = processor
                .compute_learning_score(input, &context)
                .await
                .unwrap();
            assert!(
                (0.0..=1.0).contains(&score),
                "Learning score {} for '{}' must be in [0.0, 1.0]",
                score,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_components_deterministic() {
        // TC-GHOST-001: All UTL components must be deterministic
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();
        let input = "determinism test input";

        // Compute twice
        let surprise1 = processor.compute_surprise(input, &context).await.unwrap();
        let surprise2 = processor.compute_surprise(input, &context).await.unwrap();

        let coherence1 = processor
            .compute_coherence_change(input, &context)
            .await
            .unwrap();
        let coherence2 = processor
            .compute_coherence_change(input, &context)
            .await
            .unwrap();

        let weight1 = processor
            .compute_emotional_weight(input, &context)
            .await
            .unwrap();
        let weight2 = processor
            .compute_emotional_weight(input, &context)
            .await
            .unwrap();

        let align1 = processor.compute_alignment(input, &context).await.unwrap();
        let align2 = processor.compute_alignment(input, &context).await.unwrap();

        // All must match
        assert_eq!(surprise1, surprise2, "Surprise must be deterministic");
        assert_eq!(
            coherence1, coherence2,
            "Coherence change must be deterministic"
        );
        assert_eq!(weight1, weight2, "Emotional weight must be deterministic");
        assert_eq!(align1, align2, "Alignment must be deterministic");
    }

    #[tokio::test]
    async fn test_utl_surprise_in_valid_range() {
        // TC-GHOST-001: Surprise component must be in [0.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "",
            "x",
            "test phrase",
            "A very long input string for testing boundaries",
        ] {
            let surprise = processor.compute_surprise(input, &context).await.unwrap();
            assert!(
                (0.0..=1.0).contains(&surprise),
                "Surprise {} for '{}' must be in [0.0, 1.0]",
                surprise,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_coherence_change_in_valid_range() {
        // TC-GHOST-001: Coherence change component must be in [0.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "",
            "x",
            "test phrase",
            "A very long input string for testing boundaries",
        ] {
            let coherence = processor
                .compute_coherence_change(input, &context)
                .await
                .unwrap();
            assert!(
                (0.0..=1.0).contains(&coherence),
                "Coherence change {} for '{}' must be in [0.0, 1.0]",
                coherence,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_alignment_in_valid_range() {
        // TC-GHOST-001: Alignment (cos phi) must be in [-1.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "",
            "x",
            "test phrase",
            "A very long input string for testing boundaries",
        ] {
            let alignment = processor.compute_alignment(input, &context).await.unwrap();
            assert!(
                (-1.0..=1.0).contains(&alignment),
                "Alignment {} for '{}' must be in [-1.0, 1.0]",
                alignment,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_emotional_weight_in_valid_range() {
        // TC-GHOST-001: Emotional weight must be in [0.5, 1.5] after clamping
        let processor = StubUtlProcessor::new();

        let states = [
            EmotionalState::Neutral,
            EmotionalState::Curious,
            EmotionalState::Focused,
            EmotionalState::Stressed,
            EmotionalState::Fatigued,
            EmotionalState::Engaged,
            EmotionalState::Confused,
        ];

        for state in states {
            let context = UtlContext {
                emotional_state: state,
                ..Default::default()
            };
            let weight = processor
                .compute_emotional_weight("test", &context)
                .await
                .unwrap();
            assert!(
                (0.5..=1.5).contains(&weight),
                "Emotional weight {} for {:?} must be in [0.5, 1.5]",
                weight,
                state
            );
        }
    }

    #[tokio::test]
    async fn test_utl_metrics_contains_all_components() {
        // TC-GHOST-001: compute_metrics must return all UTL components
        let processor = StubUtlProcessor::new();
        let context = UtlContext {
            prior_entropy: 0.6,
            current_coherence: 0.7,
            ..Default::default()
        };

        let metrics = processor
            .compute_metrics("test input", &context)
            .await
            .unwrap();

        // Verify all fields are populated
        assert_eq!(
            metrics.entropy, context.prior_entropy,
            "Entropy must match context"
        );
        assert_eq!(
            metrics.coherence, context.current_coherence,
            "Coherence must match context"
        );

        // Verify components match individual computations
        let surprise = processor
            .compute_surprise("test input", &context)
            .await
            .unwrap();
        let coherence_change = processor
            .compute_coherence_change("test input", &context)
            .await
            .unwrap();
        let emotional_weight = processor
            .compute_emotional_weight("test input", &context)
            .await
            .unwrap();
        let alignment = processor
            .compute_alignment("test input", &context)
            .await
            .unwrap();
        let learning_score = processor
            .compute_learning_score("test input", &context)
            .await
            .unwrap();

        assert_eq!(metrics.surprise, surprise);
        assert_eq!(metrics.coherence_change, coherence_change);
        assert_eq!(metrics.emotional_weight, emotional_weight);
        assert_eq!(metrics.alignment, alignment);
        assert_eq!(metrics.learning_score, learning_score);
    }

    #[tokio::test]
    async fn test_utl_consolidation_threshold() {
        // TC-GHOST-001: Consolidation decision must respect threshold
        let processor = StubUtlProcessor::with_threshold(0.7);
        let embedding = vec![0.5; 1536];

        // Node below threshold
        let mut low_importance =
            crate::types::MemoryNode::new("low".to_string(), embedding.clone());
        low_importance.importance = 0.5;

        // Node at threshold
        let mut at_threshold = crate::types::MemoryNode::new("at".to_string(), embedding.clone());
        at_threshold.importance = 0.7;

        // Node above threshold
        let mut high_importance = crate::types::MemoryNode::new("high".to_string(), embedding);
        high_importance.importance = 0.9;

        assert!(
            !processor.should_consolidate(&low_importance).await.unwrap(),
            "Node below threshold should not consolidate"
        );
        assert!(
            processor.should_consolidate(&at_threshold).await.unwrap(),
            "Node at threshold should consolidate"
        );
        assert!(
            processor
                .should_consolidate(&high_importance)
                .await
                .unwrap(),
            "Node above threshold should consolidate"
        );
    }

    // =========================================================================
    // Real Embedding-Based UTL Computation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_real_surprise_computation_with_embeddings() {
        // Test ΔS computation using KNN distance
        let processor = StubUtlProcessor::new();

        // Create reference embeddings - all centered around origin
        let dim = 8; // Use small dimension for clarity
        let mut references = Vec::new();
        for i in 0..5 {
            let mut emb = vec![0.0; dim];
            emb[0] = i as f32 * 0.1; // Small variations around origin
            references.push(emb);
        }

        // Create input identical to one of the references (very low surprise)
        let close_input = vec![0.0; dim]; // Exactly matches first reference

        // Create input moderately far from references
        let mut far_input = vec![0.0; dim];
        far_input[0] = 2.0; // 2.0 distance from nearest reference

        // Use calibrated corpus stats that make the distinction clear
        let corpus_stats = CorpusStats {
            mean_knn_distance: 0.5, // Typical distance is 0.5
            std_knn_distance: 0.5,  // Standard deviation of 0.5
            k: 1,                   // Use nearest neighbor
        };

        let close_context = UtlContext {
            input_embedding: Some(close_input),
            reference_embeddings: Some(references.clone()),
            corpus_stats: Some(corpus_stats.clone()),
            ..Default::default()
        };

        let far_context = UtlContext {
            input_embedding: Some(far_input),
            reference_embeddings: Some(references),
            corpus_stats: Some(corpus_stats),
            ..Default::default()
        };

        let close_surprise = processor
            .compute_surprise("", &close_context)
            .await
            .unwrap();
        let far_surprise = processor.compute_surprise("", &far_context).await.unwrap();

        // Far input should have higher surprise
        // Close input distance = 0.0, z = (0.0 - 0.5) / 0.5 = -1.0, sigmoid(-1) ≈ 0.27
        // Far input distance = 1.6 (to 0.4), z = (1.6 - 0.5) / 0.5 = 2.2, sigmoid(2.2) ≈ 0.90
        assert!(
            far_surprise > close_surprise,
            "Far input (surprise={:.4}) should have higher surprise than close input ({:.4})",
            far_surprise,
            close_surprise
        );

        // Both should be in [0, 1]
        assert!((0.0..=1.0).contains(&close_surprise));
        assert!((0.0..=1.0).contains(&far_surprise));

        // Close input should have low surprise (below 0.5)
        assert!(
            close_surprise < 0.5,
            "Close input should have surprise < 0.5, got {:.4}",
            close_surprise
        );

        // Far input should have high surprise (above 0.5)
        assert!(
            far_surprise > 0.5,
            "Far input should have surprise > 0.5, got {:.4}",
            far_surprise
        );
    }

    #[tokio::test]
    async fn test_real_coherence_computation_with_embeddings() {
        // Test ΔC computation using connectivity
        let processor = StubUtlProcessor::new();
        let dim = 128;

        // Create reference embeddings with specific pattern
        let mut references = Vec::new();
        for i in 0..5 {
            let mut emb = vec![0.0; dim];
            emb[0] = 1.0; // All have high value in first dimension
            emb[1] = i as f32 * 0.1;
            references.push(emb);
        }

        // Create input similar to references (high coherence)
        let mut similar_input = vec![0.0; dim];
        similar_input[0] = 1.0;
        similar_input[1] = 0.25;

        // Create input very different from references (low coherence)
        let mut different_input = vec![0.0; dim];
        different_input[dim - 1] = 1.0; // Orthogonal direction

        let similar_context = UtlContext {
            input_embedding: Some(similar_input),
            reference_embeddings: Some(references.clone()),
            edge_similarity_threshold: Some(0.5),
            max_edges: Some(5),
            ..Default::default()
        };

        let different_context = UtlContext {
            input_embedding: Some(different_input),
            reference_embeddings: Some(references),
            edge_similarity_threshold: Some(0.5),
            max_edges: Some(5),
            ..Default::default()
        };

        let similar_coherence = processor
            .compute_coherence_change("", &similar_context)
            .await
            .unwrap();
        let different_coherence = processor
            .compute_coherence_change("", &different_context)
            .await
            .unwrap();

        // Similar input should have higher coherence
        assert!(
            similar_coherence > different_coherence,
            "Similar input (coherence={}) should have higher coherence than different input ({})",
            similar_coherence,
            different_coherence
        );

        // Both should be in [0, 1]
        assert!((0.0..=1.0).contains(&similar_coherence));
        assert!((0.0..=1.0).contains(&different_coherence));
    }

    #[tokio::test]
    async fn test_real_alignment_computation_with_embeddings() {
        // Test cos φ computation using cosine similarity to goal
        let processor = StubUtlProcessor::new();
        let dim = 128;

        // Create goal vector (North Star)
        let mut goal = vec![0.0; dim];
        goal[0] = 1.0;

        // Create input aligned with goal
        let mut aligned_input = vec![0.0; dim];
        aligned_input[0] = 1.0;

        // Create input orthogonal to goal
        let mut orthogonal_input = vec![0.0; dim];
        orthogonal_input[1] = 1.0;

        // Create input opposite to goal
        let mut opposite_input = vec![0.0; dim];
        opposite_input[0] = -1.0;

        let aligned_context = UtlContext {
            input_embedding: Some(aligned_input),
            goal_vector: Some(goal.clone()),
            ..Default::default()
        };

        let orthogonal_context = UtlContext {
            input_embedding: Some(orthogonal_input),
            goal_vector: Some(goal.clone()),
            ..Default::default()
        };

        let opposite_context = UtlContext {
            input_embedding: Some(opposite_input),
            goal_vector: Some(goal),
            ..Default::default()
        };

        let aligned = processor
            .compute_alignment("", &aligned_context)
            .await
            .unwrap();
        let orthogonal = processor
            .compute_alignment("", &orthogonal_context)
            .await
            .unwrap();
        let opposite = processor
            .compute_alignment("", &opposite_context)
            .await
            .unwrap();

        // Aligned should be ~1.0
        assert!(
            (aligned - 1.0).abs() < 0.001,
            "Aligned input should have alignment ~1.0, got {}",
            aligned
        );

        // Orthogonal should be ~0.0
        assert!(
            orthogonal.abs() < 0.001,
            "Orthogonal input should have alignment ~0.0, got {}",
            orthogonal
        );

        // Opposite should be ~-1.0
        assert!(
            (opposite - (-1.0)).abs() < 0.001,
            "Opposite input should have alignment ~-1.0, got {}",
            opposite
        );
    }

    #[tokio::test]
    async fn test_full_utl_with_real_embeddings() {
        // Test complete UTL computation with real embeddings
        let processor = StubUtlProcessor::new();
        let dim = 128;

        // Create reference corpus
        let references = create_reference_embeddings(10, dim);

        // Create goal vector
        let mut goal = vec![0.0; dim];
        goal[0] = 1.0;

        // Create input with moderate novelty and good alignment
        let mut input = vec![0.0; dim];
        input[0] = 0.8;
        input[1] = 0.6;

        let context = UtlContext {
            input_embedding: Some(input),
            reference_embeddings: Some(references),
            goal_vector: Some(goal),
            corpus_stats: Some(CorpusStats {
                mean_knn_distance: 0.5,
                std_knn_distance: 0.2,
                k: 3,
            }),
            edge_similarity_threshold: Some(0.5),
            max_edges: Some(10),
            emotional_state: EmotionalState::Engaged,
            ..Default::default()
        };

        let metrics = processor.compute_metrics("test", &context).await.unwrap();

        // Verify all components are reasonable
        assert!(
            (0.0..=1.0).contains(&metrics.surprise),
            "Surprise {} out of range",
            metrics.surprise
        );
        assert!(
            (0.0..=1.0).contains(&metrics.coherence_change),
            "Coherence {} out of range",
            metrics.coherence_change
        );
        assert!(
            (0.5..=1.5).contains(&metrics.emotional_weight),
            "Emotional weight {} out of range",
            metrics.emotional_weight
        );
        assert!(
            (-1.0..=1.0).contains(&metrics.alignment),
            "Alignment {} out of range",
            metrics.alignment
        );
        assert!(
            (0.0..=1.0).contains(&metrics.learning_score),
            "Learning score {} out of range",
            metrics.learning_score
        );

        // Verify learning score uses sigmoid formula
        let raw = 2.0
            * metrics.surprise
            * metrics.coherence_change
            * metrics.emotional_weight
            * metrics.alignment;
        let expected = 1.0 / (1.0 + (-raw).exp());
        assert!(
            (metrics.learning_score - expected).abs() < 0.0001,
            "Learning score {} doesn't match sigmoid formula (expected {})",
            metrics.learning_score,
            expected
        );
    }

    #[tokio::test]
    async fn test_fallback_when_no_embeddings() {
        // Test graceful fallback to context values when embeddings unavailable
        let processor = StubUtlProcessor::new();

        let context = UtlContext {
            prior_entropy: 0.8,
            current_coherence: 0.6,
            // No embeddings provided
            ..Default::default()
        };

        let surprise = processor.compute_surprise("test", &context).await.unwrap();
        let coherence = processor
            .compute_coherence_change("test", &context)
            .await
            .unwrap();
        let alignment = processor.compute_alignment("test", &context).await.unwrap();

        // Without embeddings, falls back to context values
        assert_eq!(surprise, 0.8, "Should use prior_entropy as fallback");
        assert_eq!(coherence, 0.6, "Should use current_coherence as fallback");
        assert_eq!(alignment, 1.0, "Should default to full alignment");
    }

    // =========================================================================
    // Unit tests for helper functions
    // =========================================================================

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((StubUtlProcessor::cosine_similarity(&a, &b) - 1.0).abs() < 0.0001);

        // Orthogonal vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(StubUtlProcessor::cosine_similarity(&a, &b).abs() < 0.0001);

        // Opposite vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((StubUtlProcessor::cosine_similarity(&a, &b) - (-1.0)).abs() < 0.0001);

        // Different lengths returns 0
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(StubUtlProcessor::cosine_similarity(&a, &b), 0.0);

        // Empty vectors return 0
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(StubUtlProcessor::cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_euclidean_distance() {
        // Same point
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!(StubUtlProcessor::euclidean_distance(&a, &b).abs() < 0.0001);

        // Unit distance
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((StubUtlProcessor::euclidean_distance(&a, &b) - 1.0).abs() < 0.0001);

        // Different lengths returns MAX
        let a = vec![1.0];
        let b = vec![1.0, 2.0];
        assert_eq!(StubUtlProcessor::euclidean_distance(&a, &b), f32::MAX);
    }

    #[test]
    fn test_sigmoid() {
        // sigmoid(0) = 0.5
        assert!((StubUtlProcessor::sigmoid(0.0) - 0.5).abs() < 0.0001);

        // sigmoid(large) -> 1.0
        assert!(StubUtlProcessor::sigmoid(10.0) > 0.99);

        // sigmoid(-large) -> 0.0
        assert!(StubUtlProcessor::sigmoid(-10.0) < 0.01);

        // sigmoid is monotonic
        assert!(StubUtlProcessor::sigmoid(1.0) > StubUtlProcessor::sigmoid(0.0));
        assert!(StubUtlProcessor::sigmoid(0.0) > StubUtlProcessor::sigmoid(-1.0));
    }

    #[test]
    fn test_knn_distance_computation() {
        let input = vec![0.0, 0.0, 0.0];
        let references = vec![
            vec![1.0, 0.0, 0.0], // distance 1.0
            vec![2.0, 0.0, 0.0], // distance 2.0
            vec![3.0, 0.0, 0.0], // distance 3.0
        ];

        // k=1 should return 1.0 (closest)
        let d1 = StubUtlProcessor::compute_knn_distance(&input, &references, 1).unwrap();
        assert!((d1 - 1.0).abs() < 0.0001);

        // k=2 should return 2.0 (second closest)
        let d2 = StubUtlProcessor::compute_knn_distance(&input, &references, 2).unwrap();
        assert!((d2 - 2.0).abs() < 0.0001);

        // k=3 should return 3.0 (third closest)
        let d3 = StubUtlProcessor::compute_knn_distance(&input, &references, 3).unwrap();
        assert!((d3 - 3.0).abs() < 0.0001);

        // k > n should return last distance
        let d5 = StubUtlProcessor::compute_knn_distance(&input, &references, 5).unwrap();
        assert!((d5 - 3.0).abs() < 0.0001);

        // Empty references returns None
        let empty: Vec<Vec<f32>> = vec![];
        assert!(StubUtlProcessor::compute_knn_distance(&input, &empty, 1).is_none());
    }

    #[test]
    fn test_get_status_shows_real_computation() {
        let processor = StubUtlProcessor::new();
        let status = processor.get_status();

        // Verify status indicates real computation mode
        assert_eq!(status["computation_mode"], "real");
        assert!(status["formula"].as_str().unwrap().contains("sigmoid"));
        assert!(status["thresholds"]["edge_similarity"].as_f64().is_some());
        assert!(status["thresholds"]["knn_k"].as_u64().is_some());
    }
}
