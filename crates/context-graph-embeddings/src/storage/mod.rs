//! GDS-compatible binary storage for FusedEmbedding.
//!
//! This module provides zero-copy serialization with 64-byte alignment
//! for GPU Direct Storage (GDS) integration in the Knowledge Graph.
//!
//! # Architecture
//!
//! - **`binary`**: Core encoder/decoder with 64-byte aligned header
//! - **`batch`**: Batch encoder for multi-embedding files
//! - **`gds`**: File reader with O(1) seeking
//!
//! # Binary Format
//!
//! The format is designed for GDS compatibility:
//!
//! - 64-byte cache-line aligned header
//! - Big-endian floats for cross-platform compatibility
//! - 4KB page alignment in batch files
//! - Zero-copy decode via memory mapping
//!
//! # Example
//!
//! ```
//! # use context_graph_embeddings::storage::{
//! #     EmbeddingBinaryCodec, BatchBinaryEncoder, GdsFile, EMBEDDING_MAGIC,
//! # };
//! # use context_graph_embeddings::types::FusedEmbedding;
//! # use std::path::Path;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a test embedding (1536D vector)
//! let embedding = FusedEmbedding::new(
//!     vec![0.1f32; 1536],  // 1536D vector
//!     [0.125f32; 8],       // 8 expert weights
//!     [0, 1, 2, 3],        // 4 selected experts
//!     1000,                // pipeline latency μs
//!     0xDEADBEEF,          // content hash
//! )?;
//!
//! // Single embedding encode/decode
//! let codec = EmbeddingBinaryCodec::new();
//! let bytes = codec.encode(&embedding)?;
//! let decoded = codec.decode(&bytes)?;
//! assert_eq!(decoded.content_hash, embedding.content_hash);
//!
//! // Batch encoding to GDS files (uses temp dir)
//! let temp_dir = tempfile::tempdir()?;
//! let gds_path = temp_dir.path().join("embeddings");
//! let mut encoder = BatchBinaryEncoder::with_capacity(10);
//! for _ in 0..5 {
//!     encoder.push(&embedding)?;
//! }
//! encoder.write_gds_file(&gds_path)?;
//!
//! // Reading from GDS files
//! let mut gds = GdsFile::open(&gds_path)?;
//! assert_eq!(gds.len(), 5);
//! let emb = gds.read(2)?;  // O(1) random access
//! assert_eq!(emb.content_hash, 0xDEADBEEF);
//! # Ok(())
//! # }
//! ```

pub mod batch;
pub mod binary;
pub mod gds;

pub use batch::{BatchBinaryEncoder, EmbeddingIndexHeader, INDEX_MAGIC, INDEX_VERSION};
pub use binary::{
    CompressionType, DecodeError, EmbeddingBinaryCodec, EmbeddingHeader, EncodeError,
    FusedEmbeddingRef, EMBEDDING_BINARY_VERSION, EMBEDDING_MAGIC,
};
pub use gds::{GdsFile, GdsFileError, GdsFileIter};
