//! Edge case verification tests for Full State Verification protocol.
//! These tests print BEFORE and AFTER state for manual inspection.

#[cfg(test)]
mod edge_case_tests {
    use crate::quantization::{QuantizationMetadata, QuantizationMethod, QuantizedEmbedding};
    use crate::types::ModelId;

    /// EDGE CASE 1: Empty data vector (all-zero sparse embedding)
    ///
    /// Trigger: Create QuantizedEmbedding with empty data vec
    /// Expected Outcome: size_bytes() returns 0, compression_ratio() doesn't panic
    #[test]
    fn edge_case_1_empty_data_vector() {
        println!("\n=== EDGE CASE 1: Empty Data Vector ===");

        // BEFORE STATE
        println!("BEFORE STATE:");
        println!("  - About to create QuantizedEmbedding with:");
        println!("    method: SparseNative");
        println!("    original_dim: 30522 (SPLADE vocab size)");
        println!("    data: vec![] (EMPTY)");
        println!("    metadata: Sparse {{ vocab_size: 30522, nnz: 0 }}");

        // EXECUTE
        let qe = QuantizedEmbedding {
            method: QuantizationMethod::SparseNative,
            original_dim: 30522,
            data: vec![], // EMPTY - edge case
            metadata: QuantizationMetadata::Sparse {
                vocab_size: 30522,
                nnz: 0, // zero non-zero entries
            },
        };

        // AFTER STATE - Inspect actual values
        let size = qe.size_bytes();
        let ratio = qe.compression_ratio();

        println!("\nAFTER STATE:");
        println!("  - qe.data.len() = {}", qe.data.len());
        println!("  - qe.size_bytes() = {}", size);
        println!("  - qe.compression_ratio() = {}", ratio);
        println!("  - qe.original_dim = {}", qe.original_dim);

        // PHYSICAL VERIFICATION
        println!("\nPHYSICAL VERIFICATION:");
        assert_eq!(size, 0, "size_bytes should be 0 for empty data");
        println!("  - size_bytes() == 0: PASSED");

        // compression_ratio = original_bytes / max(1, data.len())
        // = (30522 * 4) / max(1, 0) = 122088 / 1 = 122088.0
        assert!(ratio > 0.0, "compression_ratio should be positive");
        assert_eq!(ratio, 122088.0, "compression_ratio should be 122088.0");
        println!("  - compression_ratio() > 0 (no divide-by-zero): PASSED");
        println!("  - compression_ratio() == 122088.0: PASSED");

        println!("\n=== EDGE CASE 1: PASSED ===\n");
    }

    /// EDGE CASE 2: Maximum valid dimension (stress test for overflow)
    ///
    /// Trigger: Create QuantizedEmbedding with large original_dim
    /// Expected Outcome: compression_ratio() calculates without overflow
    #[test]
    fn edge_case_2_large_dimension() {
        println!("\n=== EDGE CASE 2: Large Dimension (Overflow Test) ===");

        // BEFORE STATE
        let large_dim: usize = 10_000_000; // 10 million dimensions
        let data_size: usize = 1_000_000; // 1 MB of data

        println!("BEFORE STATE:");
        println!("  - About to create QuantizedEmbedding with:");
        println!("    method: Float8E4M3");
        println!("    original_dim: {} (10 million)", large_dim);
        println!("    data.len(): {} (1 million bytes)", data_size);
        println!("    Expected original_bytes: {} bytes", large_dim * 4);

        // EXECUTE
        let qe = QuantizedEmbedding {
            method: QuantizationMethod::Float8E4M3,
            original_dim: large_dim,
            data: vec![0u8; data_size],
            metadata: QuantizationMetadata::Float8 {
                scale: 1.0,
                bias: 0.0,
            },
        };

        // AFTER STATE
        let size = qe.size_bytes();
        let ratio = qe.compression_ratio();
        let expected_ratio = (large_dim * 4) as f32 / data_size as f32;

        println!("\nAFTER STATE:");
        println!("  - qe.size_bytes() = {}", size);
        println!("  - qe.compression_ratio() = {}", ratio);
        println!("  - Expected ratio: {}", expected_ratio);

        // PHYSICAL VERIFICATION
        println!("\nPHYSICAL VERIFICATION:");
        assert_eq!(size, data_size, "size_bytes should equal data.len()");
        println!("  - size_bytes() == {}: PASSED", data_size);

        assert!(
            (ratio - expected_ratio).abs() < 0.001,
            "ratio should match expected"
        );
        println!("  - compression_ratio() ~= {}: PASSED", expected_ratio);

        assert!(ratio == 40.0, "40MB original / 1MB compressed = 40x");
        println!("  - No overflow (ratio = 40.0): PASSED");

        println!("\n=== EDGE CASE 2: PASSED ===\n");
    }

