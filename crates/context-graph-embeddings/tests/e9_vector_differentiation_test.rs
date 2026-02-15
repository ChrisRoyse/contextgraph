//! CRITICAL DIAGNOSTIC TEST: Verify E9 vectors differ from E1 vectors in fingerprint
//!
//! This test verifies that after calling embed_all(), the SemanticFingerprint
//! has DIFFERENT vectors for e1_semantic and e9_hdc. If they are identical,
//! the entire 13-embedder system is broken.
//!
//! Run with: cargo test --package context-graph-embeddings e9_vector_differentiation -- --nocapture

use context_graph_core::traits::MultiArrayEmbeddingProvider;
use context_graph_embeddings::config::GpuConfig;
use context_graph_embeddings::provider::ProductionMultiArrayProvider;
use std::path::PathBuf;

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn vectors_are_identical(a: &[f32], b: &[f32]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-9)
}

/// CRITICAL: Verify E1 and E9 fingerprint vectors are different
#[tokio::test]
#[ignore = "requires models directory with pretrained weights"]
async fn test_e1_e9_vectors_differ_in_fingerprint() {
    let models_dir = PathBuf::from(
        std::env::var("MODELS_DIR").unwrap_or_else(|_| "./models".to_string()),
    );

    let provider = ProductionMultiArrayProvider::new(models_dir, GpuConfig::default())
        .await
        .expect("Failed to create provider");

    let test_content = "authentication failed for user";
    let output = provider.embed_all(test_content).await.expect("embed_all() failed");
    let fingerprint = output.fingerprint;

    let e1 = &fingerprint.e1_semantic;
    let e9 = &fingerprint.e9_hdc;

    let identical = vectors_are_identical(e1, e9);
    let similarity = if e1.len() == e9.len() {
        cosine_similarity(e1, e9)
    } else {
        0.0
    };

    assert!(!identical, "CRITICAL: E1 and E9 vectors must NOT be identical!");
    assert!(similarity < 0.95, "E1 and E9 should not have >0.95 similarity, got {}", similarity);
}

/// Verify E5 vector is also different from E1
#[tokio::test]
#[ignore = "requires models directory with pretrained weights"]
async fn test_e1_e5_vectors_differ_in_fingerprint() {
    let models_dir = PathBuf::from(
        std::env::var("MODELS_DIR").unwrap_or_else(|_| "./models".to_string()),
    );

    let provider = ProductionMultiArrayProvider::new(models_dir, GpuConfig::default())
        .await
        .expect("Failed to create provider");

    let output = provider.embed_all("test content for verification").await
        .expect("embed_all() failed");
    let fingerprint = output.fingerprint;

    let e1 = &fingerprint.e1_semantic;
    let e5 = fingerprint.e5_active_vector();

    // Verify dimensions are correct
    assert_eq!(e1.len(), 1024, "E1 should be 1024D");
    assert_eq!(e5.len(), 768, "E5 should be 768D");
}
