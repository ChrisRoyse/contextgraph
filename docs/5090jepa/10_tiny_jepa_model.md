# 10 — Tiny JEPA Model (Architecture, Loss, Collapse Audits)

The demo's predictor is intentionally small. The point is to prove the *contract* (data plane → trained predictor → predictions → planning → audit) works end-to-end on the operator's hardware. Anything bigger than the configurations below is out of scope.

The full plan's open questions doc (`docs/dynamicjepa/implementationplan/15_open_questions_decisions.md`) recommends "tiny deterministic architecture first for FSV." This file makes that concrete.

Current implementation status as of the Phase 7 review on 2026-04-30: the CUDA-only tiny JEPA trainer is implemented in `crates/context-graph-dynamicjepa/src/model/mod.rs` and is called through `dynamicjepa train`. It consumes real `DatasetShardRecord` rows and writes real artifact files. Counter_world Phase 7 FSV passes convergence/hash checks on a generated 60-event fixture; strict shuffled-target and covariance acceptance remain open.

## 1. Architecture (Single File of Truth)

Both encoder and predictor are MLPs implemented in candle. The current implementation is a single module, `crates/context-graph-dynamicjepa/src/model/mod.rs`, with `LinearLayer`, `MlpNetwork`, and `TinyMlpJepa`. The split below is the planned long-term module shape, not the current file layout.

```
encoder.rs       — input panel → latent embedding
predictor.rs     — (latent, action) → predicted next latent
target_encoder.rs — EMA copy of encoder; no gradient
losses.rs        — latent MSE + VICReg variance + VICReg covariance
trainer.rs       — owns optimizer, dataloader, logging
inference.rs     — load artifact, hash-check, predict, write PredictionRecord
```

### 1.1 Online Encoder

```text
Encoder(panel_dense: f32[D_in]) -> z: f32[D_z]

D_in = panel.total_dense_dim    // counter_world: 2; gridworld: 31 (25 state + 5 action + 1 step)
D_z  = config.model.encoder.out_dim   // 8 for counter_world, 32 for gridworld

layer_0: Linear(D_in, hidden[0]) + ReLU
layer_1: Linear(hidden[0], hidden[1]) + ReLU  // gridworld only
layer_out: Linear(hidden[-1], D_z)
```

### 1.2 Predictor

```text
Predictor(z: f32[D_z], action_dense: f32[D_a]) -> z_hat: f32[D_z]

cat = concat(z, action_dense)           // shape [D_z + D_a]
layer_0: Linear(D_z + D_a, hidden[0]) + ReLU
layer_1: Linear(hidden[0], hidden[1]) + ReLU
layer_out: Linear(hidden[-1], D_z)
```

### 1.3 Target Encoder (EMA, No Grad)

```text
target_encoder = EMA(encoder, momentum=0.99)  // counter_world
target_encoder = EMA(encoder, momentum=0.995) // gridworld
```

After every batch:

```rust
for (target_p, online_p) in target.params().zip(online.params()) {
    target_p.data = momentum * target_p.data + (1.0 - momentum) * online_p.data;
}
```

The target encoder is not trained directly. It produces `z_target` from the *target panel*, against which the predictor's `z_hat` is compared. This is the standard JEPA non-collapse trick (slow target encoder + bootstrap).

## 2. Loss

```text
L = w_mse * MSE(z_hat, sg(z_target))
  + w_var * VICReg_variance(z_online)        // applied to online encoder outputs
  + w_cov * VICReg_covariance(z_online)
```

