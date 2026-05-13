---
namespace: decisions
agent_id: agent-document-architect
created: 2026-05-09T00:00:00Z
updated: 2026-05-09T01:00:00Z
status: superseded
severity: high
tags: directive, mejepa, learning-formula, utl-v2, superseded-by-utml-v1, self-healing, self-learning, reward-signal
supersedes: none
superseded_by: 2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md
---

# [SUPERSEDED — see `2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md`]

> The 3-term v2.0 formula `L = ΔS × ΔC × ΔE` (mapped from human-physiology UTL) has been
> **superseded by the 4-term UTML v1.0 — JEPA Edition** `L_step = ΔP × ΔK × ΔΩ × ΔΞ`
> (machine-native, with the new representational-non-collapse term ΔΞ that biology cannot
> have). The operator updated `docs2/UTL_v2_embodied.md` on 2026-05-09 to be the JEPA
> Edition, and the formal mapping is now in the new decision file linked above. **Future
> agents should use UTML v1.0, not this document.**
>
> Why superseded: machines have no autonomic nervous system, so the v2.0 mapping of
> "embodiment" to AI/ML signals was approximate. More importantly, neural systems can
> **collapse** representationally (target encoder maps every input to the same point;
> loss reaches zero; nothing learned) — a structural failure mode no biological learner
> has. The 4-term formula adds ΔΞ as a multiplicative gate. ME-JEPA is uniquely structurally
> collapse-immune on the target side because its targets come from **frozen externally-
> defined instruments** rather than from a learnable EMA encoder.

---

# UTL v2.0 → ME-JEPA learning formula: `L = ΔS × ΔC × ΔE` [HISTORICAL]

## Operator directive (2026-05-09)

> "look at the `/home/cabdru/contextgraph/docs2/UTL_v2_embodied.md` you can use
> this to formulate a learning formula for the ME JEPA system."

The Unified Theory of Learning v2.0 (peer-reviewed-grounded human-learning
framework) is **adopted as the canonical learning formula for ME-JEPA**, with
each human-physiological term mapped to a measurable AI/ML signal. The
multiplicative gating preserves UTL's "any factor at zero zeroes the whole
product" property — critical for self-healing, because it forces the system to
**not learn** when its own internal state is corrupted.

## The mapped formula (canonical)

```
L_ME-JEPA(t) = ΔS(t) × ΔC(t) × ΔE(t)        all factors clamped to [0, 1]
```

| Symbol | UTL v2.0 meaning | ME-JEPA mapping |
|---|---|---|
| **L** | Per-moment learning efficiency | Per-step training-signal strength: how much this `(panel, oracle_verdict)` pair should move the model right now |
| **ΔS** | Surprise: prediction error | `\|predicted_oracle − actual_oracle\|` (gold) OR `γ · \|predicted_oracle − simulated_oracle\|` (counterfactual rollouts via panel perturbation) |
| **ΔC** | Coherence: integrates with prior knowledge | Composite of (a) prediction stability across recent batches, (b) gradient-direction agreement, (c) **multi-embedder coherence** (E1-E13 cross-space neighbor agreement) |
| **ΔE** | Embodiment: state permits consolidation | Composite of (a) plasticity-window — current learning-rate × training-mode, (b) signal clarity — gold-vs-weak supervision SNR, (c) drift-floor — `1 − concept_drift_z` |

### Detailed forms

**ΔS — surprise (prediction error vs durable reality):**

```
ΔS = min(1.0,
         max(|oracle_predicted − oracle_actual|,
             γ · |oracle_predicted − oracle_simulated|)
         + 0.5 · exploration_rate)
```

- `oracle_actual` from the Docker oracle = **gold supervision** (full weight).
- `oracle_simulated` from latent perturbation of `panel[t=1]` = **counterfactual
  supervision** (γ ≈ 0.7, the v2.0 mental-rehearsal coefficient). Avoids
  double-counting when the same patch is rolled out and observed.
- `exploration_rate` = fraction of recent batches that are out-of-distribution
  (high entropy across cluster centroids).

**ΔC — coherence (does the new observation integrate?):**

```
ΔC = min(1.0,
         1/(1 + CV_recent_predictions)
         + 0.3 · gradient_effectiveness
         + 0.2 · embedder_coherence)
```

- `CV_recent_predictions` = `σ/μ` over the last 100 prediction confidences.
  Low CV → the model is in a stable regime and new observations integrate.
- `gradient_effectiveness` = cosine of recent step's gradient with the EMA
  gradient direction. High = aligned with the model's learning trajectory.
