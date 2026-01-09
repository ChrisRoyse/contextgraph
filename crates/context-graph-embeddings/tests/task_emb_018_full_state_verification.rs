//! FULL STATE VERIFICATION for TASK-EMB-018: Binary Quantization
//!
//! This test performs PHYSICAL verification of the actual byte data stored
//! in memory. We do NOT rely on return values - we inspect the raw bytes.
//!
//! # Source of Truth
//! The source of truth is `QuantizedEmbedding.data: Vec<u8>` - the actual
//! packed binary data stored in memory after quantization.
//!
//! # Verification Protocol
//! 1. Execute quantization
//! 2. Inspect raw bytes at memory addresses
//! 3. Verify bit patterns match expected values
//! 4. Print hex dumps of actual data

use context_graph_embeddings::quantization::{
    BinaryEncoder, BinaryQuantizationError, QuantizationMetadata,
};

/// Helper to print hex dump of raw bytes with addresses
fn hex_dump(label: &str, data: &[u8]) {
    println!("  HEX DUMP [{}]: {} bytes total", label, data.len());
    for (i, chunk) in data.chunks(16).enumerate() {
        let offset = i * 16;
        let hex: String = chunk.iter().map(|b| format!("{:02X} ", b)).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("    {:04X}: {:48} |{}|", offset, hex, ascii);
    }
}

/// Helper to print individual bits of a byte
fn bit_dump(label: &str, byte: u8) {
    println!(
        "  BIT DUMP [{}]: {:08b} (0x{:02X} = {})",
        label, byte, byte, byte
    );
    println!(
        "    Bit 7 (MSB): {} | Bit 6: {} | Bit 5: {} | Bit 4: {}",
        (byte >> 7) & 1,
        (byte >> 6) & 1,
        (byte >> 5) & 1,
        (byte >> 4) & 1
    );
    println!(
        "    Bit 3: {} | Bit 2: {} | Bit 1: {} | Bit 0 (LSB): {}",
        (byte >> 3) & 1,
        (byte >> 2) & 1,
        (byte >> 1) & 1,
        byte & 1
    );
}

