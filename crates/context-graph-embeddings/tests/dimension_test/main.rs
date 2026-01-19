//! Integration tests for all 13 embedder dimension validations.
//!
//! FAIL FAST: Any dimension mismatch is a critical error requiring immediate fix.
//! NO FALLBACKS: All assertions must match Constitution exactly.
//!
//! # Constitution Reference
//!
//! | Model | Native Dim | Projected Dim | Quantization |
//! |-------|------------|---------------|--------------|
//! | E1 Semantic | 1024 | 1024 | PQ8 |
//! | E2 TemporalRecent | 512 | 512 | Float8E4M3 |
//! | E3 TemporalPeriodic | 512 | 512 | Float8E4M3 |
//! | E4 TemporalPositional | 512 | 512 | Float8E4M3 |
//! | E5 Causal | 768 | 768 | PQ8 |
//! | E6 Sparse | 30522 | 1536 | SparseNative |
//! | E7 Code | 1536 | 1536 | PQ8 |
//! | E8 Graph | 384 | 384 | Float8E4M3 |
//! | E9 Hdc | 10000 | 1024 | Binary |
//! | E10 Multimodal | 768 | 768 | PQ8 |
//! | E11 Entity | 384 | 384 | Float8E4M3 |
//! | E12 LateInteraction | 128 | 128 | TokenPruning |
//! | E13 Splade | 30522 | 1536 | SparseNative |
//!
//! TOTAL_DIMENSION = 10624

mod aggregate_dimensions;
mod comprehensive;
mod constants;
mod edge_cases;
mod model_metadata;
mod native_dimensions;
mod offsets;
mod projected_dimensions;
mod quantization_methods;

// Re-export constants for use in submodules
pub use constants::*;
