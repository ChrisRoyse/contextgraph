---
namespace: journal
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:55:00Z
status: success
severity: high
tags: exp2, audit-chain, provenance, b2, b3, journal-success
---

# R2 + R3 SUCCESS — orchestrator audit chain + 8 per-pair provenance fields

## R3 (per-pair provenance fields)

Added `EMBEDDER_WEIGHT_FILES` mapping (14 entries, 4 None for PE/HDC), helper
`load_provenance()`, lookup `embedder_weight_sha256()` in run_estimators.py.
The `run_pair` function now writes 8 new fields per spec §13:

- `code_commit_sha`
- `corpus_manifest_sha256`
- `embedder_j_weight_sha256`
- `embedder_k_weight_sha256`
- `pca_j_sha256`
- `pca_k_sha256`
- `hyperparams_sha256`
- `redundancy` (initially None; backfilled by effective_rank)

`backfill_pair_redundancy(rows, entropy, out_dir)` in effective_rank.py rewrites
each pair JSON in place once entropies are known.

## R2 (master audit chain)

`run_exp2.py::finalize_master_audit_chain()` reads per-phase chains
(`audit_chain_environment.json`, `audit_chain_estimators.json`,
`audit_chain_n_eff.json`) plus 11 headline deliverables plus all
`pair_results/E*_E*.json` rows, and writes `audit_chain.json` with
`chain_sha256` (64 hex). Always runs after the phase loop, even on
partial runs. Missing artefacts surface as `artefact_missing` /
`phase_chain_missing` events.

`run_estimators.py` and `effective_rank.py` both accept `--audit-chain`
flag now. Orchestrator forwards the path automatically.

## Pytest results

| When | Total | Pass | Fail | Wall |
|------|-------|------|------|------|
| Before R1 | 23 | 23 | 0 | 2.11s |
| After R1 (KSG fix) | 23 | 23 | 0 | 2.13s |
| After R3 (provenance fields) | 25 | 25 | 0 | 1.83s |
| After R2 (audit chain orchestrator) | 26 | 26 | 0 | 2.06s |

26/26 PASS. 3 new contract tests added — none of the original 23 changed
behavior, all still green.

## Production sanity check (post-fix)

```
$ python3 -c "from scripts.exp2.estimators.ksg import ksg_mi; ..."
dim=32 n=800 k=5 MI=0.0289 (independent Gaussians, expected ~0)
dim=32 rho=0.5 MI=0.4461 (KSG underestimate documented in B-5)
```

The B-4 sklearn cython crash at d=32 is fixed. The B-5 underestimate (~91%
at rho=0.5) is unchanged — that's a methodology decision for the operator,
not a code fix.

## What R2 + R3 unblock

- Constitution C7 (audit chain before viz) is now enforceable: every run
  produces `audit_chain.json` with `chain_sha256`.
- Spec §13 per-pair row schema is complete; external reviewers can re-derive
  any row from corpus + weights + code + hyperparams.
- agent-01's B-2 + B-3 blockers are addressable. (Whether they're fully
  resolved depends on a successful production run, which is gated by GPU
  memory per agent-03.)

## Files modified (for the record)

- `scripts/exp2/estimators/ksg.py` (R1)
- `scripts/exp2/run_estimators.py` (R3, R2-helper)
- `scripts/exp2/effective_rank.py` (R3-backfill, R2-helper)
- `scripts/exp2/run_exp2.py` (R2)
- `scripts/exp2/tests/test_synthetic_full_sweep.py` (R1+R3+R2 tests)
