# 10 — Training Loop + UTML

Source files covered: `crates/context-graph-mejepa-train/src/{lib,trainer/,sampler/,optim/,loss/,cert/,checkpoint,distill/,aux/,learning_signal/,lora_wiring,bin/mejepa-train,cli/}*`, `crates/context-graph-mejepa-cf/src/lib.rs`, `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`, `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`, `docs2/TheoryOfMachineLearning.md` (UTML v1.0 JEPA Edition).

---

## 1. The UTML formula

```
L_step = ΔP × ΔK × ΔΩ × ΔΞ           // all clamped to [0, 1]; multiplicative gating
```

| Factor | Meaning | Formula | Status |
|---|---|---|---|
| **ΔP** | Predictive efficiency | `clamp_[0,1]( loss / SNR + exploration_bonus )` | shipped |
| **ΔK** | Knowledge coherence | `0.5·cos_align(g_t, ḡ_prior) + 0.3·(1−fisher_violation) + 0.2·(1−ECE)` | shipped |
| **ΔΩ** | Optimization-state health | `effective_plasticity^0.4 · landscape_health^0.4 · stability_floor^0.2` | shipped |
| **ΔΞ** | Predictor health | `(1 − target_collapse)^0.5 · (1 − predictor_redundancy)^0.3 · (1 − constellation_violation_rate)^0.2` | shipped (target_collapse ≡ 0 enforced) |

**Multiplicative gating**: any one at zero zeros the whole product. ΔΞ.target_collapse ≡ 0 by construction (frozen instruments) — enforced via `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.

Source: `crates/context-graph-mejepa-train/src/learning_signal/{delta_p,delta_k,delta_omega,delta_xi,l_step}.rs`.

---

## 2. Multi-head extension

Per `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`:

```
Per head h:
  ΔP_h, ΔK_h, ΔΞ_h computed independently
  ΔΩ shared (one optimizer state)

Globally:
  ΔP_global   = Σ_h(λ_h · ΔP_h) / Σ_h(λ_h)
  ΔK_global   = Σ_h(λ_h · ΔK_h) / Σ_h(λ_h)
  ΔΞ_global   = min_h(ΔΞ_h)       // ANY-head collapse hard-gates global
  L_step      = ΔP_global · ΔK_global · ΔΩ · ΔΞ_global
```

Per-head Fisher matrices (`heal::ewc::FisherSnapshot::per_head_fisher`) so a head's update can be regularized against its own consolidated knowledge.

---

## 3. Sampling weight

```
sampling_weight(ex) = base_weight
                    · L_step(ex)
                    · (1 + 5 · operator_override_flag)
                    · (1 + 2 · agent_surprise_severity_score)      // futurebuild Phase A
                    · decay^age_in_days
