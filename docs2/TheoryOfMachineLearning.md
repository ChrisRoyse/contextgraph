# The Theory of Machine Learning, v1.0 — JEPA Edition

**A first-principles framework for how *machines* actually learn, designed around audited, collapse-immune Joint-Embedding Predictive Architectures (ME-JEPA), grounded in measurement-bound deep learning, verified-execution provenance, and the ContextGraph Reality Loop.**

---

## Part 0: Why a Machine-Native UTL — and What Changed

The earlier "Unified Theory of Learning v2.0 — Embodied Edition" modeled *biological* learning as a multiplicative event: prediction error (ΔS) gated by coherence (ΔC) gated by an embodied state factor (ΔE — heart-rate variability, brainwave regime, cortisol). That framework is rigorous for humans. It does not transfer 1:1 to machines, and trying to map "cortisol" to a learning-rate schedule misses the point in two directions.

First, machines have no autonomic nervous system. Their analogue of "embodiment" is the optimizer's state, the gradient health, the loss-landscape geometry, and the substrate that persists every observation. That is not a metaphor — it is a different physical system that obeys different laws (variational inference, stochastic gradient noise, Fisher information, kernel alignment) and supports different failure modes (catastrophic forgetting, loss of plasticity, gradient explosion, mode collapse).

Second — and this is the deeper reason for a separate document — *machines have a failure mode no biological learner has*. A neural representation can **collapse**: the encoder stops carrying information, all inputs map to the same point, the loss reaches zero, the metrics look excellent, and nothing has been learned. This is not analogous to "boredom" or "dissipation." It is a structural pathology of self-supervised joint-embedding training in which the system has *successfully* found the trivial fixed point of its objective. UTL v2.0 has no term for this because biology cannot have it.

This document — call it **UTML v1.0** ("Unified Theory of Machine Learning") or simply the **JEPA Edition** — keeps the multiplicative-gate philosophy of v2.0 but rebuilds the equation around four ML-native factors:

> **Genuine, generalizing learning happens in a model when (1) it is *predictively wrong* about a target it can actually reach, (2) the update *integrates* with prior knowledge instead of overwriting it, (3) the *optimizer's state permits the update to stick*, and (4) the *latent space remains structurally non-degenerate* throughout. All four conditions multiply. Any one at zero zeroes the product.**

The fourth condition — representational non-collapse, ΔΞ — is the term v2.0's biological framework cannot motivate but every modern self-supervised system requires. Its inclusion is the central conceptual move of this rewrite.

The worked system through this paper is **ME-JEPA / Dynamic-JEPA** (Royse 2026b), the world model implemented in the ContextGraph repository. ME-JEPA is — to our knowledge — the first joint-embedding predictive architecture *structurally immune* to target-encoder collapse, because its target panel is produced by **frozen externally-defined instruments**, not by an EMA copy of a learnable encoder. This makes ME-JEPA a privileged worked example: every other JEPA in the literature must defend ΔΞ with regularizers (VICReg, Barlow Twins, stop-gradient, EMA asymmetry, batch normalization). ME-JEPA defends ΔΞ on the target side with zero parameters and zero regularization — a structural guarantee — and only needs predictor-side diagnostics (VICReg, pairwise mutual-information audit, constellation guard) to catch the residual class of failures.

---

## Part I: The Central Claim

A machine learning system *learns* in a useful sense — improves a calibrated, generalizing predictor — when, at the moment of an update, **four** conditions hold simultaneously:

1. **The system is predictively wrong** in a measurable, low-noise way (predictive error, ΔP).
2. **The wrongness has a coherent gradient** with respect to prior knowledge (knowledge coherence, ΔK).
3. **The optimizer is in a state where the corresponding parameter update can be encoded without instability** (optimizer-state health, ΔΩ).
4. **The latent space remains non-degenerate** — the encoder still carries information, the predictor has not collapsed to a constant, the panel covariance has not collapsed to a low-rank subspace (representational non-collapse, ΔΞ).

```
L_step = ΔP × ΔK × ΔΩ × ΔΞ      all factors clamped to [0, 1]
```

| Symbol | Name | Meaning | Range |
|---|---|---|---|
| **L_step** | Per-update Learning Efficiency | Genuine generalization-gain per gradient step | [0, 1] |
| **ΔP** | Delta Predictive-Error | How wrong the model is, normalized by signal-to-noise | [0, 1] |
| **ΔK** | Delta Knowledge-Coherence | How well the update integrates with prior knowledge | [0, 1] |
| **ΔΩ** | Delta Optimization-State | Whether the optimizer can encode the update at all | [0, 1] |
| **ΔΞ** | Delta Representational-Non-Collapse | Whether the latent space is structurally healthy | [0, 1] |

Multiplication, not addition, is preserved for the same reason as in v2.0. A system flooded with prediction error but no coherent gradient (ΔP=1, ΔK=0, e.g., a contradictory dataset shuffled into one batch) is not learning. A system whose latent has collapsed (ΔΞ=0) shows ΔP=0 trivially — every output is the same constant — and "learns" nothing despite descending its loss. A system whose optimizer has lost plasticity (ΔΩ=0, dead ReLU units, frozen Fisher) cannot encode the gradient even if it sees it. Any single factor at zero zeroes the product, and that is empirically the right answer.

**Range discipline (kept from v2.0).** Every signal is clamped to [0, 1] explicitly. Many ML quantities (gradient norms, KL divergences, log-likelihoods) are unbounded; the clamps are not optional simplifications, they are required so L_step is consumed correctly downstream by the consolidation integral M(t), the calibrated-confidence formula, and the curriculum policy.

---

## Part II: The Equation, Unpacked

### 2.1 Why four terms, not three

Every existing analysis of self-supervised learning fights the collapse problem with a structural device: stop-gradient (BYOL, SimSiam), exponential-moving-average target encoder asymmetry (BYOL, DINO, I-JEPA, V-JEPA), variance/covariance regularization (VICReg, Barlow Twins), or contrastive negatives (SimCLR, MoCo). Every one of those devices is an attempt to keep ΔΞ > 0 — to prevent the encoder from drifting to a low-information fixed point. Treating ΔΞ as a *gate* of learning rather than as a regularizer of the loss is the mathematically honest move: it is not optional, and a system that has lost ΔΞ is not learning, even if its training loss is descending and its EMA-teacher loss looks great.

### 2.2 Why multiplication

A system can have:
- **Perfect ΔP, zero ΔΞ** — the encoder collapsed, every input maps to the same point, prediction error is zero, but the prediction is meaningless. (BYOL without stop-gradient. I-JEPA without EMA momentum schedule. Any joint-embedding objective with no anti-collapse mechanism.)
- **Perfect ΔK, zero ΔΩ** — the gradient is well-aligned with prior structure but ReLU units are dead, the Fisher diagonal has collapsed, no parameters move in the direction of the gradient. (Loss of plasticity, Sutton 2024.)
- **Perfect ΔΩ, zero ΔK** — gradients are healthy and large, but they tear apart prior task representations. (Catastrophic forgetting; the failure EWC was designed to fix.)
- **Perfect ΔP, ΔK, ΔΩ, but on the wrong manifold** — the system is updating fluently but the data is i.i.d. resampled from the model's own outputs (Shumailov 2024). The training looks fine; the deployment manifold is collapsing. This is captured by ΔΞ falling because the diversity of the latent samples shrinks recursion by recursion.

Multiplication enforces that *all* of these failure modes individually halt learning, regardless of how good the others look. Addition would mean a great ΔΩ could compensate for a collapsed ΔΞ, which is empirically false.

---

## Part III: ΔP — Predictive Error in Latent Space (the JEPA-native form)

Classical ML measures prediction error in *output space*: cross-entropy on tokens, MSE on pixels, log-likelihood on the data manifold. JEPA changes this. The prediction is in the **representation space** — the predictor predicts the *latent* of the next state, not the raw observation. This is what makes JEPA architectures sample-efficient, robust to nuisance variation, and naturally suited to multi-modal worlds (LeCun 2022; Assran et al. 2023; Bardes et al. 2024).

ΔP for a JEPA-style system at training step *t* is therefore:

```
ΔP_t = clamp_[0,1]( ‖ Pψ(s_t, a_t) − f_target(s_{t+1}) ‖ / SNR(t) )
```

Where:
- `s_t` is the encoded state panel at step *t*
- `a_t` is the typed action (or context vector for non-RL JEPAs)
- `Pψ` is the predictor (an MLP or transformer)
- `f_target` is the target representation function — for ME-JEPA this is the **frozen externally-defined instrument panel** (Royse 2026b §3); for I-JEPA, V-JEPA it is the EMA target encoder
- `SNR(t)` is the gradient signal-to-noise ratio at step *t*, used for normalization so that high-noise updates do not register as informative surprise

