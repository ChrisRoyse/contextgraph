//! CRITICAL TEST: Verify E9 vectors differ from E1 after storage roundtrip
//!
//! This test verifies that storing and retrieving a fingerprint preserves
//! the uniqueness of E1 and E9 vectors.

use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_storage::teleological::{
    deserialize_teleological_fingerprint, serialize_teleological_fingerprint,
};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use chrono::Utc;
use uuid::Uuid;

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
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

/// Create a test fingerprint with DISTINCT vectors for E1 and E9
fn create_test_fingerprint_with_unique_vectors() -> TeleologicalFingerprint {
    // E1: First 10 values are 0.1, 0.2, 0.3... (1024D)
    let mut e1 = vec![0.0f32; 1024];
    for (i, v) in e1.iter_mut().enumerate() {
        *v = ((i % 10) as f32 + 1.0) * 0.1;
    }
    // Normalize
    let norm1: f32 = e1.iter().map(|x| x * x).sum::<f32>().sqrt();
    for v in &mut e1 {
        *v /= norm1;
    }

    // E9: First 10 values are 0.9, 0.8, 0.7... (different pattern, 1024D)
    let mut e9 = vec![0.0f32; 1024];
    for (i, v) in e9.iter_mut().enumerate() {
        *v = (10.0 - (i % 10) as f32) * 0.1;
    }
    // Normalize
    let norm9: f32 = e9.iter().map(|x| x * x).sum::<f32>().sqrt();
    for v in &mut e9 {
        *v /= norm9;
    }

    // Verify they're different before storing
    let pre_sim = cosine_similarity(&e1, &e9);
    println!("Pre-storage E1/E9 cosine similarity: {:.6}", pre_sim);
    assert!(pre_sim < 0.9, "E1 and E9 must start as different vectors");

    // Create fingerprint with unique vectors
    let semantic = SemanticFingerprint {
        e1_semantic: e1.clone(),
        e2_temporal_recent: vec![0.0; 512],
        e3_temporal_periodic: vec![0.0; 512],
        e4_temporal_positional: vec![0.0; 512],
        e5_causal_as_cause: vec![0.0; 768],
        e5_causal_as_effect: vec![0.0; 768],
        e5_causal: Vec::new(),
        e6_sparse: context_graph_core::types::fingerprint::SparseVector::empty(),
        e7_code: vec![0.0; 1536],
        e8_graph_as_source: vec![0.0; 1024],
        e8_graph_as_target: vec![0.0; 1024],
        e8_graph: Vec::new(),
        e9_hdc: e9.clone(),
        e10_multimodal_as_intent: vec![0.0; 768],
        e10_multimodal_as_context: vec![0.0; 768],
        e10_multimodal: Vec::new(),
        e11_entity: vec![0.0; 768],
        e12_late_interaction: Vec::new(),
        e13_splade: context_graph_core::types::fingerprint::SparseVector::empty(),
    };

    TeleologicalFingerprint {
        id: Uuid::new_v4(),
        semantic,
        content_hash: [0u8; 32],
        created_at: Utc::now(),
        last_updated: Utc::now(),
        access_count: 0,
        importance: 0.5,
        e6_sparse: None,
    }
}

/// CRITICAL: Test that E1 and E9 remain different after serialization roundtrip
#[test]
fn test_e1_e9_preserved_after_serialization() {
    println!("\n========================================");
    println!("CRITICAL: E1/E9 Serialization Roundtrip");
    println!("========================================\n");

    // Create fingerprint with unique E1 and E9
    let original = create_test_fingerprint_with_unique_vectors();

    println!("Original E1 first 5 values: {:?}", &original.semantic.e1_semantic[..5]);
    println!("Original E9 first 5 values: {:?}", &original.semantic.e9_hdc[..5]);

    // Serialize
    let bytes = serialize_teleological_fingerprint(&original);
    println!("\nSerialized to {} bytes", bytes.len());

    // Deserialize
    let restored = deserialize_teleological_fingerprint(&bytes);

    println!("Restored E1 first 5 values: {:?}", &restored.semantic.e1_semantic[..5]);
    println!("Restored E9 first 5 values: {:?}", &restored.semantic.e9_hdc[..5]);

    // Verify E1 was preserved exactly
    assert_eq!(
        original.semantic.e1_semantic.len(),
        restored.semantic.e1_semantic.len(),
        "E1 dimension mismatch"
    );
    let e1_identical = vectors_are_identical(&original.semantic.e1_semantic, &restored.semantic.e1_semantic);
    assert!(e1_identical, "E1 vectors should be identical after roundtrip");
    println!("✓ E1 preserved exactly");

    // Verify E9 was preserved exactly
    assert_eq!(
        original.semantic.e9_hdc.len(),
        restored.semantic.e9_hdc.len(),
        "E9 dimension mismatch"
    );
    let e9_identical = vectors_are_identical(&original.semantic.e9_hdc, &restored.semantic.e9_hdc);
    assert!(e9_identical, "E9 vectors should be identical after roundtrip");
    println!("✓ E9 preserved exactly");

    // CRITICAL: Verify E1 and E9 are STILL different after roundtrip
    let e1_e9_identical = vectors_are_identical(&restored.semantic.e1_semantic, &restored.semantic.e9_hdc);
    let restored_sim = cosine_similarity(&restored.semantic.e1_semantic, &restored.semantic.e9_hdc);

    println!("\n========================================");
    println!("RESULTS:");
    println!("  E1 ↔ E9 identical after roundtrip: {}", e1_e9_identical);
    println!("  E1 ↔ E9 cosine similarity: {:.6}", restored_sim);

    if e1_e9_identical {
        println!("\n❌ CRITICAL BUG: E1 and E9 became identical after serialization!");
        println!("   The serialization is CORRUPTING the vectors!");
    } else {
        println!("\n✓ PASS: E1 and E9 remain different after roundtrip");
    }
    println!("========================================\n");

    assert!(!e1_e9_identical, "E1 and E9 must remain different after serialization");
}
