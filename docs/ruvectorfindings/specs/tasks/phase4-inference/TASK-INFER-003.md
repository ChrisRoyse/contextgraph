# TASK-INFER-003 — MeJepaCompiler::compile (panel materialization → predictor forward → Oracle head → conformal set → OOD score → granger attestations)

```xml
<task_spec id="TASK-INFER-003" version="1.0">

<metadata>
  <title>MeJepaCompiler::compile() in `crates/context-graph-mejepa/src/compiler.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-INFER-01">Public methods compile/verify/observe; this task lands compile (verify in 004; observe stub)</item>
    <item ref="REQ-INFER-02">RealityPrediction construction with all 11 fields populated</item>
    <item ref="REQ-INFER-07">OOD score formula evaluation in compile path; OodRefuse on >0.5</item>
    <item ref="REQ-INFER-08">ΔΩ_mean × ΔΞ_mean multiplicative gate read from CF_MEJEPA_TRAIN_CERTS last 128 steps</item>
    <item ref="REQ-INFER-10">Gate 2 PredictedFailure population (failing_tests under p_test_threshold)</item>
    <item ref="REQ-INFER-12">Gate 4 GrangerRejection p-values via context-graph-solver/granger.rs</item>
    <item ref="component:MeJepaCompiler.compile">behavior steps 1-16 in TECH-INFERENCE</item>
    <item ref="component:materialize_panel_t0">internal helper; instrument cache lookup</item>
    <item ref="component:materialize_panel_t1">internal helper; post-patch panel</item>
    <item ref="component:compute_ood_score">internal helper; finite + clamp</item>
    <item ref="component:run_granger_attestations">F-test wrapper</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-001</task_ref>
    <task_ref>TASK-INFER-002</task_ref>
    <task_ref>TASK-INFER-005</task_ref>
    <task_ref>TASK-INFER-006</task_ref>
    <task_ref>TASK-INFER-007</task_ref>
    <task_ref>TASK-INFER-008</task_ref>
    <task_ref>TASK-INFER-010</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (compiler.rs)</estimated_files>
</metadata>