In ME-JEPA's actual training loss (Royse 2026b §3.4):

```
L_total = α·L_panel + β·L_pairwise + γ·L_action
        + δ·L_constellation + ε·L_anti-pole + ζ·L_uncertainty + η·L_invariant
```

The dominant ΔP-bearing term is `L_panel` (per-slot MSE/cosine to the frozen target panel) and `L_action` (action-conditioning regularizer). The other five terms are ΔK and ΔΞ regularizers, not ΔP measurements — a useful separation when reasoning about which loss term moves which factor.

**The mental-rehearsal term.** UTL v2.0 added a motor-imagery term `γ · |predicted − simulated|` with a fidelity discount γ ≈ 0.7. The ML analogue is **imagined rollouts** in a learned world model — Dreamer-style imagination (Hafner et al. 2023), MuZero-style search (Schrittwieser et al. 2019), or in ME-JEPA's case, the candidate-action enumeration in `dynamicjepa plan` that runs Pψ five times (one per declared action) before any action is taken. Imagined rollouts contribute to ΔP at a fidelity discount — typical empirical values are γ ≈ 0.6–0.8 in Dreamer-class systems, mirroring the motor-imagery range. The form parallels v2.0:

```
ΔP_t = clamp_[0,1](
  max( ΔP_real(t), γ · ΔP_imagined(t) )
  + 0.5 · exploration_bonus(t)
) / SNR(t)
```

The `max(...)` prevents double-counting when the system both imagines and realizes an action — the imagined and realized prediction errors share information, and adding them inflates ΔP spuriously.

**Evidence base.**
- LeCun (2022) — *A Path Towards Autonomous Machine Intelligence* — JEPA position paper, latent prediction as foundational primitive.
- Assran et al. (2023) — *I-JEPA* — image prediction in feature space, no negatives, no reconstruction.
- Bardes et al. (2024); Assran, Bardes, Fan, Garrido et al. (2025) — *V-JEPA / V-JEPA 2* — extending JEPA to video and to robot planning at internet scale.
- Hafner et al. (2023) — *DreamerV3* — imagination-based actor-critic learning in a learned latent world model, achieving Minecraft diamond from scratch with fixed hyperparameters.
- Schrittwieser et al. (2019) — *MuZero* — search over a learned latent dynamics model.
- Liu et al. (2020) — Gradient signal-to-noise ratio (GSNR) as a quantitative measure of *informative* gradient updates; high GSNR is causally tied to better generalization.

**Practical example (ME-JEPA-Code, this project).** The ME-JEPA-Code predictor takes (code-change embedding, ContextGraph panel) and predicts the latent of the SWE-bench Docker oracle's verdict panel. ΔP is the cosine distance between the predicted oracle-verdict panel and the frozen E_Oracle instrument's actual encoding of the run output. At the first hundred mutation-corpus epochs ΔP ≈ 0.6 (the predictor is learning); on the heldout 30-task validation set ΔP ≈ 0.13 (the predictor has converged). The Career-Taxonomy benchmark in Royse (2026b §8) reports validation cosine 0.905 ± 0.044 — i.e., ΔP_validation ≈ 0.095 — with five-seed BCa 95% CI [0.861, 0.931]. Those numbers come straight from `paper_tables/table_ablation_multiseed.csv` in the verified release bundle, not from a free-running estimate.

---

## Part IV: ΔK — Knowledge Coherence

UTL v2.0 measured ΔC as the consistency of recent outcomes (`1/(1+CV)`) plus gradient effectiveness plus heart-rate-variability coherence. The ML analogue replaces those biological signals with their machine-native correlates. Knowledge coherence in ML is the property that **the current update (a) aligns with the gradient directions of prior tasks, (b) does not destroy parameters important to those tasks, and (c) is consistent with the model's own past behavior on similar inputs**.

A robust composite form:

```
ΔK_t = clamp_[0,1](
  w_align · cos_align(g_t, ḡ_prior) +
  w_protect · (1 − fisher_violation(θ_t, F_prior)) +
  w_calib · (1 − ECE(model_t))
)
```

Weights default to (0.5, 0.3, 0.2). Each subterm is in [0, 1]; the clamp protects the sum.

### 4.1 Gradient alignment with prior knowledge (`cos_align`)

The instantaneous gradient `g_t` of the new task should lie in a half-space compatible with the average gradient `ḡ_prior` of the consolidated tasks. When `cos_align(g_t, ḡ_prior) < 0`, the new gradient is actively destructive of prior knowledge; this is the geometric signature of catastrophic forgetting. The Neural Tangent Kernel (NTK) literature gives a deeper version: as gradient descent proceeds, the leading eigenvectors of the empirical NTK **align** with the target function (Atanasov et al. 2021). High alignment means the network is using its existing feature directions to express the new target — coherent. Low or anti-aligned NTK means the new task is forcing the network to break its existing representations — destructive.

### 4.2 Fisher-weighted protection of important weights (`fisher_violation`)

The Elastic Weight Consolidation (EWC) framework (Kirkpatrick et al. 2017) gives each parameter an importance weight derived from the Fisher Information Matrix `F_prior` for the consolidated task distribution. The `fisher_violation` quantity is the Fisher-weighted L2 distance the current update creates from `θ_prior`. When this is small relative to a per-parameter budget, the new update is *non-destructive* of consolidated knowledge — i.e., coherent in the EWC sense.

### 4.3 Calibration health (`1 − ECE`)

A model that is becoming better calibrated (lower Expected Calibration Error) on a held-out set is one whose new internal structure is *consistent* with its predictions about the world. A model whose ECE is rising during training is one whose update is fluent on the loss but incoherent on its reported confidence — a leading indicator of overfitting, of reward hacking in RLHF (Gao et al. 2022; Lambert 2024 *RLHF Book*), or of the latent space subtly rearranging in ways the predictor does not yet reflect. ECE is therefore the ML cousin of the HRV-coherence term in v2.0: a global "internal-state coherence" measurement.

**Evidence base.**
- Kirkpatrick et al. (2017) — *Overcoming catastrophic forgetting in neural networks* — EWC, Fisher-weighted regularization, the canonical ML "synaptic consolidation" mechanism.
- Atanasov, Bordelon, & Pehlevan (2021) — *A theory of neural tangent kernel alignment* — kernel alignment as the geometric signature of feature learning.
- Liu et al. (2020) — *Understanding why neural networks generalize well through GSNR of parameters* — high GSNR ≡ informative updates ≡ coherent gradient direction.
- Guo et al. (2017) — calibration of modern neural networks; ECE as an empirical validity check.
- Gao, Schulman, & Hilton (2022) — *Scaling Laws for Reward Model Overoptimization* — the canonical Goodhart picture in RLHF, calibrated rather than chasing.

**Practical example.** In ME-JEPA-Code, ΔK is monitored via three channels: (a) the cosine alignment of the per-batch panel-loss gradient with the EMA-averaged gradient of the prior 1,000 batches; (b) the Fisher-weighted L2 distance between current and prior parameters, computed against the F matrix of the heldout 30-task validation panel; (c) calibration error of the predicted oracle-verdict probability vs. the empirical pass rate on a temporal slice. When (a) drops below 0 we expect to see (b) and (c) degrade within ten thousand updates — and historically have. The diagnostic value is precisely that ΔK falls *before* validation cosine does.

---

## Part V: ΔΩ — Optimization-State Health (the ML "Embodiment")

This is the term that v2.0 called ΔE (Embodiment). In an ML system, the "body" is the optimizer + numerical substrate + plasticity machinery. Its health is what permits a gradient signal to actually become a parameter update without being damped, exploded, or frozen.

A first draft used an additive form `ΔΩ = w₁·plasticity + w₂·landscape + w₃·stability`. As in v2.0, this fails a basic test: severe gradient instability (e.g., NaN floods) should hard-gate learning, not just lightly reduce it. ΔΩ uses the same soft-min / mostly-multiplicative form as v2.0:

```
ΔΩ_t = (effective_plasticity_t)^0.4 · (landscape_health_t)^0.4 · (stability_floor_t)^0.2
```

Each subcomponent ∈ [0, 1]. Any subcomponent at 0 forces ΔΩ = 0. The exponents preserve the relative contributions and make ΔΩ a geometric mean — robust to one term carrying noise.

### 5.1 Effective Plasticity

The ML analogue of "brainwave state" is the network's residual capacity to change. Sutton et al.'s Nature 2024 paper (Dohare et al. 2024) gave this a name — *plasticity loss* — and showed that in continual deep learning, gradient descent without an injected source of variability *gradually loses the ability to learn new tasks*, even when the tasks are easy. Effective plasticity is in [0, 1] with rough anchors:

