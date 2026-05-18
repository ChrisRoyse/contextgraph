---
namespace: reports
agent_id: code-simplifier-tier2
timestamp: 2026-05-08T23:55:00Z
status: active
tags: tier2, review, cert, mincut, granger
---

# Code-Simplifier Review — Tier-2 Foundation Modules

Review of three Tier-2 modules shipped this turn:

1. `crates/context-graph-witness/src/cert.rs` (~620 lines)
2. `crates/context-graph-mincut/src/lib.rs` (~440 lines)
3. `crates/context-graph-solver/src/granger.rs` (~570 lines)

Plus the `lib.rs` re-export changes, the new `Cargo.toml` for mincut, and
the three FSV evidence-producer examples.

Verification status accepted as given (solver=42/42, witness=20/20 default
+ 35/35 ed25519, mincut=14/14, FSV evidence at /tmp/contextgraph-tier2-fsv).
Stoer-Wagner Figure-1 = 4.0 (textbook), F(2,30)@3.32 ≈ 0.0498 (textbook).
Read-only review — no code changes made.

Overall verdict: **all three modules are correct, idiomatic, well-tested
clean-room implementations**. Two findings rise to HIGH because they affect
the integrity story the certs are sold on (binding gap) and the silent-
fallthrough story `betacf` could trigger under degenerate F-statistic
inputs. No fail-open gates against the inputs that are actually validated;
no algorithmic correctness bugs against well-formed inputs.

---

## Findings by severity

### HIGH — 2

#### H1. `cert.rs:260-330` — `Stage0Certificate` and `ReadoutCertificate` do NOT bind their identifying metadata
- **Issue**: `Stage0Certificate` carries `corpus_id`, `policy_version`. `ReadoutCertificate` carries `query_id`, `head_id`, `policy_version`, `latency_ms`. None of these fields are mixed into the manifest hash (line 400-410, only `kind` + `content_hash` are hashed) nor into the witness chain entry's `action_hash` (line 284 uses `manifest_hash` only; line 347 uses `trajectory_root` only). An attacker (or a buggy serializer) can mutate any of these fields on a serialized cert and `verify()` will accept the tampered version, because nothing recomputes from those fields. The `policy_version` field in particular is the legal-gate version per `04 §5`; silently re-signed under a different policy version is a defeat of the entire Stage 0 use case.
- **Severity**: high — undermines the binding contract these certs claim. The certs verify the *artifact set* and *trajectory set* but not the *frame around them*.
- **Fix (one line of design, several lines of code)**: extend `compute_artifact_manifest_hash` to bind `corpus_id || policy_version || count || ...` (with length prefixes per existing pattern) and add an analogous `compute_readout_binding_hash(query_id, head_id, policy_version, latency_ms, trajectory_root)` that becomes the action_hash for ReadoutCertificate's witness entry. Both verifies must recompute and compare.

#### H2. `granger.rs:367` — `betacf` silently returns the partial value on non-convergence
- **Issue**: After 200 iterations, if `(del - 1.0).abs() < eps` was never satisfied, the function falls off the loop and returns the last `h`. There is no error path. Callers downstream (`regularized_incomplete_beta` → `f_distribution_upper_tail` → `granger_test.p_value`) consume that garbage as a real probability. For typical Granger inputs (small a, b in low-tens range, x in (0,1)) Lentz converges in well under 200 iter, but the docstring "Lentz's modified method to avoid underflow" is *the safety contract* the function makes — and silently returning unconverged is the textbook anti-pattern Numerical Recipes warns against ("if `MAXIT` is exceeded, throw").
- **Severity**: high — fail-closed semantic broken. A pathological caller (very small df_num, very large F-statistic where x = (d1*f)/(d1*f+d2) sits very close to 1) could surface a silently-wrong p-value and the test-stack would never know.
- **Fix (one line)**: change `betacf` return type to `SolverResult<f64>`; on loop exit without convergence return `Err(SolverError::DidNotConverge { ... })`. Propagate up through `regularized_incomplete_beta` and `f_distribution_upper_tail`. The two existing call sites already use `SolverResult`.

### MEDIUM — 8

