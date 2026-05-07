---
namespace: discoveries
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:58:00Z
status: documented
severity: low
tags: exp2, ksg, b4, scope-clarification
---

# Discovery — `ksg_entropy` does NOT exhibit the B-4 bug pattern

## Context

agent-03's handoff (`memory/handoff/agent-03-to-04--live-state.md`) and
agent-02's blocker file (B-4) both suggested the fix should "apply also to
ksg_entropy". The brief R1 also says:

> Apply also to ksg_entropy if it has the same bug pattern.

## What I found

`scripts/exp2/estimators/ksg.py::ksg_entropy` (lines 54-70) does NOT call
`NearestNeighbors.radius_neighbors`. It uses `kneighbors` only:

```python
nn = NearestNeighbors(metric="chebyshev", n_neighbors=k + 1).fit(x)
distances, _ = nn.kneighbors(x)
eps = distances[:, k]
# ... computes h = digamma(n) - digamma(k) + d * mean(log(eps)) + d * log(2)
```

Kozachenko-Leonenko entropy uses the SCALAR distance to the kth neighbour
per row, then averages `log(eps)` — no per-row radius queries needed.
The bug pattern (passing an array radius to `radius_neighbors`, which
crashes on sklearn 1.8 cython path) cannot occur here.

## Verification

Direct probe at d=32:

```
$ python3 -c "
import numpy as np
from scripts.exp2.estimators.ksg import ksg_entropy
rng = np.random.default_rng(42)
x = rng.normal(size=(200, 32))
print(ksg_entropy(x, k=5))
"
```

Returns finite value without exception. (Result subject to B-5 curse-of-
dimensionality bias, same as ksg_mi — that's a methodology issue, not B-4.)

## Implication for R1

Only `ksg_mi` was modified. `ksg_entropy` was left untouched. This is
correct minimal-impact behaviour.

## Cross-link

- agent-02 B-4 blocker incorrectly suggests applying the fix to ksg_entropy.
  The static-text "ksg_entropy line 64" referenced in the blocker is in
  fact `eps = distances[:, k]` — a scalar slice, not a `radius_neighbors`
  call. agent-02 was conservative; their suggestion was a precaution. The
  precaution is unnecessary.
