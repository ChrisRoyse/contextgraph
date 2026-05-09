# Plan: Phase 2 — Panel Materializer + Predictor + EMA + VICReg

> **Priority:** MEDIUM (biggest single work item; ~7-10 days).
> **Effort:** ~7-10 days.
> **Crate:** `crates/context-graph-mejepa/` (new).

## Goal

Ship the entire Phase 2 deliverable per doc 09 §2.3-§2.5 + §7:

- Panel materializer producing `panel[t=0]`, `panel[t=1]`, `panel[t=2]`
  given `(task_id, patch_id)`.
- Context encoder `f_θ` (6-layer transformer, hidden 1024, 8 heads).
- EMA target encoder `f_ξ` with τ = 0.996.
- Predictor `g_φ` mapping `panel[t=0..1] -> predicted panel[t=2]`.
- VICReg loss: variance + invariance + covariance.
- Oracle head: `Linear<5120, num_tests + 1>` mapping the predicted
  panel to per-test pass logits.

End-state: `cargo run -p context-graph-mejepa -- mejepa-train --dry-run
--batches 1 --batch-size 64` produces a finite loss on RTX 5090 in
under 50 ms/batch with VRAM <8 GB.

## Deliverables

1. New crate `crates/context-graph-mejepa/`.
2. Sub-modules:
   - `panel.rs` — panel materialization from `(task_id, patch_id)`.
   - `encoder.rs` — `ContextEncoder f_θ` + `TargetEncoder f_ξ`.
   - `predictor.rs` — `Predictor g_φ` (6-layer transformer).
   - `oracle_head.rs` — `OracleHead`.
   - `loss.rs` — `vicreg_loss(...)`.
   - `bin/mejepa_train.rs` — dry-run binary.
3. candle 0.9.2-alpha CUDA setup for the RTX 5090 sm_120a.
4. RocksDB CF: `CF_MEJEPA_PANELS` keyed by `(attempt_id, time_step)`.
5. 30+ unit tests + dry-run integration test.
6. FSV evidence at `/tmp/contextgraph-mejepa-fsv/`.

## Definition of Done (per doc 09 §7)

- Forward pass: `panel[t=0..1] → predictor → predicted panel[t=2]`
  with output dim 5,120 (per §3.5.5; doc text says 4,096 in §2.5 but
  the §3.5 expansion overrides — verify exact dim during impl).
- VICReg loss is finite on a single-batch dry-run.
- Memory footprint < 8 GB VRAM at batch 64 on RTX 5090.
- Forward latency < 50 ms / batch.

## Verification protocol

```bash
context-graph mejepa-train --dry-run --batches 1 --batch-size 64
# expected:
#   forward latency: <50ms/batch
#   loss: finite, ~order(10²) before training
#   no NaN/Inf
#   peak VRAM: <8 GB
```

## Component breakdown (with sub-effort estimates)

### Sub-phase 2.A: Panel materializer (~2 days)

```rust
pub fn build_panel(
    task_id: &TaskId,
    patch: &Patch,
    time_step: TimeStep,
    instruments: &InstrumentRegistry,
) -> Result<Panel, PanelBuildError>;
```

Time steps:
- **t=0** — pre-patch state. Set: E_AST, E_CFG, E_DataFlow, E_TypeGraph,
  E_Test, E_Problem on ORIGINAL code; E_Diff = 0; E_Trace = 0;
  E_Witness = 0; E_Oracle = 0; E_CommitMsg = 0.
- **t=1** — post-patch, pre-execution. E_AST..E_TypeGraph re-evaluated
  on PATCHED code; E_Diff populated; E_CommitMsg populated; E_Trace = 0;
  E_Witness partial; E_Oracle = 0.
- **t=2** — post-execution. ALL slots populated. The `E_Oracle` slice
  is the supervision signal.

Panels persist to `CF_MEJEPA_PANELS` keyed by
`(attempt_id, time_step)`. Reuse `PanelBuilder::set_from_instrument`.

### Sub-phase 2.B: Context encoder + EMA target encoder (~2 days)

```rust
pub struct ContextEncoder {
    layers: Vec<TransformerLayer>,    // 6 layers
    norm: LayerNorm,
}

pub struct TargetEncoder {
    inner: ContextEncoder,            // EMA shadow of f_θ
}

impl TargetEncoder {
    pub fn ema_update(&mut self, source: &ContextEncoder, tau: f32) {
        // ξ ← τ ξ + (1 - τ) θ, applied per-parameter
    }
    pub fn forward_no_grad(&self, x: &Tensor) -> Tensor {
        candle::no_grad(|| self.inner.forward(x))
    }
}
```

