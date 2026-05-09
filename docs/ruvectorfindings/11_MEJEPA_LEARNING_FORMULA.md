# 11 — ME-JEPA Learning Formula (UTML v1.0 Adaptation)

> **Operator directive 2026-05-09:** *"look at `/home/cabdru/contextgraph/docs2/UTL_v2_embodied.md` you can use this to formulate a learning formula for the ME JEPA system."*

> **Source theory:** `docs2/UTL_v2_embodied.md` — **Unified Theory of Machine Learning v1.0 (JEPA Edition)**, the ML-native rewrite of the earlier embodied/biological framework. UTML v1.0 keeps the multiplicative-gate philosophy but has **four** ML-native factors instead of three; the new fourth factor (ΔΞ — representational non-collapse) is the term v2.0's biological framework cannot motivate but every modern self-supervised system requires.

> **Companion docs:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan (panel + predictor + frozen target readback + VICReg + verify)
> - `10_MEJEPA_PIVOT_2026_05_09.md` — post-pivot architecture (single outer loop, no inner LLM)

---

## 0. The headline

**ME-JEPA-Code is structurally collapse-immune on the target side**, by construction, because its target panel is produced by **frozen externally-defined instruments** (E_AST, E_CFG, E_Oracle, E_Witness, ...) — not by a learnable EMA encoder. This makes `ΔΞ.target_collapse = 0 by construction` for the target panel. Learnable-target JEPA/BYOL-style systems defend ΔΞ on the target side with asymmetry or regularizers; ME-JEPA defends the target side with **zero trainable target parameters**.

This is *the* central architectural point of the system. The training-time optimizer only needs to defend ΔΞ on the **predictor** side (VICReg variance/covariance + pairwise mutual-information audit + constellation guard), which is a much smaller problem.

---

## 1. The formula

```
L_step = ΔP × ΔK × ΔΩ × ΔΞ              all factors clamped to [0, 1]
```

Per UTML v1.0 (Royse 2026 §I, §II) — multiplication enforces that any single factor at zero zeros the entire learning signal, regardless of how good the others look. Addition would let a great ΔΩ compensate for a collapsed ΔΞ, which is empirically false.

| Symbol | Name | Meaning for ME-JEPA-Code | UTML §ref |
|---|---|---|---|
| **L_step** | Per-update learning efficiency | Genuine generalization-gain per gradient step | I |
| **ΔP** | Predictive error (latent-space) | How wrong is the predictor's panel about the actual oracle-verdict panel? | III |
| **ΔK** | Knowledge coherence | Does this update integrate with prior tasks (alignment + Fisher + calibration)? | IV |
| **ΔΩ** | Optimizer-state health | Effective plasticity + loss landscape + stability floor | V |
| **ΔΞ** | Representational non-collapse | Target collapse + predictor redundancy + constellation violation (target collapse = 0 by construction in ME-JEPA) | VI |

---

## 2. ΔP — Predictive error in latent space

ME-JEPA predicts the *latent* of the post-execution panel from the pre-execution panel + patch context. This is JEPA's foundational innovation: predict in representation space, not output space (LeCun 2022; Assran et al. 2023; Bardes et al. 2024).

```
ΔP_t = clamp_[0,1](
  max( ΔP_real(t), γ · ΔP_imagined(t) )
  + 0.5 · exploration_bonus(t)
) / SNR(t)
```

For ME-JEPA-Code:
- **`ΔP_real(t) = 1 − cosine(g_φ(f_θ(panel_t01)), f_target(panel_t2))`** clamped to [0, 1]
- **`f_target(panel_t2)`** is **the frozen externally-defined instrument panel** — not an EMA target encoder. This is the structural advantage over I-JEPA/V-JEPA/etc.
- **`ΔP_imagined(t) = 1 − cosine(g_φ_imagined, f_target(panel_t2))`** is the prediction error from a counterfactual latent perturbation (UTML §III, mirroring Dreamer-style imagined rollouts in latent space; Hafner et al. 2023).
- **`γ ≈ 0.7`** is the imagination fidelity discount (Dreamer-class empirical range 0.6-0.8).
- **`SNR(t)`** is the gradient signal-to-noise ratio at step t (Liu et al. 2020). High noise → low SNR → ΔP shrinks because the gradient is not informative.
- **`exploration_bonus`** is the active-learning bump: examples chosen because the conformal interval was wide get +0.5.
- **`max(...)`** prevents double-counting when the system both imagines and realizes a transition.

