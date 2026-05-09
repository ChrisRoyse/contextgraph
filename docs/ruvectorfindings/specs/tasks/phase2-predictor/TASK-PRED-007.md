# TASK-PRED-007 — VICReg loss components + combined `vicreg_loss(predicted, target, lambdas) -> LossOutputs`

```xml
<task_spec id="TASK-PRED-007" version="1.0">

<metadata>
  <title>VICReg loss: L_predict (Huber δ=1) + L_variance (γ-floor) + L_covariance (off-diagonal) + L_invariance (MSE) + L_total combiner + VarianceFloorHistory</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-PRED-06">L_total = L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance with doc 09 §2.4 defaults</item>
    <item ref="REQ-PRED-07">LossOutputs four-term decomposition exposed as struct fields</item>
    <item ref="REQ-PRED-08">VarianceFloorHistory + MEJEPA_PRED_VICREG_DEGENERATE diagnostic on consecutive-pass debounce</item>
    <item ref="REQ-PRED-16">LossOutputs.l_predict public (SPEC-UTML compute_delta_p contract)</item>
    <item ref="REQ-PRED-18">Boundary validation: dim, finite, batch, lambdas</item>
    <item ref="component:VicregLoss">vicreg_loss + huber_loss_delta_one + sub-functions</item>
    <item ref="component:VarianceFloorHistory">record_pass + diagnostic-firing logic</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-003</task_ref>
    <task_ref>TASK-PRED-005</task_ref>
    <task_ref>TASK-PRED-006</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 modified, 1 modified (mod.rs)</estimated_files>
</metadata>

<context>
This task implements the four-term VICReg decomposition + total loss combiner, plus the
`VarianceFloorHistory` ring buffer for the MEJEPA_PRED_VICREG_DEGENERATE early-warning diagnostic.

The function signature is `vicreg_loss(predicted: &Tensor, target: &Tensor, lambdas: VicregLambdas) -> Result<LossOutputs, LossError>`,
returning the four scalar f32 components plus the combined `l_total`. The four sub-functions —
`huber_loss_delta_one`, `vicreg_variance_term`, `vicreg_covariance_term`, `vicreg_invariance_term`
— are also exposed as `pub(crate)` for test isolation.

VICReg-specific math (per Bardes et al. 2022, clean-room reimplementation; cite paper, no code copied):

  L_predict   = Huber_δ=1(predicted - target).mean()
  L_variance  = mean over panel_dim D of max(0, γ − std_batch(predicted_d))
              where std_batch is along the batch axis with ddof=1 ε-stabilized
  L_covariance= sum over off-diagonal C(z)[i,j]^2  / panel_dim
              where C(z) = (predicted - mean_batch).T @ (predicted - mean_batch) / (batch - 1)
  L_invariance= MSE(predicted, target).mean()

  L_total     = L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance

Memory: the covariance matrix is `panel_dim × panel_dim = 5120 × 5120 = 26.2M entries`. At bf16
that's ~52 MB; at f32 (we compute centered and matmul in f32 for numerical stability) it's
~105 MB. Acceptable per the predictor's 5 GB activation budget.

The variance term is computed in f32 (bf16 std is unstable for small variances). We cast back
to f32 scalar for return; LossOutputs.fields are all f32.

Iron Rule: this task does NOT call MeJepaPredictor::forward (TASK-PRED-005 already shipped),
does NOT call gradient hook (TASK-PRED-009), and does NOT write FSV files (TASK-PRED-011).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; performance_budgets; provenance — VICReg cited as paper, no code copied)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-06, 07, 08, 16, 18; AC-PRED-02-01..03; EC-PRED-05 batch=1 covariance; ERR-PRED-02 NaN; ERR-PRED-05 VicregDegenerate)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (api_contracts vicreg_loss + huber_loss_delta_one; component_contracts VicregLoss + VarianceFloorHistory; implementation_notes "vicreg-formula", "lambda-defaults")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (VicregLambdas + LossOutputs + LossError taxonomy)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-005.md (PredictedPanel — but vicreg_loss takes &amp;Tensor directly per REQ-PRED-07 signature)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-006.md (TargetPanel — same; vicreg_loss takes &amp;Tensor directly)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/loss/{mod.rs, variance_floor.rs} (placeholders shipped TASK-PRED-001)</file>
  <file purpose="reference_paper">VICReg: Bardes, Ponce, LeCun 2022 — variance/covariance/invariance regularization (cite, do NOT copy code)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-003 + TASK-PRED-005 + TASK-PRED-006 complete</check>
  <check>cargo build --release -p context-graph-mejepa succeeds before this task</check>
  <check>Loss module structure: `src/loss/mod.rs` declares `pub mod vicreg; pub mod variance_floor;`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/loss/vicreg.rs` with: `huber_loss_delta_one`, `vicreg_variance_term`, `vicreg_covariance_term`, `vicreg_invariance_term`, `vicreg_loss`
    - Replace placeholder `VarianceFloorHistory` in `loss/variance_floor.rs` with: ring buffer of last `window` flags; `record_pass(low_variance_dim_count, total_dims) -> Result<(), PredictorError>`; consecutive-True detection raises `VicregDegenerate`
    - `vicreg_loss` validates: predicted/target same shape (batch, 5120); finite; batch ≥ 2 (else `BatchTooSmall`, computed-as-0 covariance per EC-PRED-05); lambdas finite
    - Compute on candle tensors; reduce to f32 scalars at the end
    - Return `LossOutputs { l_predict, l_variance, l_covariance, l_invariance, l_total }`
    - Add 12+ unit tests covering: huber identity-target → ~0; variance zero predictor → l_variance ≈ γ; covariance identity-cov → ~0; combined formula matches sum within 1e-3; dim mismatch error; NaN-input error; batch=1 batch_too_small but proceeds with l_covariance=0; non-finite lambdas error; VarianceFloorHistory triggers after 2 consecutive degenerate passes; VarianceFloorHistory does NOT trigger after 1 pass
  </in_scope>
  <out_of_scope>
    - Calling vicreg_loss from MeJepaPredictor (the training loop in Phase 3 owns the call site)
    - Backward pass / gradient flow setup (Phase 3)
    - FSV evidence writing (TASK-PRED-011)
    - MEJEPA_PRED_FROZEN_TARGET_GRAD detection (TASK-PRED-009)
    - VRAM measurement around the loss (TASK-PRED-010)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/loss/vicreg.rs">
