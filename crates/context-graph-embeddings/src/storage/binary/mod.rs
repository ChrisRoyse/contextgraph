//! GDS-compatible binary codec for FusedEmbedding.
//!
//! Provides zero-copy serialization with 64-byte alignment for GPU Direct Storage (GDS).
//!
//! # Binary Layout
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0 | 64 | EmbeddingHeader (cache-line aligned) |
//! | 64 | 6144 | Vector: [f32; 1536] big-endian |
//! | 6208 | 32 | ExpertWeights: [f32; 8] big-endian |
//! | 6240 | 4 | SelectedExperts: [u8; TOP_K_EXPERTS] |
//! | 6244 | var | AuxData (if present) |
//!
//! # Example
//!
//! ```
//! # use context_graph_embeddings::storage::{EmbeddingBinaryCodec, EMBEDDING_MAGIC};
//! # use context_graph_embeddings::types::FusedEmbedding;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a test embedding
//! let embedding = FusedEmbedding::new(
//!     vec![0.25f32; 1536], [0.125f32; 8], [0, 1, 2, 3], 750, 0x12345678,
//! )?;
//!
//! let codec = EmbeddingBinaryCodec::new();
//! let bytes = codec.encode(&embedding)?;
//! assert_eq!(&bytes[0..4], &EMBEDDING_MAGIC);
//!
//! let decoded = codec.decode(&bytes)?;
//! assert_eq!(decoded.content_hash, embedding.content_hash);
//! # Ok(())
//! # }
//! ```

mod decode;
mod encode;
mod error;
mod reference;
#[cfg(test)]
mod tests;
mod types;

// Re-export all public types for backwards compatibility
pub use encode::EmbeddingBinaryCodec;
pub use error::{DecodeError, EncodeError};
pub use reference::FusedEmbeddingRef;
pub use types::{CompressionType, EmbeddingHeader, EMBEDDING_BINARY_VERSION, EMBEDDING_MAGIC};
