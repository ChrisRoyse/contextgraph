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
//!     vec![0.4f32; 1536], [0.125f32; 8], [0, 1, 2, 3], 700, 0xFACEFACE,
//! )?;
//! let temp_dir = tempfile::tempdir()?;
//! let gds_path = temp_dir.path().join("reader_test");
//! let mut encoder = BatchBinaryEncoder::with_capacity(10);
//! for _ in 0..3 { encoder.push(&embedding)?; }
//! encoder.write_gds_file(&gds_path)?;
//!
//! // Read from GDS file
//! let mut gds = GdsFile::open(&gds_path)?;
//! println!("File contains {} embeddings", gds.len());
//!
//! // O(1) random access
//! let emb = gds.read(1)?;
//! println!("Content hash: {:#x}", emb.content_hash);
//! # Ok(())
//! # }
//! ```

use super::error::GdsFileError;
use super::iter::GdsFileIter;
use crate::storage::batch::{EmbeddingIndexHeader, INDEX_MAGIC};
use crate::storage::binary::EmbeddingBinaryCodec;
use crate::types::FusedEmbedding;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// GDS file reader for batch embeddings.
///
/// Supports O(1) seeking to any embedding by index.
/// Reads from paired .cgeb (data) and .cgei (index) files.
#[derive(Debug)]
pub struct GdsFile {
    data_file: File,
    offsets: Vec<u64>,
    codec: EmbeddingBinaryCodec,
    data_file_hash: u64,
}

impl GdsFile {
    /// Open GDS file pair (.cgeb data + .cgei index).
    ///
    /// # Arguments
    /// * `path` - Base path without extension (e.g., "embeddings" for embeddings.cgeb + embeddings.cgei)
    ///
    /// # Errors
    /// - `GdsFileError::Io` on file open failure
    /// - `GdsFileError::InvalidIndexMagic` if index magic doesn't match "CGEI"
    pub fn open(path: &Path) -> Result<Self, GdsFileError> {
        let data_path = path.with_extension("cgeb");
        let index_path = path.with_extension("cgei");

        // Read index file
        let mut index_file = File::open(&index_path).map_err(|e| {
            GdsFileError::Io(io::Error::new(
                e.kind(),
                format!("Failed to open index file {}: {}", index_path.display(), e),
            ))
        })?;

        let mut header_bytes = [0u8; 24];
        index_file.read_exact(&mut header_bytes).map_err(|e| {
            GdsFileError::Io(io::Error::new(
                e.kind(),
                format!("Failed to read index header: {}", e),
            ))
        })?;

        let header: &EmbeddingIndexHeader = bytemuck::from_bytes(&header_bytes);

        // Validate magic - FAIL FAST
        if header.magic != INDEX_MAGIC {
            return Err(GdsFileError::InvalidIndexMagic);
        }

        let entry_count = u64::from_be(header.entry_count) as usize;
        let data_file_hash = u64::from_be(header.data_file_hash);

        // Read offset table
        let mut offsets = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let mut offset_bytes = [0u8; 8];
            index_file.read_exact(&mut offset_bytes).map_err(|e| {
                GdsFileError::Io(io::Error::new(
                    e.kind(),
                    format!("Failed to read offset table: {}", e),
                ))
            })?;
            offsets.push(u64::from_be_bytes(offset_bytes));
        }

        let data_file = File::open(&data_path).map_err(|e| {
            GdsFileError::Io(io::Error::new(
                e.kind(),
                format!("Failed to open data file {}: {}", data_path.display(), e),
            ))
        })?;

        Ok(Self {
            data_file,
            offsets,
            codec: EmbeddingBinaryCodec::new(),
            data_file_hash,
        })
    }

    /// Number of embeddings in file.
    #[inline]
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// Check if file has no embeddings.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Get the data file hash stored in the index.
    #[inline]
    pub fn data_file_hash(&self) -> u64 {
        self.data_file_hash
    }

    /// Get offset for embedding at index.
    ///
    /// # Errors
    /// - `GdsFileError::IndexOutOfBounds` if index >= len
    pub fn get_offset(&self, index: usize) -> Result<u64, GdsFileError> {
        if index >= self.offsets.len() {
            return Err(GdsFileError::IndexOutOfBounds {
                index,
                len: self.offsets.len(),
            });
        }
        Ok(self.offsets[index])
    }

    /// Read embedding at index (O(1) seek + read).
    ///
    /// # Arguments
    /// * `index` - Zero-based embedding index
    ///
    /// # Errors
    /// - `GdsFileError::IndexOutOfBounds` if index >= len
    /// - `GdsFileError::Decode` on decode failure
    /// - `GdsFileError::Io` on file I/O failure
    pub fn read(&mut self, index: usize) -> Result<FusedEmbedding, GdsFileError> {
        if index >= self.offsets.len() {
            return Err(GdsFileError::IndexOutOfBounds {
                index,
                len: self.offsets.len(),
            });
        }

        let offset = self.offsets[index];

        // Calculate size from offset difference or use MIN_BUFFER_SIZE for last entry
        let size = if index + 1 < self.offsets.len() {
            (self.offsets[index + 1] - offset) as usize
        } else {
            // Last entry - use MIN_BUFFER_SIZE (conservative, no aux_data)
            EmbeddingBinaryCodec::MIN_BUFFER_SIZE
        };

        // Ensure we read at least MIN_BUFFER_SIZE
        let read_size = size.max(EmbeddingBinaryCodec::MIN_BUFFER_SIZE);

        // Seek and read
        self.data_file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0u8; read_size];

        // For last entry, we might read past EOF - that's OK, just read what's available
        let bytes_read = self.data_file.read(&mut buffer)?;
        if bytes_read < EmbeddingBinaryCodec::MIN_BUFFER_SIZE {
            return Err(GdsFileError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "Incomplete embedding at index {}: read {} bytes, need {}",
                    index,
                    bytes_read,
                    EmbeddingBinaryCodec::MIN_BUFFER_SIZE
                ),
            )));
        }

        self.codec
            .decode(&buffer[..bytes_read])
            .map_err(GdsFileError::Decode)
    }

    /// Read multiple embeddings efficiently (batch read).
    ///
    /// # Arguments
    /// * `indices` - Slice of embedding indices to read
    ///
    /// # Errors
    /// - Same as `read`
    pub fn read_batch(&mut self, indices: &[usize]) -> Result<Vec<FusedEmbedding>, GdsFileError> {
        let mut results = Vec::with_capacity(indices.len());
        for &index in indices {
            results.push(self.read(index)?);
        }
        Ok(results)
    }

    /// Iterate over all embeddings.
    ///
    /// Returns an iterator that yields each embedding in order.
    pub fn iter(&mut self) -> GdsFileIter<'_> {
        GdsFileIter::new(self)
    }
}
