# TASK-PRED-005 — MeJepaPredictor::forward (concat 10240 → input_proj 1024 → 6 layers → output_proj 5120)

```xml
<task_spec id="TASK-PRED-005" version="1.0">

<metadata>
  <title>Implement MeJepaPredictor::forward + trainable_parameters — full 6-layer transformer pipeline with input dim contract</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-PRED-01">Input (panel_t0, panel_t1) shape (B, 5120) → output (B, 5120) — strict dim contract</item>
    <item ref="REQ-PRED-02">6-layer transformer (config.num_layers); output proj 5120</item>
    <item ref="REQ-PRED-03">Forward latency p50 &lt; 50 ms at batch 64 on RTX 5090</item>
    <item ref="REQ-PRED-04">MEJEPA_PRED_NAN_DETECTED at input + output (full-tensor scan in dry-run; sample-based in hot path)</item>
    <item ref="REQ-PRED-05">Eval-mode determinism (byte-equal SHA-256 across two consecutive forwards)</item>
    <item ref="REQ-PRED-09">trainable_parameters() returns ONLY input_proj + layers + output_proj + oracle_head; NEVER frozen_target_adapter</item>
    <item ref="REQ-PRED-17">gradient_checkpointing flag honored when set</item>
    <item ref="REQ-PRED-18">Boundary validation at MeJepaPredictor::forward entry point</item>
    <item ref="component:MeJepaPredictor">forward + trainable_parameters + architecture_summary</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-001</task_ref>
    <task_ref>TASK-PRED-002</task_ref>
    <task_ref>TASK-PRED-003</task_ref>
    <task_ref>TASK-PRED-004</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 modified, 0 created</estimated_files>
</metadata>

<context>
This is the central trainable forward pass of ME-JEPA-Code. Given two consecutive 5,120-d panels
`(panel_t0, panel_t1)`, concatenate to 10,240-d, run through `input_proj` Linear(10240, 1024),
6 stacked `TransformerLayer`s (TASK-PRED-004), and `output_proj` Linear(1024, 5120) → emit a
5,120-d `PredictedPanel`. All tensors bf16 on candle-cuda.

Two boundary-validation paths:
  - **Hot path** (dry-run / FSV when `validate_finite=false`): only check shape; rely on output-only
    finiteness check at the end (cheap reduction).
  - **Adversarial / dry-run when `validate_finite=true`**: full-tensor scan on inputs + per-layer
    output via `TransformerLayer::forward_with_layer_idx`.

`trainable_parameters()` enumerates ONLY: input_proj weights+bias, every layer's six Vars
(self_attn_qkv, self_attn_out, ffn_up, ffn_down, attn_ln {weight, bias}, ffn_ln {weight, bias}),
output_proj weights+bias, and `oracle_head.trainable_parameters()`. Critically it does NOT touch
`frozen_target_adapter` — that's the REQ-PRED-09 type-level guarantee. A startup
`debug_assert!(self.frozen_target_adapter.parameters().is_empty())` is also enforced inside
`new()` (already in TASK-PRED-001) and re-asserted on every `trainable_parameters()` call to
catch a corrupted adapter at runtime.

Iron Rule: this task does NOT call `vicreg_loss` (TASK-PRED-007), does NOT call gradient hook
(TASK-PRED-009), does NOT call VRAM check (TASK-PRED-010), and does NOT reference
`OracleHead::predict_logits/predict_probabilities` bodies (TASK-PRED-008) — only their
existence-as-method (signature already shipped in TASK-PRED-002).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; performance_budgets predictor_forward_p50)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-01..05, REQ-PRED-09, REQ-PRED-17, REQ-PRED-18; AC-PRED-01-01..03; EC-PRED-01..03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (api_contracts MeJepaPredictor::forward + trainable_parameters; implementation_notes "determinism", "vram", "architecture-headroom")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-001.md (struct skeleton + new())</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-002.md (PredictedPanel + OracleHead::trainable_parameters)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (PredictorError taxonomy)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-004.md (TransformerLayer::forward + forward_with_layer_idx)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/predictor/mod.rs (struct + new shipped)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001..004 complete</check>
  <check>cargo build --release -p context-graph-mejepa succeeds</check>
  <check>RTX 5090 with sm_120a available (test gated by `#[cfg(feature = "cuda")]`)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn forward(&self, panel_t0: &Tensor, panel_t1: &Tensor) -> Result<PredictedPanel, PredictorError>` (hot path, no per-layer finiteness check)
    - Implement `pub fn forward_dryrun(&self, panel_t0: &Tensor, panel_t1: &Tensor) -> Result<PredictedPanel, PredictorError>` (per-layer finiteness check enabled; for FSV / adversarial)
    - Implement `pub fn trainable_parameters(&self) -> Vec<&Var>` (input_proj + 6 layers + output_proj + oracle_head; debug_assert frozen_target empty)
    - Implement `pub fn architecture_summary(&self) -> ArchitectureSummary` (returns num_layers, hidden_dim, num_heads, activation, residual=true, layer_norm=true)
    - Add `ArchitectureSummary` struct in `predictor/mod.rs` (Serialize + Debug)
    - Honor `config.gradient_checkpointing` flag: when true, wrap each layer's forward in `candle_core::Tensor::checkpoint`-equivalent (or a no-op if candle 0.9.x lacks the helper — log a debug warning and proceed without checkpointing; do NOT fail closed since this is a perf hint, not a correctness requirement; explicit "checkpointing requested but unsupported" recommendation in PredictorError-free path)
    - Add 6 unit tests (cuda-gated): shape contract; dim mismatch error; NaN-input error; eval-mode determinism (byte-equal); trainable_parameters() count == expected; trainable_parameters() does NOT include frozen_target state
  </in_scope>
  <out_of_scope>
    - VICReg loss (TASK-PRED-007)
    - Gradient hook (TASK-PRED-009)
    - VRAM probing during forward (TASK-PRED-010)
    - FSV evidence file writing (TASK-PRED-011)
    - DoD CLI (TASK-PRED-012)
    - Oracle head logits / softmax (TASK-PRED-008)
    - Backward pass registration (Phase 3)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/predictor/mod.rs">
