---
namespace: decisions
agent_id: agent-document-architect
created: 2026-05-09T01:00:00Z
updated: 2026-05-09T01:00:00Z
status: active
severity: high
tags: directive, mejepa, learning-formula, utml-v1, jepa-edition, four-term, representational-non-collapse, self-healing, self-learning
supersedes: 2026-05-09--utl-v2-mejepa-learning-formula.md
---

# UTML v1.0 — JEPA Edition: ME-JEPA learning formula `L_step = ΔP × ΔK × ΔΩ × ΔΞ`

## Operator directive (2026-05-09, updated)

> "look at the `/home/cabdru/contextgraph/docs2/UTL_v2_embodied.md` you can use this to
> formulate a learning formula for the ME JEPA system. ... read this in full its been
> updated and then continue and update everything with this in mind as well, finish
> everything you were doing and update anything you already did that needs changed
> because of this."

The operator rewrote `docs2/UTL_v2_embodied.md` from the human-physiology v2.0 to the
**machine-native UTML v1.0 — JEPA Edition** on 2026-05-09. The 3-term embodied formula
`L = ΔS × ΔC × ΔE` is **superseded** by the 4-term ML-native formula:

```
L_step = ΔP × ΔK × ΔΩ × ΔΞ        all factors clamped to [0, 1]
```

Multiplicative gating preserved: any factor at zero zeroes the whole product. The new
fourth term **ΔΞ (Representational Non-Collapse)** is the term biology cannot have, and is
the central architectural innovation of ME-JEPA. Adopted as the canonical learning formula
for ME-JEPA effective 2026-05-09.

## The four terms (canonical, from `docs2/UTL_v2_embodied.md` UTML v1.0)

| Symbol | Name | What it measures | Range |
|---|---|---|---|
| **L_step** | Per-update learning efficiency | Genuine generalization gain per gradient step | [0, 1] |
| **ΔP** | Predictive Error (latent-space, JEPA-native) | How wrong, normalized by gradient SNR | [0, 1] |
| **ΔK** | Knowledge Coherence | Alignment with prior gradients, Fisher protection, calibration | [0, 1] |
| **ΔΩ** | Optimization-State Health (ML "embodiment") | Effective plasticity × landscape × stability | [0, 1] |
| **ΔΞ** | Representational Non-Collapse (NEW) | No target collapse, no predictor redundancy, no constellation violations | [0, 1] |

### ΔP — Predictive Error (latent-space, JEPA-native)

```
ΔP_t = clamp_[0,1](
         max(ΔP_real(t), γ · ΔP_imagined(t))
         + 0.5 · exploration_bonus(t)
       ) / SNR(t)
       γ ≈ 0.7 (Dreamer-class imagination discount)
       SNR(t) = gradient signal-to-noise ratio at step t
```

ΔP is the cosine/MSE distance between `Pψ(s_t, a_t)` (predictor output) and
`f_target(s_{t+1})` (target representation). For ME-JEPA, `f_target` is the **frozen
externally-defined instrument panel** (E_Oracle, E_Witness, E_StaticAnalysis, etc.), zero
trainable parameters. For other JEPAs (I-JEPA, V-JEPA, DINO, BYOL), `f_target` is an EMA
target encoder.

The `max(...)` prevents double-counting when imagined and realized predictions share
information. `γ · ΔP_imagined` is the ML analog of UTL v2.0's mental-rehearsal term —
imagined rollouts (Dreamer-style imagination, MuZero search, ME-JEPA's `dynamicjepa plan`
candidate-action enumeration) contribute to ΔP at a fidelity discount.

### ΔK — Knowledge Coherence

```
ΔK_t = clamp_[0,1](
         0.5 · cos_align(g_t, ḡ_prior)
         + 0.3 · (1 − fisher_violation(θ_t, F_prior))
         + 0.2 · (1 − ECE(model_t))
       )
```

- `cos_align(g_t, ḡ_prior)` — gradient alignment with the EMA gradient of consolidated
  tasks. Negative alignment = catastrophic forgetting in progress. NTK kernel alignment
  (Atanasov 2021) is the deeper theoretical version.
- `fisher_violation(θ_t, F_prior)` — Fisher-weighted L2 distance from `θ_prior` (EWC,
  Kirkpatrick 2017). Quantifies destruction of consolidated knowledge.
- `ECE(model_t)` — Expected Calibration Error on a held-out set. The ML analog of UTL
  v2.0's HRV-coherence term: a global "internal-state coherence" measurement. Rising ECE
  during training is the leading indicator of overfitting / RLHF reward hacking / latent
  rearrangement.

### ΔΩ — Optimization-State Health

