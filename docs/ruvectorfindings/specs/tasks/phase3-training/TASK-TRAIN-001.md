# TASK-TRAIN-001 — Foundation data models: Trainer + BatchSampler + TrainCertWriter + HoldoutEvaluator + EpochReport + HoldoutReport + EpochSummary + EpochWitnessEntry

```xml
<task_spec id="TASK-TRAIN-001" version="1.0">

<metadata>
  <title>Phase 3 trainer foundation data model structs (no logic)</title>
  <status>blocked</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-TRAIN-01..05">structural carriers for AdamW, sampler, cert writer, holdout evaluator, witness chain</item>
    <item ref="entity:Trainer">data_models Trainer</item>
    <item ref="entity:BatchSampler">data_models BatchSampler</item>
    <item ref="entity:TrainCertWriter">data_models TrainCertWriter</item>
    <item ref="entity:HoldoutEvaluator">data_models HoldoutEvaluator</item>
    <item ref="entity:EpochWitnessChain">data_models EpochWitnessChain</item>
    <item ref="entity:EpochReport">data_models EpochReport</item>
    <item ref="entity:HoldoutReport">data_models HoldoutReport</item>
    <item ref="entity:EpochSummary">data_models EpochSummary</item>
    <item ref="entity:EpochWitnessEntry">data_models EpochWitnessEntry</item>
  </implements>
  <depends_on>
    <task_ref>../utml-learning-formula/TASK-UTML-001</task_ref>
    <task_ref>../utml-learning-formula/TASK-UTML-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>5 created, 1 modified</estimated_files>
</metadata>

<context>
The Trainer state machine and its dependent foundation structs (BatchSampler, TrainCertWriter, HoldoutEvaluator, EpochWitnessChain, plus the `EpochReport`/`HoldoutReport`/`EpochSummary`/`EpochWitnessEntry` value types) are referenced by every later TRAINING task. This task ships the type system only; behavior fills in via TASK-TRAIN-003..012. UTML-domain types (LearningSignal, TrainingCertificate, ClassifierVerdict, LStepWindow) are imported from sibling tasks.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants, performance_budgets, security_requirements)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (data_requirements: TrainingConfig, LossCoefficients, HoldoutReport, EpochSummary)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (data_models: Trainer, BatchSampler, TrainCertWriter, HoldoutEvaluator, EpochReport, HoldoutReport, EpochWitnessChain, EpochWitnessEntry)</file>
  <file purpose="prior_tasks">docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-001.md (UtmlError, LearningSignal, TrainingCertificate)</file>
  <file purpose="prior_tasks">docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-009.md (LStepWindow, ClassifierVerdict)</file>
  <file purpose="instrument_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel)</file>
</input_context_files>

<prerequisites>
  <check>UTML-001..009 complete</check>
  <check>candle-core, rocksdb, ed25519-dalek deps available in workspace</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/trainer/mod.rs` with Trainer struct (fields per TECH-TRAINING.data_models.Trainer; no methods)
    - Create `src/sampler/mod.rs` with BatchSampler struct (fields per TECH-TRAINING.data_models.BatchSampler; no methods)
    - Create `src/cert/writer.rs` with TrainCertWriter struct
    - Create `src/eval/mod.rs` with HoldoutEvaluator + HoldoutReport + EpochReport + EpochSummary + EpochWitnessEntry + EpochWitnessChain
    - Create `src/aux/mod.rs` placeholder with `pub struct AuxHeadEnsemble { ... }` skeleton (real heads land in TASK-TRAIN-004)
    - Modify `src/lib.rs` to expose `pub mod trainer; pub mod sampler; pub mod cert; pub mod eval; pub mod aux;`
    - All structs derive Debug + Clone + Serialize + Deserialize where appropriate (skip Arc&lt;DB&gt;; mark `#[serde(skip)]`)
    - Add `MutationCategory` enum (8 variants: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) and `Lang` enum (11 variants: Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php) — these are mid-foundation; TASK-TRAIN-002 puts them next to the rest of the enums; here just declare as forward refs in eval/mod.rs and have TASK-TRAIN-002 move them
    - HoldoutReport.per_mutation_category_accuracy and per_language_accuracy use `HashMap<K, f32>` with the enum as key (serde supports this with `#[serde]` attrs)
  </in_scope>
  <out_of_scope>
    - Method bodies (TASK-TRAIN-003..012)
    - LoRA state struct (TASK-TRAIN-012)
    - Patch similarity graph implementation (TASK-TRAIN-007)
    - Stoer-Wagner mincut wiring (TASK-TRAIN-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/trainer/mod.rs">
use std::sync::Arc;
use rocksdb::DB;
use crate::sampler::BatchSampler;
use crate::cert::writer::TrainCertWriter;
use crate::eval::HoldoutEvaluator;
use crate::eval::EpochWitnessChain;
use crate::aux::AuxHeadEnsemble;
use crate::config::TrainingConfig;
use crate::error::TrainerError;

pub struct Trainer {
    pub config: TrainingConfig,
    // pub predictor: MeJepaPredictor (Phase 2 dep; placeholder type for now)
    // pub target: FrozenTarget (Phase 2 dep; placeholder)
    pub aux_heads: AuxHeadEnsemble,
    // pub optimizer: AdamW (TASK-TRAIN-003)
    pub sampler: BatchSampler,
    pub cert_writer: TrainCertWriter,
    pub holdout_evaluator: HoldoutEvaluator,
    pub epoch_witness: EpochWitnessChain,
    // pub distiller: AuxHeadDistiller (TASK-TRAIN-008)
    // pub lora_state: Option&lt;LoraState&gt; (TASK-TRAIN-012)
    pub rocksdb: Arc&lt;DB&gt;,
    pub step: u64,
    pub best_holdout_agreement: f32,
    pub grad_norm_running_mean: f32,
    pub device: candle_core::Device,
    pub dtype: candle_core::DType,
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/sampler/mod.rs">
use crate::config::TrainingConfig;
// SplitMix64 reused from context-graph-mejepa-corpus
use context_graph_mejepa_corpus::prng::SplitMix64;

pub struct PatchSimilarityGraph {
    // adjacency: cosine(E_Diff(T_i), E_Diff(T_j)) &gt; 0.7
    pub neighbors: Vec&lt;Vec&lt;usize&gt;&gt;, // neighbors[i] = task indices with cos &gt; threshold
}

pub struct BatchSampler {
    pub config: TrainingConfig,
    pub weights: Vec&lt;f32&gt;,
    pub overrides: Vec&lt;bool&gt;,
    pub ages_days: Vec&lt;f32&gt;,
    pub l_steps: Vec&lt;f32&gt;,
    pub task_ids: Vec&lt;String&gt;, // string TaskId placeholder
    pub patch_similarity_graph: PatchSimilarityGraph,
    pub rng: SplitMix64,
    pub force_indices: Vec&lt;usize&gt;,
    pub drop_indices: Vec&lt;usize&gt;,
    pub regular_indices: Vec&lt;usize&gt;,
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/cert/writer.rs">
use std::sync::Arc;
use std::collections::HashMap;
use rocksdb::{DB, ColumnFamily};

pub struct TrainCertWriter {
    pub rocksdb: Arc&lt;DB&gt;,
    pub cf_train_certs_name: String, // "CF_MEJEPA_TRAIN_CERTS"
    pub last_self_hash: String,      // 64-hex sha256
    pub cert_count_since_fsync: u64,
    pub code_version: String,        // 40-hex git sha
    pub corpus_sha: String,          // 64-hex
    pub embedder_versions: HashMap&lt;String, String&gt;, // 21 keys
    pub frozen_at: String,           // ISO 8601
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/eval/mod.rs">
use std::sync::Arc;
use std::collections::HashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use ed25519_dalek::SigningKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    KnownGood, SubtleFlip, OffByOne, SwapVariable,
    DeleteTestCall, WrongFile, OverEngineer, CompileError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Rust, Python, JavaScript, TypeScript, Go, Java,
    C, Cpp, CSharp, Ruby, Php,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldoutReport {
    pub step: u64,
    pub prediction_oracle_agreement: f32,
    pub conformal_coverage_calibration: f32,
    pub per_mutation_category_accuracy: HashMap&lt;MutationCategory, f32&gt;,
    pub per_language_accuracy: HashMap&lt;Lang, f32&gt;,
    pub predictor_redundancy_pairwise_mi: f32,
    pub gtau_pass_rate: f32,
    pub generic_only_warning: Option&lt;String&gt;,
    pub phase3_dod_passed: Option&lt;bool&gt;,
    pub timestamp_iso8601: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochSummary {
    pub epoch: u32,
    pub mean_l_step: f32,
    pub mean_delta_p: f32,
    pub mean_delta_k: f32,
    pub mean_delta_omega: f32,
    pub mean_delta_xi: f32,
    pub holdout_agreement: f32,
    pub best_category: MutationCategory,
    pub worst_category: MutationCategory,
    pub best_language: Lang,
    pub worst_language: Lang,
    pub total_steps_this_epoch: u64,
    pub skipped_steps_this_epoch: u64,
    pub parent_witness_hash: String, // 64-hex
    pub self_hash: String,           // 64-hex
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochWitnessEntry {
    pub bytes: Vec&lt;u8&gt;, // exactly 73 bytes; serde via base64 helper
    pub parent_witness_hash: [u8; 32],
    pub self_hash: [u8; 32],
    pub ed25519_signature: [u8; 64],
    pub layout: String, // documented byte layout
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochReport {
    pub epoch: u32,
    pub summary: EpochSummary,
    pub witness_entry: EpochWitnessEntry,
    pub phase3_dod_passed_at_this_epoch: Option&lt;bool&gt;,
}

pub struct HoldoutEvaluator {
    // pub holdout_dataset: HoldoutDataset (TASK-TRAIN-011)
    // pub calibration_dataset: CalibrationDataset (TASK-TRAIN-011)
    // pub constellation: Option&lt;ConstellationGuard&gt; (Phase 4b)
    pub rocksdb: Arc&lt;DB&gt;,
    pub cf_holdout_reports_name: String, // "CF_MEJEPA_HOLDOUT_REPORTS"
}

pub struct EpochWitnessChain {
    pub rocksdb: Arc&lt;DB&gt;,
    pub cf_epoch_witness_name: String, // "CF_MEJEPA_EPOCH_WITNESS"
    pub last_epoch_hash: [u8; 32],
    #[allow(dead_code)]
    pub ed25519_signing_key: SigningKey,
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/aux/mod.rs">
//! AuxHeadEnsemble: warnings + runtime + rss + traceback + hunk + collateral + reasoning + operator_match.
//! Skeleton; real heads ship in TASK-TRAIN-004.
pub struct AuxHeadEnsemble {
    // 8 aux heads; placeholder until TASK-TRAIN-004
}
    </signature>
  </signatures>
  <constraints>
    - All public structs have `#[derive(Debug)]` minimum
    - Serde derives for all value structs (HoldoutReport, EpochSummary, EpochWitnessEntry, EpochReport, MutationCategory, Lang)
    - State-holder structs (Trainer, BatchSampler, TrainCertWriter, HoldoutEvaluator, EpochWitnessChain) NOT serializable — they hold Arc&lt;DB&gt; and SigningKey
    - SigningKey field marked `#[allow(dead_code)]` until TASK-TRAIN-010
    - All files &lt; 400 lines
    - No method bodies in this task; only struct + enum definitions
    - HashMap keys use enum types directly; serde for HashMap&lt;Enum, f32&gt; works because enum derives Hash+Eq+Serialize+Deserialize
    - `cf_*_name` is `String` not `ColumnFamily` because CF handles are obtained at construction time (avoids lifetime issues)
  </constraints>
  <verification>
    - cargo build/test/clippy clean
    - serde round-trip test for HoldoutReport (synthetic) and EpochSummary
  </verification>
</definition_of_done>

<pseudo_code>
// All struct-only; no logic.
// Cargo.toml: add rocksdb 0.21, ed25519-dalek 2.x, candle-core 0.9, context-graph-mejepa-corpus path dep.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/trainer/mod.rs">Trainer struct skeleton</file>
  <file path="crates/context-graph-mejepa-train/src/sampler/mod.rs">BatchSampler + PatchSimilarityGraph structs</file>
  <file path="crates/context-graph-mejepa-train/src/cert/writer.rs">TrainCertWriter struct</file>
  <file path="crates/context-graph-mejepa-train/src/eval/mod.rs">HoldoutEvaluator + EpochWitnessChain + value types + MutationCategory + Lang</file>
  <file path="crates/context-graph-mejepa-train/src/aux/mod.rs">AuxHeadEnsemble skeleton</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">re-export pub mod trainer/sampler/cert/eval/aux</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add rocksdb, ed25519-dalek, context-graph-mejepa-corpus deps</file>
</files_to_modify>

<validation_criteria>
  <criterion>All structs declared and compile</criterion>
  <criterion>HoldoutReport + EpochSummary serde round-trip pass</criterion>
  <criterion>MutationCategory has 8 variants; Lang has 11 variants</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
