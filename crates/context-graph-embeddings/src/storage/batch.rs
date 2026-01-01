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
//! ```rust,ignore
//! use context_graph_embeddings::storage::BatchBinaryEncoder;
//!
//! let mut encoder = BatchBinaryEncoder::with_capacity(1000);
//! for embedding in embeddings {
//!     encoder.push(&embedding)?;
//! }
//! encoder.write_gds_file(Path::new("embeddings"))?;
//! // Creates: embeddings.cgeb (data) + embeddings.cgei (index)
//! ```

use super::binary::{CompressionType, EmbeddingBinaryCodec, EncodeError};
use crate::types::FusedEmbedding;
use bytemuck::{bytes_of, Pod, Zeroable};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

/// Index file header for batch embeddings.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EmbeddingIndexHeader {
    /// Magic bytes: "CGEI" = Context Graph Embedding Index
    pub magic: [u8; 4],
    /// Format version
    pub version: u16,
    /// Reserved
    pub _reserved: u16,
    /// Number of entries in the index
    pub entry_count: u64,
    /// Hash of associated data file (for integrity)
    pub data_file_hash: u64,
}

/// Index file magic bytes: "CGEI"
pub const INDEX_MAGIC: [u8; 4] = [0x43, 0x47, 0x45, 0x49];

/// Index file format version.
pub const INDEX_VERSION: u16 = 1;

// Compile-time assertion: index header must be exactly 24 bytes
const _INDEX_HEADER_SIZE_CHECK: () = assert!(
    std::mem::size_of::<EmbeddingIndexHeader>() == 24,
    "EmbeddingIndexHeader must be exactly 24 bytes"
);

/// Batch encoder for efficient multi-embedding serialization.
///
/// Accumulates embeddings in memory and writes GDS-compatible files
/// with 4KB page alignment for optimal I/O performance.
pub struct BatchBinaryEncoder {
    codec: EmbeddingBinaryCodec,
    buffer: Vec<u8>,
    offsets: Vec<u64>,
}

impl BatchBinaryEncoder {
    /// GDS page alignment (4KB).
    pub const PAGE_SIZE: usize = 4096;

    /// Create batch encoder with estimated capacity.
    ///
    /// # Arguments
    /// * `count` - Expected number of embeddings (for buffer pre-allocation)
    #[must_use]
    pub fn with_capacity(count: usize) -> Self {
        Self {
            codec: EmbeddingBinaryCodec::new(),
            buffer: Vec::with_capacity(count * EmbeddingBinaryCodec::MIN_BUFFER_SIZE),
            offsets: Vec::with_capacity(count),
        }
    }

    /// Create batch encoder with auxiliary data support.
    ///
    /// # Arguments
    /// * `count` - Expected number of embeddings
    #[must_use]
    pub fn with_aux_data(count: usize) -> Self {
        Self {
            codec: EmbeddingBinaryCodec::with_aux_data(CompressionType::None),
            buffer: Vec::with_capacity(count * (EmbeddingBinaryCodec::MIN_BUFFER_SIZE + 1024)),
            offsets: Vec::with_capacity(count),
        }
    }

    /// Add embedding to batch.
    ///
    /// # Errors
    /// - `EncodeError` if encoding fails (e.g., invalid dimension)
    pub fn push(&mut self, embedding: &FusedEmbedding) -> Result<(), EncodeError> {
        let offset = self.buffer.len() as u64;
        let encoded = self.codec.encode(embedding)?;
        self.offsets.push(offset);
        self.buffer.extend_from_slice(&encoded);
        Ok(())
    }

    /// Get number of embeddings in batch.
    #[inline]
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// Check if batch is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Get current buffer size in bytes.
    #[inline]
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Finalize and get bytes with offset table.
    ///
    /// Returns (data_bytes, offsets) tuple. Note that offsets are NOT page-aligned;
    /// use `write_gds_file` for page-aligned output.
    pub fn finalize(self) -> (Vec<u8>, Vec<u64>) {
        (self.buffer, self.offsets)
    }

