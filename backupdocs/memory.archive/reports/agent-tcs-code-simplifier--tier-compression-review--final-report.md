---
namespace: reports
agent_id: agent-tcs-code-simplifier
timestamp: 2026-05-08T00:00:00Z
status: active
tags: tier-compression, review, code-simplifier
---

# context-graph-tier-compression — code-simplifier review

## Verdict

**PASS** with three optional **REFINEMENT-RECOMMENDED**. No BLOCKERS.

The crate is correct, tight, and well-documented. 21 tests + 4-width FSV
already cover the algorithmic surface. The bit-packing and endpoint
preservation both withstand close inspection.

## Priority 1 — bit packing correctness: CORRECT

Encode (lib.rs:217-224): max `bits_in_acc` reached before a flush is
`7 (residue) + 8 (max n_bits) = 15`; OR-in of an 8-bit `q` keeps `acc`
within ~22 used bits, far below u64. The post-loop `if bits_in_acc > 0`
flushes the final partial byte. At `bits=8` each iteration pushes exactly
one byte; at `bits=3` the residue stays in `[0,7]`. No off-by-one, no
overflow. Decode (lib.rs:288-303) is the structural mirror.

## Priority 2 — endpoint preservation: PROVED

For `v == min`: `(v-min)/range = 0 → q.round()=0 → v_hat = min + 0 = min`.
The f64→f32 cast at lib.rs:307 widens the f32 `min` losslessly, adds zero,
and narrows back. Exact.

For `v == max`: `(max-min)/range = 1.0 → q = max_q → v_hat =
(min as f64) + ((max-min) as f64) = (max as f64) → cast back yields
exactly `max`. Both endpoints round-trip bit-for-bit at every supported
width. Tests (`round_trip_preserves_endpoints_at_every_width`,
lib/tests.rs:48-59) confirm.

## Priority 3 — header / magic / version: OK with one caveat

22-byte header is fine and self-describing. **One small risk**: at lib.rs:366
`u64 → as usize` truncates silently on 32-bit hosts. ContextGraph is x86_64
only today; defense-in-depth fix is two lines. See discovery
`agent-tcs-code-simplifier--tier-compression-n-truncation-32bit.md`.

## Priority 4 — NaN/Inf re-check on decode: ALREADY DONE

lib.rs:242-250 explicitly rejects non-finite `min`/`max` from the
deserialized blob before the `min > max` check. Correct ordering.

## Priority 5 — other observations

- Encoder walks `values` twice (is_finite, then min/max). Single fused pass
  is shorter and halves cache pressure. Discovery
  `agent-tcs-code-simplifier--tier-compression-double-finite-scan.md`.
- `packed_len` (lib.rs:399-401) multiplies untrusted `n * bits` without
  `checked_mul`. For embedding-sized n it cannot overflow, but `deserialize`
  calls it on an on-disk-controlled `n` so a `checked_mul` guard is the
  more honest contract. Discovery
  `agent-tcs-code-simplifier--tier-compression-packed-len-overflow.md`.
- `BitWidth::bits(&self)` and `levels(&self)` could be `pub const fn
  bits(self)` since the enum is Copy. Trivia.
- `mask` special-case `if n_bits == 64` (lib.rs:277-281) is unreachable for
  the four supported widths. Defensive and free; no need to remove.
- `BitWidth::from_raw` enumerates 8/7/5/3 explicitly — better than
  `try_from` magic. Keep as-is.

## Counts

- PASS: 1 (the crate as a whole)
- REFINEMENT-RECOMMENDED: 3 (the three discoveries above)
- BLOCKER: 0

## Memory paths

- `memory/discoveries/agent-tcs-code-simplifier--tier-compression-n-truncation-32bit.md`
- `memory/discoveries/agent-tcs-code-simplifier--tier-compression-double-finite-scan.md`
- `memory/discoveries/agent-tcs-code-simplifier--tier-compression-packed-len-overflow.md`
- `memory/reports/agent-tcs-code-simplifier--tier-compression-review--final-report.md`