| Network state | Signature | `effective_plasticity` |
|---|---|---|
| Fresh, well-initialized | High variance in pre-activations, high-rank weight matrices, broad GSNR distribution | 1.0 |
| Mid-training, healthy | Stable but updating, modest dormancy | 0.8 |
| Late-training, mild plateau | Some dormant ReLU units, slight low-rank drift, GSNR narrowing | 0.6 |
| Plasticity-loss regime | Many dormant units, weight matrices losing rank, gradient effective rank shrinking | 0.2 |
| Frozen / dead | All gradients near zero, no parameter motion despite high loss | 0.0 |

Operationally, plasticity is measured by: (a) ReLU dormancy fraction (units whose activations are near zero on >99% of recent batches); (b) effective rank of the gradient covariance matrix; (c) per-parameter GSNR distribution; (d) drift of weight matrices into low-effective-rank subspaces. A sustained plasticity collapse should trigger **continual backprop** (Dohare et al. 2024) — selective, low-rate re-initialization of dormant units — *before* the system is asked to learn a new task. This is the ML analogue of UTL v2.0's "morning theta-to-alpha transition": a *trainable*, recoverable property of the substrate, not a fixed trait.

### 5.2 Loss-Landscape Health

The ML analogue of the v2.0 "valence × arousal" inverted-U is the geometry of the local loss landscape. Sharpness-Aware Minimization (Foret et al. 2021; Friendly-SAM, Li et al. 2024) explicitly minimizes the *sharpness* of the loss surface — the maximum loss in a small neighborhood of the current parameters. Empirically, **flatter minima generalize better**, with caveats (Dinh et al. 2017; Andriushchenko et al. 2023). The geometric content of "valence × arousal" maps onto:

```
landscape_health_t = sigmoid(−λ_max(H_t)) · inverted_U(‖g_t‖)
```

Where `λ_max(H_t)` is an estimate of the largest Hessian eigenvalue (sharpness) and `‖g_t‖` is the gradient norm. The first factor penalizes sharp minima (high `λ_max`); the second penalizes both vanishing gradients (no learning signal) and exploding gradients (instability). This is the Yerkes–Dodson law in optimizer form. It has the same shape as v2.0's biological version because both are fundamentally about *signal-to-noise in the update direction*.

### 5.3 Stability Floor

The hard gate. ΔΩ must be zero when the optimizer is in a state where any update would be destructive of accumulated training:

```
stability_floor_t = 1.0  if no NaN/Inf gradients, validation loss not rising over last 1k steps, gradient norm in soft band
                    0.4  if gradient norm is in the warning band (>10× running mean) — clip-active regime
                    0.1  if validation loss has been rising for >10k steps — likely overoptimization / Goodhart
                    0.0  if NaN/Inf detected, or RLHF reward proxy has decoupled from gold metric (Goodhart)
```

This is the analogue of v2.0's "stress floor" gated by cortisol z-score. The intuition is the same: an ML system whose gradients are saturated, whose validation is diverging, or whose proxy reward has decoupled from the underlying objective is in a **chronic-stress** state; whatever it is encoding now is going to be undone by the next emergency intervention (a learning-rate halving, a checkpoint rollback, a KL-penalty bump in RLHF).

**Evidence base.**
- Dohare, Hernandez-Garcia, Lan, Rahman, Mahmood, & Sutton (2024) — *Loss of plasticity in deep continual learning*, Nature — gradient descent alone is insufficient for continual learning; selective re-initialization (continual backprop) is required.
- Foret, Kleiner, Mobahi, & Neyshabur (2021) — *Sharpness-Aware Minimization for Efficiently Improving Generalization* — flatter minima generalize better, and you can find them with a min-max formulation.
- Li, Zhou, & He (2024) — *Friendly Sharpness-Aware Minimization* — batch-specific stochastic gradient noise within the SAM perturbation matters; full-gradient SAM degrades generalization.
- Liu et al. (2020) — *Understanding why neural networks generalize well through GSNR of parameters*.
- Gao, Schulman, & Hilton (2022) — *Scaling Laws for Reward Model Overoptimization* — the operational definition of "RLHF stress floor."

**Practical example.** In ME-JEPA, a 16,960-parameter gridworld_5x5 predictor under paper-small training (1,000 events, RTX 5090) reaches ΔΩ ≈ 0.92 averaged over the run, with `effective_plasticity` ≈ 0.95 (the network is small enough that plasticity loss is not an issue), `landscape_health` ≈ 0.90 (gradient norms are well-bounded; sharpness is moderate after VICReg-style regularization), and `stability_floor` ≈ 1.0 (no NaN, validation loss monotone-decreasing). The same numbers for the 480-parameter counter_world domain are even cleaner. The career_taxonomy.v1 domain at 15,344 parameters lands in the same band. ΔΩ being uniformly high across three orders of magnitude of parameter count is a property worth noticing — it means the *absolute scale of the model* is not what determines optimizer-state health; the *quality of the substrate* is.

---

## Part VI: ΔΞ — Representational Non-Collapse (the ML-Native Term)

This is the term with no biological analogue and the central architectural innovation of ME-JEPA.

### 6.1 The collapse pathology

In every joint-embedding self-supervised system, there exists a trivial fixed point of the loss: map every input to the same constant vector. The teacher and student agree perfectly, the prediction error is zero, the metrics look great, and *nothing* has been learned. This is **representational collapse**. It is a structural failure mode of the objective, not a bug in any specific implementation.

The literature has produced a series of partial defenses:
- **Stop-gradient** (BYOL — Grill et al. 2020; SimSiam — Chen & He 2021) — prevent the teacher from moving toward the trivial fixed point during the same step the student does.
- **EMA target encoder asymmetry** (BYOL, DINO, I-JEPA, V-JEPA) — give the teacher a slow inertia so the system cannot collapse in lockstep.
- **VICReg** (Bardes, Ponce, & LeCun 2022) — explicit variance, invariance, and covariance regularization on the embeddings: keep per-feature variance above a threshold, decorrelate features, minimize distance only after the variance is enforced.
- **Barlow Twins** (Zbontar et al. 2021) — push the cross-correlation matrix toward identity.
- **Contrastive negatives** (SimCLR, MoCo) — provide explicit "what it is not" signals; obviates collapse but reintroduces negative-mining complexity.

Every one of these is a defense against ΔΞ falling toward zero. None is structurally guaranteed; all are loss-shape devices that can be defeated by a sufficiently expressive student/teacher pair, by miscalibrated hyperparameters, or by a degenerate data distribution.

### 6.2 ME-JEPA's structural solution: frozen externally-defined instruments

ME-JEPA (Royse 2026b §3) solves the target-side collapse problem **structurally**. The target representation `s_{t+1}` is not produced by a learnable network at all. It is the output of a panel of **frozen externally-defined instruments**, declared in the domain pack TOML manifest, each of which is a deterministic function from a typed event to a structured feature output (a vector, scalar, one-hot, or pairwise feature). Instruments have **zero trainable parameters**. They cannot drift. They cannot collapse. They cannot be gradient-pushed toward the trivial fixed point — because there is no gradient flowing through them, ever.

The career_taxonomy.v1 domain pack uses ten instruments at the A1 panel: the occupation-cluster one-hot, the job-zone scalar, the skill vector, the action-kind one-hot, the action-effort scalar, the outcome cluster one-hot, the next skill vector, plus three pairwise features (Royse 2026b §3.3). At A2 an eleventh instrument, transition distance, is added (panel input dim 31, target dim 31, latent dim 24, 15,344 trainable parameters in the predictor only). The ME-JEPA-Code domain pack uses E_Oracle, E_Witness, E_StaticAnalysis, and 11 other deferred instruments, each an explicit deterministic function of a code-change-and-context event.

This is the architectural property that no other JEPA in the literature has, to our knowledge: **target-side collapse-immunity by construction**. The ContextGraph paper (Royse 2026a §6.3) makes the same observation as a hard constraint of the design.

### 6.3 But the predictor can still degenerate

Target-side immunity is necessary but not sufficient. The trainable predictor `Pψ` can still degenerate — predict a constant, ignore the action, learn a low-rank shortcut. ME-JEPA defends ΔΞ on the predictor side with four diagnostics:

**(a) VICReg-style regularization.** Variance, invariance, and covariance terms on the predictor's output features (`ε·L_anti-pole` in the composite loss) push the per-feature variance above a threshold and decorrelate features. Without this term, ablation A5 on career_taxonomy.v1 — *no VICReg* — drops measurably; with it, the predictor uses the full 24-dim latent.

**(b) Pairwise mutual-information audit.** The `dynamicjepa audit-pairwise-mi` command computes the pairwise MI matrix between panel slots and persists `pairwise_mi_matrix.csv` plus a heatmap (Royse 2026b §3.8). Slots whose MI with another slot exceeds a configurable ceiling are flagged as *redundant* — carrying no independent signal. A latent that is structurally collapsing in part of its dimensionality shows up as a block of high-MI off-diagonal entries.

