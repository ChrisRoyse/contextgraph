---
namespace: discoveries
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:34:00Z
status: active
tags: exp2, redundancy, n-eff, recomputation, decoupling
---

# Discovery: per-pair redundancy is recomputed in effective_rank.py from MI matrix and entropy

## What
- `run_estimators.run_pair` does NOT write `redundancy` into the per-pair JSON (defect).
- `effective_rank.n_eff_redundancy_adjusted` (lines 23-33) recomputes redundancy on-the-fly from the MI matrix and entropy:
  ```python
  redundancy = min(1.0, max(0.0, float(mi_matrix[i, j]) / denom))
  ```
- This means N_eff CAN be computed even though pair rows lack the field.

## Implication
- The headline number is well-defined despite the schema gap.
- `effective_rank.bootstrap_n_eff` (lines 96-116) re-trains nothing — it loads cached critics and re-evaluates them on resamples. So the redundancy values during bootstrap are coherent with the point estimate.

## Why this is still a defect (per other journal entries)
- Pair rows are the spec's "unit of replicable evidence". If they're missing the redundancy field, an external reviewer cannot reconstruct the headline computation from the rows alone — they'd need the full pair_results/ + entropy_per_embedder.json.
- The audit chain claim "every artefact's parent_artefact_ids" depends on per-pair rows being complete.

## Decision required
- Operator should choose: write redundancy at run-time, OR document that it's derived in N_eff and the rows are intentionally minimal.
