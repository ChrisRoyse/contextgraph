# TASK-PRED-002 — OracleHead struct + PredictedPanel + OracleLogits + TargetPanel + TargetProvenance

```xml
<task_spec id="TASK-PRED-002" version="1.0">

<metadata>
  <title>Replace placeholder OracleHead + add PredictedPanel + OracleLogits + TargetPanel + TargetProvenance data models</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-PRED-01">PredictedPanel data model carries (B, 5120) tensor</item>
    <item ref="REQ-PRED-09">OracleHead struct surface + parameters() in trainable_parameters() pool</item>
    <item ref="REQ-PRED-12">OracleHead Linear(5120, num_tests + 1) construction</item>
    <item ref="component:OracleHead">struct + new()</item>
    <item ref="component:PredictedPanel">data model</item>
    <item ref="component:OracleLogits">data model</item>
    <item ref="component:TargetPanel">data model</item>
    <item ref="component:TargetProvenance">data model</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 files modified, 0 new (oracle_head.rs and data_models.rs already created as placeholders in TASK-PRED-001)</estimated_files>
</metadata>

<context>
TASK-PRED-001 declared placeholder versions of `oracle_head` and `data_models` modules. This task
fills them in with real types: `OracleHead` (single Linear projection from PANEL_DIM = 5,120 to
`num_tests + 1` per doc 09 §2.5; the `+1` slot is the dedicated "exception" probability),
`PredictedPanel` (the predictor's output), `OracleLogits` (pre-softmax), `TargetPanel` (the
frozen-target output, with provenance reference), and `TargetProvenance` (code_version +
embedder_versions + frozen_at).

`OracleHead::predict_logits` and `predict_probabilities` are declared but not implemented here —
their bodies live in TASK-PRED-008. This task ships only the type-level contract + `new()` +
constructor validation.

These data models are what every downstream Phase 2 task imports. Iron Rule: TASK-PRED-005,
TASK-PRED-006, TASK-PRED-007, TASK-PRED-008 cannot reference any of these structs until this
task ships.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (provenance rule; failure_taxonomy MEJEPA_INFER_DIM_MISMATCH; performance_budgets)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (data_requirements PredictedPanel / OracleLogits / TargetPanel; REQ-PRED-12)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (data_models PredictedPanel / TargetPanel / OracleLogits / TargetProvenance / OracleHead; component_contracts OracleHead)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-001.md (placeholders defined)</file>
  <file purpose="prior_task_files">crates/context-graph-mejepa/src/{oracle_head.rs,data_models.rs} (placeholders shipped by TASK-PRED-001)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files; PascalCase types; SCREAMING_SNAKE consts)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001 complete: `crates/context-graph-mejepa` builds clean with placeholders</check>
  <check>`cargo build --release -p context-graph-mejepa` succeeds before this task</check>
  <check>OracleHead placeholder unit struct exists in `oracle_head.rs`</check>
  <check>data_models.rs is an empty placeholder file</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace placeholder `OracleHead` with full struct: `project_to_logits: Linear`, `num_tests: usize`
    - Implement `OracleHead::new(num_tests: usize, vb: VarBuilder) -> Result<Self, PredictorError>` (Linear allocation + bounds validation)
    - Add `OracleHead::trainable_parameters(&self) -> Vec<&Var>` (returns the project_to_logits weight + bias)
    - Add `predict_logits` and `predict_probabilities` method **signatures only** (`unimplemented!()` body) — TASK-PRED-008 fills bodies
    - Fill `data_models.rs` with PredictedPanel, OracleLogits, TargetPanel, TargetProvenance
    - Add unit tests verifying num_tests bounds, struct field accessibility, TargetProvenance serde round-trip
  </in_scope>
  <out_of_scope>
    - OracleHead::predict_logits / predict_probabilities BODIES (TASK-PRED-008)
    - FrozenTargetAdapter that produces TargetPanel (TASK-PRED-006)
    - VicregLambdas + LossOutputs + errors taxonomy expansion (TASK-PRED-003)
    - Predictor full forward (TASK-PRED-005)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/oracle_head.rs">
use candle_core::{Tensor, Var};
use candle_nn::{Linear, VarBuilder};

use crate::config::PANEL_DIM;
use crate::data_models::OracleLogits;
use crate::error::PredictorError;

const ORACLE_HEAD_NUM_TESTS_MIN: usize = 1;
const ORACLE_HEAD_NUM_TESTS_MAX: usize = 10_000;

pub struct OracleHead {
    pub(crate) project_to_logits: Linear,
    pub(crate) num_tests: usize,
}

impl OracleHead {
    pub fn new(num_tests: usize, vb: VarBuilder) -&gt; Result&lt;Self, PredictorError&gt;;

    /// Returns the &amp;Var references of project_to_logits (weight + bias) for AdamW registration.
    pub fn trainable_parameters(&amp;self) -&gt; Vec&lt;&amp;Var&gt;;

    pub fn num_tests(&amp;self) -&gt; usize { self.num_tests }

    /// Body in TASK-PRED-008.
    pub fn predict_logits(&amp;self, predicted: &amp;Tensor) -&gt; Result&lt;OracleLogits, PredictorError&gt;;

    /// Body in TASK-PRED-008.
    pub fn predict_probabilities(&amp;self, logits: &amp;OracleLogits) -&gt; Result&lt;Tensor, PredictorError&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/data_models.rs">
use candle_core::Tensor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct PredictedPanel {
    pub data: Tensor,
    pub batch_size: usize,
}

#[derive(Debug)]
pub struct OracleLogits {
    pub logits: Tensor,
    pub num_tests: usize,
}

#[derive(Debug)]
pub struct TargetPanel {
    pub data: Tensor,
    pub provenance_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetProvenance {
    pub code_version: String,
    pub embedder_versions: BTreeMap&lt;String, String&gt;,
    pub frozen_at: String, // ISO 8601 UTC
}

impl TargetProvenance {
    pub fn new(
        code_version: impl Into&lt;String&gt;,
        embedder_versions: BTreeMap&lt;String, String&gt;,
        frozen_at: impl Into&lt;String&gt;,
    ) -&gt; Self;
}
    </signature>
  </signatures>
  <constraints>
    - OracleHead::new validates num_tests ∈ [ORACLE_HEAD_NUM_TESTS_MIN, ORACLE_HEAD_NUM_TESTS_MAX] (1..=10_000)
    - On out-of-range num_tests: return PredictorError::ConfigInvalid with detail `"oracle_head num_tests {n} out of [{min}, {max}]"`
    - Linear is Linear(in=5120, out=num_tests + 1, bias=true) per doc 09 §2.5
    - Use `candle_nn::linear(PANEL_DIM, num_tests + 1, vb.pp("project_to_logits"))?`
    - The "+1" slot is reserved for the EXCEPTION class (per EC-PRED-07 corpus-max sizing rule)
    - `trainable_parameters()` returns the underlying weight + bias as &Var refs (consumed by MeJepaPredictor::trainable_parameters in TASK-PRED-005)
    - TargetProvenance uses BTreeMap (deterministic JSON serialization order) — NOT HashMap
    - TargetProvenance::frozen_at is `String` (RFC 3339 / ISO 8601 UTC) to keep dependency footprint minimal; no chrono leak across crate boundary
    - PredictedPanel.batch_size is the explicit copy of `data.dims()[0]`; validate equality in TASK-PRED-005's forward
    - OracleLogits.num_tests is the explicit copy of the head's num_tests for downstream conformal calibration
    - TargetPanel.provenance_ref is a CLONE of TargetProvenance.code_version (not a borrow) so the panel can outlive the adapter
    - `predict_logits` and `predict_probabilities` bodies are `unimplemented!("body in TASK-PRED-008")` — but signatures must exist so TASK-PRED-005's forward can reference them when wiring up
    - All structs `Debug`-only (do not derive `Clone` on Tensor-bearing structs; cloning a Tensor in candle is cheap reference but explicit clones force callers to think about it)
    - Constitution coding_standards: snake_case files, PascalCase types, SCREAMING_SNAKE consts
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa` shows >= 8 tests passing (5 from TASK-PRED-001 + 3 new: num_tests bounds, TargetProvenance JSON round-trip, OracleHead trainable_parameters returns 2 Vars)
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - serde_json round-trip on a TargetProvenance is byte-equal across two encode→decode cycles
  </verification>
