---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:26:00Z
status: active
severity: medium
tags: exp2, infonce, determinism, seed, side-effect
---

# Defect: InfoNCEEstimator.fit() calls set_global_seed(seed) — mutates GLOBAL torch/numpy/random state per pair

## Where
`scripts/exp2/estimators/infonce.py:60` — `set_global_seed(self.seed)` inside `fit()`.

## Background
`set_global_seed` (common.py:161-174) sets:
- `random.seed(seed)`
- `np.random.seed(seed)`
- `torch.manual_seed(seed)`
- `torch.cuda.manual_seed_all(seed)`
- `torch.backends.cudnn.deterministic = True`
- `torch.use_deterministic_algorithms(True, warn_only=True)`

These are **process-global** state. Calling `fit` for pair (E1, E2) at seed=4202 mutates global RNG to seed=4202. Then anyone else who uses `np.random` or `random` AFTER fit is using the seed=4202 stream, not their own seed.

## Why it matters
- After `run_estimators.run_pair(cfg, arrays, 0, 1, ...)` returns, NumPy global RNG is at the post-fit-of-pair-(0,1) state, not at the original seed=42.
- Subsequent `bootstrap_ci` uses `np.random.default_rng(seed)` — that has its own state, OK.
- But `random.Random(seed)` calls used elsewhere (e.g., shuffling) MAY interact with the global random state if they use `random.seed`.
- Cross-pair determinism: pair (E1, E2) trains differently if the seed=4202 mutation happens before vs after another pair. Currently the loop runs pairs sequentially, each with its own set_global_seed; OK.
- However: an `InfoNCEEstimator.estimate` call after `fit` does NOT reset the seed; it uses whatever the current global state is. As long as no random sampling happens in `estimate`, it's deterministic. (Verified: `_estimate_tensor` does not sample.)

## Recommended fix
Use a per-instance generator instead of global state:
```python
gen = torch.Generator(device=self.device).manual_seed(self.seed)
# pass gen to torch.randperm, torch.randint
```
The implementation already does `torch.Generator(device=self.device).manual_seed(self.seed + step)` (line 96) for batch sampling — that's the right pattern. The `set_global_seed(self.seed)` at line 60 is then redundant for InfoNCE's own randomness; it's there for cuDNN deterministic algorithms.

Move the `torch.use_deterministic_algorithms(True)` and cuDNN flags to a one-time setup at process start, not per-fit. Then drop the per-fit `set_global_seed` call.

## Severity
MEDIUM. Determinism preserved within a single process by virtue of sequential order. Side-effect on global state is a Python footgun.
