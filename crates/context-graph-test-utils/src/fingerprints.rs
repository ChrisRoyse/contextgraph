//! Seeded pseudo-random test data for semantic and teleological fingerprints.
//!
//! # M-H2 (GH #485, 2026-05-19): truth-in-advertising rename + determinism
//!
//! Previously named `generate_real_*` / `create_real_*`, these helpers produce
//! Rust `Vec<f32>` / `SparseVector` / `TeleologicalFingerprint` values with
//! the correct **shape** (dimensions, non-zero L2 norm, valid sparse index
//! range) but **uniform pseudo-random content** — NOT a real embedder output.
//! The "_real_" prefix was misleading and has been removed.
//!
//! Determinism: all generators internally use a pure-Rust SplitMix64 PRNG
//! seeded from `DEFAULT_TEST_SEED` (default helpers) or a caller-supplied seed
//! (`*_with_seed` variants). Re-running the same test produces byte-identical
//! fingerprints, satisfying FSV-PROTOCOL §4 "deterministic" property.
//!
//! Dimensions match the 13-embedder architecture (slot identity is doctrine):
//!   E1: 1024D, E2-E4: 512D, E5: 768D (dual: cause+effect),
//!   E6: sparse 30K, E7: 1536D, E8: 1024D (dual: source+target),
//!   E9: 1024D (HDC), E10: 768D (dual: paraphrase+context),
//!   E11: 768D (KEPLER), E12: 128D/token (ColBERT), E13: sparse 30K (SPLADE),
//!   E14: 1024D (BGE-M3 dense)

use std::collections::BTreeSet;

use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use uuid::Uuid;

/// Default seed used by the parameter-less helpers.
///
/// Chosen as a recognizable non-zero constant. Tests that need distinct
/// fingerprints should call the `*_with_seed` variant with different seeds.
pub const DEFAULT_TEST_SEED: u64 = 0xC0FFEE_BADC0DE_u64;

// =============================================================================
// SplitMix64 PRNG (pure Rust, deterministic, no external dependency)
// =============================================================================
//
// Matches the SplitMix64 used in `context-graph-tier-compression/src/tests.rs`
// and `context-graph-mejepa-corpus` for cross-crate consistency.

/// Advance a SplitMix64 state and return the next 64-bit sample.
fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Map a SplitMix64 output to a uniform f32 in `[-1.0, 1.0)`.
fn next_f32_pm1(state: &mut u64) -> f32 {
    let raw = splitmix64_next(state);
    let mantissa = (raw >> 40) as u32; // 24 bits
    let u01 = (mantissa as f32) / ((1u32 << 24) as f32);
    u01.mul_add(2.0, -1.0)
}

/// Map a SplitMix64 output to a uniform f32 in `[lo, hi)`.
fn next_f32_in(state: &mut u64, lo: f32, hi: f32) -> f32 {
    let raw = splitmix64_next(state);
    let mantissa = (raw >> 40) as u32;
    let u01 = (mantissa as f32) / ((1u32 << 24) as f32);
    lo + u01 * (hi - lo)
}

/// Map a SplitMix64 output to a uniform u16 in `[0, modulus)`.
fn next_u16_mod(state: &mut u64, modulus: u16) -> u16 {
    debug_assert!(modulus > 0, "modulus must be positive");
    let raw = splitmix64_next(state);
    (raw % modulus as u64) as u16
}

// =============================================================================
// Vector generators
// =============================================================================

/// Generate a unit-norm `Vec<f32>` of `dim` dimensions using the default seed.
pub fn generate_random_unit_vector(dim: usize) -> Vec<f32> {
    generate_random_unit_vector_with_seed(dim, DEFAULT_TEST_SEED)
}

/// Generate a unit-norm `Vec<f32>` of `dim` dimensions using `seed`.
pub fn generate_random_unit_vector_with_seed(dim: usize, seed: u64) -> Vec<f32> {
    let mut state = seed;
    let mut vec: Vec<f32> = (0..dim).map(|_| next_f32_pm1(&mut state)).collect();
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}