**Implementation reference:** `crates/context-graph-mejepa/predictor.rs::predict()` returns the predicted latent; the existing VICReg-style loss in `loss.rs::vicreg_loss()` already computes the cosine error between predicted and target. New `learning_signal::compute_delta_p()` wraps the result with SNR normalization + exploration bonus + clamp.

**Per-chunk ΔP (per doc 14).** With AST-aware chunking wired (doc 14), each panel materialization is composed of N chunks (function / class / impl / module-decl level, ~30-50 chunks per typical SWE-bench Lite mutation entry). The per-step ΔP becomes a **per-chunk distribution** of cosine errors aggregated by the loss:

```
ΔP_t = aggregate_per_chunk(ΔP_t_chunk_i for i in 1..N_chunks(panel_t))
     where aggregate ∈ {mean, max, top-k mean, weighted-by-entity-importance}
```

The default aggregator is `mean` for stability; `max` is used when the constellation guard (doc 13 §3.3) flags a specific violating chunk so the loss focuses on the worst chunk. **This makes the ΔP signal load-bearing at function/class granularity** instead of being averaged across the whole patch, and it compounds with the ~280 chunk-level signals per mutation entry (per doc 14 §4.1) to give ME-JEPA dense per-chunk gradient signal.

**Empirical anchor.** Career-Taxonomy benchmark validation cosine 0.905 ± 0.044 → ΔP_validation ≈ 0.095, BCa 95% CI [0.861, 0.931] (Royse 2026b §8). For ME-JEPA-Code we expect similar ΔP regimes once Phase 3 training reaches the doc 09 §7 holdout-agreement gate (≥0.75 agreement → ΔP ≤ 0.25 on holdout).

---

## 3. ΔK — Knowledge coherence

```
ΔK_t = clamp_[0,1](
  0.5 · cos_align(g_t, ḡ_prior)
  + 0.3 · (1 − fisher_violation(θ_t, F_prior))
  + 0.2 · (1 − ECE(model_t))
)
```

### 3.1 Gradient alignment (`cos_align`, weight 0.5)

The instantaneous gradient `g_t` of the new batch should lie in a half-space compatible with the EMA-averaged gradient `ḡ_prior` of the consolidated training history. When `cos_align < 0`, the new gradient is actively destructive of prior knowledge — the geometric signature of catastrophic forgetting (Kirkpatrick et al. 2017). Neural Tangent Kernel theory gives a deeper version: as gradient descent proceeds, the leading eigenvectors of the empirical NTK align with the target function (Atanasov et al. 2021). High alignment = the network is using existing feature directions to express the new target = coherent. Low or anti-aligned NTK = the new task is forcing the network to break existing representations = destructive.

**Implementation:** `crates/context-graph-mejepa-train/learning_signal.rs` maintains an EMA over recent batch-gradient directions; per step, computes `cos_align(g_t, ḡ_prior)` and exposes via `ΔK.cos_align_t`.

### 3.2 Fisher-weighted protection (`fisher_violation`, weight 0.3)

Per UTML §IV.2 + Kirkpatrick et al. 2017 (EWC). Each parameter has an importance weight derived from the Fisher Information Matrix `F_prior` for the consolidated task distribution. The `fisher_violation` quantity is the Fisher-weighted L2 distance the current update creates from `θ_prior`. Small relative to a per-parameter budget = update is non-destructive of consolidated knowledge = coherent.

For ME-JEPA-Code, the Fisher matrix is computed against the heldout 30-task validation panel (post-Phase 3 training).

### 3.3 Calibration health (`1 − ECE`, weight 0.2)

A model whose Expected Calibration Error is rising during training is one whose update is fluent on the loss but incoherent on its reported confidence — a leading indicator of overfitting / Goodhart-style reward hacking (Gao et al. 2022) / latent space subtly rearranging in ways the predictor doesn't yet reflect.

ECE replaces UTL-v2's HRV-coherence term: it's the ML-native "internal-state coherence" measurement.

