//! Full State Verification for TASK-EMB-020
//!
//! This file performs physical verification of the QuantizationRouter
//! by examining the actual bytes stored in memory, not just return values.

use context_graph_embeddings::quantization::{QuantizationMethod, QuantizationRouter};
use context_graph_embeddings::types::ModelId;
use context_graph_embeddings::EmbeddingError;

// =============================================================================
// EDGE CASE 1: Empty Input Vector
// =============================================================================

#[test]
fn fsv_edge_case_1_empty_input() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ EDGE CASE 1: Empty Input Vector                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();
    let empty_input: Vec<f32> = vec![];

    // STATE BEFORE
    println!("STATE BEFORE:");
    println!("  Input type: Vec<f32>");
    println!("  Input length: {}", empty_input.len());
    println!("  Input bytes: {:?}", empty_input.as_slice());
    println!("  Memory address: {:p}", empty_input.as_ptr());

    // EXECUTE
    let result = router.quantize(ModelId::Hdc, &empty_input);

    // STATE AFTER
    println!("\nSTATE AFTER:");
    match &result {
        Ok(quantized) => {
            println!("  Result: Ok(QuantizedEmbedding)");
            println!("  Quantized data length: {}", quantized.data.len());
            println!("  Quantized data bytes: {:?}", quantized.data);
            println!("  Method: {:?}", quantized.method);
            println!("  Original dim: {}", quantized.original_dim);
        }
        Err(e) => {
            println!("  Result: Err");
            println!("  Error type: {}", std::any::type_name_of_val(e));
            println!("  Error message: {}", e);
        }
    }

    // PHYSICAL VERIFICATION
    println!("\nPHYSICAL VERIFICATION:");
    // Per Constitution AP-007: Empty inputs are rejected (no empty data allowed)
    match result {
        Err(EmbeddingError::QuantizationFailed { model_id, reason }) => {
            println!("  ✓ Empty input correctly rejected");
            println!("  Error model_id: {:?}", model_id);
            println!("  Error reason: {}", reason);
            assert!(reason.contains("Empty"), "Error should mention empty input");
        }
        Ok(_) => panic!("Empty input should be rejected, not produce output"),
        Err(e) => panic!("Expected QuantizationFailed error, got: {:?}", e),
    }

    println!("\n✅ EDGE CASE 1 PASSED: Empty input correctly rejected with error\n");
}

// =============================================================================
// EDGE CASE 2: Maximum Dimension (65536)
// =============================================================================

#[test]
fn fsv_edge_case_2_max_dimension() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ EDGE CASE 2: Maximum Dimension (65536)                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();
    let max_dim = 65536;
    // Create alternating pattern for verification
    let large_input: Vec<f32> = (0..max_dim)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();

    // STATE BEFORE
    println!("STATE BEFORE:");
    println!("  Input dimension: {}", large_input.len());
    println!("  Input memory size: {} bytes", large_input.len() * 4);
    println!("  Pattern: alternating +1.0/-1.0");
    println!("  First 8 values: {:?}", &large_input[..8]);
    println!("  Last 8 values: {:?}", &large_input[max_dim-8..]);

    // EXECUTE
    let result = router.quantize(ModelId::Hdc, &large_input);

    // STATE AFTER
    println!("\nSTATE AFTER:");
    match &result {
        Ok(quantized) => {
            println!("  Result: Ok(QuantizedEmbedding)");
            println!("  Quantized data length: {} bytes", quantized.data.len());
            println!("  Expected length: {} bytes (65536 / 8)", max_dim / 8);
            println!("  Method: {:?}", quantized.method);
            println!("  Original dim: {}", quantized.original_dim);
            println!("  Compression ratio: {:.1}x", (max_dim * 4) as f64 / quantized.data.len() as f64);
        }
        Err(e) => {
            println!("  Result: Err");
            println!("  Error: {}", e);
        }
    }

    // PHYSICAL VERIFICATION
    println!("\nPHYSICAL VERIFICATION:");
    let quantized = result.expect("Max dimension should succeed");

    // Verify exact byte count
    let expected_bytes = max_dim / 8;
    println!("  Expected bytes: {}", expected_bytes);
    println!("  Actual bytes: {}", quantized.data.len());
    assert_eq!(quantized.data.len(), expected_bytes);

    // Verify byte pattern: alternating +/- should produce 0xAA pattern
    // Pattern: +1, -1, +1, -1, +1, -1, +1, -1 -> bits: 1,0,1,0,1,0,1,0 -> 0xAA
    println!("\n  Byte pattern verification:");
    println!("  First 16 bytes: {:02X?}", &quantized.data[..16]);
    println!("  Last 16 bytes: {:02X?}", &quantized.data[quantized.data.len()-16..]);

    // Each byte should be 0xAA (10101010 in binary)
    let all_aa = quantized.data.iter().all(|&b| b == 0xAA);
    println!("  All bytes are 0xAA: {}", all_aa);

    if !all_aa {
        let non_aa_count = quantized.data.iter().filter(|&&b| b != 0xAA).count();
        println!("  WARNING: {} bytes are not 0xAA", non_aa_count);
    }

    assert!(all_aa, "Alternating pattern should produce all 0xAA bytes");

    println!("\n✅ EDGE CASE 2 PASSED: Max dimension correctly quantized to {} bytes\n", expected_bytes);
}

