---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:15:00Z
status: active
severity: medium
tags: exp2, run-estimators, bootstrap, seed, ksg, jitter
---

# Defect: bootstrap_ci uses jitter_seed=int(bcfg["seed"]) for KSG, sharing seed across all 91 pairs and across the bootstrap-resample-loop

## Where
`scripts/exp2/run_estimators.py:69-75`

```python
ksg_low, ksg_high, _ = bootstrap_ci(
    lambda idx: ksg_mi(u[idx], v[idx], k=k, jitter_seed=int(bcfg["seed"])),
    n_items=u.shape[0],
    n_resamples=n_resamples,
    seed=int(bcfg["seed"]),
    confidence=float(bcfg["confidence"]),
)
```

## Spec
`tasks/exp2/lessons.md` 2026-04-28:
> production KSG still fails on duplicate raw data. Bootstrap-only KSG uses deterministic `1e-12` tie-break jitter and records that duplicate policy in each pair result.

`docs/exp2/03_methodology.md` §6: bootstrap seed=42.

## Observation
- `jitter_seed=int(bcfg["seed"])` is **42** for every pair AND for every bootstrap resample.
- That means the same noise vector is added during EVERY bootstrap call for EVERY pair.
- For deterministic-jitter, this is correct in principle — but it means the (E1, E10) bootstrap and the (E2, E14) bootstrap apply the same `np.random.default_rng(42)` realization on different data shapes.
- Each `bootstrap_ci` resampled-batch calls `ksg_mi(u[idx], v[idx], k=k, jitter_seed=42)` repeatedly. The jitter is recomputed inside `ksg_mi` from `seed=42`, so on each call the SAME jitter is added (assuming idx-independent jitter generation). **This is consistent** within one pair across resamples (good for determinism).
- Across pairs, all use jitter from seed=42. As long as the jitter is small (1e-12) this is fine. But if the bootstrap somehow uses jitter to break ties at the corpus level, *all 91 pairs* could co-vary on the same noise pattern. Need to verify ksg.py jitter-application semantics.

## To verify in ksg.py
- Does `jitter_seed` regenerate jitter PER-CALL (deterministic jitter for the same input), or PER-BOOTSTRAP-RESAMPLE?
- Will read ksg.py next.

## Recommended fix candidate
Per-pair distinct jitter seeds: `jitter_seed=int(bcfg["seed"]) + i*100 + j` mirrors the InfoNCE seed pattern at line 53. Decouples pair statistics.

## Severity
MEDIUM. Determinism preserved; correctness depends on jitter semantics.

## Cross-check
Will inspect ksg.py to confirm jitter behavior.
