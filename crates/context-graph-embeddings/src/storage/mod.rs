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
//! ```rust,ignore
//! use context_graph_embeddings::storage::{
//!     EmbeddingBinaryCodec, BatchBinaryEncoder, GdsFile, EMBEDDING_MAGIC,
//! };
//!
//! // Single embedding
//! let codec = EmbeddingBinaryCodec::new();
//! let bytes = codec.encode(&embedding)?;
//! let decoded = codec.decode(&bytes)?;
//!
//! // Batch encoding to GDS files
//! let mut encoder = BatchBinaryEncoder::with_capacity(1000);
//! for emb in embeddings {
//!     encoder.push(&emb)?;
//! }
//! encoder.write_gds_file(Path::new("embeddings"))?;
//!
//! // Reading from GDS files
//! let mut gds = GdsFile::open(Path::new("embeddings"))?;
//! let emb = gds.read(42)?;  // O(1) random access
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
