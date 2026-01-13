//! Configuration types for token pruning of E12 (ColBERT) embeddings.
//!
//! Token pruning reduces the number of per-token embeddings while maintaining
//! semantic recall quality. Constitution target: ~50% compression with <15% info loss.
//!
//! # Constitution References
//!
//! - embeddings.models.E12_LateInteraction: "128D/tok, dense_per_token"
//! - perf.quality.info_loss: "<15%"
//! - rules: "Result<T,E>, thiserror derivation"
//! - rules: "Never unwrap() in prod"

use crate::error::{EmbeddingError, EmbeddingResult};

/// Configuration for token pruning of E12 (ColBERT) embeddings.
///
/// Token pruning reduces the number of per-token embeddings while
/// maintaining semantic recall quality. Constitution target: ~50% compression.
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::pruning::TokenPruningConfig;
///
/// // Default config
/// let config = TokenPruningConfig::default();
/// assert!(config.validate().is_ok());
/// assert_eq!(config.target_compression, 0.5);
/// assert_eq!(config.min_tokens, 64);
///
/// // Custom compression ratio
/// let config = TokenPruningConfig::with_compression(0.7).unwrap();
/// assert_eq!(config.target_compression, 0.7);
/// ```
#[derive(Debug, Clone)]
pub struct TokenPruningConfig {
    /// Target compression ratio (default: 0.5 = 50% compression)
    /// Range: (0.0, 1.0) exclusive - 0.0 means no compression, 1.0 means remove all
    pub target_compression: f32,

    /// Minimum tokens to retain (default: 64)
    /// Prevents over-pruning short sequences
    pub min_tokens: usize,

    /// Importance scoring method for ranking tokens
    pub scoring_method: ImportanceScoringMethod,
}

impl Default for TokenPruningConfig {
    fn default() -> Self {
        Self {
            target_compression: 0.5,
            min_tokens: 64,
            scoring_method: ImportanceScoringMethod::AttentionBased,
        }
    }
}

impl TokenPruningConfig {
    /// Validate configuration.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if:
    /// - target_compression not in (0.0, 1.0) exclusive
    /// - min_tokens is 0
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::pruning::TokenPruningConfig;
    ///
    /// // Valid config
    /// let config = TokenPruningConfig::default();
    /// assert!(config.validate().is_ok());
    ///
    /// // Invalid: compression at boundary
    /// let config = TokenPruningConfig {
    ///     target_compression: 0.0,
    ///     ..Default::default()
    /// };
    /// assert!(config.validate().is_err());
    /// ```
    pub fn validate(&self) -> EmbeddingResult<()> {
        if self.target_compression <= 0.0 || self.target_compression >= 1.0 {
            return Err(EmbeddingError::ConfigError {
                message: format!(
                    "target_compression must be in (0.0, 1.0), got {}",
                    self.target_compression
                ),
            });
        }
        if self.min_tokens == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "min_tokens must be at least 1".to_string(),
            });
        }
        Ok(())
    }

    /// Create a new config with custom compression ratio.
    /// Validates immediately - fails fast if invalid.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if compression ratio is not in (0.0, 1.0).
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::pruning::TokenPruningConfig;
    ///
    /// // Valid compression ratio
    /// let config = TokenPruningConfig::with_compression(0.7).unwrap();
    /// assert_eq!(config.target_compression, 0.7);
    ///
    /// // Invalid compression ratio
    /// assert!(TokenPruningConfig::with_compression(0.0).is_err());
    /// assert!(TokenPruningConfig::with_compression(1.0).is_err());
    /// ```
    pub fn with_compression(target_compression: f32) -> EmbeddingResult<Self> {
        let config = Self {
            target_compression,
            ..Default::default()
        };
        config.validate()?;
        Ok(config)
    }
}

/// Method for scoring token importance during pruning.
///
/// Different methods have different accuracy/performance tradeoffs:
/// - AttentionBased: Best accuracy, requires attention weights
/// - EmbeddingMagnitude: Fast, no additional data needed
/// - Entropy: Good for diverse token selection
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::pruning::ImportanceScoringMethod;
///
/// // Default is AttentionBased
/// assert_eq!(
///     ImportanceScoringMethod::default(),
///     ImportanceScoringMethod::AttentionBased
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportanceScoringMethod {
    /// Use attention weights from transformer layers (most accurate)
    #[default]
    AttentionBased,

    /// Use L2 norm of token embeddings (fastest, no extra data)
    EmbeddingMagnitude,

    /// Use entropy of token probability distribution (moderate)
    Entropy,
}

