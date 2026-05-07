---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:28:00Z
status: active
severity: low
tags: exp2, ksg, direct, implementation, success
---

# Success: estimators/ksg.py implements KSG-1 directly with sklearn NearestNeighbors

## What's good
- KSG-1 algorithm correct: `digamma(k) + digamma(n) - mean(digamma(n_u+1) + digamma(n_v+1))` (line 46).
- Chebyshev distance metric (line 39, 42, 43) — matches Kraskov et al. 2004 KSG-1.
- `eps = np.nextafter(distances[:, k], 0.0)` (line 41) avoids the boundary-inclusion issue of KSG (the k-NN distance must be strictly greater than the included neighbors).
- `_validate_matrix` (lines 14-25) enforces:
  - 2D shape with N >= 4
  - finite values (no NaN/Inf)
  - duplicate-row detection: raises if duplicates AND no jitter_seed; if jitter_seed, applies deterministic 1e-12 noise. Matches `tasks/exp2/lessons.md` 2026-04-28.
- Returns clamped non-negative result (line 51).

## ksg_entropy implementation
- Standard Kozachenko-Leonenko: `digamma(n) - digamma(k) + d * mean(log(eps)) + d * log(2.0)`.
- The `d * log(2.0)` term is the unit-ball volume factor for L_inf in d dimensions (volume = 2^d).
- Fails on zero `eps` (line 64) — prevents log(0).

## Compliance
- C2 fail-loud at every guard.
- C5: hyperparameters (k=5) read from config.
- The implementation matches `03_methodology.md` §5a/§5c spec.

## Note
The implementation is "sklearn_direct" — the path the exp2.yml specifies. NPEET is not used. This explains the spec/yml drift flagged earlier (`agent-01-static-audit--failure-exp2yml-ksg-implementation-mismatch.md`). Either change yml back to npeet (and add npeet to requirements.txt) or update the spec to match the current sklearn_direct implementation.

## Performance note
`NearestNeighbors.radius_neighbors` returns *all* points within radius. For 800-input corpus and 32-D, this is quick. For larger corpora it gets slow.