    /// Write GDS-compatible files: .cgeb (data) and .cgei (index).
    ///
    /// Data file is 4KB-page aligned for optimal GDS performance.
    ///
    /// # File Layout
    ///
    /// ## Data File (.cgeb)
    /// Each embedding starts at a 4KB page boundary:
    /// ```text
    /// [0x0000] Embedding 0 (6242 bytes) + padding to 4KB
    /// [0x1000] Embedding 1 (6242 bytes) + padding to 4KB
    /// [0x2000] ...
    /// ```
    ///
    /// ## Index File (.cgei)
    /// ```text
    /// [0x00] EmbeddingIndexHeader (24 bytes)
    /// [0x18] Offset table: entry_count × u64 (big-endian)
    /// ```
    ///
    /// # Errors
    /// - `io::Error` on file write failure
    pub fn write_gds_file(&self, path: &Path) -> io::Result<()> {
        if self.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot write empty batch",
            ));
        }

        // Data file
        let data_path = path.with_extension("cgeb");
        let mut data_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&data_path)?;

        // Recompute offsets with 4KB page alignment
        let mut aligned_offsets = Vec::with_capacity(self.offsets.len());
        let mut current_offset = 0u64;

        // Write each embedding with page alignment
        for (i, original_offset) in self.offsets.iter().enumerate() {
            // Calculate embedding size
            let end_offset = if i + 1 < self.offsets.len() {
                self.offsets[i + 1]
            } else {
                self.buffer.len() as u64
            };
            let embedding_size = (end_offset - original_offset) as usize;

            // Pad to page boundary (except for first entry)
            if i > 0 {
                let padding =
                    (Self::PAGE_SIZE - (current_offset as usize % Self::PAGE_SIZE)) % Self::PAGE_SIZE;
                if padding > 0 {
                    data_file.write_all(&vec![0u8; padding])?;
                    current_offset += padding as u64;
                }
            }

            aligned_offsets.push(current_offset);

            // Write embedding data
            let start = *original_offset as usize;
            let end = start + embedding_size;
            data_file.write_all(&self.buffer[start..end])?;
            current_offset += embedding_size as u64;
        }

        // Compute data file hash
        let data_hash = xxhash_rust::xxh64::xxh64(&self.buffer, 0);

        // Index file
        let index_path = path.with_extension("cgei");
        let mut index_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&index_path)?;

        let index_header = EmbeddingIndexHeader {
            magic: INDEX_MAGIC,
            version: INDEX_VERSION.to_be(),
            _reserved: 0,
            entry_count: (aligned_offsets.len() as u64).to_be(),
            data_file_hash: data_hash.to_be(),
        };

        index_file.write_all(bytes_of(&index_header))?;

        // Write offset table (big-endian)
        for &offset in &aligned_offsets {
            index_file.write_all(&offset.to_be_bytes())?;
        }

        Ok(())
    }

    /// Write unaligned files (faster but not GDS-optimized).
    ///
    /// Useful for testing or systems without GDS requirements.
    ///
    /// # Errors
    /// - `io::Error` on file write failure
    pub fn write_unaligned(&self, path: &Path) -> io::Result<()> {
        if self.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot write empty batch",
            ));
        }

        // Data file (no padding)
        let data_path = path.with_extension("cgeb");
        let mut data_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&data_path)?;

        data_file.write_all(&self.buffer)?;

        // Compute data file hash
        let data_hash = xxhash_rust::xxh64::xxh64(&self.buffer, 0);

        // Index file
        let index_path = path.with_extension("cgei");
        let mut index_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&index_path)?;

        let index_header = EmbeddingIndexHeader {
            magic: INDEX_MAGIC,
            version: INDEX_VERSION.to_be(),
            _reserved: 0,
            entry_count: (self.offsets.len() as u64).to_be(),
            data_file_hash: data_hash.to_be(),
        };

        index_file.write_all(bytes_of(&index_header))?;

        // Write offset table (big-endian)
        for &offset in &self.offsets {
            index_file.write_all(&offset.to_be_bytes())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::dimensions::{FUSED_OUTPUT, NUM_EXPERTS};
    use crate::types::FusedEmbedding;
    use std::fs;
    use tempfile::tempdir;

    fn make_test_embedding(hash: u64) -> FusedEmbedding {
        FusedEmbedding::new(
            vec![0.1; FUSED_OUTPUT],
            [0.125; NUM_EXPERTS],
            [0, 1],
            1000,
            hash,
        )
        .expect("test embedding creation")
    }

    #[test]
    fn test_index_header_is_24_bytes() {
        let size = std::mem::size_of::<EmbeddingIndexHeader>();
        println!("BEFORE: Expected size = 24 bytes");
        println!("AFTER: Actual size = {} bytes", size);
        assert_eq!(size, 24);
        println!("PASSED: EmbeddingIndexHeader is exactly 24 bytes");
    }

    #[test]
    fn test_batch_encoder_push() {
        let mut encoder = BatchBinaryEncoder::with_capacity(10);

        println!("BEFORE: encoder.len() = {}", encoder.len());

        for i in 0..5 {
            let embedding = make_test_embedding(i as u64);
            encoder.push(&embedding).expect("push should succeed");
        }

        println!("AFTER: encoder.len() = {}", encoder.len());
        assert_eq!(encoder.len(), 5);
        println!("PASSED: batch encoder accumulates embeddings");
    }

    #[test]
    fn test_batch_encoder_buffer_size() {
        let mut encoder = BatchBinaryEncoder::with_capacity(10);

        let embedding = make_test_embedding(0);
        encoder.push(&embedding).expect("push");

        println!("BEFORE: expected buffer_size = 6242");
        println!("AFTER: actual buffer_size = {}", encoder.buffer_size());
        assert_eq!(encoder.buffer_size(), 6242);
        println!("PASSED: buffer size matches single embedding");
    }

    #[test]
    fn test_batch_encoder_finalize() {
        let mut encoder = BatchBinaryEncoder::with_capacity(10);

        for i in 0..3 {
            let embedding = make_test_embedding(i as u64);
            encoder.push(&embedding).expect("push");
        }

        let (buffer, offsets) = encoder.finalize();

        println!("BEFORE: expected 3 offsets");
        println!("AFTER: got {} offsets", offsets.len());
        println!("AFTER: offsets = {:?}", offsets);
        println!("AFTER: buffer.len() = {}", buffer.len());

        assert_eq!(offsets.len(), 3);
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 6242);
        assert_eq!(offsets[2], 12484);
        assert_eq!(buffer.len(), 6242 * 3);
        println!("PASSED: finalize returns correct buffer and offsets");
    }

    #[test]
    fn test_batch_encoder_write_gds_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_batch");

        let mut encoder = BatchBinaryEncoder::with_capacity(10);
        for i in 0..3 {
            let embedding = make_test_embedding(i as u64);
            encoder.push(&embedding).expect("push");
        }

        encoder.write_gds_file(&path).expect("write_gds_file");

        // Verify files exist
        let data_path = path.with_extension("cgeb");
        let index_path = path.with_extension("cgei");

        println!("BEFORE: expecting .cgeb and .cgei files");
        println!("AFTER: data file exists = {}", data_path.exists());
        println!("AFTER: index file exists = {}", index_path.exists());

        assert!(data_path.exists());
        assert!(index_path.exists());

        // Verify index file content
        let index_bytes = fs::read(&index_path).expect("read index");
        println!("AFTER: index file size = {} bytes", index_bytes.len());

        // Header (24) + 3 offsets (3 * 8 = 24) = 48 bytes
        assert_eq!(index_bytes.len(), 48);

        // Verify magic
        assert_eq!(&index_bytes[0..4], &INDEX_MAGIC);
        println!("PASSED: write_gds_file creates valid files");
    }

    #[test]
    fn test_batch_encoder_page_alignment() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_aligned");

        let mut encoder = BatchBinaryEncoder::with_capacity(10);
        for i in 0..3 {
            let embedding = make_test_embedding(i as u64);
            encoder.push(&embedding).expect("push");
        }

        encoder.write_gds_file(&path).expect("write_gds_file");

        // Read index to get offsets
        let index_path = path.with_extension("cgei");
        let index_bytes = fs::read(&index_path).expect("read index");

        // Parse offsets (skip 24-byte header)
        let offset_0 = u64::from_be_bytes(index_bytes[24..32].try_into().unwrap());
        let offset_1 = u64::from_be_bytes(index_bytes[32..40].try_into().unwrap());
        let offset_2 = u64::from_be_bytes(index_bytes[40..48].try_into().unwrap());

        println!("BEFORE: expecting page-aligned offsets");
        println!("AFTER: offset_0 = {} (expect 0)", offset_0);
        println!("AFTER: offset_1 = {} (expect 8192)", offset_1);
        println!("AFTER: offset_2 = {} (expect 16384)", offset_2);

        // First embedding at offset 0, second at 8192 (2 pages), third at 16384
        assert_eq!(offset_0, 0);
        assert_eq!(offset_1 % 4096, 0, "offset_1 should be page-aligned");
        assert_eq!(offset_2 % 4096, 0, "offset_2 should be page-aligned");
        println!("PASSED: offsets are 4KB page-aligned");
    }

    #[test]
    fn test_batch_encoder_write_unaligned() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_unaligned");

        let mut encoder = BatchBinaryEncoder::with_capacity(10);
        for i in 0..3 {
            let embedding = make_test_embedding(i as u64);
            encoder.push(&embedding).expect("push");
        }

        encoder.write_unaligned(&path).expect("write_unaligned");

        let data_path = path.with_extension("cgeb");
        let data_bytes = fs::read(&data_path).expect("read data");

        println!("BEFORE: expecting compact data file (no padding)");
        println!("AFTER: data file size = {} bytes", data_bytes.len());

        // 3 embeddings × 6242 bytes = 18726 bytes (no padding)
        assert_eq!(data_bytes.len(), 6242 * 3);
        println!("PASSED: write_unaligned produces compact file");
    }

    #[test]
    fn test_batch_encoder_empty_fails() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_empty");

        let encoder = BatchBinaryEncoder::with_capacity(10);

        let result = encoder.write_gds_file(&path);

        println!("BEFORE: attempting to write empty batch");
        println!("AFTER: result = {:?}", result);

        assert!(result.is_err());
        println!("PASSED: empty batch write fails fast");
    }

    #[test]
    fn test_batch_encoder_is_empty() {
        let encoder = BatchBinaryEncoder::with_capacity(10);

        println!("BEFORE: new encoder");
        println!("AFTER: is_empty() = {}", encoder.is_empty());

        assert!(encoder.is_empty());
        println!("PASSED: new encoder is empty");
    }

    #[test]
    fn test_batch_encoder_magic_in_data_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_magic");

        let mut encoder = BatchBinaryEncoder::with_capacity(10);
        encoder
            .push(&make_test_embedding(42))
            .expect("push");

        encoder.write_gds_file(&path).expect("write");

        let data_path = path.with_extension("cgeb");
        let data_bytes = fs::read(&data_path).expect("read");

        println!("BEFORE: expecting CGEB magic at start of data file");
        println!(
            "AFTER: first 4 bytes = {:02x?}",
            &data_bytes[0..4]
        );

        assert_eq!(&data_bytes[0..4], &crate::storage::binary::EMBEDDING_MAGIC);
        println!("PASSED: data file starts with CGEB magic");
    }

    #[test]
    fn test_batch_encoder_with_aux_data() {
        let encoder = BatchBinaryEncoder::with_aux_data(10);

        println!("BEFORE: created encoder with aux_data support");
        println!("AFTER: encoder.is_empty() = {}", encoder.is_empty());

        assert!(encoder.is_empty());
        println!("PASSED: encoder with aux_data initializes correctly");
    }
}