/// Generate a sparse vector with `target_nnz` non-zero entries using the default seed.
///
/// Indices are in 0..30522 (BERT vocab size) with no duplicates; values are
/// positive floats in 0.1..2.0 (SPLADE/keyword score range). Content is
/// uniform pseudo-random — NOT a real SPLADE output.
pub fn generate_random_sparse_vector(target_nnz: usize) -> SparseVector {
    generate_random_sparse_vector_with_seed(target_nnz, DEFAULT_TEST_SEED)
}

/// Generate a sparse vector with `target_nnz` non-zero entries using `seed`.
pub fn generate_random_sparse_vector_with_seed(target_nnz: usize, seed: u64) -> SparseVector {
    let mut state = seed;
    let mut indices_set: BTreeSet<u16> = BTreeSet::new();
    while indices_set.len() < target_nnz {
        indices_set.insert(next_u16_mod(&mut state, 30522));
    }
    let indices: Vec<u16> = indices_set.into_iter().collect();
    let values: Vec<f32> = (0..target_nnz)
        .map(|_| next_f32_in(&mut state, 0.1, 2.0))
        .collect();
    SparseVector::new(indices, values).expect("Failed to create sparse vector")
}

/// Generate a complete `SemanticFingerprint` with correct dimensions for all
/// 14 embedder slots, using the default seed.
pub fn generate_random_semantic_fingerprint() -> SemanticFingerprint {
    generate_random_semantic_fingerprint_with_seed(DEFAULT_TEST_SEED)
}

/// Generate a complete `SemanticFingerprint` using a specific seed.
///
/// Each embedder slot is filled with a unit vector derived from a distinct
/// sub-seed (seed + slot offset), ensuring slots have visibly different
/// content for sanity-checking serialization round-trips. Slot identity is
/// doctrine (CLAUDE.md §6.2).
pub fn generate_random_semantic_fingerprint_with_seed(seed: u64) -> SemanticFingerprint {
    let sub_seed = |slot: u64| seed.wrapping_add(slot.wrapping_mul(0x9E3779B97F4A7C15));

    SemanticFingerprint {
        e1_semantic: generate_random_unit_vector_with_seed(1024, sub_seed(1)),
        e2_temporal_recent: generate_random_unit_vector_with_seed(512, sub_seed(2)),
        e3_temporal_periodic: generate_random_unit_vector_with_seed(512, sub_seed(3)),
        e4_temporal_positional: generate_random_unit_vector_with_seed(512, sub_seed(4)),
        e5_causal_as_cause: generate_random_unit_vector_with_seed(768, sub_seed(5)),
        e5_causal_as_effect: generate_random_unit_vector_with_seed(768, sub_seed(6)),
        // TST-L1: INTENTIONALLY empty. The legacy unified e5_causal field is deprecated;
        // production uses the dual vectors (e5_causal_as_cause / e5_causal_as_effect).
        e5_causal: Vec::new(),
        e6_sparse: generate_random_sparse_vector_with_seed(100, sub_seed(7)),
        e7_code: generate_random_unit_vector_with_seed(1536, sub_seed(8)),
        e8_graph_as_source: generate_random_unit_vector_with_seed(1024, sub_seed(9)),
        e8_graph_as_target: generate_random_unit_vector_with_seed(1024, sub_seed(10)),
        // TST-L1: INTENTIONALLY empty. Legacy unified e8_graph field is deprecated.
        e8_graph: Vec::new(),
        e9_hdc: generate_random_unit_vector_with_seed(1024, sub_seed(11)),
        e10_multimodal_paraphrase: generate_random_unit_vector_with_seed(768, sub_seed(12)),
        e10_multimodal_as_context: generate_random_unit_vector_with_seed(768, sub_seed(13)),
        e11_entity: generate_random_unit_vector_with_seed(768, sub_seed(14)),
        e12_late_interaction: (0..16)
            .map(|t| generate_random_unit_vector_with_seed(128, sub_seed(15 + t as u64)))
            .collect(),
        e13_splade: generate_random_sparse_vector_with_seed(500, sub_seed(31)),
        e14_bge_m3_dense: generate_random_unit_vector_with_seed(1024, sub_seed(32)),
    }
}

