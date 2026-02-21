//! Dense vector similarity functions with optional SIMD acceleration.
//!
//! This module provides core similarity primitives for dense embeddings
//! (E1, E2, E3, E4, E5, E7, E8, E9, E10, E11).
//!
//! # Performance
//!
//! SIMD (AVX2) provides 2-4x speedup on x86_64 for vectors >256 dimensions.
//! Constitution.yaml target: <5ms for pair similarity.
//!
//! # Dense Embedder Dimensions
//!
//! | Embedder | Dimension |
//! |----------|-----------|
//! | E1       | 1024      |
//! | E2       | 512       |
//! | E3       | 512       |
//! | E4       | 512       |
//! | E5       | 768       |
//! | E7       | 1536      |
//! | E8       | 1024      |
//! | E9       | 1024      |
//! | E10      | 768       |
//! | E11      | 768       |

mod error;
mod primitives;
#[cfg(target_arch = "x86_64")]
mod simd;
#[cfg(test)]
mod tests;

// Re-export public types
pub use error::DenseSimilarityError;
pub use primitives::{cosine_similarity, dot_product, euclidean_distance, l2_norm, normalize};
#[cfg(target_arch = "x86_64")]
pub use simd::cosine_similarity_simd;
