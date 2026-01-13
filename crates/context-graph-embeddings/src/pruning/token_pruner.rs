//! Token pruning quantizer for E12 (ColBERT) late-interaction embeddings.
//!
//! This module implements the `TokenPruningQuantizer` which reduces embedding size
//! by ~50% while maintaining semantic recall quality.
//!
//! # Constitution References
//!
//! - embeddings.models.E12_LateInteraction: "128D/tok, dense_per_token"
//! - perf.quality.info_loss: "<15%"
//! - rules: "Result<T,E>, thiserror derivation"
//! - rules: "Never unwrap() in prod"

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::pruning::config::{ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig};

/// E12 Late Interaction embedding dimension per token.
const LATE_INTERACTION_DIMENSION: usize = 128;

/// Token pruning quantizer for E12 Late Interaction embeddings.
///
/// This quantizer reduces the number of per-token embeddings by selecting
/// only the most important tokens based on a configurable scoring method.
///
/// # Constitution Reference
///
/// embeddings.models.E12_LateInteraction = "128D/tok"
/// Target: ~50% compression (512 -> ~256 tokens)
/// Constraint: Recall@10 degradation < 5%
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::pruning::{TokenPruningQuantizer, TokenPruningConfig, ImportanceScoringMethod};
///
/// let config = TokenPruningConfig {
///     target_compression: 0.5,
///     min_tokens: 10,
///     scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
/// };
///
/// let quantizer = TokenPruningQuantizer::new(config).unwrap();
///
/// // Create sample embeddings (100 tokens, 128D each)
/// let embeddings: Vec<Vec<f32>> = (0..100)
///     .map(|i| vec![(i as f32) * 0.1; 128])
///     .collect();
///
/// let result = quantizer.prune(&embeddings, None).unwrap();
/// assert_eq!(result.embeddings.len(), 50); // 50% retained
/// ```
pub struct TokenPruningQuantizer {
    config: TokenPruningConfig,
}