`sg` is stop-gradient (target encoder is detached). VICReg terms are applied to the **online encoder** outputs `z_online` (the encoder's view of the input panels in the batch), not to the predictor's output:

- `VICReg_variance(z) = mean over dim d of relu(target_std - sqrt(var(z[:, d]) + eps))`
- `VICReg_covariance(z) = sum over off-diagonal (i, j) of cov(z)[i, j]^2 / D_z`

Defaults from the train configs:

```
w_mse = 1.0
w_var = 1.0       (target_std = 1.0)
w_cov = 0.04
eps   = 1e-4
```

The variance term prevents the encoder from collapsing all inputs to one point. The covariance term decorrelates dims. The MSE term is the predictive objective. Collapse-prevention proof from the literature: see C-JEPA / VICReg-based JEPA — EMA alone is *not* sufficient, explicit variance + covariance constraints are required for robust convergence.

## 3. Optimizer & Schedule

AdamW (`candle-nn::optim::AdamW`) on the union of online encoder + predictor parameters. Target encoder excluded.

| Hyperparameter | Counter_world | Gridworld |
| --- | --- | --- |
| Learning rate | 1e-3 | 5e-4 |
| Weight decay | 1e-4 | 1e-4 |
| Warmup steps | 0 | 200 (linear) |
| Epochs | 200 | 30 |
| Batch size | 2 | 256 |
| Device | `cuda:0` | `cuda:0` |
| Seed | 42 | 42 |

The checked-in three-event counter_world fixture has 1 train row + 1 val row and is too small to verify train-only convergence without leakage. The current Phase 7 FSV therefore generates a deterministic 60-event counter_world fixture, compiles train/val/test shards from it, trains only on train rows, and evaluates all splits.

## 4. Training Loop (Pseudocode)

```rust
fn train(cfg: TrainConfig, shard: DatasetShardRecord) -> Result<TrainingArtifacts> {
    // (1) Validate device and load shard panels into pinned memory.
    let device = cuda_device(cfg.schedule.device)?;
    let panels = load_panels_for_shard(&shard)?;

    // (2) Build models on device.
    let mut online = Encoder::new(&cfg.model.encoder, &device)?;
    let mut predictor = Predictor::new(&cfg.model.predictor, &device)?;
    let mut target = TargetEncoder::clone_of(&online);

    // (3) Optimizer.
    let mut opt = AdamW::new(online.params().chain(predictor.params()), cfg.optim)?;

    // (4) Loop.
    let start = Instant::now();
    let mut metrics = Metrics::new();
    'epoch: for epoch in 0..cfg.schedule.epochs {
        let epoch_indices = shuffled_epoch_indices(train_indices, cfg.seed, epoch);
        for batch in batches(epoch_indices, cfg.schedule.batch_size) {
            let z_online = online.forward(&batch.input_panels)?;
            let z_target = target.forward_no_grad(&batch.target_panels)?;
            let z_hat    = predictor.forward(&z_online, &batch.actions)?;

            let l_mse = mse(&z_hat, &z_target.detach());
            let l_var = vicreg_variance(&z_online, cfg.loss.vicreg_target_std);
            let l_cov = vicreg_covariance(&z_online);
            let loss  = cfg.loss.latent_mse_weight * l_mse
                      + cfg.loss.vicreg_variance_weight * l_var
                      + cfg.loss.vicreg_covariance_weight * l_cov;

            opt.backward_step(&loss)?;
            target.ema_update(&online, cfg.model.ema_momentum);
            metrics.record_step(loss, l_mse, l_var, l_cov);
        }

        let val = evaluate(&online, &predictor, &target, &shard, "val")?;
        metrics.record_epoch(epoch, &val);

        if val.latent_mse <= cfg.stopping.target {
            break 'epoch;
        }
        if start.elapsed().as_secs_f64() > cfg.stopping.max_seconds as f64 {
            return Err(DynamicJepaError::TrainingFailed {
                message: "max_seconds budget exceeded before convergence",
                ...
            });
        }
    }

    let final_eval = evaluate(...)?;          // final val + test metrics
    save_artifact_files(&cfg, &online, &predictor, &target, &final_eval)?;
    Ok(TrainingArtifacts { metrics: metrics.into_btreemap(), ... })
}
```

Steps (5)–(7) (artifact write, hash compute, registry write) live in `11_training_runs_and_artifacts.md`.

## 5. Parameter Counts (Verify At Build Time)

| Domain | Encoder | Predictor | Target | Total params |
| --- | --- | --- | --- | --- |
| counter_world | (2→16) + (16→8) ≈ 200 | (8+1→16) + (16→8) ≈ 280 | mirror of encoder (no grad) | ≈ 480 trainable, ≈ 680 total |
| gridworld | (31→64) + (64→64) + (64→32) ≈ 6.5K | (32+5→64) + (64→64) + (64→32) ≈ 7K | mirror of encoder (no grad) | ≈ 13.5K trainable |

The current Phase 7 FSV asserts `parameter_count_trainable == 480` for counter_world. A future gridworld model FSV should assert the gridworld count remains below 20K trainable params. Above that, the test should fail with the actual count and a remediation pointing to this section. The point: the demo is intentionally tiny, and the asserts protect against accidental architecture drift.

## 6. Collapse Audits

Current Phase 7 counter_world FSV records and asserts:

1. `vicreg_variance_per_dim_min >= 0.5`.
2. Validation metrics read back from `evaluation_report.json` and the `TrainingRunRecord`.
3. Artifact files hash-verified through `get-artifact --verify-files`.

It also records `vicreg_covariance_off_diag_frobenius`. The documented strict covariance target (`<= 0.5`) is not yet an acceptance gate because the current run records ~4.87 on the generated counter fixture. This is an open model-quality issue, not a test workaround.

## 7. Leakage Audit

After a successful training run, the current FSV:

1. Trains only on rows marked `train`; validation and test rows are never included in optimizer batches.
2. Uses a seeded `shuffled_epoch_indices` function so training order changes per epoch without relying on nondeterministic host state.
3. Records random-init and shuffled-target baselines in `evaluation_report.json`.

The strict shuffled-target bound (`<= 0.10` for counter_world) is not currently passing on the monotonic 60-event fixture; the recorded `shuffled_target_cosine` remains high. Treat this as a remaining audit gap before claiming full Phase 7 model-quality acceptance.

## 8. Random-Init Baseline (`tests/random_init_audit.rs`)

The current FSV instantiates an encoder + predictor with the same architecture but **without training**, runs evaluation, records `random_init_cosine`, and asserts the trained run is materially better. The latest counter_world evidence records `random_init_cosine = -0.4014871659000763`.

Future stricter gates should restore the original bounds:

- counter_world: `cosine_avg <= 0.10`.
- gridworld: `cosine_avg <= 0.30`.

Both bounds are well above zero because randomly initialized linear layers can produce coincidentally aligned vectors on small data; the bounds are loose enough to not flake but tight enough that a randomly-init "trained" run would fail.

## 9. Determinism

All model-side random sources currently flow from `cfg.seed`: CUDA weight initialization plus deterministic epoch shuffling. The current FSV checks that the run converges and records artifact hashes, but it does not yet assert byte-identical artifact hashes across two independent runs.

If a run is non-deterministic, the most likely cause is `cuda_graph` or `cudnn_benchmark` flags. Disable them in candle's `Device::Cuda` configuration for the demo.

## 10. Why Not VICRegL/SimSiam/BYOL/DINO?

Brief reasoning for the operator and reviewer:

- **BYOL / SimSiam**: relies on architectural asymmetry to prevent collapse. Works but is harder to debug than VICReg's explicit constraints.
- **DINO**: temperature-controlled softmax + centering — overkill for our 8-dim and 32-dim latents.
- **VICReg / C-JEPA**: explicit variance + covariance terms, easy to monitor (`vicreg_variance_per_dim_min` is a single number we assert), and proved sufficient in published JEPA work. Best Occam fit.

If the operator later wants to swap the loss, the surface area is: `losses.rs` plus the seven `vicreg_*` keys in the train config. Architecture, data, and storage stay unchanged.