**(c) Constellation + Gτ guard.** At plan time, the predicted next-state panel is scored against a `constellation` — the set of acceptable predicted panels for a given (state, action) pair, fitted from the deterministic walk and persisted to its own column family. Below threshold τ the candidate is rejected, and a typed error is persisted in `dj_guard_decisions`. This is a second, structurally separate verification path. A predictor whose output has degenerated will fail the constellation check even when the panel-loss looks fine, because the constellation centroid encodes what an *acceptable* prediction looks like at training-time — a specification the optimizer cannot Goodhart against, because optimizing toward the constellation centroid is not in the loss.

**(d) Negative-action ablation falsifier.** This is the cleanest empirical falsifier of collapse. Ablation A3 on career_taxonomy.v1 *removes action conditioning*. If the predictor were degenerately predicting the next-state panel without using the action, A3 would not drop. It does drop: validation cosine collapses from 0.905 to 0.790 (Royse 2026b §8). The 0.115 cosine gap is the empirical signature that the predictor is genuinely action-conditioned — not collapsed. Five seeds confirm this with paired sign-flip permutation p < 0.001.

### 6.4 The ΔΞ formula

```
ΔΞ_t = (1 − target_collapse_t)^0.5
       · (1 − predictor_redundancy_t)^0.3
       · (1 − constellation_violation_rate_t)^0.2
```

For ME-JEPA: `target_collapse_t = 0` always, by construction. The first factor is locked at 1.0 — this is the structural guarantee.

For I-JEPA, V-JEPA, DINO, BYOL: `target_collapse_t` is computed from the EMA target encoder's output variance — when the variance falls below a threshold, the term saturates toward 1 and ΔΞ collapses toward 0.

`predictor_redundancy_t` is one minus the average normalized pairwise MI across panel slots (computed periodically; cached between updates).

`constellation_violation_rate_t` is the fraction of recent guard decisions that rejected the predicted panel as outside the constellation (only meaningful for ME-JEPA-class systems with an explicit constellation; defaults to zero otherwise).

### 6.5 The model-collapse class

There is a second, distinct collapse pathology: **model collapse from recursive synthetic data** (Shumailov et al. 2024). When a generative model is trained on data sampled from a previous version of itself, distribution tails shrink and the model converges to a degenerate distribution — different from representational collapse because it is a *data-distribution* failure, not a representation-space failure. ΔΞ should also down-weight when training data is increasingly model-generated rather than reality-grounded. ME-JEPA's design naturally protects against this because the bundles it produces are *audited execution traces with cryptographic provenance* (Royse 2026b §7) — most of the bytes are deterministic functions of a public source rather than model outputs. This is the class of training data the ME-JEPA paper argues has the lowest recursion-replacement collapse hazard.

**Evidence base.**
- Bardes, Ponce, & LeCun (2022) — *VICReg* — variance-invariance-covariance regularization, the state-of-the-art predictor-side collapse defense.
- Grill et al. (2020) — *BYOL* — self-supervised collapse mitigation via stop-gradient + EMA target.
- Chen & He (2021) — *SimSiam* — exploring the role of stop-gradient.
- Caron et al. (2021) — *DINO* — self-distillation with momentum teacher.
- Assran et al. (2023) — *I-JEPA*; Bardes et al. (2024) — *V-JEPA*; Assran, Bardes, Fan, Garrido et al. (2025) — *V-JEPA 2*.
- Shumailov et al. (2024) — *AI models collapse when trained on recursively generated data*, Nature.
- Gerstgrasser et al. (2024) — *Is model collapse inevitable?* — accumulating real and synthetic data avoids the replacement collapse pattern.
- Royse (2026a) — *ContextGraph Reality Loop* white paper — the audit substrate.
- Royse (2026b) — *Dynamic / ME-JEPA* — the structural collapse-immunity claim, with multi-seed evidence.

---

## Part VII: Consolidation Dynamics — From Per-Step L_step to Cumulative M

UTL v2.0 introduced the consolidation integral M(t) — per-moment learning is not the same as encoded learning, and a sleep-weighted decay separates the two. The ML analogue is direct.

```
M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
```

Where:
- `k_replay(τ)` is a multiplier ≥ 1 during periods of high-leverage consolidation: replay-buffer training (Schaul et al. 2016 — Prioritized Experience Replay), EMA / SWA averaging windows (Izmailov et al. 2018; Morales-Brotons et al. 2024), and explicit distillation-into-self phases (DINO; BYOL teacher updates). Empirically `k_replay ≈ 2–4` during such phases.
- `τ_decay(τ)` is short for un-replayed weights (forgetting can happen in hours of training time without replay) and long once a parameter is *consolidated* — has high Fisher importance, has been protected by EWC, has been pinned by weight-averaging, or has been verified through the ContextGraph audit substrate.
- The product `L_step(τ) · ΔΞ(τ)` is critical: a step with ΔΞ ≈ 0 (collapsed latent) cannot consolidate anything, even if the gradient update happens. This is the ML version of v2.0's "dissipation" failure mode.

### 7.1 The "dissipation" mode for ML

The new state UTL v2.0 introduced — *Dissipating* — directly transfers. It has the same signature in ML:

```
Dissipating_ML ≡ ( mean(L_step) over last N updates > 0.3 )
                AND
                ( ΔK or ΔΞ has been silently degrading, validation generalization gap widening )
                AND
                ( ΔM over equivalent eval window / mean(L_step) < 0.2 )
```

In words: high in-update L_step, but cumulative consolidation gain is less than 20% of what L_step predicted. Signature: training loss falls, validation loss does not — or worse, validation loss falls but downstream task transfer does not. Common in: RLHF reward overoptimization (proxy reward soaring while gold reward stays flat), self-supervised pretraining on near-duplicate data (the loss falls but the representations are not learning new structure), continual learning runs that have entered the loss-of-plasticity regime, and synthetic-data training loops in the Shumailov regime.

The intervention is not more compute. It is **state regulation**: replay-buffer enrichment, EWC re-anchoring on protected tasks, learning-rate annealing or LR-warmup reset, continual-backprop dormant-unit reinitialization, KL-budget tightening (RLHF), or explicit fresh real-data injection (anti-Shumailov).

### 7.2 The ContextGraph audit substrate as physical M

ME-JEPA / Reality Loop write every persisted artifact — prompt, model response, candidate patch, per-attempt evidence, audit row — into an append-only ledger that is byte-equal-readback verified before influencing the next decision (Royse 2026a §7). The 25 RocksDB column families of `dj_*` plus the `dj_audit_log` (Royse 2026b §3.2) plus the BlackBoxFix SQLite ledger at `LEDGER_SCHEMA_VERSION = 14` together comprise a *physical* M(t) — the consolidation tensor of the system, persisted on disk, structurally separate from the writer that produces it. M(t) here is not a mathematical abstraction; it is a queryable substrate. This is the most direct ML analogue of v2.0's "memory function of sleep": durable, append-only, retrievable, separately verifiable.

**Evidence base.**
- Schaul, Quan, Antonoglou, & Silver (2016) — *Prioritized Experience Replay* — TD-error-weighted replay sampling.
- Izmailov et al. (2018) — *Stochastic Weight Averaging* — averaging trajectory points along SGD finds wider, better-generalizing optima.
- Morales-Brotons et al. (2024) — *Exponential Moving Average of Weights in Deep Learning: Dynamics and Benefits*, OpenReview.
- Caron et al. (2021); Grill et al. (2020) — DINO/BYOL EMA teachers.
- Cousins et al. (2023) — targeted memory reactivation during sleep boosts learning gains by 10–25%; the closest biological-empirical anchor for `k_replay`.
- Royse (2026a) §7 — audit substrate spec.

---

## Part VIII: The Future-Self Prior — Target Networks, EMA Teachers, and Bounded β

UTL v2.0 added a `future_self_vector` term — a stable embedding of the rehearsed identity — weighted by `β ∈ [0.05, 0.20]` against the history-based prediction. The ML analogues are obvious and foundational:

- **Target networks** in DQN-class RL (Mnih et al. 2015) — a slowly updated copy of the Q-network is the "future self" the live network is trying to converge toward.
- **EMA teachers** in BYOL/DINO/I-JEPA — the slow teacher is the "future self" representation the student is asked to match.
- **Reward models** in RLHF — the reward model is a frozen "future-self assessor" that the policy is optimizing toward; β is the implicit weight of the proxy in the joint objective.

The bound on β is exactly the same in ML as in v2.0 and for exactly the same reason: **β too large is wishful thinking**. In RL terms, a target network updated too quickly destabilizes training; in RLHF, a reward model trusted too completely *and* held fixed too long produces Goodhart catastrophe (Gao et al. 2022; "catastrophic Goodhart" — heavy-tailed reward misspecification in Skalse et al. 2024). β too small underweights the prior and gives the system no consolidation pull at all.