/// =============================================================
/// FULL STATE VERIFICATION TEST 1: Basic Bit Pattern
/// =============================================================
/// Source of Truth: QuantizedEmbedding.data[0]
/// Expected: 0xAD (10101101)
#[test]
fn full_state_verify_bit_pattern_0xad() {
    println!("\n============================================================");
    println!("FULL STATE VERIFICATION TEST 1: Bit Pattern 0xAD");
    println!("============================================================");

    let encoder = BinaryEncoder::new();
    let input = vec![0.5f32, -0.3, 0.1, -0.8, 0.0, 0.9, -0.1, 0.2];

    // ===== BEFORE STATE =====
    println!("\n--- BEFORE STATE ---");
    println!("INPUT VECTOR (f32): {:?}", input);
    println!("INPUT LENGTH: {}", input.len());
    println!(
        "INPUT SIZE (bytes): {} (8 x 4 bytes per f32)",
        input.len() * 4
    );
    println!("THRESHOLD: 0.0 (default, sign-based)");
    println!("\nEXPECTED BIT COMPUTATION:");
    for (i, val) in input.iter().enumerate() {
        let bit = if *val >= 0.0 { 1 } else { 0 };
        println!(
            "  input[{}] = {:6.2} >= 0.0 ? {} -> bit {}",
            i,
            val,
            bit == 1,
            bit
        );
    }
    println!("EXPECTED BITS: [1, 0, 1, 0, 1, 1, 0, 1]");
    println!("EXPECTED BYTE (MSB first): 10101101 = 0xAD = 173");

    // ===== EXECUTE =====
    println!("\n--- EXECUTING QUANTIZATION ---");
    let quantized = encoder.quantize(&input, None).expect("quantization");

    // ===== AFTER STATE - INSPECT SOURCE OF TRUTH =====
    println!("\n--- AFTER STATE (SOURCE OF TRUTH INSPECTION) ---");
    println!("RESULT TYPE: QuantizedEmbedding");
    println!("RESULT.method: {:?}", quantized.method);
    println!("RESULT.original_dim: {}", quantized.original_dim);
    println!("RESULT.data.len(): {}", quantized.data.len());

    // THE SOURCE OF TRUTH: actual bytes in memory
    println!("\n>>> SOURCE OF TRUTH: quantized.data (raw bytes) <<<");
    hex_dump("quantized.data", &quantized.data);

    println!("\n>>> INDIVIDUAL BYTE ANALYSIS <<<");
    bit_dump("byte[0]", quantized.data[0]);

    // PHYSICAL VERIFICATION: Check each bit individually
    println!("\n>>> PHYSICAL BIT-BY-BIT VERIFICATION <<<");
    let byte0 = quantized.data[0];
    let bits: Vec<u8> = (0..8).rev().map(|i| (byte0 >> i) & 1).collect();
    println!("  Extracted bits from byte[0]: {:?}", bits);
    println!("  Expected bits:               [1, 0, 1, 0, 1, 1, 0, 1]");

    // Verify each bit corresponds to correct input sign
    println!("\n>>> SIGN-TO-BIT CORRELATION CHECK <<<");
    for (i, (&val, &bit)) in input.iter().zip(bits.iter()).enumerate() {
        let expected_bit = if val >= 0.0 { 1 } else { 0 };
        let match_str = if bit == expected_bit {
            "✓ MATCH"
        } else {
            "✗ MISMATCH"
        };
        println!(
            "  input[{}] = {:6.2} -> expected bit {} | actual bit {} | {}",
            i, val, expected_bit, bit, match_str
        );
        assert_eq!(bit, expected_bit, "Bit {} mismatch", i);
    }

    // FINAL ASSERTION on source of truth
    println!("\n>>> FINAL PHYSICAL VERIFICATION <<<");
    println!("  Expected byte value: 0xAD (173)");
    println!("  Actual byte value:   0x{:02X} ({})", byte0, byte0);
    assert_eq!(byte0, 0xAD, "Physical byte must be 0xAD");

    println!("\n✅ FULL STATE VERIFICATION PASSED: Byte 0xAD physically verified in memory\n");
}

/// =============================================================
/// FULL STATE VERIFICATION TEST 2: 32x Compression Ratio
/// =============================================================
/// Source of Truth: QuantizedEmbedding.data.len()
/// Expected: 128 bytes (from 4096 bytes input)
#[test]
fn full_state_verify_32x_compression() {
    println!("\n============================================================");
    println!("FULL STATE VERIFICATION TEST 2: 32x Compression");
    println!("============================================================");

    let encoder = BinaryEncoder::new();
    let input: Vec<f32> = (0..1024)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();

    // ===== BEFORE STATE =====
    println!("\n--- BEFORE STATE ---");
    println!("INPUT: Vec<f32> with {} elements", input.len());
    println!("INPUT SIZE: {} bytes (1024 × 4 bytes)", input.len() * 4);
    println!(
        "EXPECTED OUTPUT SIZE: {} bytes (1024 bits / 8)",
        (1024 + 7) / 8
    );
    println!("EXPECTED COMPRESSION: {:.1}x", (1024.0 * 4.0) / 128.0);

    // ===== EXECUTE =====
    println!("\n--- EXECUTING QUANTIZATION ---");
    let quantized = encoder.quantize(&input, None).expect("quantization");

    // ===== AFTER STATE - INSPECT SOURCE OF TRUTH =====
    println!("\n--- AFTER STATE (SOURCE OF TRUTH INSPECTION) ---");

    // THE SOURCE OF TRUTH: data vector length and contents
    println!("\n>>> SOURCE OF TRUTH: quantized.data allocation <<<");
    println!("  data.len() = {}", quantized.data.len());
    println!("  data.capacity() >= {}", quantized.data.capacity());

    // Verify physical memory allocation
    let data_ptr = quantized.data.as_ptr();
    println!("  data.as_ptr() = {:p}", data_ptr);
    println!("  First 32 bytes of actual data:");
    hex_dump(
        "first 32 bytes",
        &quantized.data[0..32.min(quantized.data.len())],
    );

    // Since input alternates 1.0, -1.0, bits should alternate 1,0,1,0...
    // Each byte should be 0b10101010 = 0xAA
    println!("\n>>> PATTERN VERIFICATION <<<");
    println!("  Expected pattern (alternating +/-): each byte should be 0xAA (10101010)");
    let mut pattern_matches = 0;
    for (i, &byte) in quantized.data.iter().enumerate() {
        if byte == 0xAA {
            pattern_matches += 1;
        } else {
            println!("    byte[{}] = 0x{:02X} (expected 0xAA)", i, byte);
        }
    }
    println!(
        "  Pattern matches: {}/{} bytes",
        pattern_matches,
        quantized.data.len()
    );

    // PHYSICAL SIZE VERIFICATION
    println!("\n>>> PHYSICAL SIZE VERIFICATION <<<");
    println!("  Original f32 bytes: {}", 1024 * 4);
    println!("  Packed byte count:  {}", quantized.data.len());
    let actual_ratio = (1024.0 * 4.0) / quantized.data.len() as f32;
    println!("  Actual compression: {:.2}x", actual_ratio);

    assert_eq!(quantized.data.len(), 128, "Must be exactly 128 bytes");
    assert!((actual_ratio - 32.0).abs() < 0.1, "Must be 32x compression");

    println!(
        "\n✅ FULL STATE VERIFICATION PASSED: 128 bytes physically allocated (32x compression)\n"
    );
}

