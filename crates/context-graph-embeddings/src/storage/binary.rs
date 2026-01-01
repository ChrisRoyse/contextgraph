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
//! | 6240 | 2 | SelectedExperts: [u8; 2] |
//! | 6242 | var | AuxData (if present) |
//!
//! # Example
//!
//! ```rust,ignore
//! use context_graph_embeddings::storage::{EmbeddingBinaryCodec, EMBEDDING_MAGIC};
//! use context_graph_embeddings::FusedEmbedding;
//!
//! let codec = EmbeddingBinaryCodec::new();
//! let bytes = codec.encode(&embedding)?;
//! assert_eq!(&bytes[0..4], &EMBEDDING_MAGIC);
//!
//! let decoded = codec.decode(&bytes)?;
//! assert_eq!(decoded.content_hash, embedding.content_hash);
//! ```

use crate::types::dimensions::{FUSED_OUTPUT, NUM_EXPERTS, TOP_K_EXPERTS};
use crate::types::{AuxiliaryEmbeddingData, FusedEmbedding};
use bytemuck::{bytes_of, try_from_bytes, Pod, Zeroable};
use std::fs::File;
use std::io::{self, Write};

/// Binary format version. Increment when format changes.
/// Version 1: Initial GDS-compatible format with 64-byte header.
pub const EMBEDDING_BINARY_VERSION: u16 = 1;

/// Magic bytes for file identification: "CGEB" = Context Graph Embedding Binary
pub const EMBEDDING_MAGIC: [u8; 4] = [0x43, 0x47, 0x45, 0x42];

/// Fixed-size binary header (64 bytes, cache-line aligned).
/// MUST remain exactly 64 bytes for GDS compatibility.
///
/// Layout (all multi-byte values stored big-endian in the encoded stream):
/// - [0..4] magic: "CGEB"
/// - [4..6] version: u16
/// - [6..8] flags: u16
/// - [8..12] dimension: u32
/// - [12] num_experts: u8
/// - [13] top_k: u8
/// - [14..16] reserved: [u8; 2]
/// - [16..24] content_hash: u64
/// - [24..32] pipeline_latency_us: u64
/// - [32..40] aux_data_offset: u64
/// - [40..48] aux_data_length: u64
/// - [48..64] padding: [u8; 16]
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EmbeddingHeader {
    /// Magic bytes: "CGEB" (0x43 0x47 0x45 0x42)
    pub magic: [u8; 4],
    /// Format version (big-endian)
    pub version: u16,
    /// Flags: bit 0 = has_aux_data, bit 1 = compressed_aux, bits 2-15 reserved
    pub flags: u16,
    /// Vector dimension (1536 for FusedEmbedding)
    pub dimension: u32,
    /// Number of experts (8)
    pub num_experts: u8,
    /// Top-K experts selected (2)
    pub top_k: u8,
    /// Reserved for future use
    pub _reserved: [u8; 2],
    /// Content hash (xxHash64) for integrity verification
    pub content_hash: u64,
    /// Pipeline latency in microseconds
    pub pipeline_latency_us: u64,
    /// Auxiliary data offset from start of record (0 if none)
    pub aux_data_offset: u64,
    /// Auxiliary data length in bytes (0 if none)
    pub aux_data_length: u64,
    /// Padding to reach exactly 64 bytes
    pub _padding: [u8; 16],
}

// Compile-time assertion: header must be exactly 64 bytes
const _HEADER_SIZE_CHECK: () = assert!(
    std::mem::size_of::<EmbeddingHeader>() == 64,
    "EmbeddingHeader must be exactly 64 bytes"
);


/// Compression type for auxiliary data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    /// No compression (raw bytes)
    None,
    /// LZ4 fast compression (not implemented in v1)
    Lz4,
    /// Zstd compression (not implemented in v1)
    Zstd,
}

/// Binary encoder/decoder for FusedEmbedding.
///
/// Produces GDS-compatible format:
/// - 64-byte aligned header
/// - Big-endian floats for cross-platform compatibility
/// - Zero-copy decode via memory mapping
#[derive(Debug)]
pub struct EmbeddingBinaryCodec {
    /// Include auxiliary ColBERT data in output
    include_aux_data: bool,
    /// Compression for aux_data (v1 only supports None)
    #[allow(dead_code)]
    aux_compression: CompressionType,
}

