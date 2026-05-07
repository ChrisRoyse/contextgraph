---
namespace: handoff
agent_id: agent-04-simplifier
timestamp: 2026-04-29T06:00:00Z
status: active
tags: exp2, handoff, coordinator, refinement-summary
---

# Handoff: agent-04-simplifier → coordinator

## Refinements applied (with minimal-impact diffs, no behaviour change beyond the documented defect)

### R1 — B-4 KSG sklearn API breakage [APPLIED, VERIFIED]

- File: `scripts/exp2/estimators/ksg.py` (~+8/-3 lines).
- Change: switched `NearestNeighbors.radius_neighbors(..., radius=eps_array, ...)`
  to `KDTree.query_radius(..., r=eps_array, count_only=True)`. KDTree still
  supports array radii on sklearn 1.8.
- `ksg_entropy` was NOT changed: it does not call `radius_neighbors`, so it
  doesn't have the bug. See `discoveries/agent-04-simplifier--ksg-entropy-does-not-have-b4-bug.md`.
- Verification: production probe `ksg_mi` runs at d=32 n=800 k=5 in <1s,
  returns finite values matching agent-02's KDTree-workaround sweep numbers.
- Existing test `test_b4_production_ksg_mi_fails_at_dim_32` was renamed to
  `test_b4_production_ksg_mi_runs_at_dim_32` and flipped from `pytest.raises`
  to assertion-success — per the brief's "if the fix changes the contract,
  update the test to assert success rather than failure."

### R3 — B-3 per-pair JSON 8 missing provenance fields [APPLIED, VERIFIED]

- File: `scripts/exp2/run_estimators.py` (~+90 lines).
- Added: `EMBEDDER_WEIGHT_FILES` mapping (14 entries; 4 None for E2/E3/E4
  positional/Fourier/sinusoidal-PE and E9 deterministic HDC), helper
  `load_provenance(cfg_path, out_dir, pca_models_path)`, lookup
  `embedder_weight_sha256(key, hashes)`. `run_pair` accepts `provenance`
  parameter and writes 8 new fields per spec §13.
- `redundancy` is filled in later because per-embedder entropies are
  computed in effective_rank.py; the row stores `null` until then.
- File: `scripts/exp2/effective_rank.py` (~+25 lines).
- Added: `backfill_pair_redundancy(rows, entropy, out_dir)` rewrites each
  pair JSON with computed `redundancy = MI / min(H_j, H_k)` clamped to [0,1].
  `main()` calls it right after `estimate_entropies()`.
- Honesty discipline: when an upstream artefact is absent at row-write time
  (partial run), the field records None — not a fake hash. Per the brief.
- 2 new contract tests verify the helper returns None for missing artefacts
  and surfaces hashes correctly when verification_log.json is present.

### R2 — B-2 AuditChain orchestrator wiring [APPLIED, VERIFIED]

- File: `scripts/exp2/run_exp2.py` (~+75 lines).
- Added: `AUDIT_AWARE_PHASES = {"estimators", "n_eff"}` constant,
  `finalize_master_audit_chain(config_path)` function, modified `run()` to
  forward `--audit-chain <path>` flag, modified `main()` to ALWAYS call
  finalize at end (even on partial run).
- Files: `scripts/exp2/run_estimators.py`, `scripts/exp2/effective_rank.py`
  both accept `--audit-chain` flag and write per-phase chains.
- Master `audit_chain.json` records: 3 per-phase chains (env/estimators/n_eff)
  as `phase_chain:<phase>` artefacts, 11 headline deliverables as
  `<label>` artefacts, all `pair_results/E*_E*.json` rows. Missing artefacts
  log `artefact_missing` events instead of crashing.
- 1 new test verifies the master chain is written on a stub partial run.

## Refinements DECLINED with operator decision required

### R4 — B-1 structural_upper_bound 91 vs 105 [DECLINED — operator decision]

