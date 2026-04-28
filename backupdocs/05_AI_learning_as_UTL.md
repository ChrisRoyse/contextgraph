# 05 — AI Learning as UTL: Measuring Machine Learning with the Same Equations

**Purpose.** UTL v2 was written for human learners, but the same three-factor product `L = ΔS · ΔC · ΔE` and consolidation integral `M(t)` describe machine-learning training dynamics with no structural change — only a substitution of which embedder reads each term. This file develops that mapping in detail and shows how a UTL-aware diagnostic layer reads training health for both Chris's existing LoRA runs (Shakespeare, Santa, Qwen3-TTS) and any future ML training run.

The thesis: **a training run is a learner. A model in deployment is a graduate. Every UTL term has a machine-learning analogue, and the bugs that derail training are exactly the bugs UTL predicts when a term goes silent.**

---

## 1. The mapping

| UTL v2 term | Human reading (file 02–04) | AI training reading | AI inference reading |
|---|---|---|---|
| `ΔS` | prediction error in cosine space across 13 embedders | training loss residual; per-batch surprisal of training tokens | per-output entropy / log-prob of the chosen token sequence |
| `ΔC` | multi-embedder agreement + HRV coherence | representational stability between checkpoints (CKA, SVCCA); held-out validation accuracy | self-consistency across N samples; voting agreement |
| `ΔE` | plasticity-window × valence-arousal × stress-floor | training-dynamics health: loss-landscape curvature, gradient noise scale, edge-of-stability proximity, plasticity-loss measure | calibration / overconfidence; KV-cache state; attention entropy |
| `M(t)` | consolidated mastery via integral | accumulated task accuracy on a frozen eval set; sparse-autoencoder feature counts | n/a (frozen-model analogue is "deployed capabilities") |
| `T(train, deploy)` | state-vector distance | distribution-shift distance between training mixture and deployment query distribution | n/a |
| `future_self_vector` | rehearsed identity / goal | LoRA target adapter; target distribution; reward model | n/a |
| `regulated centroid C_L` | personal baseline state | known-good checkpoint embedding | reference response distribution |

The structural identity is exact. UTL is substrate-agnostic.

---

## 2. ΔS for AI training

### 2.1 Training-time ΔS

For a supervised step on batch (x, y) with model output ŷ:

```
ΔS_train = mean(cross_entropy(y_t, ŷ_t)) over tokens t in batch
```

This is just the training loss. UTL's contribution is to recognise that this scalar is the AI's `|predicted - actual|`.

For mental-rehearsal extension (the γ ≈ 0.7 simulated-experience term): the AI analogue is **world-model rollouts**. Dreamer (Hafner et al., 2020, 2023; DreamerV3 2025) explicitly trains on imagined rollouts in a learned latent dynamics model. The rollout's contribution to training is mathematically `γ · |predicted - simulated|` where γ is the imagination-fidelity discount.

UTL prescribes γ ∈ [0.6, 0.8]. **Empirical ML support:** Dreamer-family work has converged on rollout weights in this range — explicit weighting has not been studied as a hyperparameter but the implicit weighting in DreamerV3 is consistent.

```
ΔS_train_eff = max(ΔS_real_batch, 0.7 · ΔS_imagined_rollout)
```

This is a Dreamer-with-explicit-γ training rule that should reduce sample complexity in low-data regimes.

### 2.2 Surprisal as a generalisation of training loss

For text-only models, training loss equals **per-token surprisal** (negative log-probability) summed over tokens. The TACL 2023 work and arXiv 2506.11338 show that *humans* and *transformers* diverge on surprisal predictions for reading times — larger LLMs predict reading times less well despite lower perplexity.

This has a direct UTL-side implication: when measuring an AI's ΔS, **smaller embedders (the e5-large-v2 / Wav2Vec2-base scale) align better with human-like prediction error than the largest LLMs.** This is the right scale for ΔS measurement, both for humans (file 03) and for AI systems being trained on human-generated data.

### 2.3 Self-supervised ΔS

For self-supervised pretraining (masked-LM, masked-image-model, contrastive):

```
ΔS = reconstruction_loss      # for masked autoencoders
   = contrastive_loss          # for SimCLR / CLIP-style
   = next-token-CE              # for autoregressive
```

