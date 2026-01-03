//! Batch encoding for GDS-compatible embedding files.
//!
//! Provides efficient multi-embedding serialization with 4KB page alignment
//! for optimal GPU Direct Storage (GDS) performance.
//!
//! # File Formats
//!
//! - `.cgeb` - Data file containing page-aligned embeddings
//! - `.cgei` - Index file with offset table for O(1) seeking
//!
//! # Example
//!
//! ```
//! # use context_graph_embeddings::storage::BatchBinaryEncoder;
//! # use context_graph_embeddings::types::FusedEmbedding;
//! # use std::path::Path;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create test embedding
//! let embedding = FusedEmbedding::new(
//!     vec![0.5f32; 1536], [0.125f32; 8], [0, 1, 2, 3], 500, 0xCAFEBABE,
//! )?;
//!
//! let temp_dir = tempfile::tempdir()?;
//! let gds_path = temp_dir.path().join("embeddings");
//! let mut encoder = BatchBinaryEncoder::with_capacity(10);
//! for _ in 0..3 {
//!     encoder.push(&embedding)?;
//! }
//! encoder.write_gds_file(&gds_path)?;
//! // Creates: embeddings.cgeb (data) + embeddings.cgei (index)
//! assert!(gds_path.with_extension("cgeb").exists());
//! assert!(gds_path.with_extension("cgei").exists());
//! # Ok(())
//! # }
//! ```

mod encoder;
mod types;

#[cfg(test)]
mod tests;

// Re-export public API for backwards compatibility
pub use encoder::BatchBinaryEncoder;
pub use types::{EmbeddingIndexHeader, INDEX_MAGIC, INDEX_VERSION};