<context>
This task lands the inference forward path: from (PatchBundle, TaskContext) to a fully-populated
RealityPrediction. It composes the conformal calibration (005), OOD detection (006), outcome-set
builder (007), ΔΩ × ΔΞ gate (008), and CalibrationStore (010) into one method. compile() is the
shared body of verify() — verify() (TASK-INFER-004) orders gates around it and converts OodRefuse
to EscalateToHuman. compile() returns OodRefuse from Err; verify() converts.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants ΔΩ × ΔΞ; performance_budgets p50<100ms)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-01, 02, 07, 08, 10, 12; AC-INFER-01-01..05; BR-INFER-02; EC-INFER-09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts MeJepaCompiler::compile behavior; verify_sequence_diagram; ood_diagram; degraded_state_diagram)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/types.rs (TASK-INFER-001)</file>
  <file purpose="upstream_error">crates/context-graph-mejepa/src/error.rs (TASK-INFER-002)</file>
  <file purpose="upstream_cache">crates/context-graph-mejepa/src/cache.rs (TASK-INFER-002)</file>
  <file purpose="upstream_calibration">crates/context-graph-mejepa/src/calibration.rs (TASK-INFER-010)</file>
  <file purpose="upstream_conformal">crates/context-graph-mejepa/src/conformal.rs (TASK-INFER-005, 007)</file>
  <file purpose="upstream_ood">crates/context-graph-mejepa/src/ood.rs (TASK-INFER-006)</file>
  <file purpose="upstream_dxomega_dxxi">crates/context-graph-mejepa/src/degraded.rs (TASK-INFER-008)</file>
  <file purpose="instrument_substrate">crates/context-graph-mejepa-instruments/src/lib.rs (Panel, PanelBuilder, Instrument trait, InstrumentSlot)</file>
  <file purpose="granger">crates/context-graph-solver/src/granger.rs (Granger F-test API)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-001/002/005/006/007/008/010 all complete</check>
  <check>Phase 2 PREDICTOR shipped: `Predictor`, `FrozenTarget`, `OracleHead` traits exist (placeholder mock OK)</check>
  <check>`context-graph-solver/src/granger.rs` exposes f_test(&[f32], &[f32]) -> f32 (p-value)</check>
  <check>`context-graph-mejepa-instruments` exposes `materialize_panel`-equivalent or per-slot encode</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/compiler.rs` with `pub struct MeJepaCompiler { ... }` carrying the 9 Arc-wrapped fields per TECH-INFERENCE data_models.MeJepaCompiler
    - `MeJepaCompiler::new(predictor, frozen_target, oracle_head, calibration, constellation, instrument_cache, storage, witness_chain, config) -> Result<Self, MejepaInferError>` validating fields per TECH-INFERENCE behavior block (calibration has ≥1 record else CalibrationStale; alpha ∈ [0.01, 0.50]; p_threshold ∈ [0.0001, 0.05])
    - `pub fn compile(&self, patch: &PatchBundle, context: &TaskContext) -> Result<RealityPrediction, MejepaInferError>` implementing 16-step behavior verbatim from TECH-INFERENCE
    - Internal helpers: `materialize_panel_t0`, `materialize_panel_t1`, `compute_ood_score`, `run_granger_attestations` (each as private methods)
    - `pub fn observe(&mut self, panel: &Panel, oracle_outcome: &OracleOutcome) -> Result<(), MejepaInferError>` — STUB returning `Err(MejepaInferError::FeatureDisabled { feature: "self-healing" })` per TECH-INFERENCE component_contracts.observe (Phase 5 will fill body)
    - Bootstrap defaults for ΔΩ_mean / ΔΞ_mean when CF_MEJEPA_TRAIN_CERTS empty (per EC-INFER-09): use config.bootstrap_delta_omega / bootstrap_delta_xi (0.5/0.5); set degraded_status=true
    - Singleflight wrapper around InstrumentCache miss-insert (per-key Tokio semaphore via DashMap<SlotKey, Arc<Semaphore>>)
    - Trait bounds on Predictor/FrozenTarget/OracleHead introduced as minimal interfaces (`Predictor::forward(&Panel, &Panel) -> Tensor` etc.) so this task does not block waiting for Phase 2 final API
    - Add unit tests with mock Predictor/FrozenTarget/OracleHead/CalibrationStore/ConstellationGuard/WitnessChainReader/RocksDbStore: (a) compile end-to-end on synthetic in-distribution bundle returns RealityPrediction with finite fields; (b) compile returns OodRefuse on synthetic OOD panel; (c) compile sets degraded_status=true when CF_MEJEPA_TRAIN_CERTS empty; (d) compile populates failing_tests when per-test prob < p_test_threshold; (e) compile evaluates Granger on empty claim_graph trivially (no skills, empty attestations)
  </in_scope>
  <out_of_scope>
    - 7-gate verify ladder ordering (TASK-INFER-004)
    - Source-SHA drift gate logic + witness replay (TASK-INFER-009 — compile() does NOT run those gates; verify() does)
    - MCP tool wiring (TASK-INFER-011)
    - CLI infer-test driver (TASK-INFER-013)
    - Real Phase 2 predictor weights — mocks suffice for this task
    - Real TCT constellation — verify() handles Gate 7; compile() does not call into TCT
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/compiler.rs">
use std::sync::Arc;
use crate::types::{PatchBundle, TaskContext, RealityPrediction, OracleOutcome};
use crate::error::MejepaInferError;
use crate::config::MeJepaInferConfig;
use crate::cache::InstrumentCache;
use crate::calibration::CalibrationStore;
use context_graph_mejepa_instruments::Panel;

pub trait Predictor: Send + Sync {
    fn forward(&amp;self, panel_t0: &amp;Panel, panel_t1: &amp;Panel) -&gt; Result&lt;Panel, MejepaInferError&gt;;
}
pub trait FrozenTarget: Send + Sync {
    fn forward(&amp;self, panel: &amp;Panel) -&gt; Result&lt;Panel, MejepaInferError&gt;;
}
pub trait OracleHead: Send + Sync {
    fn forward(&amp;self, panel_pred_t2: &amp;Panel) -&gt; Result&lt;Vec&lt;f32&gt;, MejepaInferError&gt;;
}
pub trait ConstellationGuard: Send + Sync {
    fn gtau_test(&amp;self, panel: &amp;Panel) -&gt; GtauOutcome;
    fn version_id(&amp;self) -&gt; Option&lt;String&gt;;
}
pub trait WitnessChainReader: Send + Sync {
    fn current_offset(&amp;self) -&gt; u64;
    fn replay_segment(&amp;self, segment: &amp;[u8]) -&gt; Result&lt;(), MejepaInferError&gt;;
}
pub trait MejepaStore: Send + Sync {
    fn read_recent_train_certs(&amp;self, n: usize) -&gt; Vec&lt;TrainCertSummary&gt;;
    fn write_live_prediction(&amp;self, key: ([u8; 16], u64), pred: &amp;RealityPrediction) -&gt; Result&lt;(), MejepaInferError&gt;;
    fn read_skill_history(&amp;self, skill: crate::types::SkillId) -&gt; Vec&lt;(f32, f32)&gt;;
}

pub struct GtauOutcome {
    pub satisfied: bool,
    pub violating_embedders: Vec&lt;crate::types::EmbedderId&gt;,
    pub violating_chunks: Vec&lt;crate::types::ChunkId&gt;,
}

pub struct TrainCertSummary {
    pub delta_omega: f32,
    pub delta_xi: f32,
}

pub struct MeJepaCompiler {
    predictor: Arc&lt;dyn Predictor&gt;,
    frozen_target: Arc&lt;dyn FrozenTarget&gt;,
    oracle_head: Arc&lt;dyn OracleHead&gt;,
    calibration: Arc&lt;CalibrationStore&gt;,
    constellation: Arc&lt;dyn ConstellationGuard&gt;,
    instrument_cache: Arc&lt;InstrumentCache&gt;,
    storage: Arc&lt;dyn MejepaStore&gt;,
    witness_chain: Arc&lt;dyn WitnessChainReader&gt;,
    config: MeJepaInferConfig,
}

impl MeJepaCompiler {
    pub fn new(
        predictor: Arc&lt;dyn Predictor&gt;,
        frozen_target: Arc&lt;dyn FrozenTarget&gt;,
        oracle_head: Arc&lt;dyn OracleHead&gt;,
        calibration: Arc&lt;CalibrationStore&gt;,
        constellation: Arc&lt;dyn ConstellationGuard&gt;,
        instrument_cache: Arc&lt;InstrumentCache&gt;,
        storage: Arc&lt;dyn MejepaStore&gt;,
        witness_chain: Arc&lt;dyn WitnessChainReader&gt;,
        config: MeJepaInferConfig,
    ) -&gt; Result&lt;Self, MejepaInferError&gt;;

    pub fn compile(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext)
        -&gt; Result&lt;RealityPrediction, MejepaInferError&gt;;

    pub fn observe(&amp;mut self, panel: &amp;Panel, oracle_outcome: &amp;OracleOutcome)
        -&gt; Result&lt;(), MejepaInferError&gt;;

    fn materialize_panel_t0(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext)
        -&gt; Result&lt;Panel, MejepaInferError&gt;;
    fn materialize_panel_t1(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext)
        -&gt; Result&lt;Panel, MejepaInferError&gt;;
    fn compute_ood_score(&amp;self, predicted: &amp;Panel, target: &amp;Panel, sigma_squared: f32)
        -&gt; Result&lt;f32, MejepaInferError&gt;;
    fn run_granger_attestations(&amp;self, claim_graph: &amp;crate::types::ClaimGraph)
        -&gt; Result&lt;std::collections::HashMap&lt;crate::types::SkillId, f32&gt;, MejepaInferError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - `&self` (not `&mut self`) on compile (Tokio-shareable per NFR-INFER-09)
    - Bootstrap defaults set `degraded_status = true` when CF_MEJEPA_TRAIN_CERTS empty (EC-INFER-09)
    - On `ood_score > config.ood_refuse_threshold`: return `Err(MejepaInferError::OodRefuse { score, panel_sha: sha256(predicted_t2 bytes) })` — do NOT proceed
    - All f32 fields validated finite-and-clamped via `RealityPrediction::try_new`; NanDetected propagated with field name + witness_chain_offset
    - `predicted_runtime_trace_summary` projected to length 32 (E_Trace 32-d projection)
    - Granger F-test: empty claim_graph yields empty attestations (EC-INFER-05); skill with no series yields p_value=1.0 (fail-closed)
    - Singleflight: per-key Tokio semaphore prevents thundering-herd on cache miss; under contention only one task encodes
    - All imports start with `file:` in error log paths per constitution &lt;coding_standards.file_organization&gt;
    - No `unwrap_or` on confidence; every `?` site wraps `e.log_context(file!())` per &lt;error_handling&gt;
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa compiler` shows ≥ 5 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `compile_returns_finite_prediction_in_distribution` passes
    - Test `compile_returns_ood_refuse_on_high_score` passes (mock predictor producing far-from-target panel → score > 0.5)
    - Test `compile_sets_degraded_status_when_train_certs_empty` passes
    - Test `compile_populates_failing_tests_below_p_test_threshold` passes
    - Test `compile_empty_claim_graph_yields_empty_attestations` passes
  </verification>
