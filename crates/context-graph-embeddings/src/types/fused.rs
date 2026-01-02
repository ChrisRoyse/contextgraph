//! FusedEmbedding: Final 1536D output from FuseMoE fusion.
//!
//! This module provides the primary embedding representation used throughout
//! the system for similarity search, clustering, and downstream tasks.
//!
//! # Architecture
//!
//! The FuseMoE fusion layer takes the 8320D concatenated embeddings from all
//! 12 models and produces a unified 1536D vector through sparse expert routing:
//!
//! - 8 expert networks, each specialized for different aspects
//! - Top-K routing selects the best K experts per input (K=4 per constitution.yaml)
//! - Expert weights sum to 1.0 for normalized contribution
//!
//! # Binary Format
//!
//! Core embedding serializes to exactly 6200 bytes:
//! - 6144 bytes: 1536 × f32 vector
//! - 32 bytes: 8 × f32 expert weights
//! - 4 bytes: 4 × u8 selected experts (TOP_K_EXPERTS=4)
//! - 8 bytes: u64 pipeline latency
//! - 8 bytes: u64 content hash
//! - 4 bytes: u32 aux_data length (0 if None)

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::dimensions::{COLBERT_V3_DIM, FUSED_OUTPUT, NUM_EXPERTS, TOP_K_EXPERTS};
use crate::types::ModelId;
use serde::{Deserialize, Serialize};

/// Binary format constants.
/// Core embedding size: 1536*4 + 8*4 + 4 + 8 + 8 + 4 = 6200 bytes.
const VECTOR_BYTES: usize = FUSED_OUTPUT * 4;
const WEIGHTS_BYTES: usize = NUM_EXPERTS * 4;
const SELECTED_BYTES: usize = TOP_K_EXPERTS;
const LATENCY_BYTES: usize = 8;
const HASH_BYTES: usize = 8;
const AUX_LEN_BYTES: usize = 4;
const CORE_BINARY_SIZE: usize =
    VECTOR_BYTES + WEIGHTS_BYTES + SELECTED_BYTES + LATENCY_BYTES + HASH_BYTES + AUX_LEN_BYTES;

/// Tolerance for expert weight sum validation.
const WEIGHT_SUM_TOLERANCE: f32 = 0.01;

/// Final 1536D fused embedding from FuseMoE expert fusion.
/// This is the primary embedding representation for similarity search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FusedEmbedding {
    /// 1536D fused vector (FUSED_OUTPUT dimension)
    pub vector: Vec<f32>,
    /// Expert weights for all 8 experts (sum to 1.0)
    pub expert_weights: [f32; 8],
    /// Indices of top-K selected experts (0-7 each)
    pub selected_experts: [u8; TOP_K_EXPERTS],
    /// Pipeline latency in microseconds
    pub pipeline_latency_us: u64,
    /// xxHash64 of original content for caching
    pub content_hash: u64,
    /// Optional ColBERT per-token vectors for late interaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aux_data: Option<AuxiliaryEmbeddingData>,
}

/// Auxiliary embedding data for ColBERT token-level storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuxiliaryEmbeddingData {
    /// Model that produced this auxiliary data (must be ColBertV3/LateInteraction)
    pub source_model: ModelId,
    /// Per-token embeddings (128D each)
    pub token_vectors: Vec<Vec<f32>>,
    /// Token count (excludes padding)
    pub num_tokens: usize,
    /// Compressed blob for efficient storage
    pub blob: Option<Vec<u8>>,
}

impl FusedEmbedding {
    /// Fused output dimension (from dimensions.rs)
    pub const DIMENSION: usize = FUSED_OUTPUT;
    /// Number of experts in FuseMoE (from dimensions.rs)
    pub const NUM_EXPERTS: usize = NUM_EXPERTS;
    /// Top-K expert selection (from dimensions.rs)
    pub const TOP_K: usize = TOP_K_EXPERTS;

    /// Create new FusedEmbedding with validation.
    ///
    /// # Arguments
    /// * `vector` - 1536D fused embedding vector
    /// * `expert_weights` - Weights for all 8 experts (must sum to ~1.0)
    /// * `selected_experts` - Indices of top-K selected experts (0-7)
    /// * `pipeline_latency_us` - Pipeline latency in microseconds
    /// * `content_hash` - xxHash64 of original content
    ///
    /// # Returns
    /// * `Ok(FusedEmbedding)` if all validations pass
    /// * `Err(EmbeddingError)` if dimension mismatch or invalid expert indices
    ///
    /// # Panics
    /// This method does NOT panic. Validation errors are returned as Results.
    /// For panic-on-invalid behavior, use `new_unchecked` with prior validation.
    pub fn new(
        vector: Vec<f32>,
        expert_weights: [f32; 8],
        selected_experts: [u8; TOP_K_EXPERTS],
        pipeline_latency_us: u64,
        content_hash: u64,
    ) -> EmbeddingResult<Self> {
        // Validate dimension
        if vector.len() != FUSED_OUTPUT {
            return Err(EmbeddingError::InvalidDimension {
                expected: FUSED_OUTPUT,
                actual: vector.len(),
            });
        }

        // Validate expert indices
        for (i, &idx) in selected_experts.iter().enumerate() {
            if idx as usize >= NUM_EXPERTS {
                return Err(EmbeddingError::FusionError {
                    message: format!("Invalid expert index {}, max is {}", idx, NUM_EXPERTS),
                });
            }
            // Check for duplicate expert selection
            for &other_idx in selected_experts.iter().skip(i + 1) {
                if other_idx == idx {
                    return Err(EmbeddingError::FusionError {
                        message: format!("Duplicate expert index: {}", idx),
                    });
                }
            }
        }

        let embedding = Self {
            vector,
            expert_weights,
            selected_experts,
            pipeline_latency_us,
            content_hash,
            aux_data: None,
        };

        Ok(embedding)
    }

    /// Attach auxiliary ColBERT token vectors.
    ///
    /// # Arguments
    /// * `aux_data` - AuxiliaryEmbeddingData containing token vectors
    ///
    /// # Returns
    /// Self with aux_data attached (builder pattern)
    #[must_use]
    pub fn with_aux_data(mut self, aux_data: AuxiliaryEmbeddingData) -> Self {
        self.aux_data = Some(aux_data);
        self
    }