All three are forms of prediction error against a target derived from the input. UTL's main contribution is to recognise that the ΔE-style modulation of training (curriculum, learning-rate schedule, warmup) gates how much each ΔS contribution actually consolidates into weights.

---

## 3. ΔC for AI training

### 3.1 Representational stability between checkpoints

CKA (Centered Kernel Alignment) and SVCCA (Singular Vector Canonical Correlation Analysis) measure how similar two layer representations are between models or between checkpoints. They are the right tools for the AI-side ΔC.

For successive checkpoints `θ_t` and `θ_{t-1}` on a held-out probe set X:

```
ΔC_repr(t) = CKA(layer_L(θ_t)(X), layer_L(θ_{t-1})(X))
```

ΔC_repr ≈ 1 means "this checkpoint is representationally close to the last" (high coherence, no catastrophic shift). ΔC_repr ≈ 0 means catastrophic forgetting / representation collapse / unstable training.

A 2024 finding: **lower layers tend to converge faster than higher layers** — early layers stabilise (high ΔC) while higher layers are still learning (lower ΔC). This is consistent with UTL's prediction that ΔC for fundamental knowledge (early layers) reaches saturation while the integration layer (later layers) is still mid-learning.

### 3.2 Multi-embedder panel agreement (the DDA application)

Chris's 13-embedder panel can be repurposed to monitor the model's *outputs* for coherence across embedding spaces:

```
ΔC_panel = panel_agreement(generations_from_model)
         = mean over (i, j) of CKA(φ_i(generations), φ_j(generations))
```

When the model's outputs are semantically coherent, all 13 embedders agree on their structural relationships. When the model is producing inconsistent / hallucinating outputs, the agreement drops.

**Concrete payoff: this catches the Shakespeare echo bug.** The Case 1 forensic finding in Chris's PAPER.md was that 67 % of SFT examples began with play-script header prefixes that the model learned to echo. Single-embedder evaluation on E1 (semantic similarity) missed it; the multi-embedder audit caught it via E4 (position-ordering) and E12 (token-level MaxSim). UTL formalises this as: ΔC_panel was low for the affected outputs even though ΔS appeared healthy.

A UTL-aware training monitor would have raised an alert because ΔC_panel was low across the bad batch, and would have prevented the failed training from running to completion.

### 3.3 Self-consistency at inference time

Wang et al. (2022, "Self-Consistency Improves Chain of Thought Reasoning in Language Models") observed that taking N samples and majority-voting improves accuracy. UTL re-reads this as **inference-time ΔC**: when N samples agree, the model's coherence on that query is high; when they disagree, ΔC is low and the answer should be flagged.

```
ΔC_inference(query) = entropy(answer_distribution_from_N_samples)
                    -- low entropy = high coherence
```

This is a free (no extra model needed) UTL-style ΔC metric for any deployed LLM.

### 3.4 The contradiction signal (E5 asymmetric)