```
ΔΩ_t = (effective_plasticity_t)^0.4
       · (landscape_health_t)^0.4
       · (stability_floor_t)^0.2
```

Geometric mean — any subcomponent at 0 forces ΔΩ = 0.

- **effective_plasticity** — residual capacity to change. Measured by ReLU dormancy
  fraction, effective rank of gradient covariance, GSNR distribution, weight-matrix rank
  drift. Sutton 2024 (Dohare et al.) shows gradient descent alone loses plasticity;
  **continual backprop** (selective dormant-unit re-init) maintains it.
- **landscape_health** = `sigmoid(−λ_max(H_t)) · inverted_U(‖g_t‖)` — flatter minima
  generalize better (SAM, Foret 2021), penalizing both vanishing and exploding gradients.
- **stability_floor** — hard gate. 1.0 if no NaN/Inf, validation not rising, gradient
  norm in soft band; 0.0 if NaN/Inf detected OR RLHF reward proxy decoupled from gold.

### ΔΞ — Representational Non-Collapse (the NEW term)

```
ΔΞ_t = (1 − target_collapse_t)^0.5
       · (1 − predictor_redundancy_t)^0.3
       · (1 − constellation_violation_rate_t)^0.2
```

**For ME-JEPA: `target_collapse_t = 0` always, by construction.** This is the structural
guarantee — frozen externally-defined instruments cannot drift, cannot collapse, cannot
be gradient-pushed toward the trivial fixed point because there is no gradient flowing
through them. The first factor is locked at 1.0.

For other JEPAs (I-JEPA, V-JEPA, DINO, BYOL), `target_collapse_t` is computed from EMA
target encoder output variance — when variance falls below a threshold, the term saturates
toward 1 and ΔΞ collapses toward 0.

- **predictor_redundancy** — fraction of normalized pairwise MI between latent slots that
  exceeds threshold. Caught by `dynamicjepa audit-pairwise-mi` (Royse 2026b §3.8).
- **constellation_violation_rate** — fraction of recent guard decisions rejecting predicted
  panel as outside the constellation. Constellation = set of acceptable predicted panels
  for (state, action), fitted from deterministic walks. Optimizer cannot Goodhart against
  it because the constellation is not in the loss.

### ME-JEPA composite training loss (operational form)

```
L_total = α·L_panel + β·L_pairwise + γ·L_action
        + δ·L_constellation + ε·L_anti-pole
        + ζ·L_uncertainty + η·L_invariant
```

Dominant ΔP-bearing terms: `L_panel` (per-slot MSE/cosine to frozen target) + `L_action`
(action-conditioning regularizer). The other five terms are ΔK and ΔΞ regularizers.

### Consolidation integral M(t)

```
M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
       k_replay = 1 normal training, 2–4 during replay/EMA/SWA windows
```

The product `L_step · ΔΞ` is critical: a step with collapsed latent (ΔΞ ≈ 0) cannot
consolidate anything even if the gradient update happens.

**The audit substrate IS physical M.** ContextGraph's append-only ledger (25 RocksDB
CFs `dj_*` + `dj_audit_log` + BlackBoxFix SQLite at `LEDGER_SCHEMA_VERSION = 14`) is the
queryable, byte-equal-readback-verified consolidation tensor. M is not a mathematical
abstraction — it is a substrate.

### State-Transfer T (deployment-ready performance)

```
T(train, deploy) = exp(−d(P_train, P_deploy) / λ)
performance_deploy = M(t) · T(train, deploy)
```

`d` = divergence between train and deployment distributions (Wasserstein, MMD, KL, or
task-specific). The ML name for managing T is **domain randomization** (Tobin 2017),
**data augmentation**, or **adversarial training**. ME-JEPA's mutation corpus (8
categories × 300 SWE-bench Lite tasks) is exactly this: a distribution of states close to
the deployment distribution of human-introduced bugs and AI-generated patches.

For cross-language coverage, T is what makes ME-JEPA work on Python AND Rust AND C AND
C++ AND Java AND C# AND JS AND TS AND Ruby. Train across realistic deployment-state
distribution from day one.

### Future-Self Prior (target networks, EMA teachers, reward models)

```
predicted_t = (1 − β_t) · f_history(s_t) + β_t · f_target(s_t)
β_t ∈ [0.05, 0.20]
β_{t+1} = clamp(β_t · (1 − η · sign(|err_target| − threshold)), 0.05, 0.20)
```

The bound prevents wishful thinking. β too large = Goodhart catastrophe (Gao 2022,
Skalse 2024). Default β = 0.10 informed by Polyak-averaging (DDPG/SAC), BYOL/DINO momentum
(0.996→1.0 → β ≈ 0.10–0.20 over a typical pretraining phase).

