//! Recency embedding computation for the Temporal-Recent model.
//!
//! Two encoding modes:
//! - **Sinusoidal** (production, `reference_time=None`): Encodes absolute Unix timestamp
//!   using transformer-style positional encoding. Different timestamps produce distinct
//!   vectors, and cosine similarity reflects temporal proximity.
//! - **Decay** (tests, `reference_time=Some`): Original exponential decay with phase
//!   variations relative to a fixed reference point.

use chrono::{DateTime, Utc};

use super::constants::{FEATURES_PER_SCALE, MAX_TIME_DELTA_SECS, TEMPORAL_RECENT_DIMENSION};

/// Compute the recency embedding for a given timestamp.
///
/// # Production path (`reference_time = None`)
///
/// Uses **sinusoidal position encoding** of the absolute Unix timestamp. Each
/// dimension pair encodes `sin(t * freq)` / `cos(t * freq)` at geometrically
/// spaced frequencies, giving:
/// - Low indices → fine-grained (second/minute differences)
/// - High indices → coarse-grained (day/week differences)
///
/// This produces genuinely distinct vectors for different timestamps. The
/// previous decay-based encoding produced delta≈0 for all memories (since
/// embedding happens immediately after creation), making all E2 vectors
/// identical and breaking HNSW search.
///
/// # Test path (`reference_time = Some(ref)`)
///
/// Uses the original exponential decay encoding relative to the given
/// reference time. This preserves backward compatibility with tests that
/// use `TemporalRecentModel::with_reference_time()`.
///
/// # Arguments
/// * `timestamp` - The timestamp to encode
/// * `reference_time` - `None` for sinusoidal (production), `Some(ref)` for decay (tests)
/// * `decay_rates` - Decay rates for each time scale (only used in decay mode)
///
/// # Returns
/// A 512-dimensional L2-normalized vector encoding temporal recency.
pub fn compute_decay_embedding(
    timestamp: DateTime<Utc>,
    reference_time: Option<DateTime<Utc>>,
    decay_rates: &[f32],
) -> Vec<f32> {
    match reference_time {
        None => compute_sinusoidal_recency(timestamp),
        Some(reference) => compute_legacy_decay(timestamp, reference, decay_rates),
    }
}

/// Sinusoidal position encoding of absolute Unix timestamp (production).
///
/// Uses transformer-style PE: for each dimension pair (2k, 2k+1):
///   vec[2k]   = sin(timestamp / base^(2k/dim))
///   vec[2k+1] = cos(timestamp / base^(2k/dim))
///
/// Base = 86400 (1 day in seconds), so:
///   - k=0: wavelength ≈ 6.28s → captures second-level differences
///   - k=127: wavelength ≈ days → captures day/week-level differences
///   - k=255: wavelength ≈ months → captures month-level differences
///
/// Properties:
///   - Nearby timestamps → high cosine similarity
///   - Distinct timestamps → distinct vectors (non-degenerate)
///   - L2-normalized for consistent cosine distance in HNSW
fn compute_sinusoidal_recency(timestamp: DateTime<Utc>) -> Vec<f32> {
    // Use f64 for precision — Unix timestamps are ~1.77 billion (2026)
    let timestamp_secs = timestamp.timestamp() as f64;

    let dim = TEMPORAL_RECENT_DIMENSION;
    let half_dim = dim / 2; // 256 sin/cos pairs for 512D

    // Base controls the frequency geometric progression.
    // 86400 (1 day) gives a good spread from seconds to months.
    let base: f64 = 86400.0;

    let mut vector = Vec::with_capacity(dim);

    for k in 0..half_dim {
        // Geometric frequency: higher k → lower frequency → coarser time scale
        let exponent = 2.0 * k as f64 / dim as f64;
        let divisor = base.powf(exponent);
        let angle = timestamp_secs / divisor;

        vector.push(angle.sin() as f32);
        vector.push(angle.cos() as f32);
    }

    // L2 normalize
    l2_normalize(&mut vector);

    vector
}

/// Legacy exponential decay encoding (for tests with fixed reference time).
///
/// Computes time delta from reference, applies exponential decay at multiple
/// scales with phase-varied cosine features.
fn compute_legacy_decay(
    timestamp: DateTime<Utc>,
    reference: DateTime<Utc>,
    decay_rates: &[f32],
) -> Vec<f32> {
    let time_delta_secs = (reference - timestamp).num_seconds() as f32;

    let mut vector = Vec::with_capacity(TEMPORAL_RECENT_DIMENSION);

    for &decay_rate in decay_rates {
        let clamped_delta = time_delta_secs.clamp(0.0, MAX_TIME_DELTA_SECS);
        let base_decay = (-decay_rate * clamped_delta).exp();

        for i in 0..FEATURES_PER_SCALE {
            let phase = (i as f32) * std::f32::consts::PI / 64.0;
            let value = base_decay * (phase + clamped_delta * decay_rate * 0.001).cos();
            vector.push(value);
        }
    }

    l2_normalize(&mut vector);

    vector
}

