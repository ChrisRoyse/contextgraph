// Inspired by ruvnet/RuVector crates/ruvector-graph-transformer/src/temporal.rs at HEAD ef5274c2
// (read 2026-05-08). Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md for the policy.
//
// Per `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §8a, §8d`:
//
//   §8a. Causal masking for the predictor
//     - MaskStrategy::Strict          — node at t attends only to t' < t.
//     - MaskStrategy::TimeWindow{w}   — node at t attends to t' in [t - w, t].
//     - MaskStrategy::CausalWithSelf  — node at t attends to t' <= t (self-inclusive).
//
//   §8d. Retrocausal-safety enforcement: explicit guard that rejects an attention
//   attempt that would violate the causal boundary, so misuse fails closed rather
//   than silently producing leakage. Use at any callsite that consumes attention
//   weights without first running `apply_mask_to_scores`.
//
// The DynamicJEPA predictor uses a sliding window over panel sequences;
// MaskStrategy::TimeWindow with window_size = 4 matches the 4-step trajectory
// format in `backupdocs/5090jepa/09_trajectory_and_dataset.md`.
//
// The mask is a row-major N×N bool grid: `mask[i * n + j] == true` means token
// i is permitted to attend to token j. `apply_mask_to_scores` turns the bool
// mask into the standard transformer additive mask (0 for allowed, NEG_INFINITY
// for forbidden) so a downstream softmax assigns zero weight to forbidden
// positions. We use NEG_INFINITY rather than a "very negative" finite value to
// keep softmax exactly fail-closed: f32::softmax(NEG_INFINITY) == 0.0 by
// definition, no numerical leakage.
//
// Fail-closed everywhere. Length mismatches, NaN scores, retrocausal attempts,
// invalid window sizes — all return SolverError::InvalidInput with a specific
// remediation. No silent fallback to "Strict if invalid window".
//
// Non-finite handling — INPUT vs OUTPUT asymmetry (deliberate):
//   - apply_mask_to_scores  : INPUT scores must be finite (NaN OR ±INFINITY
//                             both reject with CGSOLVER_NUMERICAL_INVARIANT).
//                             It is the ONLY function that writes NEG_INFINITY,
//                             and only into positions the boolean mask marks
//                             forbidden.
//   - softmax_row           : tolerates NEG_INFINITY (treats it as 0 weight,
//                             exactly fail-closed) but still rejects +INFINITY
//                             and NaN. This is what makes the
//                             apply-mask-then-softmax pipeline valid: the only
//                             non-finite value softmax_row will see is the
//                             NEG_INFINITY that apply_mask_to_scores wrote.

use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};

/// Causal masking strategy. Per `docs/ruvectorfindings/04 §8a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskStrategy {
    /// `t' < t` — token at time `t` attends only to STRICTLY-EARLIER tokens.
    /// Diagonal is forbidden (no self-attention). The classic causal triangular
    /// mask used by autoregressive transformers.
    Strict,
    /// `t - window_size <= t' <= t` — token at time `t` attends to tokens
    /// inside the trailing window of `window_size` time units, INCLUDING self.
    /// `window_size = 0` collapses to "attend to self only" — explicitly
    /// allowed (some heads in panel sequences are pure-self projections).
    TimeWindow { window_size: u64 },
    /// `t' <= t` — strictly-earlier OR self. Self-attention permitted; future
    /// forbidden. The default for read-out heads (Stage F) where the head
    /// computes a query against its own historical context.
    CausalWithSelf,
}

