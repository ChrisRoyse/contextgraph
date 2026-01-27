//! Helper functions for RocksDbTeleologicalStore.
//!
//! Contains utility functions for computing similarity.

/// Compute cosine similarity between two dense vectors.
///
/// # Panics (Debug Mode)
/// Panics if vectors have different dimensions. This catches embedding
/// dimension mismatches early during development.
///
/// # Returns (Release Mode)
/// Returns 0.0 for mismatched dimensions to avoid crashing production.
pub fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // FAIL FAST in debug mode - catches dimension mismatches during development
    debug_assert_eq!(
        a.len(),
        b.len(),
        "FAIL FAST: Cosine similarity dimension mismatch: {} vs {}",
        a.len(),
        b.len()
    );
    if a.len() != b.len() {
        return 0.0;
    }

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
        dot / denom
    }
}
