---
namespace: discoveries
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:40:00Z
status: deferred
severity: medium
tags: exp2, infonce, overfit, retry, spec-compliance, deferred
---

# R6 DEFERRED — InfoNCE overfit-gap retry-with-smaller-critic

## Why I'm deferring (judgment per brief)

The brief authorized this fix conditionally:

> "This is the most behavior-altering fix. If it's too risky to do safely in
> this session, document the proposed diff in a discovery file and DEFER.
> Use judgment."

The current `InfoNCEEstimator.fit()` raises `Exp2Error` on overfit gap > threshold,
which agent-01 correctly identified as a pipeline-halting defect (ref:
`memory/journal/agent-01-static-audit--failure-infonce-overfit-raise-blocks-pipeline.md`).
The spec contract is:

1. First fit → flag `overfit=True` instead of raising.
2. Caller retries with a smaller critic (32→32→32 instead of 64→64→64 dims).
3. If retry STILL overfits → emit row with `infonce_estimate_nats=NaN` plus a
   `ksg_only=True` flag, do NOT halt.
4. `cross_validation_report` and downstream `effective_rank` must tolerate NaN
   rows.

The reason for deferral: this single behavior change cascades through 4 modules
(infonce, run_estimators, effective_rank, visualize) and introduces a NaN
propagation surface that is impossible to verify deterministically without an
actual production run. NaN in `mi_matrix_from_rows` would corrupt
`spectrum_entropy` eigenvalues and `schur_complement` SVD silently — a real
correctness risk that exceeds the value of patching the failure path before
any production overfit has actually been observed.

## Proposed diff (for the next coder, post-production-run)

### `scripts/exp2/estimators/infonce.py`

Add `overfit: bool` to `InfoNCEResult`:

```python
@dataclass
class InfoNCEResult:
    estimate_nats: float
    train_median_last: float
    val_median_last: float
    train_val_gap: float
    train_history: list[float]
    val_history: list[float]
    stopped_step: int
    overfit: bool = False  # NEW
```

In `fit()`, replace the raise with a flag:

```python
gap = train_median - val_median
overfit = gap > float(self.hp["overfit_gap_threshold_nats"])
return InfoNCEResult(
    val_median, train_median, val_median, gap, train_hist, val_hist,
    best_step, overfit,
)
```

### `scripts/exp2/run_estimators.py`

Wrap the fit in retry-on-overfit-with-smaller-critic logic:

```python
def fit_with_overfit_retry(cfg, u, v, seed):
    """Spec §4c: on overfit, retry with smaller critic. If second try
    also overfits, return result with overfit=True; caller emits NaN.
    """
    full_hp = dict(cfg["infonce"])
    estimator = InfoNCEEstimator(full_hp, seed=seed)
    info = estimator.fit(u, v)
    if not info.overfit:
        return estimator, info, "full_critic"
    # Retry with smaller critic — same seed for determinism per spec §4c.
    small_hp = dict(full_hp)
    small_hp["critic_hidden_dim"] = 32  # was 64
    small_hp["critic_depth"] = 2  # already 2 in the default; here for clarity
    small_estimator = InfoNCEEstimator(small_hp, seed=seed)
    small_info = small_estimator.fit(u, v)
    return small_estimator, small_info, "small_critic_retry"
```

Then `run_pair` records `infonce_critic_size` ("full_critic" | "small_critic_retry"
| "ksg_only") and emits NaN for `infonce_estimate_nats` if the retry also
overfits.

### `scripts/exp2/effective_rank.py`

`mi_matrix_from_rows` must skip NaN rows or substitute KSG estimate. Two options:

a. NaN-pure: keep the cell NaN; spectrum/Schur paths must mask. Complicated.
b. Substitute: use ksg_estimate_nats for ksg_only rows. Simpler; matches spec
   F2 resolution "KSG-only result for this pair, with documented caveat."

I recommend (b) with a logged warning. Code:

```python
def mi_matrix_from_rows(rows, method="infonce"):
    m = np.zeros((14, 14), dtype=np.float64)
    for i, j in itertools.combinations(range(14), 2):
        row = rows[f"E{i + 1}_E{j + 1}"]
        value = float(row[f"{method}_estimate_nats"])
        if math.isnan(value) and method == "infonce":
            # Spec §F2: fall back to KSG-only for this pair.
            value = float(row["ksg_estimate_nats"])
            log.warning("Pair %s using KSG-only fallback (overfit_persists)", row["pair"])
        if value < 0 or not math.isfinite(value):
            raise Exp2Error(f"{row['pair']} invalid {method} MI: {value}")
        m[i, j] = m[j, i] = value
    return m
```

### `scripts/exp2/visualize.py`

`recommendation()` adds a "ksg_only" branch when `infonce_estimate_nats` is NaN.

## Risk assessment for in-session implementation

- **NaN in eigenvalues**: `np.linalg.eigvalsh` on a matrix with NaN propagates
  NaN through the entire spectrum; the resulting `n_eff_spectrum_entropy`
  becomes NaN. Without a production overfit observation, I cannot reliably
  test the (b) substitution path against real data.
- **Determinism guarantee**: Spec §4c says "smaller critic (32→32→32)" implying
  same seed. Need to confirm `set_global_seed(seed)` invoked twice with same
  seed produces same critic init (it does via `manual_seed`); but the random
  generator state inside `fit()` (e.g. `torch.randint` for batch sampling) is
  re-seeded inside the second `fit()` invocation, so the retry SHOULD be
  deterministic.
- **No regression test possible**: Overfit only manifests on highly redundant
  pairs (e.g. E1/E10 where critic memorizes joint). Synthetic Gaussian sweeps
  don't typically trigger overfit because the closed-form MI is reachable
  with high val accuracy.

## Recommendation

Defer R6 until either:

a. The first production run is attempted and we observe an actual overfit halt
   (then R6 is implemented with empirical evidence).
b. A focused coder session (not a simplifier session) where the NaN
   propagation surface can be traced fully, with new tests for each downstream
   consumer.

Status remains: code raises on overfit. This is louder than spec wants but
preserves correctness of every other downstream computation. agent-01's
HIGH-severity journal entry stays open as a known defect with
documented remediation.

## What I DID change in this session

Nothing. This is a pure deferral with documented diff, per the brief's
"document the proposed diff in a discovery file and DEFER" pathway.