/// Result of token pruning operation.
///
/// Contains the pruned embeddings and metadata about what was retained.
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::pruning::PrunedEmbeddings;
///
/// let pruned = PrunedEmbeddings {
///     embeddings: vec![vec![0.0; 128]; 10],
///     retained_indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
///     compression_ratio: 0.5,
/// };
/// assert_eq!(pruned.token_count(), 10);
/// assert_eq!(pruned.memory_bytes(), 5120); // 10 * 128 * 4
/// ```
#[derive(Debug, Clone)]
pub struct PrunedEmbeddings {
    /// Pruned token embeddings - each inner Vec is 128D
    /// Length: retained_indices.len()
    pub embeddings: Vec<Vec<f32>>,

    /// Indices of retained tokens in original sequence
    /// Sorted in ascending order to preserve positional information
    pub retained_indices: Vec<usize>,

    /// Achieved compression ratio [0, 1]
    /// 0.0 = no compression, 1.0 = all tokens removed (impossible)
    pub compression_ratio: f32,
}

impl PrunedEmbeddings {
    /// Number of tokens after pruning.
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.embeddings.len()
    }

    /// Memory size in bytes (for monitoring).
    ///
    /// Calculates based on 128D vectors with f32 elements.
    #[must_use]
    pub fn memory_bytes(&self) -> usize {
        self.embeddings.len() * 128 * std::mem::size_of::<f32>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === TokenPruningConfig Tests ===

    #[test]
    fn test_default_config_is_valid() {
        let config = TokenPruningConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.target_compression, 0.5);
        assert_eq!(config.min_tokens, 64);
        assert_eq!(config.scoring_method, ImportanceScoringMethod::AttentionBased);
    }

    #[test]
    fn test_compression_at_zero_fails() {
        let config = TokenPruningConfig {
            target_compression: 0.0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn test_compression_at_one_fails() {
        let config = TokenPruningConfig {
            target_compression: 1.0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn test_compression_negative_fails() {
        let config = TokenPruningConfig {
            target_compression: -0.1,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn test_compression_above_one_fails() {
        let config = TokenPruningConfig {
            target_compression: 1.5,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn test_min_tokens_zero_fails() {
        let config = TokenPruningConfig {
            min_tokens: 0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn test_valid_edge_cases() {
        // Just above 0.0
        let config = TokenPruningConfig {
            target_compression: 0.001,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // Just below 1.0
        let config = TokenPruningConfig {
            target_compression: 0.999,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // Min tokens = 1
        let config = TokenPruningConfig {
            min_tokens: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_with_compression_valid() {
        let config = TokenPruningConfig::with_compression(0.7).unwrap();
        assert_eq!(config.target_compression, 0.7);
    }

    #[test]
    fn test_with_compression_invalid() {
        assert!(TokenPruningConfig::with_compression(0.0).is_err());
        assert!(TokenPruningConfig::with_compression(1.0).is_err());
        assert!(TokenPruningConfig::with_compression(-0.5).is_err());
    }

    // === ImportanceScoringMethod Tests ===

    #[test]
    fn test_scoring_method_default() {
        assert_eq!(
            ImportanceScoringMethod::default(),
            ImportanceScoringMethod::AttentionBased
        );
    }

    #[test]
    fn test_scoring_method_equality() {
        assert_eq!(
            ImportanceScoringMethod::AttentionBased,
            ImportanceScoringMethod::AttentionBased
        );
        assert_ne!(
            ImportanceScoringMethod::AttentionBased,
            ImportanceScoringMethod::EmbeddingMagnitude
        );
    }

    // === PrunedEmbeddings Tests ===

    #[test]
    fn test_pruned_embeddings_token_count() {
        let pruned = PrunedEmbeddings {
            embeddings: vec![vec![0.0; 128]; 10],
            retained_indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            compression_ratio: 0.5,
        };
        assert_eq!(pruned.token_count(), 10);
    }

    #[test]
    fn test_pruned_embeddings_memory_bytes() {
        let pruned = PrunedEmbeddings {
            embeddings: vec![vec![0.0; 128]; 10],
            retained_indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            compression_ratio: 0.5,
        };
        // 10 tokens * 128 dims * 4 bytes = 5120 bytes
        assert_eq!(pruned.memory_bytes(), 5120);
    }

    #[test]
    fn test_pruned_embeddings_empty() {
        let pruned = PrunedEmbeddings {
            embeddings: vec![],
            retained_indices: vec![],
            compression_ratio: 1.0,
        };
        assert_eq!(pruned.token_count(), 0);
        assert_eq!(pruned.memory_bytes(), 0);
    }
}
