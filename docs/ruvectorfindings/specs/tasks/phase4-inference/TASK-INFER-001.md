# TASK-INFER-001 — Public inference data models (RealityPrediction, ConformalSet, OracleOutcome, VerifyVerdict, FailedGate, PatchBundle, TaskContext, ClaimGraph, SkillCitation)

```xml
<task_spec id="TASK-INFER-001" version="1.0">

<metadata>
  <title>Define public inference data models in `crates/context-graph-mejepa/src/types.rs`</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-INFER-02">RealityPrediction shape + invariants (predicted_oracle_pass, ConformalSet, ood, granger, witness offset, degraded_status, ΔΩ/ΔΞ means)</item>
    <item ref="REQ-INFER-03">VerifyVerdict { Approve, EscalateToHuman } enum (gates_passed ∈ [0,7])</item>
    <item ref="REQ-INFER-04">FailedGate 7 variants including ConstellationGuardRejected</item>
    <item ref="REQ-INFER-05">PatchBundle + TaskContext + ClaimGraph + SkillCitation</item>
    <item ref="component:RealityPrediction">RealityPrediction::try_new validation gate</item>
    <item ref="component:ConformalSet">ConformalSet alpha + outcome_set</item>
  </implements>
  <depends_on><!-- none; foundation. INSTRUMENTS Panel/PanelBuilder is shipped --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>3 created (Cargo.toml, src/lib.rs, src/types.rs)</estimated_files>
</metadata>

<context>
This task creates the new `crates/context-graph-mejepa/` crate skeleton and lands all the public
data models that downstream tasks (003 compile, 004 verify, 011 MCP, 014 FSV) consume. The structs
encode the contract between the inference path and the MCP surface; getting field shapes wrong here
cascades into every other Phase 4a task. RealityPrediction MUST validate finite + clamped at
construction (no `unwrap_or` on confidence intervals per constitution).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy, anti_patterns "no unwrap_or on confidence")</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-02..05, EC-INFER-01)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (data_models block; src/types.rs file purpose)</file>
  <file purpose="cross_domain_types">crates/context-graph-mejepa-corpus/src/swebench.rs (OracleVerdict, OracleOutcome re-export source)</file>
  <file purpose="instruments_dep">crates/context-graph-mejepa-instruments/src/lib.rs (Panel, InstrumentSlot, EmbedderId placeholder)</file>
  <file purpose="witness_dep">crates/context-graph-witness/src/lib.rs (WitnessSegment, WITNESS_ENTRY_SIZE = 73)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files, PascalCase types, SCREAMING_SNAKE error codes)</file>
</input_context_files>

<prerequisites>
  <check>Workspace builds: `cargo build --release --workspace`</check>
  <check>`crates/context-graph-mejepa-instruments` exposes `Panel`, `InstrumentSlot`, `PANEL_DIM = 5_120`</check>
  <check>`crates/context-graph-mejepa-corpus` exposes `OracleVerdict` + `OracleOutcome` enum</check>
  <check>`crates/context-graph-witness` exposes `WitnessSegment` and `WITNESS_ENTRY_SIZE`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/Cargo.toml` (workspace member, internal deps to mejepa-instruments + mejepa-corpus + witness + solver + storage + embeddings)
    - Create `crates/context-graph-mejepa/src/lib.rs` declaring `pub mod types;` (other modules added in later tasks)
    - Create `crates/context-graph-mejepa/src/types.rs` with all 9 public types
    - Add `RealityPrediction::try_new(builder) -> Result<Self, MejepaInferError>` validating finite + clamped fields (returns `MejepaInferError::NanDetected{field, witness_chain_offset}` on violation; the error type lands in TASK-INFER-002 — for this task forward-declare the field-validation logic and rely on `Result<Self, &'static str>` for the placeholder; replace with structured error in TASK-INFER-002 ONLY)
    - Add `ConformalSet { alpha: f32, outcome_set: Vec<OracleOutcome> }` with non-empty + distinct invariants
    - Add `VerifyVerdict { Approve { reality_prediction, gates_passed: u8 }, EscalateToHuman { failed_gate, reality_prediction } }`
    - Add `FailedGate` 7 variants matching REQ-INFER-04 verbatim
    - Add `PatchBundle { pre_patch_sha: [u8; 32], post_patch_sha: [u8; 32], ast_diff, witness_chain_segment, claim_graph }`
    - Add `TaskContext { task_id, problem_statement, test_stack, environment, language }` (problem_statement bounded 64 KiB)
    - Add `ClaimGraph { skills: Vec<SkillCitation> (cap 256), touched_files: Vec<PathBuf> }`
    - Add `SkillCitation { skill: SkillId, path: PathBuf, expected_outcome: String }`
    - Add `SideEffect`, `TestId`, `SkillId`, `EmbedderId`, `ChunkId` placeholder enums (closed taxonomies; SkillId variants TBD by domain in later spec)
    - Add `AstDiff { touched_paths: Vec<PathBuf>, hunks: Vec<DiffHunk> }`
    - Add `TaskEnvironment { python_version: Option<String>, rust_toolchain: Option<String>, env_vars: BTreeMap<String, String> }`
    - Add `Language` enum with 11 variants (Rust, Python, Javascript, Typescript, Go, Java, C, Cpp, CSharp, Ruby, Php) — re-export or duplicate from mejepa-instruments / mejepa-corpus depending on which crate already owns the canonical
    - Add `serde::{Serialize, Deserialize}` derives on all public types with `#[serde(rename_all = "snake_case")]` on enums
    - Add unit tests in `src/types.rs` `#[cfg(test)] mod tests` for: (a) `RealityPrediction::try_new` rejects NaN in `predicted_oracle_pass`; (b) ConformalSet rejects empty outcome_set; (c) FailedGate exhaustively constructible; (d) PatchBundle/TaskContext serde round-trip
  </in_scope>
  <out_of_scope>
    - `MejepaInferError` (lands in TASK-INFER-002)
    - `CalibrationRecord`, `ProvenanceBundle`, `MeJepaInferConfig`, `InstrumentCache`, `SlotKey` (TASK-INFER-002)
    - `MeJepaCompiler` impl (TASK-INFER-003)
    - Verify-ladder logic (TASK-INFER-004)
    - MCP tool schemas (TASK-INFER-011)
    - Re-using existing `EmbedderId` if EMBEDDERS Phase 1b shipped — defer to that crate's enum (this task ships a placeholder if EMBEDDERS not yet in tree)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/types.rs">
