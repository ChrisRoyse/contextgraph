# 11 — ME-JEPA Learning Formula (UTL-v2 Adaptation)

> **Operator directive 2026-05-09:** *"look at the `/home/cabdru/contextgraph/docs2/UTL_v2_embodied.md` you can use this to formulate a learning formula for the ME JEPA system."*

> **Companion documents:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan (panel + predictor + EMA + VICReg + verify)
> - `10_MEJEPA_PIVOT_2026_05_09.md` — post-pivot architecture (no inner LLM)
> - `docs2/UTL_v2_embodied.md` — the source learning theory (peer-reviewed neuroscience)

---

## 0. Why this doc exists

ME-JEPA-Code is being trained on a heterogeneous reward signal: the deterministic mutation corpus (Phase 0), live outer-loop observations (Phase 5), synthetic adversarial perturbations (Phase 0 §3.3), and operator overrides (Phase 4 active-learning). Each example carries a different *quality* of training signal — some examples are noisy artifacts, some are gold-standard supervision, some are in-distribution and some are far OOD.

Doc 09 §3.7 specifies a multi-objective auxiliary loss (`L_full = L_predict + λ_var L_var + λ_inv L_inv + λ_cov L_cov + α_warn L_warn + ...`) but assumes uniform weighting per example. **In practice, examples differ in informativeness and consolidatability.** UTL-v2's `L = ΔS × ΔC × ΔE` formula is the rigorously-derived weighting we adopt: each ME-JEPA training example is weighted by the product of three multiplicative factors, all clamped to [0, 1]. Examples low in any factor contribute nothing; examples high in all three drive durable improvement.

This doc operationalizes that formula end-to-end for ME-JEPA-Code training, online observe, and active learning.

---

## 1. The formula

```
L_mejepa(panel, oracle) = ΔS × ΔC × ΔE         (each ∈ [0, 1])
```

| Symbol | Cognitive analog (UTL-v2) | ME-JEPA-Code operationalization |
|---|---|---|
| **ΔS** (Surprise) | "Something unexpected happens" | Per-example prediction error: how distant is the predictor's output from the EMA target encoder's representation of the actual `panel[t=2]`? |
| **ΔC** (Coherence) | "The unexpected thing makes sense" | How well the example integrates with the prior corpus structure (latent-space coherence) |
| **ΔE** (Embodiment) | "The body is in a state where the update can stick" | Whether the model is in a state where the gradient update will consolidate (training-stability metrics) |

Multiplication preserves the UTL-v2 invariant: any factor at zero zeros the entire weight. A wildly out-of-distribution example (low ΔC) contributes nothing even if it's surprising. An example that the model is gradient-exploding on (low ΔE) contributes nothing even if it's a perfect supervision signal. An example the model already nails (low ΔS) is also noise — perfect predictions don't teach.

---

## 2. ΔS — Surprise

```
ΔS_mejepa = min(1.0,
    max( 1 − cosine(g_φ(f_θ(panel_t01)), f_ξ(panel_t2)),
         γ · |g_φ_simulated − f_ξ(panel_t2)| )
    + 0.5 · exploration_rate )
```

Where:

- **`g_φ(f_θ(panel_t01))`** = predictor's output for the example's context panels (t=0 and t=1 concatenated).
- **`f_ξ(panel_t2)`** = EMA target encoder's representation of the actual post-execution panel.
- **`1 − cosine(...)`** is the natural prediction-error signal, clamped to [0, 1] (cosine is in [-1, 1]; `1 − cosine` is in [0, 2] — divide by 2 then clamp).
- **`γ ≈ 0.7`** is the rehearsal-fidelity discount (UTL §III).
- **`|g_φ_simulated − ...|`** is the prediction error from a counterfactual perturbation (§3.7.3 latent-space `panel + ε·v` perturbation). Counterfactual training is ME-JEPA's analog of motor-imagery in UTL.
- **`exploration_rate`** is the active-learning bonus: examples chosen because the conformal interval was wide get a +0.5 bonus.
- **`max(...)`** prevents double-counting when the predictor saw both the perturbation AND the actual.
- **`min(1.0, ...)`** preserves the range invariant.

**Implementation reference:** `crates/context-graph-mejepa/predictor.rs::predict()` returns the latent; `loss.rs::vicreg_loss()` already computes prediction error via Huber. Add a new `learning_signal::compute_delta_s()` function that wraps these.

