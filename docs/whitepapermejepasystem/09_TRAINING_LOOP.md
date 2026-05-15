# 09 — The Training Loop

This document describes how the predictor is actually trained: the JEPA learning formula, the UTML (Unified Training Markup Language) carriers, the conformal calibration step, the prioritized replay buffer, the EWC catastrophic-forgetting protection, the continual backprop plasticity layer, and the online retrain cycle. Reference: `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` + `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`.

## The JEPA learning formula

Given:
- `panel_t1` = the frozen-instrument panel at pre-edit state (5,120-d)
- `panel_t2` = the frozen-instrument panel at post-edit state (5,120-d)
- `embedder_input` = the per-chunk per-embedder vectors at pre-edit state + the claim text
- `f_predictor(...)` = the trainable predictor
- `f_target(panel_t2)` = identity (because the panel is the target representation; no projection)

The core objective:

```
L_jepa = || f_predictor(embedder_input, panel_t1) - f_target(panel_t2) ||²
```

Minimize the prediction error in the same 5,120-d space.

By construction, `∂f_target/∂θ_predictor = 0`, so the gradient of `L_jepa` flows only through the predictor path. Target collapse is impossible.

## What gets trained

The predictor head MLP. Specifically:
- Per-embedder projection layers (12 of them)
- Cross-embedder attention head
- Per-question heads (Q1, Q2, Q3, Q4 sub-heads, Q5)
- Per-cell weight profiles (when cell routing is enabled)

**Not trained:**
- The 12 embedders themselves (frozen pretrained weights)
- The 15 frozen instruments
- The Docker oracle
- The AST parser

## The full loss

`L_jepa` is the core. The full training loss adds:

| Term | Role | Source |
|---|---|---|
| `L_jepa` | Predict-target match | Above |
| `L_conformal_residual` | Calibrated probability matches empirical residual | Per-cell calibration set |
| `L_contrastive` | Pull similar chunks together, push dissimilar apart | Contrastive pairs mined from cells |
| `L_consistency` | Same prediction under augmented input | Data-driven augmentation (DDA) |
| `L_OOD` | High score for in-distribution, low for OOD | Cell membership |
| `L_blind_spot` | Penalize predictions that disagree with the primary embedder more than threshold | Per-cell blind-spot z-scores |

Total: `L_total = L_jepa + α·L_conformal + β·L_contrastive + γ·L_consistency + δ·L_OOD + ε·L_blind_spot`

α, β, γ, δ, ε are per-cell calibrated via the UTML (Unified Training Markup Language) lambda-ramp policy. They start small and ramp up as the predictor's per-cell residual stabilizes.

## UTML — the lambda-ramp policy

Per `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`, the **UTML formula carriers** are the per-cell parameter sets that govern:
- Loss term weights (α, β, γ, δ, ε)
- Per-head Fisher matrix entries (for EWC)
- Per-unit dormancy scores (for CBP)
- Per-cell dissipation thresholds (when to declare a cell saturated)

The lambda-ramp prevents auxiliary losses from dominating early training. Start with `L_jepa` only. Once `L_jepa` plateaus per cell, ramp `L_conformal` to 1.0 over N steps. Then `L_contrastive`. Then the rest. Each ramp is per-cell, parameterized by the UTML carrier.

Status: TASK-UTML-005..008 pending implementation in Phase G.

## EWC — Elastic Weight Consolidation

Online retraining catastrophically forgets old cells as new data arrives. EWC fixes this by tracking per-parameter Fisher information and penalizing updates to high-Fisher parameters.

Per `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`:

```
L_ewc = sum_i F_i · (θ_i - θ_i*)²
```

where `F_i` is the Fisher information for parameter i (estimated from gradient magnitudes during training on the previous cell), and `θ_i*` is its consolidated value.

Implementation: TASK-HEAL-005 (per-head Fisher) + TASK-UTML-005 (Fisher carrier per cell). Pending Phase G.

## Continual Backprop (CBP) plasticity

Long-running online training causes units in the MLP to "die" (low activation, low gradient). CBP tracks per-unit utility and reinitializes low-utility units periodically. Keeps the network plastic across months of online updates.

Implementation: TASK-UTML-006. Pending.

## Prioritized replay buffer

Per `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md`, the replay buffer is the long-term memory that prevents the latest training stream from dominating.

Entries:
- Every `(panel_t1, claim, panel_t2)` triple from corpus training
- Every (live prediction → operator override) pair, weighted 6×
- Every prediction-vs-actual mismatch (`Verdict::Pass` predicted but `Oracle::Fail` measured)
- Every adversarial corpus case
- Every cell that the dissipation detector flags as drifting

Priorities:
| Source | Priority weight |
|---|---|
| Corpus training (cell-stratified) | 1× |
| Adversarial corpus | 4× |
| Operator override (Refuted) | 6× |
| Drifting cell rotation | 3× |
| OOD prediction mistakes | 5× |
| Out-of-corpus live mistakes | 6× |

The training step samples from the buffer with replacement, weighted by priority × recency.

## Conformal calibration

After each training step (or every K steps), the conformal calibrator re-fits the per-cell residual distribution. Per `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md`:

1. Hold out 10% of each cell's members as the calibration set.
2. Run the predictor on the calibration set; collect residuals `r_i = |y_pred - y_true|`.
3. Per cell, fit a non-conformity score function (typically a kernel density estimate or empirical CDF).
4. At inference, for a new chunk, compute its prediction error against the cell's centroid; convert to a calibrated probability using the residual distribution.
5. Emit the conformal interval `[p_low, p_high]` with coverage 1-α (typical α=0.05 for 95% coverage).

When the calibration interval is too wide, the verdict becomes `Verdict::Abstain`. The system never emits a confident verdict it can't statistically defend.

## Per-cell stratified evaluation

The convergence metric is **per-cell, not aggregate**. Per `docs/futurebuild/27_PHASE_G_ROADMAP.md`:

```python
for cell in cells:
    correlation = pearson(predictor.predict(cell.members), oracle.outcome(cell.members))
    if correlation < 0.95:
        cell.failed_gate = True
```

Ship gate: **all cells pass simultaneously over 4 consecutive rolling windows**. The system isn't allowed to ship by averaging high `known_good` performance over low `subtle_flip` performance.

## Online retrain cycle

Once the corpus-trained predictor is deployed, the daemon's job is:

1. **Subscribe** to the shift log via `context-graph-mejepa-shift-subscriber`.
2. **Materialize panels** for new edit events via the frozen instruments.
3. **Predict** on the new panel (live prediction).
4. **Ingest operator feedback** when it arrives (`mejepa_record_agent_feedback`).
5. **Sample** a mini-batch from the replay buffer + the live feedback at the priority weights above.
6. **Train** one step with the full loss `L_total`.
7. **Recalibrate conformal** every K steps.
8. **Check drift** every N steps; emergency-evict cells whose residual distribution shifted significantly.
9. **Promote** the new weights when per-cell evaluation is non-degrading.
10. **Operator approval gate**: significant weight changes require `mejepa_promote_approval` from the operator before going live.

Promotion policy:
- Auto-promote if every cell's correlation ≥ previous_correlation - ε
- Hold for approval if any cell drops by more than ε
- Roll back if the promoted weights regress

## Training run cost

For the seed corpus (399,777 chunks × 12 embedders) once the forward-cache is built:

| Phase | Compute | Wall time |
|---|---|---|
| Forward cache (running now) | 12 embedders × 4.9 M chunks on RTX 5090 par=6 | ~21 hours |
| Cell centroid materialization | CPU sweep over packed records | ~30 min |
| Pairwise MI computation | CPU × 66 pairs × 2,400 cells | ~2 hours |
| Predictor head training | RTX 5090 + corpus replay | ~12-24 hours |
| Conformal calibration | CPU per cell | ~30 min |
| Per-cell evaluation | RTX 5090 + Docker oracle subsample | ~2 hours |
| Rolling window stability assertion | 4 windows × evaluation | ~8 hours |

Total Phase G ETA from forward-cache completion to ship-gate: **~3-4 days of mostly-automated work**, gated by per-cell eval results.

## Adversarial training

Phase G includes a forced-failure step: after corpus training, run the predictor on the 30-case adversarial corpus. If the predictor's `pass_at_high_confidence` > 0 (i.e., any adversarial Fail case got marked Pass with high confidence), the predictor is rejected and the corresponding cells are flagged for retraining with the adversarial cases mixed in at 4× weight.

This is the "don't let the predictor be gamed by patches that look easy" check.

## Continual learning loop

After ship, the predictor runs online. The replay buffer grows; the operator override count grows; the corpus stays fixed but is augmented by the live feedback. Each retrain cycle should *measurably* improve the predictor on the live distribution while not regressing on the corpus.

The criteria for a successful retrain:
1. Live prediction-vs-actual correlation improves by ≥ 0.001 per cycle.
2. Corpus per-cell correlation drops by no more than ε.
3. Conformal coverage stays within nominal bounds.
4. OOD detector AUC stays above threshold.

If criteria 1-4 hold, the retrain is promoted. Otherwise it's held for operator approval.

## What's shipped now

| Component | Status |
|---|---|
| Shift subscriber | ✅ Shipped (`context-graph-mejepa-shift-subscriber`) |
| Forward cache pipeline | ✅ Running on aiwonder |
| Panel materialization | 🟡 E_Oracle shipped; 14 other instruments pending |
| Predictor head MLP | 🟡 Skeleton in `context-graph-mejepa`; full training Phase G |
| Cross-embedder attention | 🟡 Designed, implementation Phase G |
| Per-cell weight profiles | 🟡 Routing exists; per-cell training Phase G |
| Conformal calibrator | 🟡 Designed; integration Phase G |
| Prioritized replay buffer | 🟡 Designed; implementation TASK-HEAL-001..007 |
| EWC | 🟡 Designed; implementation TASK-HEAL-005 |
| CBP | 🟡 Designed; implementation TASK-UTML-006 |
| Drift detector | 🟡 Designed; implementation TASK-HEAL-002..004 |
| Operator approval gate | 🟡 Designed; implementation TASK-HEAL-006 |

Everything is grounded in code that exists or in tasks with concrete acceptance criteria. The Phase G roadmap is the operational source of truth.
