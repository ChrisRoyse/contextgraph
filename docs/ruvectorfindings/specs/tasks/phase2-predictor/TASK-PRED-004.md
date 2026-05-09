# TASK-PRED-004 — TransformerLayer forward pass (MHA + GeLU FFN + residual + LN; bf16 with fp32 master weights)

```xml
<task_spec id="TASK-PRED-004" version="1.0">

<metadata>
  <title>Implement TransformerLayer::forward — pre-LN multi-head self-attention + GeLU FFN + residual connections, bf16 activations</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-PRED-02">6-layer transformer, hidden 1024, 8 heads, GeLU, residual + LayerNorm</item>
    <item ref="REQ-PRED-04">Layer-output finiteness validation (NaN/Inf detection)</item>
    <item ref="REQ-PRED-05">Determinism: eval-mode forward yields byte-equal output</item>
    <item ref="component:TransformerLayer">forward method body</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-001</task_ref>
    <task_ref>TASK-PRED-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified, 0 created</estimated_files>
</metadata>

<context>
TASK-PRED-001 declared the `TransformerLayer` struct fields (self_attn_qkv, self_attn_out, ffn_up,
ffn_down, attn_ln, ffn_ln, num_heads, head_dim, ff_expansion) and `new()` body that allocates the
candle Linear / LayerNorm modules. This task implements `forward(&self, x: &Tensor) -> Result<Tensor, PredictorError>` —
one pre-LN transformer block:

  x  → attn_ln → MHA(num_heads, head_dim) → +residual → ffn_ln → FFN(GeLU, expansion=4) → +residual → out

The block operates in bf16 (per constitution `<tech_stack>` and TECH-PREDICTOR `<note category="determinism">`)
on candle-cuda. fp32 master weights for AdamW are a Phase 3 concern (training); this task uses
candle's standard bf16 forward path. Activations are bf16; the `1.0 / sqrt(head_dim)` scaling on
attention scores is applied as f32 then cast back, to avoid bf16 underflow on small head_dim.

NaN/Inf detection at layer output is **opt-in** via a `validate_finite_in_dryrun: bool` flag on the
`forward` signature: in the hot training path the per-layer finiteness check is too expensive
(materializes a CPU-side scan); in dry-run / FSV mode it is enabled. The full-tensor finiteness
check is performed at predictor output by TASK-PRED-005.

Iron Rule observed: this task does NOT call `MeJepaPredictor::forward` (TASK-PRED-005) and does
NOT reference any data model from TASK-PRED-002 / 003 beyond `PredictorError` (already in TASK-PRED-001
and expanded in TASK-PRED-003).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (tech_stack candle 0.9.x; performance_budgets predictor_forward_p50; testing_requirements coverage 80%)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-02 architecture; REQ-PRED-04 finiteness; REQ-PRED-05 determinism; EC-PRED-03 first-forward Inf detection)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (data_models TransformerLayer; implementation_notes "determinism" — CUBLAS_WORKSPACE_CONFIG; "vicreg-formula" pre-LN convention)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-001.md (struct skeleton + new() body)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (PredictorError::NanDetected with NanSource::Layer)</file>
  <file purpose="reference_impl">candle/candle-transformers/src/models/llama.rs (HEAD pinned via Cargo.toml; pre-LN block reference for attention scaling)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/predictor/transformer_layer.rs (struct + new shipped by TASK-PRED-001)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001 + TASK-PRED-003 complete</check>
  <check>cargo build --release -p context-graph-mejepa succeeds before this task</check>
  <check>RTX 5090 with sm_120a available for tests (gated by `#[cfg(feature = "cuda")]`)</check>
  <check>CUBLAS_WORKSPACE_CONFIG=:4096:8 set in test harness (for determinism per TECH-PREDICTOR notes)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `TransformerLayer::forward(&self, x: &Tensor, validate_finite_in_dryrun: bool) -> Result<Tensor, PredictorError>`
    - Pre-LN ordering: LN → MHA → residual; LN → FFN → residual
    - Multi-head self-attention: split QKV via single Linear(hidden, 3*hidden), reshape to (B, num_heads, T=1, head_dim); the input is a single stacked panel (no token sequence axis), so T=1; attention reduces to a learned per-token transformation of shape (B, hidden)
    - Scaling: `scores = (q @ k.T) / sqrt(head_dim)` computed in f32 to avoid bf16 underflow, cast back to bf16 before softmax
    - Softmax over the (single-token) attention axis → emits identity-like attention but the projection still trains
    - Output projection: self_attn_out: Linear(hidden, hidden)
    - Residual: `x_post_attn = x + attn_out` (in bf16)
    - LayerNorm before FFN
    - FFN: ffn_up Linear(hidden, 4*hidden) → GeLU → ffn_down Linear(4*hidden, hidden); residual added
    - When `validate_finite_in_dryrun=true`: full-tensor finiteness check on the layer output; raise `PredictorError::NanDetected { source: Layer, layer_id: Some(self_layer_idx_passed_in), ... }` on violation
    - Add a `pub(crate) fn forward_with_layer_idx(&self, x: &Tensor, layer_idx: u8, validate_finite: bool) -> Result<Tensor, PredictorError>` so the predictor can label NaN by layer id
    - Add 4 unit tests on cuda: (a) shape-preserving forward (B=2 hidden=64 → output (B=2, hidden=64)); (b) finiteness in nominal case; (c) NaN-input → NaN-output detected when validate_finite=true; (d) two consecutive forwards on same input give byte-equal output (determinism)
  </in_scope>
  <out_of_scope>
    - MeJepaPredictor::forward end-to-end (TASK-PRED-005)
    - Cross-layer composition / layer count = 6 (TASK-PRED-005)
    - Gradient checkpointing (Phase 2 default off; flag handled in TASK-PRED-005)
    - VRAM measurement during forward (TASK-PRED-010)
    - VICReg loss (TASK-PRED-007)
    - Frozen target encoding (TASK-PRED-006)
    - bf16-vs-fp32 master weight separation for AdamW (Phase 3 concern; this task uses candle bf16 weights directly)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/predictor/transformer_layer.rs">
