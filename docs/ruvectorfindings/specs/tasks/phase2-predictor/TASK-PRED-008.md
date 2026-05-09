# TASK-PRED-008 — OracleHead::predict_logits + predict_probabilities (Linear 5120 → num_tests+1 + softmax)

```xml
<task_spec id="TASK-PRED-008" version="1.0">

<metadata>
  <title>Implement OracleHead forward bodies: predict_logits (Linear projection) + predict_probabilities (softmax over per-test+exception axis)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-PRED-12">Linear(5120, num_tests + 1) projection + softmax for per-test pass probability</item>
    <item ref="REQ-PRED-04">Output finiteness validation (no NaN/Inf escape)</item>
    <item ref="REQ-PRED-18">Boundary validation: dim, finite</item>
    <item ref="component:OracleHead">predict_logits + predict_probabilities bodies</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-002</task_ref>
    <task_ref>TASK-PRED-003</task_ref>
    <task_ref>TASK-PRED-005</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 modified</estimated_files>
</metadata>

<context>
TASK-PRED-002 declared `OracleHead::new` (Linear allocation + bounds-check on num_tests) and the
method signatures for `predict_logits` and `predict_probabilities` with `unimplemented!()` bodies.
This task fills those bodies.

`predict_logits(&self, predicted: &Tensor) -> Result<OracleLogits, PredictorError>`:
  1. Validate `predicted.dims() == [batch, PANEL_DIM]`.
  2. Validate predicted is finite (sample-based scan; full scan is for FSV).
  3. Apply `self.project_to_logits.forward(predicted)?` → shape (batch, num_tests + 1).
  4. Validate output finiteness.
  5. Return OracleLogits { logits, num_tests }.

`predict_probabilities(&self, logits: &OracleLogits) -> Result<Tensor, PredictorError>`:
  1. Validate logits.logits.dims()[1] == self.num_tests + 1 (the head's recorded size).
  2. Apply `candle_nn::ops::softmax(&logits.logits, /*dim=*/1)?` over the per-test+exception axis.
  3. Validate row sums in dry-run only (skip in hot path); in this task, **validate always**
     because the cost is small (one reduction) and the safety is large.
  4. Return the probability tensor.

The "+1" slot is the dedicated EXCEPTION class per doc 09 §2.5. Phase 4 conformal calibration
consumes these probabilities; Phase 2 just emits them.

Iron Rule: this task does NOT call MeJepaPredictor::forward (that's the predictor's
responsibility); it operates on a `&Tensor` predicted panel handed in by the caller.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_INFER_DIM_MISMATCH)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-12; AC-PRED-04-01..02; EC-PRED-07 num_tests sizing)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (api_contracts OracleHead::predict_logits / predict_probabilities; component_contracts OracleHead; implementation_notes "oracle-head-sizing")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-002.md (OracleHead struct + new + signature stubs)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (PredictorError taxonomy)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-005.md (PredictedPanel / NanSource sources)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/oracle_head.rs (struct + new + signature stubs from TASK-PRED-002)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-002 + TASK-PRED-003 + TASK-PRED-005 complete</check>
  <check>cargo build --release -p context-graph-mejepa succeeds</check>
  <check>OracleHead::new is implemented with project_to_logits Linear(5120, num_tests + 1)</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace `unimplemented!()` body of `predict_logits` with full impl (validation + Linear forward + finiteness check)
    - Replace `unimplemented!()` body of `predict_probabilities` with full impl (validation + softmax + row-sum check)
    - Add `pub fn num_outputs(&self) -> usize` accessor returning `self.num_tests + 1`
    - Add 6 unit tests (cuda-gated): correct-shape input → (B, T+1) output; wrong-dim input → DimMismatch; NaN-input → NanDetected; logits with wrong T+1 → DimMismatch; softmax row-sum within 1e-5 of 1.0; softmax values ∈ [0, 1]
  </in_scope>
  <out_of_scope>
    - Conformal calibration on the probabilities (Phase 4)
    - Per-instance test subsetting (Phase 4 — at inference, not at the head's forward)
    - Backward pass through the head (Phase 3 training loop registers the Vars via OracleHead::trainable_parameters from TASK-PRED-002)
    - FSV evidence writing (TASK-PRED-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/oracle_head.rs">
use candle_core::Tensor;

use crate::config::PANEL_DIM;
use crate::data_models::OracleLogits;
use crate::error::{NanSource, PanelSide, PredictorError};

impl OracleHead {
    pub fn predict_logits(&amp;self, predicted: &amp;Tensor) -&gt; Result&lt;OracleLogits, PredictorError&gt;;

    pub fn predict_probabilities(&amp;self, logits: &amp;OracleLogits) -&gt; Result&lt;Tensor, PredictorError&gt;;

    pub fn num_outputs(&amp;self) -&gt; usize { self.num_tests + 1 }
}
    </signature>
  </signatures>
  <constraints>
    - **predict_logits validation order** (fail-closed at first failure):
        1. predicted.dims() must be 2; predicted.dims()[1] == PANEL_DIM (5120) — else `PredictorError::DimMismatch { detail: "oracle_head input wrong dim", observed, expected_panel_dim: PANEL_DIM }`
        2. ensure_finite(predicted, source=Input{Predicted}) — else `PredictorError::NanDetected{source: Input{panel: PanelSide::Predicted}, ...}`
        3. logits = self.project_to_logits.forward(predicted)?
        4. Validate logits.dims() == [batch, num_tests + 1] (defense-in-depth; if Linear is correctly sized this always holds)
        5. ensure_finite(&logits, source=Output)
        6. Return OracleLogits { logits, num_tests: self.num_tests }
    - **predict_probabilities validation order**:
        1. logits.logits.dims()[1] == self.num_tests + 1 — else `PredictorError::DimMismatch { detail: "oracle logits wrong T+1", ... }`
        2. probs = candle_nn::ops::softmax(&logits.logits, /*dim=*/1)?
        3. ensure_finite(&probs, source=Output)
        4. Validate row sums in [1.0 - 1e-5, 1.0 + 1e-5] (per AC-PRED-04-02): cast to f32, sum along axis 1, check each scalar; on violation raise PredictorError::NanDetected{source: Output, tensor_name: Some("oracle_softmax"), ...} (we treat row-sum drift as a numerical-instability bug)
        5. Return probs
    - softmax MUST be applied along axis 1 (per-test + exception axis); axis 0 is batch
    - Use the `ensure_finite` helper from TASK-PRED-005 (re-export it from `predictor::mod` as `pub(crate)` if not already)
    - File length stays &lt; 500 lines
    - No `unwrap()` outside test code
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 51 tests passing (this task adds ~6)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Test (cuda-gated): num_tests=30; predicted.shape=(8, 5120) → predict_logits returns OracleLogits with logits.shape == (8, 31)
    - Test: predicted.shape=(8, 4096) → Err(PredictorError::DimMismatch)
    - Test: NaN-injected predicted → Err(PredictorError::NanDetected)
    - Test: predict_probabilities row sums ≈ 1.0 ± 1e-5 (AC-PRED-04-02)
    - Test: predict_probabilities all values ∈ [0, 1]
    - Test: logits with wrong T+1 dim → Err(PredictorError::DimMismatch)
  </verification>
</definition_of_done>

<pseudo_code>
impl OracleHead {
    pub fn predict_logits(&self, predicted) -&gt; Result&lt;OracleLogits, PredictorError&gt; {
        if predicted.dims().len() != 2 || predicted.dims()[1] != PANEL_DIM {
            return Err(PredictorError::DimMismatch {
                detail: format!("oracle_head expects (B, {}); got {:?}", PANEL_DIM, predicted.dims()),
                observed: btreemap!{"predicted" =&gt; format!("{:?}", predicted.dims())},
                expected_panel_dim: PANEL_DIM,
            });
        }
        ensure_finite(predicted, NanSource::Input { panel: PanelSide::Predicted })?;
        let logits = self.project_to_logits.forward(predicted)?;
        debug_assert_eq!(logits.dims().len(), 2);
        debug_assert_eq!(logits.dims()[1], self.num_tests + 1);
        ensure_finite(&logits, NanSource::Output)?;
        Ok(OracleLogits { logits, num_tests: self.num_tests })
    }

    pub fn predict_probabilities(&self, logits) -&gt; Result&lt;Tensor, PredictorError&gt; {
        let expected_t1 = self.num_tests + 1;
        if logits.logits.dims().len() != 2 || logits.logits.dims()[1] != expected_t1 {
            return Err(PredictorError::DimMismatch {
                detail: format!("oracle logits wrong T+1; expected {}", expected_t1),
                observed: btreemap!{"logits" =&gt; format!("{:?}", logits.logits.dims())},
                expected_panel_dim: PANEL_DIM,  // used loosely; downstream can read expected_t1 from detail
            });
        }
        let probs = candle_nn::ops::softmax(&logits.logits, /*dim=*/1)?;
        ensure_finite(&probs, NanSource::Output)?;

        // Row-sum check (always; cheap)
        let row_sums = probs.to_dtype(F32)?.sum(1)?;  // shape (B,)
        let row_sums_v: Vec&lt;f32&gt; = row_sums.to_vec1()?;
        for (i, s) in row_sums_v.iter().enumerate() {
            if (s - 1.0).abs() &gt; 1e-5 || !s.is_finite() {
                return Err(PredictorError::NanDetected {
                    source: NanSource::Output,
                    layer_id: None,
                    tensor_name: Some("oracle_softmax_row_sum"),
                    sample_idx: Some(i),
                });
            }
        }
        Ok(probs)
    }

    pub fn num_outputs(&self) -&gt; usize { self.num_tests + 1 }
}

tests (cuda-gated):
    test_predict_logits_correct_shape
    test_predict_logits_wrong_panel_dim_returns_err
    test_predict_logits_nan_input_returns_err
    test_predict_probabilities_row_sums_to_one
    test_predict_probabilities_values_in_zero_one
    test_predict_probabilities_wrong_t1_returns_err
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/oracle_head.rs">Replace unimplemented!() bodies of predict_logits + predict_probabilities; add num_outputs accessor; add 6 unit tests</file>
  <file path="crates/context-graph-mejepa/src/predictor/mod.rs">Make ensure_finite a `pub(crate) fn` (or move to a new `predictor/finite.rs` shared module) so OracleHead can call it; alternatively duplicate the helper inline — choose duplication if it keeps cross-module coupling minimal</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 51 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>predict_logits with predicted shape (B, 5120) returns OracleLogits with shape (B, num_tests+1) (REQ-PRED-12, AC-PRED-04-01)</criterion>
  <criterion>predict_logits with wrong-dim input → Err(PredictorError::DimMismatch)</criterion>
  <criterion>predict_logits with NaN-input → Err(PredictorError::NanDetected)</criterion>
  <criterion>predict_probabilities row sums ∈ [1.0 − 1e-5, 1.0 + 1e-5] (AC-PRED-04-02)</criterion>
  <criterion>predict_probabilities values ∈ [0, 1] for every element</criterion>
  <criterion>predict_probabilities with wrong T+1 input → Err(PredictorError::DimMismatch)</criterion>
  <criterion>num_outputs() == num_tests + 1</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa oracle_head</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
