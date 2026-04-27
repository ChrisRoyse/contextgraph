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
//! | E8 | e5-large-v2 (Graph) | 1024 |
//! | E9 | HDC (projected) | 1024 |
//! | E10 | e5-base-v2 (Paraphrase) | 768 |
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

/// E8: Graph (e5-large-v2 for structure) embedding dimension.
///
/// Updated from MiniLM (384D) to e5-large-v2 (1024D) to:
/// - Share the model with E1 (no extra VRAM)
/// - Better semantic understanding for graph relationships
/// - Support asymmetric source/target embeddings via learned projections
pub const E8_DIM: usize = 1024;

/// E9: HDC (projected) embedding dimension.
///
/// The HDC model uses 10,000-bit native hypervectors internally but projects
/// to 1,024 dimensions for the fusion pipeline. This constant represents the
/// projected dimension that is stored in SemanticFingerprint.
pub const E9_DIM: usize = 1024;

/// E10: Paraphrase (e5-base-v2, text-only) embedding dimension.
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

/// E14: BGE-M3 Dense (XLM-RoBERTa-Large) embedding dimension.
///
/// Multilingual dense head from BAAI/bge-m3, CLS-pooled, L2-normalized.
/// Runs in parallel with E1-E13 and fuses into the standard retrieval pipeline.
pub const E14_DIM: usize = 1024;

/// Total number of embedders in the teleological vector architecture.
/// Updated from 13 to 14 with addition of E14 BGE-M3 Dense.
pub const NUM_EMBEDDERS: usize = 14;

/// Total dense dimensions for single-vector storage (excludes E6 sparse, E12 variable-length, and E13 sparse).
///
/// Calculated as: E1 + E2 + E3 + E4 + E5 + E7 + E8 + E9 + E10 + E11
/// = 1024 + 512 + 512 + 512 + 768 + 1536 + 1024 + 1024 + 768 + 768 = 8448
/// (E8 upgraded from 384 to 1024)
///
/// STOR-L2: This is the SINGLE-VECTOR total. Actual storage is larger because
/// E5, E8, and E10 store dual asymmetric vectors (cause/effect, source/target,
/// paraphrase/context). With E14 and dual vectors the total is:
///   9472 + E5_DIM(768) + E8_DIM(1024) + E10_DIM(768) = 12,032 dense dimensions.
/// This constant is used for validation and sizing of the primary embedding array,
/// not for total allocation. HNSW indexes handle dual vectors separately.
pub const TOTAL_DENSE_DIMS: usize = E1_DIM
    + E2_DIM
    + E3_DIM
    + E4_DIM
    + E5_DIM
    + E7_DIM
    + E8_DIM
    + E9_DIM
    + E10_DIM
    + E11_DIM
    + E14_DIM;
