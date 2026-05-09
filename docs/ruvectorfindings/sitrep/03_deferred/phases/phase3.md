# Doc 09 Phase 3 — Training Loop + Per-Step Certificates

> **Estimated effort (per doc 09 §7):** 5-7 days.
> **Status (2026-05-09):** DEFERRED — depends on Phase 2 predictor.

## Goal

Phase 3 trains the predictor `g_φ` and the context encoder `f_θ` against
the mutation corpus. The output is a checkpoint pair `(f_θ.bin, g_φ.bin,
f_ξ.bin)` that satisfies the doc 09 §7 holdout-agreement gate:
**predicted-oracle vs. real-oracle agreement ≥ 0.75 on the 10% holdout
within 10 epochs in under 12 hours on an RTX 5090.**

Per doc 09 §1.1's compiler analogy, Phase 3 is the **register allocator
+ peephole optimizer**: it tunes the learned function so the predictor's
output approximates the real Docker oracle's verdicts accurately enough
to be load-bearing in Phase 4.

## Why this phase matters

A predictor with random weights is a 4,096-d (or 5,120-d) random
projection — it produces outputs but they don't correlate with reality.
Training is what makes the predictor a verifier.

The doc 09 §7 DoD demands an empirical agreement gate, not just "loss
went down." If holdout agreement falls short, the predictor is not
load-bearing for Phase 4 — every later phase blocks on this gate.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa-train/`** — new crate with the training
   loop binary (`mejepa-train`).
2. **AdamW optimizer** with cosine decay schedule (`lr=1e-4`,
   `weight_decay=0.05`, `β₁=0.9`, `β₂=0.95`).
3. **Per-step training certificate** persisted to
   `CF_MEJEPA_TRAIN_CERTS` (per doc 04 §9). Each cert binds the step
   index, batch corpus_ids, loss components, gradient norm, EMA τ, and
   parent witness chain hash.
4. **Witness-chain entry per epoch** to
   `crates/context-graph-witness/CF_WITNESS_CHAIN`, allowing offline
   replay of the entire training run.
5. **Holdout evaluation every 1,000 steps**: predict-oracle agreement
   on the holdout split.
6. **Stop-condition**: training halts when (a) holdout agreement ≥0.75
   for 3 consecutive evals, (b) max epochs reached, or (c) gradient
   explosion detected.

## Definition of Done (per doc 09 §7)

- 10 epochs complete on the 2,400-entry corpus in under **12 hours** on
  RTX 5090.
- Holdout `agreement(predicted_oracle, real_oracle) ≥ 0.75` at end of
  training.
- Per-step certs verifiable: random-sample 100 certs, recompute the
  Merkle root, match the persisted root.
- Witness chain reproducible: replay the chain off-line and arrive at
  the same `θ`, `ξ`, `φ` checkpoints.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-train --corpus ./data/mejepa-corpus \
    --epochs 10 --batch-size 64 --output ./checkpoints
# expected:
#   wall-clock: <12h
#   final_holdout_agreement: >=0.75
#   total_steps: ~3,750 (2400 train * 10 / 64 batch)
#   certs_persisted: 3,750 (one per step)
#   witness_chain_entries: 10 (one per epoch)
```

## Component breakdown

### A. Optimizer & schedule

```rust
let optim = AdamW {
    lr_schedule: cosine_decay(initial=1e-4, min=1e-6, total_steps=3750),
    weight_decay: 0.05,
    betas: (0.9, 0.95),
};
```

Apply gradient clipping at `‖g‖_2 ≤ 1.0`. Track gradient norms in the
per-step cert; explosion detection flags steps with `‖g‖ > 10.0`.

### B. Per-step certificate

```rust
struct TrainingCert {
    step: u64,
    batch_corpus_ids: Vec<TaskId>,           // for offline replay
    loss_components: LossBreakdown,           // L_predict, L_var, L_cov, L_inv
    grad_norm: f32,
    tau: f32,                                 // EMA decay this step
    theta_sha: [u8; 32],                      // hash of f_θ params after step
    xi_sha: [u8; 32],                         // hash of f_ξ params after step
    phi_sha: [u8; 32],                        // hash of g_φ params after step
    parent_witness_hash: [u8; 32],            // chain to previous step
    self_hash: [u8; 32],                      // SHAKE-256 of all-of-the-above
}
```