impl EmbeddingBinaryCodec {
    /// Minimum buffer size without aux_data.
    /// Header(64) + Vector(6144) + Weights(32) + Selected(2) = 6242 bytes
    pub const MIN_BUFFER_SIZE: usize = 64 + (FUSED_OUTPUT * 4) + (NUM_EXPERTS * 4) + TOP_K_EXPERTS;

    /// Create codec with default settings (no aux_data).
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_aux_data: false,
            aux_compression: CompressionType::None,
        }
    }

    /// Create codec with auxiliary data support.
    #[must_use]
    pub fn with_aux_data(compression: CompressionType) -> Self {
        Self {
            include_aux_data: true,
            aux_compression: compression,
        }
    }

    /// Encode FusedEmbedding to GDS-compatible bytes.
    ///
    /// # Binary Layout
    /// | Offset | Size | Field |
    /// |--------|------|-------|
    /// | 0 | 64 | EmbeddingHeader (cache-line aligned) |
    /// | 64 | 6144 | Vector: [f32; 1536] big-endian |
    /// | 6208 | 32 | ExpertWeights: [f32; 8] big-endian |
    /// | 6240 | 2 | SelectedExperts: [u8; 2] |
    /// | 6242 | var | AuxData (if present) |
    ///
    /// # Errors
    /// - `EncodeError::InvalidDimension` if vector dimension != 1536
    pub fn encode(&self, embedding: &FusedEmbedding) -> Result<Vec<u8>, EncodeError> {
        // Validate dimension - FAIL FAST
        if embedding.vector.len() != FUSED_OUTPUT {
            return Err(EncodeError::InvalidDimension {
                expected: FUSED_OUTPUT,
                actual: embedding.vector.len(),
            });
        }

        // Prepare aux_data if requested
        let aux_blob = if self.include_aux_data {
            embedding.aux_data.as_ref().map(|a| a.to_blob())
        } else {
            None
        };

        let aux_offset = if aux_blob.is_some() {
            Self::MIN_BUFFER_SIZE as u64
        } else {
            0
        };
        let aux_length = aux_blob.as_ref().map(|b| b.len() as u64).unwrap_or(0);

        // Build header
        let mut flags: u16 = 0;
        if aux_blob.is_some() {
            flags |= 0x01; // bit 0: has_aux_data
        }

        let header = EmbeddingHeader {
            magic: EMBEDDING_MAGIC,
            version: EMBEDDING_BINARY_VERSION.to_be(),
            flags: flags.to_be(),
            dimension: (FUSED_OUTPUT as u32).to_be(),
            num_experts: NUM_EXPERTS as u8,
            top_k: TOP_K_EXPERTS as u8,
            _reserved: [0; 2],
            content_hash: embedding.content_hash.to_be(),
            pipeline_latency_us: embedding.pipeline_latency_us.to_be(),
            aux_data_offset: aux_offset.to_be(),
            aux_data_length: aux_length.to_be(),
            _padding: [0; 16],
        };

        // Allocate buffer
        let total_size = Self::MIN_BUFFER_SIZE + aux_length as usize;
        let mut buffer = Vec::with_capacity(total_size);

        // Header (64 bytes)
        buffer.extend_from_slice(bytes_of(&header));

        // Vector (6144 bytes) - big-endian
        for &val in &embedding.vector {
            buffer.extend_from_slice(&val.to_be_bytes());
        }

        // Expert weights (32 bytes) - big-endian
        for &weight in &embedding.expert_weights {
            buffer.extend_from_slice(&weight.to_be_bytes());
        }

        // Selected experts (2 bytes)
        buffer.extend_from_slice(&embedding.selected_experts);

        // Auxiliary data (if present)
        if let Some(aux) = aux_blob {
            buffer.extend_from_slice(&aux);
        }

        debug_assert_eq!(buffer.len(), total_size, "Buffer size mismatch");
        Ok(buffer)
    }

    /// Encode directly to pre-allocated buffer (zero-copy write).
    ///
    /// # Errors
    /// - `EncodeError::BufferTooSmall` if buffer is too small
    /// - `EncodeError::InvalidDimension` if vector dimension != 1536
    pub fn encode_to_buffer(
        &self,
        embedding: &FusedEmbedding,
        buffer: &mut [u8],
    ) -> Result<usize, EncodeError> {
        let encoded = self.encode(embedding)?;
        if buffer.len() < encoded.len() {
            return Err(EncodeError::BufferTooSmall {
                needed: encoded.len(),
                available: buffer.len(),
            });
        }
        buffer[..encoded.len()].copy_from_slice(&encoded);
        Ok(encoded.len())
    }

    /// Encode to file.
    ///
    /// # Errors
    /// - `EncodeError::Io` on file write failure
    pub fn encode_to_file(
        &self,
        embedding: &FusedEmbedding,
        file: &mut File,
    ) -> Result<u64, EncodeError> {
        let encoded = self.encode(embedding)?;
        file.write_all(&encoded)?;
        Ok(encoded.len() as u64)
    }

    /// Decode FusedEmbedding from bytes.
    ///
    /// # Errors
    /// - `DecodeError::BufferTooShort` if bytes < MIN_BUFFER_SIZE
    /// - `DecodeError::InvalidMagic` if magic bytes don't match
    /// - `DecodeError::UnsupportedVersion` if version > current
    /// - `DecodeError::HashMismatch` if verify_hash=true and hash doesn't match
    pub fn decode(&self, bytes: &[u8]) -> Result<FusedEmbedding, DecodeError> {
        // Validate minimum size - FAIL FAST
        if bytes.len() < Self::MIN_BUFFER_SIZE {
            return Err(DecodeError::BufferTooShort {
                needed: Self::MIN_BUFFER_SIZE,
                available: bytes.len(),
            });
        }

        // Parse header
        let header = self.decode_header(bytes)?;

        // Parse vector (big-endian)
        let mut vector = Vec::with_capacity(FUSED_OUTPUT);
        for i in 0..FUSED_OUTPUT {
            let offset = 64 + i * 4;
            let val = f32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            vector.push(val);
        }

        // Parse expert weights (big-endian)
        let mut expert_weights = [0.0f32; NUM_EXPERTS];
        for i in 0..NUM_EXPERTS {
            let offset = 64 + (FUSED_OUTPUT * 4) + i * 4;
            expert_weights[i] = f32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
        }

        // Parse selected experts
        let selected_offset = 64 + (FUSED_OUTPUT * 4) + (NUM_EXPERTS * 4);
        let selected_experts = [bytes[selected_offset], bytes[selected_offset + 1]];

        // Parse aux_data if present
        let aux_data_offset = u64::from_be(header.aux_data_offset) as usize;
        let aux_data_length = u64::from_be(header.aux_data_length) as usize;

        let aux_data = if aux_data_offset > 0 && aux_data_length > 0 {
            let end = aux_data_offset + aux_data_length;
            if bytes.len() < end {
                return Err(DecodeError::BufferTooShort {
                    needed: end,
                    available: bytes.len(),
                });
            }
            Some(
                AuxiliaryEmbeddingData::from_blob(&bytes[aux_data_offset..end])
                    .map_err(|e| DecodeError::AuxDataCorrupted(e.to_string()))?,
            )
        } else {
            None
        };

        Ok(FusedEmbedding {
            vector,
            expert_weights,
            selected_experts,
            pipeline_latency_us: u64::from_be(header.pipeline_latency_us),
            content_hash: u64::from_be(header.content_hash),
            aux_data,
        })
    }

    /// Decode header only (for seeking/filtering).
    ///
    /// # Errors
    /// - `DecodeError::BufferTooShort` if bytes < 64
    /// - `DecodeError::InvalidMagic` if magic bytes don't match
    /// - `DecodeError::UnsupportedVersion` if version > current
    pub fn decode_header(&self, bytes: &[u8]) -> Result<EmbeddingHeader, DecodeError> {
        if bytes.len() < 64 {
            return Err(DecodeError::BufferTooShort {
                needed: 64,
                available: bytes.len(),
            });
        }

        let header: &EmbeddingHeader =
            try_from_bytes(&bytes[0..64]).map_err(|_| DecodeError::InvalidMagic)?;

        // Validate magic - FAIL FAST
        if header.magic != EMBEDDING_MAGIC {
            return Err(DecodeError::InvalidMagic);
        }

        // Validate version - FAIL FAST
        let version = u16::from_be(header.version);
        if version > EMBEDDING_BINARY_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }

        Ok(*header)
    }

    /// Decode with zero-copy reference to memory-mapped buffer.
    ///
    /// Returns a borrowed view into the buffer - no heap allocation for the
    /// core data. Note that accessing vector/weights requires byte-swapping
    /// from big-endian.
    ///
    /// # Errors
    /// - Same as `decode_header`
    /// - `DecodeError::AlignmentError` if buffer not 64-byte aligned
    pub fn decode_zero_copy<'a>(
        &self,
        bytes: &'a [u8],
    ) -> Result<FusedEmbeddingRef<'a>, DecodeError> {
        // Validate alignment for zero-copy
        if bytes.as_ptr() as usize % 64 != 0 {
            return Err(DecodeError::AlignmentError {
                expected: 64,
                actual: bytes.as_ptr() as usize % 64,
            });
        }

        self.decode_header(bytes)?;

        // Validate buffer size
        if bytes.len() < Self::MIN_BUFFER_SIZE {
            return Err(DecodeError::BufferTooShort {
                needed: Self::MIN_BUFFER_SIZE,
                available: bytes.len(),
            });
        }

        // Create references into buffer (zero-copy)
        let header_ref: &EmbeddingHeader =
            try_from_bytes(&bytes[0..64]).map_err(|_| DecodeError::InvalidMagic)?;

        // Vector bytes: 64..6208 (borrowed slice, conversion happens on access)
        let vector_bytes = &bytes[64..64 + FUSED_OUTPUT * 4];

        // Expert weights bytes: 6208..6240
        let weights_bytes =
            &bytes[64 + FUSED_OUTPUT * 4..64 + FUSED_OUTPUT * 4 + NUM_EXPERTS * 4];

        // Selected experts: bytes 6240..6242
        let selected_offset = 64 + FUSED_OUTPUT * 4 + NUM_EXPERTS * 4;
        let selected_bytes = &bytes[selected_offset..selected_offset + TOP_K_EXPERTS];

        // Aux data reference (if present)
        let aux_data_offset = u64::from_be(header_ref.aux_data_offset) as usize;
        let aux_data_length = u64::from_be(header_ref.aux_data_length) as usize;
        let aux_data = if aux_data_offset > 0 && aux_data_length > 0 {
            if bytes.len() < aux_data_offset + aux_data_length {
                return Err(DecodeError::BufferTooShort {
                    needed: aux_data_offset + aux_data_length,
                    available: bytes.len(),
                });
            }
            Some(&bytes[aux_data_offset..aux_data_offset + aux_data_length])
        } else {
            None
        };

        Ok(FusedEmbeddingRef {
            header: header_ref,
            vector_bytes,
            weights_bytes,
            selected_bytes,
            aux_data,
        })
    }

    /// Compute serialized size for an embedding.
    #[must_use]
    pub fn serialized_size(&self, embedding: &FusedEmbedding) -> usize {
        let aux_size = if self.include_aux_data {
            embedding
                .aux_data
                .as_ref()
                .map(|a| a.to_blob().len())
                .unwrap_or(0)
        } else {
            0
        };
        Self::MIN_BUFFER_SIZE + aux_size
    }
}

