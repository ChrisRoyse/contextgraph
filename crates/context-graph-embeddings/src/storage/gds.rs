//! GDS file reader for batch embeddings.
//!
//! Provides O(1) seeking to any embedding by index using the index file.
//!
//! # Example
//!
//! ```rust,ignore
//! use context_graph_embeddings::storage::GdsFile;
//!
//! let mut gds = GdsFile::open(Path::new("embeddings"))?;
//! println!("File contains {} embeddings", gds.len());
//!
//! // O(1) random access
//! let embedding = gds.read(42)?;
//! println!("Content hash: {:#x}", embedding.content_hash);
//! ```

use super::batch::{EmbeddingIndexHeader, INDEX_MAGIC};
use super::binary::{DecodeError, EmbeddingBinaryCodec};
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
                    index, bytes_read, EmbeddingBinaryCodec::MIN_BUFFER_SIZE
                ),
            )));
        }

        self.codec.decode(&buffer[..bytes_read]).map_err(GdsFileError::Decode)
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
        GdsFileIter {
            file: self,
            current: 0,
        }
    }
}

/// Iterator over embeddings in a GDS file.
pub struct GdsFileIter<'a> {
    file: &'a mut GdsFile,
    current: usize,
}

impl<'a> Iterator for GdsFileIter<'a> {
    type Item = Result<FusedEmbedding, GdsFileError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.file.len() {
            return None;
        }
        let result = self.file.read(self.current);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.file.len() - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for GdsFileIter<'a> {}

/// Errors for GDS file operations.
#[derive(Debug, thiserror::Error)]
pub enum GdsFileError {
    #[error("Invalid index file magic bytes: expected 'CGEI' (0x43474549)")]
    InvalidIndexMagic,

    #[error("Index out of bounds: {index} >= {len}")]
    IndexOutOfBounds { index: usize, len: usize },

    #[error("Decode error: {0}")]
    Decode(#[from] DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BatchBinaryEncoder;
    use crate::types::dimensions::{FUSED_OUTPUT, NUM_EXPERTS};
    use crate::types::FusedEmbedding;
    use tempfile::tempdir;

    fn make_test_embedding(hash: u64) -> FusedEmbedding {
        FusedEmbedding::new(
            vec![0.1 * hash as f32; FUSED_OUTPUT],
            [0.125; NUM_EXPERTS],
            [0, 1],
            hash * 100,
            hash,
        )
        .expect("test embedding creation")
    }

    fn create_test_gds_files(path: &Path, count: usize) {
        let mut encoder = BatchBinaryEncoder::with_capacity(count);
        for i in 0..count {
            encoder.push(&make_test_embedding(i as u64)).expect("push");
        }
        encoder.write_gds_file(path).expect("write_gds_file");
    }

    #[test]
    fn test_gds_file_open() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_gds");

        create_test_gds_files(&path, 5);

        let gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: created GDS files with 5 embeddings");
        println!("AFTER: gds.len() = {}", gds.len());

        assert_eq!(gds.len(), 5);
        println!("PASSED: GdsFile opens correctly");
    }

    #[test]
    fn test_gds_file_read() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_read");

        create_test_gds_files(&path, 5);

        let mut gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: reading embedding at index 2");

        let embedding = gds.read(2).expect("read");

        println!("AFTER: content_hash = {:#x}", embedding.content_hash);
        println!("AFTER: pipeline_latency_us = {}", embedding.pipeline_latency_us);

        assert_eq!(embedding.content_hash, 2);
        assert_eq!(embedding.pipeline_latency_us, 200);
        println!("PASSED: GdsFile reads correct embedding");
    }

    #[test]
    fn test_gds_file_read_all() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_read_all");

        create_test_gds_files(&path, 10);

        let mut gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: reading all 10 embeddings");

        for i in 0..10 {
            let embedding = gds.read(i).expect(&format!("read {}", i));
            assert_eq!(
                embedding.content_hash, i as u64,
                "embedding {} has wrong hash",
                i
            );
        }

