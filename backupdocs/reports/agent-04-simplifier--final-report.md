---
namespace: reports
agent_id: agent-04-simplifier
timestamp: 2026-04-29T06:05:00Z
status: active
tags: exp2, final-report, simplifier, agent-04
---

# Final Report: agent-04-simplifier

## Task Completed: yes (partial — R6 deferred per brief, R4/R5 declined per brief)

Applied the three Tier-1 ship-blocker refinements (R1, R2, R3) identified by
agents 01-03. Declined the two operator-decision items (R4, R5) per brief
instructions. Deferred the one behaviour-altering risk item (R6) with full
proposed diff documented for the next coder, per brief authorization. Pytest
went from 23/23 PASS to 26/26 PASS with 3 new contract tests; zero regressions.

## ALL Files Modified In Repo

| File | LoC before | LoC after | Δ | One-line summary |
|------|------------|-----------|---|------------------|
| `scripts/exp2/estimators/ksg.py` | 71 | 74 | +3 | R1: KDTree.query_radius replaces NearestNeighbors.radius_neighbors to fix sklearn 1.8 cython crash at dim>=20. |
| `scripts/exp2/run_estimators.py` | 155 | 280 | +125 | R3+R2: provenance helpers, 8 new per-pair fields per spec §13, optional `--audit-chain` phase flag. |
| `scripts/exp2/effective_rank.py` | 163 | 194 | +31 | R3+R2: `backfill_pair_redundancy` rewrites pair JSONs with computed redundancy after entropies known; optional `--audit-chain` phase flag. |
| `scripts/exp2/run_exp2.py` | 73 | 148 | +75 | R2: orchestrator forwards `--audit-chain` to audit-aware phases and ALWAYS finalizes a master `audit_chain.json` (Constitution C7). |
| `scripts/exp2/tests/test_synthetic_full_sweep.py` | 287 | 407 | +120 | R1 test contract flip (B-4 fails→passes at d=32) plus 3 new tests covering provenance helper + master audit chain. |

Total +354/-0 lines across 5 files. All under the CLAUDE.md 500-line budget.

## ALL Files Written To ./memory/

### Decisions (5)
- `memory/decisions/agent-04-simplifier--r1-ksg-mi-kdtree-switch.md` — R1 rationale + diff summary.
- `memory/decisions/agent-04-simplifier--r2-audit-chain-orchestrator-wiring.md` — R2 rationale + diff summary.
- `memory/decisions/agent-04-simplifier--r3-per-pair-provenance-fields.md` — R3 rationale + EMBEDDER→weight-file mapping table + verification.
- `memory/decisions/agent-04-simplifier--r4-decline-structural-upper-bound-91-vs-105.md` — DECLINED operator decision; both options spelled out.
- `memory/decisions/agent-04-simplifier--r5-decline-ksg-curse-of-dimensionality-d32.md` — DECLINED methodology decision; three options spelled out.

### Discoveries (2)
- `memory/discoveries/agent-04-simplifier--r6-infonce-overfit-retry-deferred.md` — DEFERRED with full proposed diff; risk-vs-reward analysis.
- `memory/discoveries/agent-04-simplifier--ksg-entropy-does-not-have-b4-bug.md` — scope clarification: agent-02's precaution about ksg_entropy was unnecessary.

### Journal — successes (2)
- `memory/journal/agent-04-simplifier--success-r1-ksg-mi-kdtree-switch.md` — R1 test counts + production probe + ksg_entropy out-of-scope confirmation.
- `memory/journal/agent-04-simplifier--success-r2-r3-audit-chain-and-provenance-fields.md` — R2+R3 test counts + production sanity check + what's unblocked.

### Blockers (0)
NO new blockers. All findings during R1/R2/R3 work were either (a) addressed
in the same commit or (b) confirmed already-filed in earlier blockers.

### Handoff (1)
- `memory/handoff/agent-04-to-coordinator--refinement-summary.md` — summary
  for coordinator: applied/declined/deferred matrix, test counts, files
  modified, operator decisions, what to verify personally.

### Reports (1)
- `memory/reports/agent-04-simplifier--final-report.md` — this file.

**Total memory files written: 11**
(5 decisions + 2 discoveries + 2 journal-successes + 1 handoff + 1 report)

