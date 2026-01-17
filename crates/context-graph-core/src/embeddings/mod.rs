//! Embedding types for the 13-model teleological array.
//!
//! This module provides:
//! - `EmbedderCategory`: Category classification (Semantic, Temporal, Relational, Structural)
//! - `EmbedderConfig`: Static configuration for each embedder
//! - `QuantizationConfig`: Quantization methods
//! - `TokenPruningEmbedding` (E12): Token-level embedding with Quantizable support
//! - `DenseVector`: Generic dense vector for similarity computation
//! - `BinaryVector`: Bit-packed vector for Hamming distance
//! - `StubMultiArrayProvider`: Test implementation of MultiArrayEmbeddingProvider
//! - Stub embedders: `StubSingleEmbedder`, `StubSparseEmbedder`, `StubTokenEmbedder`
//!
//! Note: `SparseVector` for SPLADE is in `types::fingerprint::sparse`.
//! Note: `Embedder` enum is in `teleological::embedder`.
//! Note: `DistanceMetric` is in `index::config`.

pub mod category;
pub mod config;
pub mod provider;
pub mod stubs;
pub mod token_pruning;
pub mod vector;

pub use category::{category_for, max_weighted_agreement, topic_threshold, EmbedderCategory};
pub use config::{
    get_category, get_config, get_dimension, get_distance_metric, get_quantization,
    get_topic_weight, is_asymmetric, is_relational, is_semantic, is_sparse, is_structural,
    is_temporal, is_token_level, EmbedderConfig, QuantizationConfig, EMBEDDER_CONFIGS,
};
pub use provider::StubMultiArrayProvider;
pub use stubs::{StubSingleEmbedder, StubSparseEmbedder, StubTokenEmbedder};
pub use token_pruning::TokenPruningEmbedding;
pub use vector::{BinaryVector, DenseVector, VectorError};