use candle_core::{Tensor, Var};
use serde::Serialize;

use crate::config::{PANEL_DIM, CONCAT_INPUT_DIM};
use crate::data_models::PredictedPanel;
use crate::error::{NanSource, PanelSide, PredictorError};

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureSummary {
    pub num_layers: u8,
    pub hidden_dim: u32,
    pub num_heads: u8,
    pub ff_expansion: u8,
    pub activation: &'static str,   // "GeLU"
    pub residual: bool,             // true
    pub layer_norm: bool,           // true
    pub panel_dim: usize,           // 5120
    pub concat_input_dim: usize,    // 10240
    pub gradient_checkpointing: bool,
}

impl MeJepaPredictor {
    pub fn forward(
        &amp;self,
        panel_t0: &amp;Tensor,
        panel_t1: &amp;Tensor,
    ) -&gt; Result&lt;PredictedPanel, PredictorError&gt;;

    pub fn forward_dryrun(
        &amp;self,
        panel_t0: &amp;Tensor,
        panel_t1: &amp;Tensor,
    ) -&gt; Result&lt;PredictedPanel, PredictorError&gt;;

    pub fn trainable_parameters(&amp;self) -&gt; Vec&lt;&amp;Var&gt;;

    pub fn architecture_summary(&amp;self) -&gt; ArchitectureSummary;

