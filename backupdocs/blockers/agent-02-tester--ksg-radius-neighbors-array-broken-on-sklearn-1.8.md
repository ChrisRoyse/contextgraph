---
namespace: blockers
agent_id: agent-02-tester
timestamp: 2026-04-29T00:00:00Z
status: active
severity: high
tags: exp2, ksg, sklearn, ship-blocker, production-pipeline-broken, b4
---

# BLOCKER B-4 (NEW, HIGH): `ksg_mi` raises TypeError at dim>=20 on sklearn 1.8.0 — entire 91-pair sweep would fail on first call

## Symptom (runtime evidence)

`scripts/exp2/estimators/ksg.py` line 44:

```python
n_u = np.array([len(x) - 1 for x in nn_u.radius_neighbors(u, radius=eps, return_distance=False)])
```

Where `eps` is a length-N numpy array of per-query radii (from kNN distances).

When called with `dim >= 20` (production uses dim=32), sklearn 1.8.0's `radius_neighbors` raises:

```
TypeError: only 0-dimensional arrays can be converted to Python scalars
  at sklearn/metrics/_pairwise_distances_reduction/_radius_neighbors.pyx:59
```

## Reproduction

```python
python3 -c "
import numpy as np
from sklearn.neighbors import NearestNeighbors
for dim in [1, 2, 5, 10, 20, 32]:
    rng = np.random.default_rng(42)
    x = rng.normal(size=(100, dim)).astype(np.float64)
    nn = NearestNeighbors(metric='chebyshev', n_neighbors=6).fit(x)
    d, _ = nn.kneighbors(x)
    eps = np.nextafter(d[:, 5], 0.0)
    try:
        idx = nn.radius_neighbors(x, radius=eps, return_distance=False)
        print(f'dim={dim}: OK')
    except Exception as e:
        print(f'dim={dim}: FAIL {type(e).__name__}')
"
```

Output:
```
dim=1: OK
dim=2: OK
dim=5: OK
dim=10: OK
dim=20: FAIL TypeError
dim=32: FAIL TypeError
```

## Root cause

sklearn 1.8.0's `NearestNeighbors.radius_neighbors` documents `radius : float, default=None` — it is **scalar only**. Older sklearn versions (≤ 1.2) silently accepted an array via brute-force fallback. Newer versions route through the optimized `_radius_neighbors.pyx` path which expects a scalar.

`scripts/exp2/estimators/ksg.py` was written assuming array radius is supported. It works at `dim<=10` because sklearn falls back to brute-force at low dim (which still tolerates the array). At `dim>=20` (production!), it routes through the optimized path and FAILS.

## Why the test suite passed

The existing `test_ksg_recovers_low_dimensional_gaussian_mi` uses `dim=1` and `dim=1` for `test_ksg_independence_near_zero`. Both pass. NO TEST in the existing suite exercises ksg_mi at dim ≥ 20. Agent-01's static audit didn't catch this because it's an API-version compat issue, not a logic bug.

## Production impact (HIGH ship-blocker)

- The full pipeline projects ALL 14 embedders to **dim=32 via PCA** (per spec §3, exp2.yml `pca.target_dim: 32`).
- `run_estimators.run_pair` calls `ksg_mi(u, v, k=5)` on those 32-D arrays for all 91 pairs.
- The FIRST pair will throw `TypeError`, propagating up through `run_estimators.main()` and crashing the orchestrator.
- Bootstrap (`run_estimators` line 69) also calls `ksg_mi` 10,000 × 91 times — same failure.
- Effective-rank (`effective_rank.estimate_entropies`) calls `ksg_entropy` which has IDENTICAL code path at line 64 (eps is array, then `nn.radius_neighbors(... radius=eps, ...)`). Same failure.

**No 91-pair production run can complete with the current code on sklearn ≥ 1.3.**

## Workaround in test code (this agent's path)

For my synthetic-validation pytest, I will call `ksg_mi` row-by-row in a Python loop or compute counts via `kdt.query_radius` (sklearn's BallTree/KDTree which DOES still support array radius). I will NOT fix the production code — that's a separate defect for the simplifier/coder.

Alternative workaround: use `nn_u.radius_neighbors(u[i:i+1], radius=float(eps[i]))` per row.

## Fix recommendation (for simplifier/coder)

Replace lines 44-45 of `scripts/exp2/estimators/ksg.py` with sklearn's `KDTree.query_radius` (which still accepts array radius) or per-row loop:

```python
# Option A: KDTree (faster, supports array radius)
from sklearn.neighbors import KDTree
kdt_u = KDTree(u, metric='chebyshev')
counts_u = kdt_u.query_radius(u, r=eps, count_only=True) - 1  # -1 to exclude self
kdt_v = KDTree(v, metric='chebyshev')
counts_v = kdt_v.query_radius(v, r=eps, count_only=True) - 1

# Option B: per-row (slower, simpler)
counts_u = np.array([
    nn_u.radius_neighbors(u[i:i+1], radius=float(eps[i]), return_distance=False)[0].size - 1
    for i in range(n)
])
```

Apply same fix to `ksg_entropy` line 64.

## Severity rationale (HIGH)

- Ship-blocker: production pipeline cannot run.
- Detected only because I ran a dim=32 test; the existing dim=1 tests miss it.
- Constitution C2 (fail loud): ironically, KSG fails LOUD here — but the failure is an API error, not the intended "non-finite" or "negative MI" diagnostic. Still surfaces the bug, but the error message is unhelpful.

## Cross-link
- Spec docs/exp2/03_methodology.md §5c: `we use ... npeet ... or implement directly with sklearn.neighbors.NearestNeighbors`
- exp2.yml line 42: `implementation: sklearn_direct`
- Agent-01 journal `failure-exp2yml-ksg-implementation-mismatch.md` — flagged exp2.yml says sklearn_direct vs spec saying npeet, but did NOT catch this version-compat bug.