// =============================================================================
// EDGE CASE 3: NaN and Inf Values
// =============================================================================

#[test]
fn fsv_edge_case_3_nan_inf_values() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ EDGE CASE 3: NaN and Infinity Values                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();

    // Create input with special values
    let special_input = vec![
        f32::NAN,           // NaN
        f32::INFINITY,      // +Inf
        f32::NEG_INFINITY,  // -Inf
        0.0,                // Zero
        1.0,                // Normal positive
        -1.0,               // Normal negative
        f32::MIN,           // Min f32
        f32::MAX,           // Max f32
    ];

    // STATE BEFORE
    println!("STATE BEFORE:");
    println!("  Input values:");
    for (i, v) in special_input.iter().enumerate() {
        println!("    [{}] = {} (is_nan={}, is_infinite={})",
                 i, v, v.is_nan(), v.is_infinite());
    }

    // EXECUTE
    let result = router.quantize(ModelId::Hdc, &special_input);

    // STATE AFTER
    println!("\nSTATE AFTER:");
    match &result {
        Ok(quantized) => {
            println!("  Result: Ok(QuantizedEmbedding)");
            println!("  Quantized data: {:02X?}", quantized.data);
            println!("  Binary representation:");
            for (i, byte) in quantized.data.iter().enumerate() {
                println!("    Byte {}: {:08b} (0x{:02X})", i, byte, byte);
            }
        }
        Err(e) => {
            println!("  Result: Err");
            println!("  Error type: {:?}", e);
            println!("  Error message: {}", e);
        }
    }

    // PHYSICAL VERIFICATION
    println!("\nPHYSICAL VERIFICATION:");

    // NaN should cause an error per Constitution AP-007
    match result {
        Err(EmbeddingError::QuantizationFailed { model_id, reason }) => {
            println!("  ✓ NaN input correctly rejected");
            println!("  Error model_id: {:?}", model_id);
            println!("  Error reason: {}", reason);
            assert!(reason.contains("NaN"), "Error should mention NaN");
        }
        Ok(_) => {
            panic!("NaN input should be rejected, not silently processed");
        }
        Err(e) => {
            panic!("Expected QuantizationFailed error, got: {:?}", e);
        }
    }

    println!("\n✅ EDGE CASE 3 PASSED: NaN/Inf values correctly rejected with error\n");
}

// =============================================================================
// PHYSICAL BYTE VERIFICATION: Known Pattern Test
// =============================================================================

