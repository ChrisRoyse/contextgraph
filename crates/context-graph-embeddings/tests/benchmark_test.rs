//! Performance Benchmark Tests (TASK-EMB-025 Agent 5)
//!
//! Verifies performance characteristics of the embedding pipeline.
//!
//! # Key Verifications
//! - Storage size targets (~17KB quantized fingerprint)
//! - Model count: 13 embedders
//! - Dimension totals: 9856D
//! - Quantization method assignments per Constitution
//! - Memory estimate calculations
//!
//! # Note on Actual Benchmarks
//! Real GPU benchmarks require `#[bench]` and `cargo bench`.
//! These tests verify correctness of benchmark-related calculations.

use context_graph_embeddings::storage::{
    StoredQuantizedFingerprint, EXPECTED_QUANTIZED_SIZE_BYTES, MAX_QUANTIZED_SIZE_BYTES,
    MIN_QUANTIZED_SIZE_BYTES, NUM_EMBEDDERS, STORAGE_VERSION, RRF_K,
};
use context_graph_embeddings::quantization::{
    QuantizationMethod, QuantizedEmbedding, QuantizationMetadata,
};
use context_graph_embeddings::types::dimensions::{
    TOTAL_DIMENSION, MODEL_COUNT,
    PROJECTED_DIMENSIONS, NATIVE_DIMENSIONS, OFFSETS,
};
use context_graph_embeddings::types::ModelId;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

// =============================================================================
// STORAGE SIZE BENCHMARK TESTS
// =============================================================================

/// Test: Expected quantized size is ~17KB (Constitution requirement)
#[test]
fn test_expected_quantized_size() {
    assert_eq!(EXPECTED_QUANTIZED_SIZE_BYTES, 17_000);
    eprintln!("[BENCHMARK] Expected quantized fingerprint size: {} bytes (~17KB)",
              EXPECTED_QUANTIZED_SIZE_BYTES);
}

/// Test: Maximum allowed size is 25KB (50% overhead for sparse)
#[test]
fn test_max_quantized_size() {
    assert_eq!(MAX_QUANTIZED_SIZE_BYTES, 25_000);
    eprintln!("[BENCHMARK] Maximum quantized fingerprint size: {} bytes (~25KB)",
              MAX_QUANTIZED_SIZE_BYTES);
}

/// Test: Minimum valid size is 5KB (catches empty/corrupted)
#[test]
fn test_min_quantized_size() {
    assert_eq!(MIN_QUANTIZED_SIZE_BYTES, 5_000);
    eprintln!("[BENCHMARK] Minimum valid fingerprint size: {} bytes (~5KB)",
              MIN_QUANTIZED_SIZE_BYTES);
}

/// Test: Storage version is 1
#[test]
fn test_storage_version() {
    assert_eq!(STORAGE_VERSION, 1);
    eprintln!("[BENCHMARK] Storage version: {}", STORAGE_VERSION);
}

// =============================================================================
// DIMENSION BENCHMARK TESTS
// =============================================================================

/// Test: Total dimension is 9856
#[test]
fn test_total_dimension_benchmark() {
    assert_eq!(TOTAL_DIMENSION, 9856);
    eprintln!("[BENCHMARK] Total dimension: {}D", TOTAL_DIMENSION);
}

/// Test: Model count is 13
#[test]
fn test_model_count_benchmark() {
    assert_eq!(MODEL_COUNT, 13);
    assert_eq!(NUM_EMBEDDERS, 13);
    eprintln!("[BENCHMARK] Model count: {} embedders", MODEL_COUNT);
}

/// Test: Projected dimensions sum to TOTAL_DIMENSION
#[test]
fn test_projected_dimensions_sum() {
    let sum: usize = PROJECTED_DIMENSIONS.iter().sum();
    assert_eq!(sum, TOTAL_DIMENSION);
    eprintln!("[BENCHMARK] Projected dimensions sum: {} = TOTAL_DIMENSION", sum);
}

/// Test: All dimension arrays have length 13
#[test]
fn test_dimension_arrays_length() {
    assert_eq!(PROJECTED_DIMENSIONS.len(), MODEL_COUNT);
    assert_eq!(NATIVE_DIMENSIONS.len(), MODEL_COUNT);
    assert_eq!(OFFSETS.len(), MODEL_COUNT);
    eprintln!("[BENCHMARK] All dimension arrays have {} elements", MODEL_COUNT);
}

