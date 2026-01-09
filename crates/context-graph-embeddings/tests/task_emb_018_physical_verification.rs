//! Physical Verification for TASK-EMB-018: Binary Quantization
//!
//! This test MANUALLY verifies the actual byte patterns and state changes
//! in the binary quantization implementation. NO MOCKS - ALL REAL DATA.
//!
//! # Verification Protocol
//!
//! 1. Source of Truth: Packed byte data in `QuantizedEmbedding.data`
//! 2. Execute & Inspect: Run quantization, then inspect actual bytes
//! 3. Edge Case Audit: Empty, non-multiple-of-8, 10K dimension
//! 4. Evidence of Success: Print actual data for verification

use context_graph_embeddings::quantization::{
    BinaryEncoder, BinaryQuantizationError, QuantizationMetadata, QuantizationMethod,
    QuantizedEmbedding,
};

/// EDGE CASE 1: Empty input must fail fast with EMB-E012
#[test]
fn physical_verify_empty_input_fails_fast() {
    println!("\n=== EDGE CASE 1: Empty Input ===");
    let encoder = BinaryEncoder::new();
    let empty: Vec<f32> = vec![];

    println!("BEFORE: input = [] (empty vector)");
    println!("ACTION: encoder.quantize(&[], None)");

    let result = encoder.quantize(&empty, None);

    println!("AFTER: result.is_err() = {}", result.is_err());

    match result {
        Err(BinaryQuantizationError::EmptyInput) => {
            let msg = format!("{}", BinaryQuantizationError::EmptyInput);
            println!("ERROR MESSAGE: {}", msg);
            println!("VERIFY: Contains EMB-E012: {}", msg.contains("EMB-E012"));
            assert!(msg.contains("EMB-E012"), "Error must contain EMB-E012");
        }
        Err(e) => panic!("Wrong error type: {:?}", e),
        Ok(_) => panic!("Should have failed for empty input"),
    }
    println!("✓ PASS: Empty input fails fast with EMB-E012\n");
}

/// EDGE CASE 2: Dimension not multiple of 8
#[test]
fn physical_verify_non_multiple_of_8_dimension() {
    println!("\n=== EDGE CASE 2: Dimension Not Multiple of 8 ===");
    let encoder = BinaryEncoder::new();
    let input = vec![1.0f32; 13]; // 13 elements, needs ceil(13/8) = 2 bytes

    println!("BEFORE: input = [1.0; 13], expected_bytes = 2");
    println!("ACTION: quantize -> dequantize");

    let quantized = encoder.quantize(&input, None).expect("quantize");

    println!("AFTER QUANTIZE:");
    println!("  original_dim: {}", quantized.original_dim);
    println!("  data.len(): {}", quantized.data.len());
    println!("  data[0] (binary): {:08b}", quantized.data[0]);
    println!("  data[1] (binary): {:08b}", quantized.data[1]);

    assert_eq!(quantized.original_dim, 13);
    assert_eq!(quantized.data.len(), 2, "13 bits requires 2 bytes");

    // First 8 bits should all be 1 (0xFF), next 5 should be 1 (11111000 = 0xF8)
    assert_eq!(quantized.data[0], 0xFF, "First byte should be all 1s");
    assert_eq!(quantized.data[1], 0xF8, "Second byte should be 11111000");

    let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

    println!("AFTER DEQUANTIZE:");
    println!("  reconstructed.len(): {}", reconstructed.len());
    println!("  first 5 values: {:?}", &reconstructed[0..5]);

    assert_eq!(
        reconstructed.len(),
        13,
        "Must reconstruct exactly 13 values"
    );
    assert!(
        reconstructed.iter().all(|&v| v == 1.0),
        "All values should be +1.0"
    );
    println!("✓ PASS: Non-multiple-of-8 handled correctly\n");
}

/// EDGE CASE 3: 10K dimension HDC (Constitution compliance)
#[test]
fn physical_verify_10k_dimension_hdc() {
    println!("\n=== EDGE CASE 3: 10K Dimension HDC ===");
    let encoder = BinaryEncoder::new();

    // Alternating pattern: 1.0, -1.0, 1.0, -1.0, ...
    let input: Vec<f32> = (0..10000)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();

    println!("BEFORE: input = [alternating 1.0, -1.0; 10000]");
    println!("  input size (f32): {} bytes", input.len() * 4);
    println!("  expected packed: {} bytes", (10000 + 7) / 8);

    let quantized = encoder.quantize(&input, None).expect("quantize 10K");

    println!("AFTER QUANTIZE:");
    println!("  original_dim: {}", quantized.original_dim);
    println!("  packed bytes: {}", quantized.data.len());
    println!("  compression_ratio: {:.2}x", quantized.compression_ratio());
    println!("  first byte (binary): {:08b}", quantized.data[0]);

    // First byte should be 10101010 (alternating bits, MSB first)
    assert_eq!(
        quantized.data[0], 0b10101010,
        "First byte should be 10101010"
    );
    assert_eq!(quantized.data.len(), 1250, "10000 bits = 1250 bytes");

    let ratio = quantized.compression_ratio();
    assert!(
        (ratio - 32.0).abs() < 0.1,
        "Compression ratio must be ~32x, got {:.2}x",
        ratio
    );

    let reconstructed = encoder.dequantize(&quantized).expect("dequantize 10K");

    println!("AFTER DEQUANTIZE:");
    println!("  reconstructed.len(): {}", reconstructed.len());

    // Count sign mismatches
    let sign_mismatches: usize = input
        .iter()
        .zip(reconstructed.iter())
        .filter(|(&orig, &recon)| (orig >= 0.0) != (recon >= 0.0))
        .count();

    println!("  sign_mismatches: {}", sign_mismatches);
    assert_eq!(sign_mismatches, 0, "All signs must match in roundtrip");

    println!("✓ PASS: 10K HDC with 32x compression\n");
}

