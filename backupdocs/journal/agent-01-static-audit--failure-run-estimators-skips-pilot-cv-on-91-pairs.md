---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:16:00Z
status: active
severity: medium
tags: exp2, run-estimators, t-015, pilot, cross-validation
---

# Defect: run_estimators.py has no T-015 pilot phase; cross_validation_report only fires when len(rows)==91

## Where
`scripts/exp2/run_estimators.py:140-147`

## Spec
`docs/exp2/06_atomic_tasks.md`:
- T-015 Pilot Pass On 4 Pairs: `(E1, E1), (E1, random), (E1, E1+noise), (E1, E10)`. **Operator reviews; signs off on methodology before T-016.**
- T-020 Compute Cross-Validation Metrics: must run after both T-016 (InfoNCE) and T-017 (KSG) complete; failure path: "If rate < 80%, methodology revisits before T-021."

## Observed
```python
rows = []
for i, j in pairs:
    ...
    rows.append(run_pair(cfg, arrays, i, j, out))
if len(rows) == 91:
    cross_validation_report(cfg, rows, out)
else:
    write_json(out / "pilot_results.json", {"pairs": rows, "total": len(rows)})
```

Issues:
1. **No pilot definition**: T-015 calls for SPECIFIC pilot pairs `(E1,E1), (E1,random), (E1,E1+noise), (E1,E10)`. The `--limit-pairs N` flag in run_estimators just slices the first N from `itertools.combinations`. Self-pairs (E1,E1) aren't in `itertools.combinations(range(14), 2)`. Random and noisy variants don't exist as embedder slots in `EMBEDDER_KEYS`.
2. **Pilot config in exp2.yml is dead**: `run.pilot_pairs: ["E1_E1", "E1_random", "E1_E1_noisy", "E1_E10"]` (line 61). Nothing in the codebase reads `cfg["run"]["pilot_pairs"]`. Confirmed via static read.
3. **Cross-validation runs only on full 91 pairs**: if a partial run produces 90 pairs (e.g., one fails mid-run), no agreement-rate computation. Spec calls for it after T-016 + T-017.
4. **No mechanism for "rerun with smaller critic"** mentioned in T-016 DoD ("Pairs with train-val gap > 0.1 nats are flagged + re-trained with smaller critic.") — `run_pair` runs once and writes the result; no flag, no retry.

## Why it matters
- T-015 sign-off is a Constitution C16 gate. Skipping the pilot means the operator never sees the methodology in microcosm before paying ~5 hours of GPU compute.
- C5 (no hyperparam tuning post-hoc) is preserved by the no-retry path, but the spec explicitly authorized one-shot re-training with a smaller critic on overfit pairs. Implementation gives no such mechanism.

## Recommended fix
Add a `--pilot` mode that:
1. Constructs synthetic E1_E1 (self), E1_random (real E1 vs Gaussian noise), E1_E1_noisy (E1 + small noise), and E1_E10 (real pair).
2. Runs the estimator pipeline.
3. Writes `pilot_results.json` with the 4 expected outcomes.
4. Stops; operator sign-off required to proceed.

For overfit retry: detect `train_val_gap > 0.1` in `run_pair`, log to audit chain, optionally rerun with `critic_hidden_dim=32` and re-emit row.

## Severity
MEDIUM. The pilot is the operator's only checkpoint before the production InfoNCE compute. Skipping it isn't a code bug but a workflow / spec-compliance gap.
