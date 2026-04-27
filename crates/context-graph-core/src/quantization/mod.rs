//! Embedding quantization for compression and efficiency.
//!
//! This module provides INT4, INT8, and FP16 quantization for embeddings,
//! with a focus on E12 (TokenPruning/LateInteraction) embeddings per
//! SPEC-E12-QUANT-001.
//!
//! TASK-L03: Added batch operations and accuracy verification.
//! TASK-P2-006: Added SemanticFingerprint quantization (~46KB -> ~11KB).

pub mod accuracy;
pub mod batch;
pub mod fingerprint;
pub mod traits;
pub mod types;

pub use accuracy::{compute_nrmse, compute_rmse, AccuracyReport};
pub use batch::{batch_dequantize, batch_quantize};
pub use fingerprint::{
    dequantize_fingerprint, quantize_fingerprint, FingerprintQuantizeError, QuantizedFloat8,
    QuantizedPQ8, QuantizedSemanticFingerprint, QuantizedSparse,
};
pub use traits::Quantizable;
pub use types::{Precision, QuantizationError, QuantizedEmbedding};