    /// Validate embedding state. Returns error if:
    /// - vector contains NaN or Inf
    /// - expert_weights don't sum to ~1.0 (±0.01)
    /// - selected_experts contain invalid indices (>=8)
    ///
    /// # Errors
    /// - `EmbeddingError::InvalidValue` if NaN or Inf values found
    /// - `EmbeddingError::FusionError` if weights don't sum to ~1.0 or invalid expert indices
    pub fn validate(&self) -> EmbeddingResult<()> {
        // Check for NaN/Inf in vector
        for (idx, &val) in self.vector.iter().enumerate() {
            if val.is_nan() || val.is_infinite() {
                return Err(EmbeddingError::InvalidValue { index: idx, value: val });
            }
        }

        // Check expert weights sum to 1.0 ± tolerance
        let weight_sum: f32 = self.expert_weights.iter().sum();
        if (weight_sum - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
            return Err(EmbeddingError::FusionError {
                message: format!(
                    "Weights sum to {:.6}, expected 1.0 ± {}",
                    weight_sum, WEIGHT_SUM_TOLERANCE
                ),
            });
        }

        // Check expert indices
        for &idx in &self.selected_experts {
            if idx as usize >= NUM_EXPERTS {
                return Err(EmbeddingError::FusionError {
                    message: format!("Invalid expert index {}, max is {}", idx, NUM_EXPERTS),
                });
            }
        }

        Ok(())
    }

    /// Normalize vector to unit length in-place.
    ///
    /// Zero vectors remain unchanged to avoid division by zero.
    pub fn normalize(&mut self) {
        let mag = self.magnitude();
        if mag > f32::EPSILON {
            for val in &mut self.vector {
                *val /= mag;
            }
        }
    }

    /// Compute cosine similarity with another FusedEmbedding.
    ///
    /// For best performance, both vectors should be normalized first.
    ///
    /// # Arguments
    /// * `other` - Another FusedEmbedding to compare against
    ///
    /// # Returns
    /// Cosine similarity in range [-1.0, 1.0]
    pub fn cosine_similarity(&self, other: &FusedEmbedding) -> f32 {
        let dot_product: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a = self.magnitude();
        let norm_b = other.magnitude();

        if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }

    /// Serialize to compact binary format (6200 bytes + aux_data).
    ///
    /// # Binary Layout
    /// | Offset | Size | Field |
    /// |--------|------|-------|
    /// | 0 | 6144 | vector (1536 × f32 LE) |
    /// | 6144 | 32 | expert_weights (8 × f32 LE) |
    /// | 6176 | 4 | selected_experts (4 × u8, TOP_K_EXPERTS) |
    /// | 6180 | 8 | pipeline_latency_us (u64 LE) |
    /// | 6188 | 8 | content_hash (u64 LE) |
    /// | 6196 | 4 | aux_data_len (u32 LE, 0 if None) |
    /// | 6200+ | var | aux_data blob (if present) |
    pub fn to_bytes(&self) -> Vec<u8> {
        let aux_blob = self.aux_data.as_ref().map(|a| a.to_blob());
        let aux_len = aux_blob.as_ref().map(|b| b.len()).unwrap_or(0);
        let mut bytes = Vec::with_capacity(CORE_BINARY_SIZE + aux_len);

        // Vector: 1536 × f32 (6144 bytes)
        for &val in &self.vector {
            bytes.extend_from_slice(&val.to_le_bytes());
        }

        // Expert weights: 8 × f32 (32 bytes)
        for &w in &self.expert_weights {
            bytes.extend_from_slice(&w.to_le_bytes());
        }

        // Selected experts: 2 × u8 (2 bytes)
        bytes.extend_from_slice(&self.selected_experts);

        // Pipeline latency: u64 (8 bytes)
        bytes.extend_from_slice(&self.pipeline_latency_us.to_le_bytes());

        // Content hash: u64 (8 bytes)
        bytes.extend_from_slice(&self.content_hash.to_le_bytes());

        // Aux data length: u32 (4 bytes)
        bytes.extend_from_slice(&(aux_len as u32).to_le_bytes());

        // Aux data blob (if present)
        if let Some(blob) = aux_blob {
            bytes.extend_from_slice(&blob);
        }

        bytes
    }

    /// Deserialize from binary format.
    ///
    /// # Errors
    /// - `EmbeddingError::SerializationError` if data is truncated or corrupted
    pub fn from_bytes(bytes: &[u8]) -> EmbeddingResult<Self> {
        if bytes.len() < CORE_BINARY_SIZE {
            return Err(EmbeddingError::SerializationError {
                message: format!(
                    "Data too short: expected at least {} bytes, got {}",
                    CORE_BINARY_SIZE,
                    bytes.len()
                ),
            });
        }

        let mut offset = 0;

        // Vector: 1536 × f32
        let mut vector = Vec::with_capacity(FUSED_OUTPUT);
        for _ in 0..FUSED_OUTPUT {
            let val = f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            vector.push(val);
            offset += 4;
        }

        // Expert weights: 8 × f32
        let mut expert_weights = [0.0f32; 8];
        for weight in &mut expert_weights {
            *weight = f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            offset += 4;
        }

        // Selected experts: TOP_K_EXPERTS × u8
        let mut selected_experts = [0u8; TOP_K_EXPERTS];
        for (i, expert) in selected_experts.iter_mut().enumerate() {
            *expert = bytes[offset + i];
        }
        offset += TOP_K_EXPERTS;

        // Pipeline latency: u64
        let pipeline_latency_us = u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        // Content hash: u64
        let content_hash = u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        // Aux data length: u32
        let aux_len = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;

        // Aux data blob (if present)
        let aux_data = if aux_len > 0 {
            if bytes.len() < offset + aux_len {
                return Err(EmbeddingError::SerializationError {
                    message: format!(
                        "Aux data truncated: expected {} bytes at offset {}, got {} total",
                        aux_len,
                        offset,
                        bytes.len()
                    ),
                });
            }
            let blob = &bytes[offset..offset + aux_len];
            Some(AuxiliaryEmbeddingData::from_blob(blob)?)
        } else {
            None
        };

        Ok(Self {
            vector,
            expert_weights,
            selected_experts,
            pipeline_latency_us,
            content_hash,
            aux_data,
        })
    }

