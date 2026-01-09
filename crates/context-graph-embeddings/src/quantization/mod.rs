//! Quantization types for Constitution-aligned embedding compression.
//!
//! This module provides data structures and implementations for quantized embeddings
//! as specified in the Constitution's `embeddings.quantization` section.
//!
//! # Constitution Alignment
//!
//! | Method | Embedders | Compression | Max Recall Loss | Status |
//! |--------|-----------|-------------|-----------------|--------|
//! | PQ_8 | E1, E5, E7, E10 | 32x | <5% | IMPLEMENTED |
//! | Float8 | E2, E3, E4, E8, E11 | 4x | <0.3% | IMPLEMENTED |
//! | Binary | E9 | 32x | 5-10% | IMPLEMENTED |
//! | Sparse | E6, E13 | native | 0% | PASS-THROUGH |
//! | TokenPruning | E12 | ~50% | <2% | NOT IMPLEMENTED |

pub mod binary;
pub mod float8;
pub mod pq8;
pub mod router;
mod types;

#[cfg(test)]
mod edge_case_verification;

pub use binary::BinaryQuantizationError;
pub use float8::{Float8E4M3Encoder, Float8QuantizationError};
pub use pq8::{PQ8Encoder, PQ8QuantizationError};
pub use router::QuantizationRouter;
pub use types::{
    BinaryEncoder, Float8Encoder, PQ8Codebook, QuantizationMetadata, QuantizationMethod,
    QuantizedEmbedding,
};