**Implementation:** Phase 4's conformal calibration produces ECE as a side-output. Reuse it for the ΔK term.

---

## 4. ΔΩ — Optimizer-state health

ML-native "embodiment". The optimizer + numerical substrate + plasticity machinery determine whether a gradient signal can actually become a parameter update without being damped, exploded, or frozen.

```
ΔΩ_t = (effective_plasticity_t)^0.4 · (landscape_health_t)^0.4 · (stability_floor_t)^0.2
```

Geometric weighting (per UTML §V) ensures any subcomponent at 0 forces ΔΩ = 0.

### 4.1 effective_plasticity ∈ [0, 1]

Sutton 2024's *Loss of plasticity in deep continual learning* (Dohare et al., Nature) showed gradient descent alone is insufficient for continual learning — without injected variability, the network gradually loses the ability to learn new tasks even when the tasks are easy. UTML treats plasticity as a *trainable, recoverable* property.

| Network state | Signature | `effective_plasticity` |
|---|---|---|
| Fresh, well-initialized | High variance pre-activations, high-rank weight matrices, broad GSNR | 1.0 |
| Mid-training, healthy | Stable but updating, modest dormancy | 0.8 |
| Late-training, mild plateau | Some dormant ReLU units, slight low-rank drift, GSNR narrowing | 0.6 |
| Plasticity-loss regime | Many dormant units, weight rank collapsing, gradient effective rank shrinking | 0.2 |
| Frozen / dead | All gradients near zero, no parameter motion despite high loss | 0.0 |

Operational measurements: (a) ReLU dormancy fraction, (b) effective rank of gradient covariance, (c) per-parameter GSNR distribution, (d) drift into low-effective-rank subspaces.

**Intervention** when plasticity collapses: **continual backprop** (Dohare et al. 2024) — selective, low-rate re-initialization of dormant units.

### 4.2 landscape_health ∈ [0, 1]

```
landscape_health_t = sigmoid(−λ_max(H_t)) · inverted_U(‖g_t‖)
```

- **`λ_max(H_t)`** is the largest Hessian eigenvalue — sharpness. Flatter minima generalize better (Foret et al. 2021 — SAM; Friendly-SAM Li et al. 2024). Penalize sharp minima.
- **`inverted_U(‖g_t‖)`** is the ML version of Yerkes-Dodson — penalize both vanishing gradients (no learning signal) and exploding gradients (instability).

### 4.3 stability_floor ∈ [0, 1]

The hard gate. ΔΩ must be zero when any update would be destructive of accumulated training:

```
stability_floor =
  1.0 if no NaN/Inf gradients, validation loss not rising over last 1000 steps, grad norm in soft band
  0.4 if grad norm in warning band (>10× running mean) — clip-active regime
  0.1 if validation loss has been rising for >10000 steps — likely overoptimization / Goodhart
  0.0 if NaN/Inf detected, or proxy reward decoupled from gold metric (the catastrophic-Goodhart case from Skalse et al. 2024)
```

**Empirical anchor.** Career-Taxonomy paper_small_1k on RTX 5090 reports ΔΩ ≈ 0.92 averaged over the run (effective_plasticity 0.95, landscape_health 0.90, stability_floor 1.0). Same band for the 480-param counter_world and 15,344-param career_taxonomy domains — ΔΩ being uniformly high across three orders of magnitude of parameter count means the *quality of the substrate*, not the model's absolute scale, determines optimizer-state health (UTML §V.3).

---

## 5. ΔΞ — Representational non-collapse (the ML-native term)

This is the term with no biological analogue and the **central architectural innovation of ME-JEPA**.

### 5.1 The collapse pathology

In every joint-embedding self-supervised system, there's a trivial fixed point of the loss: map every input to the same constant vector. Teacher and student agree perfectly, prediction error is zero, metrics look great, and **nothing has been learned**. Structural failure mode of the objective itself.

Existing partial defenses (all on the *predictor* side):
- Stop-gradient (BYOL — Grill et al. 2020; SimSiam — Chen & He 2021)
- momentum/target-encoder asymmetry (BYOL, DINO, I-JEPA, V-JEPA)
- VICReg variance + invariance + covariance (Bardes et al. 2022)
- Barlow Twins cross-correlation toward identity (Zbontar et al. 2021)
- Contrastive negatives (SimCLR, MoCo) — obviates collapse but reintroduces negative-mining complexity