- Decision file: `decisions/agent-04-simplifier--r4-decline-structural-upper-bound-91-vs-105.md`.
- Reason: spec is internally inconsistent (00_README + 03_methodology §7a
  says 91; the math says 105). Implementation chose 105. This is a
  publication-impact decision (paper headline says "up to 91 signals"),
  not a code-quality one.
- Operator must pick: change code to 91 OR update 4 spec locations to 105.

### R5 — B-5 KSG curse of dimensionality at d=32 [DECLINED — operator decision]

- Decision file: `decisions/agent-04-simplifier--r5-decline-ksg-curse-of-dimensionality-d32.md`.
- Reason: KSG underestimates Gaussian MI by ~91% at production dim=32
  (rho=0.5: truth=4.6 nats, KSG=0.43 nats). Spec demands |err|<0.15 nats —
  unachievable at this dim. This is a methodology decision.
- Operator must pick: (a) reduce target_dim to 8/16, (b) drop KSG cross-val
  gate, or (c) recalibrate threshold.

### R6 — InfoNCE overfit retry-with-smaller-critic [DEFERRED with diff]

- Discovery file: `discoveries/agent-04-simplifier--r6-infonce-overfit-retry-deferred.md`.
- Reason: Cascades through 4 modules (infonce, run_estimators, effective_rank,
  visualize) with NaN propagation surface that cannot be reliably tested
  without an actual production overfit observation. Risk > reward in this
  session. Full proposed diff documented for next coder session.
- Brief explicitly authorized this deferral pathway: "If it's too risky to
  do safely in this session, document the proposed diff in a discovery file
  and DEFER. Use judgment."
- Status: code still raises on overfit. agent-01's HIGH severity entry stays
  open as known defect with documented remediation.

## Pytest counts before/after

| Stage | Tests | Pass | Fail | Wall |
|-------|-------|------|------|------|
| Baseline (entry to session) | 23 | 23 | 0 | 2.11s |
| After R1 (KSG fix + test flip) | 23 | 23 | 0 | 2.13s |
| After R3 (+ 2 provenance helper tests) | 25 | 25 | 0 | 1.83s |
| After R2 (+ 1 master chain test) | 26 | 26 | 0 | 2.06s |
| Final | 26 | 26 | 0 | 2.06s |

3 new tests added. Zero regressions.

## New tests added

1. `test_run_estimators_provenance_helper_handles_missing_artefacts` — confirms
   `load_provenance()` returns None for missing upstream artefacts, never
   crashes; confirms `EMBEDDER_WEIGHT_FILES` covers all 14 keys exactly.
2. `test_run_estimators_provenance_helper_pulls_weight_hashes_from_log` —
   confirms hashes from `verification_log.json` surface correctly via the
   embedder→weight-file mapping; confirms None for embedders without weight
   files (E9), and None for embedders whose hash isn't yet computed.
3. `test_orchestrator_finalize_master_audit_chain_on_partial_run` — confirms
   `run_exp2.finalize_master_audit_chain` writes a 64-hex `chain_sha256`
   even when only one per-phase chain is present, with `phase_chain_missing`
   and `artefact_missing` events recording the dangling references.

## Files modified in repo

| File | LoC before | LoC after | Δ | Summary |
|------|------------|-----------|---|---------|
| `scripts/exp2/estimators/ksg.py` | 71 | 74 | +3 | KDTree.query_radius swap (R1) |
| `scripts/exp2/run_estimators.py` | 155 | 280 | +125 | provenance helpers + 8 fields + audit-chain flag (R3, R2) |
| `scripts/exp2/effective_rank.py` | 163 | 194 | +31 | backfill_pair_redundancy + audit-chain flag (R3, R2) |
| `scripts/exp2/run_exp2.py` | 73 | 148 | +75 | AUDIT_AWARE_PHASES + finalize_master_audit_chain (R2) |
| `scripts/exp2/tests/test_synthetic_full_sweep.py` | 287 | 407 | +120 | 3 new contract tests + B-4 test flip |

Total +354 / -0 lines across 5 files. All under 500-line CLAUDE.md budget.

## Operator decisions still required

