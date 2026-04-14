//! Online (single-pass) mean + variance estimators using Welford/Chan's
//! algorithm. Used by the constellation compiler to aggregate embedder
//! statistics without holding the full member set in memory.
//!
//! - [`WelfordStats`] — scalar (f32) stream.
//! - [`WelfordVector`] — vector (per-coordinate) stream. Also exposes the
//!   running mean vector (the centroid).
//!
//! Correctness: both agree with naive `mean` / `stddev_population` over any
//! finite stream of finite `f32`s to within ~1e-5. Non-finite inputs (NaN,
//! ±Inf) are silently skipped (they would poison the running sums).

/// Scalar Welford accumulator.
#[derive(Debug, Clone)]
pub struct WelfordStats {
    count: u64,
    mean: f32,
    m2: f32,
}

impl WelfordStats {
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Observe one sample. NaN / ±Inf are skipped without updating state.
    pub fn observe(&mut self, value: f32) {
        if !value.is_finite() {
            return;
        }
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f32;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Running mean (0.0 for an empty stream).
    #[inline]
    pub fn mean(&self) -> f32 {
        self.mean
    }

    /// Population variance (`m2 / count`, not Bessel-corrected). `0.0` for
    /// fewer than 2 samples.
    pub fn variance(&self) -> f32 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / self.count as f32
    }

    /// Population stddev (`sqrt(variance)`).
    pub fn stddev(&self) -> f32 {
        self.variance().sqrt()
    }
}

impl Default for WelfordStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-coordinate Welford accumulator.
///
/// Maintains a running mean and per-coordinate M2 (sum of squared deltas). The
/// scalar `stddev_l2` summary is the L2 norm of per-coordinate stddevs over
/// `sqrt(count)` — a rough scalar dispersion measure, kept separate from the
/// cosine-based spread computed elsewhere.
///
/// # Panics
///
/// `observe` panics if the input slice length differs from the fixed
/// dimension. This is deliberate (FAIL FAST) because silently truncating or
/// zero-padding would quietly corrupt downstream centroid statistics.
#[derive(Debug, Clone)]
pub struct WelfordVector {
    count: u64,
    mean: Vec<f32>,
    m2: Vec<f32>,
}

impl WelfordVector {
    pub fn new(dim: usize) -> Self {
        Self {
            count: 0,
            mean: vec![0.0; dim],
            m2: vec![0.0; dim],
        }
    }

    /// Observe one vector. Panics on dimension mismatch.
    pub fn observe(&mut self, v: &[f32]) {
        assert_eq!(
            v.len(),
            self.mean.len(),
            "WelfordVector dim mismatch: accumulator has {}, got {}",
            self.mean.len(),
            v.len()
        );
        // Skip non-finite inputs as a whole — a single NaN in any coordinate
        // would contaminate the running mean for every subsequent call.
        if v.iter().any(|x| !x.is_finite()) {
            return;
        }
        self.count += 1;
        let inv = 1.0 / self.count as f32;
        for i in 0..self.mean.len() {
            let delta = v[i] - self.mean[i];
            self.mean[i] += delta * inv;
            let delta2 = v[i] - self.mean[i];
            self.m2[i] += delta * delta2;
        }
    }

    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Borrow the running centroid vector.
    #[inline]
    pub fn mean(&self) -> &[f32] {
        &self.mean
    }

    /// L2 norm of the centroid vector.
    pub fn mean_l2(&self) -> f32 {
        let sumsq: f32 = self.mean.iter().map(|x| x * x).sum();
        sumsq.sqrt()
    }

    /// L2 norm of the per-coordinate stddev vector (population variance).
    ///
    /// Useful as a scalar summary of vector dispersion; note this is *not*
    /// the stddev of the vectors' L2 norms — track that separately with a
    /// `WelfordStats` on the L2-norm stream when needed.
    pub fn stddev_l2(&self) -> f32 {
        if self.count < 2 {
            return 0.0;
        }
        let inv = 1.0 / self.count as f32;
        let sumsq: f32 = self.m2.iter().map(|m| m * inv).sum();
        sumsq.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_mean_stddev(samples: &[f32]) -> (f32, f32) {
        let n = samples.len() as f32;
        let mean = samples.iter().sum::<f32>() / n;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
        (mean, var.sqrt())
    }

    #[test]
    fn welford_stats_agrees_with_naive_on_small_stream() {
        let samples: Vec<f32> = (0..100).map(|i| (i as f32) * 0.1 + 0.5).collect();
        let (naive_mean, naive_std) = naive_mean_stddev(&samples);

        let mut w = WelfordStats::new();
        for &x in &samples {
            w.observe(x);
        }
        assert_eq!(w.count(), 100);
        assert!((w.mean() - naive_mean).abs() < 1e-5, "mean mismatch");
        assert!((w.stddev() - naive_std).abs() < 1e-5, "stddev mismatch");
    }

    #[test]
    fn welford_stats_handles_empty_and_single() {
        let w = WelfordStats::new();
        assert_eq!(w.count(), 0);
        assert_eq!(w.mean(), 0.0);
        assert_eq!(w.variance(), 0.0);
        let mut w = WelfordStats::new();
        w.observe(1.0);
        assert_eq!(w.count(), 1);
        assert_eq!(w.mean(), 1.0);
        assert_eq!(w.variance(), 0.0);
    }

    #[test]
    fn welford_stats_skips_non_finite() {
        let mut w = WelfordStats::new();
        w.observe(1.0);
        w.observe(f32::NAN);
        w.observe(f32::INFINITY);
        w.observe(3.0);
        assert_eq!(w.count(), 2);
        assert!((w.mean() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn welford_vector_mean_matches_naive() {
        let dim = 4;
        let samples: Vec<Vec<f32>> = (0..50)
            .map(|i| (0..dim).map(|j| (i + j) as f32 * 0.01).collect())
            .collect();
        // naive centroid
        let mut naive = vec![0.0; dim];
        for s in &samples {
            for i in 0..dim {
                naive[i] += s[i];
            }
        }
        for x in &mut naive {
            *x /= samples.len() as f32;
        }

        let mut w = WelfordVector::new(dim);
        for s in &samples {
            w.observe(s);
        }
        assert_eq!(w.count(), samples.len() as u64);
        for i in 0..dim {
            assert!(
                (w.mean()[i] - naive[i]).abs() < 1e-5,
                "coord {} mismatch: {} vs {}",
                i,
                w.mean()[i],
                naive[i]
            );
        }
    }

    #[test]
    #[should_panic(expected = "WelfordVector dim mismatch")]
    fn welford_vector_rejects_dim_mismatch() {
        let mut w = WelfordVector::new(4);
        w.observe(&[1.0, 2.0, 3.0]);
    }

    #[test]
    fn welford_vector_skips_non_finite() {
        let mut w = WelfordVector::new(2);
        w.observe(&[1.0, 2.0]);
        w.observe(&[f32::NAN, 0.0]);
        w.observe(&[3.0, 4.0]);
        assert_eq!(w.count(), 2);
        assert!((w.mean()[0] - 2.0).abs() < 1e-6);
        assert!((w.mean()[1] - 3.0).abs() < 1e-6);
    }
}