## Tests/Build Status

### Before (entry to session)

| Suite | Tests | Pass | Fail | Wall |
|-------|-------|------|------|------|
| `pytest scripts/exp2/tests/` | 23 | 23 | 0 | 2.11s |

### After

| Suite | Tests | Pass | Fail | Wall |
|-------|-------|------|------|------|
| `pytest scripts/exp2/tests/` | 26 | 26 | 0 | 2.06s |

3 new tests added. 1 existing test renamed + assertion flipped (B-4 contract
inversion: agent-02's `_fails_at_dim_32` is now `_runs_at_dim_32` post-fix).
Zero regressions.

### Lint/Build

Not exercised: this is pure Python; the Rust binary was not touched. CLAUDE.md
build commands target `npm run build` which is for the JS layer, not exp2.

## Refinements applied

| ID | Description | Status | Files |
|----|-------------|--------|-------|
| **R1** | B-4: ksg_mi sklearn 1.8 API breakage | APPLIED | ksg.py (+3), test_synthetic_full_sweep.py (test flip) |
| **R2** | B-2: AuditChain orchestrator wiring | APPLIED | run_exp2.py (+75), run_estimators.py (+~10), effective_rank.py (+~12), test (+1 case) |
| **R3** | B-3: per-pair JSON 8 missing provenance fields | APPLIED | run_estimators.py (+~115), effective_rank.py (+~20), test (+2 cases) |
| **R4** | B-1: structural_upper_bound 91 vs 105 | DECLINED | None — operator decision documented |
| **R5** | B-5: KSG curse-of-dimensionality at d=32 | DECLINED | None — methodology decision documented |
| **R6** | InfoNCE overfit retry-with-smaller-critic | DEFERRED | None — full proposed diff in discovery file |

## For The Coordinator

### What to verify personally (Constitution C9)

1. **Read `scripts/exp2/estimators/ksg.py`** — confirm the new `ksg_mi` body
   preserves the KSG-1 algorithm contract (Chebyshev metric, `np.nextafter`
   eps, `count_only - 1` with `np.maximum(..., 0)` floor, digamma formula).
2. **Read `scripts/exp2/run_estimators.py::EMBEDDER_WEIGHT_FILES`** — confirm
   14 entries with 4 None entries (E2/E3/E4/E9 are deterministic PE/HDC).
3. **Read `scripts/exp2/run_exp2.py::finalize_master_audit_chain`** — confirm
   it ALWAYS runs (in `main`, after the phase loop, even on partial run)
   and that headline_artefacts list matches `docs/exp2/08_output_deliverables.md`.
4. **Re-run pytest** — `python3 -m pytest -v scripts/exp2/tests/` should
   show 26 passed.
5. **Production sanity** — run:
   ```
   python3 -c "
   import numpy as np
   from scripts.exp2.estimators.ksg import ksg_mi
   rng = np.random.default_rng(42)
   u = rng.normal(size=(800, 32))
   v = rng.normal(size=(800, 32))
   print(ksg_mi(u, v, k=5))
   "
   ```
   Should print a small finite number (no crash). This proves B-4 is fixed.

### What to relay to the operator

1. **B-1 91 vs 105**: pick whether the headline is paper text (91) or
   formula math (105). Decision file:
   `memory/decisions/agent-04-simplifier--r4-decline-structural-upper-bound-91-vs-105.md`.
2. **B-5 KSG at d=32**: pick whether to reduce target_dim, drop KSG cross-val
   gate, or recalibrate threshold. Decision file:
   `memory/decisions/agent-04-simplifier--r5-decline-ksg-curse-of-dimensionality-d32.md`.
3. **R6 InfoNCE overfit**: schedule a focused coder session to implement
   retry-with-smaller-critic per the proposed diff. Discovery file:
   `memory/discoveries/agent-04-simplifier--r6-infonce-overfit-retry-deferred.md`.
4. **GPU memory still gates production**: agent-03 confirmed 18 GiB free
   vs 30 GiB required. The B-4 fix unblocks the harness; the GPU constraint
   still blocks T-007 onwards. Free up VRAM (or rerun later when free)
   before attempting the 91-pair sweep.

## Decisions Made

- Did NOT modify `docs/exp2/` (immutable spec per brief).
- Did NOT modify `ksg_entropy` (does not have the B-4 bug pattern; verified).
- Did NOT modify `scripts/exp2/audit_chain.py` (already correct; just wired
  it in elsewhere).
- Did NOT add `--audit-chain` to phases that don't yet need it (corpus,
  embeddings, projections, visualize, fsv) — minimal-impact: master chain
  hashes their artefacts directly.