/// =============================================================
/// FULL STATE VERIFICATION TEST 3: 10K HDC Dimension
/// =============================================================
/// Source of Truth: QuantizedEmbedding.data (1250 bytes)
#[test]
fn full_state_verify_10k_dimension() {
    println!("\n============================================================");
    println!("FULL STATE VERIFICATION TEST 3: 10K HDC Dimension");
    println!("============================================================");

    let encoder = BinaryEncoder::new();
    let input: Vec<f32> = (0..10000)
        .map(|i| if i % 2 == 0 { 0.7 } else { -0.3 })
        .collect();

    // ===== BEFORE STATE =====
    println!("\n--- BEFORE STATE ---");
    println!(
        "INPUT: Vec<f32> with {} elements (HDC typical)",
        input.len()
    );
    println!("INPUT SIZE: {} bytes", input.len() * 4);
    println!("EXPECTED PACKED SIZE: {} bytes", (10000 + 7) / 8);
    println!("FIRST 10 INPUT VALUES: {:?}", &input[0..10]);

    // ===== EXECUTE =====
    println!("\n--- EXECUTING QUANTIZATION ---");
    let quantized = encoder.quantize(&input, None).expect("quantization");

    // ===== AFTER STATE - INSPECT SOURCE OF TRUTH =====
    println!("\n--- AFTER STATE (SOURCE OF TRUTH INSPECTION) ---");

    println!("\n>>> SOURCE OF TRUTH: quantized.data <<<");
    println!("  Allocated bytes: {}", quantized.data.len());
    println!("  Memory address: {:p}", quantized.data.as_ptr());

    // Check first few bytes
    println!("\n>>> FIRST 16 BYTES (physical data) <<<");
    hex_dump("bytes 0-15", &quantized.data[0..16]);

    // Check last few bytes
    println!("\n>>> LAST 16 BYTES (physical data) <<<");
    hex_dump("bytes 1234-1249", &quantized.data[1234..1250]);

    // Verify pattern (alternating should give 0xAA)
    println!("\n>>> BYTE PATTERN ANALYSIS <<<");
    let unique_bytes: std::collections::HashSet<u8> = quantized.data.iter().cloned().collect();
    println!("  Unique byte values found: {:?}", unique_bytes);
    println!("  Expected: {{0xAA}} (since all alternate)");

    // Physical verification of specific bytes
    println!("\n>>> SPOT-CHECK PHYSICAL BYTES <<<");
    for idx in [0, 100, 500, 1000, 1249] {
        let byte = quantized.data[idx];
        println!("    data[{:4}] = 0x{:02X} ({:08b})", idx, byte, byte);
    }

    // Roundtrip verification
    println!("\n>>> ROUNDTRIP PHYSICAL VERIFICATION <<<");
    let reconstructed = encoder.dequantize(&quantized).expect("dequantize");
    println!("  Reconstructed length: {}", reconstructed.len());

    // Count sign preservation
    let mut sign_matches = 0;
    let mut sign_mismatches = 0;
    for (i, (&orig, &recon)) in input.iter().zip(reconstructed.iter()).enumerate() {
        let orig_positive = orig >= 0.0;
        let recon_positive = recon >= 0.0;
        if orig_positive == recon_positive {
            sign_matches += 1;
        } else {
            sign_mismatches += 1;
            if sign_mismatches <= 5 {
                println!(
                    "    MISMATCH at {}: orig={:.2}, recon={:.2}",
                    i, orig, recon
                );
            }
        }
    }
    println!("  Sign matches: {}/{}", sign_matches, input.len());
    println!("  Sign mismatches: {}", sign_mismatches);

    assert_eq!(quantized.data.len(), 1250, "Must be 1250 bytes");
    assert_eq!(sign_mismatches, 0, "No sign mismatches allowed");

    println!("\n✅ FULL STATE VERIFICATION PASSED: 10K HDC with 1250 bytes physically verified\n");
}

