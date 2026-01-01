//! Compile-time dimension constants for the 12-model embedding pipeline.
//!
//! These constants define the exact dimensions used throughout the fusion process:
//! - Native dimensions: Raw model output sizes
//! - Projected dimensions: Normalized sizes for FuseMoE input
//! - TOTAL_CONCATENATED: Sum of all projected dimensions
//! - FUSED_OUTPUT: Final FuseMoE output (1536D)
//!
//! # Usage
//!
//! ```rust
//! use context_graph_embeddings::types::dimensions;
//!
//! // Static buffer sizing
//! let concat_buffer = vec![0.0f32; dimensions::TOTAL_CONCATENATED];
//! assert_eq!(concat_buffer.len(), 8320);
//!
//! // Compile-time validation
//! const _: () = assert!(dimensions::TOTAL_CONCATENATED == 8320);
//! ```

// =============================================================================
// NATIVE OUTPUT DIMENSIONS (before any projection)
// =============================================================================

/// E1: Semantic embedding native dimension (e5-large-v2)
pub const SEMANTIC_NATIVE: usize = 1024;

/// E2: Temporal-Recent native dimension (custom exponential decay)
pub const TEMPORAL_RECENT_NATIVE: usize = 512;

/// E3: Temporal-Periodic native dimension (custom Fourier basis)
pub const TEMPORAL_PERIODIC_NATIVE: usize = 512;

/// E4: Temporal-Positional native dimension (custom sinusoidal PE)
pub const TEMPORAL_POSITIONAL_NATIVE: usize = 512;

/// E5: Causal embedding native dimension (Longformer)
pub const CAUSAL_NATIVE: usize = 768;

/// E6: Sparse lexical native dimension (SPLADE vocab size, ~5% active)
pub const SPARSE_NATIVE: usize = 30522;

/// E7: Code embedding native dimension (CodeT5p embed_dim)
pub const CODE_NATIVE: usize = 256;

/// E8: Graph embedding native dimension (paraphrase-MiniLM-L6-v2)
pub const GRAPH_NATIVE: usize = 384;

/// E9: Hyperdimensional computing native dimension (10K-bit vector)
pub const HDC_NATIVE: usize = 10000;

/// E10: Multimodal embedding native dimension (CLIP)
pub const MULTIMODAL_NATIVE: usize = 768;

/// E11: Entity embedding native dimension (all-MiniLM-L6-v2)
pub const ENTITY_NATIVE: usize = 384;

/// E12: Late-interaction native dimension per token (ColBERT)
pub const LATE_INTERACTION_NATIVE: usize = 128;

// =============================================================================
// PROJECTED DIMENSIONS (for FuseMoE concatenation input)
// =============================================================================

/// E1: Semantic projected dimension (no projection needed)
pub const SEMANTIC: usize = 1024;

/// E2: Temporal-Recent projected dimension (no projection needed)
pub const TEMPORAL_RECENT: usize = 512;

/// E3: Temporal-Periodic projected dimension (no projection needed)
pub const TEMPORAL_PERIODIC: usize = 512;

/// E4: Temporal-Positional projected dimension (no projection needed)
pub const TEMPORAL_POSITIONAL: usize = 512;

/// E5: Causal projected dimension (no projection needed)
pub const CAUSAL: usize = 768;

/// E6: Sparse projected dimension (30K sparse → 1536D via learned projection)
pub const SPARSE: usize = 1536;

/// E7: Code projected dimension (256 embed → 768D via projection to match CodeT5p d_model)
pub const CODE: usize = 768;

/// E8: Graph projected dimension (no projection needed)
pub const GRAPH: usize = 384;

/// E9: HDC projected dimension (10K-bit → 1024D via learned projection)
pub const HDC: usize = 1024;

/// E10: Multimodal projected dimension (no projection needed)
pub const MULTIMODAL: usize = 768;

/// E11: Entity projected dimension (no projection needed)
pub const ENTITY: usize = 384;

/// E12: Late-interaction projected dimension (pooled to single vector)
pub const LATE_INTERACTION: usize = 128;

