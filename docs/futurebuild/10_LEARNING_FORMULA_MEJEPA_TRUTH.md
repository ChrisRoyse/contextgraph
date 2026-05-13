# 10 — Learning Formula for ME-JEPA-as-Truth-System

> **Purpose.** Extend the UTML v1.0 (JEPA Edition) four-factor learning formula `L_step = ΔP × ΔK × ΔΩ × ΔΞ` to the multi-head truth-system architecture. The base formula is unchanged (it's the right formula) but each factor now applies per-head, and a head-aggregation rule is needed for the global L_step that drives the BatchSampler. This document specifies that aggregation.
>
> **Source theory:** `docs2/TheoryOfMachineLearning.md` (UTML v1.0 — JEPA Edition) + `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` (the existing single-head application). This doc adds the multi-head extension that the futurebuild plan requires.

**Status snapshot (2026-05-13):** The Phase B/Phase D UTML carrier is implemented and physically verified. `crates/context-graph-mejepa-train/src/learning_signal/per_head.rs` computes per-head L-step inputs and `delta_xi_global_min`; `TrainingCertificate` persists `per_head_l_step`, `delta_xi_global_min`, and explicit ΔP/ΔK/ΔΩ/ΔΞ component carriers; sampling weight includes the agent-surprise severity factor. FSV source of truth: `/mnt/d/contextgraph/fsv/contextgraph-utml-fsv/utml-fsv-summary.json` (`passes=true`, SHA-256 `a7d1041db76adca835649305645e4e5df8a5157673cbde40fc2ac5e18d112657`) reads back `CF_MEJEPA_TRAIN_CERTS` after reopen and covers non-finite component, zero gate, conflicting state-transfer, empty input, and negative-age edge cases. Phase E now ships policy automation for per-head Fisher, per-head dormancy, lambda ramp, and per-head/global dissipation with FSV at `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`. Remaining formula work is per-cell state-transfer reporting cadence and Phase G model-quality convergence.

---

## 1. The base formula is unchanged

```
L_step = ΔP × ΔK × ΔΩ × ΔΞ      all factors clamped to [0, 1]
```

Per `11_MEJEPA_LEARNING_FORMULA.md §1`, four ML-native factors gate every gradient update. Multiplication enforces that any one at zero zeros the whole product. The structural property — `ΔΞ.target_collapse = 0 by construction` because instruments are frozen — remains the central architectural innovation.

What changes for the futurebuild truth-system: the **predictor now has 10 heads** (panel, oracle, failure-mode, edge-case, tech-debt, perf, security, accuracy, cost, reasoning per `03_ARCHITECTURE_BLUEPRINT.md §3`), so we need to compute `L_step` per head AND aggregate to a global L_step that drives sampling-weight reweighting.

---

## 2. Per-head ΔP

Each head has its own predictive-error term. The shared trunk's gradient is the weighted sum of the per-head gradients:

```
ΔP_h(t) = clamp_[0,1](
  loss_h(predicted_h, target_h) / SNR_h(t)
)

ΔP_global(t) = sum_h(λ_h · ΔP_h(t)) / sum_h(λ_h)
```

where `λ_h` is the per-head loss coefficient (defaults: `λ_panel = 1.0`, `λ_oracle = 0.5`, `λ_failure_mode = 0.3`, `λ_edge_case = 0.2`, `λ_tech_debt = 0.1`, `λ_perf = 0.15`, `λ_security = 0.2`, `λ_accuracy = 0.1`, `λ_cost = 0.05`, `λ_reasoning = 0.1`). The defaults give the panel + oracle heads dominance early; auxiliary heads ramp as their head-specific holdout metric improves (per `03_ARCHITECTURE_BLUEPRINT.md §3`).

**Per-chunk aggregation within a head** (per `11 §2`): each head's loss is computed over the panel chunks, with `mean / max / top-k mean / weighted-by-entity-importance` aggregation. Default is `mean`; constellation-violating chunks use `max` so the loss focuses on the worst chunk.

---

## 3. Per-head ΔK

Knowledge coherence per head:

```
ΔK_h(t) = clamp_[0,1](
  0.5 · cos_align(g_h(t), ḡ_h_prior) +
  0.3 · (1 − fisher_violation_h(θ_h, F_h_prior)) +
  0.2 · (1 − ECE_h(model_h))
)

ΔK_global(t) = sum_h(λ_h · ΔK_h(t)) / sum_h(λ_h)
```

Per-head Fisher matrices `F_h_prior` are maintained separately so a head's update can be regularized against its own consolidated knowledge without being constrained by another head's. Cross-head knowledge transfer happens via the shared trunk; per-head Fisher applies only to head-specific projections.

**Operational note**: when an agent records a `Surprise` event for a specific concern (e.g., "ME-JEPA's failure-mode prediction was wrong"), only that head's `ΔK` is bumped; the panel head's coherence is unchanged. This prevents one head's miscalibration from contaminating others.

---

## 4. Per-head ΔΩ (mostly shared)

Optimization-state health is largely a property of the optimizer, not the head — the same Adam state, the same gradient norm band, the same NaN check. So `ΔΩ` is computed *globally* and shared across heads:

```
ΔΩ(t) = (effective_plasticity(t))^0.4 · (landscape_health(t))^0.4 · (stability_floor(t))^0.2
```

with one head-specific override: a head whose loss is producing exploded gradients (e.g., the security head with a poorly-tuned focal loss) can fail-close ΔΩ for that head only. This is implemented as a per-head clip: if `grad_norm_h > 10 × running_mean_grad_norm_h`, multiply that head's `ΔΩ` by 0.4 (clip-active regime per `11 §4.3`).

---

## 5. Per-head ΔΞ — the multi-head collapse defense

This is the most consequential extension. Each head can independently collapse:

```
ΔΞ_h(t) = (1 − target_collapse_h(t))^0.5
        · (1 − predictor_redundancy_h(t))^0.3
        · (1 − constellation_violation_rate_h(t))^0.2
```

For ME-JEPA: `target_collapse_h = 0` for every head, by construction (frozen instruments). The first factor is 1.0 always for every head. **Predictor redundancy and constellation violation are per-head measures**:

- `predictor_redundancy_h(t)` = average normalized pairwise MI within head h's output dimensions; computed weekly via `mejepa_audit_pairwise_mi`. A head whose output is collapsing to a low-rank subspace (e.g., the failure-mode head ignoring all but 3 classes) shows high MI between adjacent logit dimensions.
- `constellation_violation_rate_h(t)` = fraction of recent guard decisions where this head's predicted output landed outside the constellation for its concern class. Only meaningful for heads with constellations (panel head and head-specific Q3/Q4 centroids).

`ΔΞ_global(t) = min_h(ΔΞ_h(t))` — the *minimum*, not the mean. Because collapse in any one head is a structural failure that should hard-gate global learning. This is the multi-head extension of UTML's "any factor at zero zeros the product."

**Operational consequence**: when any head collapses, the BatchSampler's sampling-weight bumps zero out for the next epoch, halting the training loop until the heal scheduler intervenes (LoRA refresh on the collapsing head, or A/B/C retrain).

---

## 6. The global L_step

```
L_step(ex) = ΔP_global(ex) × ΔK_global(ex) × ΔΩ(ex) × ΔΞ_global(ex)
```

This is what the BatchSampler reads to compute sampling weights:

```
sampling_weight(ex) = base_weight
                    · L_step(ex)
                    · (1 + 5 · operator_override_flag)
                    · (1 + 2 · agent_surprise_severity_score)            // NEW
                    · decay^age_in_days
```

The `agent_surprise_severity_score` is new for the truth-system extension: when an agent records a `Surprise` event on a prediction whose contributing chunks include this example, the example's sampling weight is multiplied by `1 + 2 · severity` (severity score: Low=0.2, Medium=0.5, High=0.8, Catastrophic=1.0).

---

## 7. Per-head training certs

Per `11 §11`, every step's training cert records all four UTML factors. For multi-head, the cert is extended:

```jsonc
{
  "step": 1234,
  "loss_global": 0.0234,
  "loss_per_head": {
    "panel": 0.0210,
    "oracle": 0.043,
    "failure_mode": 0.067,
    "edge_case": 0.085,
    "tech_debt": 0.052,
    "perf": 0.041,
    "security": 0.038,
    "accuracy": 0.029,
    "cost": 0.023,
    "reasoning": 0.061
  },
  "delta_p_global": 0.42,
  "delta_p_per_head": { ... },
  "delta_k_global": 0.71,
  "delta_k_per_head": { ... },
  "delta_omega": 0.83,
  "delta_omega_components": { "effective_plasticity": 0.85, "landscape_health": 0.90, "stability_floor": 1.0 },
  "delta_xi_global": 0.997,
  "delta_xi_per_head": {
    "panel": 1.00,                         // target_collapse = 0; redundancy 0; constellation OK
    "oracle": 0.999,
    "failure_mode": 0.997,                 // some predictor redundancy creeping in
    ...
  },
  "L_step_global": 0.247,
  "sampling_weight": 0.247,
  "agent_surprise_severity_score_for_this_ex": 0.0,
  "lambda_h": { "panel": 1.0, "oracle": 0.5, ... },
  "predictor_redundancy_pairwise_mi": 0.07,
  "predictor_redundancy_pairwise_mi_source": "rolling_window_2026_05",
  "grad_norm_per_head": { ... },
  "drift_coverage": 0.91,
  "merkle_root": "..."
}
```

Persisted to `CF_MEJEPA_TRAIN_CERTS` keyed by step. Hash-chained via `parent_witness_hash → self_hash` per the existing cert chain protocol.

---

## 8. Loss aggregation in the predictor

```
L_total = α · L_panel + β · L_pairwise + γ · L_action
        + δ · L_constellation + ε · L_anti-pole + ζ · L_uncertainty + η · L_invariant
        + λ_oracle · L_oracle
        + λ_failure_mode · L_failure_mode
        + λ_edge_case · L_edge_case
        + λ_tech_debt · L_tech_debt
        + λ_perf · L_perf
        + λ_security · L_security
        + λ_accuracy · L_accuracy
        + λ_cost · L_cost
        + λ_reasoning · L_reasoning
```

The first row is the trunk's self-supervised JEPA loss (per `13 §2.2`) — unchanged. The second-through-tenth rows are the new auxiliary head losses. Defaults:

| Coefficient | Default | Ramping |
|---|---|---|
| α (panel) | 1.0 | fixed |
| β (pairwise) | 0.3 | fixed |
| γ (action) | 0.4 | fixed |
| δ (constellation) | 0.2 | fixed |
| ε (anti-pole / VICReg) | 0.5 | fixed |
| ζ (uncertainty) | 0.1 | fixed |
| η (invariant) | 0.1 | fixed |
| λ_oracle | 0.5 | fixed once shipped |
| λ_failure_mode | 0.05 → 0.3 | ramp as head-specific holdout F1 > 0.6 |
| λ_edge_case | 0.05 → 0.2 | ramp as head-specific recall on uncovered branches > 0.5 |
| λ_tech_debt | 0.05 → 0.1 | ramp as PR comments alignment > 0.5 |
| λ_perf | 0.05 → 0.15 | ramp as benchmark MAE < 10% |
| λ_security | 0.05 → 0.2 | ramp as static-analyzer agreement > 0.7 |
| λ_accuracy | 0.05 → 0.1 | ramp as ML-test metric MAE < 5% |
| λ_cost | 0.02 → 0.05 | ramp as CI-time MAE < 20% |
| λ_reasoning | 0.05 → 0.1 | ramp as reasoning-class agreement > 0.6 |

Ramping is automatic via the heal scheduler: when a head's holdout metric crosses the gate, the scheduler bumps that head's λ up by 0.05 per week until reaching the steady-state value. This avoids destabilizing the trunk early when auxiliary heads are noisy.

---

## 9. Per-head Fisher matrices for EWC++

`heal::ewc::FisherSnapshot` is shipped (per Phase 5). Extended for multi-head:

```rust
pub struct FisherSnapshot {
    pub trunk_fisher: Tensor,                      // shared across heads
    pub per_head_fisher: HashMap<HeadId, Tensor>,  // head-specific projections
    pub witness_hash: WitnessHash,
    pub ts: SystemTime,
    pub source_examples_window: (StepId, StepId),
}
```

When a head is fast-tracked (LoRA refresh), only its head-specific Fisher is consulted for the EWC++ regularization term; the trunk Fisher protects the shared body from drift across all heads.

---

## 10. Continual backprop per head

Per `11 §4.1` + REQ-UTML-07 (shipped), continual backprop re-inits dormant units. Multi-head extension: detect dormancy per head's projection layer separately:

```rust
pub fn detect_dormant_units_per_head(storage: &Storage) -> HashMap<HeadId, DormantUnitReport> {
    let mut out = HashMap::new();
    for head in heads::ALL {
        let report = continual_backprop::detect_dormant_units(storage, head)?;
        out.insert(head, report);
    }
    out
}
```

Re-init of one head's dormant units does not perturb other heads (they share the trunk but have independent projections). The scheduler runs the per-head detector at the same 1800s tick and re-inits per-head as needed.

---

## 11. State-transfer T per Python mutation category

Per `11 §8` + REQ-UTML-08 (shipped), state-transfer T is the train→deploy distribution gap:

```
T_global(train, deploy) = exp(−d(P_train, P_deploy) / λ)
performance_deploy = M(t) · T_global(train, deploy)
```

Multi-head extension for the active Phase G gate: compute T per Python mutation-category cell:

```
T_cell(c, python) = exp(−d(P_train_cell(c, python), P_deploy_cell(c, python)) / λ_cell)
```

Phase G's Python mutation-category stratified evaluation already computes this implicitly by reporting per-cell correlations. The futurebuild extension makes T explicit per cell so the active-learning queue can prioritize Python cells with low T (the deploy distribution drifted from training). Non-Python cells are future generalization, not the current ship gate.

---

## 12. The multi-head version of UTML's "Dissipating" failure mode

Per `11 §7.1`:

```
Dissipating_global ≡ ( mean(L_step_global) over last N updates > 0.3 )
                    AND ( ΔK_global or ΔΞ_global has been silently degrading )
                    AND ( ΔM / mean(L_step_global) < 0.2 )
```

Multi-head extension: dissipation can manifest in a single head while the global metric looks fine. So we also detect:

```
Dissipating_per_head_h ≡ ( mean(L_step_h) over last N updates > 0.3 )
                        AND ( ΔK_h or ΔΞ_h silently degrading )
                        AND ( head_h_holdout_metric_delta / mean(L_step_h) < 0.2 )
```

If a head dissipates, the heal scheduler triggers a LoRA refresh for that head specifically, not a global retrain. This catches one-head failures before they contaminate the trunk.

---

## 13. The five operator questions ↔ per-head L_step

Mapping the operator's Q1–Q5 (per `01_VISION_TRUTH_SYSTEM.md §3`) to the heads:

| Q | Question | Answering head(s) | Primary L_step contribution |
|---|---|---|---|
| Q1 | EXISTS? | claim graph extractor (deterministic, no head) | n/a — rule-based |
| Q2 | WORKS? | oracle head | dominant L_step component |
| Q3 | WHY (failure)? | failure-mode head | high λ; high agent-feedback signal |
| Q4 | WHAT ELSE? | edge-case + tech-debt + perf + security + accuracy + cost heads | low individual λ but many heads ⇒ aggregated meaningful share |
| Q5 | REALITY IMPACT? | reality-impact reconciler (deterministic, no head) | n/a — rule-based |

Q1 and Q5 are deterministic — no head, no learning needed. Q2, Q3, Q4 are head-driven and contribute to L_step.

---

## 14. The agent-feedback channel's role in the formula

Per `08_LIVE_PREDICTION_AGENT_API.md §2.1`, agents call `mejepa_record_agent_feedback` to write `SurpriseEvent`s. Each event has a `severity` ∈ {Low, Medium, High, Catastrophic} and a `feedback_kind` ∈ {Confirmed, Surprise, Omission, Calibration}.

Effect on L_step / sampling weight:

| Kind | severity_score | sampling_weight bump | Heal trigger |
|---|---|---|---|
| `Confirmed` | n/a | 1.1× (small reinforcement) | none |
| `Surprise` | Low=0.2, Med=0.5, High=0.8, Cat=1.0 | `1 + 2·severity_score` | if Cat: fast-track A/B/C |
| `Omission` | Low=0.3, Med=0.6, High=0.9, Cat=1.0 | `1.5 + severity_score` | if Cat: fast-track A/B/C + alert operator |
| `Calibration` | n/a | 1.2× | recalibrate the involved head's conformal predictor |

Persisted to `CF_MEJEPA_AGENT_FEEDBACK` and consumed by the BatchSampler on every batch composition.

---

## 15. The full updated formula in one block

```
For each batch step t, for each head h:
  ΔP_h(t)        = clamp_[0,1]( loss_h(predicted_h, target_h) / SNR_h(t) )
  ΔK_h(t)        = clamp_[0,1]( 0.5·cos_align_h + 0.3·(1−fisher_violation_h) + 0.2·(1−ECE_h) )
  ΔΞ_h(t)        = (1 − target_collapse_h)^0.5 · (1 − predictor_redundancy_h)^0.3 · (1 − constellation_violation_rate_h)^0.2
                   target_collapse_h ≡ 0 by construction in ME-JEPA

Globally:
  ΔP_global(t)   = Σ_h(λ_h · ΔP_h(t)) / Σ_h(λ_h)
  ΔK_global(t)   = Σ_h(λ_h · ΔK_h(t)) / Σ_h(λ_h)
  ΔΩ(t)          = (effective_plasticity(t))^0.4 · (landscape_health(t))^0.4 · (stability_floor(t))^0.2
                   per-head clip when grad_norm_h > 10·running_mean_grad_norm_h
  ΔΞ_global(t)   = min_h(ΔΞ_h(t))                                    // any-head-collapse hard-gates global
  L_step(ex)     = ΔP_global · ΔK_global · ΔΩ · ΔΞ_global

Sampling weight:
  sampling_weight(ex) = base_weight
                      · L_step(ex)
                      · (1 + 5·operator_override_flag)
                      · (1 + 2·agent_surprise_severity_score)
                      · decay^age_in_days

Loss:
  L_total(t)     = α·L_panel + β·L_pairwise + γ·L_action + δ·L_constellation + ε·L_anti-pole + ζ·L_uncertainty + η·L_invariant
                 + Σ_h(λ_h · L_h)

Consolidation integral:
  M(t)           = ∫₀ᵗ L_step(τ) · ΔΞ_global(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ

State-transfer:
  T_cell(c, l)   = exp(−d(P_train_cell(c, l), P_deploy_cell(c, l)) / λ_cell)
  performance_deploy_cell = M(t) · T_cell(c, l)

Reward:
  Reward(t)      = 0.6 · calibrated_confidence(t) + 0.4 · mean(L_step · ΔΞ_global)
  calibrated_confidence = raw_conf · conv_rate · strategy_agreement · evidence_factor · ΔΩ_mean · ΔΞ_global_mean
                          clamped to [0.10, 0.95]
```

---

## 16. What this formula extension commits us to

1. **Per-head ΔP, ΔK, ΔΞ tracking** in `learning_signal/` — complete/code_complete in `learning_signal/per_head.rs`.
2. **Per-head cert columns** in `CF_MEJEPA_TRAIN_CERTS` — Phase A-D complete for `per_head_l_step`, `delta_xi_global_min`, and component-carrier audit fields; Phase E adds per-head Fisher, dormancy, and lambda policy evidence. Richer per-head loss/lambda trend maps remain Phase F/G telemetry.
3. **`ΔΞ_global = min_h(ΔΞ_h)`** in the learning signal — complete for the persisted carrier path; BatchSampler skip/requeue automation remains Phase F/G model-quality hardening.
4. **`agent_surprise_severity_score` factor** in `sampling_weight` — complete.
5. **Per-head Fisher matrices** in Phase E policy ledger — complete/code+FSV.
6. **Per-head dormancy detection** in Phase E scheduler policy — complete/code+FSV.
7. **Per-head loss coefficient ramping** in the heal scheduler — complete/code+FSV.
8. **Dissipation detection per head** in the heal scheduler — complete/code+FSV.
9. **Per-cell state-transfer T** in eval reports — open.

All listed in [`13_TASK_INDEX.md`](13_TASK_INDEX.md). Implementation assumes the existing `learning_signal/`, `heal/ewc`, `heal/promote`, `eval/` modules are extended in-place — no new crate.
