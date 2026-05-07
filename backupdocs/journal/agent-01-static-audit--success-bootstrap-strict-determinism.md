---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:23:00Z
status: active
severity: low
tags: exp2, bootstrap, success, determinism, fail-loud
---

# Success: estimators/bootstrap.py is strict and deterministic

## What's good
- Uses `np.random.default_rng(seed)` (line 25) — the modern, deterministic, thread-safe RNG. Same seed → identical idx sequence.
- Resamples size n_items, with replacement (line 28). Standard percentile bootstrap.
- Fails on n_items <= 1 OR non-positive n_resamples (lines 21-24). Good guard.
- Fails on non-finite point_fn return (line 30). C2 fail-loud.
- Returns the full bootstrap distribution (`values`) alongside CI — caller can compute median, std, density. Useful for diagnostics.
- `np.quantile(values, [alpha, 1.0 - alpha])` — vectorized, well-tested.

## Notes
- `seed` defaults to 42 (matches §6 hyperparameters).
- `confidence` defaults to 0.95 (matches spec).
- The function signature is exactly the spec'd interface from `05_technical_spec.md` §11 with one improvement: returns the bootstrap vector too.

## Compliance
- C4: every bootstrap is at least n_resamples=10000 by default; matches spec.
- C6: deterministic via fixed seed.
- Fully aligned with spec.