use candle_core::{DType, Tensor};

use crate::config::PANEL_DIM;
use crate::error::LossError;
use crate::evidence::{LossOutputs, VicregLambdas};

/// L_predict: Huber loss δ=1 of (predicted - target), batch-mean.
pub(crate) fn huber_loss_delta_one(residual: &amp;Tensor) -&gt; Result&lt;f32, LossError&gt;;

/// L_variance: mean over feature dim D of max(0, γ - std_batch(predicted_d)).
/// Returns (l_variance: f32, low_variance_dim_count: usize).
pub(crate) fn vicreg_variance_term(predicted: &amp;Tensor, gamma: f32) -&gt; Result&lt;(f32, usize), LossError&gt;;

/// L_covariance: sum over off-diagonal C(z)[i,j]² / D, where C(z) = X^T X / (B-1) on centered X.
/// Computed in f32 for numerical stability.
pub(crate) fn vicreg_covariance_term(predicted: &amp;Tensor) -&gt; Result&lt;f32, LossError&gt;;

/// L_invariance: MSE(predicted, target).mean()
pub(crate) fn vicreg_invariance_term(predicted: &amp;Tensor, target: &amp;Tensor) -&gt; Result&lt;f32, LossError&gt;;

/// VICReg total loss: L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance.
/// Returns LossOutputs with the four-term decomposition.
///
/// Per REQ-PRED-06: defaults λ_var=25, λ_cov=1, λ_inv=25, γ=1.
/// Per REQ-PRED-08: also returns low_variance_dim_count via a side channel; callers wishing
/// to flag the variance-floor diagnostic should call `vicreg_loss_with_variance_count`.
pub fn vicreg_loss(
    predicted: &amp;Tensor,
    target: &amp;Tensor,
    lambdas: VicregLambdas,
) -&gt; Result&lt;LossOutputs, LossError&gt;;

/// Same as vicreg_loss but returns (LossOutputs, low_variance_dim_count) for VarianceFloorHistory.
pub fn vicreg_loss_with_variance_count(
    predicted: &amp;Tensor,
    target: &amp;Tensor,
    lambdas: VicregLambdas,
) -&gt; Result&lt;(LossOutputs, usize), LossError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/loss/variance_floor.rs">
use std::collections::VecDeque;

