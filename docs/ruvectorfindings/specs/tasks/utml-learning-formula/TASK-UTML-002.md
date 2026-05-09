# TASK-UTML-002 — Enums: SkipReason + UtmlState + ClassifierVerdict + DeltaPAggregator + Intervention

```xml
<task_spec id="TASK-UTML-002" version="1.0">

<metadata>
  <title>Foundation enums: SkipReason (6 variants) + UtmlState (7 variants) + Intervention + ClassifierVerdict + DeltaPAggregator</title>
  <status>blocked</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-UTML-08">UTML cycle classifier 7-state enum</item>
    <item ref="REQ-UTML-02">DeltaPAggregator (Mean | Max | TopKMean | EntityWeighted)</item>
    <item ref="REQ-UTML-07">SkipReason variants populated when L_step==0</item>
    <item ref="component:UtmlState">crates/context-graph-mejepa-train/src/learning_signal/classifier.rs</item>
    <item ref="component:DeltaPAggregator">crates/context-graph-mejepa-train/src/learning_signal/delta_p.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>3 created, 2 modified</estimated_files>
</metadata>

<context>
The classifier (TASK-UTML-009) and the per-chunk ΔP aggregator (TASK-UTML-003) consume these enums. They are simple discriminated unions but must ship before any compute function references them. Pure type definitions + serde + display impls; no logic.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants — coaching order)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-08 7-state classifier; EC-UTML-12 aggregator)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (data_models: UtmlState, ClassifierVerdict, SkipReason, DeltaPAggregator)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-001.md (UtmlError already shipped)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete (UtmlError + components shipped)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/skip_reason.rs` with `SkipReason` enum (6 variants: PerfectPrediction, CatastrophicForgetting, NanInf, GoodhartCatastrophe, SnrBelowFloor, OperatorAbort) per TECH-UTML data_models
    - Create `src/learning_signal/state.rs` with `UtmlState` enum (7 variants: Stuck, OptimizerDysregulated, LatentCollapsing, Confused, Optimal, Boring, Dissipating); `Intervention` enum (CurriculumPrune, RegulateOmega, RegulateXi, AdvanceCurriculum, None, StateRegulate); `ClassifierVerdict` struct (state, recommended_intervention, diagnostic_payload, confidence)
    - Create `src/learning_signal/aggregator.rs` with `DeltaPAggregator` enum (Mean, Max, TopKMean { k: usize }, EntityWeighted)
    - Modify `src/learning_signal/mod.rs` to re-export new types
    - Modify `src/cert/mod.rs` to change `skip_reason: Option<String>` to `skip_reason: Option<SkipReason>` (typed)
    - All enums serialize with `#[serde(rename_all = "snake_case")]` matching TECH-UTML constraints
    - Display + FromStr impls on each enum for SCREAMING_SNAKE/snake_case conversions where useful
  </in_scope>
  <out_of_scope>
    - Classifier logic (TASK-UTML-009)
    - Aggregator math (TASK-UTML-003)
    - Skip-reason emission rules (TASK-UTML-007 / TRAINING)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/skip_reason.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    PerfectPrediction,
    CatastrophicForgetting,
    NanInf,
    GoodhartCatastrophe,
    SnrBelowFloor,
    OperatorAbort,
}

impl SkipReason {
    pub fn as_str(&amp;self) -&gt; &amp;'static str;
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/state.rs">
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtmlState {
    Stuck,
    OptimizerDysregulated,
    LatentCollapsing,
    Confused,
    Optimal,
    Boring,
    Dissipating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intervention {
    CurriculumPrune,
    RegulateOmega,
    RegulateXi,
    AdvanceCurriculum,
    None,
    StateRegulate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifierVerdict {
    pub state: UtmlState,
    pub recommended_intervention: Intervention,
    pub diagnostic_payload: Value,
    pub confidence: f32,
}

impl UtmlState {
    pub fn default_intervention(&amp;self) -&gt; Intervention;
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/aggregator.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeltaPAggregator {
    Mean,
    Max,
    TopKMean { k: usize },
    EntityWeighted,
}

impl DeltaPAggregator {
    pub fn label(&amp;self) -&gt; String; // "Mean" | "Max" | "TopKMean(k=N)" | "EntityWeighted"
}
    </signature>
  </signatures>
  <constraints>
    - All enums Clone+Copy+Eq where the variant payload allows (TopKMean{k} blocks Copy; OK to leave it Clone-only)
    - serde rename_all = "snake_case" on simple enums; tag = "kind" on aggregator (so TopKMean{k} round-trips)
    - `default_intervention(&amp;self)` mirrors UTML §IX coaching order: Stuck=>CurriculumPrune, OptimizerDysregulated=>RegulateOmega, LatentCollapsing=>RegulateXi, Confused=>None, Optimal=>None, Boring=>AdvanceCurriculum, Dissipating=>StateRegulate
    - cert.skip_reason changes type from Option<String> to Option<SkipReason>; existing serde JSON form remains backwards-compatible because untagged unit variants serialize as strings already (test the round-trip)
    - All files under 200 lines
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-train` succeeds
    - `cargo test -p context-graph-mejepa-train` shows ≥ 4 new tests (one round-trip per enum + default_intervention table)
    - JSON for `SkipReason::PerfectPrediction` is `"perfect_prediction"` (lowercase snake_case)
    - JSON for `DeltaPAggregator::TopKMean{k:5}` is `{"kind":"top_k_mean","k":5}`
  </verification>
</definition_of_done>

<pseudo_code>
// state.rs
impl UtmlState {
    pub fn default_intervention(&amp;self) -&gt; Intervention {
        match self {
            UtmlState::Stuck => Intervention::CurriculumPrune,
            UtmlState::OptimizerDysregulated => Intervention::RegulateOmega,
            UtmlState::LatentCollapsing => Intervention::RegulateXi,
            UtmlState::Confused => Intervention::None,
            UtmlState::Optimal => Intervention::None,
            UtmlState::Boring => Intervention::AdvanceCurriculum,
            UtmlState::Dissipating => Intervention::StateRegulate,
        }
    }
}

// aggregator.rs
impl DeltaPAggregator {
    pub fn label(&amp;self) -&gt; String {
        match self {
            Self::Mean => "Mean".into(),
            Self::Max => "Max".into(),
            Self::TopKMean { k } => format!("TopKMean(k={})", k),
            Self::EntityWeighted => "EntityWeighted".into(),
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/skip_reason.rs">SkipReason enum</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/state.rs">UtmlState + Intervention + ClassifierVerdict</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/aggregator.rs">DeltaPAggregator</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">re-export new modules</file>
  <file path="crates/context-graph-mejepa-train/src/cert/mod.rs">TrainingCertificate.skip_reason: Option&lt;SkipReason&gt; (was Option&lt;String&gt;)</file>
</files_to_modify>

<validation_criteria>
  <criterion>SkipReason has exactly 6 variants matching TECH-UTML</criterion>
  <criterion>UtmlState has exactly 7 variants</criterion>
  <criterion>default_intervention table matches UTML §IX coaching order</criterion>
  <criterion>Serde round-trips for all enums</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