**Every one** is a defense against ΔΞ falling toward zero. **None** is structurally guaranteed; all are loss-shape devices defeatable by sufficiently expressive student/teacher pairs, miscalibrated hyperparameters, or degenerate data distributions.

### 5.2 ME-JEPA's structural solution

ME-JEPA solves the **target-side** collapse problem **structurally**:

> **The target representation `f_target(panel_t2)` is not produced by a learnable network at all.** It is the output of a panel of **frozen externally-defined instruments** — E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff, E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning. Each is a deterministic function from a typed event to a structured feature output (vector, scalar, one-hot, pairwise feature). **Instruments have zero trainable parameters. They cannot drift. They cannot collapse. They cannot be gradient-pushed toward any fixed point — there is no gradient flowing through them, ever.**

This is the architectural property no other JEPA in the literature has, to our knowledge:

```
target_collapse_t = 0    by construction in ME-JEPA, for all t
                         ⇒ (1 − target_collapse)^0.5 = 1.0 always
```

### 5.3 Predictor-side defenses (still required)

Target-side immunity is necessary but not sufficient. The trainable predictor `g_φ` can still degenerate — predict a constant, ignore the patch context, learn a low-rank shortcut. ME-JEPA defends predictor-side ΔΞ with four independent diagnostics (UTML §6.3):

1. **VICReg-style regularization** (`δ·L_constellation + ε·L_anti-pole` in the composite loss) — variance + invariance + covariance terms on the predictor's output features.
2. **Pairwise mutual-information audit** — periodically computes the pairwise MI matrix between panel slots and persists `pairwise_mi_matrix.csv`. Slots whose MI with another slot exceeds a configurable ceiling are flagged as redundant — carrying no independent signal. A latent that is structurally collapsing in part of its dimensionality shows up as a block of high-MI off-diagonal entries.
3. **Constellation + Gτ guard** — at inference time, the predicted next-state panel is scored against a *constellation* — the set of acceptable predicted panels for a given (state, action) pair, fitted at training time. Below threshold τ the candidate is rejected and a typed error is persisted. A predictor whose output has degenerated will fail the constellation check even when the panel-loss looks fine, because the constellation centroid encodes what an *acceptable* prediction looks like at training-time — a specification the optimizer cannot Goodhart against.
4. **Negative-action ablation falsifier** — the cleanest empirical falsifier of collapse. Periodically run validation with action conditioning *removed*. If the predictor were degenerately predicting the next-state panel without using the action, the score would not drop. Career-Taxonomy reports validation cosine drops 0.905 → 0.790 (paired sign-flip permutation p < 0.001) when action is ablated — the empirical signature that the predictor is genuinely action-conditioned, not collapsed.

### 5.4 The ΔΞ formula

```
ΔΞ_t = (1 − target_collapse_t)^0.5
       · (1 − predictor_redundancy_t)^0.3
       · (1 − constellation_violation_rate_t)^0.2
```

For ME-JEPA: `target_collapse_t = 0` always — first factor locked at 1.0.

`predictor_redundancy_t` = 1 minus the average normalized pairwise MI across panel slots (computed periodically; cached between updates).

`constellation_violation_rate_t` = fraction of recent guard decisions that rejected the predicted panel as outside the constellation (only meaningful for ME-JEPA-class systems with an explicit constellation; defaults to 0 otherwise).

### 5.5 The model-collapse class (Shumailov 2024)

Distinct from representational collapse: **model collapse from recursive synthetic data** (Shumailov et al. 2024 Nature). When a generative model trains on data sampled from a previous version of itself, distribution tails shrink and the model converges to a degenerate distribution.

ΔΞ should **also down-weight when training data is increasingly model-generated rather than reality-grounded**. ME-JEPA's design naturally protects against this: the bundles it trains on are **audited execution traces with cryptographic provenance** — most bytes are deterministic functions of a public source (real Python files, real Docker oracle outputs, real witness chains) rather than model outputs. ME-JEPA-Code's training corpus has the lowest recursion-replacement collapse hazard of any JEPA-class system.