/// L2 normalize a vector in place.
///
/// If the vector has zero magnitude (within epsilon), leaves it unchanged.
#[inline]
fn l2_normalize(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in vector.iter_mut() {
            *v /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // =========================================================================
    // Legacy decay tests (reference_time = Some)
    // =========================================================================

    #[test]
    fn test_compute_decay_embedding_dimension() {
        let ref_time = Utc::now();
        let timestamp = ref_time - Duration::hours(1);
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];

        let embedding = compute_decay_embedding(timestamp, Some(ref_time), &decay_rates);

        assert_eq!(embedding.len(), 512, "Must produce 512D vector");
    }

    #[test]
    fn test_compute_decay_embedding_normalized() {
        let ref_time = Utc::now();
        let timestamp = ref_time - Duration::hours(1);
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];

        let embedding = compute_decay_embedding(timestamp, Some(ref_time), &decay_rates);
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        assert!(
            (norm - 1.0).abs() < 0.001,
            "Vector must be L2 normalized, got norm = {}",
            norm
        );
    }

    #[test]
    fn test_compute_decay_embedding_no_nan() {
        let ref_time = Utc::now();
        let timestamp = ref_time - Duration::days(365); // Very old
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];

        let embedding = compute_decay_embedding(timestamp, Some(ref_time), &decay_rates);

        assert!(
            embedding.iter().all(|x| x.is_finite()),
            "Must not contain NaN or Inf values"
        );
    }

    #[test]
    fn test_compute_decay_embedding_future_timestamp() {
        let ref_time = Utc::now();
        let timestamp = ref_time + Duration::days(30); // Future
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];

        let embedding = compute_decay_embedding(timestamp, Some(ref_time), &decay_rates);

        assert!(
            embedding.iter().all(|x| x.is_finite()),
            "Future timestamps must produce valid output"
        );
    }

    // =========================================================================
    // Sinusoidal recency tests (reference_time = None)
    // =========================================================================

    #[test]
    fn test_sinusoidal_dimension() {
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];
        let embedding = compute_decay_embedding(Utc::now(), None, &decay_rates);
        assert_eq!(embedding.len(), 512, "Sinusoidal must produce 512D vector");
    }

    #[test]
    fn test_sinusoidal_normalized() {
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];
        let embedding = compute_decay_embedding(Utc::now(), None, &decay_rates);
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.001,
            "Sinusoidal vector must be L2 normalized, got norm = {}",
            norm
        );
    }

    #[test]
    fn test_sinusoidal_no_nan_or_inf() {
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];
        let embedding = compute_decay_embedding(Utc::now(), None, &decay_rates);
        assert!(
            embedding.iter().all(|x| x.is_finite()),
            "Sinusoidal must not contain NaN or Inf"
        );
    }

    #[test]
    fn test_sinusoidal_different_timestamps_different_vectors() {
        use chrono::TimeZone;
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];
        let t1 = Utc.with_ymd_and_hms(2026, 2, 25, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 2, 25, 13, 0, 0).unwrap(); // 1 hour later

        let v1 = compute_decay_embedding(t1, None, &decay_rates);
        let v2 = compute_decay_embedding(t2, None, &decay_rates);

        assert_ne!(
            v1, v2,
            "Different timestamps must produce different vectors"
        );

        // Cosine similarity should be positive (1 hour apart is relatively close)
        let dot: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        assert!(
            dot > -0.5,
            "1-hour-apart timestamps should not be anti-correlated, got {}",
            dot
        );
    }

    #[test]
    fn test_sinusoidal_nearby_more_similar_than_distant() {
        use chrono::TimeZone;
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];
        let t_now = Utc.with_ymd_and_hms(2026, 2, 25, 12, 0, 0).unwrap();
        let t_1h = t_now - Duration::hours(1);
        let t_1d = t_now - Duration::days(1);

        let v_now = compute_decay_embedding(t_now, None, &decay_rates);
        let v_1h = compute_decay_embedding(t_1h, None, &decay_rates);
        let v_1d = compute_decay_embedding(t_1d, None, &decay_rates);

        let sim_1h: f32 = v_now.iter().zip(v_1h.iter()).map(|(a, b)| a * b).sum();
        let sim_1d: f32 = v_now.iter().zip(v_1d.iter()).map(|(a, b)| a * b).sum();

        assert!(
            sim_1h > sim_1d,
            "1-hour should be more similar than 1-day: sim_1h={}, sim_1d={}",
            sim_1h,
            sim_1d
        );
    }

    #[test]
    fn test_sinusoidal_deterministic() {
        use chrono::TimeZone;
        let decay_rates = vec![1.0 / 3600.0, 1.0 / 86400.0, 1.0 / 604800.0, 1.0 / 2592000.0];
        let t = Utc.with_ymd_and_hms(2026, 2, 25, 12, 0, 0).unwrap();

        let v1 = compute_decay_embedding(t, None, &decay_rates);
        let v2 = compute_decay_embedding(t, None, &decay_rates);

        assert_eq!(v1, v2, "Same timestamp must produce identical vectors");
    }

    // =========================================================================
    // L2 normalize tests
    // =========================================================================

    #[test]
    fn test_l2_normalize() {
        let mut vector = vec![3.0, 4.0];
        l2_normalize(&mut vector);

        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
        assert!((vector[0] - 0.6).abs() < 0.001);
        assert!((vector[1] - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let mut vector = vec![0.0, 0.0, 0.0];
        l2_normalize(&mut vector);

        // Zero vector should remain unchanged
        assert!(vector.iter().all(|&x| x == 0.0));
    }
}
