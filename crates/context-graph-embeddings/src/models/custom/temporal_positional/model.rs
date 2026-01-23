//! Core Temporal-Positional embedding model implementation (E4).
//!
//! E4 encodes session sequence positions to enable "before/after" queries within a session.
//! This is distinct from E2 (V_freshness) which encodes Unix timestamps.
//!
//! # Hybrid Mode
//!
//! E4 supports a hybrid encoding mode that combines:
//! - **First 256D**: Session signature (deterministic hash-based clustering)
//! - **Last 256D**: Position encoding (sinusoidal, original approach)
//!
//! This enables same-session memories to cluster in E4 space while preserving
//! fine-grained position ordering within sessions.

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::constants::{
    DEFAULT_BASE, HYBRID_MODE_DEFAULT, MAX_BASE, MIN_BASE, TEMPORAL_POSITIONAL_DIMENSION,
};
use super::encoding::{
    compute_positional_encoding, compute_positional_encoding_from_position,
    compute_positional_encoding_hybrid,
};
use super::session_signature::compute_session_signature_or_default;
use super::timestamp::{
    extract_hybrid_position, extract_position, extract_timestamp, parse_position, parse_timestamp,
    HybridPositionInfo,
};

/// Temporal-Positional embedding model (E4 - V_ordering).
///
/// Encodes **session sequence positions** to enable "before/after" queries within a session.
/// This is distinct from E2 (V_freshness) which encodes Unix timestamps for recency.
///
/// # Hybrid Mode (Default)
///
/// By default, E4 uses **hybrid mode** which combines:
/// - **First 256D**: Session signature (deterministic hash-based clustering)
/// - **Last 256D**: Position encoding (sinusoidal, original approach)
///
/// This enables same-session memories to cluster in E4 space while preserving
/// fine-grained position ordering within sessions.
///
/// # Instruction Format
///
/// For hybrid mode, instructions should include session context:
/// - `"session:abc123 sequence:42"` (preferred) - Session ID with sequence position
/// - `"session:abc123 timestamp:2024-01-15T10:30:00Z"` - Session ID with timestamp
///
/// Legacy formats without session are still supported (backward compatible):
/// - `"sequence:N"` - Session sequence number (uses sentinel session)
/// - `"timestamp:ISO8601"` - ISO 8601 timestamp
/// - `"epoch:N"` - Unix epoch seconds
/// - Falls back to `Utc::now()` if no instruction provided
///
/// # Algorithm
///
/// **Hybrid mode (default):**
/// - First 256D: Session signature from hash-based generation
/// - Last 256D: Sinusoidal positional encoding
/// - Final L2 normalization of combined 512D vector
///
/// **Legacy mode (hybrid_mode=false):**
/// - Full 512D sinusoidal positional encoding (original behavior)
///
/// # Purpose
///
/// E4 enables queries like:
/// - "What happened before X in this session?" (within-session ordering)
/// - "What other memories are from the same session?" (session clustering)
/// - "Order these memories by when they occurred in the session"
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::TemporalPositionalModel;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use context_graph_embeddings::EmbeddingModel; // For is_initialized() trait method
///
/// fn example() -> EmbeddingResult<()> {
///     // Default: hybrid mode enabled
///     let model = TemporalPositionalModel::new();
///     assert!(model.is_initialized());
///     assert!(model.is_hybrid_mode());
///
///     // Explicit hybrid mode control
///     let model = TemporalPositionalModel::with_hybrid_mode(false);
///     assert!(!model.is_hybrid_mode()); // Legacy mode
///
///     // Custom base frequency (hybrid mode still enabled)
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

    /// Enable hybrid mode: session_signature || position_encoding.
    /// When false, uses legacy full positional encoding.
    hybrid_mode: bool,
}