use candle_core::{DType, Tensor};

use crate::error::{NanSource, PredictorError};

impl TransformerLayer {
    /// Public forward. `validate_finite_in_dryrun` enables expensive finiteness scan.
    pub fn forward(
        &amp;self,
        x: &amp;Tensor,
        validate_finite_in_dryrun: bool,
    ) -&gt; Result&lt;Tensor, PredictorError&gt;;

    /// Internal — accepts the layer index so NanDetected can be attributed correctly.
    pub(crate) fn forward_with_layer_idx(
        &amp;self,
        x: &amp;Tensor,
        layer_idx: u8,
        validate_finite_in_dryrun: bool,
    ) -&gt; Result&lt;Tensor, PredictorError&gt;;

    /// Returns (num_heads, head_dim, ff_inner_dim) — for assertions in tests.
    pub fn architecture(&amp;self) -&gt; (u8, u32, u32);
}
    </signature>
  </signatures>
  <constraints>
    - Forward operates on input of shape `(batch, hidden_dim)` — NO token-sequence axis (the input is a single stacked panel; cross-token attention degenerates to identity but learned projection still trains)
    - All Linear layers are bias=true (default candle_nn::linear); LayerNorm has affine=true
    - Pre-LN ordering: LayerNorm BEFORE attention/FFN (modern transformer practice; attribution: TECH-PREDICTOR data_models TransformerLayer "Pre-LN per modern transformer practice")
    - GeLU activation EXACT (not GeLU-approximate): use `candle_nn::ops::gelu` (or its `gelu_erf` variant if performance matters; both are acceptable; same function chosen everywhere across the 6 layers for determinism)
    - Attention scaling computed in f32: `scores = scores.to_dtype(DType::F32)? * (1.0 / (head_dim as f32).sqrt()); scores = scores.to_dtype(DType::BF16)?`
    - Softmax axis: the last dim of the (B, num_heads, T=1, T=1) score tensor — degenerate but mathematically valid
    - Residual add: `x.broadcast_add(&attn_out)?` or equivalent shape-correct candle op
    - When `validate_finite_in_dryrun = true`:
        * Sum the absolute layer output and convert to scalar; if `!sum.is_finite()` OR `sum.is_nan()`, raise NanDetected with source=Layer, layer_id=Some(layer_idx)
        * Use `tensor.abs()?.sum_all()?.to_scalar::<f32>()?` for the scalar check
    - When `validate_finite_in_dryrun = false`: NO finiteness check (hot path); the predictor's output check (TASK-PRED-005) is the safety net
    - Errors: any candle_core::Error converts to PredictorError::Candle via #[from]
    - Determinism: ensure no `Tensor::sum_all` or `Tensor::mean` with non-deterministic reduction order in the forward path; prefer `matmul` and `softmax` which candle implements deterministically with CUBLAS_WORKSPACE_CONFIG set
    - `architecture()` returns (num_heads, head_dim, ff_inner = ff_expansion * hidden_dim)
    - File length must remain < 500 lines (constitution `<file_organization>`); if approaching, split FFN to a sibling file
    - No `unwrap()` outside #[cfg(test)]; all candle errors propagate via `?`
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 20 tests passing (cumulative; this task adds ~4)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Test on cuda (gated by `#[cfg(feature = "cuda")]`): forward of (B=2, hidden=64) returns Tensor of shape (B=2, hidden=64) with bf16 dtype
    - Test: NaN-input + validate_finite_in_dryrun=true → Err(PredictorError::NanDetected{source: Layer, layer_id: Some(_), ..})
    - Test: two consecutive forwards on same input + same weights yield byte-equal SHA-256 output
  </verification>