// =============================================================================
// AGGREGATE DIMENSIONS
// =============================================================================

/// Total concatenated dimension: sum of all 12 projected dimensions.
/// This is the input size to FuseMoE gating network.
///
/// Calculated as:
/// 1024 + 512 + 512 + 512 + 768 + 1536 + 768 + 384 + 1024 + 768 + 384 + 128 = 8320
pub const TOTAL_CONCATENATED: usize = SEMANTIC
    + TEMPORAL_RECENT
    + TEMPORAL_PERIODIC
    + TEMPORAL_POSITIONAL
    + CAUSAL
    + SPARSE
    + CODE
    + GRAPH
    + HDC
    + MULTIMODAL
    + ENTITY
    + LATE_INTERACTION;

/// FuseMoE output dimension (final unified embedding).
/// Matches OpenAI ada-002 dimension for downstream compatibility.
pub const FUSED_OUTPUT: usize = 1536;

/// Number of models in the ensemble.
pub const MODEL_COUNT: usize = 12;

// =============================================================================
// COMPILE-TIME VALIDATION
// =============================================================================

/// Compile-time assertion that TOTAL_CONCATENATED equals expected value.
/// This will cause a compilation error if dimensions change incorrectly.
const _TOTAL_CONCATENATED_CHECK: () = assert!(
    TOTAL_CONCATENATED == 8320,
    "TOTAL_CONCATENATED must equal 8320"
);

/// Compile-time assertion that FUSED_OUTPUT equals expected value.
const _FUSED_OUTPUT_CHECK: () = assert!(
    FUSED_OUTPUT == 1536,
    "FUSED_OUTPUT must equal 1536"
);

/// Compile-time assertion that MODEL_COUNT equals 12.
const _MODEL_COUNT_CHECK: () = assert!(
    MODEL_COUNT == 12,
    "MODEL_COUNT must equal 12"
);

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Returns the projected dimension for a given model index (0-11).
///
/// # Panics
/// Panics if index >= 12.
///
/// # Example
/// ```rust
/// use context_graph_embeddings::types::dimensions;
///
/// assert_eq!(dimensions::projected_dimension_by_index(0), 1024); // Semantic
/// assert_eq!(dimensions::projected_dimension_by_index(5), 1536); // Sparse
/// ```
#[must_use]
pub const fn projected_dimension_by_index(index: usize) -> usize {
    match index {
        0 => SEMANTIC,
        1 => TEMPORAL_RECENT,
        2 => TEMPORAL_PERIODIC,
        3 => TEMPORAL_POSITIONAL,
        4 => CAUSAL,
        5 => SPARSE,
        6 => CODE,
        7 => GRAPH,
        8 => HDC,
        9 => MULTIMODAL,
        10 => ENTITY,
        11 => LATE_INTERACTION,
        _ => panic!("Invalid model index: must be 0-11"),
    }
}

/// Returns the native dimension for a given model index (0-11).
///
/// # Panics
/// Panics if index >= 12.
#[must_use]
pub const fn native_dimension_by_index(index: usize) -> usize {
    match index {
        0 => SEMANTIC_NATIVE,
        1 => TEMPORAL_RECENT_NATIVE,
        2 => TEMPORAL_PERIODIC_NATIVE,
        3 => TEMPORAL_POSITIONAL_NATIVE,
        4 => CAUSAL_NATIVE,
        5 => SPARSE_NATIVE,
        6 => CODE_NATIVE,
        7 => GRAPH_NATIVE,
        8 => HDC_NATIVE,
        9 => MULTIMODAL_NATIVE,
        10 => ENTITY_NATIVE,
        11 => LATE_INTERACTION_NATIVE,
        _ => panic!("Invalid model index: must be 0-11"),
    }
}

