//! Reservoir sampling (Vitter's Algorithm R) for approximate percentile
//! computation.
//!
//! For large streams (millions of cosine similarities) we can't afford to
//! retain every sample, but we still need p50/p95/min/max. Reservoir sampling
//! maintains a uniform sample of size `capacity` from an unbounded stream,
//! giving percentiles that converge to the true distribution as the sample
//! grows.
//!
//! All samples are `f32`; non-finite values are dropped.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Default seed used by `ReservoirSample::new` — deterministic across runs so
/// tests don't flake. Use `with_seed` to override when multiple reservoirs
/// must produce independent samples in the same process.
const DEFAULT_SEED: u64 = 0x6C_6F_67_69_73_74_69_63; // "logistic" in ASCII.

/// Fixed-capacity reservoir of `f32` samples.
#[derive(Debug, Clone)]
pub struct ReservoirSample {
    capacity: usize,
    samples: Vec<f32>,
    seen: u64,
    rng: StdRng,
}

impl ReservoirSample {
    /// Build a reservoir with the default seed. Capacity `0` is valid (every
    /// call to `observe` is a no-op); percentiles on an empty reservoir
    /// return `0.0`.
    pub fn new(capacity: usize) -> Self {
        Self::with_seed(capacity, DEFAULT_SEED)
    }

    pub fn with_seed(capacity: usize, seed: u64) -> Self {
        Self {
            capacity,
            samples: Vec::with_capacity(capacity),
            seen: 0,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// Observe one sample. Non-finite inputs are dropped.
    pub fn observe(&mut self, value: f32) {
        if !value.is_finite() || self.capacity == 0 {
            return;
        }
        self.seen += 1;
        if self.samples.len() < self.capacity {
            self.samples.push(value);
            return;
        }
        // Algorithm R: replace a random slot with probability capacity / seen.
        let idx = self.rng.gen_range(0..self.seen);
        if (idx as usize) < self.capacity {
            self.samples[idx as usize] = value;
        }
    }

    /// Return a fresh, ascending-sorted copy of the current samples.
    pub fn sorted(&self) -> Vec<f32> {
        let mut v = self.samples.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    /// Approximate quantile `q` in `[0.0, 1.0]` via linear interpolation on
    /// the sorted reservoir. Returns `0.0` on an empty reservoir.
    pub fn percentile(&self, q: f32) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let q = q.clamp(0.0, 1.0);
        let sorted = self.sorted();
        if sorted.len() == 1 {
            return sorted[0];
        }
        let pos = q * (sorted.len() - 1) as f32;
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        if lo == hi {
            return sorted[lo];
        }
        let frac = pos - lo as f32;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }

    /// Minimum observed-and-kept sample. Returns `0.0` on an empty reservoir.
    pub fn min(&self) -> f32 {
        self.samples
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min)
            .min(f32::INFINITY)
            .take_if_finite()
    }

    /// Maximum observed-and-kept sample. Returns `0.0` on an empty reservoir.
    pub fn max(&self) -> f32 {
        self.samples
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
            .take_if_finite_or(f32::NEG_INFINITY)
    }

    /// Raw count of samples observed via `observe` (finite inputs only).
    #[inline]
    pub fn seen(&self) -> u64 {
        self.seen
    }

    /// Current reservoir occupancy (`<= capacity`).
    #[inline]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

// --- small helper trait to keep min/max branchless-ish ---
trait FiniteFallback {
    fn take_if_finite(self) -> f32;
    fn take_if_finite_or(self, fallback: f32) -> f32;
}

impl FiniteFallback for f32 {
    fn take_if_finite(self) -> f32 {
        if self.is_finite() {
            self
        } else {
            0.0
        }
    }
    fn take_if_finite_or(self, fallback: f32) -> f32 {
        if self.is_finite() {
            self
        } else if fallback.is_finite() {
            fallback
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_reservoir_percentiles_are_zero() {
        let r = ReservoirSample::new(128);
        assert_eq!(r.percentile(0.5), 0.0);
        assert_eq!(r.min(), 0.0);
        assert_eq!(r.max(), 0.0);
        assert!(r.is_empty());
    }

    #[test]
    fn zero_capacity_reservoir_is_noop() {
        let mut r = ReservoirSample::new(0);
        for _ in 0..100 {
            r.observe(1.0);
        }
        assert_eq!(r.seen(), 0, "zero-capacity reservoir rejects all input");
        assert!(r.is_empty());
    }

    #[test]
    fn uniform_stream_percentiles_converge() {
        // 10_000 samples drawn uniformly [0, 1] with a seeded RNG.
        // The reservoir keeps 1024 of them; the resulting p50 should be close
        // to 0.5 and p95 close to 0.95 within statistical tolerance.
        let mut r = ReservoirSample::with_seed(1024, 0xDEADBEEF);
        let mut source = StdRng::seed_from_u64(0xB01D_CAFE);
        for _ in 0..10_000 {
            let v: f32 = source.gen_range(0.0..1.0);
            r.observe(v);
        }
        let p50 = r.percentile(0.5);
        let p95 = r.percentile(0.95);
        assert!(
            (p50 - 0.5).abs() < 0.03,
            "p50 off: {} (tolerance 0.03)",
            p50
        );
        assert!(
            (p95 - 0.95).abs() < 0.03,
            "p95 off: {} (tolerance 0.03)",
            p95
        );
        assert!(r.min() >= 0.0);
        assert!(r.max() <= 1.0);
    }

    #[test]
    fn reservoir_drops_non_finite() {
        let mut r = ReservoirSample::new(16);
        r.observe(f32::NAN);
        r.observe(f32::INFINITY);
        r.observe(f32::NEG_INFINITY);
        assert!(r.is_empty());
        assert_eq!(r.seen(), 0);
        r.observe(0.25);
        assert_eq!(r.seen(), 1);
    }
}