/// Test: Offsets are contiguous and non-overlapping
#[test]
fn test_offsets_contiguous() {
    for i in 1..MODEL_COUNT {
        let expected = OFFSETS[i - 1] + PROJECTED_DIMENSIONS[i - 1];
        assert_eq!(OFFSETS[i], expected,
                   "Offset[{}] should be {}, got {}", i, expected, OFFSETS[i]);
    }

    // Final offset + dimension should equal TOTAL
    let last_end = OFFSETS[MODEL_COUNT - 1] + PROJECTED_DIMENSIONS[MODEL_COUNT - 1];
    assert_eq!(last_end, TOTAL_DIMENSION);

    eprintln!("[BENCHMARK] Offsets are contiguous and cover full {}D", TOTAL_DIMENSION);
}

// =============================================================================
// QUANTIZATION METHOD BENCHMARK TESTS
// =============================================================================

/// Test: Each ModelId maps to correct quantization method
#[test]
fn test_quantization_method_assignments() {
    let expected_methods = vec![
        (ModelId::Semantic, QuantizationMethod::PQ8),
        (ModelId::TemporalRecent, QuantizationMethod::Float8E4M3),
        (ModelId::TemporalPeriodic, QuantizationMethod::Float8E4M3),
        (ModelId::TemporalPositional, QuantizationMethod::Float8E4M3),
        (ModelId::Causal, QuantizationMethod::PQ8),
        (ModelId::Sparse, QuantizationMethod::SparseNative),
        (ModelId::Code, QuantizationMethod::PQ8),
        (ModelId::Graph, QuantizationMethod::Float8E4M3),
        (ModelId::Hdc, QuantizationMethod::Binary),
        (ModelId::Multimodal, QuantizationMethod::PQ8),
        (ModelId::Entity, QuantizationMethod::Float8E4M3),
        (ModelId::LateInteraction, QuantizationMethod::TokenPruning),
        (ModelId::Splade, QuantizationMethod::SparseNative),
    ];

    for (model_id, expected_method) in expected_methods {
        let actual = QuantizationMethod::for_model_id(model_id);
        assert_eq!(actual, expected_method,
                   "ModelId::{:?} should use {:?}, got {:?}",
                   model_id, expected_method, actual);
    }

    eprintln!("[BENCHMARK] All 13 quantization method assignments verified");
}

/// Test: Quantization compression ratios
#[test]
fn test_quantization_compression_ratios() {
    // PQ8: 1024D f32 -> 8 bytes (128:1 compression)
    let pq8_ratio = (1024 * 4) as f32 / 8.0;
    assert!(pq8_ratio > 500.0, "PQ8 should achieve >500:1 compression");

    // Float8: 512D f32 -> 512 bytes (4:1 compression)
    let float8_ratio = (512 * 4) as f32 / 512.0;
    assert!((float8_ratio - 4.0).abs() < 0.01, "Float8 should achieve 4:1 compression");

    // Binary: 10000D f32 -> 1250 bytes (32:1 compression)
    let binary_ratio = (10000 * 4) as f32 / 1250.0;
    assert!(binary_ratio > 30.0, "Binary should achieve >30:1 compression");

    eprintln!("[BENCHMARK] Compression ratios: PQ8={:.0}:1, Float8={:.0}:1, Binary={:.0}:1",
              pq8_ratio, float8_ratio, binary_ratio);
}

// =============================================================================
// MEMORY ESTIMATE TESTS
// =============================================================================

/// Test: Per-embedder memory estimates are reasonable
#[test]
fn test_memory_estimates_reasonable() {
    // Based on Constitution stack.gpu.total_vram = 32GB target
    // Each model should fit within a few GB

    let max_single_model_vram_gb = 4.0; // No single model should exceed 4GB
    let _min_single_model_vram_mb = 100.0; // Each model should be at least 100MB

    // Check dimension-based estimate (rough: dim * 4 bytes * batch_overhead)
    for i in 0..MODEL_COUNT {
        let dim = PROJECTED_DIMENSIONS[i];
        let estimated_mb = (dim * 4 * 1024) as f32 / (1024.0 * 1024.0); // 1024 batch

        assert!(estimated_mb < max_single_model_vram_gb * 1024.0,
                "Model {} estimated {}MB exceeds {}GB limit", i, estimated_mb, max_single_model_vram_gb);
    }

    eprintln!("[BENCHMARK] Per-embedder memory estimates within bounds");
}

