---
namespace: reports
agent_id: agent-02-tester
timestamp: 2026-04-29T00:40:00Z
status: active
tags: exp2, final-report, tester, agent-02
---

# Final Report: agent-02-tester

## Task Completed: yes

Runtime test verification of EXP-2 implementation per the SMAP brief. Pytest baseline confirmed, T-013 synthetic Gaussian sweep added and run, all 3 agent-01 blockers verified, 2 NEW HIGH-severity ship-blockers discovered, failure-mode coverage mapped to runtime evidence.

## Files Modified In Repo

NONE. Zero production-code edits. Per `memory/decisions/agent-02-tester--no-production-code-edits.md`.

## Files Added In Repo

| Path | Purpose | Type |
|------|---------|------|
| `scripts/exp2/tests/test_synthetic_full_sweep.py` | 16-test pytest module covering M2/M6/M7/M8/B-4/B-5 | new test |
| `artefacts/exp2/synthetic_validation_report.json` | M2 deliverable — 7 rho rows with closed-form vs KSG | artefact (test-produced) |
| `artefacts/exp2/ksg_k_sweep_rho_0.5.json` | M6 deliverable — k-sweep stability data | artefact (test-produced) |

## ALL Files Written To ./memory/

### Blockers (2 NEW HIGH)
- `memory/blockers/agent-02-tester--ksg-radius-neighbors-array-broken-on-sklearn-1.8.md` — B-4: `ksg_mi` crashes at dim≥20 due to sklearn 1.8 API change. Production uses dim=32 → every pair fails on first call. Ship-blocker.
- `memory/blockers/agent-02-tester--ksg-curse-of-dimensionality-at-d-32.md` — B-5: even with B-4 fixed, KSG underestimates Gaussian MI by ~91% at d=32. Spec §3a recovery target is unachievable. Cross-validation gate (F6) will fire spuriously.

### Decisions (1)
- `memory/decisions/agent-02-tester--no-production-code-edits.md` — rationale for not editing source code; scope discipline; sketch of how InfoNCE recovery test could be added if coordinator wants it.

### Discoveries (1)
- `memory/discoveries/agent-02-tester--failure-mode-coverage-runtime-evidence.md` — F1-F7 detection map: 3/7 detected, 4/7 silent. F1 (implausibly low) and F3 (non-convergence) and F5 (wide CI) and F7 (N_eff divergence) have no automatic detector.

### Journal — verifications (5)
- `memory/journal/agent-02-tester--success-pytest-baseline-7-passed.md` — M1 baseline 7/0 pass.
- `memory/journal/agent-02-tester--verified-blocker-b1-structural-upper-bound-105.md` — B-1 confirmed via grep.
- `memory/journal/agent-02-tester--verified-blocker-b1-n-eff-formula-yields-105.md` — B-1 confirmed at runtime; function correctly produces 105.0; spec has the typo.
- `memory/journal/agent-02-tester--verified-blocker-b2-audit-chain-instantiated-only-in-verify-environment.md` — B-2 refined: AuditChain IS instantiated once (verify_environment.py:98), but never finalized; master `audit_chain.json` never written.
- `memory/journal/agent-02-tester--verified-blocker-b3-per-pair-json-missing-8-fields.md` — B-3 confirmed exactly: 8 fields missing (code_commit_sha, corpus_manifest_sha256, embedder_j_weight_sha256, embedder_k_weight_sha256, hyperparams_sha256, pca_j_sha256, pca_k_sha256, redundancy).

### Handoff (1)
- `memory/handoff/agent-02-to-03--runtime-state.md` — runtime state, sherlock's concrete command list, blocker triage.

### Reports (1)
- `memory/reports/agent-02-tester--final-report.md` — this file.

**Total memory files written: 11**
(2 blockers + 1 decision + 1 discovery + 5 journal + 1 handoff + 1 report)

## ALL Test Outcomes

| Run | Test count | Passed | Failed | Errors | Log path |
|-----|-----------|--------|--------|--------|----------|
| Baseline (existing) | 7 | 7 | 0 | 0 | `/tmp/agent-02-pytest-baseline.log` |
| New synthetic sweep | 16 | 16 | 0 | 0 | `/tmp/agent-02-pytest-synthetic.log` |
| Combined (after add) | 23 | 23 | 0 | 0 | `/tmp/agent-02-pytest-final.log` |

### Per-deliverable status