use crate::config::{VICREG_DEGENERATE_CONSECUTIVE_PASSES, VICREG_DEGENERATE_DIM_FRACTION};
use crate::error::PredictorError;

pub struct VarianceFloorHistory {
    window: usize,
    history: VecDeque&lt;bool&gt;,
}

impl VarianceFloorHistory {
    pub fn new(window: usize) -&gt; Self;

    /// Push (low_variance_dim_count > VICREG_DEGENERATE_DIM_FRACTION * total_dims) into the ring.
    /// If the ring is full and every entry is `true`, return Err(VicregDegenerate).
    pub fn record_pass(
        &amp;mut self,
        low_variance_dim_count: usize,
        total_dims: usize,
        gamma: f32,
    ) -&gt; Result&lt;(), PredictorError&gt;;

    pub fn is_full(&amp;self) -&gt; bool;
    pub fn current_history(&amp;self) -&gt; &amp;VecDeque&lt;bool&gt;;
    pub fn clear(&amp;mut self);
}
    </signature>
  </signatures>
  <constraints>
    - **vicreg_loss validation order** (fail-closed at first failure):
        1. lambdas.validate_finite() — else `LossError::NonFiniteLambda`
        2. predicted.dims() == target.dims() == [batch, PANEL_DIM] — else `LossError::DimMismatch`
        3. ensure_finite(predicted, "predicted"); ensure_finite(target, "target") — else `LossError::NanDetected{component}`
        4. batch >= 2 — if not, log + treat l_covariance = 0 (per EC-PRED-05); set `LossError::BatchTooSmall` is **NOT** raised because EC-PRED-05 says "warn but continue"; instead emit a `tracing::warn!` and proceed with l_covariance=0
    - **Huber loss δ=1**:
        residual = predicted - target
        |residual| < 1: 0.5 * residual²
        |residual| ≥ 1: |residual| - 0.5
        return mean over all elements
    - **Variance term**:
        Cast to f32 for numerical stability
        mean_batch = predicted.mean_keepdim(0)?  → shape (1, D)
        centered = predicted - mean_batch       → shape (B, D)
        # Use unbiased estimator with epsilon stabilizer:
        var_per_dim = (centered.pow(2)?.sum_keepdim(0)? / (batch - 1) as f64)?.add_scalar(1e-4)?
        std_per_dim = var_per_dim.sqrt()?       → shape (1, D)
        Compute below_floor: count where std_per_dim < gamma
        l_variance = (gamma - std_per_dim).clamp(0.0, gamma).mean_all()?.to_scalar::<f32>()?
        return (l_variance, below_floor_count)
    - **Covariance term**:
        Cast to f32 for stability
        mean_batch = predicted.mean_keepdim(0)?
        centered = predicted - mean_batch       → shape (B, D)
        cov = centered.t()?.matmul(&centered)?.affine(1.0 / (batch - 1) as f64, 0.0)?  → shape (D, D)
        # zero out diagonal: cov - cov * eye(D)
        # Implementation: build an eye mask once, multiply elementwise (cov * mask) where mask is 1 off-diagonal, 0 diagonal
        off_diag = cov * (Tensor::ones_like(&cov)? - Tensor::eye(D, DType::F32, device)?)
        l_cov = off_diag.pow(2)?.sum_all()?.to_scalar::<f32>()? / PANEL_DIM as f32
        return l_cov
    - **Invariance term**:
        l_inv = (predicted - target).pow(2)?.mean_all()?.to_scalar::<f32>()?
    - **Total**:
        l_total = l_predict + lambdas.var * l_variance + lambdas.cov * l_covariance + lambdas.inv * l_invariance
        Validate every component is finite before returning; on NaN: `LossError::NanDetected { component, detail }` (component is "l_predict"|"l_variance"|"l_covariance"|"l_invariance"|"l_total")
    - **PANEL_DIM check**: predicted.dims()[1] == PANEL_DIM (5120) hard-asserted; covers REQ-PRED-18
    - **VarianceFloorHistory.window** = `VICREG_DEGENERATE_CONSECUTIVE_PASSES` (= 2 by default per TASK-PRED-001 const)
    - **record_pass logic**:
        flag = (low_variance_dim_count as f32 / total_dims as f32) > VICREG_DEGENERATE_DIM_FRACTION
        history.push_back(flag); while history.len() > window: history.pop_front()
        if history.len() == window AND every entry is true: return Err(VicregDegenerate{ low_variance_dim_count, total_dims, gamma, consecutive_passes: window as u8 })
        else Ok(())
    - **Constants** (REQ-PRED-06 + 08):
        VICREG_DEGENERATE_DIM_FRACTION = 0.5 (50% threshold)
        VICREG_DEGENERATE_CONSECUTIVE_PASSES = 2 (debounce)
    - File length stays &lt; 500 lines per file (constitution)
    - No `unwrap()` in non-test code
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 45 tests passing (this task adds ~12)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Test: predicted == target → l_predict ≈ 0, l_invariance ≈ 0
    - Test: predicted = zeros → l_variance ≈ γ = 1.0 (per AC-PRED-02-02)
    - Test: l_total formula check: |l_total - (l_predict + 25*l_var + 1*l_cov + 25*l_inv)| < 1e-3
    - Test: dim-wrong predicted → Err(LossError::DimMismatch)
    - Test: NaN-injected predicted → Err(LossError::NanDetected{component: "predicted"})
    - Test: NaN lambdas → Err(LossError::NonFiniteLambda)
    - Test: batch=1 → l_covariance == 0 (warn-not-error per EC-PRED-05)
    - Test: VarianceFloorHistory does NOT fire after 1 degenerate pass
    - Test: VarianceFloorHistory fires after 2 consecutive degenerate passes → Err(VicregDegenerate)
  </verification>
