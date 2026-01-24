//! Dimension constants for all 13 embedders in the teleological vector architecture.
//!
//! # Embedding Dimensions
//!
//! | Embedding | Model | Dimensions |
//! |-----------|-------|------------|
//! | E1 | e5-large-v2 | 1024 |
//! | E2 | Exponential Decay | 512 |
//! | E3 | Fourier Periodic | 512 |
//! | E4 | Sinusoidal PE | 512 |
//! | E5 | Longformer SCM | 768 |
//! | E6 | SPLADE (Sparse) | ~1500 active / 30522 vocab |
//! | E7 | Qodo-Embed | 1536 |
//! | E8 | MiniLM (Graph) | 384 |
//! | E9 | HDC (projected) | 1024 |
//! | E10 | CLIP | 768 |
//! | E11 | KEPLER (Entity) | 768 |
//! | E12 | ColBERT (Late-Interaction) | 128 per token |
//! | E13 | SPLADE v3 (Sparse) | 30522 vocab |

/// E1: Semantic (e5-large-v2) embedding dimension.
pub const E1_DIM: usize = 1024;

/// E2: Temporal-Recent (exponential decay) embedding dimension.
pub const E2_DIM: usize = 512;

/// E3: Temporal-Periodic (Fourier) embedding dimension.
pub const E3_DIM: usize = 512;

/// E4: Temporal-Positional (sinusoidal PE) embedding dimension.
pub const E4_DIM: usize = 512;

/// E5: Causal (Longformer SCM) embedding dimension.
pub const E5_DIM: usize = 768;

/// E6: Sparse lexical (SPLADE) vocabulary size.
pub const E6_SPARSE_VOCAB: usize = 30_522;

/// E7: Code (Qodo-Embed) embedding dimension.
pub const E7_DIM: usize = 1536;

/// E8: Graph (MiniLM for structure) embedding dimension.
pub const E8_DIM: usize = 384;

/// E9: HDC (projected) embedding dimension.
///
/// The HDC model uses 10,000-bit native hypervectors internally but projects
/// to 1,024 dimensions for the fusion pipeline. This constant represents the
/// projected dimension that is stored in SemanticFingerprint.
pub const E9_DIM: usize = 1024;

/// E10: Multimodal (CLIP) embedding dimension.
pub const E10_DIM: usize = 768;

/// E11: Entity (KEPLER for knowledge graph) embedding dimension.
/// Updated from 384D (MiniLM) to 768D (KEPLER RoBERTa-base + TransE).
pub const E11_DIM: usize = 768;

/// E12: Late-Interaction (ColBERT) per-token embedding dimension.
pub const E12_TOKEN_DIM: usize = 128;

/// E13: SPLADE v3 sparse embedding vocabulary size.
///
/// SPLADE v3 uses BERT vocabulary (30,522 tokens).
/// This is a NEW field for Stage 1 (sparse pre-filter) of the 5-stage retrieval pipeline.
pub const E13_SPLADE_VOCAB: usize = 30_522;

/// Total number of embedders in the teleological vector architecture.
/// Updated from 12 to 13 with addition of E13 SPLADE.
pub const NUM_EMBEDDERS: usize = 13;

/// Total dense dimensions (excluding E6 sparse, E12 variable-length, and E13 sparse).
///
/// Calculated as: E1 + E2 + E3 + E4 + E5 + E7 + E8 + E9 + E10 + E11
/// = 1024 + 512 + 512 + 512 + 768 + 1536 + 384 + 1024 + 768 + 768 = 7808
pub const TOTAL_DENSE_DIMS: usize =
    E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM + E10_DIM + E11_DIM;
