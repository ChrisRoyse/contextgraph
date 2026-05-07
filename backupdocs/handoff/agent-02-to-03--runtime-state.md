---
namespace: handoff
agent_id: agent-02-tester
timestamp: 2026-04-29T00:35:00Z
status: active
tags: exp2, handoff, agent-03-sherlock, runtime-state
---

# Handoff: agent-02-tester → agent-03-sherlock

## Pytest summary

| Run | Tests | Passed | Failed | Errors | Wall |
|-----|-------|--------|--------|--------|------|
| Baseline (existing 7) | 7 | 7 | 0 | 0 | 2.96s |
| New synthetic sweep | 16 | 16 | 0 | 0 | 0.98s |
| **Combined (23 total)** | 23 | 23 | 0 | 0 | 2.09s |

Logs at `/tmp/agent-02-pytest-baseline.log` and `/tmp/agent-02-pytest-final.log`.

## Synthetic Gaussian recovery (M2/T-013)

KDTree-based KSG-1 (workaround for B-4) at the spec'd production dim=32, n=800, k=5, seed=42:

| rho | truth nats | ksg nats | abs err | meets <0.20? |
|-----|------------|----------|---------|--------------|
| 0.00 | 0.000 | 0.029 | 0.029 | YES |
| 0.10 | 0.161 | 0.035 | 0.126 | YES |
| 0.30 | 1.509 | 0.138 | 1.371 | **NO** |
| 0.50 | 4.603 | 0.427 | 4.176 | **NO** |
| 0.70 | 10.774 | 1.004 | 9.770 | **NO** |
| 0.90 | 26.572 | 2.215 | 24.357 | (not graded — rho>0.7) |
| 0.99 | 62.673 | 3.935 | 58.737 | (not graded — rho>0.7) |

KSG passes only the independence check (rho=0). At every other rho it severely underestimates. **This is the curse of dimensionality at the production d=32**, exactly as the spec §2 table warns. The spec §3a "0.15 nats recovery for rho ≤ 0.7" is unachievable.

Artefact: `artefacts/exp2/synthetic_validation_report.json` — 7 rho rows.
Artefact: `artefacts/exp2/ksg_k_sweep_rho_0.5.json` — k ∈ {3, 5, 10}: spread = 0.146 nats > spec threshold 0.1 → methodology revisit triggered.

## Agent-01 blocker verification

| ID | Status | Verdict |
|----|--------|---------|
| **B-1** structural_upper_bound 105 vs 91 | ✓ Confirmed at runtime | Function correctly produces 105 on zero-MI synthetic input. The implementation is mathematically right; the spec has a typo. Coordinator decides which to change. |
| **B-2** AuditChain never called | ✓ Confirmed (refined) | AuditChain IS instantiated once in `verify_environment.py:98`, but only writes `audit_chain_environment.json`. Orchestrator has no `audit_chain` phase; no `finalize()` call; `audit_chain.json` never produced. Agent-01's "no callers at all" was slightly incomplete — there is one stub call. |
| **B-3** per-pair JSON missing 8 fields | ✓ Confirmed exactly | regex source extraction shows precisely 8 missing fields: code_commit_sha, corpus_manifest_sha256, embedder_j_weight_sha256, embedder_k_weight_sha256, hyperparams_sha256, pca_j_sha256, pca_k_sha256, redundancy. Agent-01's count is exactly correct. |

## NEW HIGH-severity blockers found by agent-02

| ID | Description | File |
|----|-------------|------|
| **B-4** | `ksg_mi` raises `TypeError` at dim ≥ 20 on sklearn ≥ 1.3 (production uses dim=32) — every pair will fail on first call | `memory/blockers/agent-02-tester--ksg-radius-neighbors-array-broken-on-sklearn-1.8.md` |
| **B-5** | KSG-1 catastrophically underestimates Gaussian MI at d=32 (curse of dimensionality). Spec self-contradicts (§2 admits curse vs §3a demands 0.15-nat recovery). Cross-validation gate (F6) will fire spuriously on every high-MI pair. | `memory/blockers/agent-02-tester--ksg-curse-of-dimensionality-at-d-32.md` |

## Failure-mode coverage (M8)

3/7 detected, 4/7 silent. See `memory/discoveries/agent-02-tester--failure-mode-coverage-runtime-evidence.md`.

| Mode | State |
|------|-------|
| F1 implausibly low MI | Silent |
| F2 implausibly high (overfit) | Detected (infonce.py:120) |
| F3 InfoNCE non-convergence | Silent |
| F4 KSG NaN | Detected (ksg.py:_validate_matrix) |
| F5 wide bootstrap CI | Silent |
| F6 cross-val < 80% | Detected (run_estimators.py:122) — but will fire spuriously due to B-5 |
| F7 N_eff method divergence | Silent |

## Files modified

NONE (agent-02 made zero production-code edits). See `memory/decisions/agent-02-tester--no-production-code-edits.md` for rationale.