#### M1. `granger.rs:94, 146-155` — `min_required = 2*lag+2` is too lenient; the inner `df_den == 0` rejection is reachable
- **Issue**: The function docstring (line 70-72) says callers need `≥ 2*lag+2` observations because `n_observations - 2*lag - 1 ≥ 1`. The math: `n = t - lag`, so `n ≥ 2*lag + 2` ⟹ `t ≥ 3*lag + 2`. But line 94 checks `t < 2*lag + 2`, accepting `t = 2*lag + 2` which gives `n = lag + 2` and `df_den = -lag + 1`. For `lag = 1`, that's `df_den = 0` ⟹ inner branch fires (line 147-155). For `lag = 2`, that's an underflow that the `if n > 2*p+1` guard saves. Boundary inputs that pass the front check get rejected by the inner check with a less-helpful error message, and the precondition stated in the doc string doesn't match the code.
- **Severity**: medium (correctness intact; both paths reject, but the front-check error message is "Granger requires ≥ 2*lag+2 = N observations" which is wrong — `2*lag+2` is needed *for n*, not *for t*).
- **Fix (one line)**: change `let min_required = 2 * lag + 2;` to `let min_required = 3 * lag + 2;` and update the error message to say "Granger requires ≥ 3*lag+2 observations" (or, more usefully, "≥ 2*lag+2 lagged rows"). The inner df_den == 0 check then becomes unreachable and can be deleted.

#### M2. `cert.rs:310-313, 378-381` — error context from `verify_chain_bytes` is dropped
- **Issue**: Both Stage0 and Readout `verify()` use `match verify_chain_bytes(...) { Ok(r) => Ok(r), Err(_) => Err(WitnessChainInvalid { offset: 0 }) }`. The original `WitnessError`'s offset and code (e.g. `WITNESS_PREV_HASH_MISMATCH` at offset 5) are erased; the caller sees `CERT_WITNESS_CHAIN_INVALID` with offset 0 regardless of where in the chain the failure occurred.
- **Severity**: medium (correctness fine; debuggability significantly worse for chains with multiple entries).
- **Fix (one line)**: include the underlying `WitnessError` as a `#[source]` on `WitnessChainInvalid`, OR extract the offset from the underlying error before wrapping. The variant already has an `offset` field — populate it.

#### M3. `granger.rs:284-292` — `f_distribution_upper_tail(NaN | INFINITY, ...)` returns 1.0 instead of NaN/0.0
- **Issue**: Line 285 short-circuits any non-finite or non-positive `f` to `return 1.0`. For `f = NaN`, returning `1.0` masks the NaN from the caller. For `f = INFINITY`, the mathematically correct upper-tail is 0.0, not 1.0. The internal `granger_test` path handles INFINITY correctly *outside* the helper (line 167-170), which means the public helper's behavior diverges from the internal use. A direct caller of `f_distribution_upper_tail` will get a different answer than `granger_test` would.
- **Severity**: medium (a public API inconsistency; correctness for the typical Granger flow is intact via the wrapping in granger_test).
- **Fix (one line)**: split the early returns: `if f.is_nan() { return f; }` (NaN through), `if f.is_infinite() { return 0.0; }` (correct upper-tail for ∞), `if f <= 0.0 { return 1.0; }` (F=0 ⟹ p=1). Then `granger_test` can stop guarding line 166-170 and just call the helper.

#### M4. `mincut/lib.rs:178-205` — `prev_last_set` flag is dead; the `if !prev_last_set` branch is unreachable
- **Issue**: The flag is set inside the inner loop (line 197), and the inner loop runs `active_count - 1` times where `active_count = n - phase >= 2` for any reachable phase (phase loop runs `0..n-1`, so at phase `p`, `active_count = n - p >= 2`). The loop runs at least once, so `prev_last_set` is always `true` at line 238. The `prev_last = start;` reset never executes. The comment at 239-243 hypothesizes `n=2 phase=0` but at `n=2 phase=0`, `active_count = 2`, the loop runs 1 iteration, and `prev_last_set = true`.
- **Severity**: medium (dead code — and the comment misleads future readers about the invariants).
- **Fix (one line)**: remove `prev_last_set` and the `if !prev_last_set` block (lines 179, 197, 238-244).

