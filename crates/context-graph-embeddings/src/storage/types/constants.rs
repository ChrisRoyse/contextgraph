//! Constitution constants for quantized storage.
//!
//! These constants are derived from constitution.yaml `embeddings.storage_per_memory`.

/// Expected size in bytes for a complete quantized fingerprint.
/// From Constitution: "~17KB (quantized) vs 46KB uncompressed"
///
/// Breakdown (approximate):
/// - PQ-8 embeddings (E1, E5, E7, E10): 4 × 8 bytes = 32 bytes
/// - Float8 embeddings (E2, E3, E4, E8, E11): 5 × (dim/4) bytes ≈ 2,740 bytes
/// - Binary embedding (E9): 10,000 bits / 8 = 1,250 bytes
/// - Sparse embeddings (E6, E13): ~10KB combined (variable)
/// - Token pruning (E12): ~2KB (50% of original)
/// - Metadata: ~1KB
///
/// Total: ~17KB
pub const EXPECTED_QUANTIZED_SIZE_BYTES: usize = 17_000;

/// Maximum allowed size for a quantized fingerprint.
/// Allow 50% overhead for sparse vectors with many non-zeros.
pub const MAX_QUANTIZED_SIZE_BYTES: usize = 25_000;

/// Minimum valid size - catches empty or corrupted fingerprints.
pub const MIN_QUANTIZED_SIZE_BYTES: usize = 5_000;

/// Number of embedders in the multi-array system.
pub const NUM_EMBEDDERS: usize = 14;

/// Storage format version. Bump when struct layout changes.
/// Version mismatches will panic (no migration support).
pub const STORAGE_VERSION: u8 = 1;

/// RRF constant k for multi-space fusion.
/// From Constitution: "RRF(d) = Σᵢ 1/(k + rankᵢ(d)) where k=60"
pub const RRF_K: f32 = 60.0;
