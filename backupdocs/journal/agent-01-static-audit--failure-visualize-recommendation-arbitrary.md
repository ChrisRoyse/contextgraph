---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:19:00Z
status: active
severity: low
tags: exp2, visualize, recommendation, arbitrary-thresholds
---

# Defect: visualize.recommendation uses arbitrary 0.5/1.0 nat thresholds with no spec basis

## Where
`scripts/exp2/visualize.py:119-125`

```python
def recommendation(row: dict) -> str:
    mi = float(row["infonce_estimate_nats"])
    if mi >= 1.0:
        return "high redundancy"
    if mi >= 0.5:
        return "consider swap"
    return "keep both"
```

## Spec
`docs/exp2/08_output_deliverables.md` §3 per_pair_table:
> | recommendation | str | "keep both", "consider swap", "high redundancy" |

The labels match. But the spec doesn't define the THRESHOLD that produces each label.

## Issue
- Using raw MI thresholds (0.5 nats, 1.0 nats) is arbitrary.
- The methodology defines `redundancy_jk = MI_jk / min(H_j, H_k)` (range [0, 1]) — that's the natural normalized scale.
- A 0.5 nats threshold means "MI exceeds 0.5 nats" — for a pair with H_j = 5 nats, that's 10% redundancy; for a pair with H_j = 0.6 nats, that's 83% redundancy. Different judgments.

## Recommended fix
Use `redundancy` (per row 8 of the §13 schema) for thresholding:
```python
red = float(row.get("redundancy", row["infonce_estimate_nats"] / max(min(H_j, H_k), 1e-6)))
if red >= 0.6: return "high redundancy"
if red >= 0.3: return "consider swap"
return "keep both"
```

(The current `pair_results/E*_E*.json` schema written by `run_estimators.run_pair` does NOT include a `"redundancy"` field; it would need to be added there.)

## Severity
LOW. The labels are advisory (consumed by `embedder_swap_recommendation.md`), not a measurement. But spec compliance is iffy.

## Cross-check
None.
