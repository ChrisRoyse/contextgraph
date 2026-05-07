---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:09:00Z
status: active
severity: medium
tags: exp2, pca, fallback, fit-projections, spec-mismatch
---

# Defect: fit_projections.py raises Exp2Error when explained_variance < threshold instead of falling back to PCA-64

## Where
`scripts/exp2/fit_projections.py:42-49`

## Spec
`docs/exp2/03_methodology.md` §3a:
> Pass criterion: explained variance ≥ 0.85. Fail criterion: a single embedder retains < 0.70 variance — that embedder is upgraded to PCA-64 with audit-chain note.

`docs/exp2/06_atomic_tasks.md` T-009 DoD:
> [ ] If any embedder < 0.85 explained variance, refits with d=64; logged.

## Observed
```python
if explained < threshold:
    raise Exp2Error(
        f"{key}: PCA-{target_dim} retains {explained:.6f}, below {threshold}. "
        "Do not proceed. Pre-register a higher target_dim and rerun T-009."
    )
```

So if any embedder fails the 0.85 threshold, the entire fit fails. There is **no automatic d=64 fallback**, contradicting both methodology §3a (mandatory upgrade for < 0.85) and the T-009 DoD (refit at d=64 if < 0.85).

## Why it matters
- Methodology §3a's two-tier fallback (between 0.70 and 0.85: upgrade to d=64; below 0.70: investigation) is a **pre-registered** path. The implementation makes any single-embedder variance miss a hard fail. That's stricter than the methodology, but it's a deviation.
- C5: pre-registered hyperparams must be honored. The methodology pre-registered `fallback_dim: 64`. Excluding the fallback path means the operator cannot run the pipeline if even one embedder's PCA-32 retains 0.84 variance.
- C2: fail-loud is correct *only when* the spec says to fail. Here the spec says to fall back, so failing instead is silent methodology drift.

## Recommended fix
Implement the spec's two-tier fallback:
```python
if explained < threshold:  # below 0.85
    # try d=64
    fallback_dim = int(cfg["pca"].get("fallback_dim", 64))  # also re-add fallback_dim to yaml
    pca = PCA(n_components=fallback_dim, svd_solver="full", random_state=...)
    pca.fit(x)
    explained = float(np.sum(pca.explained_variance_ratio_))
    if explained < 0.70:  # methodology hard floor
        raise Exp2Error(f"{key}: PCA-{fallback_dim} retains {explained:.6f}, below 0.70 hard floor")
    # log the upgrade in audit chain
```

## Cross-check
The code in `apply_pca_models` (line 71-78) assumes a single fixed `target_dim` per embedder. After fix, the projected dim becomes per-embedder. Downstream `effective_rank.py` and `run_estimators.py` must handle 32-D vs 64-D pairs.

## Severity
MEDIUM. Will block any production run where one embedder's variance < 0.85. Not a runtime bug if all embedders happen to clear 0.85, but a fragile pre-condition.
