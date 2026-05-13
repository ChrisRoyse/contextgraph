# 21 — Cold Start and Bootstrap

> **Purpose.** What happens when ME-JEPA boots with no training data, no constellation, no calibrated conformal predictor. Specifies the seeded initial state, the first-100-prediction behavior, and the convergence path to "trustworthy."

---

## 1. The cold-start problem

On day 1 of a fresh deploy:
- `CF_MEJEPA_PANELS` is empty
- `CF_MEJEPA_TRAIN_CERTS` is empty
- The predictor has random weights (or seeded from a checkpoint)
- The constellation has empty cells (or shrunk-toward-prior placeholders)
- The conformal predictor has no calibration set
- The patch-similarity graph has no nodes

Calling `mejepa_verify(patch, context)` in this state must NOT return a dangerous false-confident `Pass`. It must return either `Verdict::OutOfDistribution` or `Verdict::Abstain` with `confidence_interval = [0.0, 1.0]` (maximum uncertainty).

---

## 2. Bootstrap modes

### 2.1 Mode A — fresh deploy (no prior state)

State:
- Predictor weights: seeded from a "baseline" checkpoint shipped under `models/mejepa-baseline-vN.safetensors` (small model, trained on the completed Phase C seed corpus during release prep).
- Constellation: bootstrapped from the completed active Phase C Python seed corpus (2,400 SWE-bench rows, with Bayesian-shrunk centroids per `04 §4`). The 5,120 non-Python fixture rows are archived future-generalization evidence and are not part of the current ship gate.
- Conformal predictor: bootstrapped on the same seed corpus's holdout split.
- Patch-similarity graph: bootstrapped from seed corpus.

Behavior:
- `mejepa_verify` returns predictions, but with widened conformal intervals (`coverage_target = 0.99` initially, narrowing to `0.90` after 1k production observations).
- Verdict liberally returns `OutOfDistribution` or `Abstain` until the rolling validation accuracy stabilizes.
- Active-learning queue prioritizes EVERY prediction in the first 100 (operator labels them gold-standard, weighting them 6×).

### 2.2 Mode B — restore from backup

State: restored from `restic` snapshot per `20_DISASTER_RECOVERY_AND_BACKUP.md §3`.

Behavior:
- Same as steady-state operation; conformal predictor and constellation are warm.
- Drift window resets to empty; first 1000 production observations populate the rolling window with widened thresholds.

### 2.3 Mode C — model promotion (from prior version)

State: previous version's weights become the new active pointer, but conformal calibration is fresh.

Behavior:
- Predictor weights are warm; calibration is recalibrated from the previous holdout split.
- No widening needed; standard production thresholds apply.

---

## 3. The seeded baseline checkpoint

**Status as of 2026-05-12:** the Phase C corpus prerequisite exists and is verified, but the release baseline checkpoint pipeline (`TASK-BOOT-002`) is still open. This section defines the release-prep artifact to build; it does not claim that `models/mejepa-baseline-vN.safetensors` already exists.

`models/mejepa-baseline-vN.safetensors` ships with the release. Built during release prep:

```
1. Read the completed Phase C corpus from the D-backed source of truth
2. Train the predictor for 10 epochs on the full seed corpus
3. Run holdout eval; assert correlation ≥ 0.65 (the baseline floor)
4. Compress to bf16 + safetensors
5. Pin SHA-256 digest in the release notes
6. Operator downloads + verifies digest before installing
```

Baseline floor of 0.65 is intentionally conservative — the live system will surpass it within the first few hundred operator-labeled examples.

If no baseline checkpoint is available (e.g., the operator wants to bootstrap entirely from production):
- Predictor weights: random init (Xavier)
- Verdict on every prediction in the first ~5000 observations: `Abstain`
- Operator must label aggressively until calibration set is large enough

---

## 4. Conformal calibration cold start

Per `09_SELF_HEALING_AUTONOMY.md §6`:

```
First 100 oracle outcomes: skip conformal interval (uniform prior); confidence_interval = [0.0, 1.0]
First 100-1000:              widened conformal at coverage_target = 0.99
> 1000:                      standard conformal at coverage_target = 0.90
```

Per-cell calibration follows the same pattern, gated by per-cell sample count.

---

## 5. Constellation cold start

Per `04 §4`:

```
Per-cell sample count < 50:  Bayesian-shrunk toward marginal centroid
50 ≤ N < 200:                shrinkage decreasing linearly
N ≥ 200:                     full per-cell centroid
```

If a cell has zero examples (e.g., a language × entity-type combination not in the seed corpus):
- Verdict for that cell: always `OutOfDistribution`
- Active-learning queue prioritizes operator labels for that cell

---

## 6. First-week behavior expectations

| Day | Expected behavior |
|---|---|
| 0 | Boot; baseline weights loaded; widened intervals; active-learning queue prioritizes everything |
| 1 | First 100 predictions; operator labels heavily; sampling-weight bumps applied |
| 2-3 | LoRA refresh fires (if drift detected against baseline); per-head metrics starting to climb |
| 4-7 | Conformal coverage stabilizing; constellation centroids refitting per-cell as N crosses thresholds |
| 7+ | Steady-state operation |

If by day 14, the Python mutation-category ship gate is not approaching 0.85 in any cell, escalate per `19 §10`.

---

## 7. Bootstrap CLI commands

```bash
# Manual bootstrap (called by daemon's startup code; also operator-callable)
context-graph-cli mejepa bootstrap [--baseline /path/to/mejepa-baseline-vN.safetensors] [--no-baseline]

# Verify bootstrap completed
mcp_call mejepa_bootstrap_status
# Returns: { stage: "warm" | "cold" | "bootstrapping",
#            n_predictions_made: ..., n_observations_consumed: ...,
#            calibration_state: ..., constellation_state: ... }

# If bootstrap failed mid-way (e.g., baseline checkpoint not found and --no-baseline not set):
context-graph-cli mejepa bootstrap --recover
```

---

## 8. What this commits us to

Tasks (in `tasks/bootstrap/` — small, ~3 tasks):

- `TASK-BOOT-001` — `mejepa_bootstrap_status` MCP tool + `bootstrap` CLI
- `TASK-BOOT-002` — baseline checkpoint build pipeline (release-prep job)
- `TASK-BOOT-003` — widened-interval cold-start path in conformal predictor

These are small but matter — without them, the daemon's first run is a confidence-disaster.
