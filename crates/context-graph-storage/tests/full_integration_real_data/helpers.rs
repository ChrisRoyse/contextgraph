//! Test Utilities - REAL Data Generation (NO MOCKS)

use std::collections::HashSet;

use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use rand::Rng;
use uuid::Uuid;

/// Create a RocksDbTeleologicalStore with initialized HNSW indexes.
/// Note: EmbedderIndexRegistry is initialized in the constructor,
/// so no separate initialization step is needed.
pub fn create_initialized_store(path: &std::path::Path) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open(path).expect("Failed to open store")
}

/// Generate a REAL random unit vector of specified dimension.
/// All vectors are normalized to have L2 norm = 1.0.
pub fn generate_real_unit_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

    // Normalize to unit vector
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in &mut vec {
            *v /= norm;
        }
    }

    vec
}

/// Generate a REAL SparseVector with realistic sparsity (~5% active).
pub fn generate_real_sparse_vector(target_nnz: usize) -> SparseVector {
    let mut rng = rand::thread_rng();

    // Generate unique sorted indices
    let mut indices_set: HashSet<u16> = HashSet::new();
    while indices_set.len() < target_nnz {
        indices_set.insert(rng.gen_range(0..30522));
    }
    let mut indices: Vec<u16> = indices_set.into_iter().collect();
    indices.sort();

    // Generate random positive values (SPLADE scores are positive)
    let values: Vec<f32> = (0..target_nnz).map(|_| rng.gen_range(0.1..2.0)).collect();

    SparseVector::new(indices, values).expect("Failed to create sparse vector")
}

/// Generate a REAL SemanticFingerprint with proper dimensions.
/// E1: 1024D, E2-E4: 512D, E5: 768D (dual: cause+effect), E6: sparse, E7: 1536D, E8: 384D,
/// E9: 1024D (projected), E10: 768D, E11: 384D, E12: 128D tokens, E13: sparse
pub fn generate_real_semantic_fingerprint() -> SemanticFingerprint {
    let e5_vec = generate_real_unit_vector(768);
    SemanticFingerprint {
        e1_semantic: generate_real_unit_vector(1024),
        e2_temporal_recent: generate_real_unit_vector(512),
        e3_temporal_periodic: generate_real_unit_vector(512),
        e4_temporal_positional: generate_real_unit_vector(512),
        e5_causal_as_cause: e5_vec.clone(),
        e5_causal_as_effect: e5_vec,
        e5_causal: Vec::new(), // Empty - using new dual format
        e6_sparse: generate_real_sparse_vector(100), // ~0.3% sparsity for E6
        e7_code: generate_real_unit_vector(1536),
        e8_graph_as_source: generate_real_unit_vector(384),
        e8_graph_as_target: generate_real_unit_vector(384),
        e8_graph: Vec::new(), // Legacy field, empty by default
        e9_hdc: generate_real_unit_vector(1024), // HDC projected dimension
        e10_multimodal: generate_real_unit_vector(768),
        e11_entity: generate_real_unit_vector(384),
        e12_late_interaction: vec![generate_real_unit_vector(128); 32], // 32 tokens
        e13_splade: generate_real_sparse_vector(150),                   // ~0.5% sparsity for E13
    }
}

/// Generate a REAL content hash (SHA-256 simulation).
pub fn generate_real_content_hash() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let mut hash = [0u8; 32];
    rng.fill(&mut hash);
    hash
}

/// Create a REAL TeleologicalFingerprint with all real data.
/// NO MOCK DATA - all vectors have correct dimensions and valid values.
pub fn create_real_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        generate_real_semantic_fingerprint(),
        generate_real_content_hash(),
    )
}

/// Create a REAL TeleologicalFingerprint with a specific ID.
pub fn create_real_fingerprint_with_id(id: Uuid) -> TeleologicalFingerprint {
    TeleologicalFingerprint::with_id(
        id,
        generate_real_semantic_fingerprint(),
        generate_real_content_hash(),
    )
}
