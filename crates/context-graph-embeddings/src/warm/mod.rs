//! Warm Model Loading System
//!
//! Pre-loads all 12 embedding models into VRAM at startup.
//!
//! This module is under active development. Currently implemented:
//! - `error`: WarmError types and exit codes
//! - `config`: WarmConfig configuration struct and QuantizationMode
//! - `state`: WarmModelState enum for tracking model loading lifecycle
//! - `handle`: ModelHandle for protected VRAM allocations
//! - `registry`: WarmModelRegistry for tracking loading state of all models
//! - `memory_pool`: WarmMemoryPools for VRAM allocation management
//! - `validation`: WarmValidator for model dimension/weight/inference validation
//! - `cuda_alloc`: WarmCudaAllocator for non-evictable VRAM allocations
//! - `loader`: WarmLoader main orchestrator for warm model loading
//! - `diagnostics`: WarmDiagnostics for comprehensive diagnostic reporting
//! - `integration`: WarmEmbeddingPipeline for unified pipeline access
//! - `inference`: InferenceEngine for GPU inference validation (TASK-EMB-015)
//!
//! # Requirements
//!
//! - CUDA 13.1+
//! - RTX 5090 or equivalent (32GB VRAM)
//! - Compute capability 12.0+

pub mod config;
pub mod cuda_alloc;
pub mod diagnostics;
pub mod error;
pub mod handle;
pub mod health;
pub mod inference;
pub mod integration;
// loader is now a directory module
pub mod loader;
pub mod memory_pool;
pub mod registry;
pub mod state;
pub mod validation;

#[cfg(test)]
mod tests;

// Re-export error types for convenient access
pub use error::{WarmError, WarmResult};

// Re-export config types for convenient access
pub use config::{QuantizationMode, WarmConfig};

// Re-export state types for convenient access
pub use state::WarmModelState;

// Re-export handle types for convenient access
pub use handle::ModelHandle;

// Re-export registry types for convenient access
pub use registry::{
    SharedWarmRegistry, WarmModelEntry, WarmModelRegistry, EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT,
};

// Re-export validation types for convenient access
pub use validation::{TestInferenceConfig, TestInput, ValidationResult, WarmValidator};

// Re-export memory pool types for convenient access
pub use memory_pool::{ModelAllocation, ModelMemoryPool, WarmMemoryPools, WorkingMemoryPool};

// Re-export CUDA allocation types for convenient access
pub use cuda_alloc::{
    GpuInfo, VramAllocation, WarmCudaAllocator, FAKE_ALLOCATION_BASE_PATTERN,
    GOLDEN_SIMILARITY_THRESHOLD, MINIMUM_VRAM_BYTES, REQUIRED_COMPUTE_MAJOR,
    REQUIRED_COMPUTE_MINOR, SIN_WAVE_ENERGY_THRESHOLD,
};

// Re-export loader types for convenient access
pub use loader::{LoadingSummary, WarmLoader};

// Re-export warm loading data types (TASK-EMB-006)
pub use loader::types::{LoadedModelWeights, TensorMetadata, WarmLoadResult};

// Re-export health check types for convenient access
pub use health::{WarmHealthCheck, WarmHealthChecker, WarmHealthStatus};

// Re-export diagnostic types for convenient access
pub use diagnostics::{
    ErrorDiagnostic, GpuDiagnostics, MemoryDiagnostics, ModelDiagnostic, SystemInfo,
    WarmDiagnosticReport, WarmDiagnostics,
};

// Re-export integration types for convenient access
pub use integration::WarmEmbeddingPipeline;

// Re-export inference types for convenient access (TASK-EMB-015, TASK-EMB-017)
pub use inference::{
    cosine_similarity, detect_sin_wave_pattern, validate_inference_output_ap007, InferenceEngine,
};