</definition_of_done>

<pseudo_code>
loss/vicreg.rs:
    fn huber_loss_delta_one(residual)
        abs = residual.abs()?
        sqr_part = (residual.sqr()?.affine(0.5, 0.0))?
        lin_part = abs.affine(1.0, -0.5)?
        // where |residual| &lt; 1: sqr_part; else lin_part
        mask_lt = abs.lt(1.0)?
        out = mask_lt.where_cond(&sqr_part, &lin_part)?
        out.mean_all()?.to_scalar::&lt;f32&gt;()

    fn vicreg_variance_term(predicted, gamma)
        let p32 = predicted.to_dtype(F32)?;
        let batch = p32.dims()[0] as f64;
        let mean = p32.mean_keepdim(0)?;
        let centered = p32.broadcast_sub(&mean)?;
        let var = centered.sqr()?.sum_keepdim(0)?.affine(1.0/(batch - 1.0), 0.0)?.affine(1.0, 1e-4)?;
        let std = var.sqrt()?;
        let below_count = std.lt(gamma as f32)?.to_dtype(F32)?.sum_all()?.to_scalar::&lt;f32&gt;()? as usize;
        let l_var = (Tensor::new(gamma, std.device())?.broadcast_sub(&std)?).clamp(0.0, gamma as f64)?.mean_all()?.to_scalar::&lt;f32&gt;()?;
        Ok((l_var, below_count))

    fn vicreg_covariance_term(predicted)
        let p32 = predicted.to_dtype(F32)?;
        let batch = p32.dims()[0] as f64;
        let mean = p32.mean_keepdim(0)?;
        let centered = p32.broadcast_sub(&mean)?;
        let cov = centered.t()?.matmul(&centered)?.affine(1.0/(batch - 1.0), 0.0)?;
        let dev = cov.device();
        let mask = Tensor::ones_like(&cov)?.sub(&Tensor::eye(PANEL_DIM, F32, dev)?)?;
        let off = cov.mul(&mask)?;
        let l_cov = off.sqr()?.sum_all()?.to_scalar::&lt;f32&gt;()? / PANEL_DIM as f32;
        Ok(l_cov)

    fn vicreg_invariance_term(predicted, target)
        let diff = predicted.sub(target)?;
        diff.sqr()?.mean_all()?.to_scalar::&lt;f32&gt;()

    fn vicreg_loss_with_variance_count(predicted, target, lambdas)
        lambdas.validate_finite()?;
        if predicted.dims() != target.dims(): return Err(DimMismatch);
        if predicted.dims().len() != 2 || predicted.dims()[1] != PANEL_DIM: return Err(DimMismatch);
        let batch = predicted.dims()[0];
        ensure_finite_loss(predicted, "predicted")?;
        ensure_finite_loss(target, "target")?;

        let l_predict = huber_loss_delta_one(&predicted.sub(target)?)?;
        let (l_variance, below_count) = vicreg_variance_term(predicted, lambdas.gamma)?;
        let l_covariance = if batch &gt;= 2 {
            vicreg_covariance_term(predicted)?
        } else {
            tracing::warn!("vicreg: batch=1, covariance term forced to 0 per EC-PRED-05");
            0.0
        };
        let l_invariance = vicreg_invariance_term(predicted, target)?;
        let l_total = l_predict
            + lambdas.var * l_variance
            + lambdas.cov * l_covariance
            + lambdas.inv * l_invariance;
        let outputs = LossOutputs { l_predict, l_variance, l_covariance, l_invariance, l_total };
        outputs.validate_finite()?;
        Ok((outputs, below_count))

    fn vicreg_loss(predicted, target, lambdas)
        let (outputs, _) = vicreg_loss_with_variance_count(predicted, target, lambdas)?;
        Ok(outputs)