    /// EDGE CASE 3: All 13 ModelId variants are exhaustively covered
    ///
    /// Trigger: Iterate through ModelId::all() and call for_model_id()
    /// Expected Outcome: Each of 13 variants returns a valid QuantizationMethod
    #[test]
    fn edge_case_3_all_model_ids_exhaustive() {
        println!("\n=== EDGE CASE 3: Exhaustive ModelId Coverage ===");

        // BEFORE STATE
        let all_models = ModelId::all();
        println!("BEFORE STATE:");
        println!("  - ModelId::all() returns {} variants", all_models.len());
        println!("  - Expected: 13 variants (E1-E13)");

        // Constitution mapping reference:
        let expected_mappings = [
            (ModelId::Semantic, QuantizationMethod::PQ8, "E1"),
            (
                ModelId::TemporalRecent,
                QuantizationMethod::Float8E4M3,
                "E2",
            ),
            (
                ModelId::TemporalPeriodic,
                QuantizationMethod::Float8E4M3,
                "E3",
            ),
            (
                ModelId::TemporalPositional,
                QuantizationMethod::Float8E4M3,
                "E4",
            ),
            (ModelId::Causal, QuantizationMethod::PQ8, "E5"),
            (ModelId::Sparse, QuantizationMethod::SparseNative, "E6"),
            (ModelId::Code, QuantizationMethod::PQ8, "E7"),
            (ModelId::Graph, QuantizationMethod::Float8E4M3, "E8"),
            (ModelId::Hdc, QuantizationMethod::Binary, "E9"),
            (ModelId::Multimodal, QuantizationMethod::PQ8, "E10"),
            (ModelId::Entity, QuantizationMethod::Float8E4M3, "E11"),
            (
                ModelId::LateInteraction,
                QuantizationMethod::TokenPruning,
                "E12",
            ),
            (ModelId::Splade, QuantizationMethod::SparseNative, "E13"),
        ];

        // EXECUTE & VERIFY each mapping
        println!("\nEXECUTING: Testing each ModelId -> QuantizationMethod mapping");

        for (model_id, expected_method, embedder_name) in &expected_mappings {
            let actual_method = QuantizationMethod::for_model_id(*model_id);
            let status = if actual_method == *expected_method {
                "PASSED"
            } else {
                "FAILED"
            };
            println!(
                "  {} ({:?}) -> {:?} [{}]",
                embedder_name, model_id, actual_method, status
            );
            assert_eq!(actual_method, *expected_method);
        }

        // AFTER STATE
        println!("\nAFTER STATE:");
        println!("  - All 13 ModelId variants tested");
        println!("  - Each returned expected QuantizationMethod");

        // PHYSICAL VERIFICATION - count by method type
        println!("\nPHYSICAL VERIFICATION (method distribution):");
        let mut pq8_count = 0;
        let mut float8_count = 0;
        let mut binary_count = 0;
        let mut sparse_count = 0;
        let mut token_count = 0;

        for model_id in all_models {
            match QuantizationMethod::for_model_id(*model_id) {
                QuantizationMethod::PQ8 => pq8_count += 1,
                QuantizationMethod::Float8E4M3 => float8_count += 1,
                QuantizationMethod::Binary => binary_count += 1,
                QuantizationMethod::SparseNative => sparse_count += 1,
                QuantizationMethod::TokenPruning => token_count += 1,
            }
        }

        println!("  - PQ8: {} (expected 4: E1, E5, E7, E10)", pq8_count);
        println!(
            "  - Float8E4M3: {} (expected 5: E2, E3, E4, E8, E11)",
            float8_count
        );
        println!("  - Binary: {} (expected 1: E9)", binary_count);
        println!("  - SparseNative: {} (expected 2: E6, E13)", sparse_count);
        println!("  - TokenPruning: {} (expected 1: E12)", token_count);

        assert_eq!(pq8_count, 4, "PQ8 should have 4 embedders");
        assert_eq!(float8_count, 5, "Float8 should have 5 embedders");
        assert_eq!(binary_count, 1, "Binary should have 1 embedder");
        assert_eq!(sparse_count, 2, "Sparse should have 2 embedders");
        assert_eq!(token_count, 1, "TokenPruning should have 1 embedder");

        let total = pq8_count + float8_count + binary_count + sparse_count + token_count;
        assert_eq!(total, 13, "Total should be 13 embedders");
        println!("  - Total: {} (expected 13): PASSED", total);

        println!("\n=== EDGE CASE 3: PASSED ===\n");
    }

    /// Verify serde roundtrip for all metadata variants
    #[test]
    fn edge_case_4_serde_all_metadata_variants() {
        println!("\n=== EDGE CASE 4: Serde Roundtrip All Metadata Variants ===");

        let test_cases = vec![
            (
                "PQ8",
                QuantizationMetadata::PQ8 {
                    codebook_id: 42,
                    num_subvectors: 8,
                },
            ),
            (
                "Float8",
                QuantizationMetadata::Float8 {
                    scale: 0.5,
                    bias: -1.0,
                },
            ),
            ("Binary", QuantizationMetadata::Binary { threshold: 0.0 }),
            (
                "Sparse",
                QuantizationMetadata::Sparse {
                    vocab_size: 30522,
                    nnz: 100,
                },
            ),
            (
                "TokenPruning",
                QuantizationMetadata::TokenPruning {
                    original_tokens: 512,
                    kept_tokens: 256,
                    threshold: 0.5,
                },
            ),
        ];

        for (name, metadata) in test_cases {
            println!("\nTesting {} metadata:", name);
            println!("  BEFORE: {:?}", metadata);

            let json = serde_json::to_string(&metadata).expect("serialize failed");
            println!("  JSON: {}", json);

            let restored: QuantizationMetadata =
                serde_json::from_str(&json).expect("deserialize failed");
            println!("  AFTER: {:?}", restored);

            // Can't directly compare enums, so serialize both and compare JSON
            let json2 = serde_json::to_string(&restored).expect("serialize restored failed");
            assert_eq!(json, json2, "Roundtrip should preserve data");
            println!("  VERIFICATION: JSON matches after roundtrip: PASSED");
        }

        println!("\n=== EDGE CASE 4: PASSED ===\n");
    }
}