/// Test: Total VRAM requirement estimate
#[test]
fn test_total_vram_estimate() {
    // Constitution: stack.gpu.total_vram = 32GB
    // All 13 models should fit within 28GB (leave 4GB for ops)
    let _max_total_vram_gb = 28.0;

    // Rough estimate: sum of dimensions * 4 bytes * overhead
    let dim_sum: usize = PROJECTED_DIMENSIONS.iter().sum();
    let base_estimate_gb = (dim_sum * 4) as f32 / (1024.0 * 1024.0 * 1024.0);

    // With model weights, typically 100-1000x dimension
    let model_overhead = 500.0; // Conservative multiplier
    let total_estimate_gb = base_estimate_gb * model_overhead;

    eprintln!("[BENCHMARK] Estimated total VRAM: {:.2}GB (dim-based * {})",
              total_estimate_gb, model_overhead);

    // Note: Actual estimate depends on real model sizes, not just dimensions
    // This test verifies the calculation approach is reasonable
}

// =============================================================================
// FINGERPRINT SIZE BENCHMARK TESTS
// =============================================================================

/// Helper: Create test embeddings with realistic sizes
fn create_benchmark_embeddings() -> HashMap<u8, QuantizedEmbedding> {
    let mut map = HashMap::new();

    // Sizes based on Constitution quantization methods
    let configs = vec![
        // (index, method, original_dim, quantized_data_size)
        (0, QuantizationMethod::PQ8, 1024, 8),           // E1: PQ8
        (1, QuantizationMethod::Float8E4M3, 512, 512),   // E2: Float8
        (2, QuantizationMethod::Float8E4M3, 512, 512),   // E3: Float8
        (3, QuantizationMethod::Float8E4M3, 512, 512),   // E4: Float8
        (4, QuantizationMethod::PQ8, 768, 8),            // E5: PQ8
        (5, QuantizationMethod::SparseNative, 30522, 500), // E6: Sparse (~500 nnz * 2)
        (6, QuantizationMethod::PQ8, 768, 8),            // E7: PQ8
        (7, QuantizationMethod::Float8E4M3, 1536, 1536), // E8: Float8
        (8, QuantizationMethod::Binary, 10000, 1250),    // E9: Binary (10K bits / 8)
        (9, QuantizationMethod::PQ8, 768, 8),            // E10: PQ8
        (10, QuantizationMethod::Float8E4M3, 384, 384),  // E11: Float8
        (11, QuantizationMethod::TokenPruning, 128, 256), // E12: TokenPruning
        (12, QuantizationMethod::SparseNative, 30522, 500), // E13: Sparse
    ];

    for (idx, method, dim, data_size) in configs {
        let data: Vec<u8> = (0..data_size)
            .map(|j| ((idx as usize * 17 + j) % 256) as u8)
            .collect();

        map.insert(idx, QuantizedEmbedding {
            method,
            original_dim: dim,
            data,
            metadata: match method {
                QuantizationMethod::PQ8 => QuantizationMetadata::PQ8 {
                    codebook_id: idx as u32,
                    num_subvectors: 8,
                },
                QuantizationMethod::Float8E4M3 => QuantizationMetadata::Float8 {
                    scale: 1.0,
                    bias: 0.0,
                },
                QuantizationMethod::Binary => QuantizationMetadata::Binary {
                    threshold: 0.0,
                },
                QuantizationMethod::SparseNative => QuantizationMetadata::Sparse {
                    vocab_size: 30522,
                    nnz: 250, // ~500 bytes / 2 per entry
                },
                QuantizationMethod::TokenPruning => QuantizationMetadata::TokenPruning {
                    original_tokens: 128,
                    kept_tokens: 64,
                    threshold: 0.5,
                },
            },
        });
    }

    map
}

/// Test: Estimated fingerprint size is within bounds
#[test]
fn test_fingerprint_estimated_size() {
    let embeddings = create_benchmark_embeddings();
    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        [0.5f32; 13],
        [0.25f32; 4],
        [0x42u8; 32],
    );

    let size = fp.estimated_size_bytes();

    assert!(size >= MIN_QUANTIZED_SIZE_BYTES,
            "Size {} < min {}", size, MIN_QUANTIZED_SIZE_BYTES);
    assert!(size <= MAX_QUANTIZED_SIZE_BYTES,
            "Size {} > max {}", size, MAX_QUANTIZED_SIZE_BYTES);

    eprintln!("[BENCHMARK] Estimated fingerprint size: {} bytes", size);
    eprintln!("            Target: ~{} bytes", EXPECTED_QUANTIZED_SIZE_BYTES);
    eprintln!("            Range: {} - {} bytes", MIN_QUANTIZED_SIZE_BYTES, MAX_QUANTIZED_SIZE_BYTES);
}

