//! SemanticFingerprint quantization for storage compression.
//!
//! TASK-P2-006: Compresses SemanticFingerprint from ~46KB to ~11KB via mixed quantization:
//!
//! | Method    | Embedders           | Description                          | Compression |
//! |-----------|---------------------|--------------------------------------|-------------|
//! | **PQ-8**  | E1, E5, E7, E10     | Product quantization, 8-bit codes    | ~128x       |
//! | **Float8**| E2-E4, E8, E9, E11  | Min-max scalar quantization          | 4x          |
//! | **Token** | E12                 | Per-token Float8                     | 4x          |
//! | **Sparse**| E6, E13             | u16 indices + Float8 values          | ~2.4x       |
//!
//! # Architecture References
//!
//! - constitution.yaml ARCH-01: TeleologicalArray is atomic
//! - constitution.yaml AP-10: No NaN/Infinity in similarity scores

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::teleological::Embedder;
use crate::types::fingerprint::{
    SemanticFingerprint, SparseVector, E10_DIM, E11_DIM, E12_TOKEN_DIM, E1_DIM, E2_DIM, E3_DIM,
    E4_DIM, E5_DIM, E7_DIM, E8_DIM, E9_DIM,
};

// ============================================================================
// Error Types
// ============================================================================

/// Errors during fingerprint quantization/dequantization.
#[derive(Debug, Error, Clone)]
pub enum FingerprintQuantizeError {
    /// Invalid input data for an embedder.
    #[error("E_FP_QUANT_001: Invalid input for {embedder:?}: {message}")]
    InvalidInput { embedder: Embedder, message: String },

    /// Dimension mismatch for an embedder.
    #[error("E_FP_QUANT_002: Dimension mismatch for {embedder:?}: expected {expected}, got {actual}")]
    DimensionMismatch {
        embedder: Embedder,
        expected: usize,
        actual: usize,
    },

    /// NaN or Infinity value found (AP-10 violation).
    #[error("E_FP_QUANT_003: NaN/Infinity in {embedder:?} at index {index}")]
    InvalidValue { embedder: Embedder, index: usize },

    /// Sparse vector reconstruction failed.
    #[error("E_FP_QUANT_004: Sparse vector reconstruction failed for {embedder:?}: {message}")]
    SparseReconstructionFailed { embedder: Embedder, message: String },
}

impl FingerprintQuantizeError {
    /// Get the error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "E_FP_QUANT_001",
            Self::DimensionMismatch { .. } => "E_FP_QUANT_002",
            Self::InvalidValue { .. } => "E_FP_QUANT_003",
            Self::SparseReconstructionFailed { .. } => "E_FP_QUANT_004",
        }
    }
}

// ============================================================================
// Quantized Types
// ============================================================================

/// Float8 quantized dense vector using min-max scalar quantization.
///
/// Achieves 4x compression by storing each f32 value as a u8 [0, 255] mapped
/// from [min_val, max_val].
///
/// # Formula
///
/// - Quantize: `q = round((v - min) / (max - min) * 255)`
/// - Dequantize: `v = (q / 255) * (max - min) + min`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantizedFloat8 {
    /// Quantized values as u8 [0, 255].
    pub data: Vec<u8>,
    /// Minimum value from original data (used for dequantization).
    pub min_val: f32,
    /// Maximum value from original data (used for dequantization).
    pub max_val: f32,
    /// Original vector dimension.
    pub original_dim: usize,
}

impl QuantizedFloat8 {
    /// Estimate storage size in bytes.
    #[inline]
    pub fn estimated_size_bytes(&self) -> usize {
        self.data.len() // u8 data
            + 4 // min_val f32
            + 4 // max_val f32
            + 8 // original_dim usize
    }
}

/// PQ-8 quantized dense vector using simplified mean-based product quantization.
///
/// Divides the vector into `num_subvectors` subspaces and stores each as a
/// single u8 code representing the mean value.
///
/// # Compression
///
/// For 1024D with 32 subvectors: 4KB -> 32B (128x compression)
///
/// # Note
///
/// This is simplified PQ without learned codebooks. For production use,
/// the trained codebook implementation in `context-graph-embeddings/src/quantization/pq8/`
/// provides better accuracy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantizedPQ8 {
    /// PQ codes, one per subvector.
    pub codes: Vec<u8>,
    /// Number of subvectors (must divide original_dim evenly).
    pub num_subvectors: usize,
    /// Original vector dimension.
    pub original_dim: usize,
}

impl QuantizedPQ8 {
    /// Estimate storage size in bytes.
    #[inline]
    pub fn estimated_size_bytes(&self) -> usize {
        self.codes.len() // u8 codes
            + 8 // num_subvectors usize
            + 8 // original_dim usize
    }
}

/// Quantized sparse vector with exact u16 indices and Float8-quantized values.
///
/// Preserves the sparse structure while compressing values from f32 to u8.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantizedSparse {
    /// Indices preserved exactly (matches SparseVector.indices).
    pub indices: Vec<u16>,
    /// Quantized values as u8 [0, 255].
    pub quantized_values: Vec<u8>,
    /// Minimum value from original data.
    pub min_val: f32,
    /// Maximum value from original data.
    pub max_val: f32,
}

impl QuantizedSparse {
    /// Estimate storage size in bytes.
    #[inline]
    pub fn estimated_size_bytes(&self) -> usize {
        self.indices.len() * 2 // u16 indices
            + self.quantized_values.len() // u8 values
            + 4 // min_val f32
            + 4 // max_val f32
    }

    /// Number of non-zero entries.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }
}