impl Default for EmbeddingBinaryCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Zero-copy reference to FusedEmbedding in memory-mapped buffer.
///
/// All data is borrowed from the underlying buffer - no heap allocation.
/// NOTE: Vector values are big-endian and need byte-swapping on read.
pub struct FusedEmbeddingRef<'a> {
    header: &'a EmbeddingHeader,
    /// Raw vector bytes (big-endian f32s), use `vector()` method for conversion
    vector_bytes: &'a [u8],
    /// Raw weights bytes (big-endian f32s), use `expert_weights()` method for conversion
    weights_bytes: &'a [u8],
    /// Selected experts bytes
    selected_bytes: &'a [u8],
    aux_data: Option<&'a [u8]>,
}

impl<'a> FusedEmbeddingRef<'a> {
    /// Get vector with byte-swapping from big-endian.
    ///
    /// This allocates a new array but does not allocate on heap
    /// (stack allocation for the fixed-size array).
    pub fn vector(&self) -> [f32; FUSED_OUTPUT] {
        let mut result = [0.0f32; FUSED_OUTPUT];
        for i in 0..FUSED_OUTPUT {
            let offset = i * 4;
            result[i] = f32::from_be_bytes([
                self.vector_bytes[offset],
                self.vector_bytes[offset + 1],
                self.vector_bytes[offset + 2],
                self.vector_bytes[offset + 3],
            ]);
        }
        result
    }

