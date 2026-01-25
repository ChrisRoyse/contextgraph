//! Session signature computation for hybrid E4 encoding.
//!
//! Generates a deterministic 256D signature from session_id that:
//! - Same session_id produces identical signature
//! - Different session_id produces orthogonal signatures (high probability)
//!
//! This module enables same-session memories to cluster in E4 embedding space
//! while the position encoding component preserves fine-grained ordering.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::constants::SESSION_SIGNATURE_DIMENSION;

/// Sentinel value used when no session_id is provided.
pub const NO_SESSION_SENTINEL: &str = "__no_session__";

/// Compute deterministic session signature from session_id.
///
/// Uses hash-based pseudo-random generation to create a stable 256D vector.
/// The signature is L2-normalized to ensure consistent similarity comparisons.
///
/// # Algorithm
///
/// For each of the 256 dimensions:
/// 1. Hash the session_id combined with the dimension index
/// 2. Convert the hash to a value in the [-1, 1] range
/// 3. L2-normalize the final vector
///
/// This approach ensures:
/// - Determinism: Same session_id always produces the same signature
/// - Orthogonality: Different session_ids have low expected similarity
/// - Uniform distribution: Values span the hypersphere uniformly
///
/// # Arguments
///
/// * `session_id` - The session identifier string
///
/// # Returns
///
/// A 256-dimensional L2-normalized vector
pub fn compute_session_signature(session_id: &str) -> Vec<f32> {
    let mut signature = Vec::with_capacity(SESSION_SIGNATURE_DIMENSION);

    // Use multiple hash seeds to generate 256 pseudo-random values
    for i in 0..SESSION_SIGNATURE_DIMENSION {
        let mut hasher = DefaultHasher::new();
        session_id.hash(&mut hasher);
        i.hash(&mut hasher); // Different seed per dimension
        let hash = hasher.finish();

        // Convert to [-1, 1] range using hash bits
        // This creates a uniform distribution over the range
        let value = ((hash as f64 / u64::MAX as f64) * 2.0 - 1.0) as f32;
        signature.push(value);
    }

    // L2 normalize to unit vector
    l2_normalize(&mut signature);

    signature
}

/// Compute session signature with fallback for missing session_id.
///
/// If session_id is None or empty, returns a "no-session" sentinel signature.
/// This ensures backward compatibility with memories that don't have session context.
///
/// # Arguments
///
/// * `session_id` - Optional session identifier
///
/// # Returns
///
/// A 256-dimensional L2-normalized vector
pub fn compute_session_signature_or_default(session_id: Option<&str>) -> Vec<f32> {
    match session_id {
        Some(id) if !id.is_empty() => compute_session_signature(id),
        _ => compute_session_signature(NO_SESSION_SENTINEL),
    }
}

/// L2 normalize a vector in place.
///
/// If the vector has zero norm, it remains unchanged.
fn l2_normalize(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in vector.iter_mut() {
            *v /= norm;
        }
    }
}

/// Compute cosine similarity between two session signatures.
///
/// Both vectors are assumed to be L2-normalized, so this is just the dot product.
#[allow(dead_code)]
pub fn signature_similarity(sig1: &[f32], sig2: &[f32]) -> f32 {
    if sig1.len() != sig2.len() {
        return 0.0;
    }
    sig1.iter().zip(sig2.iter()).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_signature_deterministic() {
        let sig1 = compute_session_signature("session-123");
        let sig2 = compute_session_signature("session-123");
        assert_eq!(sig1, sig2, "Same session should produce identical signature");
    }

    #[test]
    fn test_different_sessions_different_signatures() {
        let sig1 = compute_session_signature("session-123");
        let sig2 = compute_session_signature("session-456");
        let sim = signature_similarity(&sig1, &sig2);
        // With 256D and hash-based generation, expected similarity is near 0
        // Allow some variance, but should be much less than 0.5
        assert!(
            sim.abs() < 0.3,
            "Different sessions should have low similarity: {}",
            sim
        );
    }

    #[test]
    fn test_session_signature_normalized() {
        let sig = compute_session_signature("session-123");
        let norm: f32 = sig.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "Signature should be L2 normalized, got norm: {}",
            norm
        );
    }

    #[test]
    fn test_session_signature_dimension() {
        let sig = compute_session_signature("session-123");
        assert_eq!(sig.len(), SESSION_SIGNATURE_DIMENSION);
    }

    #[test]
    fn test_session_signature_or_default_with_id() {
        let sig1 = compute_session_signature("my-session");
        let sig2 = compute_session_signature_or_default(Some("my-session"));
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_session_signature_or_default_none() {
        let sig1 = compute_session_signature(NO_SESSION_SENTINEL);
        let sig2 = compute_session_signature_or_default(None);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_session_signature_or_default_empty() {
        let sig1 = compute_session_signature(NO_SESSION_SENTINEL);
        let sig2 = compute_session_signature_or_default(Some(""));
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_same_session_self_similarity() {
        let sig = compute_session_signature("test-session");
        let sim = signature_similarity(&sig, &sig);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Self-similarity should be 1.0, got: {}",
            sim
        );
    }

    #[test]
    fn test_many_sessions_orthogonal() {
        // Generate 100 different session signatures and check they're mostly orthogonal
        let sigs: Vec<Vec<f32>> = (0..100)
            .map(|i| compute_session_signature(&format!("session-{}", i)))
            .collect();

        let mut total_sim = 0.0;
        let mut count = 0;
        for i in 0..sigs.len() {
            for j in (i + 1)..sigs.len() {
                total_sim += signature_similarity(&sigs[i], &sigs[j]).abs();
                count += 1;
            }
        }
        let avg_sim = total_sim / count as f32;
        // Average pairwise similarity should be very low for orthogonal vectors
        assert!(
            avg_sim < 0.15,
            "Average pairwise similarity should be low: {}",
            avg_sim
        );
    }

    #[test]
    fn test_uuid_session_ids() {
        // Test with realistic UUID session IDs
        let sig1 = compute_session_signature("a1b2c3d4-e5f6-7890-abcd-ef1234567890");
        let sig2 = compute_session_signature("11111111-2222-3333-4444-555555555555");

        assert_eq!(sig1.len(), SESSION_SIGNATURE_DIMENSION);
        assert_eq!(sig2.len(), SESSION_SIGNATURE_DIMENSION);

        let sim = signature_similarity(&sig1, &sig2);
        assert!(sim.abs() < 0.3, "UUID sessions should be orthogonal: {}", sim);
    }
}