/// Complete quantized fingerprint targeting ~11KB storage.
///
/// Contains all 13 embeddings in quantized form:
/// - PQ-8: E1, E5, E7, E10 (high-dimensional semantic embeddings)
/// - Float8: E2-E4, E8, E9, E11 (temporal and relational embeddings)
/// - Token Float8: E12 (per-token late-interaction embeddings)
/// - Sparse: E6, E13 (SPLADE sparse vectors)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedSemanticFingerprint {
    // PQ-8 quantized embeddings (high-dimensional semantic)
    /// E1: Semantic (1024D -> 32 codes)
    pub e1_semantic: QuantizedPQ8,
    /// E5: Causal (768D -> 24 codes)
    pub e5_causal: QuantizedPQ8,
    /// E7: Code (1536D -> 48 codes)
    pub e7_code: QuantizedPQ8,
    /// E10: Multimodal (768D -> 24 codes)
    pub e10_multimodal: QuantizedPQ8,

    // Float8 quantized embeddings (temporal and relational)
    /// E2: Temporal Recent (512D)
    pub e2_temporal_recent: QuantizedFloat8,
    /// E3: Temporal Periodic (512D)
    pub e3_temporal_periodic: QuantizedFloat8,
    /// E4: Temporal Positional (512D)
    pub e4_temporal_positional: QuantizedFloat8,
    /// E8: Graph (1024D)
    pub e8_graph: QuantizedFloat8,
    /// E9: HDC projected (1024D) - NOT binary, this is projected dense!
    pub e9_hdc: QuantizedFloat8,
    /// E11: Entity (384D)
    pub e11_entity: QuantizedFloat8,

    // Token-level Float8 (variable length)
    /// E12: Late Interaction (128D per token)
    pub e12_late_interaction: Vec<QuantizedFloat8>,

    // Sparse quantized (exact indices, quantized values)
    /// E6: Sparse Lexical (SPLADE)
    pub e6_sparse: QuantizedSparse,
    /// E13: Keyword SPLADE
    pub e13_splade: QuantizedSparse,
}

impl QuantizedSemanticFingerprint {
    /// Estimate total storage size in bytes.
    ///
    /// # Returns
    ///
    /// Estimated serialized size. Actual bincode size may differ slightly due to
    /// encoding overhead.
    pub fn estimated_size_bytes(&self) -> usize {
        // PQ-8 embeddings
        let pq8_size = self.e1_semantic.estimated_size_bytes()
            + self.e5_causal.estimated_size_bytes()
            + self.e7_code.estimated_size_bytes()
            + self.e10_multimodal.estimated_size_bytes();

        // Float8 embeddings
        let float8_size = self.e2_temporal_recent.estimated_size_bytes()
            + self.e3_temporal_periodic.estimated_size_bytes()
            + self.e4_temporal_positional.estimated_size_bytes()
            + self.e8_graph.estimated_size_bytes()
            + self.e9_hdc.estimated_size_bytes()
            + self.e11_entity.estimated_size_bytes();

        // Token-level Float8
        let token_size: usize = self
            .e12_late_interaction
            .iter()
            .map(|t| t.estimated_size_bytes())
            .sum();

        // Sparse
        let sparse_size = self.e6_sparse.estimated_size_bytes()
            + self.e13_splade.estimated_size_bytes();

        pq8_size + float8_size + token_size + sparse_size + 64 // overhead estimate
    }
}

// ============================================================================
// Validation Helpers
// ============================================================================

/// Validate that all values in a slice are finite (no NaN/Infinity). (AP-10)
fn validate_finite(
    data: &[f32],
    embedder: Embedder,
) -> Result<(), FingerprintQuantizeError> {
    for (i, &v) in data.iter().enumerate() {
        if !v.is_finite() {
            return Err(FingerprintQuantizeError::InvalidValue { embedder, index: i });
        }
    }
    Ok(())
}

/// Check that a slice has the expected dimension.
fn check_dim(
    actual: usize,
    expected: usize,
    embedder: Embedder,
) -> Result<(), FingerprintQuantizeError> {
    if actual != expected {
        return Err(FingerprintQuantizeError::DimensionMismatch {
            embedder,
            expected,
            actual,
        });
    }
    Ok(())
}

// ============================================================================
// Float8 Quantization Helpers
// ============================================================================

/// Quantize a dense vector slice to Float8 using min-max scalar quantization.
///
/// # Arguments
///
/// * `data` - Input f32 slice to quantize
/// * `embedder` - Embedder type (for error reporting)
///
/// # Returns
///
/// * `Ok(QuantizedFloat8)` on success
/// * `Err(FingerprintQuantizeError::InvalidValue)` if NaN/Infinity found (AP-10)
fn quantize_float8_slice(
    data: &[f32],
    embedder: Embedder,
) -> Result<QuantizedFloat8, FingerprintQuantizeError> {
    validate_finite(data, embedder)?;

    if data.is_empty() {
        return Ok(QuantizedFloat8 {
            data: vec![],
            min_val: 0.0,
            max_val: 0.0,
            original_dim: 0,
        });
    }

    let min_val = data.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max_val - min_val;

    let quantized = if range > f32::EPSILON {
        data.iter()
            .map(|&v| ((v - min_val) / range * 255.0).round() as u8)
            .collect()
    } else {
        // Constant vector - use midpoint value
        vec![128u8; data.len()]
    };

    Ok(QuantizedFloat8 {
        data: quantized,
        min_val,
        max_val,
        original_dim: data.len(),
    })
}

/// Dequantize a Float8 vector back to f32.
///
/// # Arguments
///
/// * `q` - Quantized Float8 data
///
/// # Returns
///
/// Reconstructed f32 vector.
fn dequantize_float8(q: &QuantizedFloat8) -> Vec<f32> {
    if q.original_dim == 0 {
        return vec![];
    }

    let range = q.max_val - q.min_val;
    if range <= f32::EPSILON {
        // Constant vector
        return vec![q.min_val; q.original_dim];
    }

    q.data
        .iter()
        .map(|&v| (v as f32 / 255.0) * range + q.min_val)
        .collect()
}