</definition_of_done>

<pseudo_code>
oracle_head.rs:
    const ORACLE_HEAD_NUM_TESTS_MIN: usize = 1;
    const ORACLE_HEAD_NUM_TESTS_MAX: usize = 10_000;
    pub struct OracleHead { project_to_logits, num_tests }
    impl OracleHead::new(num_tests, vb)
        if num_tests &lt; MIN || num_tests &gt; MAX:
            return Err(ConfigInvalid { detail })
        project_to_logits = candle_nn::linear(PANEL_DIM, num_tests + 1, vb.pp("project_to_logits"))?
        Ok(Self { project_to_logits, num_tests })
    impl OracleHead::trainable_parameters(&amp;self)
        // candle_nn::Linear stores weight + optional bias; expose them as &amp;Var
        let mut out = Vec::with_capacity(2);
        out.push(self.project_to_logits.weight().as_var());  // implementation note in TECH-PREDICTOR
        if let Some(bias) = self.project_to_logits.bias() { out.push(bias.as_var()); }
        out
    impl OracleHead::predict_logits(&amp;self, predicted)
        unimplemented!("body in TASK-PRED-008")
    impl OracleHead::predict_probabilities(&amp;self, logits)
        unimplemented!("body in TASK-PRED-008")

data_models.rs:
    pub struct PredictedPanel { data: Tensor, batch_size: usize }
    pub struct OracleLogits { logits: Tensor, num_tests: usize }
    pub struct TargetPanel { data: Tensor, provenance_ref: String }
    #[derive(Serialize, Deserialize)]
    pub struct TargetProvenance { code_version, embedder_versions: BTreeMap, frozen_at }
    impl TargetProvenance::new(code_version, embedder_versions, frozen_at) -&gt; Self

