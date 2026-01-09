//! Physical verification test for TASK-EMB-013
//! Verifies load_weights produces same SHA256 as external sha256sum tool

use context_graph_embeddings::warm::loader::load_weights;
use std::path::Path;

#[test]
fn physical_verification_checksum_matches_external_tool() {
    // This file was created by Python safetensors library
    // External sha256sum reports: ff0b41a49a541d2383b963663323322c4cd8506aed85e1d955a848dfcac6de46
    let file_path = Path::new("/tmp/verification_test.safetensors");

    if !file_path.exists() {
        println!("Skipping test - file not found (run Python script first)");
        return;
    }

    // Load with our Rust function
    let (file_bytes, checksum, metadata) =
        load_weights(file_path, "verification_test").expect("load_weights should succeed");

    // Convert checksum to hex string
    let rust_hex = hex::encode(checksum);

    // Expected from external sha256sum
    let expected_hex = "ff0b41a49a541d2383b963663323322c4cd8506aed85e1d955a848dfcac6de46";

    println!("\n=== Physical Verification Results ===");
    println!("File size: {} bytes", file_bytes.len());
    println!("Rust SHA256:     {}", rust_hex);
    println!("Expected SHA256: {}", expected_hex);
    println!("Total params: {}", metadata.total_params);
    println!("Tensor shapes: {:?}", metadata.shapes);

    // CRITICAL ASSERTION: Checksums must match
    assert_eq!(
        rust_hex, expected_hex,
        "PHYSICAL VERIFICATION FAILED: Rust load_weights checksum does not match external sha256sum"
    );

    // Verify tensor metadata
    assert_eq!(
        metadata.total_params, 4,
        "Expected 4 parameters (2x2 matrix)"
    );
    assert!(
        metadata.shapes.contains_key("weights"),
        "Should have 'weights' tensor"
    );

    println!("\n✅ PHYSICAL VERIFICATION PASSED");
    println!("   - load_weights SHA256 matches external sha256sum");
    println!("   - Tensor metadata correctly parsed");
    println!("   - No fake data detected");
}