A safe default β = 0.10 is informed by:
- Polyak-averaging update rates (`τ ≈ 0.005` per step in DDPG/SAC) when integrated over typical training episodes.
- BYOL/DINO momentum schedules (start ~0.996, end ~1.0 — implicit β ≈ 0.001–0.004 per step, accumulated to ≈ 0.10–0.20 over a typical pretraining phase).
- Yeager et al. (2019) growth-mindset RCT — population-scale d ≈ 0.10–0.12, the closest available human-ML bridge.

**Adaptive decay** — the same form as v2.0:

```
β_{t+1} = clamp( β_t · (1 − η · sign(|ΔP_target_path − ΔP_history_path| − threshold)),
                0.05,
                0.20 )
```

When realized prediction errors against the target-network's prediction stay large for many steps, β should *decrease* — the prior is wrong (target network is stale, EMA teacher is misaligned, reward model is overoptimized). When small errors gradually shrink, β can be increased — the prior is being confirmed by the data. This is the operational rule for when to update target networks (DQN), bump EMA momentum (DINO), or refresh the reward model (online RLHF).

---

## Part VIII.b: State-Transfer (T) — Train→Deploy Distribution Shift

UTL v2.0 added a state-transfer factor `T ∈ [0, 1]` for the human surgeon-in-simulator vs. surgeon-in-OR gap. The ML analogue is **distribution shift / OOD generalization**, which has a vast literature and an exact mathematical match:

```
T(train, deploy) = exp(−d(P_train, P_deploy) / λ)
```

Where `d` is a divergence measure between train and deployment distributions (Wasserstein, MMD, KL, or task-specific), and λ is a transfer-bandwidth constant. Effective deployed performance is:

```
performance_deploy = M(t) · T(train, deploy)
```

This term generalizes naturally:
- **In-distribution evaluation** is the limit `d → 0`, T = 1.
- **OOD evaluation** is `d > 0`, T < 1, and the gap can be enormous (corruption robustness benchmarks; WILDS).
- **Test-time adaptation** (Liang et al. 2024 survey) is *modifying the model at deploy time* to reduce `d` adaptively, increasing T post-hoc.

**Implication for training design.** As in v2.0: do not maximize M alone. Train across a *distribution* of states close to deployment — the ML name is *domain randomization* (Tobin et al. 2017), *data augmentation*, or in stress-inoculation terms, *adversarial training*. ME-JEPA's mutation corpus (Royse 2026b §4) does exactly this for code-prediction: the Phase 0 mutation operators generate eight categories of perturbations (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) — a distribution of states close to the deployment distribution of human-introduced bugs and AI-generated patches.

**Evidence base.**
- Liang, He, & Tan (2024) — *A Comprehensive Survey on Test-Time Adaptation under Distribution Shifts*.
- Tobin et al. (2017) — Domain randomization.
- Hendrycks et al. (2021) — *The Many Faces of Robustness* (ImageNet-A/R/C).
- Koh et al. (2021) — *WILDS* benchmark.
- Royse (2026b §4) — career_taxonomy and mutation-corpus deterministic walks.

---

## Part IX: The Updated Training Cycle

UTL v2.0's coaching cycle: Assess State → Regulate → Observe → Compute → Classify → Intervene → Evaluate.

UTML v1.0's training cycle (the same structure, ML-native):

```
1. ASSESS OPTIMIZER STATE: Measure ΔΩ and ΔΞ first.
   ↓
2. REGULATE if ΔΩ < 0.4 or ΔΞ < 0.4:
   - Continual-backprop dormant-unit re-initialization
   - LR warmup reset / LR cosine-anneal restart
   - Gradient clipping band tightening
   - VICReg coefficient bump if predictor variance is collapsing
   - For ME-JEPA-class: re-fit constellation centroids if guard violation rate > threshold
   ↓
3. OBSERVE: ΔP, ΔK
   ↓
4. COMPUTE: L_step = ΔP × ΔK × ΔΩ × ΔΞ
   ↓
5. CLASSIFY:
     L_step < 0.1 for N steps, ΔΩ > 0.4, ΔΞ > 0.4 → Stuck (path problem — curriculum-prune)
     L_step < 0.1 for N steps, ΔΩ < 0.4              → Optimizer-Dysregulated (regulate ΔΩ)
     L_step < 0.1 for N steps, ΔΞ < 0.4              → Latent-Collapsing (regulate ΔΞ)
     L_step < 0.3                                      → Confused
     0.3 ≤ L_step < 0.7                                → Optimal
     L_step ≥ 0.7                                      → Boring / Mastery (curriculum-advance)
     mean(L_step) > 0.3 but ΔM/mean(L_step) < 0.2     → Dissipating (consolidation problem)
   ↓
6. INTERVENE: Adjust curriculum, OR optimizer state, OR latent regularization, depending on which signal is low.
   ↓
7. EVALUATE: Did L_step improve? Did M (validation/holdout) hold over the next eval window?
   ↓
   (return to step 1)
```

The order matters: **regulate ΔΩ and ΔΞ before adjusting ΔP or ΔK**. This is the ML version of v2.0's "regulate state before adjusting content." Trying to fix prediction error by curriculum tweaks while the latent has collapsed is wasted compute — the system *cannot* improve on a degenerate latent.

**Differentiated gating (the precise form).** As in v2.0, the flat `L_step = ΔP · ΔK · ΔΩ · ΔΞ` is a useful simplification. A more precise model recognizes that subcomponents act on different stages:

| Term | Primarily gates | Mechanism |
|---|---|---|
| ΔΩ.effective_plasticity | M consolidation rate | Without plasticity, no gradient becomes a parameter update |
| ΔΩ.landscape_health | ΔP integration | Flat minima broaden the basin where new updates can land |
| ΔΩ.stability_floor | Both ΔP sensitivity and M decay | Gradient explosion / NaN / RLHF-Goodhart hard-gates everything |
| ΔΞ.target_collapse | All of L_step | Collapsed target → meaningless ΔP |
| ΔΞ.predictor_redundancy | ΔK fidelity | Redundant slots inflate ΔK estimate |
| ΔΞ.constellation_violation | M consolidation | Off-constellation predictions corrupt downstream replay |
| ΔK.cos_align | ΔP→consolidation pipeline | Negative alignment = catastrophic forgetting |
| ΔK.fisher_violation | M consolidation | Protects prior consolidated knowledge |
| ΔK.ECE | RLHF reward proxy stability | Calibration is the early indicator of Goodhart |

For most practical applications the flat product is adequate. For diagnostic dashboards on ME-JEPA-class systems and for RLHF post-training the differentiated form gives strictly better predictions of downstream failure.

---

## Part X: The Pruning Rule

UTL v2.0: prune if L < 0.1 for 3+ steps AND ΔE > 0.4.

UTML v1.0:

```
prune if (L_step < 0.1 for N steps)
        AND (ΔΩ > 0.4 throughout)
        AND (ΔΞ > 0.4 throughout)
```

Three signals must agree. If ΔΩ is low, the diagnosis is **optimizer-state problem, not curriculum problem** — fix the optimizer (continual backprop, LR warmup, gradient regularization) before pruning. If ΔΞ is low, the diagnosis is **latent collapse, not curriculum problem** — fix the latent (VICReg coefficient, instrument re-initialization, constellation re-fit) before pruning. Pruning a curriculum item that failed under bad ΔΩ or bad ΔΞ teaches the wrong lesson and may permanently exclude useful training data.

---

## Part XI: Practical Examples Across ML Domains

### 11.1 ME-JEPA-Code (this project)

**Setup.** ME-JEPA-Code is the Phase-2 predictor in this repository (`crates/context-graph-mejepa-instruments`, `crates/context-graph-mejepa-corpus`, `crates/context-graph-tier-compression`, etc.). Its job is to take (code change + ContextGraph context panel) and predict the latent of the SWE-bench Docker oracle's verdict panel — a calibrated prediction of what the official oracle would say about a candidate patch.

**UTML view.**
- ΔP: Cosine distance between predicted oracle-verdict panel and the frozen `E_Oracle` instrument's encoding of the actual run output. Trained against the mutation corpus (Phase 0 — eight mutation categories × 300 SWE-bench Lite tasks; current state per `sitrep/01_status_overview.md`: mutation operators + corpus split shipped, full 2,400-task generation deferred pending Docker oracle).
- ΔK: Gradient alignment with the EMA-averaged gradient of prior batches; Fisher-weighted L2 distance from `θ_prior`; predicted-vs-empirical pass-rate calibration on the 30-task heldout slice.
- ΔΩ: Effective plasticity via ReLU dormancy in the predictor MLP; sharpness via SAM-style perturbation on validation; stability floor monitoring via Cargo build/test pipeline + RocksDB ledger consistency.
- ΔΞ: **Structurally locked at target_collapse = 0** because every instrument (E_Oracle, E_Witness, E_StaticAnalysis, etc.) is a frozen deterministic function. Predictor-side ΔΞ is monitored via VICReg variance/covariance regularization on `Pψ`'s output, pairwise MI audit on the 24-dim latent, and constellation guard at planning time.