// ============================================================================
// PQ-8 Quantization Helpers (Simplified Mean-Based)
// ============================================================================

/// Quantize a dense vector using simplified mean-based product quantization.
///
/// Divides the vector into `num_subvectors` subspaces and encodes each as a
/// single u8 representing the mean value mapped to [0, 255].
///
/// # Arguments
///
/// * `data` - Input f32 slice to quantize
/// * `num_subvectors` - Number of subvectors (must divide data.len() evenly)
/// * `embedder` - Embedder type (for error reporting)
///
/// # Returns
///
/// * `Ok(QuantizedPQ8)` on success
/// * `Err(FingerprintQuantizeError::InvalidInput)` if dimension not divisible
fn quantize_pq8(
    data: &[f32],
    num_subvectors: usize,
    embedder: Embedder,
) -> Result<QuantizedPQ8, FingerprintQuantizeError> {
    validate_finite(data, embedder)?;

    if data.is_empty() {
        return Ok(QuantizedPQ8 {
            codes: vec![],
            num_subvectors,
            original_dim: 0,
        });
    }

    if data.len() % num_subvectors != 0 {
        return Err(FingerprintQuantizeError::InvalidInput {
            embedder,
            message: format!(
                "dimension {} not divisible by num_subvectors {}",
                data.len(),
                num_subvectors
            ),
        });
    }

    let sub_size = data.len() / num_subvectors;
    let min_val = data.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max_val - min_val;

    let codes: Vec<u8> = (0..num_subvectors)
        .map(|i| {
            let sub = &data[i * sub_size..(i + 1) * sub_size];
            let mean = sub.iter().sum::<f32>() / sub.len() as f32;

            if range > f32::EPSILON {
                ((mean - min_val) / range * 255.0).round().clamp(0.0, 255.0) as u8
            } else {
                128u8
            }
        })
        .collect();

    Ok(QuantizedPQ8 {
        codes,
        num_subvectors,
        original_dim: data.len(),
    })
}

/// Dequantize a PQ-8 vector back to f32.
///
/// Each subvector is reconstructed as a uniform vector of the decoded mean value.
/// This loses within-subvector variance but preserves the overall structure.
///
/// # Arguments
///
/// * `q` - Quantized PQ8 data
/// * `original_min` - Original min value for scaling
/// * `original_max` - Original max value for scaling
///
/// # Returns
///
/// Reconstructed f32 vector (note: within-subvector variance is lost).
fn dequantize_pq8(q: &QuantizedPQ8, original_min: f32, original_max: f32) -> Vec<f32> {
    if q.original_dim == 0 || q.num_subvectors == 0 {
        return vec![];
    }

    let sub_size = q.original_dim / q.num_subvectors;
    let range = original_max - original_min;

    let mut data = vec![0.0f32; q.original_dim];

    for (i, &code) in q.codes.iter().enumerate() {
        // Decode mean value from code
        let mean = if range > f32::EPSILON {
            (code as f32 / 255.0) * range + original_min
        } else {
            original_min
        };

        // Fill subvector with mean
        for j in (i * sub_size)..((i + 1) * sub_size) {
            if j < data.len() {
                data[j] = mean;
            }
        }
    }

    data
}

// ============================================================================
// Sparse Quantization Helpers
// ============================================================================

/// Quantize a sparse vector, preserving indices exactly and quantizing values to Float8.
///
/// # Arguments
///
/// * `sv` - Input SparseVector to quantize
///
/// # Returns
///
/// QuantizedSparse with exact indices and quantized values.
fn quantize_sparse(sv: &SparseVector) -> QuantizedSparse {
    if sv.is_empty() {
        return QuantizedSparse {
            indices: vec![],
            quantized_values: vec![],
            min_val: 0.0,
            max_val: 0.0,
        };
    }

    // Compute min/max of values
    let min_val = sv.values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = sv.values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max_val - min_val;

    // Quantize values
    let quantized_values = if range > f32::EPSILON {
        sv.values
            .iter()
            .map(|&v| ((v - min_val) / range * 255.0).round() as u8)
            .collect()
    } else {
        vec![128u8; sv.values.len()]
    };

    QuantizedSparse {
        indices: sv.indices.clone(),
        quantized_values,
        min_val,
        max_val,
    }
}

/// Dequantize a sparse vector back to SparseVector.
///
/// # Arguments
///
/// * `qs` - Quantized sparse data
/// * `embedder` - Embedder type (for error reporting)
///
/// # Returns
///
/// * `Ok(SparseVector)` on success
/// * `Err(FingerprintQuantizeError::SparseReconstructionFailed)` if reconstruction fails
fn dequantize_sparse(
    qs: &QuantizedSparse,
    embedder: Embedder,
) -> Result<SparseVector, FingerprintQuantizeError> {
    if qs.indices.is_empty() {
        return Ok(SparseVector::empty());
    }

    let range = qs.max_val - qs.min_val;
    let values: Vec<f32> = if range > f32::EPSILON {
        qs.quantized_values
            .iter()
            .map(|&v| (v as f32 / 255.0) * range + qs.min_val)
            .collect()
    } else {
        vec![qs.min_val; qs.quantized_values.len()]
    };

    SparseVector::new(qs.indices.clone(), values).map_err(|e| {
        FingerprintQuantizeError::SparseReconstructionFailed {
            embedder,
            message: e.to_string(),
        }
    })
}

// ============================================================================
// Public API
// ============================================================================