- `embedder_coherence` = how stable the multi-embedder (E1-E13) neighbor sets
  are across the 13 spaces. Drift in one embedder while others stay still =
  loss of coherence. **This uses the existing 13-embedder system in
  `crates/context-graph-embeddings/`.** No new infrastructure needed.

**ΔE — embodiment (model state permits the update to stick?):**

```
ΔE = (plasticity_window)^0.4 × (signal_clarity)^0.4 × (drift_floor)^0.2
```

- `plasticity_window ∈ [0, 1]`:
  - 0.0 → frozen (inference-only mode)
  - 0.2 → high-beta-equivalent: anomaly-disabled high LR (anxious/locked)
  - 0.6 → standard online fine-tune (alert/focused)
  - 0.85 → optimal mode: cosine-decayed LR + EWC++ active (alpha)
  - 1.0 → scheduled offline retrain window (theta — peak consolidation)
- `signal_clarity ∈ [0, 1]`:
  - 1.0 → gold oracle verdict
  - 0.6 → operator override / annotated label
  - 0.3 → heuristic / aux-head agreement
  - 0.1 → weak / noisy
- `drift_floor ∈ [0, 1]`:
  - 1.0 → in-distribution (z-score of current batch < 0.5σ from training)
  - 0.8 → mild drift (0.5–1σ)
  - 0.4 → moderate drift (1–2σ) — **trigger A/B/C retrain**
  - 0.1 → severe drift (>2σ) — **refuse predictions, full retrain**

Geometric weighting: any subcomponent at 0 zeros ΔE. **A model in severe drift
(drift_floor → 0) should NOT update online** — that's catastrophic forgetting
waiting to happen. Same for frozen mode (plasticity_window = 0) and pure-noise
batches (signal_clarity = 0).

### Consolidation integral M(t)

```
M_ME-JEPA(t) = ∫₀ᵗ L(τ) · ΔE(τ) · k(τ) · e^(-(t-τ)/τ_decay(τ)) dτ
```

- `k(τ)` = consolidation multiplier:
  - 1.0 during **online observe()** updates (the live shift-log subscriber, doc 09 §3.8).
  - 3-5× during **scheduled offline retrain** windows (analog of slow-wave sleep).
- `τ_decay(τ)`:
  - Hours for unconsolidated traces (recent online updates not yet seen by retrain).
  - Months/years for retrain-consolidated weights protected by EWC++.

The `ΔE(τ)` factor inside the integral matters: high-L moments encoded in a
dysregulated state (low ΔE) **dissipate** rather than consolidate. The system
must filter for high-ΔE encoding moments.

### State-transfer T (cross-project / cross-language deployment)

```
T(SWE-bench-Lite, target_project) = exp(-d(state_train, state_deploy) / λ)
performance_deploy = M_ME-JEPA(t) · T(SWE-bench-Lite, target_project)
```

- `d(·, ·)` = distance over the joint deployment-state vector:
  `(language, repo_size, test_density, dependency_graph_depth, coverage_pattern)`.
- `λ ≈ 1.5` default. Tighter for high-stakes (production code, OSS libraries).
  Looser for prototypes / scratch repos.

**Implication**: training only on SWE-bench Lite Python predicts well on Python
repos and gradually less well as language / size / structure diverge. Cross-
language coverage requires capturing attempts in Rust / C / C++ / Java / C# /
JS / TS / Ruby (the supported languages per the `09 §3.1` AST harness state).
**Stress-inoculation training** = train across realistic deployment-state
distribution from day one, not Python-only.

### Future-self prior (bounded extrapolation)

```
predicted_t = (1 − β_t) · f(history_t) + β_t · capability_target_vector
β_t ∈ [0.05, 0.20]
β_{t+1} = clamp(β_t · (1 − η · sign(|err_future| − threshold)), 0.05, 0.20)
```

- `capability_target_vector` = a stable embedding of "ME-JEPA correctly
  predicts reality on any dynamic project, any language" (the end goal).
- β decays adaptively when prediction error against the target stays large
  (the target is too far). Rises when error shrinks (the model is converging
  on the goal).

The bound is critical: β too large = wishful prediction divorced from history.

### Reward (for the optimizer that tunes ME-JEPA's data factory)

```
Reward = 0.6 · prediction_oracle_correlation + 0.4 · mean(L · ΔE)
```

The optimizer (Claude Code) maximizes this reward by improving the data
factory (instruments, hooks, shift-log capture, witness chain) so ME-JEPA gets
richer L per attempt at higher ΔE.

### Learning states (UTL v2.0 § XII, mapped)