Persisted to `CF_MEJEPA_TRAIN_CERTS` keyed by `step`. The chain of
`parent_witness_hash → self_hash` makes the entire training run a
hash-chained log auditable post-hoc.

### C. Holdout evaluation

Every 1,000 steps, evaluate on the 240-entry holdout split:

```rust
fn evaluate_holdout(
    predictor: &Predictor,
    holdout: &[(Panel, OracleVerdict)],
) -> HoldoutMetrics {
    let mut correct = 0;
    for (panel_t01, real_verdict) in holdout {
        let predicted = predictor.forward(panel_t01).oracle_head_logits();
        let predicted_verdict = argmax_to_verdict(predicted);
        if matches_aggregate(predicted_verdict, real_verdict) {
            correct += 1;
        }
    }
    HoldoutMetrics { agreement: correct as f64 / holdout.len() as f64 }
}
```

### D. Stop condition

```
epoch_done OR (holdout_agreement >= 0.75 for 3 evals consecutive)
OR (epochs > max_epochs) OR (grad_norm > 10.0)
```

## Critical path within Phase 3

1. AdamW with cosine decay — straightforward.
2. Forward + backward + EMA update wired to candle 0.9.2-alpha sm_120a.
3. Per-step cert emission — borrows from `context-graph-witness/cert.rs`
   (already shipped for the Stage 0 + Stage F + Merkle workflow).
4. Holdout eval scheduling.
5. Wall-clock optimization to fit under 12h: gradient checkpointing,
   bf16 mixed precision, panel batch prefetch.

## Dependencies

- ⏳ Phase 2 predictor + EMA target encoder shipped.
- ⏳ Phase 0 full corpus generated (2,400 entries with oracle verdicts).
- ⏳ Repo-atomic train/cal/holdout split with cross_cut_weights = [0,0,0]
  (✅ shipped in Phase 0 split wrapper).
- ⏳ candle 0.9.2-alpha sm_120a CUDA build.
- ⏳ `context-graph-witness::TrainingCert` extension of cert.rs.

## Implementation order (recommended)

**Sub-phase 3.A**: Skeleton training loop with random initialization,
synthetic 100-entry mini-corpus, no certs/witness yet. Verify forward +
backward + AdamW step compiles and produces finite loss. ~1 day.

**Sub-phase 3.B**: Per-step cert emission. Borrow from
`context-graph-witness/cert.rs`. ~1 day.

**Sub-phase 3.C**: Witness chain entry per epoch. ~0.5 days.

**Sub-phase 3.D**: Holdout eval every 1,000 steps. ~1 day.

**Sub-phase 3.E**: Wall-clock optimization (gradient checkpointing,
bf16, prefetch). Target <12h on RTX 5090. ~2 days.

**Sub-phase 3.F**: Run the full 10-epoch training and verify the
holdout-agreement gate. ~1-2 days (mostly compute time).

## Risks (per doc 09 §8)

1. **Training collapses despite VICReg**: monitor variance + covariance
   loss curves. If variance drops below `γ * 0.5`, increase `λ_var`.
2. **Holdout agreement < 0.75**: indicates either (a) corpus too small,
   (b) panel dim too low (5,120 → 8,192), or (c) predictor too shallow
   (6 → 12 layers). Doc 09 §8 prescribes (b) before (c).
3. **Wall-clock > 12h**: gradient checkpointing reduces memory at the
   cost of recomputation; bf16 mixed precision halves wall-clock with
   minimal accuracy loss. If still too slow, reduce panel dim
   temporarily.
4. **EMA drift**: the EMA τ=0.996 means the target encoder follows the
   context encoder with ~250-step lag. If the loss diverges, drop τ to
   0.99 to tighten the coupling.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.4,
  §7`
- Cert format: `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §9`
- Phase 2 dependency: `phase2.md`
- Phase 0 dependency: `phase0.md`
- Existing cert primitive (will be extended): `02_shipped/tier2_foundations.md`