**Range invariant:** if `ΔS > 1.0` after computation, the formula is wrong — investigate. If `ΔS = 0`, the predictor already nails this example; mark as low-priority for sampling and move on.

---

## 3. ΔC — Coherence

```
ΔC_mejepa = min(1.0,
    1 / (1 + CV_recent_loss)
    + 0.3 · gradient_effectiveness
    + 0.2 · latent_cluster_coherence )
```

Where:

- **`CV_recent_loss`** = coefficient of variation of the predictor's loss across the last N=100 batches. Low CV = stable training = high coherence.
- **`gradient_effectiveness`** = ratio of `(prev_loss − current_loss) / |g|`. High = each unit of gradient norm is moving the loss down.
- **`latent_cluster_coherence`** = the example's latent distance to its nearest existing-corpus cluster centroid, normalized to [0, 1]. Examples that fall NEAR an existing cluster are coherent; examples that fall in empty latent regions are incoherent (likely OOD or noise).

**Implementation reference:** k-means or Leiden community detection (per doc 05 §2.5) over the training-corpus latents produces cluster centroids. Each new example computes its distance to nearest centroid via `context-graph-solver`'s vector primitives.

**Edge case:** for the very first 100 examples (when there are no clusters yet), default `latent_cluster_coherence = 0.5` (neutral). After 1,000+ examples, the cluster topology is reliable and coherence becomes a meaningful gate.

**HRV-coherence analog:** UTL-v2's `0.2 · HRV_coherence` term operationalizes "internal-state coherence" of the learner (cardiovascular oscillator locked at 0.10 Hz). For ME-JEPA, the analog is **gradient-update coherence across the optimizer's recent step history** — measured by the cosine similarity of consecutive gradient steps. High cosine = the optimizer is converging coherently; low cosine = the optimizer is thrashing. The latent-cluster-coherence term captures this functionally.

---

## 4. ΔE — Embodiment (the new term)

UTL-v2 uses geometric weighting to enforce a hard-gate: any subcomponent at 0 zeros the whole product.

```
ΔE_mejepa = (plasticity_window)^0.4 · (drift_floor)^0.4 · (gradient_floor)^0.2
```

### 4.1 plasticity_window ∈ [0, 1]

The model's "encoding readiness." Mapped from UTL-v2's brainwave-state ladder (high beta → theta) onto training-state machinery:

| State | Meaning | `plasticity_window` |
|---|---|---|
| Mid-fine-tune cycle (gradient steps 1-50 of an EWC++ refresh) | model is actively rewriting weights — high noise, low encoding stability | 0.2 |
| Recent-fine-tune (50-200 steps post EWC++ trigger) | model is settling — moderate stability | 0.6 |
| Steady-state training (no recent EWC++ trigger; loss curves smooth) | optimal encoding regime | 0.85 |
| Periodic-full-retrain epoch boundary (between epochs N and N+1, k-multiplier window) | weight space integration; UTL's "delta sleep" analog | **1.0** for online observe; M(t) integral-only |

**Implementation reference:** `crates/context-graph-mejepa-train/scheduler.rs::current_state()` exposes which regime we're in. Compute `plasticity_window` as a function of `(steps_since_last_ewc_refresh, recent_loss_curvature, scheduler_phase)`.

### 4.2 drift_floor ∈ [0, 1]

UTL-v2's stress-floor analog. Maps to ME-JEPA's drift detector + conformal coverage:

```
drift_floor =
  1.0   if conformal coverage ∈ [0.88, 0.92]    (calibrated, healthy)
  0.8   if coverage ∈ [0.85, 0.88) ∪ (0.92, 0.95]   (mild drift)
  0.4   if coverage ∈ [0.80, 0.85) ∪ (0.95, 1.00]   (significant drift)
  0.1   if coverage < 0.80                          (severe drift; pause online observe)
  0.0   if rollback in progress                     (hard-gate; reject all updates)
```

**Implementation reference:** `crates/context-graph-mejepa/conformal.rs::current_coverage()` from Phase 4. Drift detector + rollback path (Phase 5) writes its state to `CF_MEJEPA_DRIFT_STATE`. Read both, take the lower bound.

### 4.3 gradient_floor ∈ [0, 1]

```
gradient_floor =
  1.0   if grad_norm < 1.0                  (clean gradients)
  0.5   if grad_norm ∈ [1.0, 5.0]           (acceptable)
  0.1   if grad_norm ∈ [5.0, 10.0]          (warning; clip more aggressively)
  0.0   if grad_norm > 10.0                 (gradient explosion — drop this example)
```

