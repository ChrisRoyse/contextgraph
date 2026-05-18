---
namespace: reports
agent_id: code-simplifier-tier1
timestamp: 2026-05-08T23:15:00Z
status: active
tags: tier1, review, mmr, conformal, ed25519
---

# Code-Simplifier Review — Tier-1 Foundation Modules

Review of three Tier-1 modules shipped this turn:

1. `crates/context-graph-solver/src/mmr.rs` (~390 lines)
2. `crates/context-graph-solver/src/conformal.rs` (~430 lines)
3. `crates/context-graph-witness/src/sign.rs` (~440 lines)

Plus the `lib.rs` re-export changes and `Cargo.toml` feature gate.

Verification status accepted as given (30/30 + 18/18 tests, FSV evidence on
disk). This review is read-only — no code edits were made. The findings below
are scoped strictly to the six files listed in the prompt.

Overall verdict: **all three modules are correct, well-tested, and idiomatic
clean-room implementations**. The findings are simplifications, dead code,
minor consistency nits, and a couple of real-but-low-impact bugs in error
classification. Nothing high-severity; the high-severity items I list are
"high" only in the relative sense (most likely to cause confusion downstream),
not "high" in the audit sense of "ships broken."

---

## Findings by severity

### HIGH — 0
None. There are no correctness bugs, no missing validation paths, no fail-open
gaps, no security issues.

### MEDIUM — 5

#### M1. `conformal.rs:60-62` — `CalibrationScore` is dead code
- **Issue**: `pub struct CalibrationScore { pub score: f64 }` is declared but
  never used in any function signature, never re-exported from `lib.rs`, and
  never constructed in tests. The actual calibration API takes `&[f64]` directly.
- **Severity**: medium (dead public API surface — confusing for callers reading
  the docs, suggests a richer API than exists).
- **Fix (one line)**: delete `pub struct CalibrationScore` and its derives
  (lines 58–62 inclusive).

#### M2. `sign.rs:59,85` — `SignError::SignatureMalformed` is dead code
- **Issue**: The variant is declared (line 59) and given a code string
  (line 85) but is never constructed anywhere. `ed25519-dalek` 2.1's
  `Signature::from_bytes` takes `&[u8; 64]` and is infallible after the
  length check we already perform.
- **Severity**: medium (advertises a failure mode that cannot occur — telemetry
  consumers will never see this code).
- **Fix (one line)**: delete the `SignatureMalformed` variant from the enum and
  its match arm in `code()`. Update tests if any reference it (none do per grep).

#### M3. `mmr.rs:142` — sentinel clause `best.mmr_score == f64::NEG_INFINITY` is provably redundant
- **Issue**: With finite `relevances` (validated lines 91–99) and similarities
  in `[0,1]` (validated lines 119–128), `mmr_score = λ*rel - (1-λ)*sim` is
  always finite. The first iteration's `best.mmr_score == NEG_INFINITY` so
  `mmr_score > best.mmr_score` is always true. The third clause never fires.
- **Severity**: medium (unnecessary defense that obscures the algorithmic
  invariant; small clarity win).
- **Fix (one line)**: drop the third disjunct, leaving
  `take = mmr_score > best.mmr_score || (mmr_score == best.mmr_score && idx < best.index)`.

#### M4. `sign.rs:163-186, 192-213` — codec/segment validation duplicated across the two verify paths
- **Issue**: `verify_segment_with_expected_pubkey` checks codec length, then
  calls `verify_segment` which checks codec length AGAIN; both call
  `validate_segment` once and then `digest_for_segment` which calls
  `validate_segment` a SECOND time. So a verify pass walks the segment's
  prev-hash chain twice and re-parses every entry twice.
- **Severity**: medium (correctness fine; one extra O(n·shake256) pass per verify.
  At n=1 segments today this is negligible; at the 100s of entries the chain
  reaches in long-running optimizers, it doubles verify cost).
- **Fix (one line each)**: in `digest_for_segment`, drop the `validate_segment`
  call (callers always validate first) OR remove the call from `verify_segment`
  and rely on the digest path. Document which one is the canonical entry point.