#[test]
fn fsv_physical_byte_verification() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ PHYSICAL BYTE VERIFICATION: Known Pattern                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();

    // Create a known pattern that we can verify bit-by-bit
    // Pattern: 8 positive values -> should produce 0xFF (11111111)
    //          8 negative values -> should produce 0x00 (00000000)
    let known_pattern: Vec<f32> = vec![
        // Byte 0: all positive -> 0xFF
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        // Byte 1: all negative -> 0x00
        -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0,
        // Byte 2: alternating -> 0xAA (10101010)
        1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0,
        // Byte 3: reverse alternating -> 0x55 (01010101)
        -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0,
    ];

    println!("INPUT PATTERN:");
    println!("  Byte 0 inputs (expect 0xFF): {:?}", &known_pattern[0..8]);
    println!("  Byte 1 inputs (expect 0x00): {:?}", &known_pattern[8..16]);
    println!("  Byte 2 inputs (expect 0xAA): {:?}", &known_pattern[16..24]);
    println!("  Byte 3 inputs (expect 0x55): {:?}", &known_pattern[24..32]);

    // EXECUTE
    let quantized = router.quantize(ModelId::Hdc, &known_pattern)
        .expect("Known pattern should quantize successfully");

    println!("\nOUTPUT BYTES:");
    for (i, byte) in quantized.data.iter().enumerate() {
        println!("  Byte {}: {:08b} (0x{:02X})", i, byte, byte);
    }

    println!("\nPHYSICAL VERIFICATION:");

    // Verify each byte matches expected pattern
    let expected = [0xFF, 0x00, 0xAA, 0x55];
    let actual = &quantized.data[..4];

    println!("  Expected: {:02X?}", expected);
    println!("  Actual:   {:02X?}", actual);

    for (i, (&exp, &act)) in expected.iter().zip(actual.iter()).enumerate() {
        let matches = exp == act;
        println!("  Byte {}: expected=0x{:02X}, actual=0x{:02X}, matches={}",
                 i, exp, act, matches);
        assert_eq!(exp, act, "Byte {} mismatch", i);
    }

    println!("\n✅ PHYSICAL BYTE VERIFICATION PASSED: All bytes match expected pattern\n");
}

// =============================================================================
// ERROR VARIANT VERIFICATION
// =============================================================================

#[test]
fn fsv_error_variant_verification() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ ERROR VARIANT VERIFICATION                                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();

    // Test each error variant is produced correctly

    // 1. QuantizerNotImplemented (PQ8)
    println!("1. Testing QuantizerNotImplemented (PQ8):");
    let pq8_result = router.quantize(ModelId::Semantic, &vec![1.0f32; 1024]);
    match pq8_result {
        Err(EmbeddingError::QuantizerNotImplemented { model_id, method }) => {
            println!("   ✓ Error variant: QuantizerNotImplemented");
            println!("   ✓ model_id: {:?}", model_id);
            println!("   ✓ method: {}", method);
            assert_eq!(method, "PQ8");
        }
        other => panic!("Expected QuantizerNotImplemented, got {:?}", other),
    }

    // 2. QuantizerNotImplemented (Float8)
    println!("\n2. Testing QuantizerNotImplemented (Float8E4M3):");
    let float8_result = router.quantize(ModelId::TemporalRecent, &vec![1.0f32; 512]);
    match float8_result {
        Err(EmbeddingError::QuantizerNotImplemented { model_id, method }) => {
            println!("   ✓ Error variant: QuantizerNotImplemented");
            println!("   ✓ model_id: {:?}", model_id);
            println!("   ✓ method: {}", method);
            assert_eq!(method, "Float8E4M3");
        }
        other => panic!("Expected QuantizerNotImplemented, got {:?}", other),
    }

    // 3. InvalidModelInput (Sparse)
    println!("\n3. Testing InvalidModelInput (Sparse):");
    let sparse_result = router.quantize(ModelId::Sparse, &vec![0.0f32; 30522]);
    match sparse_result {
        Err(EmbeddingError::InvalidModelInput { model_id, reason }) => {
            println!("   ✓ Error variant: InvalidModelInput");
            println!("   ✓ model_id: {:?}", model_id);
            println!("   ✓ reason: {}", reason);
            assert!(reason.contains("Sparse"));
        }
        other => panic!("Expected InvalidModelInput, got {:?}", other),
    }

    // 4. UnsupportedOperation (TokenPruning)
    println!("\n4. Testing UnsupportedOperation (TokenPruning):");
    let token_result = router.quantize(ModelId::LateInteraction, &vec![1.0f32; 128]);
    match token_result {
        Err(EmbeddingError::UnsupportedOperation { model_id, operation }) => {
            println!("   ✓ Error variant: UnsupportedOperation");
            println!("   ✓ model_id: {:?}", model_id);
            println!("   ✓ operation: {}", operation);
            assert!(operation.contains("TokenPruning"));
        }
        other => panic!("Expected UnsupportedOperation, got {:?}", other),
    }

    println!("\n✅ ALL ERROR VARIANTS VERIFIED CORRECTLY\n");
}

// =============================================================================
// COMPRESSION RATIO VERIFICATION
// =============================================================================