#### M5. `mincut/lib.rs:207-218` — recomputing `cut_weight_phase` from the matrix is mathematically redundant
- **Issue**: The comment at 207-212 says "weights_to_set[last] was overwritten when last entered S; we must recompute." But examination of the inner loop (line 200-204) shows `weights_to_set` is updated only for `j` where `active[j] && !in_set[j]`, which excludes `last` once it joins the set. So `weights_to_set[last]` *at the moment `last` was selected* is exactly the cut-of-the-phase weight. The local variable `best_w` (line 188) holds that value at the end of the inner loop (since `last == chosen` on the final iteration). Recomputing via the row sum in `mat[last*n+j]` produces the same answer in O(n) more work per phase.
- **Severity**: medium (the comment is wrong — readers will be misled; perf is O(n) per phase, n phases, so O(n²) avoidable work over the whole algorithm).
- **Fix (one line)**: replace the recompute loop with `let cut_weight_phase = best_w;` (capture `best_w` from the inner loop and re-use after).

#### M6. `cert.rs:296-299, 365-368` — emptiness re-checks duplicate the `issue()`-time validation but skip it for direct field access
- **Issue**: `Stage0Certificate::verify` checks `self.artifacts.is_empty()` and `ReadoutCertificate::verify_with_tree` checks `self.trajectory_count == 0`. The struct fields are `pub`, so callers CAN construct via field-access (defending here is correct). But `ReadoutCertificate.trajectory_count` is a `usize` independent of the supplied tree's `tree.leaves().len()` — a mismatched cert (count=1, tree has 5 leaves, root happens to match) is not detected. The cert's stated count and the tree's actual count diverge silently.
- **Severity**: medium (a binding gap on `trajectory_count`).
- **Fix (one line)**: in `verify_with_tree`, also check `tree.leaves().len() == self.trajectory_count`, returning a structured error on mismatch.

#### M7. `cert.rs:111-120` — `MerkleProof.leaf_count` field is part of the proof but unsigned/unbound
- **Issue**: `MerkleProof.leaf_count` is supplied by the proof producer and used by `verify_proof` for length validation and bounds checking. An attacker can adjust `leaf_count` to whatever value makes `expected_proof_length(leaf_count) == proof.siblings.len()`; the verify still succeeds or fails on the merkle-hash check, not on `leaf_count`. The field is informational, not a security-relevant binding. Carrying it in the proof is misleading — readers think it's "the size of the tree" and trust it.
- **Severity**: medium (the field is a footgun in API hands; not exploitable in itself but invites callers to read it as authoritative).
- **Fix (one line of doc)**: add a `///` comment to `MerkleProof.leaf_count` explicitly stating "informational; not bound to the root. Verify caller-side that this matches the tree the cert was issued against." OR remove it and infer the validation length from `siblings.len()` alone (changes the invariant slightly — would need to derive expected length from index instead).

#### M8. `granger.rs:159-164` — `denom <= 0.0` guard is unreachable for valid inputs
- **Issue**: `ssr_unrestricted` is `Σ resid²`, always non-negative. It can be exactly 0 (perfect fit) but never negative. The branch `denom <= 0.0` is defending against `< 0`, which is impossible from the code path. Equality with 0 is correctly handled (sets f_statistic = INFINITY). The defensive `< 0` half is dead.
- **Severity**: medium (dead defense; the comment "Unrestricted residuals are zero: perfect fit. F → ∞" is correct for `denom == 0`, just over-broadly stated).
- **Fix (one line)**: change to `if denom == 0.0 { f64::INFINITY } else { numerator / denom }` and simplify the comment to "Unrestricted residuals are exactly zero: perfect fit."

### LOW — 8

#### L1. `cert.rs:266, 328` — `Stage0Certificate.policy_version` and `ReadoutCertificate.policy_version` lack docstrings explaining their semantics
- **Issue**: Both structs have a `policy_version: String` field with no inline doc. The Stage 0 cert's policy version is what the legal-gate version contract is about (per `04 §5`), so an undocumented public field is a footgun. Combined with H1 (the field isn't bound), readers will assume "this is the policy version the cert is valid under" when in fact it's "a string the issuer typed in."
- **Fix (one line each)**: add `/// Policy version the issuer was operating under at issue time. NOT bound to the manifest hash — see H1.` (Once H1 is fixed, the second sentence goes away.)

#### L2. `cert.rs:64-79` — `CertError::code()` enumeration is alphabetically inconsistent with the variant declaration order
- **Issue**: The variants are declared in order `EmptyLeaves, ProofIndexOutOfBounds, ProofLengthInvalid, MerkleRootMismatch, ProofRejected, WitnessChainInvalid, WitnessChainMissing, ManifestHashMismatch, ArtifactsEmpty, TrajectoriesEmpty`. The `code()` match arms follow the same order. Both diverge from the natural grouping (Merkle errors together, Stage 0 errors together, witness errors together). Cosmetic.
- **Severity**: low (style — readability for someone scanning for "all the merkle codes").
- **Fix (one line cluster)**: re-order variants and code arms to group by domain: Merkle (5), Witness (2), Manifest (1), Empty (2). Match the order in both the `Error` enum and the `code()` impl.