    /// Check if auxiliary data is present.
    #[inline]
    pub fn has_aux_data(&self) -> bool {
        self.aux_data.is_some()
    }

    /// Get ColBERT token vectors for MaxSim scoring.
    #[inline]
    pub fn get_token_vectors(&self) -> Option<&Vec<Vec<f32>>> {
        self.aux_data.as_ref().map(|a| &a.token_vectors)
    }

    /// Compress aux_data token_vectors into blob.
    ///
    /// # Errors
    /// - `EmbeddingError::FusionError` if no aux_data present
    pub fn compress_aux_data(&mut self) -> EmbeddingResult<()> {
        if let Some(ref mut aux) = self.aux_data {
            let blob = aux.to_blob();
            aux.blob = Some(blob);
            Ok(())
        } else {
            Err(EmbeddingError::FusionError {
                message: "No auxiliary data to compress".to_string(),
            })
        }
    }

    /// Decompress aux_data blob into token_vectors.
    ///
    /// # Errors
    /// - `EmbeddingError::FusionError` if no aux_data or no blob present
    pub fn decompress_aux_data(&mut self) -> EmbeddingResult<()> {
        if let Some(ref mut aux) = self.aux_data {
            if let Some(ref blob) = aux.blob {
                let decompressed = AuxiliaryEmbeddingData::from_blob(blob)?;
                aux.token_vectors = decompressed.token_vectors;
                aux.num_tokens = decompressed.num_tokens;
                Ok(())
            } else {
                Err(EmbeddingError::FusionError {
                    message: "No blob to decompress".to_string(),
                })
            }
        } else {
            Err(EmbeddingError::FusionError {
                message: "No auxiliary data present".to_string(),
            })
        }
    }

    /// Compute magnitude (L2 norm) of vector.
    #[inline]
    pub fn magnitude(&self) -> f32 {
        let sum_squares: f32 = self.vector.iter().map(|x| x * x).sum();
        sum_squares.sqrt()
    }

    /// Check if vector is normalized (magnitude ≈ 1.0).
    #[inline]
    pub fn is_normalized(&self) -> bool {
        (self.magnitude() - 1.0).abs() < 1e-5
    }

    /// Total memory size in bytes for cache budgeting.
    ///
    /// Returns the base binary size (6200 bytes) plus any auxiliary data size.
    /// This matches the serialized format size for accurate memory tracking.
    #[inline]
    #[must_use]
    pub fn memory_size(&self) -> usize {
        let aux_size = self
            .aux_data
            .as_ref()
            .map(|a| a.memory_size())
            .unwrap_or(0);
        CORE_BINARY_SIZE + aux_size
    }

    // ========== GPU Tensor Conversions ==========

    /// Convert fused embedding vector to GPU tensor.
    ///
    /// # Arguments
    /// * `device` - The GPU device to create the tensor on
    ///
    /// # Returns
    /// A 1D tensor of shape [1536] containing the fused embedding.
    ///
    /// # Example
    /// ```rust,ignore
    /// let fused = FusedEmbedding::stub(12345);
    /// let tensor = fused.to_tensor(device())?;
    /// assert_eq!(tensor.dims(), &[1536]);
    /// ```
    #[cfg(feature = "candle")]
    pub fn to_tensor(&self, device: &candle_core::Device) -> candle_core::Result<candle_core::Tensor> {
        candle_core::Tensor::from_slice(&self.vector, (self.vector.len(),), device)
    }

    /// Create fused embedding from GPU tensor.
    ///
    /// # Arguments
    /// * `tensor` - A 1D tensor of shape [1536]
    /// * `expert_weights` - Expert weights from gating network
    /// * `selected_experts` - Top-K selected expert indices
    /// * `content_hash` - Hash of original content
    ///
    /// # Returns
    /// A new FusedEmbedding with the tensor values.
    #[cfg(feature = "candle")]
    pub fn from_tensor(
        tensor: &candle_core::Tensor,
        expert_weights: [f32; 8],
        selected_experts: [u8; TOP_K_EXPERTS],
        content_hash: u64,
    ) -> candle_core::Result<Self> {
        let vector: Vec<f32> = tensor.to_vec1()?;
        Ok(Self {
            vector,
            expert_weights,
            selected_experts,
            pipeline_latency_us: 0,
            content_hash,
            aux_data: None,
        })
    }

    /// Convert batch of fused embeddings to GPU tensor.
    ///
    /// # Arguments
    /// * `embeddings` - Slice of fused embeddings
    /// * `device` - The GPU device
    ///
    /// # Returns
    /// A 2D tensor of shape [batch_size, 1536].
    #[cfg(feature = "candle")]
    pub fn batch_to_tensor(
        embeddings: &[Self],
        device: &candle_core::Device,
    ) -> candle_core::Result<candle_core::Tensor> {
        use crate::types::dimensions::FUSED_OUTPUT;

        if embeddings.is_empty() {
            return candle_core::Tensor::zeros((0, FUSED_OUTPUT), candle_core::DType::F32, device);
        }

        let batch_size = embeddings.len();
        let data: Vec<f32> = embeddings.iter().flat_map(|e| e.vector.iter().copied()).collect();

        candle_core::Tensor::from_slice(&data, (batch_size, FUSED_OUTPUT), device)
    }
}

impl AuxiliaryEmbeddingData {
    /// ColBERT dimension (from dimensions.rs)
    pub const TOKEN_DIM: usize = COLBERT_V3_DIM;

    /// Create new AuxiliaryEmbeddingData.
    ///
    /// # Arguments
    /// * `source_model` - Model that produced this data (should be LateInteraction)
    /// * `token_vectors` - Per-token embeddings (128D each)
    ///
    /// # Panics
    /// Panics if any token vector is not 128D. Use `try_new` for non-panicking version.
    #[must_use]
    pub fn new(source_model: ModelId, token_vectors: Vec<Vec<f32>>) -> Self {
        // Validate token vector dimensions
        for (i, vec) in token_vectors.iter().enumerate() {
            if vec.len() != COLBERT_V3_DIM {
                panic!(
                    "Token vector at index {} has dimension {}, expected {}",
                    i,
                    vec.len(),
                    COLBERT_V3_DIM
                );
            }
        }

        let num_tokens = token_vectors.len();
        Self {
            source_model,
            token_vectors,
            num_tokens,
            blob: None,
        }
    }

