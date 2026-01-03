//! FuseMoE fusion layer components.
//!
//! This module implements the Mixture-of-Experts fusion for combining
//! 12 model embeddings into a unified 1536D representation.
//!
//! # Components
//!
//! ## CPU Components
//! - [`GatingNetwork`]: Routes 8320D concatenated embeddings to 8 experts
//! - [`LayerNorm`]: Input normalization for stability
//! - [`Linear`]: Projection layer for the gating network
//! - [`Expert`]: Single expert FFN (8320 -> 4096 -> 1536)
//! - [`ExpertPool`]: Pool of 8 experts with top-k routing
//! - [`Activation`]: Activation functions for experts (GELU, ReLU, SiLU)
//!
//! ## GPU Components (feature = "candle")
//! - [`GpuLayerNorm`]: GPU-accelerated layer normalization
//! - [`GpuLinear`]: GPU-accelerated linear layer with cuBLAS GEMM
//! - [`GpuGatingNetwork`]: GPU-accelerated gating network
//! - [`GpuExpert`]: GPU-accelerated expert network
//! - [`GpuExpertPool`]: GPU-accelerated expert pool with top-k routing
//! - [`GpuFuseMoE`]: Complete GPU fusion layer (60-100x speedup)
//!
//! # Example (CPU)
//!
//! ```
//! use context_graph_embeddings::config::FusionConfig;
//! use context_graph_embeddings::types::dimensions::{TOTAL_CONCATENATED, FUSED_OUTPUT, TOP_K_EXPERTS};
//!
//! // FusionConfig defines the fusion architecture
//! let config = FusionConfig::default();
//!
//! // Verify dimensions are correct
//! assert_eq!(TOTAL_CONCATENATED, 8320);  // 12 models concatenated
//! assert_eq!(FUSED_OUTPUT, 1536);         // Output dimension
//! assert_eq!(TOP_K_EXPERTS, 4);           // Top-4 expert routing
//! ```
//!
//! # Example (GPU)
//!
//! ```rust,no_run
//! # use context_graph_embeddings::gpu::init_gpu;
//! # use context_graph_embeddings::config::FusionConfig;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize GPU for fusion layer
//! let device = init_gpu()?;
//! let config = FusionConfig::default();
//!
//! // GPU is ready for tensor operations
//! assert!(device.is_cuda());
//! # Ok(())
//! # }
//! ```

pub mod experts;
pub mod gating;
#[cfg(feature = "candle")]
pub mod gpu_fusion;

pub use experts::{Activation, Expert, ExpertPool};
pub use gating::{GatingNetwork, LayerNorm, Linear};

// GPU exports (available with cuda feature)
#[cfg(feature = "candle")]
pub use gpu_fusion::{
    GpuActivation, GpuExpert, GpuExpertPool, GpuFuseMoE, GpuGatingNetwork,
    GpuLayerNorm, GpuLinear,
};