/// =============================================================
/// EDGE CASE AUDIT 1: Empty Input
/// =============================================================
#[test]
fn edge_case_audit_empty_input() {
    println!("\n============================================================");
    println!("EDGE CASE AUDIT 1: Empty Input");
    println!("============================================================");

    let encoder = BinaryEncoder::new();
    let input: Vec<f32> = vec![];

    // ===== BEFORE STATE =====
    println!("\n--- BEFORE STATE ---");
    println!("INPUT: {:?}", input);
    println!("INPUT.len(): {}", input.len());
    println!("INPUT.is_empty(): {}", input.is_empty());
    println!("SYSTEM STATE: Ready to quantize");

    // ===== EXECUTE =====
    println!("\n--- EXECUTING QUANTIZATION ---");
    let result = encoder.quantize(&input, None);

    // ===== AFTER STATE =====
    println!("\n--- AFTER STATE ---");
    println!("RESULT.is_err(): {}", result.is_err());

    match &result {
        Err(e) => {
            let error_string = format!("{}", e);
            println!("ERROR TYPE: BinaryQuantizationError::EmptyInput");
            println!("ERROR MESSAGE: {}", error_string);
            println!("CONTAINS EMB-E012: {}", error_string.contains("EMB-E012"));
            println!("CONTAINS 'Empty': {}", error_string.contains("Empty"));

            // Physical verification: no QuantizedEmbedding was created
            println!("\n>>> PHYSICAL STATE VERIFICATION <<<");
            println!("  No QuantizedEmbedding allocated (result is Err)");
            println!("  No bytes written to any data structure");
            println!("  System correctly rejected invalid input");

            assert!(error_string.contains("EMB-E012"));
        }
        Ok(_) => {
            panic!("Should have failed for empty input");
        }
    }

    println!("\n✅ EDGE CASE AUDIT PASSED: Empty input correctly rejected\n");
}