/// Build an N×N row-major boolean mask over `timestamps` under `strategy`.
///
/// `mask[i * n + j] == true` ⇔ token `i` may attend to token `j`.
///
/// Errors fail-closed:
/// - `CGSOLVER_INVALID_INPUT` if `timestamps` is empty (no tokens to mask).
pub fn build_causal_mask(timestamps: &[u64], strategy: MaskStrategy) -> SolverResult<Vec<bool>> {
    if timestamps.is_empty() {
        return Err(SolverError::invalid(
            "timestamps",
            "cannot build a causal mask over an empty token sequence",
            "supply at least one timestamp",
        ));
    }
    let n = timestamps.len();
    let len = n.checked_mul(n).ok_or_else(|| {
        SolverError::invalid(
            "timestamps",
            format!("timestamp count overflows N*N mask allocation: n = {n}"),
            "use a smaller token sequence",
        )
    })?;
    let mut mask = vec![false; len];
    for i in 0..n {
        let t_i = timestamps[i];
        for j in 0..n {
            let t_j = timestamps[j];
            let allowed = match strategy {
                MaskStrategy::Strict => t_j < t_i,
                MaskStrategy::CausalWithSelf => t_j <= t_i,
                MaskStrategy::TimeWindow { window_size } => {
                    if t_j > t_i {
                        false
                    } else {
                        // t_i - t_j computed as u64 (we know t_j <= t_i)
                        (t_i - t_j) <= window_size
                    }
                }
            };
            mask[i * n + j] = allowed;
        }
    }
    Ok(mask)
}

/// Apply the boolean mask to a row-major N×N attention-score matrix in-place.
/// Forbidden positions become `f32::NEG_INFINITY`; allowed positions are left
/// untouched. Non-finite input scores are rejected before mutation (`NaN`
/// propagates through softmax; infinities can hide upstream numerical faults).
///
/// Errors fail-closed:
/// - `CGSOLVER_INVALID_INPUT` if `scores` length ≠ `mask` length, or `n^2` ≠ length.
/// - `CGSOLVER_NUMERICAL_INVARIANT` if any score is non-finite.
pub fn apply_mask_to_scores(scores: &mut [f32], mask: &[bool], n: usize) -> SolverResult<()> {
    if n == 0 {
        return Err(SolverError::invalid(
            "n",
            "cannot apply a causal mask with zero tokens",
            "supply at least one token (n >= 1)",
        ));
    }
    let expected = n.checked_mul(n).ok_or_else(|| {
        SolverError::invalid(
            "n",
            format!("token count overflows usize: n = {n}"),
            "use a smaller token count",
        )
    })?;
    if scores.len() != expected {
        return Err(SolverError::invalid(
            "scores",
            format!(
                "scores length {} does not match expected n*n = {} for n = {}",
                scores.len(),
                expected,
                n
            ),
            "pass a row-major N×N score matrix flattened to length N*N",
        ));
    }
    if mask.len() != expected {
        return Err(SolverError::invalid(
            "mask",
            format!(
                "mask length {} does not match expected n*n = {} for n = {}",
                mask.len(),
                expected,
                n
            ),
            "build the mask with build_causal_mask over the same timestamps",
        ));
    }
    for (idx, value) in scores.iter().enumerate() {
        if !value.is_finite() {
            let i = idx / n;
            let j = idx % n;
            return Err(SolverError::invariant(
                "scores",
                format!("score[{i}, {j}] is non-finite: {value}"),
                "scrub NaN/Inf from the upstream attention computation; fail closed rather than masking it",
            ));
        }
    }
    for (idx, allowed) in mask.iter().enumerate() {
        if !allowed {
            scores[idx] = f32::NEG_INFINITY;
        }
    }
    Ok(())
}