---

## 6. Per-example sampling weight

```
sampling_weight(ex) = base_weight
                    · L_step(ex)
                    · (1 + 5 · operator_override_flag)         // gold up-weight
                    · decay^age_in_days                          // recency-weighted
```

Where `base_weight = 1.0`, `decay = 0.995` (half-life ~140 days), and `L_step(ex) ∈ [0, 1]` is the four-factor product from §1.

Examples with `sampling_weight < 0.05` drop from the current epoch. Examples with `sampling_weight > 1.0` (gold-standard) are guaranteed inclusion.

This integrates UTML's L_step into the doc 09 §3.7.2 sampling reweighting mechanism. The previous `(1 + 2·residual_error_z_score)` term from doc 09 is **replaced** by `L_step(ex)` since L_step already captures hard-example mining via ΔP — and additionally enforces ΔΞ-safety + ΔΩ-safety + ΔK-coherence, which the old residual-z-score did not.

---

## 7. Consolidation integral M(t)

```
M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
```

The product **`L_step(τ) · ΔΞ(τ)` is critical**: a step with ΔΞ ≈ 0 (collapsed latent) cannot consolidate anything, even if the gradient update happens. This is the ML version of UTL-v2's "dissipation" failure mode (UTML §VII.1).

| Component | UTML default | Map to ME-JEPA |
|---|---|---|
| `k_replay(τ)` ≥ 1 | 1.0 normal training; 2-4 during replay-buffer / EMA / SWA / distillation phases | Periodic full retrain (every 1,000 attempts in Phase 5) bumps k_replay to ~3-4 |
| `τ_decay(τ)` | short for un-replayed weights (hours-of-training); long for consolidated parameters (months-years) | 7 days for examples not yet bound into the EWC++ Fisher snapshot; 365 days for high-Fisher consolidated examples |

### 7.1 The "dissipation" failure mode

```
Dissipating_ML ≡ ( mean(L_step) over last N updates > 0.3 )
                AND
                ( ΔK or ΔΞ has been silently degrading; validation generalization gap widening )
                AND
                ( ΔM over equivalent eval window / mean(L_step) < 0.2 )
```