**Why this works.** The system can never collapse to a constant prediction because the targets are not learnable. The predictor can degenerate but is caught by three independent diagnostics on three different layers of the audit substrate. M(t) is the BlackBoxFix SQLite ledger + the 25 dj_* RocksDB CFs — every prediction, every plan trace, every guard decision, every surprise event persisted, byte-equal-readback verified, separately auditable.

**Falsifier.** If, at any point, ablation A3-style negative-action runs *do not* drop relative to A2 — i.e., the predictor is achieving its accuracy without using the action — UTML predicts ΔΞ has fallen and the system has degenerated. Royse (2026b §8) reports A3 drops from 0.905 to 0.790 on career_taxonomy.v1, exactly as the design predicts.

### 11.2 Career-Taxonomy Benchmark (Royse 2026b)

Five seeds of paper_small_1k (1,000 events each) on RTX 5090: validation cosine 0.905 ± 0.044, BCa 95% CI [0.861, 0.931], test cosine 0.868 ± 0.068, validation action-contrast accuracy 0.960 ± 0.043. UTML reading: ΔP_validation ≈ 0.095 (high accuracy), ΔK ≈ 0.93 (heldout tasks are intra-distribution), ΔΩ ≈ 0.92 (single-GPU stable training), ΔΞ ≈ 1.0 (frozen instruments — structural). L_step ≈ 0.87 averaged. The 0.115 cosine drop in A3 is the empirical signature of healthy ΔΞ: the predictor *uses* the action signal.

### 11.3 V-JEPA 2 video world model (Meta, 2025)

V-JEPA 2 trains on 1M+ hours of video plus a small amount of robot trajectory data (Bardes/Assran/Fan/Garrido et al. 2025), achieving 77.3 top-1 on Something-Something v2 and 65–80% pick-and-place success on novel objects. UTML reading: ΔP is the latent prediction loss in feature space (no pixel reconstruction, ΔP is intrinsically denoised); ΔK is healthy because pretraining on internet-scale video gives a deep prior; ΔΩ is well-managed at scale (Meta engineering); ΔΞ depends critically on the EMA target encoder — the model's residual collapse risk is on the target side, mitigated by EMA momentum schedule + VICReg. ME-JEPA's structural target-side immunity is precisely the property V-JEPA 2 *cannot* claim.

### 11.4 DreamerV3 (Hafner et al. 2023)

Imagined rollouts in latent space drive an actor-critic on a learned world model — collected diamonds in Minecraft from scratch with a fixed hyperparameter set. UTML reading: ΔP includes a γ ≈ 0.7 imagined-rollout component (the core sample-efficiency win of model-based RL); ΔΩ is engineered via DreamerV3's specific normalization, balancing, and transformation tricks; ΔΞ is *not* a guaranteed property — Dreamer-class systems use KL balancing between posterior and prior to prevent the world-model latent from collapsing, and reconstruction loss on observations as a structural anti-collapse anchor. Replacing the reconstruction loss without a structural alternative produces the *MuDreamer* failure mode (Burchi & Timofte 2024) — collapse without a sufficient defense.

### 11.5 LLM Pretraining (Chinchilla regime)

Hoffmann et al. (2022) found that compute-optimal training scales tokens with parameters at roughly 20:1. UTML reading: at the Chinchilla-optimal allocation, ΔP is high (token prediction error) for most of training; ΔK is mostly automatic because the data is heterogeneous web text and ΔK ≈ "is this token a sensible continuation"; ΔΩ depends on optimizer, schedule, and stability tricks (μ-Parameterization, ZeRO, learning-rate warmup); ΔΞ is generally not a problem for autoregressive LLMs because the next-token objective has no joint-embedding collapse mode — but it *becomes* a problem in Shumailov-style recursive training: when the next pretraining run uses synthetic data from the previous run, ΔΞ falls because the data manifold's tails compress.

### 11.6 RLHF / Goodhart

