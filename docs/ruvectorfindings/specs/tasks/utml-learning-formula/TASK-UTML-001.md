# TASK-UTML-001 — Data models: UtmlError + LearningSignal + TrainingCertificate + ΔP/ΔK/ΔΩ/ΔΞ component structs

```xml
<task_spec id="TASK-UTML-001" version="1.0">

<metadata>
  <title>Foundation data models for UTML: UtmlError + UtmlErrorCode + LearningSignal + TrainingCertificate + DeltaPComponents + DeltaKComponents + DeltaOmegaComponents + DeltaXiComponents</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-UTML-01">L_step structure carrier</item>
    <item ref="REQ-UTML-07">Per-step training certificate field set</item>
    <item ref="component:UtmlError">crates/context-graph-mejepa-train/src/error.rs</item>
    <item ref="component:LearningSignal">crates/context-graph-mejepa-train/src/learning_signal/mod.rs</item>
    <item ref="component:TrainingCertificate">crates/context-graph-mejepa-train/src/cert/mod.rs</item>
  </implements>
  <depends_on><!-- none; foundation --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>5 created, 1 modified (Cargo.toml workspace member add)</estimated_files>
</metadata>

<context>
The new `context-graph-mejepa-train` crate hosts both UTML (this domain) and TRAINING (Phase 3 trainer host). This task creates the crate skeleton and the UTML-domain foundation data models. Every later UTML and TRAINING task imports these types; nothing else can be authored before this. No compute or business logic — pure type definitions, serde derives, and constructors.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants, failure_taxonomy, coding_standards)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (REQ-UTML-01, REQ-UTML-07; entity TrainingCertificate, LearningSignalInputs)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (data_models: UtmlError, UtmlErrorCode, DeltaPComponents, DeltaKComponents, DeltaOmegaComponents, DeltaXiComponents, LearningSignal, TrainingCertificate)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards</file>
  <file purpose="instrument_panel_dim">crates/context-graph-mejepa-instruments/src/lib.rs (PANEL_DIM = 5_120)</file>
  <file purpose="workspace_root">Cargo.toml (workspace members)</file>
</input_context_files>

<prerequisites>
  <check>Workspace builds: `cargo build --release --workspace` succeeds before any new crate is added</check>
  <check>`crates/context-graph-mejepa-instruments` is shipped (Phase 1 dependency)</check>
  <check>`crates/context-graph-witness` is shipped (cert chain primitives)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create new crate `crates/context-graph-mejepa-train/` with `Cargo.toml`, `src/lib.rs`
    - Add it to root `Cargo.toml` workspace members
    - Create `src/error.rs` with `UtmlError` struct + `UtmlErrorCode` enum (8 variants per TECH-UTML); each variant returns its SCREAMING_SNAKE `.code()` tag
    - Create `src/learning_signal/mod.rs` exposing `pub mod components;` and `pub mod l_step;` (empty stubs allowed; populated by later tasks)
    - Create `src/learning_signal/components.rs` with `DeltaPComponents`, `DeltaKComponents`, `DeltaOmegaComponents`, `DeltaXiComponents`, `LearningSignal` structs
    - Create `src/cert/mod.rs` with `TrainingCertificate` struct + serde derive
    - All structs implement `Serialize + Deserialize + Clone + Debug + PartialEq` (deterministic JSON via sorted keys helper)
    - No compute logic; only type definitions, constructors (`::new`), and validators
  </in_scope>
  <out_of_scope>
    - `compute_delta_*` functions (TASK-UTML-003..006)
    - `SkipReason` enum (TASK-UTML-002)
    - `UtmlState` / `ClassifierVerdict` enums (TASK-UTML-002)
    - `compute_l_step` (TASK-UTML-007)
    - Cert chain hash logic (TASK-UTML-007 / TASK-TRAIN-009)
    - Trainer struct (TRAINING domain)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/error.rs">
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UtmlErrorCode {
    UtmlFactorOutOfRange,
    UtmlSnrBelowFloor,
    UtmlCatastrophicForgetting,
    UtmlClassifierAmbiguous,
    MejepaInstrumentGradientLeak,
    MejepaObserveBatchNan,
    MejepaObserveDriftDetected,
    MejepaObserveIntegrityViolation,
}

impl UtmlErrorCode {
    pub fn as_screaming_snake(&amp;self) -&gt; &amp;'static str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtmlError {
    pub code: UtmlErrorCode,
    pub message: String,
    pub step: Option&lt;u64&gt;,
    pub context: Value,
}

impl UtmlError {
    pub fn new(code: UtmlErrorCode, message: impl Into&lt;String&gt;) -&gt; Self;
    pub fn with_step(self, step: u64) -&gt; Self;
    pub fn with_context(self, ctx: Value) -&gt; Self;
    pub fn code(&amp;self) -&gt; &amp;'static str;
}

impl std::fmt::Display for UtmlError { /* SCREAMING_SNAKE: message */ }
impl std::error::Error for UtmlError {}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/components.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaPComponents {
    pub delta_p_real: f32,
    pub delta_p_imagined: Option&lt;f32&gt;,
    pub snr: f32,
    pub exploration_bonus: f32,
    pub gamma: f32,
    pub aggregator: String, // "Mean" | "Max" | "TopKMean(k=N)" | "EntityWeighted"
    pub per_chunk_values: Vec&lt;f32&gt;,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaKComponents {
    pub cos_align: f32,
    pub fisher_violation: f32,
    pub ece: f32,
    pub embedder_coherence: f32,
    pub embedder_coherence_source: String, // 'computed' | 'bootstrap_neutral_n_lt_1000'
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaOmegaComponents {
    pub effective_plasticity: f32,
    pub landscape_health: f32,
    pub stability_floor: f32, // ∈ {1.0, 0.4, 0.1, 0.0}
    pub agent_state_score: f32,
    pub agent_state_source: String, // 'e17_transcript' | 'default_neutral_no_transcript'
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaXiComponents {
    pub target_collapse: f32, // MUST equal 0.0 for ME-JEPA
    pub predictor_redundancy: f32,
    pub constellation_violation_rate: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningSignal {
    pub step: u64,
    pub delta_p: f32,
    pub delta_k: f32,
    pub delta_omega: f32,
    pub delta_xi: f32,
    pub l_step: f32,
    pub delta_p_components: DeltaPComponents,
    pub delta_k_components: DeltaKComponents,
    pub delta_omega_components: DeltaOmegaComponents,
    pub delta_xi_components: DeltaXiComponents,
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/cert/mod.rs">
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::learning_signal::components::LearningSignal;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingCertificate {
    pub step: u64,
    pub loss: f32,
    pub signal: LearningSignal,
    pub sampling_weight: f32,
    pub grad_norm: f32,
    pub drift_coverage: f32,
    pub skip_reason: Option&lt;String&gt;, // populated by TASK-UTML-002 enum; serialized via Display
    pub parent_witness_hash: String, // 64-hex sha256
    pub self_hash: String,           // 64-hex sha256
    pub merkle_root: String,         // 64-hex sha256
    pub code_version: String,        // 40-hex git sha
    pub corpus_sha: String,          // 64-hex
    pub embedder_versions: HashMap&lt;String, String&gt;, // 21 keys (E1-E21)
    pub frozen_at: String,           // ISO 8601 UTC
    pub timestamp_iso8601: String,   // ISO 8601 UTC
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/lib.rs">
//! ME-JEPA Phase 3 Training Loop + UTML v1.0 Learning Signal.
//! Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).
pub mod error;
pub mod learning_signal;
pub mod cert;

pub use error::{UtmlError, UtmlErrorCode};
    </signature>
    <signature file="crates/context-graph-mejepa-train/Cargo.toml">
[package]
name = "context-graph-mejepa-train"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
context-graph-mejepa-instruments = { path = "../context-graph-mejepa-instruments" }
context-graph-witness = { path = "../context-graph-witness" }

[dev-dependencies]
proptest = "1"
    </signature>
  </signatures>
  <constraints>
    - All file paths use snake_case files; PascalCase types; SCREAMING_SNAKE error codes per `<coding_standards>`
    - All files under 500 lines per `<file_organization>`
    - Every variant of `UtmlErrorCode::as_screaming_snake` returns the literal SCREAMING_SNAKE name from `<failure_taxonomy>` (e.g., `MejepaInstrumentGradientLeak` → `"MEJEPA_INSTRUMENT_GRADIENT_LEAK"`)
    - `embedder_versions` map size validated to be exactly 21 in cert constructor (or test); 21 keys named `e1`..`e21`
    - Provenance comment at top of `lib.rs` per `<provenance>`: "Inspired by ruvnet/RuVector at HEAD ef5274c2"
    - NO compute logic; structs only
    - `target_collapse: f32` field exists as plain f32 (assertion enforced by `compute_delta_xi` later in TASK-UTML-006); cert validation done in cert/verify.rs in TASK-TRAIN-009
    - JSON serialization MUST be sorted-key-stable: implement `to_canonical_json(&amp;self) -&gt; String` helper using `serde_json::to_value` + sort
    - No `unwrap_or` on confidence intervals; no silent fallback paths per `<error_handling>`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-train` succeeds
    - `cargo test -p context-graph-mejepa-train` runs (test count may be small; constructor + serde round-trip)
    - `cargo clippy -p context-graph-mejepa-train -- -D warnings` clean
    - `cargo fmt -p context-graph-mejepa-train --check` passes
  </verification>