/// Generate a deterministic 32-byte content hash using the default seed.
pub fn generate_random_content_hash() -> [u8; 32] {
    generate_random_content_hash_with_seed(DEFAULT_TEST_SEED)
}

/// Generate a deterministic 32-byte content hash from `seed`.
pub fn generate_random_content_hash_with_seed(seed: u64) -> [u8; 32] {
    let mut state = seed;
    let mut hash = [0u8; 32];
    for chunk in hash.chunks_exact_mut(8) {
        let bytes = splitmix64_next(&mut state).to_le_bytes();
        chunk.copy_from_slice(&bytes);
    }
    hash
}

// =============================================================================
// TeleologicalFingerprint constructors
// =============================================================================

/// Create a `TeleologicalFingerprint` with a new random Uuid using the default seed.
pub fn create_random_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        generate_random_semantic_fingerprint(),
        generate_random_content_hash(),
    )
}

/// Create a `TeleologicalFingerprint` with a specific Uuid using the default seed.
pub fn create_random_fingerprint_with_id(id: Uuid) -> TeleologicalFingerprint {
    TeleologicalFingerprint::with_id(
        id,
        generate_random_semantic_fingerprint(),
        generate_random_content_hash(),
    )
}

/// Create a `TeleologicalFingerprint` with a specific Uuid and seed.
///
/// Useful when a test needs N reproducible-but-distinct fingerprints — pass
/// `seed = base + i` for the i-th fingerprint.
pub fn create_random_fingerprint_with_id_and_seed(id: Uuid, seed: u64) -> TeleologicalFingerprint {
    TeleologicalFingerprint::with_id(
        id,
        generate_random_semantic_fingerprint_with_seed(seed),
        generate_random_content_hash_with_seed(seed),
    )
}