**Implementation reference:** AdamW step (Phase 3) emits `grad_norm` per step into the per-step training certificate.

---

## 5. Per-example sampling weight (Phase 3 + Phase 5 hot path)

```
sampling_weight(ex) = base_weight
                    × L_mejepa(ex)
                    × (1 + 5 · operator_override_flag)        // gold up-weight
                    × decay^age_in_days                        // recency-weighted
```

Where:

- **`base_weight = 1.0`** — uniform default.
- **`L_mejepa(ex) ∈ [0, 1]`** — the UTL-v2-derived weight from §1.
- **`operator_override_flag ∈ {0, 1}`** — flips on when an operator's manual review disagrees with ME-JEPA's verdict.
- **`decay = 0.995`** — half-life ~140 days for an example to fully decay if never re-touched.
- **`age_in_days`** — time since the example was added (or last validated against the live oracle).

Examples with `sampling_weight < 0.05` are dropped from the current epoch's sampling pool (probability ε of being sampled). Examples with `sampling_weight > 1.0` (gold-standard) are guaranteed inclusion in the next 100 batches.

This integrates UTL-v2's `L_mejepa` into the doc 09 §3.7.2 sampling reweighting mechanism. The `(1 + 2·residual_error_z_score)` term from doc 09 is REPLACED by `L_mejepa(ex)` since L_mejepa already captures hard-example mining via ΔS.

---

## 6. Consolidation integral M(t)

UTL-v2's M(t) maps to ME-JEPA's online-observe cumulative effect:

```
M(t) = ∫₀ᵗ L_mejepa(τ) · ΔE(τ) · k(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
```

| UTL-v2 | ME-JEPA |
|---|---|
| `k(τ)` ≈ 1.0 awake; ≈ 3-5× during slow-wave sleep | `k(τ) = 1.0` during online observe; `k(τ) = 4.0` during the periodic full retrain (§5.4) |
| `τ_decay` short for unconsolidated traces | `τ_decay = 7 days` for examples not yet retained in the EWC++ Fisher snapshot |
| `τ_decay` long for integrated memories | `τ_decay = 365 days` for examples bound into the Fisher matrix at high importance |

**Operational meaning:** an example seen ONCE during online observe contributes `L_mejepa × 1.0` to M. The same example, replayed during the periodic full retrain (every 1,000 attempts) and bound into the EWC++ Fisher snapshot, contributes `L_mejepa × 4.0` and persists for ~year-long decay.

**This is why "study right before sleep" outperforms "study and forget"** in UTL-v2 — and why ME-JEPA's periodic-full-retrain step (binding examples into Fisher at peak `k(τ)`) is mathematically equivalent to consolidating during slow-wave sleep.

---

## 7. State-transfer training (UTL-v2 §V.3 analog)

UTL-v2 §V.3 cites the surgeon-simulator-vs-OR transfer problem: a learner perfect in the low-stress simulator cannot transfer skill to the high-stress OR.

For ME-JEPA, the analog is **distribution-transfer**: ME-JEPA trained on the deterministic mutation corpus (Phase 0) may fail to generalize to live outer-loop observations (Phase 5) because the live-corpus distribution differs from the synthetic-corpus distribution.

UTL-v2's prescription: practice the skill at progressively elevated arousal so the encoded representation is robust to the OR's cortisol signature.

**ME-JEPA's prescription (Phase 5 §3.5):**

1. **Phase 3 base training** is on the static mutation corpus (low-stress / clean synthetic distribution).
2. **Phase 5 online observe** progressively introduces live-distribution examples, weighted by `L_mejepa` (§5).
3. **Active-learning loop** (§3.7.4) surfaces the uncertain in-distribution examples for explicit Docker oracle re-runs, building robustness in the high-uncertainty regime.
4. **Cross-task transfer** (§3.7.5) up-weights examples from similar tasks in the patch-similarity graph, mimicking domain-progressive training.

The result: ME-JEPA's verify() coverage on live patches (Phase 8 evaluation) approaches its mutation-corpus coverage, because the training distribution was progressively expanded toward reality.

---

## 8. Implementation roadmap