tests:
    test OracleHead::new rejects num_tests=0 with ConfigInvalid
    test OracleHead::new rejects num_tests=10_001 with ConfigInvalid
    test OracleHead::new(num_tests=30) -&gt; Ok and trainable_parameters().len() in [1, 2]
    test TargetProvenance JSON round-trip byte-equal (BTreeMap stable order)
    test PredictedPanel struct accessible (compile-only)
</pseudo_code>

<files_to_create>
  <!-- none; modifying placeholders from TASK-PRED-001 -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/oracle_head.rs">Replace placeholder unit struct with full OracleHead + new() + trainable_parameters() + signature-only predict_logits / predict_probabilities</file>
  <file path="crates/context-graph-mejepa/src/data_models.rs">Fill empty placeholder with PredictedPanel + OracleLogits + TargetPanel + TargetProvenance</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">Add `pub use data_models::{PredictedPanel, OracleLogits, TargetPanel, TargetProvenance};` and `pub use oracle_head::OracleHead;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 8 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>OracleHead::new(num_tests=30) returns Ok with project_to_logits sized to 31 outputs</criterion>
  <criterion>OracleHead::new(num_tests=0) returns Err(ConfigInvalid)</criterion>
  <criterion>OracleHead::new(num_tests=10_001) returns Err(ConfigInvalid)</criterion>
  <criterion>TargetProvenance JSON round-trip is byte-equal (deterministic key order via BTreeMap)</criterion>
  <criterion>PredictedPanel, OracleLogits, TargetPanel, TargetProvenance are publicly re-exported from lib.rs</criterion>
  <criterion>predict_logits / predict_probabilities exist as method signatures (unimplemented body) — referenced by TASK-PRED-005 and TASK-PRED-008</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