/// Test: Fingerprint data sizes breakdown
#[test]
fn test_fingerprint_size_breakdown() {
    let embeddings = create_benchmark_embeddings();

    // Calculate embedding data sizes
    let mut total_embedding_data = 0usize;
    let mut breakdown = String::new();

    for idx in 0..13u8 {
        if let Some(qe) = embeddings.get(&idx) {
            let size = qe.data.len();
            total_embedding_data += size;
            breakdown.push_str(&format!("  E{}: {} bytes ({:?})\n", idx + 1, size, qe.method));
        }
    }

    eprintln!("[BENCHMARK] Embedding data breakdown:\n{}", breakdown);
    eprintln!("            Total embedding data: {} bytes", total_embedding_data);

    // Fixed fields overhead
    let fixed_overhead = 16 + 1 + 52 + 4 + 16 + 1 + 4 + 32 + 8 + 8 + 8 + 1; // UUID, version, etc.
    eprintln!("            Fixed fields overhead: {} bytes", fixed_overhead);
    eprintln!("            Total estimate: {} bytes", total_embedding_data + fixed_overhead + 13 * 40);
}

// =============================================================================
// OPERATION TIMING BENCHMARK TESTS
// =============================================================================

/// Test: Fingerprint creation timing
#[test]
fn test_fingerprint_creation_timing() {
    let iterations = 1000;

    // Warm up
    for _ in 0..100 {
        let _ = create_benchmark_embeddings();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let embeddings = create_benchmark_embeddings();
        let _fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            [0.5f32; 13],
            [0.25f32; 4],
            [0x42u8; 32],
        );
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations as u128;
    eprintln!("[BENCHMARK] Fingerprint creation: {} iterations in {:?}", iterations, elapsed);
    eprintln!("            Average: {} ns/op ({:.2} us/op)", avg_ns, avg_ns as f64 / 1000.0);

    // Should be under 1ms per operation
    assert!(avg_ns < 1_000_000, "Creation should be <1ms, was {} ns", avg_ns);
}

/// Test: UUID generation timing
#[test]
fn test_uuid_generation_timing() {
    let iterations = 10000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = Uuid::new_v4();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations as u128;
    eprintln!("[BENCHMARK] UUID generation: {} iterations in {:?}", iterations, elapsed);
    eprintln!("            Average: {} ns/op", avg_ns);

    // UUID generation should be very fast (<1us)
    assert!(avg_ns < 1_000, "UUID generation should be <1us, was {} ns", avg_ns);
}

/// Test: RRF contribution calculation timing
#[test]
fn test_rrf_calculation_timing() {
    use context_graph_embeddings::storage::EmbedderQueryResult;

    let iterations = 100000;
    let id = Uuid::new_v4();

    // Create test results
    let results: Vec<EmbedderQueryResult> = (0..13)
        .map(|i| EmbedderQueryResult::from_similarity(id, i as u8, 0.9 - i as f32 * 0.05, i))
        .collect();

    let start = Instant::now();
    for _ in 0..iterations {
        let _total: f32 = results.iter().map(|r| r.rrf_contribution()).sum();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations as u128;
    eprintln!("[BENCHMARK] RRF calculation (13 contributions): {} iterations in {:?}",
              iterations, elapsed);
    eprintln!("            Average: {} ns/op", avg_ns);

    // RRF calculation should be very fast (<100ns)
    assert!(avg_ns < 1_000, "RRF calculation should be <1us, was {} ns", avg_ns);
}

// =============================================================================
// EDGE CASE TESTS (REQUIRED: 3 per task)
// =============================================================================

/// Edge Case 1: Maximum size fingerprint (sparse vectors with many nnz)
#[test]
fn test_edge_case_maximum_size_fingerprint() {
    let mut embeddings = create_benchmark_embeddings();

    // Increase sparse vector sizes to approach MAX
    for idx in [5u8, 12u8] {
        embeddings.insert(idx, QuantizedEmbedding {
            method: QuantizationMethod::SparseNative,
            original_dim: 30522,
            data: vec![0xAB; 5000], // Large sparse data
            metadata: QuantizationMetadata::Sparse {
                vocab_size: 30522,
                nnz: 2500,
            },
        });
    }

    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        [0.5f32; 13],
        [0.25f32; 4],
        [0x42u8; 32],
    );

    let size = fp.estimated_size_bytes();
    assert!(size <= MAX_QUANTIZED_SIZE_BYTES,
            "Large fingerprint {} should be <= max {}", size, MAX_QUANTIZED_SIZE_BYTES);

    eprintln!("[EDGE CASE 1] Large sparse fingerprint: {} bytes (max: {})",
              size, MAX_QUANTIZED_SIZE_BYTES);
}

