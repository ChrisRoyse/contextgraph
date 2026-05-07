---
namespace: journal
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:16:00Z
status: active
severity: high
tags: exp2, b4, ksg, sklearn, runtime-confirmed, ship-blocker
---

# B-4 KSG `radius_neighbors(radius=array)` failure at dim=32 — independently reproduced

## Repro
```
python3 -c "
import numpy as np, sys; sys.path.insert(0, '.')
from scripts.exp2.estimators.ksg import ksg_mi
np.random.seed(42)
u = np.random.randn(800, 32).astype(np.float32)
v = np.random.randn(800, 32).astype(np.float32)
ksg_mi(u, v, k=5)
"
```

## Stack trace (verbatim)
```
File "scripts/exp2/estimators/ksg.py", line 44, in ksg_mi
    n_u = np.array([len(x) - 1 for x in nn_u.radius_neighbors(u, radius=eps, return_distance=False)])
File "sklearn/neighbors/_base.py", line 1195, in radius_neighbors
    results = RadiusNeighbors.compute(...)
File "sklearn/metrics/_pairwise_distances_reduction/_dispatcher.py", line 421, in compute
    return RadiusNeighbors64.compute(...)
File "sklearn/metrics/_pairwise_distances_reduction/_radius_neighbors.pyx", line 59
TypeError: only 0-dimensional arrays can be converted to Python scalars
```

## sklearn version on this box
Live error path is `_radius_neighbors.pyx:59` — confirms sklearn ≥ 1.3 cython optimised path; it expects scalar `radius`. Production passes a length-N array → instant crash.

## Implication
- Production `pca.target_dim: 32` per `scripts/exp2/exp2.yml`.
- `run_estimators.run_pair` calls `ksg_mi` per pair → first pair crashes → orchestrator dies → 0 of 91 pairs estimated.
- Same code path in `ksg_entropy` (line 64) → effective_rank's H estimates also crash.
- Bootstrap loop calls ksg_mi 10,000 × 91 → same bug.

## Cross-link
- Agent-02 blocker: `memory/blockers/agent-02-tester--ksg-radius-neighbors-array-broken-on-sklearn-1.8.md`
- Agent-02 final report cited fix: switch to `KDTree.query_radius(r=eps, count_only=True)`.

## Verdict
INDEPENDENTLY CONFIRMED at runtime. B-4 is a real, reproducible ship-blocker. No coordinator override possible — the code physically cannot complete a single pair.

## Confidence: HIGH (live exception captured, traceback shown)