    /// Get vector as Vec<f32> (heap allocation).
    pub fn vector_vec(&self) -> Vec<f32> {
        self.vector().to_vec()
    }

    /// Get expert weights with byte-swapping from big-endian.
    pub fn expert_weights(&self) -> [f32; NUM_EXPERTS] {
        let mut result = [0.0f32; NUM_EXPERTS];
        for i in 0..NUM_EXPERTS {
            let offset = i * 4;
            result[i] = f32::from_be_bytes([
                self.weights_bytes[offset],
                self.weights_bytes[offset + 1],
                self.weights_bytes[offset + 2],
                self.weights_bytes[offset + 3],
            ]);
        }
        result
    }

    /// Get selected experts.
    #[inline]
    pub fn selected_experts(&self) -> [u8; TOP_K_EXPERTS] {
        [self.selected_bytes[0], self.selected_bytes[1]]
    }

    /// Convert to owned FusedEmbedding (allocates).
    pub fn to_owned(&self) -> FusedEmbedding {
        let vector = self.vector().to_vec();
        let aux_data = self
            .aux_data
            .and_then(|blob| AuxiliaryEmbeddingData::from_blob(blob).ok());

        FusedEmbedding {
            vector,
            expert_weights: self.expert_weights(),
            selected_experts: self.selected_experts(),
            pipeline_latency_us: u64::from_be(self.header.pipeline_latency_us),
            content_hash: u64::from_be(self.header.content_hash),
            aux_data,
        }
    }

