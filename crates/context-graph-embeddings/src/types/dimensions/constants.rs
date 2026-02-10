//! Native and projected dimension constants for the 13-model embedding pipeline.
//!
//! This module defines the exact dimensions for each model in the ensemble:
//! - Native dimensions: Raw model output sizes
//! - Projected dimensions: Target sizes for Multi-Array Storage

// =============================================================================
// NATIVE OUTPUT DIMENSIONS (before any projection)
// =============================================================================

/// E1: Semantic embedding native dimension (e5-large-v2)
pub const SEMANTIC_NATIVE: usize = 1024;

/// E2: Temporal-Recent native dimension (custom exponential decay)
pub const TEMPORAL_RECENT_NATIVE: usize = 512;

/// E3: Temporal-Periodic native dimension (custom Fourier basis)
pub const TEMPORAL_PERIODIC_NATIVE: usize = 512;

/// E4: Temporal-Positional native dimension (custom sinusoidal PE)
pub const TEMPORAL_POSITIONAL_NATIVE: usize = 512;

/// E5: Causal embedding native dimension (nomic-embed-text-v1.5)
pub const CAUSAL_NATIVE: usize = 768;

/// E6: Sparse lexical native dimension (SPLADE vocab size, ~5% active)
pub const SPARSE_NATIVE: usize = 30522;

/// E7: Code embedding native dimension (Qodo-Embed 1536D)
pub const CODE_NATIVE: usize = 1536;

/// E8: Graph embedding native dimension (e5-large-v2, upgraded from MiniLM 384D)
pub const GRAPH_NATIVE: usize = 1024;

/// E9: Hyperdimensional computing native dimension (10K-bit vector)
pub const HDC_NATIVE: usize = 10000;

/// E10: Multimodal embedding native dimension (CLIP)
pub const MULTIMODAL_NATIVE: usize = 768;

/// E11: Entity embedding native dimension (legacy MiniLM-L6-v2).
/// Note: Production E11 uses ModelId::Kepler at 768D (RoBERTa + TransE).
pub const ENTITY_NATIVE: usize = 384;

/// E12: Late-interaction native dimension per token (ColBERT)
pub const LATE_INTERACTION_NATIVE: usize = 128;

/// E13: SPLADE v3 native dimension (30K sparse vocabulary)
pub const SPLADE_NATIVE: usize = 30522;

// =============================================================================
// PROJECTED DIMENSIONS (for Multi-Array Storage)
// =============================================================================

/// E1: Semantic projected dimension (no projection needed)
pub const SEMANTIC: usize = 1024;

/// E2: Temporal-Recent projected dimension (no projection needed)
pub const TEMPORAL_RECENT: usize = 512;

/// E3: Temporal-Periodic projected dimension (no projection needed)
pub const TEMPORAL_PERIODIC: usize = 512;

/// E4: Temporal-Positional projected dimension (no projection needed)
pub const TEMPORAL_POSITIONAL: usize = 512;

/// E5: Causal projected dimension (no projection needed)
pub const CAUSAL: usize = 768;

/// E6: Sparse projected dimension (30K sparse -> 1536D via learned projection)
pub const SPARSE: usize = 1536;

/// E7: Code projected dimension (Qodo-Embed 1536D, no projection needed)
pub const CODE: usize = 1536;

/// E8: Graph projected dimension (e5-large-v2, no projection needed)
pub const GRAPH: usize = 1024;

/// E9: HDC projected dimension (10K-bit -> 1024D via learned projection)
pub const HDC: usize = 1024;

/// E10: Multimodal projected dimension (no projection needed)
pub const MULTIMODAL: usize = 768;

/// E11: Entity projected dimension (legacy MiniLM, no projection needed).
/// Note: Production E11 uses ModelId::Kepler at 768D.
pub const ENTITY: usize = 384;

/// E12: Late-interaction projected dimension (pooled to single vector)
pub const LATE_INTERACTION: usize = 128;

/// E13: SPLADE v3 projected dimension (30K sparse -> 1536D via learned projection)
pub const SPLADE: usize = 1536;
