//! Quantization types for Constitution-aligned embedding compression.
//!
//! This module provides data structures for quantized embeddings as specified
//! in the Constitution's `embeddings.quantization` section. Actual quantization
//! implementations are in the Logic Layer (TASK-EMB-016, 017, 018).
//!
//! # Constitution Alignment
//!
//! | Method | Embedders | Compression | Max Recall Loss |
//! |--------|-----------|-------------|-----------------|
//! | PQ_8 | E1, E5, E7, E10 | 32x | <5% |
//! | Float8 | E2, E3, E4, E8, E11 | 4x | <0.3% |
//! | Binary | E9 | 32x | 5-10% |
//! | Sparse | E6, E13 | native | 0% |
//! | TokenPruning | E12 | ~50% | <2% |

mod types;
pub mod binary;
pub mod router;

#[cfg(test)]
mod edge_case_verification;

pub use types::{
    BinaryEncoder, Float8Encoder, PQ8Codebook, QuantizationMetadata, QuantizationMethod,
    QuantizedEmbedding,
};
pub use binary::BinaryQuantizationError;
pub use router::QuantizationRouter;