In words: high in-update L_step, but cumulative consolidation gain is less than 20% of what L_step predicted. Training loss falls; validation does not (or worse, validation falls but downstream task transfer doesn't). Common in: RLHF reward overoptimization, self-supervised pretraining on near-duplicate data, continual learning runs in the loss-of-plasticity regime, and synthetic-data training loops in the Shumailov regime.

**Intervention is not more compute. It is state regulation:** replay-buffer enrichment, EWC re-anchoring on protected tasks, learning-rate annealing or LR-warmup reset, continual-backprop dormant-unit re-initialization, or fresh real-data injection (anti-Shumailov).

### 7.2 The ContextGraph audit substrate as physical M

ME-JEPA writes every persisted artifact — prompt, model response, candidate patch, per-attempt evidence, audit row — into an append-only ledger byte-equal-readback verified before influencing the next decision (Royse 2026a §7). The 25 RocksDB column families plus the BlackBoxFix SQLite ledger together comprise a **physical** M(t) — the consolidation tensor of the system, persisted on disk, structurally separate from the writer that produces it. M(t) here is not a mathematical abstraction; it is a queryable substrate.

This is the most direct ML analogue of UTL-v2's "memory function of sleep": durable, append-only, retrievable, separately verifiable.

---

## 8. State-transfer T (train → deploy gap)

UTML §VIII.b. The ML analogue of UTL-v2's surgeon-in-simulator-vs-OR transfer problem:

```
T(train, deploy) = exp(−d(P_train, P_deploy) / λ)
performance_deploy = M(t) · T(train, deploy)
```

Where `d` is a divergence measure (Wasserstein, MMD, KL, or task-specific) and `λ` is a transfer-bandwidth constant.

For ME-JEPA-Code:
- **Training distribution** = Phase 0 mutation corpus (8 categories × 300 SWE-bench Lite tasks = 2,400 deterministic mutations)
- **Deployment distribution** = real outer-loop Claude Code patches against SWE-bench Lite

The mutation corpus is precisely **domain randomization** in UTML's framing (Tobin et al. 2017) — generating a distribution of states close to the deployment distribution of human-introduced bugs and AI-generated patches. This is why doc 09 §3.1's mutation-corpus design is correct: it explicitly trains across a state distribution close to deployment, raising T post-training.

**Phase 5 active learning (§3.7.4)** further raises T at deploy time: when ME-JEPA's conformal interval is wide on a real attempt (high uncertainty in-distribution), surface to Docker oracle for ground-truth labeling; when OOD score is high, surface to operator. Both close `d(P_train, P_deploy)` adaptively.

---

## 9. The training cycle (UTML §IX adapted)

```
1. ASSESS: Measure ΔΩ and ΔΞ FIRST.
   ↓
2. REGULATE if ΔΩ < 0.4 or ΔΞ < 0.4:
   - Continual-backprop dormant-unit re-initialization
   - LR warmup reset / LR cosine-anneal restart
   - Gradient clipping band tightening
   - VICReg coefficient bump if predictor variance is collapsing
   - Re-fit constellation centroids if guard violation rate > threshold
   ↓
3. OBSERVE: ΔP, ΔK
   ↓
4. COMPUTE: L_step = ΔP · ΔK · ΔΩ · ΔΞ
   ↓
5. CLASSIFY:
     L_step < 0.1 for N steps, ΔΩ > 0.4, ΔΞ > 0.4 → Stuck (curriculum-prune)
     L_step < 0.1, ΔΩ < 0.4                       → Optimizer-Dysregulated (regulate ΔΩ)
     L_step < 0.1, ΔΞ < 0.4                       → Latent-Collapsing (regulate ΔΞ)
     L_step < 0.3                                 → Confused
     0.3 ≤ L_step < 0.7                            → Optimal
     L_step ≥ 0.7                                  → Boring / Mastery (advance)
     mean(L_step) > 0.3 but ΔM/mean(L_step) < 0.2 → Dissipating (consolidation problem)
   ↓
6. INTERVENE: curriculum, OR optimizer state, OR latent regularization — based on which signal is low
   ↓
7. EVALUATE: Did L_step improve? Did M (validation/holdout) hold over the next eval window?
   ↓
   (loop)
```

**Order matters.** Regulate ΔΩ and ΔΞ **before** adjusting ΔP or ΔK. Trying to fix prediction error by curriculum tweaks while the latent has collapsed is wasted compute — the system *cannot* improve on a degenerate latent.

### 9.1 Differentiated gating (precise form, UTML §IX bottom)

| Term | Primarily gates | Mechanism |
|---|---|---|
| ΔΩ.effective_plasticity | M consolidation rate | No plasticity ⇒ no gradient becomes parameter update |
| ΔΩ.landscape_health | ΔP integration | Flat minima broaden the basin where new updates can land |
| ΔΩ.stability_floor | Both ΔP sensitivity and M decay | NaN / overoptimization / Goodhart hard-gates everything |
| ΔΞ.target_collapse | All of L_step | Collapsed target ⇒ meaningless ΔP. **For ME-JEPA: 0 by construction.** |
| ΔΞ.predictor_redundancy | ΔK fidelity | Redundant slots inflate ΔK estimate |
| ΔΞ.constellation_violation | M consolidation | Off-constellation predictions corrupt downstream replay |
| ΔK.cos_align | ΔP→consolidation pipeline | Negative alignment = catastrophic forgetting |
| ΔK.fisher_violation | M consolidation | Protects prior consolidated knowledge |
| ΔK.ECE | RLHF / proxy-reward stability | Calibration is the early indicator of Goodhart |

---

## 10. Pruning rule (UTML §X)

```
prune_curriculum_item if (L_step < 0.1 for N steps)
                         AND (ΔΩ > 0.4 throughout)
                         AND (ΔΞ > 0.4 throughout)
```

**All three signals must agree.**

- If ΔΩ is low: diagnosis is *optimizer-state problem, not curriculum problem*. Fix the optimizer (continual backprop, LR warmup, gradient regularization) before pruning.
- If ΔΞ is low: diagnosis is *latent collapse, not curriculum problem*. Fix the latent (VICReg coefficient, instrument re-initialization, constellation re-fit) before pruning.

Pruning a curriculum item that failed under bad ΔΩ or bad ΔΞ teaches the wrong lesson and may permanently exclude useful training data.

---

## 11. Per-step training certificate (Phase 3 deliverable)

Every step's emitted training-cert JSON includes:

```json
{
  "step": 1234,
  "loss": 0.0234,
  "delta_p": 0.42,
  "delta_k": 0.71,
  "delta_omega": 0.83,
  "delta_xi": 1.0,
  "L_step": 0.247,
  "delta_omega_components": {
    "effective_plasticity": 0.85,
    "landscape_health": 0.90,
    "stability_floor": 1.0
  },
  "delta_xi_components": {
    "target_collapse": 0.0,
    "predictor_redundancy": 0.07,
    "constellation_violation_rate": 0.02
  },
  "sampling_weight": 0.247,
  "grad_norm": 0.83,
  "drift_coverage": 0.91,
  "merkle_root": "..."
}
```

Persisted to `CF_MEJEPA_TRAIN_CERTS` keyed by step. The chain of `parent_witness_hash → self_hash` makes the full training run a hash-chained log auditable post-hoc — this is the "physical M(t)" from UTML §XII (the audit substrate).

### 11.1 FSV protocol per Phase 3 DoD

1. Generate 100 synthetic examples spanning corner cases (perfect prediction, total miss, high-drift, low-grad, collapsed latent, OOD).
2. Compute L_step for each example.
3. Verify computed L_step matches the expected envelope per the formula.
4. Run a 1-epoch dry-train on the synthetic batch with sampling_weight active; verify the high-L_step examples appear more often in the sampled batches.
5. Verify ΔΞ.target_collapse is **always 0** (it should be, by construction; if not, an instrument was accidentally made trainable — fix the root cause).

### 11.2 Edge-case audit

- **ΔP = 0** (perfect prediction): L_step = 0. Skipped. Verify in `step.batch_indices` excludes the perfect-prediction example.
- **ΔK = 0** (catastrophic forgetting / cos_align < 0): L_step = 0. Step aborted; rollback to prior checkpoint. Verify in `CF_MEJEPA_DRIFT_STATE`.
- **ΔΩ = 0** (NaN / explosion / Goodhart): L_step = 0. Step abort + alert. Verify in alert log + `CF_MEJEPA_TRAIN_CERTS` shows skip reason.
- **ΔΞ.target_collapse > 0**: should be IMPOSSIBLE for ME-JEPA. If observed, an instrument has gradient flow — fix at the trait level.
- **ΔΞ.predictor_redundancy > 0.5**: VICReg coefficient too low — bump and refit.
- **All four at 1.0**: L_step = 1.0. Highest-priority example. Verify per-example weight reaches `sampling_weight ≥ 1.0` after `(1 + 5·operator_override) × decay^0` factors.

---

## 12. Reward and confidence (UTML §XIV bottom)

```
calibrated_confidence = raw_conf
                       · conv_rate
                       · strategy_agreement
                       · evidence_factor
                       · ΔΩ_mean
                       · ΔΞ_mean
                       clamped to [0.10, 0.95]

evidence_factor = 0.5 + 0.5 · min(1.0, num_paths / threshold)

Reward = 0.6 · calibrated_confidence + 0.4 · mean(L_step · ΔΞ)
```

- **Confidence is gated by both ΔΩ AND ΔΞ.** A confident prediction in a collapsed latent or a chronically unstable optimizer is not trustworthy.
- **Reward integrand is `L_step · ΔΞ`** — a step that does not preserve representational integrity does not contribute to the reward, even if its loss looked great.

For ME-JEPA-Code's `MeJepaCompiler::verify(patch)` API, the calibrated_confidence becomes the confidence interval reported in `Verdict::Pass{confidence_interval}` — the conformal interval computed by the existing Phase 4 conformal calibration is multiplied by the runtime-observed ΔΩ_mean × ΔΞ_mean to fail-closed when the model is in a degraded state.

---

## 13. Implementation roadmap

| Component | Where | Phase | Status |
|---|---|---|---|
| `compute_delta_p(predicted, target, snr, ...)` | `crates/context-graph-mejepa/learning_signal.rs::delta_p` | Phase 3 | deferred |
| `compute_delta_k(grad, ema_grad, fisher, ece)` | `learning_signal.rs::delta_k` | Phase 3 | deferred |
| `compute_delta_omega(plasticity, landscape, stability)` | `learning_signal.rs::delta_omega` | Phase 3 | deferred |
| `compute_delta_xi(target_collapse, redundancy, violation)` (target_collapse always 0 for ME-JEPA) | `learning_signal.rs::delta_xi` | Phase 3 | deferred |
| `sampling_weight(ex)` integrated into BatchSampler | `crates/context-graph-mejepa-train/sampler.rs` | Phase 3 | deferred |
| EMA running mean of recent batch gradients | `learning_signal.rs::grad_ema` | Phase 3 | deferred |
| Pairwise MI audit job (`dynamicjepa audit-pairwise-mi` analog) | `crates/context-graph-mejepa/audit.rs` | Phase 3 | deferred |
| Constellation centroid fitter | `crates/context-graph-mejepa/constellation.rs` | Phase 3 | deferred |
| Per-step (ΔP, ΔK, ΔΩ, ΔΞ, L_step) cert extension | `crates/context-graph-witness/cert.rs` | Phase 3 | deferred (cert format ✅ shipped; extension needed) |
| Periodic full-retrain k_replay=4 multiplier | `crates/context-graph-mejepa-train/scheduler.rs::full_retrain_step` | Phase 5 | deferred |
| Continual-backprop dormant-unit re-init | `crates/context-graph-mejepa-train/plasticity.rs` | Phase 5 | deferred |
| State-transfer T detector (active learning trigger) | `crates/context-graph-mejepa/state_transfer.rs` | Phase 5 | deferred |

---

## 14. Why this is the right formula for ME-JEPA-Code

UTML v1.0 is the rigorous ML rewrite of the embodied-learning UTL-v2 framework. Choosing it over the simpler three-term form gives ME-JEPA-Code:

1. **Structural collapse-immunity made measurable.** ΔΞ.target_collapse = 0 by construction is the *quantification* of why ME-JEPA's frozen-instrument architecture is the right design — and the formula makes the property visible in every per-step training certificate.
2. **Self-healing properties tied to specific signals.** When `ΔΩ < 0.4`, the system needs continual backprop. When `ΔΞ < 0.4`, the system needs constellation re-fit. The formula tells you *which intervention* to run, not just "retrain something".
3. **State-transfer T is now first-class.** Phase 8 evaluation against ≥0.95 correlation on holdout is exactly `performance_deploy = M(t) · T(train, deploy)`. The formula tells you whether a low Phase 8 score is a training problem (M(t) low) or a distribution-shift problem (T low).
4. **Goodhart catastrophe is gated.** If proxy reward decouples from gold metric, `ΔΩ.stability_floor → 0.1`, which zeros L_step, which halts updates that would otherwise compound the failure.
5. **Synthetic-data collapse (Shumailov 2024) is gated.** ΔΞ down-weights when training data is increasingly model-generated. ME-JEPA's training corpus is reality-grounded by design (Docker oracle outputs, real Python source, real witness chains), so this term stays favorable — but the formula makes the protection auditable.
6. **The audit substrate is the formula's M(t).** Every signal in L_step is persisted in the training certificate; M(t) integrates them into the BlackBox SQLite ledger; reading both back gives the byte-equal-verifiable consolidation tensor of the system.

---

## See also

- Source theory: `docs2/UTL_v2_embodied.md` (UTML v1.0 — JEPA Edition)
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md` §3.4 (training procedure), §3.7 (multi-objective loss), §5 (self-healing)
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- Specs/status: `specs/_README.md` and `specs/implementation-plan.md` (Phase 2 predictor + Phase 3 ΔP / ΔK / ΔΩ / ΔΞ integration)
- ContextGraph Reality Loop white paper: Royse (2026a) — UTML §XII canonical reference for the audit substrate as physical M(t)
- Dynamic / ME-JEPA paper: Royse (2026b) — the structural collapse-immunity claim with multi-seed evidence