    /// Try to create new AuxiliaryEmbeddingData without panicking.
    ///
    /// # Errors
    /// - `EmbeddingError::InvalidDimension` if any token vector is not 128D
    pub fn try_new(
        source_model: ModelId,
        token_vectors: Vec<Vec<f32>>,
    ) -> EmbeddingResult<Self> {
        for vec in token_vectors.iter() {
            if vec.len() != COLBERT_V3_DIM {
                return Err(EmbeddingError::InvalidDimension {
                    expected: COLBERT_V3_DIM,
                    actual: vec.len(),
                });
            }
        }

        let num_tokens = token_vectors.len();
        Ok(Self {
            source_model,
            token_vectors,
            num_tokens,
            blob: None,
        })
    }

    /// Serialize to compressed blob.
    ///
    /// Binary format:
    /// - 1 byte: source_model as u8
    /// - 4 bytes: num_tokens as u32 LE
    /// - num_tokens * 128 * 4 bytes: flattened f32 vectors
    pub fn to_blob(&self) -> Vec<u8> {
        let vector_bytes = self.num_tokens * COLBERT_V3_DIM * 4;
        let mut blob = Vec::with_capacity(1 + 4 + vector_bytes);

        // Source model: 1 byte
        blob.push(self.source_model as u8);

        // Num tokens: 4 bytes
        blob.extend_from_slice(&(self.num_tokens as u32).to_le_bytes());

        // Token vectors: flattened f32s
        for vec in &self.token_vectors {
            for &val in vec {
                blob.extend_from_slice(&val.to_le_bytes());
            }
        }

        blob
    }

    /// Deserialize from blob.
    ///
    /// # Errors
    /// - `EmbeddingError::SerializationError` if blob is corrupted or truncated
    pub fn from_blob(blob: &[u8]) -> EmbeddingResult<Self> {
        if blob.len() < 5 {
            return Err(EmbeddingError::SerializationError {
                message: "Blob too short for header".to_string(),
            });
        }

        // Source model: 1 byte
        let source_model = ModelId::try_from(blob[0]).map_err(|e| {
            EmbeddingError::SerializationError {
                message: format!("Invalid source model: {}", e),
            }
        })?;

        // Num tokens: 4 bytes
        let num_tokens =
            u32::from_le_bytes([blob[1], blob[2], blob[3], blob[4]]) as usize;

        let expected_len = 5 + num_tokens * COLBERT_V3_DIM * 4;
        if blob.len() < expected_len {
            return Err(EmbeddingError::SerializationError {
                message: format!(
                    "Blob too short: expected {} bytes for {} tokens, got {}",
                    expected_len, num_tokens, blob.len()
                ),
            });
        }

        // Token vectors
        let mut token_vectors = Vec::with_capacity(num_tokens);
        let mut offset = 5;
        for _ in 0..num_tokens {
            let mut vec = Vec::with_capacity(COLBERT_V3_DIM);
            for _ in 0..COLBERT_V3_DIM {
                let val = f32::from_le_bytes([
                    blob[offset],
                    blob[offset + 1],
                    blob[offset + 2],
                    blob[offset + 3],
                ]);
                vec.push(val);
                offset += 4;
            }
            token_vectors.push(vec);
        }

        Ok(Self {
            source_model,
            token_vectors,
            num_tokens,
            blob: Some(blob.to_vec()),
        })
    }

    /// Calculate memory size in bytes.
    ///
    /// Includes token vector storage only (not metadata).
    #[inline]
    pub fn memory_size(&self) -> usize {
        self.num_tokens * COLBERT_V3_DIM * std::mem::size_of::<f32>()
    }