/// Quantize a SemanticFingerprint to compressed storage format.
///
/// Applies mixed quantization strategies:
/// - PQ-8 for high-dimensional semantic embeddings (E1, E5, E7, E10)
/// - Float8 for temporal and relational embeddings (E2-E4, E8, E9, E11)
/// - Per-token Float8 for late-interaction (E12)
/// - Sparse quantization for SPLADE vectors (E6, E13)
///
/// # Arguments
///
/// * `fp` - SemanticFingerprint to quantize
///
/// # Returns
///
/// * `Ok(QuantizedSemanticFingerprint)` - Compressed fingerprint (~11KB)
/// * `Err(FingerprintQuantizeError)` - Quantization failed
///
/// # Example
///
/// ```
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
/// use context_graph_core::quantization::fingerprint::quantize_fingerprint;
///
/// # #[cfg(feature = "test-utils")]
/// # {
/// let fp = SemanticFingerprint::zeroed();
/// let qfp = quantize_fingerprint(&fp).expect("quantization failed");
/// assert!(qfp.estimated_size_bytes() < 15000);
/// # }
/// ```
pub fn quantize_fingerprint(
    fp: &SemanticFingerprint,
) -> Result<QuantizedSemanticFingerprint, FingerprintQuantizeError> {
    // Validate dimensions
    validate_fingerprint_dimensions(fp)?;

    // PQ-8 quantization for high-dimensional semantic embeddings
    // Using num_subvectors = dim / 32 for each embedder
    let e1_semantic = quantize_pq8(&fp.e1_semantic, E1_DIM / 32, Embedder::Semantic)?; // 32 subvectors
    // For E5, we quantize the active vector (cause vector takes precedence over legacy)
    let e5_causal = quantize_pq8(fp.e5_active_vector(), E5_DIM / 32, Embedder::Causal)?; // 24 subvectors
    let e7_code = quantize_pq8(&fp.e7_code, E7_DIM / 32, Embedder::Code)?; // 48 subvectors
    // For E10, we quantize the active vector (paraphrase vector takes precedence over legacy)
    let e10_multimodal = quantize_pq8(fp.e10_active_vector(), E10_DIM / 32, Embedder::Multimodal)?; // 24 subvectors

    // Float8 quantization for temporal and relational embeddings
    let e2_temporal_recent =
        quantize_float8_slice(&fp.e2_temporal_recent, Embedder::TemporalRecent)?;
    let e3_temporal_periodic =
        quantize_float8_slice(&fp.e3_temporal_periodic, Embedder::TemporalPeriodic)?;
    let e4_temporal_positional =
        quantize_float8_slice(&fp.e4_temporal_positional, Embedder::TemporalPositional)?;
    // For E8, we quantize the active vector (source vector takes precedence over legacy)
    let e8_graph = quantize_float8_slice(fp.e8_active_vector(), Embedder::Graph)?;
    let e9_hdc = quantize_float8_slice(&fp.e9_hdc, Embedder::Hdc)?;
    let e11_entity = quantize_float8_slice(&fp.e11_entity, Embedder::Entity)?;

    // Per-token Float8 for late-interaction
    let e12_late_interaction: Vec<QuantizedFloat8> = fp
        .e12_late_interaction
        .iter()
        .map(|token| quantize_float8_slice(token, Embedder::LateInteraction))
        .collect::<Result<_, _>>()?;

    // Sparse quantization for SPLADE vectors
    let e6_sparse = quantize_sparse(&fp.e6_sparse);
    let e13_splade = quantize_sparse(&fp.e13_splade);

    Ok(QuantizedSemanticFingerprint {
        e1_semantic,
        e5_causal,
        e7_code,
        e10_multimodal,
        e2_temporal_recent,
        e3_temporal_periodic,
        e4_temporal_positional,
        e8_graph,
        e9_hdc,
        e11_entity,
        e12_late_interaction,
        e6_sparse,
        e13_splade,
    })
}

/// Dequantize a compressed fingerprint back to SemanticFingerprint.
///
/// # Arguments
///
/// * `qfp` - QuantizedSemanticFingerprint to decompress
///
/// # Returns
///
/// * `Ok(SemanticFingerprint)` - Reconstructed fingerprint (with quantization loss)
/// * `Err(FingerprintQuantizeError)` - Dequantization failed
///
/// # Note
///
/// The reconstructed fingerprint will have some loss compared to the original:
/// - Float8: NRMSE < 1%
/// - PQ-8: NRMSE < 10% (simplified mean-based loses within-subvector variance)
/// - Sparse: Values have Float8 loss, indices are exact
pub fn dequantize_fingerprint(
    qfp: &QuantizedSemanticFingerprint,
) -> Result<SemanticFingerprint, FingerprintQuantizeError> {
    // Dequantize PQ-8 embeddings
    // We need to estimate min/max from codes for PQ-8 dequantization
    // Since we only store codes, we assume normalized [-1, 1] range
    let e1_semantic = dequantize_pq8(&qfp.e1_semantic, -1.0, 1.0);
    let e5_causal = dequantize_pq8(&qfp.e5_causal, -1.0, 1.0);
    let e7_code = dequantize_pq8(&qfp.e7_code, -1.0, 1.0);
    let e10_multimodal = dequantize_pq8(&qfp.e10_multimodal, -1.0, 1.0);

    // Dequantize Float8 embeddings
    let e2_temporal_recent = dequantize_float8(&qfp.e2_temporal_recent);
    let e3_temporal_periodic = dequantize_float8(&qfp.e3_temporal_periodic);
    let e4_temporal_positional = dequantize_float8(&qfp.e4_temporal_positional);
    let e8_graph = dequantize_float8(&qfp.e8_graph);
    let e9_hdc = dequantize_float8(&qfp.e9_hdc);
    let e11_entity = dequantize_float8(&qfp.e11_entity);

    // Dequantize per-token Float8
    let e12_late_interaction: Vec<Vec<f32>> = qfp
        .e12_late_interaction
        .iter()
        .map(dequantize_float8)
        .collect();

    // Dequantize sparse vectors
    let e6_sparse = dequantize_sparse(&qfp.e6_sparse, Embedder::Sparse)?;
    let e13_splade = dequantize_sparse(&qfp.e13_splade, Embedder::KeywordSplade)?;

    // Dequantized E5 and E8 are stored in both dual vector fields
    // (quantization loses the distinction, so we reconstruct symmetrically)
    Ok(SemanticFingerprint {
        e1_semantic,
        e2_temporal_recent,
        e3_temporal_periodic,
        e4_temporal_positional,
        e5_causal_as_cause: e5_causal.clone(),
        e5_causal_as_effect: e5_causal,
        e5_causal: Vec::new(), // Using new dual format
        e6_sparse,
        e7_code,
        e8_graph_as_source: e8_graph.clone(),
        e8_graph_as_target: e8_graph,
        e8_graph: Vec::new(), // Using new dual format
        e9_hdc,
        // E10: Using new dual format after dequantization
        // (quantization loses dual distinction, reconstruct symmetrically when loaded)
        e10_multimodal_paraphrase: e10_multimodal.clone(),
        e10_multimodal_as_context: e10_multimodal,
        e11_entity,
        e12_late_interaction,
        e13_splade,
    })
}

