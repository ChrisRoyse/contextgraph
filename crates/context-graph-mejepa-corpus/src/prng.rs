use crate::{MutationError, MutationResult};

// SplitMix64 PRNG. Same algorithm used by `granger_fsv.rs` for synthetic
// Box-Muller samples; identical state machine here so corpus generation is
// byte-exact reproducible across builds. SplitMix64 is in the public domain
// (Vigna, 2014, https://dx.doi.org/10.1145/2714064.2660195).

/// 64-bit SplitMix64 generator. Pure-functional `next` returning (u64, next_state)
/// is intentional — keeps the operators free of `&mut self` so site-selection
/// loops are simpler to reason about.
#[derive(Debug, Clone, Copy)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Restore a previously saved internal state. This is required for
    /// deterministic Phase 3 sampler checkpoint/resume.
    pub fn from_state(state: u64) -> Self {
        Self { state }
    }

    /// Expose the exact internal state for checkpointing. Restoring this value
    /// with `from_state` makes the next draw byte-identical.
    pub fn state(&self) -> u64 {
        self.state
    }

    /// Advance the state and return a 64-bit pseudo-random integer.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Pick an index in `0..len` uniformly. Returns `None` when `len == 0`
    /// (caller should have rejected the empty-candidate case BEFORE getting
    /// here — this is just defensive).
    pub fn pick_index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            None
        } else {
            Some((self.next_u64() as usize) % len)
        }
    }

    pub fn next_unit_f32(&mut self) -> f32 {
        let raw = self.next_u64() >> 40;
        (raw as f32) / ((1u64 << 24) as f32)
    }

    pub fn next_f32_signed(&mut self) -> f32 {
        self.next_unit_f32() * 2.0 - 1.0
    }
}

pub(crate) fn pick_index_or_fail(rng: &mut SplitMix64, len: usize) -> MutationResult<usize> {
    rng.pick_index(len).ok_or_else(|| {
        MutationError::op_failed(
            "rng",
            "SplitMix64::pick_index returned None despite non-empty candidates",
            "internal error; report",
        )
    })
}