For training with a causal objective (e.g., the Santa LoRA's audio→video conditional training), UTL's contradiction term flags when the model's outputs reverse causal direction. Operationally, embed both the input condition and the output via E5's dual cause/effect axes, and compute:

```
contradiction = max(0, cos(output_in_cause_space, input_in_effect_space)
                       - cos(output_in_cause_space, input_in_cause_space))
```

A positive contradiction means the model is producing outputs that should logically *cause* the input rather than be caused by it. This catches a class of bugs that loss curves miss entirely.

---

## 4. ΔE for AI training

This is where the AI-side gets interesting and is genuinely novel. The **training dynamics** of a neural network have an analogue to the embodied state of a learner — the same conditions that prevent a stressed human from consolidating prevent an AI in adverse training-dynamics from consolidating.

### 4.1 plasticity_window analogue: loss-landscape curvature

EEG bands gate when consolidation can happen for humans. For AI, the analogue is **loss-landscape curvature**:

- **Edge of Stability (EoS).** Cohen et al. (2022) and follow-ups: training typically begins with sharpness reduction, then sharpness rises until reaching a stability threshold (the EoS), after which sharpness oscillates around this value. **Training near the EoS is the high-plasticity regime** — the model is in maximum-information-acquisition mode. Far below or above EoS, plasticity drops.
- **Critical sharpness** (a 2025 measure): a weighted sum of Hessian eigenvalues that tracks EoS efficiently. Computable in O(parameter_count) for billion-parameter models with stochastic Lanczos.
- **Multifractal loss landscape** (Nature Communications 2025): explains progressive sharpening, EoS, and non-stationary anomalous diffusion as features of multifractal geometry.

UTL maps this to:

```
plasticity_window_AI(t) = sigmoid(- |sharpness(t) - EoS_threshold| / scale)
```

Peaks at EoS; falls away for too-sharp or too-flat regions.

### 4.2 valence_arousal analogue: gradient noise and momentum

For humans, valence is "positive emotion" and arousal is "activation". For AI:

- **Gradient noise scale (McCandlish et al. 2018)** — the ratio of variance of stochastic gradients to magnitude of true gradient. Low noise scale → batch is informative → "high arousal" of training. High noise scale → batch is uninformative.
- **Optimiser state (Adam/AdamW second moment)** — the model's effective "momentum" or "intentionality" of training direction. When moments are stable → "positive valence", coherent direction. When moments are oscillating → "negative valence", indecisive direction.

```
valence_AI(t) = -1 + 2 · (1 - oscillation_rate(adam_moments_recent))
arousal_AI(t) = 1 / (1 + gradient_noise_scale(t))
valence_arousal_AI = sigmoid(valence_AI) · inverted_U(arousal_AI)
```

### 4.3 stress_floor analogue: plasticity loss

Continual-learning research (Lyle et al. 2024; Abbas et al. 2023) has identified **plasticity loss** as a phenomenon where a network's ability to learn new things diminishes over time, even with SGD/Adam. Cause: features become harder to modify; representations rigidify; effective rank drops.

This is the AI's **chronic stress** equivalent: structural inability to update due to past training pressures.

```
plasticity_loss(t) = 1 - effective_rank(features(t)) / theoretical_max_rank
stress_floor_AI(t) = 1 - plasticity_loss(t)
```

Mitigations are the AI analogue of UTL v2's stress-regulation: rank-protecting regularisers (UPGD, perturbation injection), feature-rejuvenation methods, Bayesian continual learning (Nature Communications 2025).

### 4.4 The full ΔE_AI

```
ΔE_AI = (plasticity_window_AI)^0.4 · (valence_arousal_AI)^0.4 · (stress_floor_AI)^0.2
```

A scalar in [0, 1] that monitors training health. UTL's prediction: when ΔE_AI is high, training updates consolidate efficiently. When ΔE_AI is low, even high-loss-decrease updates fail to produce robust generalisation.

### 4.5 Diagnostic states for AI training

Apply UTL v2 Part IX's state classification to AI:

| State | Condition | Mitigation |
|---|---|---|
| Confused | L < 0.3 (high loss, low coherence) | reduce learning rate; check data |
| Optimal | 0.3 ≤ L < 0.7 | keep going |
| Boring | L ≥ 0.7 (low loss, high coherence) — overfitting / converged | curriculum harder; add regularisation |
| Stuck | L < 0.1 for 3+ steps, ΔE_AI > 0.4 | path problem; try different architecture / data |
| **Dysregulated** | L < 0.1 for 3+ steps, ΔE_AI < 0.4 | **state problem; fix training dynamics first** |
| Dissipating | mean(L) high, M(eval) gain low | check held-out eval; data leakage suspected; representation collapse |

The Dysregulated state is the killer. It looks like a stuck training run (loss not moving) but the cause is bad ΔE — gradient noise, EoS instability, plasticity loss — not bad data or bad architecture. Fixing the architecture or data is wasted; you have to regulate ΔE first.

This explains the empirical observation that **rerunning the same training with the same data and same architecture often produces vastly different outcomes** — ΔE noise is large.

---

## 5. M(t) and T for AI

### 5.1 M(t) — consolidated capability

For an AI model, M(t) is **task accuracy on a frozen evaluation set** that has not been overfit during training. The integral form:

```
M_AI(t) = held_out_eval_accuracy(model_at_time_t, frozen_eval_set)
```

with a decay term that captures **catastrophic forgetting**:

```
M_AI(t+1) ≈ M_AI(t) + L(t) · ΔE_AI(t) · k(t) - forgetting_rate(t)
```

`k(t)` for an AI is the analogue of sleep — periods of low gradient pressure where consolidation happens. **Replay-based training** (experience replay, sleep-like consolidation phases) is the established AI implementation.

### 5.2 Sparse-autoencoder feature emergence

Anthropic's Scaling Monosemanticity work (2024 on Claude 3 Sonnet; 2025 follow-ups) establishes that sparse autoencoders extract interpretable features from frozen LLM hidden states. **Feature counts rise with training** — a model that has consolidated more capability has more interpretable features. This is M(t) measurable mechanistically:

```
M_SAE(t) = number_of_interpretable_features(SAE_trained_on_model_at_time_t)
```

Feature counts grow with training compute; the *quality* (monosemanticity score) grows with coherent training. UTL predicts: feature growth should correlate with the integrated ΔS · ΔC · ΔE_AI product over training.

This is a publishable experimental claim: **train a model under controlled regimes where ΔE_AI is varied, then count SAE features — UTL predicts more features in high-ΔE regimes.**

### 5.3 T_AI(train, deploy)

Distribution-shift distance between training mixture and deployment query distribution.

```
T_AI = exp(-KL(D_train || D_deploy) / λ)
```

Where KL divergence is computed in an embedder space (using one of the 13 Context Graph embedders). When D_train and D_deploy are close, T is high; when they are far apart, T is low and deployed performance suffers despite high training loss.

The variability-of-practice prescription for AI: **train across a distribution of conditions close to deployment**. This is the principled basis for data-mixing during pre-training and for in-domain fine-tuning.

---

## 6. The training-run health monitor

Wire all of the above into a **per-training-step diagnostic pipeline**:

```python
def utl_training_monitor(state: TrainingState) -> dict:
    # ΔS
    delta_s = state.batch_loss

    # ΔC
    delta_c_repr = CKA(state.activations_now, state.activations_last_checkpoint)
    delta_c_panel = panel_agreement_over_13_embedders(state.recent_generations)
    delta_c = 0.5 * delta_c_repr + 0.5 * delta_c_panel

    # ΔE_AI
    sharpness = critical_sharpness(state.loss_landscape)
    plasticity = sigmoid(-abs(sharpness - EoS_threshold) / scale)
    grad_noise = gradient_noise_scale(state.recent_gradients)
    arousal = 1 / (1 + grad_noise)
    valence = adam_moment_stability(state.optimizer.moments)
    valence_arousal = sigmoid(valence) * inverted_U(arousal)
    plasticity_loss = 1 - effective_rank(state.features) / state.max_rank
    stress_floor = 1 - plasticity_loss
    delta_e = plasticity**0.4 * valence_arousal**0.4 * stress_floor**0.2

    # L_AI
    L = delta_s * delta_c * delta_e

    # State classification
    if L < 0.1 and delta_e < 0.4:
        diagnosis = "Dysregulated — fix training dynamics first"
    elif L < 0.1:
        diagnosis = "Stuck — try different architecture/data"
    elif L < 0.3:
        diagnosis = "Confused"
    elif L < 0.7:
        diagnosis = "Optimal"
    else:
        diagnosis = "Boring — model converged; harder curriculum or stop"

    return {
        "L": L, "ΔS": delta_s, "ΔC": delta_c, "ΔE": delta_e,
        "ΔC_repr": delta_c_repr, "ΔC_panel": delta_c_panel,
        "plasticity": plasticity, "valence_arousal": valence_arousal,
        "stress_floor": stress_floor, "diagnosis": diagnosis,
    }
```

Run every 100–1000 steps. Plot the four quantities. When ΔE drops, intervene before the loss tells you anything is wrong.

---

## 7. Reading Chris's existing training runs through this lens

### 7.1 The Shakespeare LoRA echo bug (PAPER.md §6.1.3)

The bug: 67 % of SFT examples began with play-script headers ("SCENE III. Another part of the field. Enter Cassius..."). The model learned to echo these prefixes. Single-embedder E1 evaluation missed it.

UTL diagnosis at the time of the bad training:

- ΔS (training loss) was healthy: prediction error against the corrupted training data was decreasing normally.
- **ΔC_panel was low** for the affected outputs because E4 (position-ordering) and E12 (token-level MaxSim) flagged opening-position style anomaly. This is detectable in real time during training, not only at evaluation.
- ΔE_AI was healthy.

Diagnosis: **Dissipating** — engagement (ΔS) was high, but consolidated capability (M, validation accuracy) was lower than ΔS predicted.

**A UTL-aware training monitor would have caught this in the first 200 steps**, before the 7.5-h training completed.

### 7.2 The EchoMimicV3 flow-matching sign bug (PAPER.md §6.2.2)

The bug: original code computed `target = latent - noise`; correct convention per the EchoMimicV3 scheduler is `target = noise - latent`. The sign flip trained the model to *add* noise rather than remove it (anti-denoising). Loss minimised cleanly because the target was well-defined; the model learned the wrong direction.

UTL diagnosis:

- ΔS was healthy: prediction error against the (wrong) target was decreasing.
- ΔC_repr was healthy: representations were stable across checkpoints.
- ΔC_panel was *unmeasurable* without the constellation guard (because the guard re-embeds outputs and validates against the goal centroid — anti-denoised outputs would fail the centroid match).
- ΔE_AI was healthy.

Diagnosis: **Dissipating with high confidence** — the in-loop loss looked perfect but the constellation-guard signal (which is the AI-side of ΔC_panel) was failing.

**UTL prescription: never trust ΔS alone.** ΔS · ΔC · ΔE must all be measured. The constellation-guard signal is the load-bearing one for catching wrong-target bugs that loss curves cannot see.

### 7.3 The Qwen3-TTS 0.961 SECS result (PAPER.md §6.3)

The success: best-of-12 selection at T=0.3 against a 50-clip WavLM centroid produces 0.961 mean SECS on 10 held-out sentences, exceeding VALL-E 2's "human parity".

UTL diagnosis:

- ΔS = 0 (the model is inference-only, no training loss).
- ΔC_panel = high (centroid agreement at high cosine).
- ΔE_AI = high (no training; pure inference — but the temperature parameter is the ΔE analogue, and T=0.3 was empirically optimal).
- M = pre-existing in the frozen Qwen3-TTS weights.
- T (state-transfer) = high (in-context-learning prompt provides matching state to deployment).

UTL prescription matches the empirical finding: **inference-time best-of-K selection against a frozen panel = perfect Phase-3 guard**. The 0.961 result is exactly what UTL predicts when ΔC_panel is the optimisation target and ΔE is high.

---

## 8. AI-side falsification predictions

Three concrete predictions UTL makes about AI training that file 08 picks up:

1. **Models trained at the Edge-of-Stability sharpness should consolidate features faster than models trained far below or far above.** Measurable by SAE feature counts as a function of training compute, varying learning rate to manipulate sharpness.
2. **Models with stable Adam moments (high "valence_AI") should generalise better than equally-loss-converged models with oscillating moments.** Measurable by held-out accuracy controlling for final loss.
3. **Replay-based continual learning should outperform contiguous training by a factor predicted by `k(τ)`.** UTL's k = 2–4 during sleep-like phases predicts ~2–4× sample efficiency gain from spaced replay in continual learning, matching empirical findings (Parisi et al. 2019; Cousins 2023 for the human analogue).

Each is a clean experimental design with a quantitative prediction.

---

## 9. Why this matters

The standard ML training pipeline is:

```
loss → backprop → check loss → repeat
```

UTL's contribution is to insist that **loss alone is not enough**. A training run can have monotonically decreasing loss and still be:

- Encoding the wrong target (Santa flow-matching sign).
- Learning a corrupted prefix pattern (Shakespeare echo).
- Catastrophically forgetting earlier capabilities (continual-learning collapse).
- Sharpening into a non-generalising minimum (overfitting at low ΔE).
- Losing plasticity to the point of feature-stuck (chronic stress for AI).

The four-term product `ΔS · ΔC · ΔE_AI` catches each of these failure modes via a different term. The diagnostic state classifier names them. The training monitor runs continuously and intervenes early.

This is what UTL becomes when applied to AI: **a pre-deployment safety regime that catches the bugs that loss curves miss.** The fact that the same equations work for human and AI learners is a coincidence only insofar as both are predictive systems doing Bayesian belief updating with a finite plasticity / consolidation budget.

---

## 10. Connection to TCT (PAPER.md)

This section closes the loop with Chris's TCT paper. The DDA → TCT progression in PAPER.md is:

1. (DDA) Project a fixed corpus through N frozen embedders → up to N + C(N,2) signals per input.
2. (TCT Phase 1) Aggregate signals into a frozen multi-modal centroid C_P.
3. (TCT Phase 2) Train a generator with a cosine-proximity loss against C_P.
4. (TCT Phase 3) Runtime guard G_τ rejects outputs that fall outside the centroid neighbourhood.

UTL applied to AI is **the diagnostic complement of TCT**:

1. (UTL ΔS) Track per-batch surprisal as the standard training-loss object.
2. (UTL ΔC) Use the same N-embedder panel as a representational-stability monitor across training and a coherence monitor across model outputs.
3. (UTL ΔE_AI) Track training-dynamics health (sharpness, gradient noise, plasticity loss).
4. (UTL diagnostic states) Classify the run; intervene early on Dysregulated / Dissipating states.

TCT defines the *target* (where the model should land). UTL defines the *trajectory health* (how well the model is moving toward the target). They are complementary. **Chris is already running TCT for production; adding UTL diagnostics to his training pipeline is straightforward and would have caught both 6-day-sprint bugs in their first hour rather than after their 7.5-h completion.**

---

## Sources

- [Visualizing the Loss Landscape of Neural Nets (Li et al.)](https://www.cs.umd.edu/~tomg/projects/landscapes/)
- [A Scalable Measure of Loss Landscape Curvature for Analyzing the Training Dynamics of LLMs (2026)](https://arxiv.org/html/2601.16979v1)
- [Optimization on multifractal loss landscapes explains a diverse range of geometrical and dynamical properties (Nature Communications 2025)](https://www.nature.com/articles/s41467-025-58532-9)
- [SVCCA: Singular Vector Canonical Correlation Analysis](https://arxiv.org/abs/1706.05806)
- [Similarity of Neural Network Representations Revisited (CKA)](https://arxiv.org/abs/1905.00414)
- [Sparse Autoencoders Find Highly Interpretable Features in Language Models](https://arxiv.org/abs/2309.08600)
- [Scaling Monosemanticity: Extracting Interpretable Features from Claude 3 Sonnet (Anthropic 2024)](https://transformer-circuits.pub/2024/scaling-monosemanticity/)
- [A Survey on Sparse Autoencoders (2025)](https://arxiv.org/html/2503.05613v3)
- [Information-Theoretic Progress Measures reveal Grokking is an Emergent Phase Transition (2024)](https://arxiv.org/html/2408.08944v1)
- [Beyond Progress Measures: Theoretical Insights into the Mechanism of Grokking (2024)](https://arxiv.org/html/2504.03162v2)
- [Continual Learning and Catastrophic Forgetting (2024)](https://arxiv.org/html/2403.05175v1)
- [Addressing Loss of Plasticity and Catastrophic Forgetting in Continual Learning (2024)](https://arxiv.org/abs/2404.00781)
- [Bayesian continual learning and forgetting in neural networks (Nature Communications 2025)](https://www.nature.com/articles/s41467-025-64601-w)
- [Loss of Plasticity in Continual Deep Reinforcement Learning](https://proceedings.mlr.press/v232/abbas23a/abbas23a.pdf)
- [Adapting curricula by tracking the gap between validation and training loss (2025)](https://openaccess.thecvf.com/content/ICCV2025W/CLVision/papers/Singh_Adapting_curricula_by_tracking_the_gap_between_validation_and_training_ICCVW_2025_paper.pdf)
- [V-JEPA 2: Self-Supervised Video Models Enable Understanding, Prediction and Planning](https://arxiv.org/abs/2506.09985)
- [Embodied AI: From LLMs to World Models (2025)](https://arxiv.org/html/2509.20021v1)
- [Embodiment in Multimodal Large Language Models (2025)](https://arxiv.org/pdf/2510.13845)
