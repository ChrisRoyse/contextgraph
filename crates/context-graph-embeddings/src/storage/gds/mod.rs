//! GDS file reader for batch embeddings.
//!
//! Provides O(1) seeking to any embedding by index using the index file.
//!
//! # Example
//!
//! ```
//! # use context_graph_embeddings::storage::{GdsFile, BatchBinaryEncoder};
//! # use context_graph_embeddings::types::FusedEmbedding;
//! # use std::path::Path;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create test embedding and GDS file
//! let embedding = FusedEmbedding::new(
//!     vec![0.3f32; 1536], [0.125f32; 8], [0, 1, 2, 3], 600, 0xBEEFCAFE,
//! )?;
//! let temp_dir = tempfile::tempdir()?;
//! let gds_path = temp_dir.path().join("test_embeddings");
//! let mut encoder = BatchBinaryEncoder::with_capacity(10);
//! for _ in 0..5 { encoder.push(&embedding)?; }
//! encoder.write_gds_file(&gds_path)?;
//!
//! // Open and read GDS file
//! let mut gds = GdsFile::open(&gds_path)?;
//! println!("File contains {} embeddings", gds.len());
//!
//! // O(1) random access
//! let emb = gds.read(2)?;
//! println!("Content hash: {:#x}", emb.content_hash);
//! assert_eq!(emb.content_hash, 0xBEEFCAFE);
//! # Ok(())
//! # }
//! ```

mod error;
mod iter;
mod reader;

#[cfg(test)]
mod tests;

// Re-export public API for backwards compatibility
pub use error::GdsFileError;
pub use iter::GdsFileIter;
pub use reader::GdsFile;
