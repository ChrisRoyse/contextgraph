---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:24:00Z
status: active
severity: high
tags: exp2, infonce, overfit, fail-loud, pipeline-blocker
---

# Defect: InfoNCEEstimator.fit() RAISES on overfit gap > threshold instead of flagging + retraining with smaller critic

## Where
`scripts/exp2/estimators/infonce.py:120-121`

```python
gap = train_median - val_median
if gap > float(self.hp["overfit_gap_threshold_nats"]):
    raise Exp2Error(f"InfoNCE overfit gap {gap:.6f} exceeds threshold")
```

## Spec
`docs/exp2/03_methodology.md` §4c:
> Stopping criterion: train-val gap > 0.1 nats → flagged as overfit, run with smaller critic (32 → 32 → 32).

`docs/exp2/06_atomic_tasks.md` T-016 DoD:
> [ ] Pairs with train-val gap > 0.1 nats are flagged + re-trained with smaller critic.

`docs/exp2/07_validation_harness.md` F2 (Pair MI Estimate Implausibly High) Resolution:
> Re-run with smaller critic.
> If overfitting persists: KSG-only result for this pair, with documented caveat.

## Observed
The implementation raises a hard error on overfit gap > threshold, halting the entire 91-pair sweep. The spec says "flag and re-train"; F2 says rerun-with-smaller-critic, then if STILL overfit, fall back to KSG-only with a caveat.

## Why this is HIGH-severity
- Production run on 91 pairs is ~5 hours GPU. If pair 47 hits an overfit gap (entirely possible for highly redundant pairs like E1/E10 where the critic memorizes the joint), pairs 48-91 never run.
- The spec's two-tier remediation (smaller critic, then KSG-only) is missing.
- Constitution C2 wants "fail loud", but the methodology spec defines exactly *how* to handle this case — flag and retry, not crash the whole sweep.

## Recommended fix
1. Don't raise. Instead, flag in result: `info.overfit = True`.
2. Caller (`run_estimators.run_pair`) decides:
   - First attempt: full critic.
   - On overfit: rerun with `critic_hidden_dim=32, critic_depth=2` (smaller).
   - On second overfit: emit row with `infonce_estimate_nats=NaN` (or KSG-only flag) plus diagnostic.
3. Update T-020 cross_validation_report to handle pairs with KSG-only.

## Cross-check
Synthetic test that constructs a high-MI joint distribution and runs `InfoNCEEstimator.fit` to confirm raise behavior would catch this. Currently `tests/test_estimators.py` should be checked.

## Severity
HIGH. Blocks production run reliability.