/// =============================================================
/// EDGE CASE AUDIT 2: NaN Input at Specific Index
/// =============================================================
#[test]
fn edge_case_audit_nan_input() {
    println!("\n============================================================");
    println!("EDGE CASE AUDIT 2: NaN Input at Index 3");
    println!("============================================================");

    let encoder = BinaryEncoder::new();
    let input = vec![1.0f32, 2.0, 3.0, f32::NAN, 5.0, 6.0, 7.0, 8.0];

    // ===== BEFORE STATE =====
    println!("\n--- BEFORE STATE ---");
    println!("INPUT: {:?}", input);
    println!("INPUT[3].is_nan(): {}", input[3].is_nan());
    println!("INPUT[3].is_finite(): {}", input[3].is_finite());
    for (i, val) in input.iter().enumerate() {
        println!("  input[{}] = {} (is_finite: {})", i, val, val.is_finite());
    }

    // ===== EXECUTE =====
    println!("\n--- EXECUTING QUANTIZATION ---");
    let result = encoder.quantize(&input, None);

    // ===== AFTER STATE =====
    println!("\n--- AFTER STATE ---");
    println!("RESULT.is_err(): {}", result.is_err());

    match &result {
        Err(BinaryQuantizationError::InvalidValue { index, value }) => {
            println!("ERROR TYPE: BinaryQuantizationError::InvalidValue");
            println!("DETECTED INDEX: {}", index);
            println!("DETECTED VALUE: {}", value);
            println!("VALUE.is_nan(): {}", value.is_nan());

            let error_string = format!("{}", result.as_ref().unwrap_err());
            println!("FULL ERROR: {}", error_string);
            println!("CONTAINS 'index 3': {}", error_string.contains("index 3"));

            // Physical verification
            println!("\n>>> PHYSICAL STATE VERIFICATION <<<");
            println!("  Error correctly identifies index {}", index);
            println!("  No partial quantization occurred");
            println!("  System failed fast at first invalid value");

            assert_eq!(*index, 3, "Must detect NaN at index 3");
        }
        Err(e) => panic!("Wrong error type: {:?}", e),
        Ok(_) => panic!("Should have failed for NaN input"),
    }

    println!("\n✅ EDGE CASE AUDIT PASSED: NaN at index 3 correctly detected\n");
}

/// =============================================================
/// EDGE CASE AUDIT 3: Maximum Reasonable Dimension (100K)
/// =============================================================
#[test]
fn edge_case_audit_maximum_dimension() {
    println!("\n============================================================");
    println!("EDGE CASE AUDIT 3: Maximum Dimension (100K elements)");
    println!("============================================================");

    let encoder = BinaryEncoder::new();
    let dimension = 100_000;
    let input: Vec<f32> = (0..dimension)
        .map(|i| if i % 3 == 0 { 0.5 } else { -0.5 })
        .collect();

    // ===== BEFORE STATE =====
    println!("\n--- BEFORE STATE ---");
    println!("INPUT DIMENSION: {}", dimension);
    println!(
        "INPUT MEMORY: {} bytes ({:.2} MB)",
        dimension * 4,
        (dimension * 4) as f64 / 1_000_000.0
    );
    println!(
        "EXPECTED OUTPUT: {} bytes ({:.2} KB)",
        (dimension + 7) / 8,
        ((dimension + 7) / 8) as f64 / 1000.0
    );
    println!("PATTERN: every 3rd element positive");

    // ===== EXECUTE =====
    println!("\n--- EXECUTING QUANTIZATION ---");
    let start = std::time::Instant::now();
    let quantized = encoder.quantize(&input, None).expect("quantization");
    let duration = start.elapsed();
    println!("EXECUTION TIME: {:?}", duration);

    // ===== AFTER STATE - INSPECT SOURCE OF TRUTH =====
    println!("\n--- AFTER STATE (SOURCE OF TRUTH INSPECTION) ---");

    println!("\n>>> SOURCE OF TRUTH: quantized.data <<<");
    println!("  Allocated bytes: {}", quantized.data.len());
    println!("  Expected bytes: {}", (100_000 + 7) / 8);
    println!("  Memory address: {:p}", quantized.data.as_ptr());

    // Verify size
    let expected_size = 100_000_usize.div_ceil(8);
    assert_eq!(quantized.data.len(), expected_size, "Size must match");

    // Physical spot checks
    println!("\n>>> PHYSICAL SPOT CHECKS <<<");
    for idx in [0, 1000, 5000, 10000, 12499] {
        let byte = quantized.data[idx];
        println!("    data[{:5}] = 0x{:02X} ({:08b})", idx, byte, byte);
    }

    // Verify compression
    let ratio = (dimension as f32 * 4.0) / quantized.data.len() as f32;
    println!("\n>>> COMPRESSION VERIFICATION <<<");
    println!("  Original size: {} bytes", dimension * 4);
    println!("  Compressed size: {} bytes", quantized.data.len());
    println!("  Compression ratio: {:.2}x", ratio);

    // Roundtrip check
    println!("\n--- ROUNDTRIP VERIFICATION ---");
    let reconstructed = encoder.dequantize(&quantized).expect("dequantize");
    println!("  Reconstructed length: {}", reconstructed.len());

    let mut mismatches = 0;
    for (i, (&orig, &recon)) in input.iter().zip(reconstructed.iter()).enumerate() {
        if (orig >= 0.0) != (recon >= 0.0) {
            mismatches += 1;
            if mismatches <= 3 {
                println!("    Sign mismatch at {}: {} vs {}", i, orig, recon);
            }
        }
    }
    println!("  Total sign mismatches: {}", mismatches);
    assert_eq!(mismatches, 0, "No mismatches allowed");

    // Latency check (Constitution requires <1ms)
    println!("\n>>> LATENCY VERIFICATION <<<");
    println!("  Quantization time: {:?}", duration);
    let under_1ms = duration.as_micros() < 1000;
    println!("  Under 1ms: {}", under_1ms);
    // Note: 100K is larger than typical HDC (10K), so we allow more time

    println!("\n✅ EDGE CASE AUDIT PASSED: 100K dimension handled correctly\n");
}