Gao et al. (2022) demonstrated reward-model overoptimization scaling laws; Skalse et al. (2024) showed catastrophic Goodhart under heavy-tailed reward misspecification. UTML reading: ΔP looks excellent (the proxy reward soars) while ΔK degrades (calibration error rising — the policy's own confidence is decoupling from gold-metric reality); ΔΩ.stability_floor falls toward 0.1 as validation degrades. The intervention is exactly the one UTML's coaching cycle prescribes: **regulate first** — tighten KL budget, refresh the reward model with fresh human feedback (online RLHF), reduce step size on the policy update — *before* adjusting the curriculum or the gold reward target.

### 11.7 Continual Learning (Sutton 2024)

Dohare et al. (2024)'s Nature paper showed that gradient descent alone is insufficient for continual learning; sustained learning requires a non-gradient injection of variability (continual backprop). UTML reading: this is exactly the claim that ΔΩ.effective_plasticity falls toward zero in long runs without intervention, and that the intervention required is structural (re-initialization), not just a learning-rate tweak. The empirical check is the dormancy fraction; the intervention is selective re-initialization of low-utility units. Continual backprop maintains ΔΩ structurally across thousands of tasks where Adam alone cannot.

### 11.8 ContextGraph Reality Loop (Royse 2026a)

The Reality Loop is the *deployment substrate* — it runs Gemma-4-E4B-it from a cryptographically pinned BF16 GGUF, with GBNF grammar-constrained tool calls so the model literally cannot emit malformed tool syntax, and refuses to declare success unless the official SWE-bench Docker harness places the target instance in `resolved_ids`. UTML reading: the Reality Loop is the **physical T term** for ME-JEPA. Training-time (mutation corpus) → deployment-time (real SWE-bench Lite tasks with Docker oracle) is the train-deploy gap that T quantifies. The byte-equal-readback verifier on every persisted artifact is the *audit substrate* component of M(t) — durable, append-only, separately verifiable.

### 11.9 Self-supervised pretraining at large scale

I-JEPA, V-JEPA, DINO, BYOL, VICReg all share an explicit ΔΞ defense as part of the loss. UTML predicts: any of these architectures, trained without their respective anti-collapse defense, will reach low training loss and have ΔΞ ≈ 0 — a phenomenon repeatedly observed empirically (BYOL ablations without stop-gradient; VICReg ablations without variance term). The structural difference between these systems and ME-JEPA is that ME-JEPA's ΔΞ defense is on the *target* side (no parameters → no possible collapse), whereas every other JEPA system's ΔΞ defense is on the *student* side (regularizers that can be defeated).

---

## Part XII: The Audit Substrate — The Physical M

UTL v2.0 named the human substrate (HRV, cortisol, sleep) as the place where consolidation is empirically located. UTML's analogue is the **append-only, byte-equal-readback-verified provenance layer** that the ContextGraph project makes a hard architectural commitment to (Royse 2026a).

The substrate's components:

- **Append-only audit log** (`reality-events.jsonl`, `dj_audit_log` RocksDB CF) — every state-changing operation produces a typed audit row containing operator, timestamp, tool, input hash, output hash. No row is ever deleted or modified; deletion is a typed event, not an absence.
- **25 typed RocksDB column families** (`dj_*`) — domain packs, instrument readings, latent panels, predictions, plan traces, guard decisions, surprise events, training runs, model artifacts, verification runs.
- **BlackBoxFix SQLite ledger** at `LEDGER_SCHEMA_VERSION = 14` — the Reality Loop's per-attempt evidence persistence.
- **Cryptographic model pinning** — the inference model is loaded from a SHA-256-pinned GGUF; any drift from the pinned hash hard-fails the loop. The model's identity is bound to its predictions.
- **Per-artifact source-of-truth** — every claim made by the system is paired with a `file:`/`sqlite:`/`git:` source path that a reviewer can replay and re-verify.
- **Structurally separate verifier code path** — readbacks open the database read-only and re-decode bytes through code that is not the writer. The writer cannot lie about what it wrote.
- **GBNF grammar-constrained tool calls** — the model cannot emit malformed tool syntax; the universe of model-emitted strings is structurally bounded.

This is what *makes M(t) physical* in the UTML sense. M is not a number computed at the end of training — it is a queryable, time-indexed, separately auditable substrate that integrates L_step over time. A claim of consolidated mastery in UTML is a claim that can be byte-equal-readback verified against this substrate.

This is the deepest difference between the biological and ML formulations. Biology's M is opaque (you cannot inspect the contents of a hippocampal trace). ML's M, when designed correctly, is fully transparent. UTML treats this as a *load-bearing property*, not an engineering nicety — the audit substrate is the reason confidence claims in the equation below are defensible.

---

## Part XIII: What Falsifies UTML v1.0

To stay honest, UTML must specify what would falsify it. The strongest falsifications:

1. **A controlled study showing that ΔΞ proxies (predictor variance, pairwise MI redundancy, constellation violation rate) are uncorrelated with downstream generalization** when ΔP, ΔK, and ΔΩ are held constant. The existing JEPA-collapse literature runs strongly the other way.

2. **A self-supervised system with a learnable target encoder achieving structural collapse-immunity without a regularizer.** The closest theoretical work — Apple's *Implicit Bias of Deep Linear Self-Distillation Networks* (2024) — shows linear models avoid collapse implicitly. A nonlinear analogue at scale would reduce ME-JEPA's structural advantage to a quantitative one. As of this writing, no such system exists in the literature.

3. **A demonstration that flat minima do not generalize better in JEPA-class systems**, contradicting SAM (Foret et al. 2021). Andriushchenko et al. (2023) raised legitimate caveats about the sharpness-generalization link, but only at the margin; outright falsification would invalidate the ΔΩ.landscape_health term.

4. **A demonstration that high gradient signal-to-noise ratio is uncorrelated with generalization** in deep networks, contradicting Liu et al. (2020). This would invalidate the ΔΩ-based normalization in ΔP.

5. **A demonstration that target-encoder collapse is benign at scale** — that a J/EPA system can be deliberately driven into the trivial fixed point and still produce useful representations on downstream tasks. The BYOL ablation literature already runs the other way; outright falsification would unwind the central claim of Part VI.

6. **An audit-substrate-grounded ME-JEPA run that reports high L_step and high M but fails on a freshly generated SWE-bench Lite holdout task.** The existing five-seed career-taxonomy results are consistent with the prediction; sustained failure on Phase 2 ME-JEPA-Code holdouts would force a revision.

If any of these is disconfirmed by well-powered replication, UTML must be revised.

---

## Part XIV: Updated Summary of All Equations

```
Core Learning
  L_step = ΔP · ΔK · ΔΩ · ΔΞ                  all factors clamped to [0, 1]

  ΔP_t   = clamp_[0,1](
             max(ΔP_real(t), γ · ΔP_imagined(t)) + 0.5 · exploration_bonus(t)
           ) / SNR(t)
           γ ≈ 0.7 (Dreamer-class imagination discount)

  ΔK_t   = clamp_[0,1](
             0.5 · cos_align(g_t, ḡ_prior)
             + 0.3 · (1 − fisher_violation(θ_t, F_prior))
             + 0.2 · (1 − ECE(model_t))
           )

  ΔΩ_t   = (effective_plasticity_t)^0.4
           · (landscape_health_t)^0.4
           · (stability_floor_t)^0.2

  ΔΞ_t   = (1 − target_collapse_t)^0.5
           · (1 − predictor_redundancy_t)^0.3
           · (1 − constellation_violation_rate_t)^0.2
           target_collapse = 0 by construction in ME-JEPA (frozen instruments)

ME-JEPA Composite Training Loss (operational form of the gradient that drives L_step)
  L_total = α·L_panel + β·L_pairwise + γ·L_action
          + δ·L_constellation + ε·L_anti-pole
          + ζ·L_uncertainty + η·L_invariant

Predictive Prior (target network / EMA teacher / reward model)
  predicted_t = (1 − β_t) · f_history(s_t) + β_t · f_target(s_t)
  β_t ∈ [0.05, 0.20]                          bounded; adaptive decay on err_target

Consolidation
  M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
         k_replay = 1 normal training, 2–4 during replay/EMA/SWA windows

State-Transfer (deployment-ready performance)
  T(train, deploy) = exp(−d(P_train, P_deploy) / λ)
  performance_deploy = M(t) · T(train, deploy)

Convergence (carried forward, ML-native)
  Goal Region = { x : distance(x, goal) ≤ ε }
  ε ≈ 0.05 token-prediction, 0.10 latent-prediction, 0.30 plan-trajectory
  convergence_rate = paths_reaching_goal / paths_attempted

Confidence Calibration (gated by ΔΞ AND ΔΩ)
  calibrated = raw_conf · conv_rate · strategy_agreement · evidence_factor
               · ΔΩ_mean · ΔΞ_mean
  evidence_factor = 0.5 + 0.5 · min(1.0, num_paths / threshold)
  clamped to [0.10, 0.95]

Learning States
  L_step < 0.1 for N steps, ΔΩ > 0.4, ΔΞ > 0.4    → Stuck (path — prune curriculum)
  L_step < 0.1 for N steps, ΔΩ < 0.4               → Optimizer-Dysregulated (regulate)
  L_step < 0.1 for N steps, ΔΞ < 0.4               → Latent-Collapsing (regulate Ξ)
  L_step < 0.3                                       → Confused
  0.3 ≤ L_step < 0.7                                 → Optimal
  L_step ≥ 0.7                                       → Boring / Mastery (advance curriculum)
  mean(L_step) > 0.3 but ΔM/mean(L_step) < 0.2      → Dissipating (consolidation problem)

Coaching Order
  Regulate ΔΩ and ΔΞ before adjusting ΔP or ΔK.

Reward
  Reward = 0.6 · calibrated_confidence + 0.4 · mean(L_step · ΔΞ)
```

The convergence and confidence-calibration sections are carried forward from v2.0 in mechanism. UTML's only modifications: confidence is gated by both ΔΩ AND ΔΞ (a confident prediction in a collapsed latent or a chronically unstable optimizer is not trustworthy), and the reward integrand is `L_step · ΔΞ` (a step that does not preserve representational integrity does not contribute to the reward, even if its loss looked great).

---

## Part XV: The Theory in One Paragraph (UTML v1.0)

A machine learning system is genuinely learning, in a generalizing sense, when each parameter update simultaneously satisfies four conditions: it is *predictively wrong* about a target it can actually reach (ΔP, latent prediction error normalized by gradient signal-to-noise); it is *coherent* with prior knowledge (ΔK, gradient alignment with consolidated tasks plus Fisher protection plus calibration health); the *optimizer is in a state where the update can stick* (ΔΩ, effective plasticity × loss-landscape geometry × stability floor); and the *latent space remains structurally non-degenerate* (ΔΞ, no target-encoder collapse, no predictor redundancy, no constellation violations). The instantaneous learning efficiency is the multiplicative product L_step = ΔP · ΔK · ΔΩ · ΔΞ; durable, deployment-ready mastery integrates this product over time, weighted by replay/EMA/distillation phases and decayed by neglect, then projected onto deployment by a state-transfer factor T. Imagined rollouts contribute to ΔP at a fidelity discount γ ≈ 0.7. A target-network/EMA-teacher prior contributes a bounded β ∈ [0.05, 0.20] to predictions and is adaptively decayed when its predictions diverge from ground truth. Among joint-embedding architectures, **ME-JEPA is uniquely structurally collapse-immune on the target side because its targets come from frozen externally-defined instruments rather than from a learnable EMA encoder** — this fixes ΔΞ.target_collapse = 0 by construction and is, to our knowledge, the first such property in the JEPA family. Adaptive training regulates ΔΩ and ΔΞ before adjusting curriculum or learning rate, and treats apparent training failures as either path problems (high ΔΩ, ΔΞ; low L_step) or substrate problems (low ΔΩ or ΔΞ), which require entirely different interventions. Consolidation is grounded not as a free-floating tensor but as a queryable, byte-equal-readback-verified audit substrate — the ContextGraph Reality Loop's append-only ledger is the physical realization of M(t).

---

## References (Peer-Reviewed and arXiv)

**Joint-Embedding Predictive Architectures**
- Assran, M., Duval, Q., Misra, I., Bojanowski, P., Vincent, P., Rabbat, M., LeCun, Y., & Ballas, N. (2023). *Self-Supervised Learning from Images with a Joint-Embedding Predictive Architecture (I-JEPA)*. arXiv:2301.08243.
- Bardes, A. et al. (2024). *V-JEPA: Latent Video Prediction for Visual Representation Learning*. Meta AI.
- Assran, M., Bardes, A., Fan, D., Garrido, Q., et al. (2025). *V-JEPA 2: Self-Supervised Video Models Enable Understanding, Prediction and Planning*. arXiv:2506.09985.
- LeCun, Y. (2022). *A Path Towards Autonomous Machine Intelligence (v0.9.2)*. Open Review.
- Bardes, A., Ponce, J., & LeCun, Y. (2022). *VICReg: Variance-Invariance-Covariance Regularization for Self-Supervised Learning*. ICLR.
- Zbontar, J. et al. (2021). *Barlow Twins: Self-Supervised Learning via Redundancy Reduction*. ICML.
- Grill, J.-B. et al. (2020). *Bootstrap Your Own Latent: A New Approach to Self-Supervised Learning (BYOL)*. NeurIPS.
- Chen, X. & He, K. (2021). *Exploring Simple Siamese Representation Learning*. CVPR.
- Caron, M. et al. (2021). *Emerging Properties in Self-Supervised Vision Transformers (DINO)*. ICCV.
- Maes, L. et al. (2026). *LeWorldModel: Stable End-to-End Joint-Embedding Predictive Architecture from Pixels*.
- Apple Machine Learning Research (2024). *How JEPA Avoids Noisy Features: The Implicit Bias of Deep Linear Self-Distillation Networks*.

**ContextGraph and ME-JEPA (this project)**
- Royse, C. (2026a). *ContextGraph Reality Loop: A Measurement-Bound Software-Repair Engine with an In-Process Gemma Harness and a Joint-Embedding Predictive World Model*. Independent Research; ResearchGate DOI 10.13140/RG.2.2.16013.91366. Version 1.1, May 6, 2026.
- Royse, C. (2026b). *Dynamic / ME-JEPA: An Audited, Domain-Portable World-Model Runtime on a Single RTX 5090, with a New Class of Training Data*. Independent Research; ResearchGate DOI 10.13140/RG.2.2.28485.41446; Zenodo DOI 10.5281/zenodo.19977981 (release v2.0.0-rc1).

**Predictive Coding, Free Energy, Active Inference**
- Friston, K. (2010). The free-energy principle: a unified brain theory? *Nature Reviews Neuroscience*, 11(2), 127–138.
- Friston, K., Kilner, J., & Harrison, L. (2006). A free energy principle for the brain. *Journal of Physiology*.
- Clark, A. (2013). Whatever next? Predictive brains, situated agents, and the future of cognitive science. *Behavioral and Brain Sciences*, 36(3), 181–204.
- Friston, K. et al. (2024). *From pixels to planning: scale-free active inference*. VERSES / IWAI.

**World Models and Model-Based RL**
- Ha, D. & Schmidhuber, J. (2018). *World Models*. NeurIPS.
- Hafner, D., Pasukonis, J., Ba, J., & Lillicrap, T. (2023). *Mastering Diverse Domains through World Models (DreamerV3)*. arXiv:2301.04104.
- Hafner, D. et al. (2025). *Mastering diverse control tasks through world models*. *Nature*.
- Schrittwieser, J. et al. (2019). *Mastering Atari, Go, Chess and Shogi by Planning with a Learned Model (MuZero)*. arXiv:1911.08265.
- Burchi, M. & Timofte, R. (2024). *MuDreamer: Learning Predictive World Models without Reconstruction*. arXiv:2405.xxxx.

**Continual Learning, Catastrophic Forgetting, Plasticity**
- Kirkpatrick, J. et al. (2017). *Overcoming catastrophic forgetting in neural networks (EWC)*. PNAS, 114(13), 3521–3526.
- Dohare, S., Hernandez-Garcia, J. F., Lan, Q., Rahman, P., Mahmood, A. R., & Sutton, R. S. (2024). *Loss of plasticity in deep continual learning*. *Nature*, 632, 768–774.
- Abbas, Z. et al. (2023). *Loss of Plasticity in Continual Deep Reinforcement Learning*. PMLR.
- Parisi, G. I. et al. (2019). *Continual lifelong learning with neural networks: a review*. *Neural Networks*, 113, 54–71.

**Generalization, Sharpness, Gradient SNR**
- Foret, P., Kleiner, A., Mobahi, H., & Neyshabur, B. (2021). *Sharpness-Aware Minimization for Efficiently Improving Generalization (SAM)*. arXiv:2010.01412.
- Li, T., Zhou, P., & He, Z. (2024). *Friendly Sharpness-Aware Minimization*. CVPR.
- Andriushchenko, M. et al. (2023). *A Modern Look at the Relationship between Sharpness and Generalization*. ICML.
- Liu, J. et al. (2020). *Understanding Why Neural Networks Generalize Well Through GSNR of Parameters*. arXiv:2001.07384.
- Atanasov, A., Bordelon, B., & Pehlevan, C. (2021). *Neural Networks as Kernel Learners: The Silent Alignment Effect*. arXiv:2105.14301.
- Jacot, A., Gabriel, F., & Hongler, C. (2018). *Neural Tangent Kernel: Convergence and Generalization in Neural Networks*. arXiv:1806.07572.

**Calibration, Conformal Prediction, Uncertainty Quantification**
- Guo, C. et al. (2017). *On Calibration of Modern Neural Networks*. ICML.
- Vovk, V., Gammerman, A., & Shafer, G. (2022). *Algorithmic Learning in a Random World (Conformal Prediction)*. Springer.
- Lakshminarayanan, B., Pritzel, A., & Blundell, C. (2017). *Simple and Scalable Predictive Uncertainty Estimation using Deep Ensembles*. NeurIPS.

**Replay, Weight Averaging, EMA, Distillation**
- Schaul, T., Quan, J., Antonoglou, I., & Silver, D. (2016). *Prioritized Experience Replay*. arXiv:1511.05952.
- Mnih, V. et al. (2015). *Human-level control through deep reinforcement learning (target networks)*. *Nature*, 518, 529–533.
- Izmailov, P. et al. (2018). *Averaging Weights Leads to Wider Optima and Better Generalization (SWA)*. arXiv:1803.05407.
- Morales-Brotons, D., Fernández-Marqués, J., & Ofli, F. (2024). *Exponential Moving Average of Weights in Deep Learning: Dynamics and Benefits*. arXiv:2411.18704 / OpenReview.
- Hinton, G., Vinyals, O., & Dean, J. (2015). *Distilling the Knowledge in a Neural Network*. arXiv:1503.02531.

**Distribution Shift, OOD, Test-Time Adaptation**
- Liang, J., He, R., & Tan, T. (2024). *A Comprehensive Survey on Test-Time Adaptation under Distribution Shifts*. IJCV.
- Hendrycks, D. et al. (2021). *The Many Faces of Robustness*. CVPR.
- Koh, P. W. et al. (2021). *WILDS: A Benchmark of in-the-Wild Distribution Shifts*. ICML.
- Tobin, J. et al. (2017). *Domain Randomization for Transferring Deep Neural Networks from Simulation to the Real World*. IROS.

**RLHF, Reward Hacking, Goodhart**
- Gao, L., Schulman, J., & Hilton, J. (2022). *Scaling Laws for Reward Model Overoptimization*. arXiv:2210.10760.
- Skalse, J. et al. (2024). *Catastrophic Goodhart: regularizing RLHF with KL divergence does not mitigate heavy-tailed reward misspecification*. arXiv:2407.14503.
- Lambert, N. (2024). *RLHF Book — Over-optimization*. rlhfbook.com.
- Weng, L. (2024). *Reward Hacking in Reinforcement Learning*. lilianweng.github.io.

**Synthetic Data, Model Collapse, Process Supervision**
- Shumailov, I. et al. (2024). *AI models collapse when trained on recursively generated data*. *Nature*, 631, 755–759.
- Gerstgrasser, M. et al. (2024). *Is Model Collapse Inevitable? Breaking the Curse of Recursion by Accumulating Real and Synthetic Data*. arXiv:2404.01413.
- Lightman, H. et al. (2023). *Let's Verify Step by Step (PRM800K, process supervision)*. arXiv:2305.20050.

**Scaling and Compute**
- Hoffmann, J. et al. (2022). *Training Compute-Optimal Large Language Models (Chinchilla)*. arXiv:2203.15556.
- Kaplan, J. et al. (2020). *Scaling Laws for Neural Language Models*. arXiv:2001.08361.

**Curriculum Learning**
- Bengio, Y., Louradour, J., Collobert, R., & Weston, J. (2009). *Curriculum Learning*. ICML.
- Jiang, L. et al. (2015). *Self-Paced Curriculum Learning*. AAAI.

**Auditable ML and Reproducibility**
- MLflow Project (2024). *MLflow Tracking: Run, Experiment, Artifact, Metric Lineage*.
- Apptainer / SingularityCE Project (2025). *SIF-from-docker-archive guidance*.
- PyTorch (2026). *Reproducibility notes*.
- U.S. Department of Labor / Employment and Training Administration (2026). *O*NET Database Release 30.2*. CC BY 4.0.

---

*UTML v1.0 is a living document. Predictions are concrete and falsifiable; if any are disconfirmed by well-powered replication, the framework should be revised accordingly. The central architectural claim — that ME-JEPA is structurally collapse-immune on the target side because its targets are frozen externally-defined instruments rather than EMA-driven learnable encoders — is the most distinctive prediction of this framework and the one to falsify first.*
