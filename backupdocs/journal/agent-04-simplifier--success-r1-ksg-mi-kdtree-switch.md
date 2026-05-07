---
namespace: journal
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:11:00Z
status: success
severity: high
tags: exp2, ksg, b4, fix, journal-success
---

# R1 SUCCESS — ksg_mi B-4 fix lands clean

## What happened

Applied 6-line replacement in `scripts/exp2/estimators/ksg.py::ksg_mi` switching
`NearestNeighbors.radius_neighbors` to `KDTree.query_radius` (per agent-02's
KDTree validation in test code). Also added the `KDTree` import.

## Pytest results

| When | Total | Pass | Fail | Wall |
|------|-------|------|------|------|
| Before R1 | 23 | 23 | 0 | 2.11s |
| After R1 | 23 | 23 | 0 | 2.13s |

Test `test_b4_production_ksg_mi_runs_at_dim_32` (renamed from `_fails_at_dim_32`)
now asserts success at d=32 — the production-blocking crash is gone. All other
22 tests untouched and still passing.

## Why ksg_entropy was not modified

ksg_entropy uses `kneighbors` (which returns scalar distances per row) and computes
`d * mean(log(eps))` directly — no `radius_neighbors` call. The bug pattern only
appears in ksg_mi where we need to count points within per-query radii in the
marginal spaces u, v.

## What this unblocks

Production pipeline can now run `run_estimators.run_pair` on PCA-32 projected
embeddings without crashing on the first pair. T-016/T-017/T-018 are no longer
gated by B-4 (still gated by GPU memory + B-5 methodology).