/// Returns the byte offset into a concatenated embedding vector for model at index.
///
/// Used for zero-copy slicing into the 8320D concatenated vector.
///
/// # Example
/// ```rust
/// use context_graph_embeddings::types::dimensions;
///
/// // Semantic (E1) starts at offset 0
/// assert_eq!(dimensions::offset_by_index(0), 0);
///
/// // TemporalRecent (E2) starts after Semantic's 1024 elements
/// assert_eq!(dimensions::offset_by_index(1), 1024);
///
/// // Causal (E5) starts after Semantic + 3x Temporal
/// assert_eq!(dimensions::offset_by_index(4), 1024 + 512 + 512 + 512);
/// ```
#[must_use]
pub const fn offset_by_index(index: usize) -> usize {
    match index {
        0 => 0,
        1 => SEMANTIC,
        2 => SEMANTIC + TEMPORAL_RECENT,
        3 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC,
        4 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL,
        5 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL,
        6 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE,
        7 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE + CODE,
        8 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE + CODE + GRAPH,
        9 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE + CODE + GRAPH + HDC,
        10 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE + CODE + GRAPH + HDC + MULTIMODAL,
        11 => SEMANTIC + TEMPORAL_RECENT + TEMPORAL_PERIODIC + TEMPORAL_POSITIONAL + CAUSAL + SPARSE + CODE + GRAPH + HDC + MULTIMODAL + ENTITY,
        _ => panic!("Invalid model index: must be 0-11"),
    }
}

/// All projected dimensions in order (E1-E12).
pub const PROJECTED_DIMENSIONS: [usize; MODEL_COUNT] = [
    SEMANTIC,
    TEMPORAL_RECENT,
    TEMPORAL_PERIODIC,
    TEMPORAL_POSITIONAL,
    CAUSAL,
    SPARSE,
    CODE,
    GRAPH,
    HDC,
    MULTIMODAL,
    ENTITY,
    LATE_INTERACTION,
];

/// All native dimensions in order (E1-E12).
pub const NATIVE_DIMENSIONS: [usize; MODEL_COUNT] = [
    SEMANTIC_NATIVE,
    TEMPORAL_RECENT_NATIVE,
    TEMPORAL_PERIODIC_NATIVE,
    TEMPORAL_POSITIONAL_NATIVE,
    CAUSAL_NATIVE,
    SPARSE_NATIVE,
    CODE_NATIVE,
    GRAPH_NATIVE,
    HDC_NATIVE,
    MULTIMODAL_NATIVE,
    ENTITY_NATIVE,
    LATE_INTERACTION_NATIVE,
];