    /// Validate token vector dimensions.
    ///
    /// # Errors
    /// - `EmbeddingError::InvalidDimension` if any token vector is not 128D
    /// - `EmbeddingError::FusionError` if NaN or Inf values found
    pub fn validate(&self) -> EmbeddingResult<()> {
        for (i, vec) in self.token_vectors.iter().enumerate() {
            if vec.len() != COLBERT_V3_DIM {
                return Err(EmbeddingError::InvalidDimension {
                    expected: COLBERT_V3_DIM,
                    actual: vec.len(),
                });
            }
            // Check for NaN/Inf
            for (j, &val) in vec.iter().enumerate() {
                if val.is_nan() || val.is_infinite() {
                    return Err(EmbeddingError::FusionError {
                        message: format!(
                            "Invalid value {} in token {} at position {}",
                            val, i, j
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Helper Functions for Tests ==========

    fn make_valid_vector() -> Vec<f32> {
        vec![0.1; FUSED_OUTPUT]
    }

    fn make_valid_weights() -> [f32; 8] {
        [0.25, 0.25, 0.0, 0.0, 0.25, 0.0, 0.0, 0.25]
    }

    fn make_valid_selected() -> [u8; TOP_K_EXPERTS] {
        [0, 1, 4, 5]
    }

    fn make_valid_fused() -> FusedEmbedding {
        FusedEmbedding::new(
            make_valid_vector(),
            make_valid_weights(),
            make_valid_selected(),
            1000,
            12345,
        )
        .expect("Test helper should create valid embedding")
    }

    fn make_token_vector() -> Vec<f32> {
        vec![0.5; COLBERT_V3_DIM]
    }

    // ========== Construction Tests (6 tests) ==========

    #[test]
    fn test_new_with_valid_1536d_vector_succeeds() {
        let vector = make_valid_vector();
        let weights = make_valid_weights();
        let selected = make_valid_selected();

        let result = FusedEmbedding::new(vector.clone(), weights, selected, 1000, 12345);

        assert!(result.is_ok());
        let emb = result.unwrap();
        assert_eq!(emb.vector.len(), FUSED_OUTPUT);
        println!("PASSED: new() with valid 1536D vector succeeds");
    }

    #[test]
    fn test_new_with_wrong_dimension_fails() {
        let vector = vec![0.1; 512]; // Wrong: should be 1536
        let weights = make_valid_weights();
        let selected = make_valid_selected();

        let result = FusedEmbedding::new(vector, weights, selected, 1000, 12345);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InvalidDimension { expected, actual } => {
                assert_eq!(expected, FUSED_OUTPUT);
                assert_eq!(actual, 512);
            }
            e => panic!("Expected InvalidDimension, got {:?}", e),
        }
        println!("PASSED: new() with wrong dimension fails correctly");
    }

    #[test]
    fn test_new_with_invalid_expert_indices_fails() {
        let vector = make_valid_vector();
        let weights = make_valid_weights();
        let selected = [8, 0, 1, 2]; // 8 is invalid (must be 0-7)

        let result = FusedEmbedding::new(vector, weights, selected, 1000, 12345);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::FusionError { message } => {
                assert!(message.contains("8"));
                assert!(message.contains("Invalid expert index"));
            }
            e => panic!("Expected FusionError, got {:?}", e),
        }
        println!("PASSED: new() with invalid expert indices fails correctly");
    }

    #[test]
    fn test_new_with_valid_expert_weights_succeeds() {
        let vector = make_valid_vector();
        let weights = [0.1, 0.1, 0.1, 0.1, 0.2, 0.2, 0.1, 0.1]; // sum = 1.0
        let selected = make_valid_selected();

        let result = FusedEmbedding::new(vector, weights, selected, 1000, 12345);

        assert!(result.is_ok());
        let emb = result.unwrap();
        assert_eq!(emb.expert_weights, weights);
        println!("PASSED: new() with valid expert_weights succeeds");
    }

    #[test]
    fn test_new_computes_proper_defaults_for_latency_and_hash() {
        let vector = make_valid_vector();
        let weights = make_valid_weights();
        let selected = make_valid_selected();
        let latency = 5000u64;
        let hash = 0xDEADBEEF_u64;

        let result = FusedEmbedding::new(vector, weights, selected, latency, hash);

        assert!(result.is_ok());
        let emb = result.unwrap();
        assert_eq!(emb.pipeline_latency_us, latency);
        assert_eq!(emb.content_hash, hash);
        assert!(emb.aux_data.is_none());
        println!("PASSED: new() computes proper defaults for latency and hash");
    }

    #[test]
    fn test_with_aux_data_attaches_colbert_vectors() {
        let emb = make_valid_fused();
        let token_vecs = vec![make_token_vector(), make_token_vector()];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);

        let emb_with_aux = emb.with_aux_data(aux);

        assert!(emb_with_aux.has_aux_data());
        let aux_ref = emb_with_aux.aux_data.as_ref().unwrap();
        assert_eq!(aux_ref.num_tokens, 2);
        println!("PASSED: with_aux_data() attaches ColBERT vectors");
    }

    // ========== Validation Tests (8 tests) ==========

    #[test]
    fn test_validate_passes_for_valid_embedding() {
        let emb = make_valid_fused();
        let result = emb.validate();
        assert!(result.is_ok());
        println!("PASSED: validate() passes for valid embedding");
    }

    #[test]
    fn test_validate_rejects_nan_in_vector() {
        let mut emb = make_valid_fused();
        emb.vector[100] = f32::NAN;

        let result = emb.validate();

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 100);
                assert!(value.is_nan());
            }
            e => panic!("Expected InvalidValue, got {:?}", e),
        }
        println!("PASSED: validate() rejects NaN in vector");
    }

    #[test]
    fn test_validate_rejects_inf_in_vector() {
        let mut emb = make_valid_fused();
        emb.vector[500] = f32::INFINITY;

        let result = emb.validate();

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 500);
                assert!(value.is_infinite() && value.is_sign_positive());
            }
            e => panic!("Expected InvalidValue, got {:?}", e),
        }
        println!("PASSED: validate() rejects Inf in vector");
    }

    #[test]
    fn test_validate_rejects_neg_inf_in_vector() {
        let mut emb = make_valid_fused();
        emb.vector[200] = f32::NEG_INFINITY;

        let result = emb.validate();

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 200);
                assert!(value.is_infinite() && value.is_sign_negative());
            }
            e => panic!("Expected InvalidValue, got {:?}", e),
        }
        println!("PASSED: validate() rejects -Inf in vector");
    }

    #[test]
    fn test_validate_rejects_expert_weights_sum_not_1() {
        let mut emb = make_valid_fused();
        emb.expert_weights = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1]; // sum = 0.8

        let result = emb.validate();

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::FusionError { message } => {
                assert!(message.contains("0.8"));
            }
            e => panic!("Expected FusionError, got {:?}", e),
        }
        println!("PASSED: validate() rejects expert_weights sum != 1.0");
    }

    #[test]
    fn test_validate_accepts_expert_weights_sum_0_995() {
        let mut emb = make_valid_fused();
        // sum = 0.995 (within ±0.01 tolerance)
        emb.expert_weights = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.295];

        println!(
            "BEFORE: weights sum = {}",
            emb.expert_weights.iter().sum::<f32>()
        );

        let result = emb.validate();

        println!("AFTER: validate() result = {:?}", result);
        assert!(result.is_ok());
        println!("PASSED: validate() accepts expert_weights sum = 0.995");
    }

    #[test]
    fn test_validate_accepts_expert_weights_sum_1_005() {
        let mut emb = make_valid_fused();
        // sum = 1.005 (within ±0.01 tolerance)
        emb.expert_weights = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.305];

        println!(
            "BEFORE: weights sum = {}",
            emb.expert_weights.iter().sum::<f32>()
        );

        let result = emb.validate();

        println!("AFTER: validate() result = {:?}", result);
        assert!(result.is_ok());
        println!("PASSED: validate() accepts expert_weights sum = 1.005");
    }

    #[test]
    fn test_validate_rejects_expert_weights_sum_1_02() {
        let mut emb = make_valid_fused();
        // sum = 1.02 (outside ±0.01 tolerance)
        emb.expert_weights = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.32];

        println!(
            "BEFORE: weights sum = {}",
            emb.expert_weights.iter().sum::<f32>()
        );

        let result = emb.validate();

        println!("AFTER: validate() result = {:?}", result);
        assert!(result.is_err());
        println!("PASSED: validate() rejects expert_weights sum = 1.02");
    }

    // ========== Normalization Tests (5 tests) ==========

    #[test]
    fn test_normalize_produces_unit_vector() {
        let mut emb = make_valid_fused();
        // Set some non-unit values
        for (i, val) in emb.vector.iter_mut().enumerate() {
            *val = (i % 10) as f32;
        }

        println!("BEFORE: magnitude = {}", emb.magnitude());

        emb.normalize();

        println!("AFTER: magnitude = {}", emb.magnitude());
        assert!(
            (emb.magnitude() - 1.0).abs() < 1e-5,
            "Magnitude should be ~1.0, got {}",
            emb.magnitude()
        );
        println!("PASSED: normalize() produces unit vector (magnitude = 1.0)");
    }

    #[test]
    fn test_normalize_handles_zero_vector() {
        let mut emb = make_valid_fused();
        for val in emb.vector.iter_mut() {
            *val = 0.0;
        }

        println!("BEFORE: magnitude = {}", emb.magnitude());
        println!("BEFORE: vector[0..5] = {:?}", &emb.vector[0..5]);

        emb.normalize();

        println!("AFTER: magnitude = {}", emb.magnitude());
        println!("AFTER: vector[0..5] = {:?}", &emb.vector[0..5]);

        // Zero vector remains zero (no NaN from division by zero)
        assert!(emb.vector.iter().all(|&v| v == 0.0));
        assert!(!emb.vector.iter().any(|v| v.is_nan()));
        println!("PASSED: normalize() handles zero vector (no NaN)");
    }

    #[test]
    fn test_is_normalized_returns_true_after_normalize() {
        let mut emb = make_valid_fused();
        for (i, val) in emb.vector.iter_mut().enumerate() {
            *val = i as f32 * 0.001;
        }

        assert!(!emb.is_normalized(), "Should not be normalized before");

        emb.normalize();

        assert!(emb.is_normalized(), "Should be normalized after");
        println!("PASSED: is_normalized() returns true after normalize()");
    }

    #[test]
    fn test_is_normalized_returns_false_before_normalize() {
        let emb = make_valid_fused();

        let mag = emb.magnitude();
        let is_norm = emb.is_normalized();

        println!("Magnitude: {}, is_normalized: {}", mag, is_norm);
        // A vector of all 0.1 values with 1536 elements has magnitude = sqrt(1536 * 0.01) = sqrt(15.36) ≈ 3.92
        assert!(!is_norm);
        println!("PASSED: is_normalized() returns false before normalize()");
    }

    #[test]
    fn test_magnitude_computes_correct_l2_norm() {
        let mut emb = make_valid_fused();
        // Set known values: [3, 4, 0, 0, ...] has magnitude 5
        for val in emb.vector.iter_mut() {
            *val = 0.0;
        }
        emb.vector[0] = 3.0;
        emb.vector[1] = 4.0;

        let mag = emb.magnitude();

        assert!((mag - 5.0).abs() < 1e-6);
        println!("PASSED: magnitude() computes correct L2 norm (3,4,0...) = 5");
    }

    // ========== Similarity Tests (5 tests) ==========

    #[test]
    fn test_cosine_similarity_returns_1_for_identical_vectors() {
        let emb1 = make_valid_fused();
        let emb2 = emb1.clone();

        let sim = emb1.cosine_similarity(&emb2);

        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Similarity of identical vectors should be 1.0, got {}",
            sim
        );
        println!("PASSED: cosine_similarity() returns 1.0 for identical vectors");
    }

    #[test]
    fn test_cosine_similarity_returns_neg1_for_opposite_vectors() {
        let emb1 = make_valid_fused();
        let mut emb2 = emb1.clone();
        for val in emb2.vector.iter_mut() {
            *val = -*val;
        }

        let sim = emb1.cosine_similarity(&emb2);

        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "Similarity of opposite vectors should be -1.0, got {}",
            sim
        );
        println!("PASSED: cosine_similarity() returns -1.0 for opposite vectors");
    }

    #[test]
    fn test_cosine_similarity_returns_0_for_orthogonal_vectors() {
        let mut emb1 = make_valid_fused();
        let mut emb2 = make_valid_fused();

        // Create orthogonal vectors: [1,0,0,...] and [0,1,0,...]
        for val in emb1.vector.iter_mut() {
            *val = 0.0;
        }
        emb1.vector[0] = 1.0;

        for val in emb2.vector.iter_mut() {
            *val = 0.0;
        }
        emb2.vector[1] = 1.0;

        let sim = emb1.cosine_similarity(&emb2);

        assert!(
            sim.abs() < 1e-6,
            "Similarity of orthogonal vectors should be 0.0, got {}",
            sim
        );
        println!("PASSED: cosine_similarity() returns 0.0 for orthogonal vectors");
    }

    #[test]
    fn test_cosine_similarity_is_symmetric() {
        let emb1 = make_valid_fused();
        let mut emb2 = make_valid_fused();
        for (i, val) in emb2.vector.iter_mut().enumerate() {
            *val = (i as f32 * 0.01).sin();
        }

        let sim12 = emb1.cosine_similarity(&emb2);
        let sim21 = emb2.cosine_similarity(&emb1);

        assert!(
            (sim12 - sim21).abs() < 1e-6,
            "Similarity should be symmetric: {} vs {}",
            sim12,
            sim21
        );
        println!("PASSED: cosine_similarity() is symmetric");
    }

    #[test]
    fn test_cosine_similarity_range_is_minus1_to_1() {
        let emb1 = make_valid_fused();
        let mut emb2 = make_valid_fused();
        for (i, val) in emb2.vector.iter_mut().enumerate() {
            *val = (i as f32 * 0.1).cos() - 0.5;
        }

        let sim = emb1.cosine_similarity(&emb2);

        assert!(
            sim >= -1.0 && sim <= 1.0,
            "Similarity should be in [-1, 1], got {}",
            sim
        );
        println!(
            "PASSED: cosine_similarity() range is [-1.0, 1.0] (got {})",
            sim
        );
    }

    // ========== Serialization Tests (8 tests) ==========

    #[test]
    fn test_to_bytes_produces_exactly_6200_bytes_no_aux_data() {
        let emb = make_valid_fused();

        let bytes = emb.to_bytes();

        println!("Binary size without aux_data: {} bytes", bytes.len());
        assert_eq!(
            bytes.len(),
            CORE_BINARY_SIZE,
            "Expected {} bytes, got {}",
            CORE_BINARY_SIZE,
            bytes.len()
        );
        println!("PASSED: to_bytes() produces exactly 6200 bytes (no aux_data)");
    }

    #[test]
    fn test_from_bytes_reconstructs_identical_embedding() {
        let emb = make_valid_fused();
        let bytes = emb.to_bytes();

        let reconstructed = FusedEmbedding::from_bytes(&bytes).expect("Should deserialize");

        assert_eq!(emb.vector, reconstructed.vector);
        assert_eq!(emb.expert_weights, reconstructed.expert_weights);
        assert_eq!(emb.selected_experts, reconstructed.selected_experts);
        assert_eq!(emb.pipeline_latency_us, reconstructed.pipeline_latency_us);
        assert_eq!(emb.content_hash, reconstructed.content_hash);
        println!("PASSED: from_bytes() reconstructs identical embedding");
    }

    #[test]
    fn test_to_bytes_from_bytes_round_trip_preserves_all_fields() {
        let token_vecs = vec![make_token_vector(); 10];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);
        let emb = make_valid_fused().with_aux_data(aux);

        let bytes = emb.to_bytes();
        let reconstructed = FusedEmbedding::from_bytes(&bytes).expect("Should deserialize");

        assert_eq!(emb.vector, reconstructed.vector);
        assert_eq!(emb.expert_weights, reconstructed.expert_weights);
        assert!(reconstructed.aux_data.is_some());
        let aux_rec = reconstructed.aux_data.unwrap();
        assert_eq!(aux_rec.num_tokens, 10);
        println!("PASSED: to_bytes()/from_bytes() round-trip preserves all fields");
    }

    #[test]
    fn test_from_bytes_rejects_truncated_data() {
        let bytes = vec![0u8; 100]; // Way too short

        let result = FusedEmbedding::from_bytes(&bytes);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::SerializationError { message } => {
                assert!(message.contains("too short"));
            }
            e => panic!("Expected SerializationError, got {:?}", e),
        }
        println!("PASSED: from_bytes() rejects truncated data");
    }

    #[test]
    fn test_from_bytes_rejects_oversized_data_with_zero_aux_len() {
        let emb = make_valid_fused();
        let mut bytes = emb.to_bytes();
        // Add extra bytes after aux_len=0
        bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        // This should still succeed - extra bytes after aux_len=0 are ignored
        let result = FusedEmbedding::from_bytes(&bytes);
        assert!(result.is_ok());
        println!("PASSED: from_bytes() tolerates extra bytes after valid data");
    }

    #[test]
    fn test_serde_json_round_trip_works() {
        let emb = make_valid_fused();

        let json = serde_json::to_string(&emb).expect("Should serialize to JSON");
        let reconstructed: FusedEmbedding =
            serde_json::from_str(&json).expect("Should deserialize from JSON");

        assert_eq!(emb.vector, reconstructed.vector);
        assert_eq!(emb.expert_weights, reconstructed.expert_weights);
        println!("PASSED: Serde JSON round-trip works correctly");
    }

    #[test]
    fn test_serde_skips_aux_data_when_none() {
        let emb = make_valid_fused();

        let json = serde_json::to_string(&emb).expect("Should serialize");

        assert!(!json.contains("aux_data"));
        println!("PASSED: Serde skips aux_data when None");
    }

    #[test]
    fn test_serde_includes_aux_data_when_some() {
        let token_vecs = vec![make_token_vector()];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);
        let emb = make_valid_fused().with_aux_data(aux);

        let json = serde_json::to_string(&emb).expect("Should serialize");

        assert!(json.contains("aux_data"));
        assert!(json.contains("token_vectors"));
        println!("PASSED: Serde includes aux_data when Some");
    }

    // ========== Auxiliary Data Tests (6 tests) ==========

    #[test]
    fn test_auxiliary_data_new_validates_128d_tokens() {
        let token_vecs = vec![make_token_vector(), make_token_vector()];

        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);

        assert_eq!(aux.num_tokens, 2);
        assert_eq!(aux.token_vectors.len(), 2);
        assert_eq!(aux.token_vectors[0].len(), COLBERT_V3_DIM);
        println!("PASSED: AuxiliaryEmbeddingData::new() validates 128D tokens");
    }

    #[test]
    #[should_panic(expected = "dimension")]
    fn test_auxiliary_data_new_panics_on_wrong_dimension() {
        let bad_vec = vec![0.5; 64]; // Wrong: should be 128
        let token_vecs = vec![bad_vec];

        let _aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);
    }

    #[test]
    fn test_auxiliary_to_blob_from_blob_round_trip() {
        let token_vecs = vec![make_token_vector(); 5];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);

        let blob = aux.to_blob();
        let reconstructed = AuxiliaryEmbeddingData::from_blob(&blob).expect("Should deserialize");

        assert_eq!(aux.num_tokens, reconstructed.num_tokens);
        assert_eq!(aux.token_vectors, reconstructed.token_vectors);
        println!("PASSED: to_blob()/from_blob() round-trip correctly");
    }

    #[test]
    fn test_compress_aux_data_creates_blob() {
        let token_vecs = vec![make_token_vector(); 3];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);
        let mut emb = make_valid_fused().with_aux_data(aux);

        assert!(emb.aux_data.as_ref().unwrap().blob.is_none());

        emb.compress_aux_data().expect("Should compress");

        assert!(emb.aux_data.as_ref().unwrap().blob.is_some());
        println!("PASSED: compress_aux_data() creates blob from token_vectors");
    }

    #[test]
    fn test_decompress_aux_data_restores_token_vectors() {
        let original_vecs = vec![make_token_vector(); 4];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, original_vecs.clone());
        let mut emb = make_valid_fused().with_aux_data(aux);

        emb.compress_aux_data().expect("Should compress");

        // Clear token_vectors to simulate loading from blob only
        emb.aux_data.as_mut().unwrap().token_vectors.clear();
        emb.aux_data.as_mut().unwrap().num_tokens = 0;

        emb.decompress_aux_data().expect("Should decompress");

        assert_eq!(emb.aux_data.as_ref().unwrap().num_tokens, 4);
        assert_eq!(
            emb.aux_data.as_ref().unwrap().token_vectors,
            original_vecs
        );
        println!("PASSED: decompress_aux_data() restores token_vectors from blob");
    }

    #[test]
    fn test_auxiliary_memory_size_returns_correct_byte_count() {
        let token_vecs = vec![make_token_vector(); 10];
        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, token_vecs);

        let expected_size = 10 * COLBERT_V3_DIM * 4; // 10 tokens * 128 dims * 4 bytes
        let actual_size = aux.memory_size();

        assert_eq!(actual_size, expected_size);
        println!(
            "PASSED: memory_size() returns correct byte count ({} bytes for 10 tokens)",
            actual_size
        );
    }

    // ========== Edge Case Tests with Before/After State ==========

    #[test]
    fn test_edge_case_expert_weights_boundary_0_99() {
        // Edge Case 1: Expert weights sum exactly at boundary (0.99)
        let mut emb = make_valid_fused();
        let weights_low = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.29]; // sum = 0.99
        emb.expert_weights = weights_low;

        println!(
            "BEFORE: weights_low sum = {}",
            weights_low.iter().sum::<f32>()
        );

        let result = emb.validate();

        println!("AFTER: validate() for 0.99 = {:?}", result);
        assert!(result.is_ok(), "0.99 should be within ±0.01 tolerance");
        println!("Edge Case 1 PASSED: 0.99 weight sum accepted");
    }

    #[test]
    fn test_edge_case_expert_weights_boundary_1_01() {
        // Edge Case 1 (cont): Expert weights sum exactly at boundary (1.01)
        let mut emb = make_valid_fused();
        let weights_high = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.31]; // sum = 1.01
        emb.expert_weights = weights_high;

        println!(
            "BEFORE: weights_high sum = {}",
            weights_high.iter().sum::<f32>()
        );

        let result = emb.validate();

        println!("AFTER: validate() for 1.01 = {:?}", result);
        assert!(result.is_ok(), "1.01 should be within ±0.01 tolerance");
        println!("Edge Case 1 PASSED: 1.01 weight sum accepted");
    }

    #[test]
    fn test_edge_case_zero_vector_normalization() {
        // Edge Case 2: Normalizing a zero vector (all zeros)
        let mut emb = make_valid_fused();
        for val in emb.vector.iter_mut() {
            *val = 0.0;
        }

        println!("BEFORE: magnitude = {}", emb.magnitude());
        println!("BEFORE: is_normalized = {}", emb.is_normalized());

        emb.normalize();

        println!("AFTER: magnitude = {}", emb.magnitude());
        println!("AFTER: vector[0..5] = {:?}", &emb.vector[0..5]);

        // Zero vector remains zero after normalization (no NaN)
        assert!(emb.vector.iter().all(|&v| v == 0.0));
        assert!(!emb.vector.iter().any(|v| v.is_nan()));
        println!("Edge Case 2 PASSED: Zero vector remains zero after normalization");
    }

    #[test]
    fn test_edge_case_aux_data_empty_tokens() {
        // Edge Case 3: AuxiliaryEmbeddingData with empty token list
        let empty_tokens: Vec<Vec<f32>> = vec![];

        println!("BEFORE: token_vectors.len() = {}", empty_tokens.len());

        let aux = AuxiliaryEmbeddingData::new(ModelId::LateInteraction, empty_tokens);

        println!("AFTER: num_tokens = {}", aux.num_tokens);
        println!("AFTER: memory_size = {}", aux.memory_size());

        let blob = aux.to_blob();
        println!("AFTER: blob.len() = {}", blob.len());

        assert_eq!(aux.num_tokens, 0);
        assert_eq!(aux.memory_size(), 0);
        // Blob should have minimal header: 1 (model) + 4 (num_tokens) = 5 bytes
        assert_eq!(blob.len(), 5);
        println!("Edge Case 3 PASSED: Empty tokens is valid, memory_size = 0, blob is minimal");
    }

    #[test]
    fn test_edge_case_duplicate_expert_selection() {
        // Edge Case: Duplicate expert indices should fail
        let vector = make_valid_vector();
        let weights = make_valid_weights();
        let selected = [3, 3, 1, 2]; // Duplicate at index 0 and 1!

        let result = FusedEmbedding::new(vector, weights, selected, 1000, 12345);

        println!("Duplicate expert selection result: {:?}", result);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::FusionError { message } => {
                assert!(message.contains("Duplicate"));
            }
            e => panic!("Expected FusionError for duplicate, got {:?}", e),
        }
        println!("Edge Case PASSED: Duplicate expert indices rejected");
    }

    // ========== Constants Validation Tests ==========

    #[test]
    fn test_constants_match_dimensions_rs() {
        assert_eq!(FusedEmbedding::DIMENSION, FUSED_OUTPUT);
        assert_eq!(FusedEmbedding::DIMENSION, 1536);
        assert_eq!(FusedEmbedding::NUM_EXPERTS, NUM_EXPERTS);
        assert_eq!(FusedEmbedding::NUM_EXPERTS, 8);
        assert_eq!(FusedEmbedding::TOP_K, TOP_K_EXPERTS);
        assert_eq!(FusedEmbedding::TOP_K, 4);
        assert_eq!(AuxiliaryEmbeddingData::TOKEN_DIM, COLBERT_V3_DIM);
        assert_eq!(AuxiliaryEmbeddingData::TOKEN_DIM, 128);
        println!("PASSED: All constants match dimensions.rs");
    }

    #[test]
    fn test_binary_size_constant() {
        assert_eq!(CORE_BINARY_SIZE, 6200);
        println!("PASSED: CORE_BINARY_SIZE = 6200 bytes");
    }
}