## Files added

| Path | Purpose |
|------|---------|
| `scripts/exp2/tests/test_synthetic_full_sweep.py` | 16 pytest cases for T-013 sweep, B-4 reproduction, k-sweep, bootstrap determinism, F4 detection |
| `artefacts/exp2/synthetic_validation_report.json` | M2 deliverable — 7 rho rows |
| `artefacts/exp2/ksg_k_sweep_rho_0.5.json` | M6 deliverable — k-sweep stability |

## Concrete commands for agent-03-sherlock

### Tier 1: CPU-only smoke (no daemon)
```bash
cd /home/cabdru/contextgraph
python3 -m pytest -v scripts/exp2/tests
# Expected: 23 passed (7 baseline + 16 new). Anything else means regression.
```

### Tier 2: T-001 environment verifier (hits real GPU)
```bash
python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml --skip-cargo
# Expected: fails with "VRAM < 30000 MiB" if GPU is loaded; passes if free.
# Side-effect: produces artefacts/exp2/audit_chain_environment.json
```

### Tier 3: Live MCP daemon — manual FSV
```bash
# Confirm daemon binary exists:
ls -lh target/release/context-graph-mcp
# Verify weights present (E1 first):
ls models/

# Then run manual_fsv (it spawns its own MCP child via subprocess):
python3 scripts/exp2/manual_fsv.py --config scripts/exp2/exp2.yml
# Expected per agent-01 success notes: 3 happy + 3 edge cases pass.
# Output: artefacts/exp2/manual_fsv_log.json
# Source-of-truth check: agent-03 should open the RocksDB CFs in
# artefacts/exp2/manual_fsv_rocksdb_*/ and verify the fingerprints persisted.
```

### Tier 4: Inspect the existing manual_fsv artefacts
The current state already has:
- `artefacts/exp2/manual_fsv_log.json` (from prior runs)
- `artefacts/exp2/manual_fsv_rocksdb_*` (RocksDB instances)
- `artefacts/exp2/audit_chain_environment.json` (from prior verify_environment run)
- `artefacts/exp2/verification_log.json` (from prior verify_environment run)
- `artefacts/exp2/run.log.jsonl` (from prior runs)

Sherlock should compare these against the latest run output to verify determinism.

### Tier 5: ConfRm B-4 in production (if curious)
```bash
# Reproduce the sklearn radius_neighbors bug:
python3 -c "
import numpy as np
from scripts.exp2.estimators.ksg import ksg_mi
rng = np.random.default_rng(0)
u = rng.normal(size=(200, 32))
v = rng.normal(size=(200, 32))
ksg_mi(u, v, k=5)  # Should raise TypeError
"
```

### Tier 6: Things NOT to run
- `scripts/exp2/build_corpus.py` — needs internet (HuggingFace datasets, BEIR, Wikipedia, FLORES). Will fail fast on offline boxes; document but skip.
- `scripts/exp2/extract_embeddings.py` — needs all 14 weights + MCP daemon up + 30 GiB free VRAM. Likely blocked.
- `scripts/exp2/run_estimators.py` — would crash on first pair due to B-4 + B-5.
- `scripts/exp2/run_exp2.py --phase all` — same as above plus skip the missing audit_chain phase.

### How to interpret manual_fsv outcomes
- 6 cases pass → MCP daemon healthy, RocksDB schema intact, embedders producing valid fingerprints. This is the foundation; without it, EXP-2 cannot run regardless of other fixes.
- 6 cases fail → daemon or weights regressed. Capture full traceback; do not silently retry.
- Partial pass (3-5/6) → flake or partial regression; investigate specific case.

### What sherlock should write
- `memory/journal/agent-03-sherlock--manual-fsv-{success|failure}-{descriptor}.md` per case.
- `memory/journal/agent-03-sherlock--rocksdb-cf-inspection.md` after looking at the persisted CFs.
- `memory/handoff/agent-03-to-04--live-state.md` summarizing daemon state, RocksDB integrity, and any new findings.

## Critical context for sherlock

The pipeline is currently NOT runnable end-to-end:
1. B-4 makes ksg_mi crash at dim=32 → blocks `run_estimators.py`.
2. B-5 means even after B-4 fix, KSG values would fail the cross-val gate.
3. B-2 means no audit chain even if 1+2 were fixed.
4. T-001 likely fails on GPU memory.

Therefore sherlock's mission is **NOT** to run the full pipeline. It's to verify:
- The MCP daemon is healthy (manual_fsv).
- RocksDB CFs contain what the manual_fsv produced.
- The verify_environment honest fail (T-001 / Constitution C16).
- Build corpus phase (S1 Shakespeare, partial S2 code) — the offline-runnable strata only.

## Confidence: HIGH for all M1-M8 verification work; MEDIUM for the B-5 implication claim (theoretical but well-grounded).
