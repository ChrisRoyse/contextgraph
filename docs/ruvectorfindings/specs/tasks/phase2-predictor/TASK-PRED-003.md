# TASK-PRED-003 — VicregLambdas + LossOutputs + PredictorError + LossError + ForwardPassEvidence + VicregLossEvidence

```xml
<task_spec id="TASK-PRED-003" version="1.0">

<metadata>
  <title>Expand PredictorError taxonomy + add LossError + VicregLambdas + LossOutputs + Evidence DTOs (DTOs only — bodies elsewhere)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-PRED-04">MEJEPA_PRED_NAN_DETECTED error variant</item>
    <item ref="REQ-PRED-06">VicregLambdas defaults λ_var=25, λ_cov=1, λ_inv=25, γ=1</item>
    <item ref="REQ-PRED-07">LossOutputs four-term decomposition</item>
    <item ref="REQ-PRED-08">VICReg degenerate variance error variant</item>
    <item ref="REQ-PRED-10">MEJEPA_PRED_FROZEN_TARGET_GRAD CRITICAL variant</item>
    <item ref="REQ-PRED-13">MEJEPA_PRED_VRAM_EXCEEDED variant</item>
    <item ref="REQ-PRED-14">MEJEPA_PRED_VRAM_EXCEEDED warn threshold variant context</item>
    <item ref="REQ-PRED-16">LossOutputs.l_predict public field for SPEC-UTML compute_delta_p</item>
    <item ref="REQ-PRED-18">DimMismatch boundary validation variant</item>
    <item ref="component:PredictorError">full taxonomy expansion</item>
    <item ref="component:LossError">data model</item>
    <item ref="component:VicregLambdas">data model</item>
    <item ref="component:LossOutputs">data model</item>
    <item ref="component:ForwardPassEvidence">FSV DTO</item>
    <item ref="component:VicregLossEvidence">FSV DTO</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 modified, 1 modified (lib.rs re-exports), 0 created (placeholders shipped in TASK-PRED-001)</estimated_files>
</metadata>

<context>
TASK-PRED-001 declared a placeholder `PredictorError` enum with three variants
(`ConfigInvalid`, `DeviceUnavailable`, `Candle`). This task expands it to the full taxonomy
mandated by SPEC-PREDICTOR `<error_states>` and TECH-PREDICTOR `<data_models PredictorError>`,
adds the parallel `LossError` enum for VICReg-side errors, and introduces the four data DTOs
that are read from / written to JSON FSV evidence files: `VicregLambdas`, `LossOutputs`,
`ForwardPassEvidence`, `VicregLossEvidence`.

This is the canonical fail-closed boundary for Phase 2: every `Result<_, PredictorError>` returned
by downstream tasks (forward, gradient hook, VRAM check, VICReg loss, frozen target encoding,
oracle head logits) carries a SCREAMING_SNAKE `.code()` tag from this taxonomy. The
`LossOutputs.l_predict` field is the SPEC-UTML cross-spec contract — making it public here is
deliberate and load-bearing per REQ-PRED-16.

DTOs only: no method bodies that compute values are written here. The bodies are in
TASK-PRED-007 (vicreg_loss), TASK-PRED-009 (gradient hook), TASK-PRED-010 (VRAM), and
TASK-PRED-011 (FSV writer/reader).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_*; error_handling fail-closed; coding_standards error codes SCREAMING_SNAKE)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (error_states ERR-PRED-01..06; data_requirements VicregLambdas / LossOutputs)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (data_models PredictorError / LossError / VicregLambdas / LossOutputs / ForwardPassEvidence / VicregLossEvidence)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-001.md (placeholder PredictorError with 3 variants)</file>
  <file purpose="prior_task_files">crates/context-graph-mejepa/src/{error.rs, evidence.rs} (placeholders shipped by TASK-PRED-001)</file>
  <file purpose="cross_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (compute_delta_p reads LossOutputs.l_predict; this contract MUST stay public)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (SCREAMING_SNAKE error codes; thiserror pattern)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001 complete: error.rs has placeholder PredictorError enum with 3 variants</check>
  <check>evidence.rs is an empty placeholder file</check>
  <check>cargo build --release -p context-graph-mejepa succeeds before this task</check>
  <check>thiserror is available in workspace dependencies</check>
  <check>serde and serde_json are available</check>
</prerequisites>

<scope>
  <in_scope>
    - Expand `PredictorError` to the full 7-variant taxonomy: ConfigInvalid, DeviceUnavailable, DimMismatch, NanDetected, VramExceeded, FrozenTargetGrad, VicregDegenerate (plus the existing `Candle(#[from] candle_core::Error)` for inner-error wrapping)
    - Each variant carries structured context as named fields (no opaque String dump)
    - Add `pub fn code(&self) -> &'static str` returning the canonical SCREAMING_SNAKE code
    - Add `pub fn is_critical(&self) -> bool` (true only for FrozenTargetGrad per BR-PRED-01)
    - Add `LossError` enum: NanDetected, DimMismatch, BatchTooSmall, NonFiniteLambda; with `.code()`
    - Add `From<LossError> for PredictorError` (loss errors propagate up by mapping NanDetected→NanDetected, DimMismatch→DimMismatch, others→ConfigInvalid with a detail string)
    - Add `VicregLambdas` struct + `Default` impl (var=25.0, cov=1.0, inv=25.0, gamma=1.0)
    - Add `LossOutputs` struct (5 public f32 fields: l_predict, l_variance, l_covariance, l_invariance, l_total)
    - Fill `evidence.rs` with `ForwardPassEvidence` + `VicregLossEvidence` (Serialize + Deserialize)
    - Add unit tests: error code mapping; VicregLambdas defaults; LossOutputs serde round-trip; Evidence serde round-trip
  </in_scope>
  <out_of_scope>
    - vicreg_loss function body (TASK-PRED-007)
    - VarianceFloorHistory full impl (TASK-PRED-007)
    - Gradient hook body (TASK-PRED-009)
    - VRAM probing body (TASK-PRED-010)
    - Evidence file writing/reading bodies (TASK-PRED-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/error.rs">
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PredictorError {
    #[error("MEJEPA_PRED_CONFIG_INVALID: {detail}")]
    ConfigInvalid { detail: String },

    #[error("MEJEPA_PRED_DEVICE_UNAVAILABLE: {detail}")]
    DeviceUnavailable { detail: String },

    #[error("MEJEPA_PRED_DIM_MISMATCH: {detail}")]
    DimMismatch {
        detail: String,
        observed: BTreeMap&lt;String, String&gt;, // e.g. {"panel_t0": "[64, 4096]", "panel_t1": "[64, 5120]"}
        expected_panel_dim: usize,
    },

    #[error("MEJEPA_PRED_NAN_DETECTED: source={source} layer_id={layer_id:?} tensor={tensor_name:?}")]
    NanDetected {
        source: NanSource,           // Input { panel: PanelSide } | Layer { idx: u8 } | Output | Loss { component: &'static str }
        layer_id: Option&lt;u8&gt;,
        tensor_name: Option&lt;&amp;'static str&gt;,
        sample_idx: Option&lt;usize&gt;,
    },

    #[error("MEJEPA_PRED_VRAM_EXCEEDED: vram_resident_bytes={vram_resident_bytes} threshold_bytes={threshold_bytes}")]
    VramExceeded {
        vram_resident_bytes: u64,
        threshold_bytes: u64,
    },

    #[error("MEJEPA_PRED_FROZEN_TARGET_GRAD: instrument_id={instrument_id} grad_norm={grad_norm:.6e} threshold={threshold:.6e} fix_at={fix_at}")]
    FrozenTargetGrad {
        instrument_id: String,
        grad_norm: f64,
        threshold: f64,
        fix_at: String, // file:crates/context-graph-mejepa-instruments/src/...
    },

    #[error("MEJEPA_PRED_VICREG_DEGENERATE: low_variance_dim_count={low_variance_dim_count} total_dims={total_dims} gamma={gamma} consecutive_passes={consecutive_passes}")]
    VicregDegenerate {
        low_variance_dim_count: usize,
        total_dims: usize,
        gamma: f32,
        consecutive_passes: u8,
    },

    #[error("MEJEPA_PRED_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NanSource { Input { panel: PanelSide }, Layer, Output, Loss }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide { T0, T1, T2, Predicted, Target }

impl PredictorError {
    pub fn code(&amp;self) -&gt; &amp;'static str;
    pub fn is_critical(&amp;self) -&gt; bool;
}

#[derive(Debug, Error)]
pub enum LossError {
    #[error("MEJEPA_PRED_DIM_MISMATCH (loss): {detail}")]
    DimMismatch { detail: String },

    #[error("MEJEPA_PRED_NAN_DETECTED (loss component={component}): {detail}")]
    NanDetected { component: &amp;'static str, detail: String },

    #[error("MEJEPA_PRED_BATCH_TOO_SMALL: batch={batch} minimum=2")]
    BatchTooSmall { batch: usize },

    #[error("MEJEPA_PRED_NON_FINITE_LAMBDA: lambdas={lambdas_dump}")]
    NonFiniteLambda { lambdas_dump: String },

    #[error("MEJEPA_PRED_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
}

impl LossError { pub fn code(&amp;self) -&gt; &amp;'static str; }

impl From&lt;LossError&gt; for PredictorError;
    </signature>
    <signature file="crates/context-graph-mejepa/src/evidence.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VicregLambdas {
    pub var: f32,
    pub cov: f32,
    pub inv: f32,
    pub gamma: f32,
}

impl Default for VicregLambdas { fn default() -&gt; Self; }

impl VicregLambdas {
    pub fn validate_finite(&amp;self) -&gt; Result&lt;(), crate::error::LossError&gt;;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LossOutputs {
    pub l_predict: f32,    // PUBLIC — SPEC-UTML compute_delta_p contract per REQ-PRED-16
    pub l_variance: f32,
    pub l_covariance: f32,
    pub l_invariance: f32,
    pub l_total: f32,
}

impl LossOutputs {
    /// Verifies finiteness of every component; returns LossError::NanDetected on violation.
    pub fn validate_finite(&amp;self) -&gt; Result&lt;(), crate::error::LossError&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardPassEvidence {
    pub panel_dim: usize,           // == 5120
    pub batch_size: usize,
    pub forward_latency_ms_p50: f32,
    pub forward_latency_ms_p99: f32,
    pub output_finite: bool,        // must be true
    pub output_sha256: String,      // hex digest of bf16 bytes for determinism
    pub vram_resident_bytes: u64,
    pub device: String,             // "Cuda(0) sm_120a"
    pub code_version: String,       // git SHA hex
    pub dod_pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VicregLossEvidence {
    pub batch_size: usize,
    pub lambdas: VicregLambdas,
    pub l_predict: f32,
    pub l_variance: f32,
    pub l_covariance: f32,
    pub l_invariance: f32,
    pub l_total: f32,
    pub components_finite: bool,
    pub formula_check: bool,        // |l_total - (l_predict + var*l_variance + cov*l_covariance + inv*l_invariance)| &lt; 1e-3
    pub dod_pass: bool,
}
    </signature>
  </signatures>
  <constraints>
    - PredictorError `.code()` returns EXACTLY: MEJEPA_PRED_CONFIG_INVALID, MEJEPA_PRED_DEVICE_UNAVAILABLE, MEJEPA_PRED_DIM_MISMATCH, MEJEPA_PRED_NAN_DETECTED, MEJEPA_PRED_VRAM_EXCEEDED, MEJEPA_PRED_FROZEN_TARGET_GRAD, MEJEPA_PRED_VICREG_DEGENERATE, MEJEPA_PRED_CANDLE
    - LossError `.code()` returns EXACTLY: MEJEPA_PRED_DIM_MISMATCH, MEJEPA_PRED_NAN_DETECTED, MEJEPA_PRED_BATCH_TOO_SMALL, MEJEPA_PRED_NON_FINITE_LAMBDA, MEJEPA_PRED_CANDLE
    - `is_critical()` returns true ONLY for `FrozenTargetGrad` (per BR-PRED-01 — no exception)
    - VicregLambdas::default() returns var=25.0, cov=1.0, inv=25.0, gamma=1.0 EXACTLY (constitution-locked per BR-PRED-06)
    - VicregLambdas::validate_finite returns LossError::NonFiniteLambda if any field is NaN/Inf
    - LossOutputs is `Copy` (5 f32s); `l_predict` is `pub` per REQ-PRED-16
    - LossOutputs::validate_finite checks every f32 is finite; returns LossError::NanDetected with component name on violation
    - ForwardPassEvidence and VicregLossEvidence derive Serialize + Deserialize for FSV protocol
    - serde_json round-trip on a populated VicregLossEvidence is byte-equal (deterministic field order via #[derive])
    - From<LossError> for PredictorError maps:
        LossError::NanDetected{component, detail} → PredictorError::NanDetected{ source: Loss, ... }
        LossError::DimMismatch{detail} → PredictorError::DimMismatch{detail, observed: empty, expected_panel_dim: PANEL_DIM}
        LossError::BatchTooSmall{batch} → PredictorError::ConfigInvalid{detail: format!("vicreg batch={batch} too small")}
        LossError::NonFiniteLambda{lambdas_dump} → PredictorError::ConfigInvalid{detail}
        LossError::Candle(e) → PredictorError::Candle(e)
    - Constitution `<error_handling>` "fail-closed at boundaries" — no Result-discarding `let _ = ...`
    - Error messages include source-of-truth file paths starting with `file:` per constitution `<file_organization>` (the `fix_at` field on FrozenTargetGrad is the canonical example)
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 16 tests passing (cumulative; this task adds ~8)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - VicregLambdas::default() == VicregLambdas { var: 25.0, cov: 1.0, inv: 25.0, gamma: 1.0 }
    - serde_json::to_string(&VicregLambdas::default()) → re-parse → equal (round-trip)
    - serde_json round-trip on a fully-populated VicregLossEvidence is byte-equal
    - PredictorError::FrozenTargetGrad{...}.is_critical() == true
    - PredictorError::DimMismatch{...}.is_critical() == false
  </verification>
</definition_of_done>

<pseudo_code>
error.rs:
    expand placeholder enum to full 7+1 variants
    impl PredictorError::code(&self) -&gt; &'static str
        match self {
            ConfigInvalid{..} =&gt; "MEJEPA_PRED_CONFIG_INVALID",
            DeviceUnavailable{..} =&gt; "MEJEPA_PRED_DEVICE_UNAVAILABLE",
            DimMismatch{..} =&gt; "MEJEPA_PRED_DIM_MISMATCH",
            NanDetected{..} =&gt; "MEJEPA_PRED_NAN_DETECTED",
            VramExceeded{..} =&gt; "MEJEPA_PRED_VRAM_EXCEEDED",
            FrozenTargetGrad{..} =&gt; "MEJEPA_PRED_FROZEN_TARGET_GRAD",
            VicregDegenerate{..} =&gt; "MEJEPA_PRED_VICREG_DEGENERATE",
            Candle(_) =&gt; "MEJEPA_PRED_CANDLE",
        }
    impl PredictorError::is_critical(&self) -&gt; bool
        matches!(self, FrozenTargetGrad{..})
    enum LossError { ... } with .code() and #[from] candle_core::Error
    impl From&lt;LossError&gt; for PredictorError per the mapping above

evidence.rs:
    struct VicregLambdas { var, cov, inv, gamma }
    impl Default { var: 25.0, cov: 1.0, inv: 25.0, gamma: 1.0 }
    impl validate_finite()
        for (name, v) in [("var", self.var), ("cov", self.cov), ("inv", self.inv), ("gamma", self.gamma)]:
            if !v.is_finite(): return Err(LossError::NonFiniteLambda { lambdas_dump: format!("{:?}", self) })
        Ok(())
    struct LossOutputs { l_predict, l_variance, l_covariance, l_invariance, l_total } #[derive(Copy)]
    impl validate_finite() — check each component finite; on NaN return LossError::NanDetected with component name
    struct ForwardPassEvidence
    struct VicregLossEvidence

lib.rs additions:
    pub use error::{PredictorError, LossError, NanSource, PanelSide};
    pub use evidence::{VicregLambdas, LossOutputs, ForwardPassEvidence, VicregLossEvidence};

tests:
    test predictor_error_codes_match_taxonomy (each variant -&gt; expected SCREAMING_SNAKE)
    test loss_error_codes_match_taxonomy
    test from_loss_error_to_predictor_error preserves NanDetected and DimMismatch identity
    test vicreg_lambdas_default
    test vicreg_lambdas_validate_finite rejects NaN
    test loss_outputs_validate_finite rejects NaN component
    test loss_outputs_serde_round_trip
    test vicreg_loss_evidence_serde_round_trip
    test forward_pass_evidence_serde_round_trip
    test predictor_error_is_critical_only_frozen_target_grad
</pseudo_code>

<files_to_create>
  <!-- none; modifying placeholders from TASK-PRED-001 -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/error.rs">Replace placeholder 3-variant enum with full 7+1 variant taxonomy + LossError + From impl + .code() + .is_critical()</file>
  <file path="crates/context-graph-mejepa/src/evidence.rs">Fill empty placeholder with VicregLambdas + LossOutputs + ForwardPassEvidence + VicregLossEvidence</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">Add re-exports for the new public surface</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 16 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>PredictorError::code() returns the canonical SCREAMING_SNAKE tag for all 8 variants</criterion>
  <criterion>LossError::code() returns the canonical SCREAMING_SNAKE tag for all 5 variants</criterion>
  <criterion>VicregLambdas::default() == { var: 25.0, cov: 1.0, inv: 25.0, gamma: 1.0 }</criterion>
  <criterion>LossOutputs.l_predict is `pub` (SPEC-UTML cross-spec contract per REQ-PRED-16)</criterion>
  <criterion>FrozenTargetGrad is the only variant for which is_critical() == true</criterion>
  <criterion>serde_json round-trip on VicregLossEvidence + ForwardPassEvidence is byte-equal</criterion>
  <criterion>From&lt;LossError&gt; for PredictorError preserves NanDetected and DimMismatch identity</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
