//! Tests for PQ-8 codebook persistence (save/load).
//!
//! Tests for:
//! - Codebook save and load
//! - File format validation
//! - Error handling for invalid files

use crate::quantization::pq8::encoder::PQ8Encoder;
use crate::quantization::pq8::training::generate_realistic_embeddings;
use crate::quantization::pq8::types::PQ8QuantizationError;
use crate::quantization::types::PQ8Codebook;
use std::path::Path;
use std::sync::Arc;

#[test]
fn test_codebook_save_and_load() {
    // Train a codebook
    let samples = generate_realistic_embeddings(300, 256, 42);
    let original = PQ8Codebook::train(&samples, None).expect("training");

    // Save to temp file
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let path = temp_dir.path().join("test_pq8_codebook.bin");
    original.save(&path).expect("save should succeed");

    // Verify file exists
    assert!(path.exists(), "Codebook file should exist after save");

    // Load it back
    let loaded = PQ8Codebook::load(&path).expect("load should succeed");

    // Verify all fields match
    assert_eq!(loaded.embedding_dim, original.embedding_dim);
    assert_eq!(loaded.num_subvectors, original.num_subvectors);
    assert_eq!(loaded.num_centroids, original.num_centroids);
    assert_eq!(loaded.codebook_id, original.codebook_id);
    assert_eq!(loaded.centroids.len(), original.centroids.len());

    // Verify centroid values match exactly
    for (sv_idx, (orig_sv, load_sv)) in original
        .centroids
        .iter()
        .zip(loaded.centroids.iter())
        .enumerate()
    {
        for (c_idx, (orig_c, load_c)) in orig_sv.iter().zip(load_sv.iter()).enumerate() {
            for (d_idx, (&orig_v, &load_v)) in orig_c.iter().zip(load_c.iter()).enumerate() {
                assert!(
                    (orig_v - load_v).abs() < 1e-7,
                    "Centroid mismatch at sv={}, c={}, d={}: {} vs {}",
                    sv_idx,
                    c_idx,
                    d_idx,
                    orig_v,
                    load_v
                );
            }
        }
    }
}

#[test]
fn test_codebook_load_invalid_magic() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let path = temp_dir.path().join("test_invalid_magic.bin");
    std::fs::write(&path, b"INVL12345678").expect("write");

    let result = PQ8Codebook::load(&path);
    assert!(result.is_err());
    match result.unwrap_err() {
        PQ8QuantizationError::InvalidCodebookFormat { message } => {
            assert!(message.contains("magic"));
        }
        e => panic!("Expected InvalidCodebookFormat, got {:?}", e),
    }
}

#[test]
fn test_codebook_load_nonexistent_file() {
    let path = Path::new("/nonexistent/path/to/codebook.bin");
    let result = PQ8Codebook::load(path);

    assert!(result.is_err());
    match result.unwrap_err() {
        PQ8QuantizationError::IoError { message } => {
            assert!(message.contains("Failed to open"));
        }
        e => panic!("Expected IoError, got {:?}", e),
    }
}

#[test]
fn test_loaded_codebook_produces_same_quantization() {
    // Train and save
    let samples = generate_realistic_embeddings(300, 256, 42);
    let original = PQ8Codebook::train(&samples, None).expect("training");

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let path = temp_dir.path().join("test_quantization_match.bin");
    original.save(&path).expect("save");

    // Create encoders
    let original_encoder = PQ8Encoder::with_codebook(Arc::new(original));
    let loaded = PQ8Codebook::load(&path).expect("load");
    let loaded_encoder = PQ8Encoder::with_codebook(Arc::new(loaded));

    // Test with same input
    let test_embedding = generate_realistic_embeddings(1, 256, 99999).remove(0);

    let q1 = original_encoder.quantize(&test_embedding).expect("q1");
    let q2 = loaded_encoder.quantize(&test_embedding).expect("q2");

    // Quantized bytes should be identical
    assert_eq!(q1.data, q2.data, "Quantized data should match");

    // Dequantized values should be identical
    let d1 = original_encoder.dequantize(&q1).expect("d1");
    let d2 = loaded_encoder.dequantize(&q2).expect("d2");

    for (i, (&v1, &v2)) in d1.iter().zip(d2.iter()).enumerate() {
        assert!(
            (v1 - v2).abs() < 1e-7,
            "Dequantized value mismatch at {}: {} vs {}",
            i,
            v1,
            v2
        );
    }
}
