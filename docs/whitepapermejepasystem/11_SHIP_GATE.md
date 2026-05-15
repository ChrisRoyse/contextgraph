# 11 — The Ship Gate

The single metric that closes the truth system. Nothing else gates ME-JEPA going live.

## The metric

```
prediction_oracle_correlation ≥ 0.95
   stable over 4 consecutive rolling windows
   stratified per (mutation_category × language) cell
```

Plain English: for **every cell** in the corpus, the predictor's predicted-Pass probability must correlate with the Docker oracle's actual outcome at Pearson ρ ≥ 0.95, **and** that correlation must hold for **four windows in a row** (so the system isn't allowed to ship on a lucky window).

## Why these specific numbers

### Why 0.95

At ρ = 0.95, the predictor explains 90% of the variance in oracle outcomes (`ρ² = 0.9025`). For a binary outcome, this corresponds to roughly **5% miscategorization rate** if the predictor commits to a verdict.

5% is the threshold at which human spot-checking becomes statistically marginal: if you're going to check 5% of patches anyway, you're not gaining linear value from checking 100%. Conversely, at 0.9 (~80% explained variance), you're still missing 1 in 5, which is too many to defend "you don't need to review this."

The number is a **judgment call**, not derived from first principles. It's the operator's chosen line where they're willing to stop reviewing.

### Why 4 windows

A predictor can be lucky on one window. A predictor can drift over time. The 4-window requirement enforces:

1. **No lucky-window ship.** A new training session's first eval might hit 0.95 by chance. The next 3 windows test whether it stays there.
2. **No drift-blind ship.** If the predictor's correlation is decaying, 4 windows of stability catch the decay.
3. **No selection-bias ship.** The 4 windows are rolled forward in time across the corpus's train→holdout boundary. Each window uses different chunks of holdout data.

The window size is the smaller of `300/N_windows` cells or "1 week of live retrain activity," whichever is appropriate for the phase.

### Why per-cell stratification

The most dangerous failure of an aggregate metric: high overall correlation that hides terrible per-cell performance. Imagine:

- `(known_good, Python, django)` cell: correlation 0.99 ← agent claims accepted
- `(subtle_flip, Python, django)` cell: correlation 0.45 ← every adversarial mutation is misclassified
- Aggregate over all django cells: ~0.85 (still well above some imagined 0.8 threshold)

The aggregate hides that the system is useless on the exact failure mode it's supposed to catch. Per-cell stratification forces every cell — adversarial included — to pull its own weight.

## What "rolling window" means concretely

Per the corpus split:
- 76.3% of repos → train
- 13.3% of repos → calibration
- 10.3% of repos → holdout

For evaluation, the holdout is subdivided into N windows by time-of-evaluation. Each window has its own subset of holdout cells.

The 4-window stability check:
1. At each retrain promotion, re-evaluate against the current window.
2. Slide the window forward (add new chunks, remove oldest).
3. Re-evaluate per cell.
4. If 4 consecutive windows all hit ≥ 0.95 per cell → ship gate flips green.
5. If any window dips below 0.95 on any cell → reset the counter.

## The full evaluation pipeline

Implementation: `context-graph-mejepa::eval` module, invoked by `mejepa eval run` CLI subcommand.

Per `docs/futurebuild/12_IMPLEMENTATION_PHASES.md`:

```
1. Build evaluation graph
   - Load holdout cells
   - Load predictor checkpoint
   - Load conformal calibrator
   - Load Docker oracle bridge

2. For each cell in holdout:
   a. Sample N members (typically 30-100 per cell)
   b. Run predictor forward
   c. Run Docker oracle for ground truth
   d. Compute per-cell Pearson correlation
   e. Compute per-cell conformal coverage
   f. Compute per-cell OOD AUC

3. Aggregate
   - All cells correlation ≥ 0.95? gate_flag = True else False
   - All cells coverage in nominal bounds? coverage_ok
   - All cells OOD AUC ≥ threshold? ood_ok

4. Stability assertion
   - Check current eval against last 3 windows
   - If 4 consecutive green → ship_gate = STABLE_GREEN
   - Else → ship_gate = NOT_YET

5. Persist eval row to CF_MEJEPA_EVAL_HISTORY
```

## Auxiliary gates

Per `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md §7`, the ship gate is gated on additional auxiliary gates:

| Gate | Threshold | Source |
|---|---|---|
| Per-cell correlation | ≥ 0.95 per cell × 4 windows | Above |
| Conformal coverage | within nominal ±2σ | Per-cell calibration set |
| OOD AUC | ≥ 0.90 | OOD holdout set |
| Adversarial pass-at-high-confidence | == 0 (no false positives) | 30-case adversarial corpus |
| Adversarial correct-classification | ≥ 24/30 | 30-case adversarial corpus |
| ECE (Expected Calibration Error) | ≤ 0.02 | Per-window |

All gates must be green simultaneously over the 4-window stability window.

## Current state

Per the project's last measurement:

```
prediction_oracle_correlation = 0.866667 < 0.95
```

(Source: `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`)

This is **the only outstanding ship gate**. Phase A–D is 35/35 PRESENT (per the independent audit 2026-05-12). All 16 FSV evidence files exist. All 17 GitHub issues (#3–#19) including the umbrella gate #15 are closed. Phase E entry is unblocked.

The gap from 0.867 → 0.95 is **Phase G convergence work**: train the predictor head against the seed corpus (currently being produced), run the per-cell eval, recalibrate conformal, hit the four-window stability assertion.

## Why this isn't yet at 0.95

The 0.867 measurement was on an earlier predictor with **partial corpus coverage**. The forward-cache run currently grinding on aiwonder is producing the substrate for the *full* per-cell training that brings the correlation above 0.95.

Estimated post-corpus state:
- Per-cell training: pulls per-cell correlations up from average 0.87 to per-cell ≥ 0.95 on the easier cells (`known_good`, `compile_error`).
- Harder cells (`subtle_flip`, `over_engineer`) require adversarial training mix-in to clear 0.95.
- Calibration: conformal recalibration on the full holdout closes the residual gap.
- Stability: 4 rolling windows on a stable predictor should fire green within 4 retrain cycles.

ETA from forward-cache completion to ship-gate-green: **~3-5 days of mostly-automated training** (Phase G item-by-item per `docs/futurebuild/27_PHASE_G_ROADMAP.md`).

## What happens when the ship gate fires green

1. **`mejepa_ship_gate_status`** returns `STABLE_GREEN` with the 4-window history.
2. The predictor checkpoint is tagged as the production version.
3. The operator approves promotion via `mejepa_promote_approval`.
4. The MCP server begins serving production predictions to live agents.
5. The convergence claim ("human review of AI-produced Python code is statistically unjustified") is now defensible.
6. Online retrain continues, monitoring per-cell correlation continuously. If any cell drifts below 0.95, the ship gate auto-yellows and the operator is notified.

## What "ship" means

"Ship" does NOT mean "delete the senior engineer." It means:

1. **Default to trust** for predictions with `Verdict::Pass | confidence ≥ 0.95 | ood < 0.5`. The patch merges without human review.
2. **Spot-check** at a sampled rate (~5%) to verify the predictor stays calibrated.
3. **Review** when `Verdict::Abstain | OutOfDistribution | GuardRejected`. The predictor itself is telling you it's unsure; the operator handles those cases.
4. **Override** when the operator disagrees with the predictor's `Pass`. Use `mejepa_record_agent_feedback` to feedback.

Reviewer fatigue collapses because the *base rate* of review-needed patches drops from 100% to ~5-10%.

## After Python ships

Python is the first language. Once `prediction_oracle_correlation ≥ 0.95` holds stably for Python:

1. Generalize to the 10 other languages already chunked in the multi-lang fixture corpus.
2. Add embedders that better serve non-Python (BGE-M3 already does multilingual reasonably).
3. Expand the corpus to project-specific cells (per-customer, per-repo).
4. Add more failure-mode categories beyond the current 8 mutations.

But none of that happens before the Python gate is green. The convergence rule: **one language, one corpus, one gate, then expand**.

## The honest assessment

The 0.95 number is **achievable** given:
1. The frozen-instrument target panel (collapse-immune by construction).
2. The 12-embedder coverage (which provides enough signal density).
3. The 2,400-cell labeled corpus (which provides the supervision).
4. The Docker oracle (which provides the deterministic ground truth).
5. Active learning + replay (which keeps the predictor improving online).

It is **not guaranteed**. The remaining unknowns:
- Will the per-cell stratification hold up under the adversarial mutations?
- Will conformal calibration cover the OOD tail?
- Will the four-window stability stay green or oscillate?
- Will the adversarial corpus catch the patches we didn't think to generate?

These are empirical questions the Phase G evaluation will answer. The infrastructure is in place; the convergence is the work that remains.

## TL;DR

`prediction_oracle_correlation ≥ 0.95 × 4 consecutive windows × per (mutation_category × language)` is the single gate. Everything ME-JEPA does serves this. When it's green, the verifier is the system. Until then, the operator reviews.
