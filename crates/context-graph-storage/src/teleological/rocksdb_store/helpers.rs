//! Helper functions for RocksDbTeleologicalStore.
//!
//! Contains utility functions for computing similarity and formatting.

/// Encode a byte slice as lowercase hexadecimal string.
///
/// Used in error messages for RocksDB keys that are raw byte arrays.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Compute cosine similarity between two dense vectors.
///
/// # Panics
/// STG-06 FIX: Panics in ALL build modes if vectors have different dimensions.
/// Dimension mismatches indicate upstream embedding pipeline bugs that must
/// be caught immediately, not silently hidden by returning 0.0.
pub fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // STG-06 FIX: FAIL FAST in ALL build modes. Dimension mismatches are programming
    // bugs that must be caught immediately. Previously, release mode silently returned
    // 0.0, hiding upstream bugs and suppressing legitimate search results.
    assert_eq!(
        a.len(),
        b.len(),
        "FATAL: Cosine similarity dimension mismatch: vector A has {} dimensions, \
         vector B has {} dimensions. This indicates a bug in the embedding pipeline \
         or a cross-embedder comparison (AP-02 violation). Fix the caller.",
        a.len(),
        b.len()
    );

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a.sqrt()) * (norm_b.sqrt());
    if denom < f32::EPSILON {
        0.0
    } else {
        // Normalize from [-1, 1] to [0, 1] to match core crate's cosine_similarity()
        (dot / denom + 1.0) / 2.0
    }
}