```

- `base_weight = 1.0`
- `decay = 0.995` (half-life ~140 days)
- `operator_override_flag ∈ {0, 1}`; sets `+5× weight bump` for gold-standard examples
- `agent_surprise_severity_score ∈ {Low=0.2, Medium=0.5, High=0.8, Catastrophic=1.0}`
- Examples with `sampling_weight < 0.05` drop from current epoch
- Examples with `sampling_weight > 1.0` are guaranteed inclusion

Source: `crates/context-graph-mejepa-train/src/sampler/`.

---

## 4. Optimizer

| Property | Value |
|---|---|
| Algorithm | AdamW |
| Learning rate | 5e-4 |
| Weight decay | 0.04 |
| Batch size | 64 |
| Num epochs (full retrain) | 10 |
| LR schedule | Cosine decay |
| LR floor | 1e-6 |
| Grad clip | 10× running mean of grad_norm (per-head clip) |

Source: `crates/context-graph-mejepa-train/src/optim/`.

---

## 5. Trainer orchestration

Source: `crates/context-graph-mejepa-train/src/trainer/`.

```rust
loop {
    // 1. BatchSampler produces sampled (panel_t-1, panel_t, action) tuples
    let batch = sampler.sample_batch(batch_size);
    
    // 2. Forward through trunk + heads
    let outputs = predictor.forward(batch);
    
    // 3. Compute multi-head loss
    let loss = compute_loss(outputs, batch.targets, lambdas);
    
    // 4. Backward
    loss.backward();
    
    // 5. Optimizer step with gradient norm tracking
    let grad_norm = compute_grad_norm();
    optimizer.step();
    
    // 6. Compute ΔP/ΔK/ΔΩ/ΔΞ + L_step
    let deltas = compute_deltas(outputs, gradients);
    
    // 7. Write TrainingCertificate (hash-chained)
    let cert = TrainingCertificate {
        step, loss_global, loss_per_head, delta_p_global, delta_p_per_head,
        delta_k_global, delta_k_per_head, delta_omega, delta_omega_components,
        delta_xi_global, delta_xi_per_head, L_step_global, sampling_weight,
        agent_surprise_severity_score_for_this_ex,
        lambda_h, predictor_redundancy_pairwise_mi, predictor_redundancy_pairwise_mi_source,
        grad_norm_per_head, drift_coverage, merkle_root,
        parent_witness_hash, self_hash,
    };
    persist(CF_MEJEPA_TRAIN_CERTS, step, cert);
    
    // 8. Witness chain entry
    witness_chain.append(witness_entry(cert.self_hash));
    
    // 9. Update sampling weight for the involved examples (over-sample hard ones)
    sampler.update_weights(batch, deltas);
    
    // 10. Periodic holdout eval
    if step % 1000 == 0 {
        let holdout_metrics = run_holdout_eval();
        persist(CF_MEJEPA_EVAL_REPORTS, ...);
    }
}
```

---

## 6. Training certificate (CF_MEJEPA_TRAIN_CERTS)

Each step writes:

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
  "delta_xi_per_head": { ... },
  "L_step_global": 0.247,
  "sampling_weight": 0.247,
  "agent_surprise_severity_score_for_this_ex": 0.0,
  "lambda_h": { "panel": 1.0, "oracle": 0.5, ... },
  "predictor_redundancy_pairwise_mi": 0.07,
  "predictor_redundancy_pairwise_mi_source": "rolling_window_2026_05",
  "grad_norm_per_head": { ... },
  "drift_coverage": 0.91,
  "merkle_root": "...",
  "parent_witness_hash": "shake256:73:...",
  "self_hash": "shake256:73:..."
}
```

Hash chain: `self_hash = SHAKE-256(parent_witness_hash || serialize(cert_body))`. Validation: `optimizer_witness_chain_verify` MCP tool.

---

## 7. UTML carriers (REQ-UTML-01..09 — all shipped)

| Carrier | Function | File |
|---|---|---|
| REQ-UTML-01: ΔP | `compute_delta_p(predicted, target, snr, exploration_bonus)` | `learning_signal/delta_p.rs` |
| REQ-UTML-02: ΔK | `compute_delta_k(grad, ema_grad, fisher, ece, embedder_coherence)` | `delta_k.rs` |
| REQ-UTML-03: ΔΩ | `compute_delta_omega(plasticity, landscape, stability, agent_state)` | `delta_omega.rs` |
| REQ-UTML-04: ΔΞ | `compute_delta_xi(target_collapse, redundancy, violation)` (target_collapse ≡ 0) | `delta_xi.rs` — fails closed `MEJEPA_INSTRUMENT_GRADIENT_LEAK` on leak |
| REQ-UTML-05: EMA gradient | running mean of recent batch gradients | inline |
| REQ-UTML-06: Pairwise MI audit | `PairwiseMiAuditor` + CSV writer | `pairwise_mi_matrix.rs` + `mejepa_audit_pairwise_mi` MCP tool |
| REQ-UTML-07: Continual backprop | `DormantUnitDetector` + `reinit_dormant_units` | `continual_backprop.rs` + `heal/regulate.rs` |
| REQ-UTML-08: State-transfer T | `compute_state_transfer` + `performance_deploy` | `state_transfer.rs` |
| REQ-UTML-09: FSV corner cases | 100 synthetic examples spanning corner cases | `train_edge_cases_fsv` example |

---

## 8. Consolidation integral M(t)

Per UTML §VII:

```
M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
```

Operational mapping:

| Term | Value in ME-JEPA |
|---|---|
| `L_step(τ)` | per-step formula above |
| `ΔΞ(τ)` | 1 × predictor health × constellation violation rate (frozen instruments → target_collapse=0) |
| `k_replay(τ)` | 1 normally; 3-4 during periodic full-retrain |
| `τ_decay(τ)` | 7 days for un-replayed weights; 365 days for high-Fisher consolidated examples |

Product `L_step · ΔΞ` is the "did this step learn anything generalizable" signal. The scheduler keeps it high via:
- Soft drift → LoRA refresh (raises L_step for the violating head)
- Plasticity loss → continual backprop (raises ΔΩ.effective_plasticity)
- Calibration drift → recalibrate (raises ΔK.ECE)
- Constellation staleness → refit (lowers ΔΞ.constellation_violation_rate)

---

## 9. State-transfer T

Per UTML §VIII:

```
T_global(train, deploy)   = exp(−d(P_train, P_deploy) / λ)
performance_deploy        = M(t) · T_global

Per-cell (multi-head extension):
T_cell(c, l)              = exp(−d(P_train_cell(c, l), P_deploy_cell(c, l)) / λ_cell)
performance_deploy_cell   = M(t) · T_cell(c, l)
```

Computed per (`category`, `language`) cell. Persisted to `CF_MEJEPA_STATE_TRANSFER`. Surfaced in weekly eval report. Active-learning queue prioritizes cells with low T (deploy distribution drifted from training).

---

## 10. The "Dissipating ML" failure mode

Per UTML §VII.1:

```
Dissipating_global ≡ ( mean(L_step) over last N updates > 0.3 )
                    AND ( ΔK or ΔΞ silently degrading; validation generalization gap widening )
                    AND ( ΔM / mean(L_step) < 0.2 )
```

Multi-head extension:

```
Dissipating_per_head_h ≡ ( mean(L_step_h) > 0.3 )
                        AND ( ΔK_h or ΔΞ_h silently degrading )
                        AND ( head_h_holdout_metric_delta / mean(L_step_h) < 0.2 )
```

Detection: rolling training loss trend vs rolling holdout-correlation trend. If training loss decreasing > 5% over 1000 steps but holdout correlation flat or decreasing → `MEJEPA_DISSIPATION_DETECTED` (futurebuild Phase E, `TASK-HEAL-003`).

Intervention: replay-buffer enrichment + EWC re-anchor + LR annealing/warmup reset + continual backprop reinit + fresh real-data injection (the flywheel handles this naturally).

---

## 11. LoRA wiring + distillation

Source: `lora_wiring.rs`, `distill/mod.rs`.

- **LoRA**: rank `r=16` adapter per head (default); applied on top of active weights during refresh.
- **Distillation**: aux-head distillation back into trunk every 1,000 steps. Source: `aux/mod.rs`. Tracked in `CF_MEJEPA_DISTILL_STEPS`.

---

## 12. Holdout evaluation cadence

| Cadence | What runs |
|---|---|
| Every 1,000 steps | per-(category × language) correlation check; persisted to `CF_MEJEPA_EVAL_REPORTS` |
| Every 10,000 steps | full periodic retrain via heal scheduler (default `training.full_retrain_every_attempts`) |
| Weekly (daemon cron) | full holdout eval + ship-gate status report + active-learning queue surface |

---

## 13. FSV evidence

| FSV | Output |
|---|---|
| Training cert chain (100k steps) | `/tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json` |
| UTML L_step over 100 synthetic examples | `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json` |
| Holdout eval | `/tmp/contextgraph-mejepa-train-fsv/holdout-eval-evidence.json` |
| Target collapse leak (negative test) | rejected with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`; cert NOT written |

---

## 14. Cross-references

- [`07_instruments_panel.md`](07_instruments_panel.md) — frozen target side
- [`09_predictor_multi_head.md`](09_predictor_multi_head.md) — predictor side
- [`12_self_healing_scheduler.md`](12_self_healing_scheduler.md) — heal scheduler triggers
- [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) — pairwise MI + redundancy
- [`15_witness_chain.md`](15_witness_chain.md) — training cert chain