/// Verify 32x compression ratio matches Constitution
#[test]
fn physical_verify_32x_compression_ratio() {
    println!("\n=== 32x COMPRESSION RATIO VERIFICATION ===");
    let encoder = BinaryEncoder::new();
    let input = vec![0.0f32; 1024]; // 1024 f32 = 4096 bytes

    println!("BEFORE: 1024 f32 values = {} bytes", 1024 * 4);

    let quantized = encoder.quantize(&input, None).expect("quantize");

    println!("AFTER:");
    println!("  packed bytes: {}", quantized.data.len());
    println!("  original f32 bytes: {}", 1024 * 4);
    println!("  compression_ratio: {:.2}x", quantized.compression_ratio());

    assert_eq!(quantized.data.len(), 128, "1024 bits = 128 bytes");
    assert!(
        (quantized.compression_ratio() - 32.0).abs() < 0.1,
        "Must be 32x compression"
    );

    println!("✓ PASS: 32x compression verified\n");
}

/// Verify actual bit pattern for basic input
#[test]
fn physical_verify_bit_pattern_0xad() {
    println!("\n=== BIT PATTERN VERIFICATION ===");
    let encoder = BinaryEncoder::new();
    let input = vec![0.5, -0.3, 0.1, -0.8, 0.0, 0.9, -0.1, 0.2];

    println!("INPUT: {:?}", input);
    println!("THRESHOLD: 0.0 (default, sign-based)");
    println!("EXPECTED BITS (>= 0.0):");
    println!("  [0.5>=0]  -> 1");
    println!("  [-0.3>=0] -> 0");
    println!("  [0.1>=0]  -> 1");
    println!("  [-0.8>=0] -> 0");
    println!("  [0.0>=0]  -> 1");
    println!("  [0.9>=0]  -> 1");
    println!("  [-0.1>=0] -> 0");
    println!("  [0.2>=0]  -> 1");
    println!("EXPECTED: 10101101 = 0xAD = 173");

    let quantized = encoder.quantize(&input, None).expect("quantize");

    println!("ACTUAL:");
    println!("  byte value: {}", quantized.data[0]);
    println!("  hex: 0x{:02X}", quantized.data[0]);
    println!("  binary: {:08b}", quantized.data[0]);

    assert_eq!(
        quantized.data[0], 0xAD,
        "Bit pattern must be 0xAD (10101101)"
    );

    println!("✓ PASS: Bit pattern 0xAD verified\n");
}