impl TokenPruningQuantizer {
    /// Create a new token pruning quantizer.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration specifying compression ratio, min tokens, and scoring method
    ///
    /// # Errors
    ///
    /// Returns `EmbeddingError::ConfigError` if config validation fails:
    /// - `target_compression` not in (0.0, 1.0) exclusive
    /// - `min_tokens` is 0
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::pruning::{TokenPruningQuantizer, TokenPruningConfig};
    ///
    /// // Valid config
    /// let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default());
    /// assert!(quantizer.is_ok());
    ///
    /// // Invalid config
    /// let bad_config = TokenPruningConfig {
    ///     target_compression: 1.5, // Invalid: > 1.0
    ///     ..Default::default()
    /// };
    /// assert!(TokenPruningQuantizer::new(bad_config).is_err());
    /// ```
    pub fn new(config: TokenPruningConfig) -> EmbeddingResult<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Prune low-importance tokens from E12 embeddings.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Token embeddings, shape [num_tokens, 128]
    /// * `attention_weights` - Optional attention weights for importance scoring
    ///
    /// # Returns
    ///
    /// Pruned embeddings with retained token indices.
    ///
    /// # Guarantees
    ///
    /// - Output has at least `min_tokens` tokens
    /// - `retained_indices` is sorted in ascending order
    /// - Compression ratio approximately matches `target_compression`
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::EmptyInput` if embeddings is empty
    /// - `EmbeddingError::InvalidDimension` if any embedding is not 128D
    ///
    /// # Algorithm
    ///
    /// 1. Validate input (non-empty, correct dimension)
    /// 2. Calculate target token count based on compression ratio
    /// 3. Enforce min_tokens constraint
    /// 4. Score all tokens using configured method
    /// 5. Select top-K tokens by importance
    /// 6. Sort retained indices to preserve positional order
    /// 7. Extract embeddings for retained tokens
    pub fn prune(
        &self,
        embeddings: &[Vec<f32>],
        attention_weights: Option<&[f32]>,
    ) -> EmbeddingResult<PrunedEmbeddings> {
        // Step 1: Validate input
        if embeddings.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        for embedding in embeddings.iter() {
            if embedding.len() != LATE_INTERACTION_DIMENSION {
                return Err(EmbeddingError::InvalidDimension {
                    expected: LATE_INTERACTION_DIMENSION,
                    actual: embedding.len(),
                });
            }
        }

        let num_tokens = embeddings.len();

        // Step 2: Calculate target token count
        // target_compression = 0.5 means remove 50%, so keep 50%
        let retention_ratio = 1.0 - self.config.target_compression;
        let mut target_count = (retention_ratio * num_tokens as f32).floor() as usize;

        // Step 3: Enforce min_tokens constraint (can't retain more than we have)
        target_count = target_count.max(self.config.min_tokens);
        target_count = target_count.min(num_tokens);

        // Step 4: Check if pruning is needed
        if target_count >= num_tokens {
            // No pruning needed - return all embeddings unchanged
            return Ok(PrunedEmbeddings {
                embeddings: embeddings.to_vec(),
                retained_indices: (0..num_tokens).collect(),
                compression_ratio: 0.0,
            });
        }

        // Step 5: Score all tokens
        let scores = self.score_tokens(embeddings, attention_weights);

        // Step 6: Rank tokens by importance (descending)
        let mut indexed_scores: Vec<(usize, f32)> =
            scores.into_iter().enumerate().collect();
        indexed_scores.sort_by(|a, b| {
            // Sort by score descending (highest first)
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 7: Select top-K tokens
        let top_k_indices: Vec<usize> = indexed_scores
            .iter()
            .take(target_count)
            .map(|(idx, _)| *idx)
            .collect();

        // Step 8: Sort retained indices to preserve positional order
        let mut retained_indices = top_k_indices;
        retained_indices.sort();

        // Step 9: Extract pruned embeddings
        let pruned_embeddings: Vec<Vec<f32>> = retained_indices
            .iter()
            .map(|&idx| embeddings[idx].clone())
            .collect();

        // Step 10: Calculate achieved compression
        let compression_ratio = 1.0 - (retained_indices.len() as f32 / num_tokens as f32);

        Ok(PrunedEmbeddings {
            embeddings: pruned_embeddings,
            retained_indices,
            compression_ratio,
        })
    }

    /// Score tokens by their importance using the configured method.
    fn score_tokens(
        &self,
        embeddings: &[Vec<f32>],
        attention_weights: Option<&[f32]>,
    ) -> Vec<f32> {
        match self.config.scoring_method {
            ImportanceScoringMethod::AttentionBased => {
                // Use attention weights if available, otherwise fall back to magnitude
                attention_weights
                    .map(|w| w.to_vec())
                    .unwrap_or_else(|| self.score_by_magnitude(embeddings))
            }
            ImportanceScoringMethod::EmbeddingMagnitude => self.score_by_magnitude(embeddings),
            ImportanceScoringMethod::Entropy => self.score_by_entropy(embeddings),
        }
    }

    /// Score by L2 norm (magnitude) of each token embedding.
    ///
    /// Higher magnitude embeddings tend to be more semantically significant.
    fn score_by_magnitude(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
        embeddings
            .iter()
            .map(|emb| emb.iter().map(|x| x * x).sum::<f32>().sqrt())
            .collect()
    }

    /// Score by entropy of normalized embedding values.
    ///
    /// Higher entropy tokens carry more information and are more important.
    fn score_by_entropy(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
        embeddings
            .iter()
            .map(|emb| {
                let sum: f32 = emb.iter().map(|x| x.abs()).sum();
                if sum == 0.0 {
                    return 0.0;
                }
                let probs: Vec<f32> = emb.iter().map(|x| x.abs() / sum).collect();
                probs
                    .iter()
                    .filter(|&&p| p > 0.0)
                    .map(|&p| -p * p.ln())
                    .sum()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Helper to create synthetic 128D embeddings ===
    fn make_embedding(base: f32, variation: f32) -> Vec<f32> {
        (0..128)
            .map(|i| base + variation * (i as f32 / 128.0))
            .collect()
    }

    fn make_embeddings(count: usize) -> Vec<Vec<f32>> {
        (0..count)
            .map(|i| make_embedding(i as f32 * 0.1, 0.5))
            .collect()
    }

    // === Construction Tests ===

    #[test]
    fn test_new_with_default_config() {
        let config = TokenPruningConfig::default();
        let quantizer = TokenPruningQuantizer::new(config);
        assert!(quantizer.is_ok());
    }

    #[test]
    fn test_new_with_invalid_config_fails() {
        let config = TokenPruningConfig {
            target_compression: 1.5, // Invalid: > 1.0
            ..Default::default()
        };
        let result = TokenPruningQuantizer::new(config);
        assert!(result.is_err());
    }

    // === Empty Input Tests ===

    #[test]
    fn test_prune_empty_input_returns_error() {
        let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
        let result = quantizer.prune(&[], None);
        assert!(matches!(result, Err(EmbeddingError::EmptyInput)));
    }

    // === Invalid Dimension Tests ===

    #[test]
    fn test_prune_wrong_dimension_returns_error() {
        let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
        let bad_embeddings = vec![vec![0.0; 64]]; // 64D instead of 128D
        let result = quantizer.prune(&bad_embeddings, None);
        assert!(matches!(
            result,
            Err(EmbeddingError::InvalidDimension {
                expected: 128,
                actual: 64
            })
        ));
    }

    // === No Pruning Needed Tests ===

    #[test]
    fn test_prune_input_smaller_than_min_tokens_no_pruning() {
        let config = TokenPruningConfig {
            min_tokens: 64,
            target_compression: 0.5,
            ..Default::default()
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Only 32 tokens - less than min_tokens (64)
        let embeddings = make_embeddings(32);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should keep all 32 tokens
        assert_eq!(result.embeddings.len(), 32);
        assert_eq!(result.retained_indices.len(), 32);
        assert_eq!(result.compression_ratio, 0.0); // No compression
    }

    // === Standard Pruning Tests ===

    #[test]
    fn test_prune_50_percent_compression() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 10,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(100);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Target: 50 tokens (100 * 0.5)
        assert_eq!(result.embeddings.len(), 50);
        assert_eq!(result.retained_indices.len(), 50);

        // Compression should be approximately 0.5
        assert!((result.compression_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_retained_indices_are_sorted() {
        let config = TokenPruningConfig {
            target_compression: 0.7, // 70% compression -> keep 30%
            min_tokens: 5,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(50);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Verify indices are sorted ascending
        let mut sorted_indices = result.retained_indices.clone();
        sorted_indices.sort();
        assert_eq!(result.retained_indices, sorted_indices);
    }

    // === Min Tokens Constraint Tests ===

    #[test]
    fn test_min_tokens_respected() {
        let config = TokenPruningConfig {
            target_compression: 0.99, // 99% compression would leave 1 token
            min_tokens: 20,           // But min_tokens forces at least 20
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(100);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should have at least min_tokens (20)
        assert_eq!(result.embeddings.len(), 20);
    }

    // === Scoring Method Tests ===

    #[test]
    fn test_magnitude_scoring_prefers_larger_norms() {
        let config = TokenPruningConfig {
            target_compression: 0.5, // Keep 50%
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Create embeddings with increasing magnitudes
        let embeddings: Vec<Vec<f32>> = (0..10)
            .map(|i| vec![(i as f32 + 1.0) * 0.1; 128])
            .collect();

        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should retain indices 5-9 (highest magnitudes)
        // All retained indices should be >= 5
        for idx in &result.retained_indices {
            assert!(
                *idx >= 5,
                "Expected high-magnitude tokens, got index {}",
                idx
            );
        }
    }

    #[test]
    fn test_attention_based_uses_provided_weights() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::AttentionBased,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(10);

        // Attention weights: first 5 tokens have high scores
        let attention: Vec<f32> = (0..10)
            .map(|i| if i < 5 { 1.0 } else { 0.1 })
            .collect();

        let result = quantizer.prune(&embeddings, Some(&attention)).unwrap();

        // Should retain first 5 tokens (highest attention)
        for idx in &result.retained_indices {
            assert!(
                *idx < 5,
                "Expected high-attention tokens (0-4), got index {}",
                idx
            );
        }
    }

    #[test]
    fn test_entropy_scoring_produces_valid_scores() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::Entropy,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(20);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should retain 10 tokens
        assert_eq!(result.embeddings.len(), 10);
    }

    // === Edge Case Tests ===

    #[test]
    fn test_single_token_input() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            ..Default::default()
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(1);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should keep the single token
        assert_eq!(result.embeddings.len(), 1);
        assert_eq!(result.retained_indices, vec![0]);
        assert_eq!(result.compression_ratio, 0.0);
    }

    #[test]
    fn test_zero_embedding_values() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::Entropy,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // All zeros - entropy scoring should handle gracefully
        let embeddings = vec![vec![0.0; 128]; 10];
        let result = quantizer.prune(&embeddings, None);

        assert!(result.is_ok());
        let pruned = result.unwrap();
        assert_eq!(pruned.embeddings.len(), 5); // 50% of 10
    }

    #[test]
    fn test_embeddings_are_copied_correctly() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Create distinct embeddings
        let embeddings: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; 128]).collect();

        let result = quantizer.prune(&embeddings, None).unwrap();

        // Verify each retained embedding matches the original at that index
        for (pruned_idx, &original_idx) in result.retained_indices.iter().enumerate() {
            assert_eq!(
                result.embeddings[pruned_idx], embeddings[original_idx],
                "Embedding at pruned index {} should match original index {}",
                pruned_idx, original_idx
            );
        }
    }

    // === Additional Edge Case Tests ===

    #[test]
    fn test_attention_fallback_to_magnitude() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::AttentionBased,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Create embeddings with increasing magnitudes
        let embeddings: Vec<Vec<f32>> = (0..10)
            .map(|i| vec![(i as f32 + 1.0) * 0.1; 128])
            .collect();

        // No attention weights provided - should fall back to magnitude
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should retain the 5 highest magnitude tokens (indices 5-9)
        for idx in &result.retained_indices {
            assert!(
                *idx >= 5,
                "Expected high-magnitude tokens when falling back, got index {}",
                idx
            );
        }
    }

    #[test]
    fn test_wrong_dimension_256d() {
        let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
        let bad_embeddings = vec![vec![0.0; 256]]; // 256D instead of 128D
        let result = quantizer.prune(&bad_embeddings, None);
        assert!(matches!(
            result,
            Err(EmbeddingError::InvalidDimension {
                expected: 128,
                actual: 256
            })
        ));
    }
}