    /// Get content hash.
    #[inline]
    pub fn content_hash(&self) -> u64 {
        u64::from_be(self.header.content_hash)
    }

    /// Get pipeline latency in microseconds.
    #[inline]
    pub fn pipeline_latency_us(&self) -> u64 {
        u64::from_be(self.header.pipeline_latency_us)
    }

    /// Check if embedding has auxiliary data.
    #[inline]
    pub fn has_aux_data(&self) -> bool {
        self.aux_data.is_some()
    }

    /// Get raw aux_data bytes (for deferred parsing).
    #[inline]
    pub fn aux_data_bytes(&self) -> Option<&'a [u8]> {
        self.aux_data
    }

    /// Get header reference.
    #[inline]
    pub fn header(&self) -> &EmbeddingHeader {
        self.header
    }

    /// Get raw vector bytes (big-endian).
    #[inline]
    pub fn vector_bytes_raw(&self) -> &'a [u8] {
        self.vector_bytes
    }

    /// Get raw weights bytes (big-endian).
    #[inline]
    pub fn weights_bytes_raw(&self) -> &'a [u8] {
        self.weights_bytes
    }
}

/// Errors during encoding.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("Buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall { needed: usize, available: usize },

    #[error("Invalid embedding dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Errors during decoding.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Invalid magic bytes: expected 'CGEB' (0x43474542)")]
    InvalidMagic,

    #[error("Unsupported format version: {0} (max supported: {EMBEDDING_BINARY_VERSION})")]
    UnsupportedVersion(u16),

    #[error("Buffer too short: need {needed} bytes, have {available}")]
    BufferTooShort { needed: usize, available: usize },

    #[error("Content hash mismatch: file may be corrupted")]
    HashMismatch,

    #[error("Alignment error: expected {expected}-byte alignment, got offset {actual}")]
    AlignmentError { expected: usize, actual: usize },

    #[error("Auxiliary data corrupted: {0}")]
    AuxDataCorrupted(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::dimensions::{FUSED_OUTPUT, NUM_EXPERTS};

    fn make_test_embedding() -> FusedEmbedding {
        FusedEmbedding::new(
            vec![0.1; FUSED_OUTPUT],
            [0.125; NUM_EXPERTS], // sum = 1.0
            [0, 1],
            1000,
            0xDEADBEEF,
        )
        .expect("test embedding creation")
    }

    // ========== Header Tests ==========

    #[test]
    fn test_header_is_exactly_64_bytes() {
        let size = std::mem::size_of::<EmbeddingHeader>();
        println!("BEFORE: Expected size = 64 bytes");
        println!("AFTER: Actual size = {} bytes", size);
        assert_eq!(size, 64);
        println!("PASSED: EmbeddingHeader is exactly 64 bytes");
    }

    #[test]
    fn test_header_alignment_is_suitable_for_pod() {
        // EmbeddingHeader uses #[repr(C)] for Pod compatibility
        // The 64-byte alignment happens via the buffer placement, not the struct itself
        let align = std::mem::align_of::<EmbeddingHeader>();
        println!("BEFORE: EmbeddingHeader needs Pod-compatible alignment");
        println!("AFTER: Actual alignment = {} bytes", align);
        // 8-byte alignment (for u64 fields) is sufficient for Pod
        assert!(align >= 8, "Alignment must be at least 8 bytes for u64 fields");
        println!("PASSED: EmbeddingHeader has Pod-compatible alignment");
    }

    #[test]
    fn test_min_buffer_size_is_6242() {
        println!("BEFORE: Expected MIN_BUFFER_SIZE = 6242");
        println!(
            "AFTER: Actual MIN_BUFFER_SIZE = {}",
            EmbeddingBinaryCodec::MIN_BUFFER_SIZE
        );
        assert_eq!(EmbeddingBinaryCodec::MIN_BUFFER_SIZE, 6242);
        println!("PASSED: MIN_BUFFER_SIZE is exactly 6242 bytes");
    }

    // ========== Encode Tests ==========

    #[test]
    fn test_encode_produces_6242_bytes_no_aux() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();

        println!(
            "BEFORE: embedding.vector.len() = {}",
            embedding.vector.len()
        );

        let bytes = codec.encode(&embedding).expect("encode should succeed");

        println!("AFTER: bytes.len() = {}", bytes.len());
        assert_eq!(bytes.len(), EmbeddingBinaryCodec::MIN_BUFFER_SIZE);
        assert_eq!(bytes.len(), 6242);
        println!("PASSED: encode produces exactly 6242 bytes (no aux_data)");
    }

    #[test]
    fn test_encode_writes_correct_magic() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();

        let bytes = codec.encode(&embedding).expect("encode");

        println!(
            "MAGIC: {:02x} {:02x} {:02x} {:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3]
        );
        assert_eq!(&bytes[0..4], &EMBEDDING_MAGIC);
        println!("PASSED: magic bytes = 'CGEB' (0x43474542)");
    }

    #[test]
    fn test_encode_fails_fast_on_wrong_dimension() {
        let codec = EmbeddingBinaryCodec::new();
        let bad_embedding = FusedEmbedding {
            vector: vec![0.0; 512], // WRONG dimension
            expert_weights: [0.125; 8],
            selected_experts: [0, 1],
            pipeline_latency_us: 0,
            content_hash: 0,
            aux_data: None,
        };

        let result = codec.encode(&bad_embedding);

        println!("Result: {:?}", result);
        assert!(result.is_err());
        match result.unwrap_err() {
            EncodeError::InvalidDimension { expected, actual } => {
                assert_eq!(expected, 1536);
                assert_eq!(actual, 512);
            }
            e => panic!("Expected InvalidDimension, got {:?}", e),
        }
        println!("PASSED: encode fails fast on wrong dimension");
    }

    // ========== Decode Tests ==========

    #[test]
    fn test_decode_round_trip_preserves_all_fields() {
        let codec = EmbeddingBinaryCodec::new();
        let original = make_test_embedding();

        println!("BEFORE: vector[0..3] = {:?}", &original.vector[0..3]);
        println!("BEFORE: expert_weights = {:?}", original.expert_weights);
        println!(
            "BEFORE: selected_experts = {:?}",
            original.selected_experts
        );
        println!("BEFORE: content_hash = {:#x}", original.content_hash);

        let bytes = codec.encode(&original).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!("AFTER: vector[0..3] = {:?}", &decoded.vector[0..3]);
        println!("AFTER: expert_weights = {:?}", decoded.expert_weights);
        println!("AFTER: selected_experts = {:?}", decoded.selected_experts);
        println!("AFTER: content_hash = {:#x}", decoded.content_hash);

        assert_eq!(original.vector, decoded.vector);
        assert_eq!(original.expert_weights, decoded.expert_weights);
        assert_eq!(original.selected_experts, decoded.selected_experts);
        assert_eq!(original.pipeline_latency_us, decoded.pipeline_latency_us);
        assert_eq!(original.content_hash, decoded.content_hash);
        println!("PASSED: round-trip preserves all fields");
    }

    #[test]
    fn test_decode_fails_fast_on_invalid_magic() {
        let codec = EmbeddingBinaryCodec::new();
        let mut buffer = vec![0u8; EmbeddingBinaryCodec::MIN_BUFFER_SIZE];
        buffer[0..4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Bad magic

        println!("BEFORE: buffer[0..4] = {:02x?}", &buffer[0..4]);

        let result = codec.decode(&buffer);

        println!("AFTER: result = {:?}", result);
        assert!(matches!(result, Err(DecodeError::InvalidMagic)));
        println!("PASSED: decode fails fast on invalid magic");
    }

    #[test]
    fn test_decode_fails_fast_on_truncated_buffer() {
        let codec = EmbeddingBinaryCodec::new();
        let buffer = vec![0u8; 100]; // Way too short

        println!("BEFORE: buffer.len() = {}", buffer.len());

        let result = codec.decode(&buffer);

        println!("AFTER: result = {:?}", result);
        match result {
            Err(DecodeError::BufferTooShort { needed, available }) => {
                assert_eq!(needed, 6242);
                assert_eq!(available, 100);
            }
            _ => panic!("Expected BufferTooShort"),
        }
        println!("PASSED: decode fails fast on truncated buffer");
    }

    #[test]
    fn test_decode_fails_fast_on_unsupported_version() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();
        let mut bytes = codec.encode(&embedding).expect("encode");

        // Corrupt version to 99 (big-endian)
        bytes[4] = 0x00;
        bytes[5] = 0x63; // 99 in big-endian

        println!("BEFORE: version bytes = {:02x?}", &bytes[4..6]);

        let result = codec.decode(&bytes);

        println!("AFTER: result = {:?}", result);
        assert!(matches!(result, Err(DecodeError::UnsupportedVersion(99))));
        println!("PASSED: decode fails fast on unsupported version");
    }

    // ========== Big-Endian Tests ==========

    #[test]
    fn test_encode_uses_big_endian_floats() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();
        embedding.vector[0] = 1.0f32; // Known value

        let bytes = codec.encode(&embedding).expect("encode");

        // 1.0f32 in big-endian = 0x3F800000
        let expected_be = 1.0f32.to_be_bytes();
        println!("Expected BE bytes for 1.0: {:02x?}", expected_be);
        println!("Actual bytes at offset 64: {:02x?}", &bytes[64..68]);

        assert_eq!(&bytes[64..68], &expected_be);
        println!("PASSED: encode uses big-endian for floats");
    }

    #[test]
    fn test_decode_converts_big_endian_correctly() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();
        embedding.vector[0] = std::f32::consts::PI;

        println!("BEFORE: original.vector[0] = {}", embedding.vector[0]);

        let bytes = codec.encode(&embedding).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!("AFTER: decoded.vector[0] = {}", decoded.vector[0]);
        assert!((embedding.vector[0] - decoded.vector[0]).abs() < 1e-7);
        println!("PASSED: decode converts big-endian correctly (PI preserved)");
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_edge_case_max_content_hash() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();
        embedding.content_hash = u64::MAX;

        println!("BEFORE: content_hash = {:#x}", embedding.content_hash);

        let bytes = codec.encode(&embedding).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!("AFTER: content_hash = {:#x}", decoded.content_hash);
        assert_eq!(decoded.content_hash, u64::MAX);
        println!("Edge Case PASSED: u64::MAX content_hash preserved");
    }

    #[test]
    fn test_edge_case_zero_vector() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();
        for v in &mut embedding.vector {
            *v = 0.0;
        }

        println!("BEFORE: all vector elements = 0.0");

        let bytes = codec.encode(&embedding).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!("AFTER: decoded.vector[0..5] = {:?}", &decoded.vector[0..5]);
        assert!(decoded.vector.iter().all(|&v| v == 0.0));
        println!("Edge Case PASSED: zero vector preserved");
    }

    #[test]
    fn test_edge_case_negative_floats() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();
        embedding.vector[0] = -1.5;
        embedding.vector[1] = f32::MIN;

        println!(
            "BEFORE: vector[0] = {}, vector[1] = {}",
            embedding.vector[0], embedding.vector[1]
        );

        let bytes = codec.encode(&embedding).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!(
            "AFTER: vector[0] = {}, vector[1] = {}",
            decoded.vector[0], decoded.vector[1]
        );
        assert_eq!(decoded.vector[0], -1.5);
        assert_eq!(decoded.vector[1], f32::MIN);
        println!("Edge Case PASSED: negative floats preserved");
    }

    #[test]
    fn test_edge_case_max_pipeline_latency() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();
        embedding.pipeline_latency_us = u64::MAX;

        println!(
            "BEFORE: pipeline_latency_us = {}",
            embedding.pipeline_latency_us
        );

        let bytes = codec.encode(&embedding).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!(
            "AFTER: pipeline_latency_us = {}",
            decoded.pipeline_latency_us
        );
        assert_eq!(decoded.pipeline_latency_us, u64::MAX);
        println!("Edge Case PASSED: u64::MAX pipeline_latency_us preserved");
    }

    #[test]
    fn test_encode_to_buffer() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();
        let mut buffer = vec![0u8; 10000];

        println!("BEFORE: buffer all zeros");

        let written = codec
            .encode_to_buffer(&embedding, &mut buffer)
            .expect("encode_to_buffer");

        println!("AFTER: written = {} bytes", written);
        println!(
            "AFTER: buffer[0..4] (magic) = {:02x?}",
            &buffer[0..4]
        );

        assert_eq!(written, 6242);
        assert_eq!(&buffer[0..4], &EMBEDDING_MAGIC);
        println!("PASSED: encode_to_buffer works correctly");
    }

    #[test]
    fn test_encode_to_buffer_too_small() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();
        let mut buffer = vec![0u8; 100]; // Too small

        let result = codec.encode_to_buffer(&embedding, &mut buffer);

        println!("Result: {:?}", result);
        match result {
            Err(EncodeError::BufferTooSmall { needed, available }) => {
                assert_eq!(needed, 6242);
                assert_eq!(available, 100);
            }
            _ => panic!("Expected BufferTooSmall"),
        }
        println!("PASSED: encode_to_buffer fails on small buffer");
    }

    #[test]
    fn test_serialized_size() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();

        let size = codec.serialized_size(&embedding);

        println!("BEFORE: Expected size = 6242");
        println!("AFTER: Actual size = {}", size);
        assert_eq!(size, 6242);
        println!("PASSED: serialized_size returns correct value");
    }

    #[test]
    fn test_decode_header_only() {
        let codec = EmbeddingBinaryCodec::new();
        let embedding = make_test_embedding();
        let bytes = codec.encode(&embedding).expect("encode");

        let header = codec.decode_header(&bytes).expect("decode_header");

        println!("Header magic: {:02x?}", header.magic);
        println!("Header version (BE): {:#06x}", header.version);
        println!(
            "Header content_hash (BE): {:#018x}",
            header.content_hash
        );

        assert_eq!(header.magic, EMBEDDING_MAGIC);
        assert_eq!(u16::from_be(header.version), 1);
        assert_eq!(u64::from_be(header.content_hash), 0xDEADBEEF);
        println!("PASSED: decode_header extracts header correctly");
    }

    #[test]
    fn test_special_float_values() {
        let codec = EmbeddingBinaryCodec::new();
        let mut embedding = make_test_embedding();

        // Test special float values
        embedding.vector[0] = f32::INFINITY;
        embedding.vector[1] = f32::NEG_INFINITY;
        embedding.vector[2] = 0.0;
        embedding.vector[3] = -0.0;
        // Note: NaN != NaN, so we skip NaN test for equality

        println!("BEFORE: INF={}, NEG_INF={}, ZERO={}, NEG_ZERO={}",
            embedding.vector[0], embedding.vector[1], embedding.vector[2], embedding.vector[3]);

        let bytes = codec.encode(&embedding).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");

        println!("AFTER: INF={}, NEG_INF={}, ZERO={}, NEG_ZERO={}",
            decoded.vector[0], decoded.vector[1], decoded.vector[2], decoded.vector[3]);

        assert!(decoded.vector[0].is_infinite() && decoded.vector[0].is_sign_positive());
        assert!(decoded.vector[1].is_infinite() && decoded.vector[1].is_sign_negative());
        assert_eq!(decoded.vector[2], 0.0);
        assert_eq!(decoded.vector[3], -0.0);
        println!("PASSED: special float values preserved");
    }
}