loss/variance_floor.rs:
    impl VarianceFloorHistory::new(window) -&gt; Self { Self { window, history: VecDeque::new() } }
    impl record_pass(low_count, total, gamma)
        let flag = (low_count as f32 / total as f32) &gt; VICREG_DEGENERATE_DIM_FRACTION;
        self.history.push_back(flag);
        while self.history.len() &gt; self.window: self.history.pop_front();
        if self.history.len() == self.window && self.history.iter().all(|&b| b) {
            return Err(PredictorError::VicregDegenerate {
                low_variance_dim_count: low_count, total_dims: total, gamma,
                consecutive_passes: self.window as u8,
            });
        }
        Ok(())

tests:
    huber_residual_zero_returns_zero
    huber_residual_one_returns_half
    huber_residual_three_returns_two_point_five
    variance_zero_predictor_returns_gamma  (predicted = zeros)
    variance_unit_predictor_returns_zero   (predicted = unit gaussian, large batch)
    covariance_identity_returns_zero       (orthogonal columns)
    invariance_predicted_eq_target_returns_zero
    vicreg_total_formula_check_within_tolerance
    vicreg_dim_mismatch_returns_err
    vicreg_nan_predicted_returns_err
    vicreg_non_finite_lambdas_returns_err
    vicreg_batch_one_l_covariance_is_zero
    variance_floor_history_no_trigger_after_one_pass
    variance_floor_history_triggers_after_two_consecutive_passes
    variance_floor_history_resets_with_clear
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/loss/vicreg.rs">VICReg loss with 4 sub-functions + vicreg_loss + vicreg_loss_with_variance_count + 12 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/loss/variance_floor.rs">Replace placeholder with full VarianceFloorHistory: ring buffer + record_pass + diagnostic firing; 3 tests</file>
  <file path="crates/context-graph-mejepa/src/loss/mod.rs">Export `pub mod vicreg; pub mod variance_floor;` and `pub use {vicreg::vicreg_loss, vicreg::vicreg_loss_with_variance_count, variance_floor::VarianceFloorHistory};`</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">Add `pub use loss::{vicreg_loss, vicreg_loss_with_variance_count, VarianceFloorHistory};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 45 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>predicted == target → l_predict, l_invariance both ≈ 0 (within 1e-3)</criterion>
  <criterion>predicted = zeros, batch ≥ 2 → l_variance ≈ γ = 1.0 ± 0.05 (AC-PRED-02-02)</criterion>
  <criterion>Formula check: |l_total - (l_predict + 25·l_var + 1·l_cov + 25·l_inv)| &lt; 1e-3 (REQ-PRED-06)</criterion>
  <criterion>NaN inputs raise LossError::NanDetected with the offending component name</criterion>
  <criterion>Non-finite lambdas raise LossError::NonFiniteLambda</criterion>
  <criterion>batch=1 logs a warning and returns l_covariance == 0 (EC-PRED-05)</criterion>
  <criterion>VarianceFloorHistory fires only after `window` consecutive degenerate passes (REQ-PRED-08 debounce)</criterion>
  <criterion>LossOutputs.l_predict is the public field consumed by SPEC-UTML compute_delta_p (REQ-PRED-16; visual check)</criterion>
  <criterion>Each loss file is &lt; 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa loss::</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