</definition_of_done>

<pseudo_code>
fn compile(&self, patch, context) -> Result<RealityPrediction, MejepaInferError> {
    self.validate_patch_bundle(patch)?;
    self.validate_task_context(context)?;

    // Step 2: training health
    let certs = self.storage.read_recent_train_certs(self.config.train_cert_window_steps);
    let (dom, dxi, degraded) = if certs.is_empty() {
        (self.config.bootstrap_delta_omega, self.config.bootstrap_delta_xi, true)
    } else {
        let mean_omega = certs.iter().map(|c| c.delta_omega).sum::<f32>() / certs.len() as f32;
        let mean_xi    = certs.iter().map(|c| c.delta_xi).sum::<f32>() / certs.len() as f32;
        let degraded   = mean_omega < 0.4 || mean_xi < 0.4;
        (mean_omega, mean_xi, degraded)
    };

    // Step 3: panels
    let panel_t0 = self.materialize_panel_t0(patch, context)?;
    let panel_t1 = self.materialize_panel_t1(patch, context)?;

    // Step 4: forward
    let panel_pred_t2 = self.predictor.forward(&panel_t0, &panel_t1)?;
    let panel_target_t1 = self.frozen_target.forward(&panel_t1)?;

    // Step 5: load active calibration (sigma + tau)
    let calib = self.calibration.load_active()?;

    // Step 5: OOD
    let ood = self.compute_ood_score(&panel_pred_t2, &panel_target_t1, calib.sigma_squared)?;
    if ood > self.config.ood_refuse_threshold {
        let panel_sha = sha256_of_panel_bytes(&panel_pred_t2);
        return Err(MejepaInferError::OodRefuse { score: ood, panel_sha });
    }

    // Step 7: oracle logits
    let logits = self.oracle_head.forward(&panel_pred_t2)?;
    let per_test_probs = softmax_sigmoid(&logits);
    let predicted_oracle_pass = per_test_probs.iter().product::<f32>().clamp(0.0, 1.0);
    let predicted_failing_tests = per_test_probs.iter().enumerate()
        .filter_map(|(i, &p)| (p < self.config.p_test_threshold).then(|| TestId(i as u32)))
        .collect();

    // Steps 9-10: outcome_set with delta gating
    let multiplier = (dom * dxi).clamp(0.10, 0.95);
    let outcome_set = build_outcome_set(&per_test_probs, context, calib.alpha, calib.tau, multiplier)?;

    // Step 11: granger
    let granger_attestations = self.run_granger_attestations(&patch.claim_graph)?;

    // Step 12: trace summary
    let trace_summary = panel_pred_t2.slice_e_trace_to_32().to_vec();

    // Step 13: side effects
    let predicted_side_effect_set = read_side_effects(&panel_pred_t2);

    // Step 14: witness offset
    let witness_chain_offset = self.witness_chain.current_offset();

    // Step 15-16: build
    let confidence = ConformalSet::try_new(calib.alpha, outcome_set)?;
    RealityPrediction::try_new(RealityPredictionBuilder {
        predicted_oracle_pass,
        predicted_failing_tests,
        predicted_runtime_trace_summary: trace_summary,
        predicted_side_effect_set,
        confidence,
        out_of_distribution_score: ood,
        granger_attestations,
        witness_chain_offset,
        degraded_status: degraded,
        delta_omega_mean: dom,
        delta_xi_mean: dxi,
    })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/compiler.rs">MeJepaCompiler + Predictor/FrozenTarget/OracleHead/ConstellationGuard/WitnessChainReader/MejepaStore traits + 4 internal helpers</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod compiler;` and `pub use compiler::*;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa compiler shows ≥ 5 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>compile is &amp;self (not &amp;mut)</criterion>
  <criterion>OodRefuse error fires when synthetic ood_score > 0.5</criterion>
  <criterion>degraded_status=true when train_certs empty (bootstrap)</criterion>
  <criterion>Empty claim_graph yields HashMap len 0</criterion>
  <criterion>No unwrap_or on confidence</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa compiler</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