#### M5. `conformal.rs:302-347` — `empirical_coverage` defers `Rank` rejection until inside the per-element loop
- **Issue**: For `NonconformityMeasure::Rank`, `empirical_coverage` returns the
  "Rank coverage requires per-example ranks" error from inside the truth-distance
  loop (line 334-340). With an empty `truth_distances` slice the empty-check
  fires first; with a non-empty slice the function does one f64 finite check
  before erroring. Cleaner: reject `Rank` at the top of the function before any
  iteration.
- **Severity**: medium (correctness fine, ergonomics minor — error message will
  arrive regardless, but lifting the check makes the function structure read as
  "validate, then iterate" instead of "iterate, validate per element").
- **Fix (one line)**: add a top-level `if matches!(self.config.measure, Rank)`
  guard immediately after the threshold check.

### LOW — 9

#### L1. `mmr.rs:30-37` — `MmrConfig::default { lambda: 0.5, limit: 10 }` magic numbers undocumented
- **Issue**: The defaults are reasonable (λ=0.5 balanced, limit=10 typical
  retrieval k) but neither the docstring on `MmrConfig` nor the `Default` impl
  explains the choice. A future caller reading
  `MmrConfig::default()` won't know if 10 is meaningful or arbitrary.
- **Fix (one line)**: add `/// Default: λ=0.5 (balanced), limit=10 (typical retrieval k).`
  above the `Default` impl.

#### L2. `mmr.rs:104-110` — `best_pos = 0usize` initial value is dead
- **Issue**: On the first iteration of the inner loop, the `take` branch fires
  unconditionally (per M3 analysis), so `best_pos = 0` is overwritten on the
  first pass through. Initializing to a sentinel value (or unwrapping after
  the loop) would document that "best_pos is meaningful after the loop."
- **Severity**: low (style; not a bug — the value is correctly overwritten).
- **Fix (one line)**: leave as-is OR refactor to `Option<usize>` with a final
  `expect("remaining is non-empty")`.

#### L3. `conformal.rs:41-46` — `ConformalConfig` struct missing top-level doc comment
- **Issue**: Fields have inline docs but the struct itself doesn't say what it
  bundles. Inconsistent with `MmrConfig` (which has a top-level doc).
- **Fix (one line)**: add `/// Configuration for the split conformal predictor.`
  above the struct.

#### L4. `conformal.rs:126,130` — `config()` and `is_calibrated()` accessors missing doc comments
- **Issue**: Public API methods without docs; readers depending on rustdoc see
  bare signatures.
- **Fix (one line each)**: brief one-line `///` comment.

#### L5. `conformal.rs:107` — alpha range check has a redundant clause
- **Issue**: `!(0.0..1.0).contains(&config.alpha) || config.alpha == 0.0` —
  `(0.0..1.0)` is a half-open Range whose `.contains(&0.0)` returns `true`,
  so `!contains(&0.0)` is `false` and we fall through to the explicit
  `config.alpha == 0.0` clause. Functionally correct but tortured. Cleaner:
  `!(0.0 < config.alpha && config.alpha < 1.0)` or use exclusive range
  `(0.0..1.0)` with `>` semantics.
- **Severity**: low (correctness intact; readability win).
- **Fix (one line)**: replace the conditional with
  `!(config.alpha.is_finite() && 0.0 < config.alpha && config.alpha < 1.0)`.

#### L6. `conformal.rs:67-77, ConformalReport` could derive `Copy`
- **Issue**: All four fields are `Copy` (`usize`, `f64`, `usize`, `f64`).
  `ConformalConfig` derives `Copy` already (line 40), so for consistency
  `ConformalReport` could too. Callers currently must `.clone()` to keep a
  copy when the predictor borrows it.
- **Fix (one line)**: add `Copy` to the `derive` list.

#### L7. `conformal.rs:232-238, 261-266, 304-309` — `field` argument abuses `SolverError::invalid` semantics
- **Issue**: The `SolverError::invalid` constructor takes `field: &'static str`
  meant to identify the input field that failed validation. These call sites
  pass function names (`"predict_set"`, `"empirical_coverage"`) instead of an
  actual field. Telemetry consumers grouping errors by `field` will see
  function-named "fields" mixed in with real ones.