</definition_of_done>

<pseudo_code>
fn forward(&amp;self, x: &amp;Tensor, validate_finite_in_dryrun: bool) -&gt; Result&lt;Tensor, PredictorError&gt; {
    self.forward_with_layer_idx(x, 0, validate_finite_in_dryrun)
}

fn forward_with_layer_idx(&amp;self, x: &amp;Tensor, layer_idx: u8, validate_finite_in_dryrun: bool) -&gt; Result&lt;Tensor, PredictorError&gt; {
    // x: (batch, hidden_dim) bf16

    // ----- Self-attention sub-block (pre-LN) -----
    let normed = self.attn_ln.forward(x)?;                          // (B, hidden)
    let qkv = self.self_attn_qkv.forward(&amp;normed)?;                 // (B, 3*hidden)
    let (batch, _) = (qkv.dims()[0], qkv.dims()[1]);
    let head_dim = self.head_dim as usize;
    let num_heads = self.num_heads as usize;

    // Reshape to (B, 3, num_heads, head_dim) and split
    let qkv = qkv.reshape((batch, 3, num_heads, head_dim))?;
    let q = qkv.get_on_dim(1, 0)?;                                   // (B, num_heads, head_dim)
    let k = qkv.get_on_dim(1, 1)?;
    let v = qkv.get_on_dim(1, 2)?;

    // Add singleton T=1 axis: (B, num_heads, T=1, head_dim)
    let q = q.unsqueeze(2)?;
    let k = k.unsqueeze(2)?;
    let v = v.unsqueeze(2)?;

    // Scores = q @ k.T / sqrt(head_dim) — compute scaling in f32 then cast back
    let scores = q.matmul(&amp;k.transpose(2, 3)?)?;                     // (B, num_heads, 1, 1)
    let scale = 1.0_f32 / (head_dim as f32).sqrt();
    let scores = scores.to_dtype(DType::F32)?.affine(scale as f64, 0.0)?.to_dtype(x.dtype())?;
    let attn = candle_nn::ops::softmax(&amp;scores, /*dim=*/3)?;
    let out = attn.matmul(&amp;v)?.squeeze(2)?;                          // (B, num_heads, head_dim)
    let out = out.reshape((batch, num_heads * head_dim))?;           // (B, hidden)
    let attn_out = self.self_attn_out.forward(&amp;out)?;
    let x = x.add(&amp;attn_out)?;                                       // residual

    // ----- FFN sub-block (pre-LN) -----
    let normed = self.ffn_ln.forward(&amp;x)?;
    let up = self.ffn_up.forward(&amp;normed)?;
    let gelu = candle_nn::ops::gelu(&amp;up)?;                           // EXACT GeLU
    let down = self.ffn_down.forward(&amp;gelu)?;
    let out = x.add(&amp;down)?;

    if validate_finite_in_dryrun {
        let sum = out.abs()?.sum_all()?.to_scalar::&lt;f32&gt;()?;
        if !sum.is_finite() || sum.is_nan() {
            return Err(PredictorError::NanDetected {
                source: NanSource::Layer,
                layer_id: Some(layer_idx),
                tensor_name: Some("layer_output"),
                sample_idx: None,
            });
        }
    }

    Ok(out)
}

fn architecture(&amp;self) -&gt; (u8, u32, u32) {
    let ff_inner = (self.ff_expansion as u32) * (self.head_dim * self.num_heads as u32);
    (self.num_heads, self.head_dim, ff_inner)
}

// tests (gated by #[cfg(feature = "cuda")]):
//   shape_preserving_forward_b2_h64: input (2, 64) bf16 cuda → output (2, 64) bf16
//   nominal_finiteness: validate_finite_in_dryrun=true; nominal random input → Ok
//   nan_detection_layer: input contains NaN; validate_finite_in_dryrun=true → Err(NanDetected{source: Layer})
//   determinism_byte_equal: two forwards with identical inputs+weights → same SHA-256 of bf16 bytes
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/predictor/transformer_layer.rs">Add forward + forward_with_layer_idx + architecture method bodies; add 4 unit tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa (cuda feature gated) shows >= 20 tests passing (this task adds 4)</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>TransformerLayer::forward preserves shape (B, hidden) → (B, hidden) bf16</criterion>
  <criterion>NaN-input with validate_finite_in_dryrun=true → Err(PredictorError::NanDetected{source: Layer, layer_id: Some(_), ..})</criterion>
  <criterion>Two consecutive forwards on same input + weights → byte-equal SHA-256 output</criterion>
  <criterion>GeLU activation chosen consistently (gelu OR gelu_erf, same across all layers)</criterion>
  <criterion>Pre-LN ordering: LayerNorm BEFORE attention and BEFORE FFN</criterion>
  <criterion>File length &lt; 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>CUBLAS_WORKSPACE_CONFIG=:4096:8 cargo test --release -p context-graph-mejepa transformer_layer</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
