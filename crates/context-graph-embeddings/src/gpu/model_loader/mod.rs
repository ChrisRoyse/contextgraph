//! GPU ModelLoader for loading pretrained BERT models from safetensors.
//!
//! # Architecture
//!
//! This module provides GPU-accelerated model loading via Candle's VarBuilder.
//! It loads safetensors files from local model directories and constructs
//! complete BERT architecture components for embedding generation.
//!
//! # Supported Architectures
//!
//! | Model Type | Architecture | Example |
//! |------------|--------------|---------|
//! | BERT | BertModel | e5-large-v2, all-MiniLM-L6-v2 |
//! | MPNet | MPNetModel | all-mpnet-base-v2 |
//!
//! # Module Structure
//!
//! - [`config`] - BERT configuration parsing from config.json
//! - [`error`] - Error types for model loading operations
//! - [`weights`] - Weight structures for BERT components
//! - [`loader`] - GPU model loader implementation
//! - [`embedding_loader`] - Embedding and pooler weight loading
//! - [`layer_loader`] - Encoder layer weight loading
//! - [`tensor_utils`] - Tensor loading utilities
//!
//! # Usage
//!
//! ```rust,no_run
//! use context_graph_embeddings::gpu::{GpuModelLoader, BertConfig};
//!
//! // GpuModelLoader initializes GPU automatically
//! let loader = GpuModelLoader::new().expect("GPU init should succeed");
//!
//! // Verify GPU device is available
//! let device = loader.device();
//! assert!(device.is_cuda());
//! ```

mod batch_loader;
mod config;
mod embedding_loader;
mod error;
mod layer_loader;
mod loader;
mod tensor_utils;
mod weights;

// Re-export all public types for backwards compatibility
pub use config::BertConfig;
pub use error::ModelLoadError;
pub use loader::GpuModelLoader;
pub use weights::{
    AttentionWeights, BertWeights, EmbeddingWeights, EncoderLayerWeights, FfnWeights,
    PoolerWeights,
};
