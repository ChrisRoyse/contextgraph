//! REAL data generation for semantic and teleological fingerprints.
//!
//! All vectors have correct dimensions and valid values — NO MOCK DATA.
//! Dimensions match the 13-embedder architecture:
//!   E1: 1024D, E2-E4: 512D, E5: 768D (dual: cause+effect),
//!   E6: sparse 30K, E7: 1536D, E8: 1024D (dual: source+target),
//!   E9: 1024D (HDC), E10: 768D (dual: paraphrase+context),
//!   E11: 768D (KEPLER), E12: 128D/token (ColBERT), E13: sparse 30K (SPLADE)

use std::collections::HashSet;

use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use rand::Rng;
use uuid::Uuid;

/// Generate a random unit vector of `dim` dimensions (L2 norm = 1.0).
pub fn generate_real_unit_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}

/// Generate a sparse vector with `target_nnz` non-zero entries.
///
/// Indices are drawn uniformly from 0..30522 (BERT vocab size).
/// Values are positive (0.1..2.0), matching SPLADE/keyword score ranges.
pub fn generate_real_sparse_vector(target_nnz: usize) -> SparseVector {
    let mut rng = rand::thread_rng();
    let mut indices_set: HashSet<u16> = HashSet::new();
    while indices_set.len() < target_nnz {
        indices_set.insert(rng.gen_range(0..30522));
    }
    let mut indices: Vec<u16> = indices_set.into_iter().collect();
    indices.sort();
    let values: Vec<f32> = (0..target_nnz).map(|_| rng.gen_range(0.1..2.0)).collect();
    SparseVector::new(indices, values).expect("Failed to create sparse vector")
}

/// Generate a complete `SemanticFingerprint` with correct dimensions for all 13 embedders.
pub fn generate_real_semantic_fingerprint() -> SemanticFingerprint {
    let e5_vec = generate_real_unit_vector(768);
    SemanticFingerprint {
        e1_semantic: generate_real_unit_vector(1024),
        e2_temporal_recent: generate_real_unit_vector(512),
        e3_temporal_periodic: generate_real_unit_vector(512),
        e4_temporal_positional: generate_real_unit_vector(512),
        e5_causal_as_cause: e5_vec.clone(),
        e5_causal_as_effect: e5_vec,
        e5_causal: Vec::new(),
        e6_sparse: generate_real_sparse_vector(100),
        e7_code: generate_real_unit_vector(1536),
        e8_graph_as_source: generate_real_unit_vector(1024),
        e8_graph_as_target: generate_real_unit_vector(1024),
        e8_graph: Vec::new(),
        e9_hdc: generate_real_unit_vector(1024),
        e10_multimodal_paraphrase: generate_real_unit_vector(768),
        e10_multimodal_as_context: generate_real_unit_vector(768),
        e11_entity: generate_real_unit_vector(768),
        e12_late_interaction: vec![generate_real_unit_vector(128); 16],
        e13_splade: generate_real_sparse_vector(500),
    }
}

/// Generate a random 32-byte content hash.
pub fn generate_real_content_hash() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let mut hash = [0u8; 32];
    rng.fill(&mut hash);
    hash
}

/// Create a `TeleologicalFingerprint` with a new random ID.
pub fn create_real_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(generate_real_semantic_fingerprint(), generate_real_content_hash())
}

/// Create a `TeleologicalFingerprint` with a specific UUID.
pub fn create_real_fingerprint_with_id(id: Uuid) -> TeleologicalFingerprint {
    TeleologicalFingerprint::with_id(id, generate_real_semantic_fingerprint(), generate_real_content_hash())
}

/// Create a `TeleologicalFingerprint` with a random ID and real content hash.
///
/// The `_id` parameter is ignored — `TeleologicalFingerprint::new()` generates
/// its own UUID. Kept for backward compatibility with callers that pass a UUID.
pub fn generate_real_teleological_fingerprint(_id: Uuid) -> TeleologicalFingerprint {
    create_real_fingerprint()
}

/// Format bytes as hex string (limited to first 64 bytes for display).
pub fn hex_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(64)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