- Did NOT touch `verify_environment.py` (its chain wiring was already
  correct).
- Treated R6 as deferred with proposed diff, NOT applied — the brief
  explicitly authorized this pathway and the NaN propagation surface in
  4 modules is a real risk without an actual production overfit observation.
- For R4/R5: explicit operator decisions documented; no unilateral resolution.

## Discoveries

- `ksg_entropy` does NOT have the B-4 bug pattern; agent-02's precaution
  about it in their blocker file was unnecessary. (Documented separately.)
- The 14-embedder panel maps to 10 weight files; 4 embedders (E2/E3/E4/E9)
  are deterministic PE/HDC with no learned weights and so legitimately
  record None for `embedder_*_weight_sha256`. This is honest, not a defect.
- Master audit chain finalization can run on a partial-run state (only the
  env phase chain present) and produce a valid 64-hex `chain_sha256` with
  dangling references logged as `*_missing` events. This honors Constitution
  C7 even when the run is gated by GPU memory or B-5.

## Failures

None. Every applied refinement passed on first invocation; pytest never
regressed; no behavioural change beyond the documented defect for each fix.

## Successes

- 3 ship-blockers (R1, R2, R3) addressed with minimal-impact diffs.
- 3 new contract tests pinning the helpers in place.
- B-4 production sanity check: `ksg_mi` runs at d=32 n=800 k=5 in <1s
  without exception (was crashing on first call).
- 26/26 pytest pass; zero regressions.
- 2 operator decisions cleanly documented, not silently ignored.
- 1 risky refinement (R6) deferred with full proposed diff, not skipped.

## Tests/Build status (final)

- pytest: 26/26 passed.
- Build: not exercised (Rust binary already built; exp2 is pure Python).
- Lint: not exercised (no Rust changes; Python style matches surrounding code).

## Confidence: HIGH

- Every applied refinement has a `decisions/` file with rationale, root-cause
  analysis, and verification block.
- Every applied refinement has at least one new pytest case.
- pytest 26/26 PASS at session end (was 23/23 at entry; 3 added; 1 flipped).
- B-4 fix verified end-to-end with a direct Python probe at the production
  hyperparameter (d=32, n=800, k=5) — no crash, finite output, numbers
  match agent-02's KDTree-workaround sweep within float noise.
- 2 declined refinements documented as explicit operator decisions with
  pros/cons of each option, not silently ignored.
- 1 deferred refinement has the full proposed diff attached for the next
  coder, not a vague "TODO".

## Limitations

- Did not run the full 91-pair production sweep — gated by 30 GiB GPU free
  requirement (only 18 GiB free per agent-03's measurement). The B-4 fix
  unblocks the harness; downstream blockers remain.
- Did not verify R3 fields end-to-end on real artefacts because no
  production run has occurred. The unit tests verify the helper contracts,
  not the integration on real `verification_log.json` and `pca_models.npz`
  inputs. (The agent-02 + agent-03 artefact inventories confirm those files'
  schemas are correct, so the helper should "just work" on a real run.)
- Did not implement R6 (InfoNCE overfit retry) — deferred with documented
  diff, per brief authorization. agent-01's HIGH severity entry stays open
  as a known defect.
- Did not modify ksg_entropy — confirmed not affected by B-4. If a future
  audit finds a different bug in ksg_entropy, that is a new finding.
- Did not exhaustively wire `--audit-chain` to every phase script (only
  estimators + n_eff). Other phases' artefacts are still hashed by the
  master chain's headline_artefacts list, so the audit trail is complete;
  but per-phase chains for corpus/embeddings/projections/visualize/fsv would
  be a future ergonomic improvement (not a correctness gap).

The implementation continues to honor the SMAP discipline: every change has
a memory file; every change is verified by a pytest case; every declined
or deferred change is documented with its rationale.