| Component | Where | Phase | Status |
|---|---|---|---|
| `compute_delta_s(panel, target_panel, conformal_state)` | `crates/context-graph-mejepa/learning_signal.rs::delta_s` | Phase 3 | deferred |
| `compute_delta_c(latent, cluster_centroids, recent_grad_history)` | `crates/context-graph-mejepa/learning_signal.rs::delta_c` | Phase 3 | deferred |
| `compute_delta_e(scheduler_state, drift_state, grad_norm)` | `crates/context-graph-mejepa/learning_signal.rs::delta_e` | Phase 3 | deferred |
| `sampling_weight(ex)` integrated into `BatchSampler` | `crates/context-graph-mejepa-train/sampler.rs` | Phase 3 | deferred |
| Cluster centroids maintained via online k-means / Leiden | `crates/context-graph-solver/cluster.rs` | Phase 3 / Tier 2.5 | deferred |
| Drift state in `CF_MEJEPA_DRIFT_STATE` | `crates/context-graph-storage` (CF added in Phase 6) | Phase 5 | deferred |
| Per-step `(L_mejepa, ΔS, ΔC, ΔE)` recorded in training cert | `crates/context-graph-witness/cert.rs` extension | Phase 3 | deferred (cert format ✅ shipped; extension needed) |
| Periodic-full-retrain k(τ)=4 multiplier | `crates/context-graph-mejepa-train/scheduler.rs::full_retrain_step` | Phase 5 | deferred |

---

## 9. Test/verification protocol

For Phase 3's per-step training certificate, every step's emitted JSON includes:

```json
{
  "step": 1234,
  "loss": 0.0234,
  "delta_s": 0.42,
  "delta_c": 0.71,
  "delta_e": 0.83,
  "L_mejepa": 0.247,
  "sampling_weight": 0.247,
  "grad_norm": 0.83,
  "drift_coverage": 0.91,
  "plasticity_window": 0.85,
  "merkle_root": "..."
}
```

**FSV gate per Phase 3 DoD:**

1. Generate 100 synthetic examples spanning the corner cases (perfect prediction, total miss, high-drift, low-grad, etc.).
2. Compute `L_mejepa` for each.
3. Verify the computed `L_mejepa` matches the expected envelope per the formula.
4. Run a 1-epoch dry-train on the synthetic batch with sampling_weight active; verify the high-L_mejepa examples appear more often in the sampled batches.

**Edge case audit:**

- ΔS = 0 (perfect prediction): `L_mejepa = 0`. Example skipped. Verify by checking `step.batch_indices` excludes the perfect-prediction example.
- ΔC = 0 (extreme OOD): `L_mejepa = 0`. Example logged as OOD-skipped. Verify in the OOD-skip log.
- ΔE = 0 (gradient explosion): `L_mejepa = 0`. Step abort + alert. Verify in the alert log + `CF_MEJEPA_TRAIN_CERTS` shows skip reason.
- All three at 1.0: `L_mejepa = 1.0`. Highest-priority example. Verify the per-example weight reaches `sampling_weight ≥ 1.0` after `(1 + 5·operator_override) × decay^0` factors.

---

## 10. Connection to the system goal

The end goal (doc 09 §1) is `ME-JEPA's predicted_oracle_pass ≥ 0.95 with calibrated confidence` — at which point human code review is statistically wasted. This learning formula optimizes the *path* from "raw mutation corpus" to that end-state by:

1. **Filtering noise** (`ΔS = 0` or `ΔC = 0` examples contribute nothing — no wasted gradient).
2. **Protecting consolidated knowledge** (`ΔE = 0` during gradient explosion or hard drift hard-gates the update — no catastrophic forgetting).
3. **Up-weighting gold supervision** (operator override × 5 — calibration-anchor signals dominate).
4. **Recency-weighted continuous learning** (decay = 0.995 — old examples fade gracefully unless revalidated).

The system is **self-healing** because drift triggers `ΔE → 0` which auto-pauses online observe until the rollback path completes. The system is **self-learning** because every Claude Code outer-loop turn produces (panel, oracle) tuples weighted by L_mejepa — informative ones drive the model forward, noise is silently dropped.

---

## See also

- Source theory: `docs2/UTL_v2_embodied.md` (the rigorous neuroscience-grounded formula)
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md` §3.4 (training procedure), §3.7 (multi-objective loss), §5 (self-healing)
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- Sitrep status: `sitrep/01_status_overview.md`
- Implementation plans: `sitrep/04_implementation_plans/plan_phase2_predictor.md` (where ΔS computation lives) and Phase 3 plan (where ΔC and ΔE integrate)