| Pattern | Learning state | Action |
|---|---|---|
| L < 0.1 for 3+ steps, ΔE > 0.4 | **Stuck** (path problem) | Prune or change architecture |
| L < 0.1 for 3+ steps, ΔE < 0.4 | **Dysregulated** (state problem) | Regulate first: pause online updates, run drift detection, retrain if needed |
| L < 0.3 | **Confused** | Reduce challenge / improve coverage |
| 0.3 ≤ L < 0.7 | **Optimal** | Continue |
| L ≥ 0.7 | **Mastery / Boring** | Increase challenge (harder mutation categories, harder benchmarks) |
| mean(L) > 0.3 but ΔM(24h)/mean(L) < 0.2 | **Dissipating** (consolidation problem) | Increase k(τ): trigger offline retrain window |

**Coaching order for ME-JEPA self-tuning**: regulate ΔE first, then adjust
ΔS / ΔC. If ΔE is low, increasing the training challenge wastes signal.

## Self-healing protocol (UTL v2.0 §VIII coaching cycle)

```
1. ASSESS STATE: compute current ΔE_model.
   ↓
2. REGULATE if ΔE < 0.4 (state intervention before content):
   - Drift severe? → pause inference, trigger A/B/C retrain (doc 09 §5.3).
   - Plasticity collapsed (LR too low or grad explosion)? → reset LR schedule.
   - Signal clarity too low? → reweight batch toward gold-supervised examples.
   ↓
3. OBSERVE: ΔS (oracle prediction error), ΔC (multi-embedder agreement).
   ↓
4. COMPUTE: L = ΔS × ΔC × ΔE.
   ↓
5. CLASSIFY: which learning state is the model in?
   ↓
6. INTERVENE: adjust either CONTENT (more data, harder mutations) or STATE
   (drift correction, retrain) depending on which signal is low.
   ↓
7. EVALUATE: did L improve? Did M (consolidation) hold over the 24h window?
   ↓ (return to step 1)
```

## Pruning rule

```
prune learning path if (L < 0.1 for 3+ steps) AND (ΔE > 0.4 throughout)
```

If ΔE was low, the diagnosis is **bad model state, not bad data path**.
Pruning a path because the model was dysregulated when it saw it teaches the
wrong lesson.

## Confidence calibration (gated by ΔE_mean)

```
calibrated_confidence = raw_confidence
                       · prediction_oracle_correlation
                       · gate_agreement_factor
                       · evidence_factor
                       · ΔE_mean
clamped to [0.10, 0.95]
```

Confidence formed in a dysregulated model state should not be trusted to the
same degree as confidence formed when the model is healthy.

## What this means for the existing infrastructure

The 13-embedder system in `crates/context-graph-embeddings/` (E1-E13, with
asymmetric E5 / E8 / E10) is the **substrate for ΔC**. No new embedding code
needed; ME-JEPA uses what's there.

The witness chain in `crates/context-graph-witness/` records every L step
deterministically — every training certificate is auditable. Append-only chain
is the audit substrate for "did this gradient update happen, and was the model
in a healthy state when it did?".

The reality-shift principle (`docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`)
is the **continuous source of ΔS**. Every shift = potential gold supervision
when the oracle eventually verdicts.

The mutation corpus (`crates/context-graph-mejepa-corpus/`) is the **bootstrap
ΔS distribution**. Each of the 8 mutation categories teaches a different shape
of prediction error.

## Related

- `docs2/UTL_v2_embodied.md` — the full UTL v2.0 framework with peer-reviewed
  citations.
- `decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`
  — the companion directive that scopes when this formula applies.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §5 (self-healing
  operations) — to be updated to use this formula explicitly.

## Falsification (UTL v2.0 §XI.6 discipline)

This formula is FALSIFIABLE. Any of these would invalidate it:

1. **Drift_floor proxies (KL divergence, embedding shift) uncorrelated with
   ME-JEPA prediction degradation**, holding ΔS and ΔC constant. (If true,
   ΔE is wrong as conceptualized — drop the drift_floor term.)
2. **Counterfactual / latent perturbation rollouts producing zero gradient
   benefit** in a well-controlled comparison vs gold-only training. (If true,
   the γ ≈ 0.7 mental-rehearsal coefficient is misapplied.)
3. **Implementation-intention / capability_target_vector having zero effect**
   on convergence rate compared to history-only prediction in a
   well-controlled study. (If true, drop the future-self prior.)

If any of these falsifications hold, this decision file gets `status: superseded`
and a new file describes the corrected formula.

## Tags

`directive` `mejepa` `learning-formula` `utl-v2` `self-healing` `self-learning`
`reward-signal` `multiplicative-gating` `drift-detection` `cross-language`
`state-transfer` `bounded-extrapolation` `falsifiable`