#### L3. `mincut/lib.rs:113-119` — `NonSquareMatrix.rows = weights.len() / n.max(1)` gives misleading row count
- **Issue**: `n.max(1)` is a no-op since `n >= 2` was already enforced. More importantly, `weights.len() / n` is integer division — if `weights.len() = 7` and `n = 2`, the error reports `rows = 3` even though the matrix is malformed and "rows" isn't well-defined. Same for `weights.len() = 5, n = 3` → reports `rows = 1`.
- **Severity**: low (the error code and the broad message are right; the field is just decorative).
- **Fix (one line)**: change to `rows: weights.len()` (report the actual length) and rename the field to `actual_len`, or drop the field and rely on the error message that already includes both shape and count.

#### L4. `granger.rs:323` — `max_iter`, `eps`, `fpmin` are loop-local consts that should be module-level for callers who care
- **Issue**: A future maintainer wanting to tighten convergence (e.g., `eps = 1e-16` for stricter Granger p-values in calibration tests) has to edit inside the function. Module-level `const` is a one-liner that doesn't change behavior.
- **Severity**: low (micro-style).
- **Fix (one line each)**: lift `BETACF_MAX_ITER`, `BETACF_EPS`, `BETACF_FPMIN` to module-level `const`s.

#### L5. `cert.rs:138-140` — `levels.last().expect("at least one level")` repeated; could bind once
- **Issue**: The `while` loop reads `levels.last()` twice per iteration (line 140 condition + line 141 in the body). Bind once to a local for clarity:
  ```rust
  while let Some(prev) = levels.last() {
      if prev.len() <= 1 { break; }
      ...
  }
  ```
- **Severity**: low (style).
- **Fix (one line)**: rewrite the loop with `while let Some(prev) = levels.last() { if prev.len() <= 1 { break; } ... }` — drops two `expect(...)` calls.

#### L6. `mincut/lib.rs:248-263` — three separate row/col zero-out loops can be merged
- **Issue**: After merging `last` into `prev_last`, lines 250-256 update the merged row/col, line 258-259 zero `(prev_last, last)` and `(last, prev_last)`, and lines 260-263 zero the entire `last` row + col. The triple-loop is O(3n); a single `for j in 0..n { ... }` covering all three responsibilities works in O(n).
- **Severity**: low (perf is irrelevant at typical n; clarity win).
- **Fix (one line cluster)**: collapse into a single loop over `j in 0..n` with branches `j == last`, `j == prev_last`, otherwise.

#### L7. `cert.rs:36-62` — error variant names are mixed-case relative to their codes
- **Issue**: `EmptyLeaves` ↔ `CERT_MERKLE_EMPTY_LEAVES` (insertion of MERKLE), `ArtifactsEmpty` ↔ `CERT_ARTIFACTS_EMPTY` (clean), `WitnessChainMissing` ↔ `CERT_WITNESS_CHAIN_MISSING` (clean). The `EmptyLeaves` ↔ `CERT_MERKLE_EMPTY_LEAVES` mismatch is harmless but a future renamer will trip over it.
- **Severity**: low (consistency).
- **Fix (one line)**: rename `EmptyLeaves` → `MerkleEmptyLeaves` so variant + code match. (Or, less intrusively, leave it.)

#### L8. `granger.rs:557-579` — `splitmix64` and `pseudo_normal` test helpers are duplicated in `granger_fsv.rs:10-25`
- **Issue**: The same Box-Muller-via-SplitMix64 pseudo-Gaussian sampler is implemented twice (once in tests, once in the FSV example). If the seeding strategy changes in one, the other goes silently stale.
- **Severity**: low (DRY).
- **Fix (one line)**: pull the helpers into a small `tests/test_util.rs` (or feature-gated `pub fn` under `#[cfg(any(test, feature = "test-util"))]`) and call from both. OR leave as-is since FSV examples are intentionally self-contained per the per-binary FSV pattern.

---

## Items deliberately NOT flagged (and why)

