---
namespace: reports
agent_id: agent-200-coordinator
timestamp: 2026-05-08T23:50:00Z
status: active
tags: tier1, mmr, conformal, ed25519, clean-room, fsv, final-report
---

# Final Report — Tier-1 Foundation Gaps (Generic MMR + Conformal + Ed25519)

## Task Completed: yes

## Summary

Shipped three clean-room Tier-1 foundation primitives (per
`docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` Tier 1.1, 1.4, 1.6)
required by the ME-JEPA-Code reality compiler (per
`docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §4.2, §5.5):

1. **Generic MMR diversity reranker** — `crates/context-graph-solver/src/mmr.rs`.
   Closure-based + matrix-based selection paths, [0,1]-validated similarity,
   stable tiebreak on index, fail-closed on NaN/Inf/out-of-range inputs.
2. **Conformal predictor (split / inductive)** — `crates/context-graph-solver/src/conformal.rs`.
   Three nonconformity measures (Distance, NormalizedDistance, Rank),
   finite-sample 1-α coverage guarantee with `k = ceil((n+1)*(1-α))`-th order
   statistic, fail-closed before calibration, `empirical_coverage` diagnostic
   for FSV.
3. **Ed25519 segment signing** — `crates/context-graph-witness/src/sign.rs`
   behind a non-default `ed25519` Cargo feature. 96-byte codec
   (32 pubkey + 64 sig), domain-separated digest
   (`context-graph-witness-segment-v1` || pubkey || segment), `verify_segment`,
   `verify_segment_with_expected_pubkey`, `WitnessKeypair` with custom Debug
   that redacts the signing key, `zeroize`-on-drop SigningKey via the
   ed25519-dalek `zeroize` feature, two-tier fail-closed defense
   (chain-integrity gate runs BEFORE ed25519).

The code-simplifier subagent reviewed all three modules and surfaced 14
findings (0 HIGH, 5 MEDIUM, 9 LOW). I personally verified each finding before
applying refinements. Applied: M1 (delete dead `CalibrationScore`),
M2 (delete dead `SignatureMalformed` variant), M3 (drop redundant
NEG_INFINITY sentinel), M5 (lift Rank rejection to top of `empirical_coverage`),
L1 (`MmrConfig::default` doc), L3 (`ConformalConfig` doc), L4 (`config()` /
`is_calibrated()` docs), L8 (`public_key_bytes` / `SignError::code` docs),
L9 (bind `public_key_bytes()` once in `sign_segment`). Skipped: M4
(defense-in-depth at trivial cost), L2/L5/L6/L7 (cosmetic / nicety).

## Files Created or Modified in the Repo

### Created (clean-room, with provenance headers citing RuVector ef5274c2)

- `crates/context-graph-solver/src/mmr.rs` — generic MMR module (~440 lines incl. tests).
- `crates/context-graph-solver/src/conformal.rs` — conformal predictor (~560 lines incl. tests).
- `crates/context-graph-witness/src/sign.rs` — ed25519 signing (~420 lines incl. tests).
- `crates/context-graph-solver/examples/tier1_fsv.rs` — FSV evidence-producer for MMR + conformal.
- `crates/context-graph-witness/examples/ed25519_fsv.rs` — FSV evidence-producer for ed25519.

### Modified

- `crates/context-graph-solver/src/lib.rs` — added `pub mod {mmr, conformal}` + re-exports.
- `crates/context-graph-witness/src/lib.rs` — added `#[cfg(feature = "ed25519")] pub mod sign` + re-exports.
- `crates/context-graph-witness/Cargo.toml` — added `ed25519` feature flag, `ed25519-dalek` (with `zeroize` sub-feature) and `zeroize` dependencies, `serde_json` dev-dependency, `[[example]]` declaration with `required-features = ["ed25519"]`.

## Files Written to ./memory/

- `context/tier1-foundation-gaps-initiative.md` — initiative scope, what shipped, what was deferred.
- `decisions/agent-200-coordinator--tier1-module-placement.md` — locked decision: MMR + conformal in `context-graph-solver`, ed25519 signing in `context-graph-witness`, plus the rule for future signing schemes (ML-DSA-65, SLH-DSA-128s).
- `reports/code-simplifier-tier1-review--final-report.md` — full code-simplifier findings (14 items).
- `reports/agent-200-coordinator--tier1-foundation-gaps-final-report.md` — this report.

## Decisions Made

- **Module placement**: MMR + conformal land in `context-graph-solver` rather than
  a new `context-graph-retrieval` crate. Rationale: doc 06 said "new module under
  existing retrieval crate" but no retrieval crate exists; creating one for two
  ~300-LOC modules is over-engineering. Future moves are re-export edits.
  See `memory/decisions/agent-200-coordinator--tier1-module-placement.md`.
- **Defense-in-depth on tampered-segment**: kept `validate_segment` calls in
  both `verify_segment` and `digest_for_segment` despite code-simplifier M4.
  The chain-integrity gate firing before ed25519 means tampered prev_hash
  bytes are caught with a more-specific error code
  (`WITNESS_SIGNATURE_SEGMENT_PREV_HASH_MISMATCH`) rather than a generic
  `WITNESS_SIGNATURE_VERIFICATION_FAILED`. Better telemetry for ~zero perf
  cost at typical chain sizes.
- **Skipped BMSSP and 8/7/5/3-bit tier compression in this turn**: per
  operator-confirmed scope ("Tier 1 foundation gaps" with "Stop at strong,
  fully-tested boundary"). Deferred items are documented in
  `memory/context/tier1-foundation-gaps-initiative.md`. BMSSP must be a real
  V-cycle multigrid implementation, not a CG wrapper.

## Discoveries

- **`ed25519-dalek` 2.x's `Signature::from_bytes` is infallible** after a
  64-byte length check. Originally I declared a `SignatureMalformed` error
  variant — code-simplifier flagged it as dead code; I verified and removed
  it. Lesson: verify error variants are reachable before merging.
- **The linter actively strengthens validation across edits**. Multiple times
  during this turn the linter expanded `validate_config` / `validate_segment`
  / similarity-range checks. The expansions were correct improvements; I
  accepted them and re-ran tests.
- **FSV examples must declare `required-features` in Cargo.toml** when they
  depend on a non-default feature, otherwise `cargo run --example NAME` fails
  with `unresolved import` because the feature isn't auto-enabled.
- **The chain-integrity gate (linter-added in `validate_segment`) catches
  tampered prev_hash bytes BEFORE ed25519 verification**. This means
  ed25519-only tests of "tamper a byte; verify rejects" need to target a byte
  the chain check cannot detect — e.g. action_hash of the LAST entry. Two-tier
  fail-closed defense in depth, but tests must be specific about which gate
  they're exercising.

## Failures

- **First FSV run had wrong tampering target**: I tampered byte 10 (in entry
  0's prev_hash) expecting `WITNESS_SIGNATURE_VERIFICATION_FAILED` but got
  `WITNESS_SIGNATURE_SEGMENT_PREV_HASH_MISMATCH`. The ed25519 gate never
  fired because the chain-integrity gate caught it first. Lesson: when
  testing a specific gate, tamper a byte that ONLY that gate can see.
  Updated FSV to test BOTH gates explicitly.
- **`SolverError::invalid` was missing the `with_context_owned` helper my
  initial MMR draft expected**. Fixed by inlining the context into the
  `format!` argument of `message`. Lesson: verify helper APIs exist before
  using them in new code.

## Successes

- **All 49 tests pass** (31 solver + 18 witness with ed25519 feature).
- **Physical FSV evidence on disk** at `/tmp/contextgraph-tier1-fsv/` —
  219-byte segment (73×3), 96-byte codec (32 pubkey + 64 sig), 32-byte pubkey,
  pubkey hex `332ebe8d27cb7323b3a401c1c13b5dd64bccc0e10ecda1c2b5d11a03779a85e5`,
  empirical coverage 0.9 ±0.05 of stated 0.9 (matches conformal's
  finite-sample guarantee).
- **Two-tier defense in depth verified**: tampered prev_hash → chain gate;
  tampered last action_hash → ed25519 gate; tampered signature → ed25519
  gate. Each path produces a distinct, telemetry-grade error code.
- **Custom `Debug` impl on `WitnessKeypair`**: prints only the public key
  hex, redacts the signing key. Logs and panic messages cannot leak signing
  material.

## Blockers (if any)

None.

## Tests / Build

- `cargo test --release -p context-graph-solver` → **31/31 pass** (0 ignored).
  - 13 MMR tests (including all boundaries: NaN, λ out of range, similarity
    out of range, empty input, limit=0, etc.)
  - 11 conformal tests (alpha=0/1 rejected, uncalibrated rejected, three
    nonconformity measures, empirical coverage within 0.05 of stated, etc.)
  - 7 existing PageRank/CG/CSR tests (unaffected).
- `cargo test --release -p context-graph-witness --features ed25519` → **18/18 pass**.
  - 15 sign tests (roundtrip, tampered byte, tampered signature, pubkey
    swap, pinned pubkey mismatch, weak key rejection, empty/truncated/
    broken-link segment rejection, etc.)
  - 3 existing chain tests (unaffected).
- `cargo build --release` → workspace builds clean.
- FSV examples: both produce evidence files; all expected/actual codes and
  values match.

## Source-of-truth files (FSV evidence)

- `file:/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json` (2344 bytes)
  — MMR + conformal expected-vs-actual evidence.
- `file:/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json` (1894 bytes)
  — ed25519 + chain-validation expected-vs-actual evidence.
- `file:/tmp/contextgraph-tier1-fsv/witness-segment.bin` (219 bytes = 73×3)
  — raw signed witness segment (3 chained 73-byte entries).
- `file:/tmp/contextgraph-tier1-fsv/witness-segment.sig` (96 bytes)
  — ed25519 signature codec.
- `file:/tmp/contextgraph-tier1-fsv/witness-segment.pub` (32 bytes)
  — public key (hex `332ebe8d27cb7323b3a401c1c13b5dd64bccc0e10ecda1c2b5d11a03779a85e5`).

## For the Next Agent

- **Read first**: `./memory/context/tier1-foundation-gaps-initiative.md`
  for the still-pending Tier-1 work (BMSSP V-cycle, 8/7/5/3-bit tier
  compression). And `./memory/decisions/agent-200-coordinator--tier1-module-placement.md`
  for where new related modules should live.
- **Watch out for**: the linter may expand validation rules during edits.
  Re-run tests after every edit pass. Never assume the file is the same as
  what you wrote 30 seconds ago.
- **Priority**: BMSSP V-cycle is the next-highest-leverage Tier-1 item — it
  unblocks Laplacian solves over harness-transition graphs (per doc 03 §1b).
  MUST be a real algebraic-multigrid V-cycle, not a CG wrapper. Plan for
  ~3-5 days. Reference: `ruvector/crates/ruvector-solver/src/bmssp.rs` HEAD `ef5274c2`.
- **Integration opportunity**: `optimizer_recall_recommendations` in
  `context-graph-mcp/src/handlers/tools/reality_loop/diversity.rs` could now
  delegate to the generic `context_graph_solver::mmr_select` — currently
  duplicates the algorithm. Refactor in a focused turn after BMSSP lands
  (so we don't conflate two unrelated changes).

## Confidence: high

All three modules ship with comprehensive boundary tests, physical FSV
evidence on disk, and a code-simplifier review with no high-severity
findings. The clean-room rule is honored — every new file has a provenance
header citing the RuVector source it was inspired by; no code copied; no
path-deps; no ruvnet npm packages. The ed25519 feature is non-default so
existing callers are unaffected.

The conformal predictor passes the textbook empirical-coverage test (0.9
within 0.05 of stated 0.9 on 200/400 calibration/test split). The ed25519
signing covers a real 219-byte witness segment, signs it, writes to disk,
re-reads it, re-verifies — proving the codec is durable across IO. The MMR
reranker correctly handles all three λ regimes (1.0 = pure relevance, 0.0
= pure diversity, 0.3 = balanced) with deterministic tie-breaking.