| Mission deliverable | Status | Evidence |
|---------------------|--------|----------|
| M1 baseline pytest run | DONE | 7/0/0 pass |
| M2 Synthetic Gaussian sweep T-013 | DONE | `artefacts/exp2/synthetic_validation_report.json` — 7 rho rows; KSG-d32 catastrophic underestimate documented |
| M3 Verify B-1 structural_upper_bound | DONE | grep confirms literal 105 at line 145; runtime confirms function produces 105 on zero-MI |
| M4 Verify B-2 AuditChain never called | DONE (refined) | exactly 1 stub instantiation found; no finalize() call; orchestrator has no chain phase |
| M5 Verify B-3 per-pair JSON missing | DONE | 8 fields missing exactly as agent-01 reported |
| M6 KSG k-sweep stability | DONE | `artefacts/exp2/ksg_k_sweep_rho_0.5.json` — spread 0.146 > 0.1 threshold |
| M7 Bootstrap determinism | DONE | `test_bootstrap_ci_is_deterministic_re_check` passes |
| M8 F1-F7 failure-mode coverage | DONE | 3/7 detected, 4/7 silent |
| M9 Manual FSV deferred to agent-03 | DONE | confirmed manual_fsv.py imports cleanly |

## Failures (with full error messages)

### Initial M2 run failure (BEFORE workaround)
Error captured at `/tmp/agent-02-pytest-synthetic.log`:
```
TypeError: only 0-dimensional arrays can be converted to Python scalars
  at sklearn/metrics/_pairwise_distances_reduction/_radius_neighbors.pyx:59
  via scripts/exp2/estimators/ksg.py:44
       n_u = np.array([len(x) - 1 for x in nn_u.radius_neighbors(u, radius=eps, return_distance=False)])
```

This is the source of NEW BLOCKER B-4. After installing the KDTree workaround in test code (NOT production), the synthetic sweep runs cleanly but exposes the spec contradiction (NEW BLOCKER B-5).

### Synthetic Gaussian recovery numbers (post-workaround)

| rho | truth nats | ksg nats | abs err | meets <0.20? |
|-----|------------|----------|---------|--------------|
| 0.00 | 0.000 | 0.029 | 0.029 | YES |
| 0.10 | 0.161 | 0.035 | 0.126 | YES |
| 0.30 | 1.509 | 0.138 | **1.371** | NO |
| 0.50 | 4.603 | 0.427 | **4.176** | NO |
| 0.70 | 10.774 | 1.004 | **9.770** | NO |
| 0.90 | 26.572 | 2.215 | 24.357 | n/a (rho>0.7) |
| 0.99 | 62.673 | 3.935 | 58.737 | n/a (rho>0.7) |

KSG fails the spec recovery target at every rho > 0.1.

## Blocker verification status (B-1, B-2, B-3 from agent-01)

| Blocker | Verdict | Evidence |
|---------|---------|----------|
| B-1 structural_upper_bound 105 vs 91 | CONFIRMED REAL | Static (grep line 145 = `: 105`); runtime (function returns 105.0 on synthetic zero-MI). Resolution requires SPEC fix (typo in §7a) OR semantic re-interpretation (label "structural_upper_bound" = 91 = pair count, not max value). Coordinator decides. |
| B-2 AuditChain never called | CONFIRMED REAL (refined) | grep shows exactly ONE instantiation in `verify_environment.py:98` (writes `audit_chain_environment.json`). No master chain. No `finalize()` call. `audit_chain.json` never produced. Agent-01's "no callers" understated; one stub exists. |
| B-3 per-pair JSON missing fields | CONFIRMED REAL | regex source extraction confirms exactly 8 missing fields, all provenance hashes. Agent-01's count is precise. |

## NEW blockers (HIGH severity)

| ID | Description |
|----|-------------|
| **B-4** | `ksg_mi` and `ksg_entropy` crash at dim≥20 on sklearn≥1.3 due to `radius_neighbors` API change (now scalar-only). Production uses dim=32 → every pair fails. Ship-blocker for the 91-pair sweep AND for entropy estimation. Trivial fix: switch to `KDTree.query_radius` or per-row loop. |
| **B-5** | KSG-1 underestimates Gaussian MI by ~91% at d=32 (curse of dimensionality). Spec §3a target |err|<0.15 for rho≤0.7 is unachievable. Cross-validation gate (F6) will fire on every high-MI pair, falsely flagging InfoNCE as the wrong estimator. Methodology-level decision needed: (a) reduce dim to 8/16, (b) use only InfoNCE, or (c) accept bias and recalibrate threshold. |