- **`MerkleTree` build using RFC-6962-style odd-leaf duplication**: matches verify_proof's sibling logic; correct invariant. The alternative (zero-pad odd levels) breaks the proof structure in subtle ways.
- **Domain-separation tags (`MERKLE_LEAF_TAG`, `MERKLE_NODE_TAG`)**: correct application of cross-protocol separation per the spec. Tag mixing is right.
- **`f_distribution_upper_tail.x = (d1*f) / (d1*f + d2)` formula**: this is the textbook substitution for the F-distribution-CDF-via-incomplete-beta. Correct.
- **`regularized_incomplete_beta` branch boundary `x < (a+1)/(a+b+2)`**: textbook (Numerical Recipes 6.4). Continuous at the boundary — small numerical seam but no correctness gap.
- **`ln_gamma` reflection formula for `x < 0.5`**: standard. The Granger flow only ever calls `ln_gamma(d1/2)` and `ln_gamma(d2/2)` with `d1, d2 ≥ 1`, so the reflection branch is unreachable in practice.
- **Lanczos coefficients hardcoded with 8 terms**: matches Numerical Recipes 6.1 exactly; ~1e-10 accuracy, sufficient for p-value computation where 1e-6 is the practical floor.
- **`StoerWagnerConfig::default { symmetry_tolerance: 1e-9 }`**: reasonable for f64 inputs; tightening would reject fp-arithmetic-corrupted symmetric matrices that are actually fine.
- **`weights_from_edges` silently dropping self-loops via `i != j`**: this is the right behavior — Stoer-Wagner requires zero diagonal, and self-loops have no algorithmic meaning for global mincut. Documenting this in a `///` comment would be a +0 simplification.
- **`stoer_wagner` implementing the dense O(V³) variant rather than heap-augmented O(V² log V)**: appropriate for the documented use case (tens to hundreds of nodes). Adding a heap variant would double the test surface for a perf win that doesn't matter yet.
- **`ols` using Gaussian elimination not QR**: appropriate for k = 1 + 2*lag (typically 3-7 columns). QR would be over-engineering at this scale.
- **`Stage0Certificate` and `ReadoutCertificate` not deriving `Copy`**: they own `Vec<u8>` and `String`, which aren't Copy. Correct.
- **`MerkleTree` not implementing `serde`**: not part of the surface today; FSV evidence persists raw bytes via separate paths. Adding serde now would lock in a layout we may want to evolve.
- **`granger::ols` re-implementing OLS rather than calling `cg::ConjugateGradientSolver`**: CG is iterative, OLS via Gaussian elim is direct. For k ≤ 7, direct is faster and exact. The doc comment at line 184-187 already justifies this.
- **`pseudo_normal` test helper using Box-Muller fed by SplitMix64**: per the inline comment ("Sequential seeds produce strongly-decorrelated outputs"), correct rationale. The `>> 11` truncates to 53 bits for f64 mantissa precision. Right.

---

## Summary

| Severity | Count |
|----------|-------|
| HIGH     | 2 |
| MEDIUM   | 8 |
| LOW      | 8 |
| **Total**| **18** |

The two HIGH findings (H1 cert-binding gap, H2 betacf silent fallthrough) are
the only items I would block a merge on. Everything else is simplification,
dead code, doc clarity, or numerical-edge polish. The modules ship in a
state worth merging once H1 and H2 are addressed; the M-tier items deserve
a dedicated cleanup turn (especially M4 dead `prev_last_set` and M5 redundant
recompute, which together remove ~15 lines of misleading code from
`stoer_wagner`).

## Source-of-truth files referenced
- file:/home/cabdru/contextgraph/crates/context-graph-witness/src/cert.rs
- file:/home/cabdru/contextgraph/crates/context-graph-witness/src/lib.rs
- file:/home/cabdru/contextgraph/crates/context-graph-witness/Cargo.toml
- file:/home/cabdru/contextgraph/crates/context-graph-witness/examples/cert_fsv.rs
- file:/home/cabdru/contextgraph/crates/context-graph-mincut/src/lib.rs
- file:/home/cabdru/contextgraph/crates/context-graph-mincut/Cargo.toml
- file:/home/cabdru/contextgraph/crates/context-graph-mincut/examples/mincut_fsv.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/granger.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/lib.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/src/error.rs
- file:/home/cabdru/contextgraph/crates/context-graph-solver/examples/granger_fsv.rs
