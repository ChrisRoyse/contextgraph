#![deny(deprecated)]
#![allow(clippy::module_inception)]

//! Embedding pipeline for Context Graph.
//!
//! This crate provides GPU-accelerated text-to-embedding conversion using
//! local models via the Candle backend (mandatory).
//!
//! # Mandatory Feature: `candle`
//!
//! This crate requires the `candle` feature to be enabled. The crate is designed
//! for GPU-first architecture targeting NVIDIA RTX 5090 with CUDA 13.2.
//! Building without the `candle` feature will result in a compile error.
//!
//! # Architecture
//!
//! - **ModelId**: Enum identifying the 13 models in the ensemble (E1-E13)
//! - **EmbeddingProvider**: Trait for embedding generation
//! - **EmbeddingModel**: Trait for individual model implementations
//! - **ModelRegistry**: Manages model lifecycle and GPU resources
//!
//! # Example
//!
//! ```rust
//! use context_graph_embeddings::{ModelId, ModelRegistryConfig};
//!
//! // Check model dimensions statically
//! assert_eq!(ModelId::Semantic.dimension(), 1024);
//! assert_eq!(ModelId::Code.dimension(), 1536); // Qodo-Embed-1-1.5B native
//! assert_eq!(ModelId::Kepler.dimension(), 768);
//!
//! // Registry config defaults
//! let config = ModelRegistryConfig::default();
//! assert!(config.max_concurrent_loads > 0);
//! ```

// =============================================================================
// MANDATORY FEATURE GATE - GPU-First Architecture
// =============================================================================
// The `candle` feature is REQUIRED for context-graph-embeddings.
// This crate is designed for GPU-first architecture targeting NVIDIA RTX 5090
// with CUDA 13.2. All embedding operations require Candle's GPU acceleration.
// =============================================================================

#[cfg(not(feature = "candle"))]
compile_error!(
    "The `candle` feature is required for context-graph-embeddings. \
     This crate is designed for GPU-first architecture targeting NVIDIA RTX 5090 with CUDA 13.2. \
     Build with: cargo build --features candle \
     Or enable the default feature: cargo build (candle is now default)"
);

pub mod adapters;
pub mod batch;
pub mod cache;
pub mod config;
pub mod error;
pub mod global_provider;
pub mod gpu;
pub mod learner;
pub mod models;
pub mod provider;
pub mod pruning;
pub mod quantization;
pub mod storage;
pub mod training;
pub mod traits;
pub mod types;
pub mod warm;

// Cache re-exports
pub use cache::{CacheKey, CacheStats, EmbeddingCache};

pub use config::{
    BatchConfig, CacheConfig, EmbeddingConfig, EvictionPolicy, GpuConfig, ModelPathConfig,
    PaddingStrategy,
};
pub use error::{EmbeddingError, EmbeddingResult};
pub use provider::{EmbeddingProvider, ProductionMultiArrayProvider};
pub use traits::{
    get_memory_estimate, DevicePlacement, EmbeddingModel, ModelFactory, QuantizationMode,
    SingleModelConfig, MEMORY_ESTIMATES, TOTAL_MEMORY_ESTIMATE,
};

// Type re-exports for public API
pub use types::{
    ImageFormat, InputType, ModelEmbedding, ModelId, ModelInput, MultiArrayEmbedding,
    TokenizerFamily,
};

// Re-export dimensions module for constant access
pub use types::dimensions;

// Quantization re-exports
pub use quantization::{
    BinaryEncoder, Float8Encoder, PQ8Codebook, QuantizationMetadata, QuantizationMethod,
    QuantizationRouter, QuantizedEmbedding,
};

// Model registry re-exports
pub use models::{MemoryTracker, ModelRegistry, ModelRegistryConfig, RegistryStats};

// Storage re-exports
pub use storage::{
    EmbedderQueryResult, IndexEntry, MultiSpaceQueryResult, StoredQuantizedFingerprint,
    EXPECTED_QUANTIZED_SIZE_BYTES, MAX_QUANTIZED_SIZE_BYTES, RRF_K, STORAGE_VERSION,
};

// Pruning re-exports
pub use pruning::{
    ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig, TokenPruningQuantizer,
};

// Global warm provider re-exports (TASK-EMB-016)
// Model accessor functions added for graph/causal discovery services
pub use global_provider::{
    get_warm_causal_model, get_warm_graph_model, get_warm_provider,
    initialize_global_warm_provider, initialize_global_warm_provider_with_models_dir,
    is_warm_initialized, warm_status_message,
};

// Learner-state E15-E21 embedder namespace.
pub use learner::{
    embed_calibration_sample, embed_learner_signal, learner_embedder_specs,
    learner_model_asset_requirements, preflight_learner_assets, state_vector_from_outputs,
    synthetic_calibration_fixture, CalibrationDatasetSpec, LearnerAssetFileGroup,
    LearnerAssetFileStatus, LearnerAssetPreflightReport, LearnerCalibrationAssetStatus,
    LearnerCalibrationFixture, LearnerCalibrationSample, LearnerEmbedderInput, LearnerEmbedderSlot,
    LearnerEmbedderSpec, LearnerEmbeddingOutput, LearnerModelAssetRequirement,
    LearnerModelAssetStatus, CALIBRATION_DATASET_MANIFEST, LEARNER_EMBEDDER_COUNT,
    UTL_PLANNED_TOTAL_EMBEDDERS,
};

// Adapter re-exports
pub use adapters::E7CodeEmbeddingProvider;
