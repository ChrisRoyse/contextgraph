//! Compile-time dimension constants for the 14-model embedding pipeline.
//!
//! These constants define the exact dimensions used throughout the embedding pipeline:
//! - Native dimensions: Raw model output sizes
//! - Projected dimensions: Target sizes for Multi-Array Storage
//! - TOTAL_DIMENSION: Sum of all projected dimensions (12032D)
//!
//! # Multi-Array Storage
//!
//! All 14 embeddings are stored SEPARATELY at their native dimensions.
//! The 14-embedding array IS the teleological vector (Royse 2026).
//!
//! # Usage
//!
//! ```rust
//! use context_graph_embeddings::types::dimensions;
//!
//! // Total dimension for memory calculations
//! assert_eq!(dimensions::TOTAL_DIMENSION, 12032);
//!
//! // Compile-time validation
//! const _: () = assert!(dimensions::TOTAL_DIMENSION == 12032);
//! ```

mod aggregates;
mod arrays;
mod constants;
mod helpers;

// =============================================================================
// RE-EXPORTS
// =============================================================================

// Native dimensions
pub use constants::{
    CAUSAL_NATIVE, CODE_NATIVE, ENTITY_NATIVE, GRAPH_NATIVE, HDC_NATIVE, KEPLER_NATIVE,
    LATE_INTERACTION_NATIVE, MULTIMODAL_NATIVE, SEMANTIC_NATIVE, SPARSE_NATIVE, SPLADE_NATIVE,
    TEMPORAL_PERIODIC_NATIVE, TEMPORAL_POSITIONAL_NATIVE, TEMPORAL_RECENT_NATIVE,
};

// Projected dimensions
pub use constants::{
    CAUSAL, CODE, ENTITY, GRAPH, HDC, KEPLER, LATE_INTERACTION, MULTIMODAL, SEMANTIC, SPARSE,
    SPLADE, TEMPORAL_PERIODIC, TEMPORAL_POSITIONAL, TEMPORAL_RECENT,
};

// Aggregate dimensions
pub use aggregates::{MODEL_COUNT, TOTAL_DIMENSION};

// Helper functions
pub use helpers::{native_dimension_by_index, offset_by_index, projected_dimension_by_index};

// Static arrays
pub use arrays::{NATIVE_DIMENSIONS, OFFSETS, PROJECTED_DIMENSIONS};

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_dimension_sum() {
        // Manually verify sum (includes SPLADE + KEPLER)
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
            + LATE_INTERACTION
            + super::constants::SPLADE
            + super::constants::KEPLER;
        assert_eq!(sum, TOTAL_DIMENSION);
        assert_eq!(TOTAL_DIMENSION, 12032);
    }

    #[test]
    fn test_model_count() {
        assert_eq!(MODEL_COUNT, 14);
        assert_eq!(PROJECTED_DIMENSIONS.len(), 14);
        assert_eq!(NATIVE_DIMENSIONS.len(), 14);
        assert_eq!(OFFSETS.len(), 14);
    }

    #[test]
    fn test_projected_dimension_by_index() {
        assert_eq!(projected_dimension_by_index(0), 1024); // Semantic
        assert_eq!(projected_dimension_by_index(5), 1536); // Sparse (projected)
        assert_eq!(projected_dimension_by_index(6), 1536); // Code (Qodo-Embed 1536D)
        assert_eq!(projected_dimension_by_index(8), 1024); // HDC (projected)
        assert_eq!(projected_dimension_by_index(11), 128); // LateInteraction
        assert_eq!(projected_dimension_by_index(12), 1536); // Splade (projected)
    }

    #[test]
    fn test_native_dimension_by_index() {
        assert_eq!(native_dimension_by_index(5), 30522); // Sparse native
        assert_eq!(native_dimension_by_index(6), 1536); // Code native (Qodo-Embed 1536D)
        assert_eq!(native_dimension_by_index(8), 10000); // HDC native
        assert_eq!(native_dimension_by_index(12), 30522); // Splade native
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
        // Kepler (index 13) offset + dimension should equal TOTAL
        assert_eq!(
            offset_by_index(13) + super::constants::KEPLER,
            TOTAL_DIMENSION
        );
    }

    #[test]
    fn test_projected_dimensions_array() {
        assert_eq!(PROJECTED_DIMENSIONS[0], SEMANTIC);
        assert_eq!(PROJECTED_DIMENSIONS[5], SPARSE);
        assert_eq!(PROJECTED_DIMENSIONS[11], LATE_INTERACTION);
        assert_eq!(PROJECTED_DIMENSIONS[12], super::constants::SPLADE);
        assert_eq!(PROJECTED_DIMENSIONS[13], super::constants::KEPLER);

        // Sum of array equals TOTAL_DIMENSION
        let sum: usize = PROJECTED_DIMENSIONS.iter().sum();
        assert_eq!(sum, TOTAL_DIMENSION);
    }

    #[test]
    fn test_offsets_array_consistency() {
        // Verify OFFSETS array matches offset_by_index function
        for (i, &offset) in OFFSETS.iter().enumerate().take(MODEL_COUNT) {
            assert_eq!(offset, offset_by_index(i), "Mismatch at index {}", i);
        }
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_sparse_projection_ratio() {
        // SPLADE projects from 30K sparse to 1536 dense
        assert!(SPARSE_NATIVE > SPARSE);
        let ratio = SPARSE_NATIVE as f64 / SPARSE as f64;
        assert!(ratio > 19.0 && ratio < 20.0); // ~19.8x compression
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_hdc_projection_ratio() {
        // HDC projects from 10K-bit to 1024
        assert!(HDC_NATIVE > HDC);
        let ratio = HDC_NATIVE as f64 / HDC as f64;
        assert!(ratio > 9.0 && ratio < 10.0); // ~9.77x compression
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_code_dimension() {
        // Qodo-Embed outputs 1536D natively (no projection needed)
        assert_eq!(CODE, CODE_NATIVE);
        assert_eq!(CODE, 1536);
        assert_eq!(CODE_NATIVE, 1536);
    }

    // Edge Case Tests with Before/After State Printing

    #[test]
    fn test_edge_case_invalid_index_projected() {
        // Test that invalid index panics
        let result = std::panic::catch_unwind(|| projected_dimension_by_index(14));
        assert!(result.is_err(), "Index 14 should panic");
        println!("Edge Case 1 PASSED: projected_dimension_by_index(14) panics correctly");
    }

    #[test]
    fn test_edge_case_invalid_index_native() {
        let result = std::panic::catch_unwind(|| native_dimension_by_index(255));
        assert!(result.is_err(), "Index 255 should panic");
        println!("Edge Case 2 PASSED: native_dimension_by_index(255) panics correctly");
    }

    #[test]
    fn test_edge_case_offset_boundary() {
        // Last valid offset (Kepler at index 13) + its dimension should equal TOTAL
        let last_offset = offset_by_index(13);
        let last_dim = projected_dimension_by_index(13);
        println!("Before: last_offset={}, last_dim={}", last_offset, last_dim);

        let computed_total = last_offset + last_dim;
        println!(
            "After: computed_total={}, TOTAL_DIMENSION={}",
            computed_total, TOTAL_DIMENSION
        );

        assert_eq!(computed_total, TOTAL_DIMENSION);
        println!("Edge Case 3 PASSED: offset boundary calculation correct");
    }
}