/// Retrocausal-safety guard. Per §8d, REJECT an attention attempt from token
/// `query_idx` to token `key_idx` if it crosses the causal boundary defined by
/// `strategy`. This is the explicit fail-closed check callers run BEFORE
/// reading off an attention weight at a specific position when they did not
/// pre-mask the score matrix.
///
/// Use case: per-cell auditing or sparse spot checks (e.g. a single
/// query/key pair surfaced from a verifier trace). For BULK masking over an
/// N×N score matrix, prefer `apply_mask_to_scores` — calling this in an
/// O(N²) loop duplicates work that the bulk path does in one pass and
/// produces an error per offending cell rather than masking them to
/// NEG_INFINITY.
///
/// Returns Ok(()) if attendance is permitted; otherwise CGSOLVER_INVALID_INPUT
/// with `field = "attention_position"` and a message naming the offending
/// (query, key, t_query, t_key) tuple so the caller can log + surface the leak
/// site.
///
/// Errors fail-closed:
/// - `CGSOLVER_INVALID_INPUT` if either index is out of bounds.
/// - `CGSOLVER_INVALID_INPUT` if attendance violates the strategy.
pub fn retrocausal_safety_check(
    timestamps: &[u64],
    query_idx: usize,
    key_idx: usize,
    strategy: MaskStrategy,
) -> SolverResult<()> {
    if timestamps.is_empty() {
        return Err(SolverError::invalid(
            "timestamps",
            "cannot check retrocausal safety over an empty token sequence",
            "supply at least one timestamp",
        ));
    }
    let n = timestamps.len();
    if query_idx >= n {
        return Err(SolverError::invalid(
            "query_idx",
            format!("query_idx = {query_idx} but timestamps length = {n}"),
            "pass a query_idx strictly less than timestamps.len()",
        ));
    }
    if key_idx >= n {
        return Err(SolverError::invalid(
            "key_idx",
            format!("key_idx = {key_idx} but timestamps length = {n}"),
            "pass a key_idx strictly less than timestamps.len()",
        ));
    }
    let t_q = timestamps[query_idx];
    let t_k = timestamps[key_idx];
    let allowed = match strategy {
        MaskStrategy::Strict => t_k < t_q,
        MaskStrategy::CausalWithSelf => t_k <= t_q,
        MaskStrategy::TimeWindow { window_size } => t_k <= t_q && (t_q - t_k) <= window_size,
    };
    if allowed {
        Ok(())
    } else {
        Err(SolverError::invalid(
            "attention_position",
            format!(
                "retrocausal-safety violation: query_idx = {query_idx} (t = {t_q}) attempted to attend to key_idx = {key_idx} (t = {t_k}) under strategy {strategy:?}"
            ),
            "pre-mask the attention score matrix with apply_mask_to_scores or restrict key_idx to the causal cone",
        ))
    }
}

/// Convenience: numerically-stable softmax of a row, treating NEG_INFINITY as
/// "exactly zero weight" and NaN as a fail-closed error. Used by the FSV
/// example to demonstrate that masked positions get exactly 0 post-softmax.
///
/// Errors fail-closed:
/// - `CGSOLVER_NUMERICAL_INVARIANT` if any score is NaN.
/// - `CGSOLVER_NUMERICAL_INVARIANT` if every score in the row is NEG_INFINITY
///   (a fully-masked row would produce 0/0; impossible to define a softmax).
pub fn softmax_row(scores: &[f32]) -> SolverResult<Vec<f32>> {
    if scores.is_empty() {
        return Err(SolverError::invalid(
            "scores",
            "cannot softmax an empty row",
            "supply at least one score",
        ));
    }
    for (j, value) in scores.iter().enumerate() {
        if value.is_nan() {
            return Err(SolverError::invariant(
                "scores",
                format!("score[{j}] is NaN"),
                "scrub NaN from the upstream attention computation",
            ));
        }
        if value.is_infinite() && value.is_sign_positive() {
            return Err(SolverError::invariant(
                "scores",
                format!("score[{j}] is +inf"),
                "scrub +inf from the upstream attention computation",
            ));
        }
    }
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !max.is_finite() {
        return Err(SolverError::invariant(
            "scores",
            "every score is NEG_INFINITY: cannot define softmax over a fully-masked row",
            "ensure at least one position is unmasked",
        ));
    }
    let exps: Vec<f32> = scores
        .iter()
        .map(|s| if s.is_finite() { (s - max).exp() } else { 0.0 })
        .collect();
    let sum: f32 = exps.iter().sum();
    if sum <= 0.0 {
        return Err(SolverError::invariant(
            "scores",
            format!("softmax denominator non-positive: {sum}"),
            "verify scores were not all NEG_INFINITY before softmax",
        ));
    }
    Ok(exps.into_iter().map(|e| e / sum).collect())
}

#[cfg(test)]
#[path = "causal_mask_tests.rs"]
mod causal_mask_tests;