impl TemporalPositionalModel {
    /// Create a new TemporalPositionalModel with default settings.
    ///
    /// Uses default base frequency (10000.0) and hybrid mode enabled.
    /// Model is immediately ready for use (no loading required).
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DEFAULT_BASE,
            d_model: TEMPORAL_POSITIONAL_DIMENSION,
            initialized: AtomicBool::new(true),
            hybrid_mode: HYBRID_MODE_DEFAULT,
        }
    }

    /// Create a model with explicit hybrid mode setting.
    ///
    /// # Arguments
    /// * `hybrid` - If true, uses session_signature || position_encoding.
    ///              If false, uses legacy full positional encoding.
    #[must_use]
    pub fn with_hybrid_mode(hybrid: bool) -> Self {
        Self {
            base: DEFAULT_BASE,
            d_model: TEMPORAL_POSITIONAL_DIMENSION,
            initialized: AtomicBool::new(true),
            hybrid_mode: hybrid,
        }
    }

    /// Create a model with custom base frequency.
    ///
    /// Hybrid mode is enabled by default.
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
            hybrid_mode: HYBRID_MODE_DEFAULT,
        })
    }

    /// Create a model with custom base frequency and hybrid mode setting.
    ///
    /// # Arguments
    /// * `base` - Base frequency for positional encoding. Must be > 1.0.
    /// * `hybrid` - If true, uses hybrid mode; if false, legacy mode.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if base is not in valid range.
    pub fn with_base_and_hybrid_mode(base: f32, hybrid: bool) -> EmbeddingResult<Self> {
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
            hybrid_mode: hybrid,
        })
    }

    /// Get the base frequency used by this model.
    #[must_use]
    pub fn base(&self) -> f32 {
        self.base
    }

    /// Check if hybrid mode is enabled.
    ///
    /// In hybrid mode, embeddings combine session signature (256D) with
    /// position encoding (256D) for a total of 512D.
    #[must_use]
    pub fn is_hybrid_mode(&self) -> bool {
        self.hybrid_mode
    }

    /// Compute the positional encoding for a given position (legacy mode).
    ///
    /// # Arguments
    /// * `position` - The position value (sequence number or Unix timestamp)
    /// * `is_sequence` - True if position is a session sequence number
    fn compute_positional_encoding_from_pos(
        &self,
        position: i64,
        is_sequence: bool,
    ) -> Vec<f32> {
        compute_positional_encoding_from_position(position, self.base, self.d_model, is_sequence)
    }

    /// Compute hybrid embedding: session_signature || position_encoding.
    ///
    /// # Arguments
    /// * `info` - Hybrid position info containing session_id and position
    ///
    /// # Returns
    /// A 512D L2-normalized vector with:
    /// - First 256D: Session signature
    /// - Last 256D: Position encoding
    fn compute_hybrid_embedding(&self, info: &HybridPositionInfo) -> Vec<f32> {
        let mut embedding = Vec::with_capacity(TEMPORAL_POSITIONAL_DIMENSION);

        // First 256D: Session signature (or sentinel for no session)
        let session_sig = compute_session_signature_or_default(info.session_id.as_deref());
        embedding.extend(session_sig);

        // Last 256D: Position encoding (reduced dimension)
        let position_enc =
            compute_positional_encoding_hybrid(info.position, self.base, info.is_sequence);
        embedding.extend(position_enc);

        // Final L2 normalization of combined vector
        // This ensures the full 512D vector is unit length
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for v in &mut embedding {
                *v /= norm;
            }
        }

        debug_assert_eq!(embedding.len(), TEMPORAL_POSITIONAL_DIMENSION);
        embedding
    }

    /// Compute the transformer-style positional encoding for a given timestamp.
    ///
    /// Legacy method for backward compatibility.
    fn compute_positional_encoding(&self, timestamp: DateTime<Utc>) -> Vec<f32> {
        compute_positional_encoding(timestamp, self.base, self.d_model)
    }

    /// Extract timestamp from ModelInput.
    ///
    /// Legacy method for backward compatibility.
    fn extract_timestamp(&self, input: &ModelInput) -> DateTime<Utc> {
        extract_timestamp(input)
    }

    /// Parse timestamp from instruction string (legacy API).
    ///
    /// Supports formats:
    /// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
    /// - Unix epoch: "epoch:1705315800"
    ///
    /// For new code, prefer `parse_position()` which also supports sequence numbers.
    pub fn parse_timestamp(instruction: &str) -> Option<DateTime<Utc>> {
        parse_timestamp(instruction)
    }

    /// Parse position from instruction string.
    ///
    /// Supports formats (priority order):
    /// - Sequence: "sequence:123" (preferred for E4)
    /// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
    /// - Unix epoch: "epoch:1705315800"
    pub fn parse_position(instruction: &str) -> Option<super::timestamp::PositionInfo> {
        parse_position(instruction)
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

        // 2. Compute embedding based on mode (hybrid vs legacy)
        let vector = if self.hybrid_mode {
            // Hybrid mode: session_signature || position_encoding (256D + 256D)
            let hybrid_info = extract_hybrid_position(input);
            self.compute_hybrid_embedding(&hybrid_info)
        } else {
            // Legacy mode: full 512D positional encoding
            let position_info = extract_position(input);
            self.compute_positional_encoding_from_pos(
                position_info.position,
                position_info.is_sequence,
            )
        };

        let latency_us = start.elapsed().as_micros() as u64;

        // 3. Create and return ModelEmbedding
        let embedding = ModelEmbedding::new(ModelId::TemporalPositional, vector, latency_us);

        // Validate output (checks dimension, NaN, Inf)
        embedding.validate()?;

        Ok(embedding)
    }
}

// Implement Send and Sync explicitly (safe due to AtomicBool usage)
unsafe impl Send for TemporalPositionalModel {}
unsafe impl Sync for TemporalPositionalModel {}