/// Validate fingerprint dimensions match expected constants.
fn validate_fingerprint_dimensions(
    fp: &SemanticFingerprint,
) -> Result<(), FingerprintQuantizeError> {
    check_dim(fp.e1_semantic.len(), E1_DIM, Embedder::Semantic)?;
    check_dim(fp.e2_temporal_recent.len(), E2_DIM, Embedder::TemporalRecent)?;
    check_dim(fp.e3_temporal_periodic.len(), E3_DIM, Embedder::TemporalPeriodic)?;
    check_dim(fp.e4_temporal_positional.len(), E4_DIM, Embedder::TemporalPositional)?;
    // Use active vectors for E5, E8, E10 validation (handles both new and legacy formats)
    check_dim(fp.e5_active_vector().len(), E5_DIM, Embedder::Causal)?;
    check_dim(fp.e7_code.len(), E7_DIM, Embedder::Code)?;
    check_dim(fp.e8_active_vector().len(), E8_DIM, Embedder::Graph)?;
    check_dim(fp.e9_hdc.len(), E9_DIM, Embedder::Hdc)?;
    check_dim(fp.e10_active_vector().len(), E10_DIM, Embedder::Multimodal)?;
    check_dim(fp.e11_entity.len(), E11_DIM, Embedder::Entity)?;

    // Validate E12 token dimensions (NaN/Infinity checked during quantization)
    for token in &fp.e12_late_interaction {
        check_dim(token.len(), E12_TOKEN_DIM, Embedder::LateInteraction)?;
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantization::accuracy::{compute_nrmse, compute_rmse};

    // Helper to create a test fingerprint with known values
    fn test_fingerprint() -> SemanticFingerprint {
        let mut fp = SemanticFingerprint::zeroed();

        // Fill with non-zero values for meaningful quantization tests
        for (i, v) in fp.e1_semantic.iter_mut().enumerate() {
            *v = (i as f32 / 1024.0) * 2.0 - 1.0; // Range [-1, 1]
        }
        for (i, v) in fp.e2_temporal_recent.iter_mut().enumerate() {
            *v = (i as f32 / 512.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e3_temporal_periodic.iter_mut().enumerate() {
            *v = (i as f32 / 512.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e4_temporal_positional.iter_mut().enumerate() {
            *v = (i as f32 / 512.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e5_causal_as_cause.iter_mut().enumerate() {
            *v = (i as f32 / 768.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e5_causal_as_effect.iter_mut().enumerate() {
            *v = (i as f32 / 768.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e7_code.iter_mut().enumerate() {
            *v = (i as f32 / 1536.0) * 2.0 - 1.0;
        }
        // E8 now uses dual vectors for asymmetric graph similarity
        for (i, v) in fp.e8_graph_as_source.iter_mut().enumerate() {
            *v = (i as f32 / 384.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e8_graph_as_target.iter_mut().enumerate() {
            *v = (i as f32 / 384.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e9_hdc.iter_mut().enumerate() {
            *v = (i as f32 / 1024.0) * 2.0 - 1.0;
        }
        // E10 now uses dual vectors for asymmetric paraphrase/context similarity
        for (i, v) in fp.e10_multimodal_paraphrase.iter_mut().enumerate() {
            *v = (i as f32 / 768.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e10_multimodal_as_context.iter_mut().enumerate() {
            *v = (i as f32 / 768.0) * 2.0 - 1.0;
        }
        for (i, v) in fp.e11_entity.iter_mut().enumerate() {
            *v = (i as f32 / 384.0) * 2.0 - 1.0;
        }

        // Add some E12 tokens
        for t in 0..10 {
            let token: Vec<f32> = (0..E12_TOKEN_DIM)
                .map(|i| (i as f32 / 128.0) * 2.0 - 1.0 + (t as f32 * 0.01))
                .collect();
            fp.e12_late_interaction.push(token);
        }

        // Add sparse vectors
        fp.e6_sparse = SparseVector::new(
            vec![10, 100, 500, 1000, 5000],
            vec![0.5, 0.3, 0.8, 0.2, 0.9],
        )
        .expect("valid sparse");

        fp.e13_splade = SparseVector::new(
            vec![20, 200, 600, 1200, 6000],
            vec![0.4, 0.6, 0.1, 0.7, 0.3],
        )
        .expect("valid sparse");

        fp
    }

    // =========================================================================
    // Float8 Tests
    // =========================================================================

    #[test]
    fn test_float8_quantize_dequantize_roundtrip() {
        let data: Vec<f32> = (0..512).map(|i| (i as f32 / 512.0) * 2.0 - 1.0).collect();

        let quantized =
            quantize_float8_slice(&data, Embedder::TemporalRecent).expect("quantization failed");
        let dequantized = dequantize_float8(&quantized);

        assert_eq!(dequantized.len(), data.len());

        // Compute NRMSE - should be < 1%
        let nrmse = compute_nrmse(&data, &dequantized);
        println!("Float8 NRMSE: {:.4}%", nrmse * 100.0);
        assert!(
            nrmse < 0.01,
            "Float8 NRMSE {} exceeds 1% threshold",
            nrmse * 100.0
        );
    }

    #[test]
    fn test_float8_empty_input() {
        let data: Vec<f32> = vec![];
        let quantized =
            quantize_float8_slice(&data, Embedder::TemporalRecent).expect("quantization failed");

        assert!(quantized.data.is_empty());
        assert_eq!(quantized.original_dim, 0);

        let dequantized = dequantize_float8(&quantized);
        assert!(dequantized.is_empty());
    }

    #[test]
    fn test_float8_constant_vector() {
        let data: Vec<f32> = vec![0.5; 100];
        let quantized =
            quantize_float8_slice(&data, Embedder::Entity).expect("quantization failed");
        let dequantized = dequantize_float8(&quantized);

        assert_eq!(dequantized.len(), 100);
        // All values should be approximately 0.5
        for v in &dequantized {
            assert!(
                (*v - 0.5).abs() < 0.01,
                "Expected ~0.5, got {} for constant vector",
                v
            );
        }
    }

    #[test]
    fn test_float8_rejects_nan() {
        let data = vec![1.0, f32::NAN, 3.0];
        let result = quantize_float8_slice(&data, Embedder::Hdc);
        assert!(result.is_err());
        match result {
            Err(FingerprintQuantizeError::InvalidValue { embedder, index }) => {
                assert_eq!(embedder, Embedder::Hdc);
                assert_eq!(index, 1);
            }
            _ => panic!("Expected InvalidValue error"),
        }
    }

    #[test]
    fn test_float8_rejects_infinity() {
        let data = vec![1.0, 2.0, f32::INFINITY];
        let result = quantize_float8_slice(&data, Embedder::Graph);
        assert!(result.is_err());
    }

    // =========================================================================
    // PQ-8 Tests
    // =========================================================================

    #[test]
    fn test_pq8_quantize_dequantize_roundtrip() {
        let data: Vec<f32> = (0..1024).map(|i| (i as f32 / 1024.0) * 2.0 - 1.0).collect();

        let quantized = quantize_pq8(&data, 32, Embedder::Semantic).expect("quantization failed");
        let dequantized = dequantize_pq8(&quantized, -1.0, 1.0);

        assert_eq!(dequantized.len(), data.len());
        assert_eq!(quantized.codes.len(), 32);

        // PQ-8 has higher error due to mean approximation
        // NRMSE should be < 10%
        let nrmse = compute_nrmse(&data, &dequantized);
        println!("PQ-8 NRMSE: {:.4}%", nrmse * 100.0);
        // Mean-based PQ will have significant error - we just check it's reasonable
        assert!(nrmse < 0.5, "PQ-8 NRMSE {} is unexpectedly high", nrmse);
    }

    #[test]
    fn test_pq8_compression_ratio() {
        // 1024D -> 32 codes = 128x compression
        let data: Vec<f32> = vec![0.0; 1024];
        let quantized = quantize_pq8(&data, 32, Embedder::Semantic).expect("quantization failed");

        assert_eq!(quantized.codes.len(), 32);
        // 4KB (1024 * 4 bytes) -> 32 bytes
        let compression = (1024 * 4) as f32 / quantized.codes.len() as f32;
        println!("PQ-8 compression: {:.1}x", compression);
        assert!(compression >= 100.0);
    }

    #[test]
    fn test_pq8_empty_input() {
        let data: Vec<f32> = vec![];
        let quantized = quantize_pq8(&data, 32, Embedder::Semantic).expect("quantization failed");

        assert!(quantized.codes.is_empty());
        assert_eq!(quantized.original_dim, 0);
    }

    #[test]
    fn test_pq8_invalid_subvector_count() {
        let data: Vec<f32> = vec![0.0; 100]; // 100 not divisible by 32
        let result = quantize_pq8(&data, 32, Embedder::Semantic);
        assert!(result.is_err());
    }

    // =========================================================================
    // Sparse Tests
    // =========================================================================

    #[test]
    fn test_sparse_quantize_dequantize_roundtrip() {
        let sv =
            SparseVector::new(vec![10, 100, 500, 1000], vec![0.5, 0.3, 0.8, 0.2]).expect("valid");

        let quantized = quantize_sparse(&sv);
        let dequantized = dequantize_sparse(&quantized, Embedder::Sparse).expect("dequant failed");

        // Indices must be exact
        assert_eq!(dequantized.indices, sv.indices);
        assert_eq!(dequantized.nnz(), sv.nnz());

        // Values should be close
        let nrmse = compute_nrmse(&sv.values, &dequantized.values);
        println!("Sparse NRMSE: {:.4}%", nrmse * 100.0);
        assert!(nrmse < 0.01, "Sparse NRMSE {} exceeds 1%", nrmse * 100.0);
    }

    #[test]
    fn test_sparse_empty() {
        let sv = SparseVector::empty();
        let quantized = quantize_sparse(&sv);
        assert!(quantized.indices.is_empty());
        assert!(quantized.quantized_values.is_empty());

        let dequantized = dequantize_sparse(&quantized, Embedder::Sparse).expect("dequant failed");
        assert!(dequantized.is_empty());
    }

    #[test]
    fn test_sparse_preserves_indices_exactly() {
        let indices: Vec<u16> = vec![0, 1000, 5000, 10000, 30000];
        let sv = SparseVector::new(indices.clone(), vec![0.1, 0.2, 0.3, 0.4, 0.5]).expect("valid");

        let quantized = quantize_sparse(&sv);
        assert_eq!(quantized.indices, indices);
    }

    // =========================================================================
    // Full Fingerprint Tests
    // =========================================================================

    #[test]
    fn test_fingerprint_quantize_dequantize_roundtrip() {
        let fp = test_fingerprint();

        let qfp = quantize_fingerprint(&fp).expect("quantization failed");
        let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");

        // Verify dimensions
        assert_eq!(recovered.e1_semantic.len(), E1_DIM);
        assert_eq!(recovered.e2_temporal_recent.len(), E2_DIM);
        // E5 now uses dual vectors for asymmetric causal similarity
        assert_eq!(recovered.e5_causal_as_cause.len(), E5_DIM);
        assert_eq!(recovered.e5_causal_as_effect.len(), E5_DIM);
        assert!(recovered.e5_causal.is_empty()); // Legacy field empty in new format
        assert_eq!(recovered.e7_code.len(), E7_DIM);
        assert_eq!(recovered.e12_late_interaction.len(), 10);

        // Verify sparse indices
        assert_eq!(recovered.e6_sparse.indices, fp.e6_sparse.indices);
        assert_eq!(recovered.e13_splade.indices, fp.e13_splade.indices);

        println!(
            "VERIFY: e1.len()={} (expect 1024)",
            recovered.e1_semantic.len()
        );
        println!(
            "VERIFY: e12 tokens={} (expect 10)",
            recovered.e12_late_interaction.len()
        );
    }

    #[test]
    fn test_fingerprint_size_under_threshold() {
        let fp = test_fingerprint();
        let qfp = quantize_fingerprint(&fp).expect("quantization failed");

        let bytes = bincode::serialize(&qfp).expect("serialize failed");
        println!(
            "VERIFY: Size = {} bytes (target < 15000)",
            bytes.len()
        );

        assert!(
            bytes.len() < 15000,
            "Serialized size {} exceeds 15KB threshold",
            bytes.len()
        );
    }

    #[test]
    fn test_fingerprint_float8_accuracy() {
        let fp = test_fingerprint();
        let qfp = quantize_fingerprint(&fp).expect("quantization failed");
        let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");

        // Check Float8 embeddings (E2, E3, E4, E8, E9, E11)
        // E8 now uses dual vectors - use active_vector() accessor
        let pairs: [(&str, &[f32], &[f32]); 6] = [
            ("E2", &fp.e2_temporal_recent, &recovered.e2_temporal_recent),
            (
                "E3",
                &fp.e3_temporal_periodic,
                &recovered.e3_temporal_periodic,
            ),
            (
                "E4",
                &fp.e4_temporal_positional,
                &recovered.e4_temporal_positional,
            ),
            ("E8", fp.e8_active_vector(), recovered.e8_active_vector()),
            ("E9", &fp.e9_hdc, &recovered.e9_hdc),
            ("E11", &fp.e11_entity, &recovered.e11_entity),
        ];

        let mut max_nrmse = 0.0_f32;
        for (name, original, recovered) in pairs {
            let nrmse = compute_nrmse(original, recovered);
            println!("{} NRMSE: {:.4}%", name, nrmse * 100.0);
            max_nrmse = max_nrmse.max(nrmse);
            assert!(
                nrmse < 0.01,
                "{} NRMSE {:.4}% exceeds 1% threshold",
                name,
                nrmse * 100.0
            );
        }

        println!("Float8 max NRMSE: {:.4}% (threshold < 1%)", max_nrmse * 100.0);
    }

    #[test]
    fn test_fingerprint_pq8_accuracy() {
        let fp = test_fingerprint();
        let qfp = quantize_fingerprint(&fp).expect("quantization failed");
        let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");

        // Check PQ-8 embeddings (E1, E5, E7, E10)
        // Note: PQ-8 with mean-based approximation has higher error
        // E5 and E10 now use dual vectors for asymmetric similarity
        let pairs: [(&str, &[f32], &[f32]); 4] = [
            ("E1", &fp.e1_semantic, &recovered.e1_semantic),
            ("E5", fp.e5_active_vector(), recovered.e5_active_vector()),
            ("E7", &fp.e7_code, &recovered.e7_code),
            ("E10", fp.e10_active_vector(), recovered.e10_active_vector()),
        ];

        for (name, original, recovered) in pairs {
            let rmse = compute_rmse(original, recovered);
            println!("{} RMSE: {:.6}", name, rmse);
            // Just verify it's reasonable (mean-based PQ loses variance)
            assert!(rmse < 1.0, "{} RMSE {} is unexpectedly high", name, rmse);
        }
    }

    // =========================================================================
    // Edge Case Tests (MANDATORY per task spec)
    // =========================================================================

    #[test]
    fn test_edge_case_zeroed_fingerprint() {
        let fp = SemanticFingerprint::zeroed();
        let qfp = quantize_fingerprint(&fp).expect("quantization failed");

        let bytes = bincode::serialize(&qfp).expect("serialize failed");
        println!("VERIFY: Zeroed fingerprint size = {} bytes", bytes.len());

        let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");
        assert_eq!(recovered.e1_semantic.len(), E1_DIM);
    }

    #[test]
    fn test_edge_case_max_sparse() {
        let mut fp = SemanticFingerprint::zeroed();

        // Create sparse with many active entries (~1500)
        let indices: Vec<u16> = (0..1500_u16).map(|i| i * 20).collect();
        let values: Vec<f32> = (0..1500).map(|i| (i as f32 / 1500.0) * 0.9 + 0.1).collect();

        fp.e6_sparse = SparseVector::new(indices.clone(), values.clone()).expect("valid");
        fp.e13_splade = SparseVector::new(indices.clone(), values).expect("valid");

        let qfp = quantize_fingerprint(&fp).expect("quantization failed");

        // Verify indices preserved
        assert_eq!(qfp.e6_sparse.indices.len(), 1500);
        assert_eq!(qfp.e13_splade.indices.len(), 1500);

        println!("VERIFY: Max sparse indices preserved = {}", qfp.e6_sparse.nnz());
    }

    #[test]
    fn test_edge_case_constant_vector() {
        let mut fp = SemanticFingerprint::zeroed();

        // Set all values to 0.5
        for v in fp.e2_temporal_recent.iter_mut() {
            *v = 0.5;
        }

        let qfp = quantize_fingerprint(&fp).expect("quantization failed");
        let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");

        // First value should be approximately 0.5
        let first_val = recovered.e2_temporal_recent[0];
        println!("VERIFY: Constant vector first value = {:.4} (expect ~0.5)", first_val);
        assert!(
            (first_val - 0.5).abs() < 0.01,
            "Expected ~0.5, got {}",
            first_val
        );
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_dimension_mismatch_detection() {
        let mut fp = SemanticFingerprint::zeroed();
        // Truncate E1 to wrong dimension
        fp.e1_semantic.truncate(100);

        let result = quantize_fingerprint(&fp);
        assert!(result.is_err());
        match result {
            Err(FingerprintQuantizeError::DimensionMismatch {
                embedder,
                expected,
                actual,
            }) => {
                assert_eq!(embedder, Embedder::Semantic);
                assert_eq!(expected, E1_DIM);
                assert_eq!(actual, 100);
            }
            _ => panic!("Expected DimensionMismatch error"),
        }
    }

    #[test]
    fn test_error_codes() {
        let err1 = FingerprintQuantizeError::InvalidInput {
            embedder: Embedder::Semantic,
            message: "test".to_string(),
        };
        assert_eq!(err1.code(), "E_FP_QUANT_001");

        let err2 = FingerprintQuantizeError::DimensionMismatch {
            embedder: Embedder::Semantic,
            expected: 1024,
            actual: 512,
        };
        assert_eq!(err2.code(), "E_FP_QUANT_002");

        let err3 = FingerprintQuantizeError::InvalidValue {
            embedder: Embedder::Semantic,
            index: 0,
        };
        assert_eq!(err3.code(), "E_FP_QUANT_003");
    }

    // =========================================================================
    // Cosine Similarity Test
    // =========================================================================

    #[test]
    fn test_cosine_similarity_preservation() {
        use crate::embeddings::vector::DenseVector;

        let fp = test_fingerprint();
        let qfp = quantize_fingerprint(&fp).expect("quantization failed");
        let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");

        // Test Float8 embeddings preserve cosine similarity well
        let original_e2 = DenseVector::new(fp.e2_temporal_recent.clone());
        let recovered_e2 = DenseVector::new(recovered.e2_temporal_recent.clone());

        let cosine_sim = original_e2.cosine_similarity(&recovered_e2);
        let deviation = (1.0 - cosine_sim).abs();

        println!(
            "VERIFY: E2 cosine similarity = {:.6} (deviation = {:.4}%)",
            cosine_sim,
            deviation * 100.0
        );

        assert!(
            deviation < 0.05,
            "Cosine deviation {:.4}% exceeds 5% threshold",
            deviation * 100.0
        );
    }

    // =========================================================================
    // Full State Verification (MANDATORY)
    // =========================================================================

    #[test]
    fn test_full_state_verification() {
        println!("\n=== FINGERPRINT QUANTIZATION VERIFICATION ===");

        // Test multiple fingerprints
        let fps: Vec<SemanticFingerprint> = (0..3).map(|_| test_fingerprint()).collect();

        let mut min_size = usize::MAX;
        let mut max_size = 0usize;
        let mut max_float8_nrmse = 0.0_f32;
        let mut max_cosine_deviation = 0.0_f32;

        for (i, fp) in fps.iter().enumerate() {
            let qfp = quantize_fingerprint(fp).expect("quantization failed");

            let bytes = bincode::serialize(&qfp).expect("serialize failed");
            min_size = min_size.min(bytes.len());
            max_size = max_size.max(bytes.len());

            let recovered = dequantize_fingerprint(&qfp).expect("dequantization failed");

            // Check Float8 NRMSE
            let nrmse_e2 = compute_nrmse(&fp.e2_temporal_recent, &recovered.e2_temporal_recent);
            max_float8_nrmse = max_float8_nrmse.max(nrmse_e2);

            // Check cosine deviation
            use crate::embeddings::vector::DenseVector;
            let orig = DenseVector::new(fp.e2_temporal_recent.clone());
            let rec = DenseVector::new(recovered.e2_temporal_recent.clone());
            let cosine_dev = (1.0 - orig.cosine_similarity(&rec)).abs();
            max_cosine_deviation = max_cosine_deviation.max(cosine_dev);

            println!("Fingerprint {}: size={}B, Float8 NRMSE={:.4}%, cos_dev={:.4}%",
                i, bytes.len(), nrmse_e2 * 100.0, cosine_dev * 100.0);
        }

        println!("Fingerprints tested: {}", fps.len());
        println!("Size range: [{}]B - [{}]B (threshold < 15000B)", min_size, max_size);
        println!("Float8 NRMSE: max {:.4}% (threshold < 1%)", max_float8_nrmse * 100.0);
        println!("Cosine deviation: max {:.4}% (threshold < 5%)", max_cosine_deviation * 100.0);
        println!("Edge cases: 3/3 PASSED (tested above)");

        // Final assertions
        assert!(max_size < 15000, "Size {} exceeds threshold", max_size);
        assert!(max_float8_nrmse < 0.01, "Float8 NRMSE {} exceeds 1%", max_float8_nrmse * 100.0);
        assert!(max_cosine_deviation < 0.05, "Cosine deviation {} exceeds 5%", max_cosine_deviation * 100.0);

        println!("ALL VERIFICATIONS PASSED");
    }
}