### Confidence calibration (gated by BOTH ΔΩ AND ΔΞ)

```
calibrated_confidence = raw_conf · convergence_rate · strategy_agreement
                       · evidence_factor · ΔΩ_mean · ΔΞ_mean
clamped to [0.10, 0.95]
```

A confident prediction in a collapsed latent (ΔΞ ≈ 0) or a chronically unstable optimizer
(ΔΩ ≈ 0) is not trustworthy.

### Reward (for the data-factory optimizer)

```
Reward = 0.6 · calibrated_confidence + 0.4 · mean(L_step · ΔΞ)
```

Note `L_step · ΔΞ` (not `L_step` alone): a step that does not preserve representational
integrity does not contribute to reward, even if its loss looked great.

## Learning states (UTML v1.0 §IX)

| Pattern | State | Action |
|---|---|---|
| L_step < 0.1 for N steps, ΔΩ > 0.4, ΔΞ > 0.4 | **Stuck** (path) | Curriculum-prune |
| L_step < 0.1 for N steps, ΔΩ < 0.4 | **Optimizer-Dysregulated** | Regulate ΔΩ first (continual backprop, LR reset, gradient clipping) |
| L_step < 0.1 for N steps, ΔΞ < 0.4 | **Latent-Collapsing** | Regulate ΔΞ first (VICReg coefficient, instrument re-init, constellation re-fit) |
| L_step < 0.3 | Confused | Improve coverage |
| 0.3 ≤ L_step < 0.7 | Optimal | Continue |
| L_step ≥ 0.7 | Boring/Mastery | Advance curriculum (harder mutations, broader languages) |
| mean(L_step) > 0.3 but ΔM/mean(L_step) < 0.2 | **Dissipating** | Replay-buffer enrichment, EWC re-anchoring, k_replay window |

**Coaching order:** regulate ΔΩ AND ΔΞ before adjusting ΔP or ΔK. Trying to fix prediction
error by curriculum tweaks while the latent has collapsed is wasted compute.

## Pruning rule

```
prune curriculum item if (L_step < 0.1 for N steps)
                       AND (ΔΩ > 0.4 throughout)
                       AND (ΔΞ > 0.4 throughout)
```

If ΔΩ low → optimizer-state problem, fix that first. If ΔΞ low → latent collapse, fix
that first. Three signals must agree before pruning.

## ME-JEPA's structural advantage (the central UTML v1.0 claim)

Among joint-embedding architectures, **ME-JEPA is uniquely structurally collapse-immune
on the target side** because its targets come from frozen externally-defined instruments
rather than from a learnable EMA encoder. This fixes `ΔΞ.target_collapse = 0` by
construction.

Every other JEPA-class system (I-JEPA, V-JEPA, DINO, BYOL, VICReg, Barlow Twins, SimCLR,
MoCo) defends ΔΞ with regularizers or contrastive negatives — all loss-shape devices that
can be defeated by sufficiently expressive student/teacher pairs, miscalibrated
hyperparameters, or degenerate data distributions. ME-JEPA defends target-side ΔΞ
**structurally** with zero parameters and zero regularization.

## Self-healing protocol (UTML v1.0 §IX, mapped to ME-JEPA)

```
1. ASSESS OPTIMIZER STATE: measure ΔΩ and ΔΞ first
   ↓
2. REGULATE if ΔΩ < 0.4 or ΔΞ < 0.4 (state intervention before content):
   - ΔΩ low → continual-backprop dormant-unit re-init / LR warmup reset / gradient clip
   - ΔΞ low → VICReg coefficient bump / pairwise-MI audit / constellation re-fit
   ↓
3. OBSERVE: ΔP, ΔK
   ↓
4. COMPUTE: L_step = ΔP × ΔK × ΔΩ × ΔΞ
   ↓
5. CLASSIFY: which learning state (table above)?
   ↓
6. INTERVENE: adjust curriculum OR optimizer state OR latent regularization
   ↓
7. EVALUATE: did L_step improve? Did M (validation) hold over the eval window?
   ↓ (return to step 1)
```

## Robust error logging (operator directive 2026-05-09)

> "if something doesn't work it should error out and have robust error logging so that if
> something does fail then we know exactly what failed and how to fix it."

Every fallible ME-JEPA op returns a taxonomy-defined error with `.code()`. Failure-mode
taxonomy under UTML v1.0:

- `MEJEPA_DELTA_P_NAN` — predictor output non-finite
- `MEJEPA_DELTA_K_FISHER_VIOLATION` — fisher_violation > budget; reject update or surface drift
- `MEJEPA_DELTA_K_ECE_RISING` — calibration degrading; possible overfitting or RLHF Goodhart
- `MEJEPA_DELTA_OMEGA_PLASTICITY_LOSS` — ReLU dormancy >threshold; trigger continual backprop
- `MEJEPA_DELTA_OMEGA_LANDSCAPE_SHARP` — `λ_max(H_t)` >threshold; tighten SAM / reduce LR
- `MEJEPA_DELTA_OMEGA_STABILITY_BREACH` — gradient explosion / NaN / RLHF proxy decoupled
- `MEJEPA_DELTA_XI_TARGET_COLLAPSE` — should NEVER fire on ME-JEPA (structural); fires on legacy JEPAs
- `MEJEPA_DELTA_XI_PREDICTOR_REDUNDANCY` — pairwise MI block detected; predictor degenerating
- `MEJEPA_DELTA_XI_CONSTELLATION_VIOLATION` — guard rejected predicted panel
- `MEJEPA_OBSERVE_DRIFT_DETECTED` — drift severity ≥ hard
- `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` — witness-chain broken
- `MEJEPA_INFER_OOD_REFUSE` — OOD score too high; refuse prediction
- `MEJEPA_INFER_DIM_MISMATCH` — panel dimensionality wrong
- `MEJEPA_INFER_INSTRUMENT_MISSING` — required instrument unavailable for this language
- `MEJEPA_RETRAIN_HOLDOUT_REGRESSION` — Mode B/C did not beat Mode A on holdout
- `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES` — < N samples in conformal calibration window

No silent fallback paths. No `unwrap_or(0.5)` on a confidence interval. No mock data.

## Connection to existing infrastructure

- **Frozen externally-defined instruments (E_Oracle, E_Witness, ...)** — already in
  `crates/context-graph-mejepa-instruments/`. ΔΞ.target_collapse = 0 by construction.
- **13-embedder system in `crates/context-graph-embeddings/`** — substrate for ΔK
  measurements (cos_align uses gradient-direction alignment across spaces;
  embedder-coherence is part of the latent-cluster-coherence term).
- **Witness chain** — `crates/context-graph-witness/` records every L_step deterministically.
- **25 RocksDB CFs `dj_*` + ledger** — the physical M(t).
- **Reality loop** — continuous source of `(panel, oracle_verdict)` pairs feeding
  `mejepa.observe()` via §3.8 shift-log subscriber.
- **Mutation corpus** — `crates/context-graph-mejepa-corpus/` — bootstrap ΔP distribution.

## Falsification criteria (UTML v1.0 §XIII)

This formula is **falsifiable**. Any of these would invalidate UTML v1.0:

1. **ΔΞ proxies (predictor variance, pairwise MI redundancy, constellation violation rate)
   uncorrelated with downstream generalization** when ΔP, ΔK, ΔΩ are held constant.
2. **A self-supervised system with a learnable target encoder achieving structural
   collapse-immunity without a regularizer.** No such system exists as of 2026.
3. **Flat minima do NOT generalize better in JEPA-class systems** (contradicts SAM).
4. **High GSNR uncorrelated with generalization** (contradicts Liu 2020).
5. **Target-encoder collapse is benign at scale** (contradicts BYOL ablation literature).
6. **An audit-substrate-grounded ME-JEPA run reports high L_step + high M but fails on
   freshly generated SWE-bench Lite holdout.** Existing 5-seed career-taxonomy results
   are consistent with the prediction; sustained failure on Phase 2 holdouts forces
   revision.

If any falsification holds, this decision file gets `status: superseded` and a corrected
version describes the new formula.

## Related

- `docs2/UTL_v2_embodied.md` — UTML v1.0 — JEPA Edition (the canonical source theory).
- `2026-05-09--utl-v2-mejepa-learning-formula.md` — the now-superseded 3-term v2.0 mapping.
- `2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md` — companion directive
  (single reality loop, Claude Code is SWE-bench actor).
- `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` — public-facing operational doc
  (must be updated to UTML v1.0).
- `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` — pivot doc (Section B must be
  updated to UTML v1.0).
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan (§5 self-healing
  uses UTML v1.0 coaching cycle).
- Royse (2026a) — *ContextGraph Reality Loop* white paper (the audit substrate).
- Royse (2026b) — *Dynamic / ME-JEPA* paper (the structural collapse-immunity claim).

## Tags

`directive` `mejepa` `learning-formula` `utml-v1` `jepa-edition` `four-term`
`representational-non-collapse` `target-collapse-immunity` `frozen-instruments`
`vicreg` `ewc` `continual-backprop` `sam` `goodhart` `audit-substrate`
`physical-m` `self-healing` `self-learning` `cross-language` `falsifiable`