        println!("AFTER: all 10 embeddings read successfully");
        println!("PASSED: GdsFile reads all embeddings correctly");
    }

    #[test]
    fn test_gds_file_out_of_bounds() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_oob");

        create_test_gds_files(&path, 5);

        let mut gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: attempting to read index 10 (out of bounds)");

        let result = gds.read(10);

        println!("AFTER: result = {:?}", result);

        match result {
            Err(GdsFileError::IndexOutOfBounds { index, len }) => {
                assert_eq!(index, 10);
                assert_eq!(len, 5);
            }
            _ => panic!("Expected IndexOutOfBounds"),
        }
        println!("PASSED: GdsFile fails fast on out-of-bounds");
    }

    #[test]
    fn test_gds_file_invalid_magic() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_bad_magic");

        // Create valid files first
        create_test_gds_files(&path, 5);

        // Corrupt the index file magic
        let index_path = path.with_extension("cgei");
        let mut bytes = std::fs::read(&index_path).expect("read index");
        bytes[0..4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        std::fs::write(&index_path, bytes).expect("write corrupted");

        println!("BEFORE: corrupted index magic bytes");

        let result = GdsFile::open(&path);

        println!("AFTER: result = {:?}", result);

        assert!(matches!(result, Err(GdsFileError::InvalidIndexMagic)));
        println!("PASSED: GdsFile fails fast on invalid magic");
    }

    #[test]
    fn test_gds_file_is_empty() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_empty_check");

        create_test_gds_files(&path, 3);

        let gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: file with 3 embeddings");
        println!("AFTER: is_empty() = {}", gds.is_empty());

        assert!(!gds.is_empty());
        println!("PASSED: is_empty returns false for non-empty file");
    }

    #[test]
    fn test_gds_file_get_offset() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_offset");

        create_test_gds_files(&path, 5);

        let gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: getting offsets for 5 embeddings");

        let offset_0 = gds.get_offset(0).expect("offset 0");
        let offset_1 = gds.get_offset(1).expect("offset 1");

        println!("AFTER: offset_0 = {}", offset_0);
        println!("AFTER: offset_1 = {}", offset_1);

        assert_eq!(offset_0, 0);
        // offset_1 should be page-aligned (4096 or 8192)
        assert!(offset_1 % 4096 == 0, "offset_1 should be page-aligned");
        println!("PASSED: get_offset returns correct values");
    }

    #[test]
    fn test_gds_file_read_batch() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_batch_read");

        create_test_gds_files(&path, 10);

        let mut gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: batch reading indices [0, 5, 9]");

        let embeddings = gds.read_batch(&[0, 5, 9]).expect("read_batch");

        println!("AFTER: got {} embeddings", embeddings.len());
        println!("AFTER: hashes = {:?}",
            embeddings.iter().map(|e| e.content_hash).collect::<Vec<_>>());

        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].content_hash, 0);
        assert_eq!(embeddings[1].content_hash, 5);
        assert_eq!(embeddings[2].content_hash, 9);
        println!("PASSED: read_batch returns correct embeddings");
    }

    #[test]
    fn test_gds_file_iter() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_iter");

        create_test_gds_files(&path, 5);

        let mut gds = GdsFile::open(&path).expect("open");

        println!("BEFORE: iterating over 5 embeddings");

        let embeddings: Vec<_> = gds.iter().collect::<Result<_, _>>().expect("iter");

        println!("AFTER: collected {} embeddings", embeddings.len());

        assert_eq!(embeddings.len(), 5);
        for (i, e) in embeddings.iter().enumerate() {
            assert_eq!(e.content_hash, i as u64);
        }
        println!("PASSED: iter yields all embeddings in order");
    }

    #[test]
    fn test_gds_file_iter_size_hint() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_iter_size");

        create_test_gds_files(&path, 5);

        let mut gds = GdsFile::open(&path).expect("open");
        let iter = gds.iter();

        println!("BEFORE: iterator size_hint");
        println!("AFTER: size_hint = {:?}", iter.size_hint());

        assert_eq!(iter.size_hint(), (5, Some(5)));
        println!("PASSED: iter size_hint is correct");
    }

    #[test]
    fn test_gds_file_data_file_hash() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_hash");

        create_test_gds_files(&path, 3);

        let gds = GdsFile::open(&path).expect("open");
        let hash = gds.data_file_hash();

        println!("BEFORE: checking data file hash");
        println!("AFTER: data_file_hash = {:#x}", hash);

        // Hash should be non-zero for non-empty data
        assert_ne!(hash, 0);
        println!("PASSED: data_file_hash is non-zero");
    }

    #[test]
    fn test_gds_file_missing_data_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_missing_data");

        // Create only index file (simulate missing data file)
        create_test_gds_files(&path, 3);
        std::fs::remove_file(path.with_extension("cgeb")).expect("remove data file");

        println!("BEFORE: opening with missing data file");

        let result = GdsFile::open(&path);

        println!("AFTER: result = {:?}", result);

        assert!(matches!(result, Err(GdsFileError::Io(_))));
        println!("PASSED: fails on missing data file");
    }

    #[test]
    fn test_gds_file_missing_index_file() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test_missing_index");

        // Create only data file (simulate missing index file)
        create_test_gds_files(&path, 3);
        std::fs::remove_file(path.with_extension("cgei")).expect("remove index file");

        println!("BEFORE: opening with missing index file");

        let result = GdsFile::open(&path);

        println!("AFTER: result = {:?}", result);

        assert!(matches!(result, Err(GdsFileError::Io(_))));
        println!("PASSED: fails on missing index file");
    }
}
