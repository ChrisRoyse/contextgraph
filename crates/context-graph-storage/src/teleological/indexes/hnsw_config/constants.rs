//! Dimension constants for 13 embedders.
//!
//! Mirrored from context-graph-core for independence.

/// E1 Semantic: 1024D (e5-large-v2, Matryoshka-capable)
pub const E1_DIM: usize = 1024;

/// E2 Temporal Recent: 512D (exponential decay)
pub const E2_DIM: usize = 512;

/// E3 Temporal Periodic: 512D (Fourier)
pub const E3_DIM: usize = 512;

/// E4 Temporal Positional: 512D (sinusoidal PE)
pub const E4_DIM: usize = 512;

/// E5 Causal: 768D (Longformer SCM)
pub const E5_DIM: usize = 768;

/// E6 Sparse: 30522 vocab (BERT vocabulary)
pub const E6_SPARSE_VOCAB: usize = 30_522;

/// E7 Code: 1536D (Qodo-Embed)
pub const E7_DIM: usize = 1536;

/// E8 Graph: 1024D (e5-large-v2, shared with E1)
pub const E8_DIM: usize = 1024;

/// E9 HDC: 1024D (projected from 10K-bit hypervector)
pub const E9_DIM: usize = 1024;

/// E10 Multimodal: 768D (CLIP)
pub const E10_DIM: usize = 768;

/// E11 Entity: 768D (KEPLER RoBERTa-base + TransE)
pub const E11_DIM: usize = 768;

/// E12 Late Interaction: 128D per token (ColBERT)
pub const E12_TOKEN_DIM: usize = 128;

/// E13 SPLADE: 30522 vocab (sparse BM25)
pub const E13_SPLADE_VOCAB: usize = 30_522;

/// Number of core embedders (E1-E13)
pub const NUM_EMBEDDERS: usize = 13;

/// E1 Matryoshka truncated dimension for Stage 2
pub const E1_MATRYOSHKA_DIM: usize = 128;

/// Topic profile dimension (one per embedder)
pub const TOPIC_PROFILE_DIM: usize = 13;
