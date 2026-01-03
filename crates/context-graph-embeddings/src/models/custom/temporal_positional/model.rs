//! Core Temporal-Positional embedding model implementation.

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::constants::{DEFAULT_BASE, MAX_BASE, MIN_BASE, TEMPORAL_POSITIONAL_DIMENSION};
use super::encoding::compute_positional_encoding;
use super::timestamp::{extract_timestamp, parse_timestamp};

/// Temporal-Positional embedding model (E4).
///
/// Encodes absolute time positions using transformer-style sinusoidal encoding.
/// This is a custom model with no pretrained weights - it computes embeddings
/// from timestamps using the standard transformer PE formula.
///
/// # Algorithm
///
/// For position pos (Unix timestamp in seconds) and dimension index i:
///   - PE(pos, 2i) = sin(pos / base^(2i/d_model))
///   - PE(pos, 2i+1) = cos(pos / base^(2i/d_model))
///
/// This produces unique encodings for each timestamp that:
///   - Are deterministic for the same timestamp
///   - Can represent relative positions through attention
///   - Scale gracefully for far-future timestamps
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::TemporalPositionalModel;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use context_graph_embeddings::EmbeddingModel; // For is_initialized() trait method
///
/// fn example() -> EmbeddingResult<()> {
///     // Default base (10000.0)
///     let model = TemporalPositionalModel::new();
///     assert!(model.is_initialized());
///     assert_eq!(model.base(), 10000.0);
///
///     // Custom base frequency
///     let model = TemporalPositionalModel::with_base(5000.0)?;
///     assert_eq!(model.base(), 5000.0);
///     Ok(())
/// }
/// ```
pub struct TemporalPositionalModel {
    /// Base frequency for positional encoding (default 10000.0).
    base: f32,

    /// d_model dimension (always 512).
    d_model: usize,

    /// Always true for custom models (no weights to load).
    initialized: AtomicBool,
}

impl TemporalPositionalModel {
    /// Create a new TemporalPositionalModel with default base frequency (10000.0).
    ///
    /// Uses the standard transformer positional encoding base.
    /// Model is immediately ready for use (no loading required).
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DEFAULT_BASE,
            d_model: TEMPORAL_POSITIONAL_DIMENSION,
            initialized: AtomicBool::new(true),
        }
    }

    /// Create a model with custom base frequency.
    ///
    /// # Arguments
    /// * `base` - Base frequency for positional encoding. Must be > 1.0.
    ///   Larger values create slower-varying frequencies.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if base is not in valid range (1.0, 1e10).
    pub fn with_base(base: f32) -> EmbeddingResult<Self> {
        if base <= MIN_BASE || !base.is_finite() || base > MAX_BASE {
            return Err(EmbeddingError::ConfigError {
                message: format!(
                    "TemporalPositionalModel base must be in range ({}, {}], got {}",
                    MIN_BASE, MAX_BASE, base
                ),
            });
        }

        Ok(Self {
            base,
            d_model: TEMPORAL_POSITIONAL_DIMENSION,
            initialized: AtomicBool::new(true),
        })
    }

    /// Get the base frequency used by this model.
    #[must_use]
    pub fn base(&self) -> f32 {
        self.base
    }

    /// Compute the transformer-style positional encoding for a given timestamp.
    fn compute_positional_encoding(&self, timestamp: DateTime<Utc>) -> Vec<f32> {
        compute_positional_encoding(timestamp, self.base, self.d_model)
    }

    /// Extract timestamp from ModelInput.
    fn extract_timestamp(&self, input: &ModelInput) -> DateTime<Utc> {
        extract_timestamp(input)
    }

    /// Parse timestamp from instruction string.
    ///
    /// Supports formats:
    /// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
    /// - Unix epoch: "epoch:1705315800"
    pub fn parse_timestamp(instruction: &str) -> Option<DateTime<Utc>> {
        parse_timestamp(instruction)
    }
}

impl Default for TemporalPositionalModel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingModel for TemporalPositionalModel {
    fn model_id(&self) -> ModelId {
        ModelId::TemporalPositional
    }

    fn supported_input_types(&self) -> &[InputType] {
        // TemporalPositional supports Text input (timestamp via instruction field)
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        // 1. Validate input type
        self.validate_input(input)?;

        let start = std::time::Instant::now();

        // 2. Extract timestamp from input
        let timestamp = self.extract_timestamp(input);

        // 3. Compute positional encoding
        let vector = self.compute_positional_encoding(timestamp);

        let latency_us = start.elapsed().as_micros() as u64;

        // 4. Create and return ModelEmbedding
        let embedding = ModelEmbedding::new(ModelId::TemporalPositional, vector, latency_us);

        // Validate output (checks dimension, NaN, Inf)
        embedding.validate()?;

        Ok(embedding)
    }
}

// Implement Send and Sync explicitly (safe due to AtomicBool usage)
unsafe impl Send for TemporalPositionalModel {}
unsafe impl Sync for TemporalPositionalModel {}