1. **B-1 structural_upper_bound**: 91 (paper headline) or 105 (formula math)?
2. **B-5 KSG d=32 bias**: reduce target_dim, drop KSG gate, or recalibrate threshold?
3. **R6 InfoNCE overfit**: implement the proposed retry-with-smaller-critic diff
   in a focused coder session, OR keep current fail-loud behaviour (which is
   louder than spec wants but doesn't risk NaN propagation).

## What the coordinator should personally verify

1. **`scripts/exp2/estimators/ksg.py`** — read the new `ksg_mi` body; confirm
   the algorithm contract is preserved (Chebyshev metric, eps shrunk by
   `np.nextafter`, count_only-1 with floor at 0, digamma formula identical).
2. **`scripts/exp2/run_estimators.py::EMBEDDER_WEIGHT_FILES`** — confirm 14
   entries and the 4 None entries match the deterministic-PE/HDC embedders
   per `docs/exp2/03_methodology.md`.
3. **`scripts/exp2/run_exp2.py::finalize_master_audit_chain`** — confirm
   the artefact list (3 phase chains + 11 headline + N pair rows) matches
   `docs/exp2/08_output_deliverables.md`.
4. **pytest 26/26 PASS** — re-run `python3 -m pytest -v scripts/exp2/tests/`
   to confirm.
5. **Production sanity** — `python3 -c "from scripts.exp2.estimators.ksg import ksg_mi; ..."`
   at d=32 n=800 should run without crash (B-4 fix verification).

## What I did NOT change in repo

- `docs/exp2/` — immutable per brief.
- `scripts/exp2/audit_chain.py` — already correct, no edits.
- `scripts/exp2/verify_environment.py` — already wires its own chain.
- `scripts/exp2/build_corpus.py`, `extract_embeddings.py`, `fit_projections.py`,
  `visualize.py`, `fsv.py`, `manual_fsv.py`, `mcp_client.py`, `replay.py`,
  `write_distribution.py`, `common.py`, `estimators/infonce.py`,
  `estimators/bootstrap.py`, `estimators/synthetic.py` — out of scope, not
  touched.
- `artefacts/exp2/` — no artefact mutations.

## Confidence: HIGH

- Every applied refinement has a corresponding `decisions/` file with
  rationale, root-cause analysis, and verification.
- Every applied refinement has at least one new pytest case locking in the
  contract.
- 26/26 pytest pass at session end (was 23/23 at entry; 3 added).
- B-4 fix verified end-to-end with a direct Python probe at the production
  hyperparameter d=32 — no crash, finite output matching agent-02's KDTree
  workaround numbers.
- Two declined refinements (R4, R5) are documented as explicit operator
  decisions, not silently ignored.
- One deferred refinement (R6) has the full proposed diff attached for the
  next coder.

## Files I wrote to ./memory/

### Decisions (5)
- `decisions/agent-04-simplifier--r1-ksg-mi-kdtree-switch.md`
- `decisions/agent-04-simplifier--r2-audit-chain-orchestrator-wiring.md`
- `decisions/agent-04-simplifier--r3-per-pair-provenance-fields.md`
- `decisions/agent-04-simplifier--r4-decline-structural-upper-bound-91-vs-105.md`
- `decisions/agent-04-simplifier--r5-decline-ksg-curse-of-dimensionality-d32.md`

### Discoveries (2)
- `discoveries/agent-04-simplifier--r6-infonce-overfit-retry-deferred.md`
- `discoveries/agent-04-simplifier--ksg-entropy-does-not-have-b4-bug.md`

### Journal — successes (2)
- `journal/agent-04-simplifier--success-r1-ksg-mi-kdtree-switch.md`
- `journal/agent-04-simplifier--success-r2-r3-audit-chain-and-provenance-fields.md`

### Blockers (0)
- No new blockers found while fixing.

### Handoff (1)
- `handoff/agent-04-to-coordinator--refinement-summary.md` (this file).

### Reports (1)
- `reports/agent-04-simplifier--final-report.md` (next).

**Total memory files written: 11**
(5 decisions + 2 discoveries + 2 journal-success + 1 handoff + 1 report)