</definition_of_done>

<pseudo_code>
// error.rs
impl UtmlErrorCode {
    pub fn as_screaming_snake(&amp;self) -&gt; &amp;'static str {
        match self {
            Self::UtmlFactorOutOfRange => "UTML_FACTOR_OUT_OF_RANGE",
            Self::UtmlSnrBelowFloor => "UTML_SNR_BELOW_FLOOR",
            Self::UtmlCatastrophicForgetting => "UTML_CATASTROPHIC_FORGETTING",
            Self::UtmlClassifierAmbiguous => "UTML_CLASSIFIER_AMBIGUOUS",
            Self::MejepaInstrumentGradientLeak => "MEJEPA_INSTRUMENT_GRADIENT_LEAK",
            Self::MejepaObserveBatchNan => "MEJEPA_OBSERVE_BATCH_NAN",
            Self::MejepaObserveDriftDetected => "MEJEPA_OBSERVE_DRIFT_DETECTED",
            Self::MejepaObserveIntegrityViolation => "MEJEPA_OBSERVE_INTEGRITY_VIOLATION",
        }
    }
}

// cert/mod.rs
impl TrainingCertificate {
    pub fn validate_invariants(&amp;self) -&gt; Result&lt;(), UtmlError&gt; {
        if self.embedder_versions.len() != 21 {
            return Err(UtmlError::new(
                UtmlErrorCode::UtmlFactorOutOfRange,
                "embedder_versions must have 21 keys (E1-E21)",
            ));
        }
        Ok(())
    }
    pub fn to_canonical_json(&amp;self) -&gt; serde_json::Result&lt;String&gt; {
        // sort keys via serde_json::Value::to_string roundtrip with BTreeMap
        let v = serde_json::to_value(self)?;
        let sorted = sort_keys(&amp;v);
        serde_json::to_string(&amp;sorted)
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">workspace crate manifest</file>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">module re-exports + provenance comment</file>
  <file path="crates/context-graph-mejepa-train/src/error.rs">UtmlError + UtmlErrorCode (8 variants)</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod components; pub mod l_step (stub); pub mod sampling_weight (stub)</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/components.rs">DeltaPComponents, DeltaKComponents, DeltaOmegaComponents, DeltaXiComponents, LearningSignal</file>
  <file path="crates/context-graph-mejepa-train/src/cert/mod.rs">TrainingCertificate struct + validate_invariants + to_canonical_json</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml">add `crates/context-graph-mejepa-train` to workspace members</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 8 UtmlErrorCode variants map to the exact SCREAMING_SNAKE codes from `<failure_taxonomy>`</criterion>
  <criterion>All 4 component structs round-trip via serde_json (test included)</criterion>
  <criterion>TrainingCertificate.embedder_versions size validated to 21</criterion>
  <criterion>to_canonical_json produces byte-equal output across two invocations on identical input (determinism unit test)</criterion>
  <criterion>cargo build/test/clippy/fmt all green</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
  <command>cargo fmt -p context-graph-mejepa-train --check</command>
</test_commands>

</task_spec>
```