/// Verify all error codes contain EMB-E012
#[test]
fn physical_verify_emb_e012_error_codes() {
    println!("\n=== EMB-E012 ERROR CODE VERIFICATION ===");
    let encoder = BinaryEncoder::new();

    // Test 1: EmptyInput
    let empty_err = BinaryQuantizationError::EmptyInput;
    let empty_msg = format!("{}", empty_err);
    println!("EmptyInput: {}", empty_msg);
    assert!(
        empty_msg.contains("EMB-E012"),
        "EmptyInput must have EMB-E012"
    );

    // Test 2: InvalidValue (NaN)
    let nan_err = encoder.quantize(&[1.0, f32::NAN, 2.0], None).unwrap_err();
    let nan_msg = format!("{}", nan_err);
    println!("InvalidValue (NaN): {}", nan_msg);
    assert!(
        nan_msg.contains("EMB-E012"),
        "InvalidValue must have EMB-E012"
    );
    assert!(nan_msg.contains("index 1"), "Must identify index 1");

    // Test 3: InvalidValue (Infinity)
    let inf_err = encoder
        .quantize(&[1.0, 2.0, f32::INFINITY], None)
        .unwrap_err();
    let inf_msg = format!("{}", inf_err);
    println!("InvalidValue (Inf): {}", inf_msg);
    assert!(
        inf_msg.contains("EMB-E012"),
        "InvalidValue must have EMB-E012"
    );
    assert!(inf_msg.contains("index 2"), "Must identify index 2");

    // Test 4: DataLengthMismatch
    let bad_data = QuantizedEmbedding {
        method: QuantizationMethod::Binary,
        original_dim: 16, // Needs 2 bytes
        data: vec![0xFF], // Only 1 byte
        metadata: QuantizationMetadata::Binary { threshold: 0.0 },
    };
    let len_err = encoder.dequantize(&bad_data).unwrap_err();
    let len_msg = format!("{}", len_err);
    println!("DataLengthMismatch: {}", len_msg);
    assert!(
        len_msg.contains("EMB-E012"),
        "DataLengthMismatch must have EMB-E012"
    );

    // Test 5: MetadataMismatch
    let bad_meta = QuantizedEmbedding {
        method: QuantizationMethod::Binary,
        original_dim: 8,
        data: vec![0xFF],
        metadata: QuantizationMetadata::Float8 {
            scale: 1.0,
            bias: 0.0,
        },
    };
    let meta_err = encoder.dequantize(&bad_meta).unwrap_err();
    let meta_msg = format!("{}", meta_err);
    println!("MetadataMismatch: {}", meta_msg);
    assert!(
        meta_msg.contains("EMB-E012"),
        "MetadataMismatch must have EMB-E012"
    );

    println!("✓ PASS: All error codes verified\n");
}

/// Verify Hamming distance and similarity
#[test]
fn physical_verify_hamming_metrics() {
    println!("\n=== HAMMING METRICS VERIFICATION ===");
    let encoder = BinaryEncoder::new();

    let a_input = vec![1.0f32; 8];
    let b_input = vec![1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0];

    println!("A input: {:?}", a_input);
    println!("B input: {:?}", b_input);

    let a = encoder.quantize(&a_input, None).expect("a");
    let b = encoder.quantize(&b_input, None).expect("b");

    println!("A byte: {:08b} (0x{:02X})", a.data[0], a.data[0]);
    println!("B byte: {:08b} (0x{:02X})", b.data[0], b.data[0]);

    let distance = BinaryEncoder::hamming_distance(&a, &b);
    let similarity = BinaryEncoder::hamming_similarity(&a, &b);

    println!("Hamming distance: {}", distance);
    println!("Hamming similarity: {:.4}", similarity);

    // A = 11111111, B = 11110000
    // XOR = 00001111 = 4 bits different
    assert_eq!(distance, 4, "Should have 4 differing bits");
    assert!(
        (similarity - 0.5).abs() < 0.01,
        "Should be 50% similar (4/8 same)"
    );

    // Test identical
    let a_copy = encoder.quantize(&a_input, None).expect("a_copy");
    let identical_dist = BinaryEncoder::hamming_distance(&a, &a_copy);
    println!("Identical distance: {}", identical_dist);
    assert_eq!(identical_dist, 0, "Identical should have distance 0");

    // Test opposite
    let opposite_input = vec![-1.0f32; 8];
    let opposite = encoder.quantize(&opposite_input, None).expect("opposite");
    let opposite_dist = BinaryEncoder::hamming_distance(&a, &opposite);
    println!("Opposite distance: {}", opposite_dist);
    assert_eq!(opposite_dist, 8, "Opposite should have distance = dim");

    println!("✓ PASS: Hamming metrics verified\n");
}

/// Verify bipolar dequantization produces +1.0 and -1.0 only
#[test]
fn physical_verify_bipolar_values() {
    println!("\n=== BIPOLAR DEQUANTIZATION VERIFICATION ===");
    let encoder = BinaryEncoder::new();
    let input = vec![0.5, -0.5, 0.1, -0.9, 0.0, -0.0001];

    println!("INPUT: {:?}", input);

    let quantized = encoder.quantize(&input, None).expect("quantize");
    let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

    println!("RECONSTRUCTED: {:?}", reconstructed);

    // All values must be exactly +1.0 or -1.0
    for (i, &val) in reconstructed.iter().enumerate() {
        assert!(
            val == 1.0 || val == -1.0,
            "Value at {} must be +1.0 or -1.0, got {}",
            i,
            val
        );
    }

    // Check specific values match expected signs
    assert_eq!(reconstructed[0], 1.0, "0.5 >= 0 -> +1.0");
    assert_eq!(reconstructed[1], -1.0, "-0.5 < 0 -> -1.0");
    assert_eq!(reconstructed[2], 1.0, "0.1 >= 0 -> +1.0");
    assert_eq!(reconstructed[3], -1.0, "-0.9 < 0 -> -1.0");
    assert_eq!(reconstructed[4], 1.0, "0.0 >= 0 -> +1.0");
    assert_eq!(reconstructed[5], -1.0, "-0.0001 < 0 -> -1.0");

    println!("✓ PASS: Bipolar values verified\n");
}