Both share architecture (6 layers, hidden 1024, 8 heads, GeLU,
pre-LN). EMA target encoder freezes its parameters from the gradient
graph and updates via the rule outside.

### Sub-phase 2.C: Predictor (~2 days)

```rust
pub struct Predictor {
    input_proj: Linear<8192, 4096>,   // panel[t=0] || panel[t=1] = 10240
                                       // (5120*2) → projected to 4096
    layers: Vec<TransformerLayer>,    // 6 layers, hidden 1024, 8 heads
    output_proj: Linear<1024, 5120>,  // hidden → predicted panel[t=2]
}
```

Note: doc 09 §2.5 says "input dim 8192 → output dim 4096" but §3.5.5
expands the panel to 5120-d. Resolve during implementation: most
likely `input = 5120 * 2 = 10240` and `output = 5120`.

### Sub-phase 2.D: VICReg loss (~2 days)

```rust
pub fn vicreg_loss(
    predicted_t2: &Tensor,        // [batch, 5120] from predictor
    real_t2: &Tensor,              // [batch, 5120] from EMA target encoder
    config: VicRegConfig,
) -> Tensor {
    let l_predict = huber_loss(predicted_t2, real_t2, delta=1.0);
    let l_var = variance_loss(predicted_t2, gamma=1.0)
              + variance_loss(real_t2, gamma=1.0);
    let l_inv = mse(predicted_t2, real_t2);
    let l_cov = covariance_loss(predicted_t2)
              + covariance_loss(real_t2);
    l_predict
        + config.lambda_var * l_var
        + config.lambda_inv * l_inv
        + config.lambda_cov * l_cov
}
```

Defaults: `λ_var = 25, λ_inv = 25, λ_cov = 1, γ = 1`.

### Sub-phase 2.E: Oracle head + dry-run verification (~1 day)

```rust
pub struct OracleHead {
    project_to_logits: Linear<5120, /* num_tests + 1 */ 32>,
}
```

The +1 is the "exception" class. Number of test slots is task-specific;
for SWE-bench Lite use a max of 31 (covering >99th percentile of test
counts) + 1 exception slot = 32.

Dry-run: assemble random panels, run forward, check:
- Output shape correct.
- Loss is finite.
- VRAM peak <8 GB.
- Forward latency <50 ms.

## Dependencies

- ⏳ Phase 0 RocksDB CFs (to persist panels).
- ⏳ At least 7 of 14 Phase 1 instruments shipped (E_AST, E_CFG,
  E_DataFlow, E_TypeGraph, E_Diff, E_Witness, E_Oracle) — placeholder
  hashed-feature versions OK.
- ⏳ candle 0.9.2-alpha sm_120a CUDA build.
- ⏳ RTX 5090 hardware (operator's machine).

## Risks (per Phase 2 risks)

1. **JEPA generalizes across repositories?**: open empirical question;
   train on diverse subset, evaluate on held-out repos.
2. **5,120-d panels expressive enough?**: if test-agreement plateaus
   <0.95, increase to 8,192 at 2× compute cost.
3. **Memory footprint at batch 64**: predictor + 2× encoders + VICReg
   activations should fit under 8 GB at batch 64 on 32 GB VRAM RTX 5090;
   monitor during dry-run.

## Fresh-agent recipe

1. Read `../03_deferred/phases/phase2.md` for full context.
2. Bootstrap candle 0.9.2-alpha sm_120a:
   ```toml
   [dependencies]
   candle-core = { version = "0.9.2-alpha", features = ["cuda"] }
   candle-nn = "0.9.2-alpha"
   ```
3. Implement sub-phases in order A → B → C → D → E.
4. Each sub-phase has its own micro-FSV (small synthetic batch through
   the just-implemented stage).
5. Final dry-run: assemble all stages, verify the §7 DoD.
6. Update `01_status_overview.md` to mark Phase 2 shipped.

## See also

- Phase 2 phase doc: `../03_deferred/phases/phase2.md`
- Phase 1 trait: `02_shipped/phase1_scaffolding_and_e_oracle.md`
- VICReg reference: doc 09 §2.5 (paper: Bardes et al. 2022).
- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.3,
  §2.4, §2.5, §7`.