/// All offsets into concatenated vector in order (E1-E12).
pub const OFFSETS: [usize; MODEL_COUNT] = [
    offset_by_index(0),
    offset_by_index(1),
    offset_by_index(2),
    offset_by_index(3),
    offset_by_index(4),
    offset_by_index(5),
    offset_by_index(6),
    offset_by_index(7),
    offset_by_index(8),
    offset_by_index(9),
    offset_by_index(10),
    offset_by_index(11),
];

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_concatenated_sum() {
        // Manually verify sum
        let sum = SEMANTIC
            + TEMPORAL_RECENT
            + TEMPORAL_PERIODIC
            + TEMPORAL_POSITIONAL
            + CAUSAL
            + SPARSE
            + CODE
            + GRAPH
            + HDC
            + MULTIMODAL
            + ENTITY
            + LATE_INTERACTION;
        assert_eq!(sum, TOTAL_CONCATENATED);
        assert_eq!(TOTAL_CONCATENATED, 8320);
    }

    #[test]
    fn test_fused_output_dimension() {
        assert_eq!(FUSED_OUTPUT, 1536);
    }

    #[test]
    fn test_model_count() {
        assert_eq!(MODEL_COUNT, 12);
        assert_eq!(PROJECTED_DIMENSIONS.len(), 12);
        assert_eq!(NATIVE_DIMENSIONS.len(), 12);
        assert_eq!(OFFSETS.len(), 12);
    }

    #[test]
    fn test_projected_dimension_by_index() {
        assert_eq!(projected_dimension_by_index(0), 1024);  // Semantic
        assert_eq!(projected_dimension_by_index(5), 1536);  // Sparse (projected)
        assert_eq!(projected_dimension_by_index(6), 768);   // Code (projected)
        assert_eq!(projected_dimension_by_index(8), 1024);  // HDC (projected)
        assert_eq!(projected_dimension_by_index(11), 128);  // LateInteraction
    }

    #[test]
    fn test_native_dimension_by_index() {
        assert_eq!(native_dimension_by_index(5), 30522);    // Sparse native
        assert_eq!(native_dimension_by_index(6), 256);      // Code native
        assert_eq!(native_dimension_by_index(8), 10000);    // HDC native
    }

    #[test]
    fn test_offset_calculations() {
        // E1 starts at 0
        assert_eq!(offset_by_index(0), 0);
        // E2 starts after E1 (1024)
        assert_eq!(offset_by_index(1), 1024);
        // E3 starts after E1+E2 (1024+512)
        assert_eq!(offset_by_index(2), 1536);
        // E5 starts after all temporals
        assert_eq!(offset_by_index(4), 1024 + 512 + 512 + 512);
        // Last offset + last dimension should equal TOTAL
        assert_eq!(offset_by_index(11) + LATE_INTERACTION, TOTAL_CONCATENATED);
    }

    #[test]
    fn test_projected_dimensions_array() {
        assert_eq!(PROJECTED_DIMENSIONS[0], SEMANTIC);
        assert_eq!(PROJECTED_DIMENSIONS[5], SPARSE);
        assert_eq!(PROJECTED_DIMENSIONS[11], LATE_INTERACTION);

        // Sum of array equals TOTAL_CONCATENATED
        let sum: usize = PROJECTED_DIMENSIONS.iter().sum();
        assert_eq!(sum, TOTAL_CONCATENATED);
    }

    #[test]
    fn test_offsets_array_consistency() {
        // Verify OFFSETS array matches offset_by_index function
        for i in 0..MODEL_COUNT {
            assert_eq!(OFFSETS[i], offset_by_index(i), "Mismatch at index {}", i);
        }
    }

    #[test]
    fn test_sparse_projection_ratio() {
        // SPLADE projects from 30K sparse to 1536 dense
        assert!(SPARSE_NATIVE > SPARSE);
        let ratio = SPARSE_NATIVE as f64 / SPARSE as f64;
        assert!(ratio > 19.0 && ratio < 20.0); // ~19.8x compression
    }

    #[test]
    fn test_hdc_projection_ratio() {
        // HDC projects from 10K-bit to 1024
        assert!(HDC_NATIVE > HDC);
        let ratio = HDC_NATIVE as f64 / HDC as f64;
        assert!(ratio > 9.0 && ratio < 10.0); // ~9.77x compression
    }

    #[test]
    fn test_code_projection_ratio() {
        // CodeT5p projects from 256 embed to 768 (expansion)
        assert!(CODE > CODE_NATIVE);
        assert_eq!(CODE, 768);
        assert_eq!(CODE_NATIVE, 256);
    }

    // Edge Case Tests with Before/After State Printing

    #[test]
    fn test_edge_case_invalid_index_projected() {
        // Test that invalid index panics
        let result = std::panic::catch_unwind(|| {
            projected_dimension_by_index(12)
        });
        assert!(result.is_err(), "Index 12 should panic");
        println!("Edge Case 1 PASSED: projected_dimension_by_index(12) panics correctly");
    }

    #[test]
    fn test_edge_case_invalid_index_native() {
        let result = std::panic::catch_unwind(|| {
            native_dimension_by_index(255)
        });
        assert!(result.is_err(), "Index 255 should panic");
        println!("Edge Case 2 PASSED: native_dimension_by_index(255) panics correctly");
    }

    #[test]
    fn test_edge_case_offset_boundary() {
        // Last valid offset + its dimension should equal TOTAL
        let last_offset = offset_by_index(11);
        let last_dim = projected_dimension_by_index(11);
        println!("Before: last_offset={}, last_dim={}", last_offset, last_dim);

        let computed_total = last_offset + last_dim;
        println!("After: computed_total={}, TOTAL_CONCATENATED={}", computed_total, TOTAL_CONCATENATED);

        assert_eq!(computed_total, TOTAL_CONCATENATED);
        println!("Edge Case 3 PASSED: offset boundary calculation correct");
    }
}