    /// Internal — both forward and forward_dryrun delegate here.
    fn forward_inner(
        &amp;self,
        panel_t0: &amp;Tensor,
        panel_t1: &amp;Tensor,
        validate_finite: bool,
    ) -&gt; Result&lt;PredictedPanel, PredictorError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - Validate `panel_t0.dims() == [batch, PANEL_DIM]` AND `panel_t1.dims() == [batch, PANEL_DIM]` AND batch sizes equal; on violation: `PredictorError::DimMismatch { detail, observed: {"panel_t0": "...", "panel_t1": "..."}, expected_panel_dim: PANEL_DIM }`
    - When `validate_finite=true`: full-tensor scan on inputs (`tensor.abs()?.sum_all()?.to_scalar::<f32>()?` finite check); on NaN: `PredictorError::NanDetected { source: Input { panel: T0|T1 }, ... }`
    - Concat: `Tensor::cat(&[panel_t0, panel_t1], 1)?` → shape (batch, CONCAT_INPUT_DIM = 10240); assert dims after concat
    - Pipeline: input_proj → for layer in self.layers: layer.forward_with_layer_idx(x, layer_idx as u8, validate_finite) → output_proj
    - Output finiteness check at the very end (always, hot AND dry-run): `out.abs()?.sum_all()?.to_scalar::<f32>()?`; on NaN: `PredictorError::NanDetected { source: Output, ... }`
    - dtype must equal `self.dtype` (BF16 on cuda); on mismatch: `PredictorError::DimMismatch { detail: "dtype mismatch", ... }` (we treat dtype as part of the dim contract)
    - device must equal `self.device`; on mismatch: same error
    - `trainable_parameters()` enumeration order MUST be deterministic (input_proj.weight, input_proj.bias, then each layer in order: self_attn_qkv.{w,b}, self_attn_out.{w,b}, ffn_up.{w,b}, ffn_down.{w,b}, attn_ln.{w,b}, ffn_ln.{w,b}, then output_proj.{w,b}, then oracle_head.trainable_parameters())
    - `trainable_parameters()` runs `debug_assert!(self.frozen_target_adapter.parameters().is_empty())` at the start (the type-level guarantee is REQ-PRED-09; this is the fast cross-check)
    - Trainable parameter count budget: input_proj (10240*1024 + 1024) + 6 * (qkv: 1024*3072 + 3072 ; out: 1024*1024 + 1024 ; ffn_up: 1024*4096 + 4096 ; ffn_down: 4096*1024 + 1024 ; attn_ln: 2048 ; ffn_ln: 2048) + output_proj (1024*5120 + 5120) + oracle_head (5120 * (num_tests+1) + (num_tests+1)) — verify the count fits the ~3 GB target at bf16 (15.7M predictor params + ~6.4M oracle head ≈ 22M params * 2 bytes ≈ 44 MB ≪ 3 GB; activation memory dominates)
    - When `config.gradient_checkpointing == true`: log a debug entry "gradient_checkpointing requested" but do not fail; if candle-core 0.9.x exposes `Tensor::checkpoint` in stable API, wrap each layer; otherwise emit a one-time `tracing::warn!("gradient_checkpointing not yet supported in candle 0.9.x")` and proceed
    - Iron Rule: this method NEVER calls oracle_head.predict_logits / predict_probabilities (TASK-PRED-008's job); the oracle head is owned but not invoked here
    - File path: stay within `crates/context-graph-mejepa/src/predictor/mod.rs`; if the file approaches 500 lines, split tests to `predictor/tests.rs` (sibling file) per constitution `<file_organization>`
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 26 tests passing (this task adds 6)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Test on cuda: forward with batch=4, PANEL_DIM-correct inputs returns PredictedPanel with shape (4, 5120) and bf16 dtype
    - Test: panel_t0 shape (4, 4096) → Err(PredictorError::DimMismatch { observed: {"panel_t0": "[4, 4096]", ..}, ..})
    - Test: panel_t0 with NaN + forward_dryrun → Err(PredictorError::NanDetected { source: Input{panel: T0}, ..})
    - Test: two forward() calls with same inputs + weights → byte-equal SHA-256 of bf16 output
    - Test: trainable_parameters().len() == expected (input_proj 2 + 6*8 + output_proj 2 + oracle_head 1or2) — exact count is verifiable
    - Test: trainable_parameters() does NOT include any Var alias of frozen_target_adapter weights (Var-pointer-equality check vs. Vec<&Var>)
  </verification>
</definition_of_done>

<pseudo_code>
fn forward(&amp;self, p0, p1) -&gt; Result&lt;PredictedPanel, PredictorError&gt; {
    self.forward_inner(p0, p1, /*validate_finite=*/false)
}

fn forward_dryrun(&amp;self, p0, p1) -&gt; Result&lt;PredictedPanel, PredictorError&gt; {
    self.forward_inner(p0, p1, /*validate_finite=*/true)
}

fn forward_inner(&amp;self, panel_t0, panel_t1, validate_finite) -&gt; Result&lt;PredictedPanel, PredictorError&gt; {
    // 1. Shape + device + dtype validation
    let dims_t0 = panel_t0.dims();
    let dims_t1 = panel_t1.dims();
    if dims_t0.len() != 2 || dims_t1.len() != 2 || dims_t0[1] != PANEL_DIM || dims_t1[1] != PANEL_DIM || dims_t0[0] != dims_t1[0] {
        return Err(PredictorError::DimMismatch {
            detail: format!("panel_t0={:?} panel_t1={:?} expected (B, {})", dims_t0, dims_t1, PANEL_DIM),
            observed: btreemap!{"panel_t0" =&gt; format!("{:?}", dims_t0), "panel_t1" =&gt; format!("{:?}", dims_t1)},
            expected_panel_dim: PANEL_DIM,
        });
    }
    if panel_t0.dtype() != self.dtype || panel_t1.dtype() != self.dtype {
        return Err(PredictorError::DimMismatch { detail: "dtype mismatch".to_string(), ... });
    }
    let batch = dims_t0[0];

    // 2. Input finiteness (dry-run only)
    if validate_finite {
        ensure_finite(panel_t0, NanSource::Input { panel: PanelSide::T0 })?;
        ensure_finite(panel_t1, NanSource::Input { panel: PanelSide::T1 })?;
    }

    // 3. Concat
    let x = Tensor::cat(&[panel_t0, panel_t1], 1)?;
    debug_assert_eq!(x.dims(), &[batch, CONCAT_INPUT_DIM]);

    // 4. Input projection
    let mut x = self.input_proj.forward(&x)?;

    // 5. Layers
    for (i, layer) in self.layers.iter().enumerate() {
        x = layer.forward_with_layer_idx(&x, i as u8, validate_finite)?;
    }

    // 6. Output projection
    let predicted = self.output_proj.forward(&x)?;
    debug_assert_eq!(predicted.dims(), &[batch, PANEL_DIM]);

    // 7. Output finiteness (always)
    ensure_finite(&predicted, NanSource::Output)?;

    Ok(PredictedPanel { data: predicted, batch_size: batch })
}

fn ensure_finite(t: &Tensor, source: NanSource) -&gt; Result&lt;(), PredictorError&gt; {
    let sum = t.abs()?.sum_all()?.to_scalar::&lt;f32&gt;()?;
    if !sum.is_finite() || sum.is_nan() {
        return Err(PredictorError::NanDetected { source, layer_id: None, tensor_name: None, sample_idx: None });
    }
    Ok(())
}

fn trainable_parameters(&amp;self) -&gt; Vec&lt;&amp;Var&gt; {
    debug_assert!(self.frozen_target_adapter.parameters().is_empty(),
        "REQ-PRED-09 violated: frozen_target_adapter has trainable params");
    let mut out = Vec::new();
    push_linear(&mut out, &self.input_proj);
    for layer in &self.layers {
        push_linear(&mut out, &layer.self_attn_qkv);
        push_linear(&mut out, &layer.self_attn_out);
        push_linear(&mut out, &layer.ffn_up);
        push_linear(&mut out, &layer.ffn_down);
        push_layer_norm(&mut out, &layer.attn_ln);
        push_layer_norm(&mut out, &layer.ffn_ln);
    }
    push_linear(&mut out, &self.output_proj);
    out.extend(self.oracle_head.trainable_parameters());
    out
}

fn architecture_summary(&amp;self) -&gt; ArchitectureSummary {
    ArchitectureSummary {
        num_layers: self.config.num_layers,
        hidden_dim: self.config.hidden_dim,
        num_heads: self.config.num_heads,
        ff_expansion: self.config.ff_expansion,
        activation: "GeLU",
        residual: true,
        layer_norm: true,
        panel_dim: PANEL_DIM,
        concat_input_dim: CONCAT_INPUT_DIM,
        gradient_checkpointing: self.config.gradient_checkpointing,
    }
}

// tests (cuda-gated):
//   shape_contract_b4
//   dim_mismatch_panel_t0_4096_returns_err
//   dim_mismatch_inconsistent_batch_returns_err  (panel_t0 batch=4, panel_t1 batch=2)
//   nan_input_with_dryrun_detected
//   determinism_byte_equal_two_forwards
//   trainable_parameters_count_matches_expected
//   trainable_parameters_excludes_frozen_target  (Var-pointer-equality assertion)
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/predictor/mod.rs">Add forward + forward_dryrun + forward_inner + trainable_parameters + architecture_summary; declare ArchitectureSummary struct; add 6+ unit tests; if file exceeds ~450 lines, split tests to `predictor/tests.rs` sibling</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">Re-export ArchitectureSummary if useful for FSV (TASK-PRED-011)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 26 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>forward with shape-correct inputs returns PredictedPanel of shape (B, 5120) bf16</criterion>
  <criterion>forward with panel_t0 shape (B, 4096) → Err(PredictorError::DimMismatch) with both observed shapes in context</criterion>
  <criterion>forward_dryrun with NaN-injected panel_t0 → Err(PredictorError::NanDetected{source: Input{panel: T0}, ..})</criterion>
  <criterion>Two forwards on same inputs + weights → byte-equal SHA-256 (REQ-PRED-05)</criterion>
  <criterion>trainable_parameters() count matches expected (deterministic enumeration order)</criterion>
  <criterion>trainable_parameters() does NOT contain any &amp;Var alias of frozen_target_adapter (REQ-PRED-09)</criterion>
  <criterion>architecture_summary() returns the canonical doc-09 §2.4 values</criterion>
  <criterion>File length stays &lt; 500 lines (split tests if needed)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>CUBLAS_WORKSPACE_CONFIG=:4096:8 cargo test --release -p context-graph-mejepa predictor</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