## For The Next Agent (agent-03-sherlock)

### Concrete commands

```bash
# 1. Smoke pytest (sanity check):
cd /home/cabdru/contextgraph && python3 -m pytest -v scripts/exp2/tests
# Expected: 23 passed.

# 2. Real environment verifier (Constitution C16 honest fail):
python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml --skip-cargo
# Expected: fails with VRAM < 30000 MiB if GPU loaded.

# 3. Live MCP manual FSV (the daemon-dependent test):
ls target/release/context-graph-mcp models/  # confirm prerequisites
python3 scripts/exp2/manual_fsv.py --config scripts/exp2/exp2.yml
# Expected: 3 happy + 3 edge cases pass; produces artefacts/exp2/manual_fsv_log.json

# 4. RocksDB inspection (per Constitution C5 source-of-truth):
ls -lh artefacts/exp2/manual_fsv_rocksdb_*/
# Sherlock should open these CFs and verify fingerprints exist.

# 5. Reproduce B-4 to confirm independently:
python3 -c "
import numpy as np
from scripts.exp2.estimators.ksg import ksg_mi
rng = np.random.default_rng(0)
u = rng.normal(size=(200, 32)); v = rng.normal(size=(200, 32))
try: ksg_mi(u, v, k=5)
except Exception as e: print('B-4 reproduced:', type(e).__name__, e)
"
```

### What sherlock should verify
- T-001 honest fail behaviour.
- MCP daemon health via manual_fsv.
- RocksDB CF persistence (CF_FINGERPRINTS, CF_TRAINING_RECORDS).
- existing artefacts/exp2/ contents are consistent with claimed prior runs.
- offline-runnable corpus strata (S1 Shakespeare).

### What sherlock should NOT do
- Run the full `run_exp2.py --phase all` — will crash on B-4.
- Run extract_embeddings without the daemon up + 30 GiB free VRAM — guaranteed fail.
- "Fix" any of the blockers — that's the simplifier (agent-04) or post-pipeline coder.

## Decisions Made

- No production code edits (see `memory/decisions/agent-02-tester--no-production-code-edits.md`).
- Used a test-only KDTree workaround for B-4 to validate KSG algorithm correctness independent of the sklearn-API bug.
- Did not run InfoNCE in M2 because (a) CUDA worked so no patch needed; (b) running InfoNCE would have only confirmed "InfoNCE works at d=32, KSG doesn't" which the spec §2 table already says.
- Treated B-1 as a SPEC defect (typo in §7a "= 91" should be "= 105") rather than a code defect, since the function is mathematically right.

## Discoveries

- B-4 (sklearn radius_neighbors API change) is the single most important runtime defect in the pipeline. Without this fix, NO pair can be estimated.
- B-5 (KSG curse of dimensionality at d=32) is a methodology-level issue that will cascade into spurious cross-validation failures even if B-4 is fixed.
- 4 of 7 spec failure modes (F1, F3, F5, F7) have no automatic detector — Constitution C2 (fail loud) is partially violated.
- AuditChain is wired into ONE phase (verify_environment) — agent-01's "no callers" was a slight overstatement.

## Successes

- 23/23 pytest pass.
- 3/3 agent-01 blockers verified at runtime/source.
- 2 NEW HIGH blockers identified that the static audit could not have caught.
- All M1-M8 deliverables shipped (M9 deferred to sherlock per brief).
- Synthetic validation artefact produced; bootstrap determinism re-confirmed.

## Tests/Build status

- pytest: 23 passed, 0 failed, 0 errors, 0 skipped.
- Build: not exercised (Rust crate untouched; exp2 is pure Python).
- Lint: not exercised (out of scope for tester role).

## Confidence: HIGH

- Every B-1/B-2/B-3 verification was either grep-evidenced or runtime-evidenced.
- Both NEW blockers (B-4, B-5) have published reproductions in the blocker .md files.
- All 16 new tests pass on first invocation post-workaround.
- Zero production code touched, so no risk of introducing regressions.
- Numbers in the synthetic_validation_report.json are reproducible given seed=42.

## Limitations

- Did not run full pipeline (would have crashed on B-4 anyway; coordinator sequencing).
- Did not run InfoNCE recovery (low-risk by spec §2; could be added if needed).
- Did not verify offline corpus strata (S1 Shakespeare); deferred to sherlock.
- Did not personally inspect RocksDB CFs from the prior manual_fsv runs; that is sherlock's job.
