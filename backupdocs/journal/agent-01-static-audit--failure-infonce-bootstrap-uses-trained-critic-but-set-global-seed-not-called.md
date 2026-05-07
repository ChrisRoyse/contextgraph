---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:27:00Z
status: active
severity: medium
tags: exp2, infonce, bootstrap, determinism, replay
---

# Issue: bootstrap.estimate uses trained critic; replay determinism depends on critic state matching exactly

## Where
`scripts/exp2/run_estimators.py:62-68` — InfoNCE bootstrap:

```python
inf_low, inf_high, _ = bootstrap_ci(
    lambda idx: estimator.estimate(u[idx], v[idx]),
    ...
)
```

## Spec
`docs/exp2/03_methodology.md` §8:
> - Train each InfoNCE critic *once* on the full 800-input corpus.
> - For each bootstrap resample, evaluate the (already-trained) critic on the resample. This is fast.

Implementation matches spec. Good.

## Concern: replay determinism
The critic is saved via `estimator.save(critic_path)` (run_estimators.py:57). The saved state is `state_dict + hp + seed`.

For replay (`fsv.compare_replay`):
1. Reload critic from disk.
2. Run bootstrap on the same seed.
3. Get same idx sequence.
4. `estimate(u[idx], v[idx])` reads u, v which are the persisted projected.npz arrays.

**If projected.npz differs by even one float, the critic outputs will differ → replay fail.** The PCA fit + apply must be deterministic. Let me verify:
- `fit_projections.fit_pca_models` uses `PCA(svd_solver="full", random_state=cfg["run"]["random_seed"])` — deterministic.
- `apply_pca_models` uses `(x - mean) @ components.T` — deterministic given inputs.
- `extract_embeddings` reads from `get_training_record` MCP tool — deterministic given the same MCP daemon code, same model weights, same input text.

So replay determinism through the chain SHOULD hold. But:
- `set_global_seed` in `fit_projections` is NOT called. Does the PCA `random_state` need it? No, sklearn PCA with `svd_solver="full"` doesn't use random projection.
- `extract_embeddings.py` doesn't call `set_global_seed`. The MCP daemon's reproducibility is its own concern.

## Why this matters for FSV
`fsv.compare_replay` already only checks 3 scalars per pair. If the critic loaded from disk produces slightly different bootstrap outputs (e.g., due to torch determinism subtleties), the replay fails.

## Recommended fix
- Add a determinism canary test: run the full pipeline twice with seed=42 on synthetic data, assert identical output bytes. This is `tests/test_synthetic_validation.py` territory.
- Wire `set_global_seed` to all entry points (currently only InfoNCE.fit calls it; verify_environment.py does NOT).

## Severity
MEDIUM. Replay test in `fsv.py` is the canary. If determinism fails, the issue surfaces there.

## Cross-check
Agent-02: run `pytest scripts/exp2/tests/` and confirm any deterministic-replay tests exist; if not, that's a gap.