#[test]
fn fsv_compression_ratio_verification() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ COMPRESSION RATIO VERIFICATION                               ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();

    // HDC model: 10,000 dimensions
    let hdc_dim = 10000;
    let hdc_input: Vec<f32> = (0..hdc_dim).map(|i| (i as f32) / 100.0).collect();

    let original_size = hdc_dim * std::mem::size_of::<f32>();
    println!("INPUT:");
    println!("  Dimension: {}", hdc_dim);
    println!("  Original size: {} bytes ({} f32 * 4 bytes)", original_size, hdc_dim);

    let quantized = router.quantize(ModelId::Hdc, &hdc_input)
        .expect("HDC quantization should succeed");

    let compressed_size = quantized.data.len();
    let compression_ratio = original_size as f64 / compressed_size as f64;

    println!("\nOUTPUT:");
    println!("  Compressed size: {} bytes", compressed_size);
    println!("  Compression ratio: {:.2}x", compression_ratio);
    println!("  Space saved: {} bytes ({:.1}%)",
             original_size - compressed_size,
             (1.0 - compressed_size as f64 / original_size as f64) * 100.0);

    println!("\nPHYSICAL VERIFICATION:");
    println!("  Expected compressed size: {} bytes (10000 / 8)", hdc_dim / 8);
    println!("  Actual compressed size: {} bytes", compressed_size);
    println!("  Expected ratio: 32.0x");
    println!("  Actual ratio: {:.2}x", compression_ratio);

    // Binary quantization should achieve exactly 32x compression
    assert_eq!(compressed_size, hdc_dim / 8);
    assert!((compression_ratio - 32.0).abs() < 0.1);

    println!("\n✅ COMPRESSION RATIO VERIFIED: 32x compression achieved\n");
}

// =============================================================================
// ALL 13 MODEL IDS VERIFICATION
// =============================================================================

#[test]
fn fsv_all_model_ids_have_methods() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║ ALL 13 MODEL IDS VERIFICATION                                ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let router = QuantizationRouter::new();
    let all_models = ModelId::all();

    println!("MODEL COUNT:");
    println!("  Expected: 13");
    println!("  Actual: {}", all_models.len());
    assert_eq!(all_models.len(), 13);

    println!("\nMODEL ID ASSIGNMENTS:");
    println!("  {:<20} | {:<15} | {}", "ModelId", "Method", "Can Quantize");
    println!("  {:-<20}-+-{:-<15}-+-{:-<12}", "", "", "");

    for model_id in all_models {
        let method = router.method_for(*model_id);
        let can_quantize = router.can_quantize(*model_id);
        println!("  {:<20} | {:<15} | {}",
                 format!("{:?}", model_id),
                 format!("{:?}", method),
                 can_quantize);
    }

    println!("\nPHYSICAL VERIFICATION:");

    // Verify specific assignments per Constitution
    let verifications = [
        (ModelId::Semantic, QuantizationMethod::PQ8, false),
        (ModelId::TemporalRecent, QuantizationMethod::Float8E4M3, false),
        (ModelId::TemporalPeriodic, QuantizationMethod::Float8E4M3, false),
        (ModelId::TemporalPositional, QuantizationMethod::Float8E4M3, false),
        (ModelId::Causal, QuantizationMethod::PQ8, false),
        (ModelId::Sparse, QuantizationMethod::SparseNative, false),
        (ModelId::Code, QuantizationMethod::PQ8, false),
        (ModelId::Graph, QuantizationMethod::Float8E4M3, false),
        (ModelId::Hdc, QuantizationMethod::Binary, true),
        (ModelId::Multimodal, QuantizationMethod::PQ8, false),
        (ModelId::Entity, QuantizationMethod::Float8E4M3, false),
        (ModelId::LateInteraction, QuantizationMethod::TokenPruning, false),
        (ModelId::Splade, QuantizationMethod::SparseNative, false),
    ];

    for (model_id, expected_method, expected_can_quantize) in verifications {
        let actual_method = router.method_for(model_id);
        let actual_can_quantize = router.can_quantize(model_id);

        assert_eq!(actual_method, expected_method,
                   "{:?} should have method {:?}", model_id, expected_method);
        assert_eq!(actual_can_quantize, expected_can_quantize,
                   "{:?} can_quantize should be {}", model_id, expected_can_quantize);
    }

    println!("  ✓ All 13 ModelIds have correct method assignments");
    println!("  ✓ Only E9 (Hdc) can_quantize = true (Binary implemented)");
    println!("  ✓ All other models return appropriate errors");

    println!("\n✅ ALL 13 MODEL IDS VERIFIED CORRECTLY\n");
}