use std::path::PathBuf;
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust, Python, Javascript, Typescript, Go, Java, C, Cpp, CSharp, Ruby, Php,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleOutcome { Pass, Fail, OutOfDistribution, Abstain }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConformalSet {
    pub alpha: f32,
    pub outcome_set: Vec&lt;OracleOutcome&gt;,
}

impl ConformalSet {
    pub fn try_new(alpha: f32, outcome_set: Vec&lt;OracleOutcome&gt;) -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RealityPrediction {
    pub predicted_oracle_pass: f32,
    pub predicted_failing_tests: Vec&lt;TestId&gt;,
    pub predicted_runtime_trace_summary: Vec&lt;f32&gt;,
    pub predicted_side_effect_set: Vec&lt;SideEffect&gt;,
    pub confidence: ConformalSet,
    pub out_of_distribution_score: f32,
    pub granger_attestations: std::collections::HashMap&lt;SkillId, f32&gt;,
    pub witness_chain_offset: u64,
    pub degraded_status: bool,
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
}

impl RealityPrediction {
    /// Validates: every f32 finite; predicted_oracle_pass ∈ [0,1]; out_of_distribution_score ∈ [0,1];
    /// every granger_attestations value ∈ [0,1]; predicted_runtime_trace_summary.len() == 32.
    /// Placeholder error type until TASK-INFER-002 lands MejepaInferError.
    pub fn try_new(b: RealityPredictionBuilder) -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealityPredictionBuilder {
    pub predicted_oracle_pass: f32,
    pub predicted_failing_tests: Vec&lt;TestId&gt;,
    pub predicted_runtime_trace_summary: Vec&lt;f32&gt;,
    pub predicted_side_effect_set: Vec&lt;SideEffect&gt;,
    pub confidence: ConformalSet,
    pub out_of_distribution_score: f32,
    pub granger_attestations: std::collections::HashMap&lt;SkillId, f32&gt;,
    pub witness_chain_offset: u64,
    pub degraded_status: bool,
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum VerifyVerdict {
    Approve { reality_prediction: RealityPrediction, gates_passed: u8 },
    EscalateToHuman { failed_gate: FailedGate, reality_prediction: RealityPrediction },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailedGate {
    OutOfDistribution { score: f32 },
    LowConfidence { entropy: f32 },
    PredictedFailure { failing_tests: Vec&lt;TestId&gt; },
    GrangerRejection { skill: SkillId, p_value: f32 },
    WitnessChainBroken,
    SourceShaDrift { path: PathBuf },
    ConstellationGuardRejected { violating_embedders: Vec&lt;EmbedderId&gt;, violating_chunks: Vec&lt;ChunkId&gt; },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchBundle {
    pub pre_patch_sha: [u8; 32],
    pub post_patch_sha: [u8; 32],
    pub ast_diff: AstDiff,
    pub witness_chain_segment: Vec&lt;u8&gt;,   // len % 73 == 0; len &lt;= 73 * 8192
    pub claim_graph: ClaimGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstDiff {
    pub touched_paths: Vec&lt;PathBuf&gt;,
    pub hunks: Vec&lt;DiffHunk&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub path: PathBuf,
    pub start_line: u32,
    pub end_line: u32,
    pub pre_sha: [u8; 32],
    pub post_sha: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub task_id: TaskId,
    pub problem_statement: String,    // bounded 64 KiB
    pub test_stack: Vec&lt;TestId&gt;,
    pub environment: TaskEnvironment,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEnvironment {
    pub python_version: Option&lt;String&gt;,
    pub rust_toolchain: Option&lt;String&gt;,
    pub env_vars: BTreeMap&lt;String, String&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimGraph {
    pub skills: Vec&lt;SkillCitation&gt;,   // cap 256
    pub touched_files: Vec&lt;PathBuf&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCitation {
    pub skill: SkillId,
    pub path: PathBuf,
    pub expected_outcome: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TestId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillId {
    /// Closed taxonomy filled out by SPEC-CLAIM-GRAPH (out of scope here).
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderId {
    E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11,
    E12, E13, E14, E15, E16, E17, E18, E19, E20, E21,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ChunkId {
    pub file_sha: [u8; 32],
    pub language: Language,
    pub entity_type: EntityType,
    pub line_start: u32,
    pub line_end: u32,
    pub parent_chain_hash: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Function, Method, Class, Struct, Impl, Module, Trait, Interface,
    Macro, Enum, Union, Namespace, TopLevelStmt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SideEffect {
    FileWrite { path: PathBuf },
    FileDelete { path: PathBuf },
    NetworkCall { host: String, port: u16 },
    SubprocessSpawn { argv0: String },
    EnvVarMutation { name: String },
}
    </signature>
  </signatures>
  <constraints>
    - All public structs derive `Debug`, `Clone`, `Serialize`, `Deserialize`. Enums also derive `PartialEq`. Where natural, also derive `Eq`, `Hash`, `Copy`.
    - `#[serde(rename_all = "snake_case")]` on every enum
    - `#[serde(deny_unknown_fields)]` on PatchBundle, TaskContext, ClaimGraph, RealityPrediction (rejects extra MCP-side fields per audit-9 lesson)
    - `RealityPrediction::try_new` rejects NaN/Inf in any f32 field; rejects predicted_oracle_pass / out_of_distribution_score outside [0,1]; rejects predicted_runtime_trace_summary.len() != 32; rejects empty `confidence.outcome_set`
    - `ConformalSet::try_new` rejects empty outcome_set; rejects alpha outside [0.01, 0.50]; rejects duplicate outcomes
    - `ChunkId.line_end >= line_start` enforced via `try_new`
    - `ClaimGraph.skills.len() <= 256` enforced via `try_new`
    - `TaskContext.problem_statement.len() <= 65536` enforced via `try_new`
    - `PatchBundle.witness_chain_segment.len() % 73 == 0` enforced via `try_new`; max 73 * 8192 = 598_016 bytes
    - All field names match the TECH-INFERENCE data_models block byte-for-byte (consumer contract)
    - No `unwrap_or` on any confidence-related field
    - Rust 2021 edition; Rust 1.84+ per constitution
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds (pulls fresh crate into workspace)
    - `cargo test -p context-graph-mejepa types` shows ≥ 5 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `try_new_rejects_nan_predicted_oracle_pass` passes
    - Test `conformal_set_rejects_empty` passes
    - Test `failed_gate_serde_round_trips_all_seven_variants` passes
    - Test `patch_bundle_serde_round_trip` passes
    - Test `language_enum_eleven_variants_round_trip` passes
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/Cargo.toml:
  [package]
  name = "context-graph-mejepa"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  serde = { workspace = true, features = ["derive"] }
  serde_json = { workspace = true }
  context-graph-mejepa-instruments = { path = "../context-graph-mejepa-instruments" }
  context-graph-mejepa-corpus       = { path = "../context-graph-mejepa-corpus" }
  context-graph-witness             = { path = "../context-graph-witness" }

crates/context-graph-mejepa/src/lib.rs:
  pub mod types;
  pub use types::*;

crates/context-graph-mejepa/src/types.rs (high-level body):
  // ... struct + enum definitions per signatures block ...

  impl RealityPrediction {
      pub fn try_new(b: RealityPredictionBuilder) -> Result<Self, &'static str> {
          if !b.predicted_oracle_pass.is_finite() { return Err("predicted_oracle_pass non-finite"); }
          if !(0.0..=1.0).contains(&b.predicted_oracle_pass) { return Err("predicted_oracle_pass out of range"); }
          if !b.out_of_distribution_score.is_finite() { return Err("out_of_distribution_score non-finite"); }
          if !(0.0..=1.0).contains(&b.out_of_distribution_score) { return Err("out_of_distribution_score out of range"); }
          if b.predicted_runtime_trace_summary.len() != 32 { return Err("predicted_runtime_trace_summary.len() != 32"); }
          if b.predicted_runtime_trace_summary.iter().any(|x| !x.is_finite()) { return Err("trace_summary non-finite"); }
          if !b.delta_omega_mean.is_finite() || !(0.0..=1.0).contains(&b.delta_omega_mean) { return Err("delta_omega_mean invalid"); }
          if !b.delta_xi_mean.is_finite() || !(0.0..=1.0).contains(&b.delta_xi_mean) { return Err("delta_xi_mean invalid"); }
          for (_, p) in &b.granger_attestations {
              if !p.is_finite() || !(0.0..=1.0).contains(p) { return Err("granger p_value invalid"); }
          }
          if b.confidence.outcome_set.is_empty() { return Err("confidence outcome_set empty"); }
          // construct
          Ok(Self { ... })
      }
  }

  impl ConformalSet {
      pub fn try_new(alpha: f32, outcome_set: Vec<OracleOutcome>) -> Result<Self, &'static str> {
          if !alpha.is_finite() || !(0.01..=0.50).contains(&alpha) { return Err("alpha out of range [0.01, 0.50]"); }
          if outcome_set.is_empty() { return Err("outcome_set empty"); }
          let mut seen = std::collections::HashSet::new();
          for o in &outcome_set { if !seen.insert(*o) { return Err("outcome_set has duplicates"); } }
          Ok(Self { alpha, outcome_set })
      }
  }

  // Tests:
  #[cfg(test)]
  mod tests {
      use super::*;
      // each invariant proven with NaN/empty/duplicate/oversize fixtures
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/Cargo.toml">workspace member metadata + deps to mejepa-instruments + mejepa-corpus + witness + serde</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">crate-root with `pub mod types;` and `pub use types::*;`</file>
  <file path="crates/context-graph-mejepa/src/types.rs">all 9 public types + RealityPredictionBuilder + tests module</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml (workspace root)">add `crates/context-graph-mejepa` to `[workspace.members]`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows ≥ 5 passing tests in the types module</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>RealityPrediction::try_new returns Err on NaN in predicted_oracle_pass</criterion>
  <criterion>RealityPrediction::try_new returns Err on outcome_set empty</criterion>
  <criterion>ConformalSet::try_new returns Err on duplicate outcomes</criterion>
  <criterion>FailedGate serde round-trips all 7 variants byte-equal</criterion>
  <criterion>Language enum has exactly 11 variants in declaration order rust→php</criterion>
  <criterion>EntityType enum has exactly 13 variants per constitution &lt;tct_invariants&gt; rule 5</criterion>
  <criterion>EmbedderId enum has exactly 21 variants E1..E21</criterion>
  <criterion>PatchBundle witness_chain_segment.len() validation rejects len % 73 != 0</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa types</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