/// Create a `TeleologicalFingerprint` with the specified UUID.
///
/// TST-M1 FIX: Now uses the provided `id` via `create_random_fingerprint_with_id()`
/// instead of ignoring it.
pub fn generate_random_teleological_fingerprint(id: Uuid) -> TeleologicalFingerprint {
    create_random_fingerprint_with_id(id)
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

// =============================================================================
// Regression tests for the helpers themselves (M-H2, GH #485)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seed_is_reproducible_unit_vector() {
        let a = generate_random_unit_vector(1024);
        let b = generate_random_unit_vector(1024);
        assert_eq!(a, b, "default-seed vectors must be byte-identical");
        let norm_sq: f32 = a.iter().map(|x| x * x).sum();
        assert!(
            (norm_sq - 1.0).abs() < 1e-5,
            "unit vector norm^2 = {} (expected ~1.0)",
            norm_sq
        );
    }

    #[test]
    fn distinct_seeds_produce_distinct_vectors() {
        let a = generate_random_unit_vector_with_seed(1024, 0);
        let b = generate_random_unit_vector_with_seed(1024, u64::MAX);
        assert_ne!(a, b, "seed=0 vs seed=u64::MAX must differ");
    }

    #[test]
    fn seed_zero_produces_valid_unit_vector() {
        // Edge case: seed=0 must not panic and must produce a valid unit vector.
        let v = generate_random_unit_vector_with_seed(1024, 0);
        assert_eq!(v.len(), 1024);
        let norm_sq: f32 = v.iter().map(|x| x * x).sum();
        assert!(
            (norm_sq - 1.0).abs() < 1e-5,
            "seed=0 norm^2 = {} (expected ~1.0)",
            norm_sq
        );
        assert!(
            v.iter().any(|&x| x != 0.0),
            "seed=0 must not produce all zeros"
        );
    }

    #[test]
    fn seed_u64_max_produces_valid_unit_vector() {
        let v = generate_random_unit_vector_with_seed(512, u64::MAX);
        assert_eq!(v.len(), 512);
        let norm_sq: f32 = v.iter().map(|x| x * x).sum();
        assert!(
            (norm_sq - 1.0).abs() < 1e-5,
            "seed=u64::MAX norm^2 = {} (expected ~1.0)",
            norm_sq
        );
    }

    #[test]
    fn sparse_vector_has_correct_nnz_and_sorted_indices() {
        let sv = generate_random_sparse_vector(100);
        assert_eq!(sv.indices.len(), 100, "exactly target_nnz non-zeros");
        assert_eq!(sv.values.len(), 100);
        for w in sv.indices.windows(2) {
            assert!(w[0] < w[1], "indices must be strictly ascending");
        }
        assert!(sv.indices.iter().all(|&i| i < 30522));
        assert!(sv.values.iter().all(|&v| (0.1..2.0).contains(&v)));
    }

    #[test]
    fn content_hash_is_deterministic_and_non_zero() {
        let h1 = generate_random_content_hash();
        let h2 = generate_random_content_hash();
        assert_eq!(h1, h2, "default-seed hash must be byte-identical");
        // M-L2 alignment: hash must not be all-zero
        // (`no_fake_data_test.rs` rejects all-zero checksums).
        assert!(h1.iter().any(|&b| b != 0), "hash must not be all zero");
    }

    #[test]
    fn semantic_fingerprint_has_correct_dimensions() {
        let fp = generate_random_semantic_fingerprint();
        assert_eq!(fp.e1_semantic.len(), 1024);
        assert_eq!(fp.e2_temporal_recent.len(), 512);
        assert_eq!(fp.e3_temporal_periodic.len(), 512);
        assert_eq!(fp.e4_temporal_positional.len(), 512);
        assert_eq!(fp.e5_causal_as_cause.len(), 768);
        assert_eq!(fp.e5_causal_as_effect.len(), 768);
        assert_eq!(fp.e7_code.len(), 1536);
        assert_eq!(fp.e8_graph_as_source.len(), 1024);
        assert_eq!(fp.e8_graph_as_target.len(), 1024);
        assert_eq!(fp.e9_hdc.len(), 1024);
        assert_eq!(fp.e10_multimodal_paraphrase.len(), 768);
        assert_eq!(fp.e10_multimodal_as_context.len(), 768);
        assert_eq!(fp.e11_entity.len(), 768);
        assert_eq!(fp.e12_late_interaction.len(), 16);
        assert!(fp.e12_late_interaction.iter().all(|t| t.len() == 128));
        assert_eq!(fp.e14_bge_m3_dense.len(), 1024);
        // Slot identity: distinct sub-seeds => distinct slot content.
        assert_ne!(
            &fp.e1_semantic[..16],
            &fp.e9_hdc[..16],
            "different slots must have different content (slot identity)"
        );
    }

    #[test]
    fn fingerprint_with_id_preserves_id() {
        let id = Uuid::new_v4();
        let fp = create_random_fingerprint_with_id(id);
        assert_eq!(fp.id, id);
    }

    #[test]
    fn distinct_seeds_via_with_id_and_seed_yield_distinct_content() {
        let id = Uuid::new_v4();
        let a = create_random_fingerprint_with_id_and_seed(id, 1);
        let b = create_random_fingerprint_with_id_and_seed(id, 2);
        assert_eq!(a.id, b.id, "id is preserved across distinct seeds");
        assert_ne!(
            a.content_hash, b.content_hash,
            "distinct seeds => distinct content hashes"
        );
    }
}