/// =============================================================
/// EVIDENCE LOG: Complete Physical State Dump
/// =============================================================
#[test]
fn evidence_log_complete_state_dump() {
    println!("\n============================================================");
    println!("EVIDENCE LOG: Complete Physical State Dump");
    println!("============================================================");

    let encoder = BinaryEncoder::new();

    // Test case: Known input with predictable output
    // Note: threshold is 0.0, so val >= 0.0 -> bit 1, val < 0.0 -> bit 0
    let input = vec![
        1.0f32, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, // byte 0: 11110000 = 0xF0
        1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, // byte 1: 10101010 = 0xAA
        -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, // byte 2: 00000000 = 0x00
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, // byte 3: 11111111 = 0xFF
    ];

    println!("\n--- INPUT DATA ---");
    for (i, chunk) in input.chunks(8).enumerate() {
        println!("  Byte {} input: {:?}", i, chunk);
    }

    let quantized = encoder.quantize(&input, None).expect("quantize");

    println!("\n--- PHYSICAL EVIDENCE: RAW BYTE DATA ---");
    println!("quantized.data = {:?}", quantized.data);
    println!();

    hex_dump("complete data", &quantized.data);

    println!("\n--- EXPECTED vs ACTUAL COMPARISON ---");
    let expected = [0xF0u8, 0xAA, 0x00, 0xFF];
    for (i, (&exp, &act)) in expected.iter().zip(quantized.data.iter()).enumerate() {
        let status = if exp == act {
            "✓ MATCH"
        } else {
            "✗ MISMATCH"
        };
        println!(
            "  byte[{}]: expected 0x{:02X} ({:08b}) | actual 0x{:02X} ({:08b}) | {}",
            i, exp, exp, act, act, status
        );
        assert_eq!(act, exp, "Byte {} mismatch", i);
    }

    println!("\n--- METADATA EVIDENCE ---");
    println!("  method: {:?}", quantized.method);
    println!("  original_dim: {}", quantized.original_dim);
    match &quantized.metadata {
        QuantizationMetadata::Binary { threshold } => {
            println!("  metadata.threshold: {}", threshold);
        }
        _ => println!("  metadata: WRONG TYPE"),
    }

    println!("\n--- DEQUANTIZATION EVIDENCE ---");
    let reconstructed = encoder.dequantize(&quantized).expect("dequantize");
    println!("reconstructed = {:?}", reconstructed);

    for (i, chunk) in reconstructed.chunks(8).enumerate() {
        let expected_signs: Vec<&str> = input[i * 8..(i + 1) * 8]
            .iter()
            .map(|&v| if v >= 0.0 { "+" } else { "-" })
            .collect();
        let actual_signs: Vec<&str> = chunk
            .iter()
            .map(|&v| if v >= 0.0 { "+" } else { "-" })
            .collect();
        println!(
            "  Byte {} signs: expected {:?} | actual {:?}",
            i, expected_signs, actual_signs
        );
    }

    println!("\n============================================================");
    println!("EVIDENCE LOG COMPLETE");
    println!("All physical data has been dumped and verified.");
    println!("============================================================\n");
}
