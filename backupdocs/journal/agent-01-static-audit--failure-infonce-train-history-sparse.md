---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:25:00Z
status: active
severity: medium
tags: exp2, infonce, train-history, median-tail, methodology
---

# Defect: InfoNCEEstimator records train/val every 10 steps then uses tail=100 for median

## Where
`scripts/exp2/estimators/infonce.py:102-118`

```python
if step % 10 == 0 or step == train_steps - 1:
    with torch.no_grad():
        train_val = float(self._estimate_tensor(u[train_idx], v[train_idx]).cpu())
        val_val = float(self._estimate_tensor(u[val_idx], v[val_idx]).cpu())
    train_hist.append(train_val)
    val_hist.append(val_val)
    ...
tail = min(100, len(val_hist))
train_median = float(np.median(train_hist[-tail:]))
val_median = float(np.median(val_hist[-tail:]))
```

## Spec
`docs/exp2/03_methodology.md` §4d Reported Quantity:
> For each pair, we report the **median val InfoNCE** over the last 1000 steps of training.

## Observed
- Training records every 10 steps. So 5000 steps → 500 records.
- Tail = 100 records → covers the **last 1000 steps** since 100 × 10 = 1000.

OK. This actually matches spec! BUT only if `train_steps = 5000` AND `step % 10 == 0`. If the user changes `train_steps` to 1000, tail=100 records covers the last 1000 steps still — but that's the entire run.

## Edge case: early stopping
If early stopping fires at step 500 with patience 500 (best_val at step 0, no improvement for 500 steps), `len(val_hist) = 50`, `tail = 50`. The median is over the last 500 steps, not 1000. The spec said 1000.

## Why it matters
- Two pairs with different early-stop timings produce medians over different effective windows. The reported `infonce_estimate_nats` is the median, but the window length varies. Reduces comparability.
- C5 (no hyperparam tuning on reported numbers): the effective tail-window is conditional on early stopping behavior, which is data-dependent.

## Recommended fix
Either:
1. Always use the last 1000 steps (= last 100 records) and document that early-stopped runs report median-over-shorter window with a `infonce_tail_steps` field.
2. Disable early stopping for production (patience > train_steps); keep it for dev.

## Severity
MEDIUM. Won't crash; produces slightly different windows per pair. The cross-validation tolerance is 0.15 nats; the variation should usually be within that.

## Cross-check
Test: write a short fit run, check `len(val_hist)` and validate tail size. Agent-02 should verify.
