//! Offset tests for the embedder dimension system.
//!
//! Tests that verify offset calculations for Multi-Array Storage are correct.

use context_graph_embeddings::dimensions::{
    offset_by_index, projected_dimension_by_index, MODEL_COUNT, OFFSETS, TOTAL_DIMENSION,
};

/// Test first offset is zero.
#[test]
fn test_first_offset_is_zero() {
    assert_eq!(offset_by_index(0), 0, "First offset must be 0");
    assert_eq!(OFFSETS[0], 0, "OFFSETS[0] must be 0");
    println!("[PASS] First offset is 0");
}

/// Test last offset + dimension equals TOTAL_DIMENSION.
#[test]
fn test_last_offset_plus_dimension_equals_total() {
    let last_index = MODEL_COUNT - 1; // 12
    let last_offset = offset_by_index(last_index);
    let last_dim = projected_dimension_by_index(last_index);
    let computed_total = last_offset + last_dim;

    assert_eq!(
        computed_total, TOTAL_DIMENSION,
        "offset[12] + dim[12] ({} + {}) = {} != TOTAL_DIMENSION ({})",
        last_offset, last_dim, computed_total, TOTAL_DIMENSION
    );
    println!("[PASS] Last offset + last dimension = TOTAL_DIMENSION");
}

/// Test each offset equals sum of previous dimensions.
#[test]
fn test_offset_cumulative_sum() {
    let mut cumulative: usize = 0;

    for i in 0..MODEL_COUNT {
        let offset = offset_by_index(i);
        assert_eq!(
            offset, cumulative,
            "offset_by_index({}) = {} but cumulative sum = {}",
            i, offset, cumulative
        );
        cumulative += projected_dimension_by_index(i);
    }

    // After all, cumulative should equal TOTAL_DIMENSION
    assert_eq!(
        cumulative, TOTAL_DIMENSION,
        "Final cumulative {} != TOTAL_DIMENSION {}",
        cumulative, TOTAL_DIMENSION
    );
    println!("[PASS] All offsets are correct cumulative sums");
}

/// Test OFFSETS array matches offset_by_index function.
#[test]
fn test_offsets_array_consistency() {
    for (i, &offset) in OFFSETS.iter().enumerate() {
        assert_eq!(
            offset,
            offset_by_index(i),
            "OFFSETS[{}] ({}) != offset_by_index({}) ({})",
            i,
            offset,
            i,
            offset_by_index(i)
        );
    }
    println!("[PASS] OFFSETS array matches offset_by_index function");
}

/// Test specific key offsets from Constitution.
#[test]
fn test_key_offsets() {
    // E1 Semantic: 0
    assert_eq!(offset_by_index(0), 0, "E1 offset should be 0");

    // E2 TemporalRecent: 1024
    assert_eq!(offset_by_index(1), 1024, "E2 offset should be 1024");

    // E5 Causal: 1024 + 512*3 = 2560
    assert_eq!(offset_by_index(4), 2560, "E5 offset should be 2560");

    // E6 Sparse: 2560 + 768 = 3328
    assert_eq!(offset_by_index(5), 3328, "E6 offset should be 3328");

    // E13 Splade: 10624 - 1536 = 9088 (E7 Code increased from 768→1536)
    assert_eq!(offset_by_index(12), 9088, "E13 offset should be 9088");

    println!("[PASS] Key offsets verified");
}