- **Severity**: low (it works; it's a naming hygiene issue).
- **Fix (one line each)**: pass an actual field — e.g. `"calibration"` for the
  uncalibrated case, `"distances"` for length-mismatch, `"truth_distances"` for
  the coverage path.

#### L8. `sign.rs:79,124` — `WitnessKeypair::public_key_bytes` and `SignError::code` missing doc comments
- **Issue**: Public-API methods bare of `///` blocks. Inconsistent with the
  generous docs elsewhere in `sign.rs`.
- **Fix (one line each)**: brief `///` comment.

#### L9. `sign.rs:131-138` — `public_key_bytes()` called twice in `sign_segment`
- **Issue**: Lines 132 and 135 each call `self.public_key_bytes()`, which
  copies 32 bytes out of the `VerifyingKey`. Trivial cost, but two calls when
  one binding would suffice.
- **Severity**: low (perf is irrelevant at 32 bytes; it's a clarity nit).
- **Fix (one line)**: bind `let pk = self.public_key_bytes();` once and reuse.

---

## Items deliberately NOT flagged (and why)

- **`WitnessKeypair` storing both `signing` and `verifying`** (`sign.rs:100-103`):
  the verifying key is derivable from the signing key, but caching avoids
  recomputing the curve point on every `public_key_bytes()` call. The memory
  cost is 32 bytes; the simplification "drop verifying, derive on demand" would
  hurt the hot path. Keep as-is.

- **`validate_segment` calls `from_bytes` defensively** (`sign.rs:245-250`):
  after the modulo check, each chunk is exactly 73 bytes so `from_bytes`
  cannot fail. The `.map_err(...)` is unreachable. Removing it would either
  use `unwrap` (panicky) or `expect` (also panicky). Defensive `map_err` is
  the right call; flagging this as dead code would worsen the contract.

- **`MmrSelection` field-per-item vs `PredictionSet` column-major shape**:
  inconsistent in shape but each is right for its domain — MMR consumers want
  per-pick provenance (score components per selection); conformal consumers
  want indexable parallel arrays. Don't unify.

- **`CalibrationScore` could be argued as "future-proofing" the API**: it isn't.
  If/when the calibrate API takes a richer struct, the field set will change;
  adding it now without using it locks us in. Better to delete (M1) and
  reintroduce when needed.

- **`select`'s `O(k·n)` similarity-call pattern**: re-asking the closure for
  `(idx, chosen.index)` on every outer iteration is the standard greedy-MMR
  cost. Caching `max_sim` per remaining candidate would change the algorithm's
  memory profile from O(k) to O(n). Worth doing only if a profile shows it
  matters.

- **`Cargo.toml` ed25519 feature**: gating ed25519-dalek + zeroize behind
  `ed25519` is correct. The `zeroize` feature on `ed25519-dalek` (line 12)
  is what gives `SigningKey` its zeroize-on-drop guarantee — verified.

---

## Summary

| Severity | Count |
|----------|-------|
| HIGH     | 0 |
| MEDIUM   | 5 |
| LOW      | 9 |
| **Total**| **14** |

All findings are simplifications or hygiene fixes. No correctness bugs, no
fail-open gates, no security regressions. The modules ship in a state worth
merging; the M-tier items (especially M1 and M2 dead-code removals and M3
sentinel cleanup) are worth a short follow-up turn.

## Source-of-truth files referenced
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/mmr.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/conformal.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/lib.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/error.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/examples/tier1_fsv.rs
- file:/home/cabdru/contextgraph/crates/context-graph-witness/src/sign.rs
- file:/home/cabdru/contextgraph/crates/context-graph-witness/src/lib.rs
- file:/home/cabdru/contextgraph/crates/context-graph-witness/Cargo.toml
- file:/home/cabdru/contextgraph/crates/context-graph-witness/examples/ed25519_fsv.rs