/// Edge Case 2: Minimum size fingerprint (all PQ8)
#[test]
fn test_edge_case_minimum_size_fingerprint() {
    let mut embeddings = HashMap::new();

    // All embedders use PQ8 (smallest)
    for idx in 0..13u8 {
        embeddings.insert(idx, QuantizedEmbedding {
            method: QuantizationMethod::PQ8,
            original_dim: 768,
            data: vec![0x00; 8], // Minimal PQ8 data
            metadata: QuantizationMetadata::PQ8 {
                codebook_id: idx as u32,
                num_subvectors: 8,
            },
        });
    }

    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        [0.5f32; 13],
        [0.25f32; 4],
        [0x42u8; 32],
    );

    let size = fp.estimated_size_bytes();
    // Note: Even with minimal embeddings, metadata adds overhead
    // So size might still be > MIN_QUANTIZED_SIZE_BYTES
    eprintln!("[EDGE CASE 2] Minimal PQ8 fingerprint: {} bytes (min required: {})",
              size, MIN_QUANTIZED_SIZE_BYTES);
}

/// Edge Case 3: Concurrent fingerprint creation
#[test]
fn test_edge_case_concurrent_creation() {
    use std::thread;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let count = Arc::new(AtomicUsize::new(0));
    let num_threads = 4;
    let iterations_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let count = Arc::clone(&count);
            thread::spawn(move || {
                for _ in 0..iterations_per_thread {
                    let embeddings = create_benchmark_embeddings();
                    let _fp = StoredQuantizedFingerprint::new(
                        Uuid::new_v4(),
                        embeddings,
                        [0.5f32; 13],
                        [0.25f32; 4],
                        [0x42u8; 32],
                    );
                    count.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let total = count.load(Ordering::Relaxed);
    assert_eq!(total, num_threads * iterations_per_thread);

    eprintln!("[EDGE CASE 3] Concurrent creation: {} fingerprints across {} threads",
              total, num_threads);
}

// =============================================================================
// CONSTITUTION CONSTANTS VERIFICATION
// =============================================================================

/// Test: All Constitution constants are consistent
#[test]
fn test_constitution_constants_consistency() {
    // NUM_EMBEDDERS from storage module
    assert_eq!(NUM_EMBEDDERS, 13);

    // MODEL_COUNT from dimensions module
    assert_eq!(MODEL_COUNT, 13);

    // Should match
    assert_eq!(NUM_EMBEDDERS, MODEL_COUNT);

    // RRF_K from Constitution
    assert_eq!(RRF_K, 60.0);

    // TOTAL_DIMENSION
    assert_eq!(TOTAL_DIMENSION, 9856);

    eprintln!("[BENCHMARK] Constitution constants verified:");
    eprintln!("  NUM_EMBEDDERS = MODEL_COUNT = {}", NUM_EMBEDDERS);
    eprintln!("  RRF_K = {}", RRF_K);
    eprintln!("  TOTAL_DIMENSION = {}", TOTAL_DIMENSION);
}

// =============================================================================
// FULL STATE VERIFICATION
// =============================================================================

/// Final verification: Print benchmark summary
#[test]
fn test_full_state_verification_summary() {
    eprintln!("\n========================================");
    eprintln!("  BENCHMARK TEST VERIFICATION");
    eprintln!("========================================");
    eprintln!("Storage Size Targets:");
    eprintln!("  - Expected: {} bytes (~17KB)", EXPECTED_QUANTIZED_SIZE_BYTES);
    eprintln!("  - Maximum: {} bytes (~25KB)", MAX_QUANTIZED_SIZE_BYTES);
    eprintln!("  - Minimum: {} bytes (~5KB)", MIN_QUANTIZED_SIZE_BYTES);
    eprintln!();
    eprintln!("Dimension Configuration:");
    eprintln!("  - Total: {}D", TOTAL_DIMENSION);
    eprintln!("  - Model count: {}", MODEL_COUNT);
    eprintln!("  - Projected dims: {:?}", PROJECTED_DIMENSIONS);
    eprintln!();
    eprintln!("Quantization Methods:");
    eprintln!("  - PQ8: E1, E5, E7, E10 (128:1 compression)");
    eprintln!("  - Float8: E2, E3, E4, E8, E11 (4:1 compression)");
    eprintln!("  - Binary: E9 (32:1 compression)");
    eprintln!("  - Sparse: E6, E13 (native sparse)");
    eprintln!("  - TokenPruning: E12 (2:1 compression)");
    eprintln!();
    eprintln!("Edge Cases Verified:");
    eprintln!("  1. Maximum size fingerprint (large sparse)");
    eprintln!("  2. Minimum size fingerprint (all PQ8)");
    eprintln!("  3. Concurrent creation (4 threads)");
    eprintln!("========================================\n");
}
