# TASK-EVAL-002 — Internal types + error taxonomy (ActiveLearningQueueState, ActiveLearningQueueEntry, ActiveLearningLabel, PatchGraph, EvalError, QueueError, GraphError, EvalConfig)

```xml
<task_spec id="TASK-EVAL-002" version="1.0">

<metadata>
  <title>Define internal eval types + error taxonomy in `crates/context-graph-mejepa/src/eval/{queue,task_graph,error,config}.rs`</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-EVAL-05">ActiveLearningQueue + ActiveLearningQueueEntry + ActiveLearningLabel + LabelMethod (struct shape only — enqueue/dedup/eviction logic lands in TASK-EVAL-009)</item>
    <item ref="REQ-EVAL-07">PatchGraph + PatchGraphEdge struct shape (graph build logic lands in TASK-EVAL-010)</item>
    <item ref="REQ-EVAL-13">Fail-closed error taxonomy: 14 EvalError variants + .code() SCREAMING_SNAKE tags + .log_context()</item>
    <item ref="component:EvalError">SCREAMING_SNAKE error tags per constitution failure_taxonomy extension</item>
    <item ref="component:ActiveLearningQueueState">heap + entries_by_panel_sha + max_observed_outcome_set_len</item>
    <item ref="component:EvalConfig">alpha, thresholds, min_samples, shrinkage, queue capacity, graph top-k, λ, etc.</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-001</task_ref><!-- ShipGateThresholds, MutationCategory, LabelMethod, etc. -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>4 created (queue.rs, task_graph.rs, error.rs, config.rs); 1 modified (eval/mod.rs)</estimated_files>
</metadata>

<context>
This task lands the internal helper types: the active-learning queue state (BinaryHeap + dedup
index), the patch-similarity graph value-types, the structured error taxonomy with `.code()` +
`.log_context()`, and the `EvalConfig` knob block. After this task, `EvalReport::try_new` can be
upgraded from the `&'static str` placeholder to `Result<Self, EvalError>`. The error type is the
single fail-closed surface for the entire eval domain — every fallible function returns
`Result<T, EvalError>` per constitution `<error_handling>`.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy extension; error_handling rule "robust error logging: every failure logs context")</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-05 active learning queue; REQ-EVAL-07 patch similarity graph; REQ-EVAL-13 error taxonomy 9 codes)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (data_models: ActiveLearningQueueState, ActiveLearningQueueEntry, ActiveLearningLabel, LabelProvenance, PatchGraph, PatchGraphEdge, EvalError 14 variants, QueueError 4 variants, GraphError 5 variants, EvalConfig 18 knobs)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase8-evaluation/TASK-EVAL-001.md (ShipGateThresholds, LabelMethod, MutationCategory, etc.)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase4-inference/TASK-INFER-002.md (MejepaInferError pattern — clone the .code() + .log_context() approach)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-001 shipped: `crates/context-graph-mejepa/src/eval/types.rs` exposes `ShipGateThresholds`, `LabelMethod`, `MutationCategory`, `SplitMode`</check>
  <check>`thiserror`, `tracing`, `parking_lot`, `ulid` available as workspace deps</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/queue.rs` with:
      - `ActiveLearningQueueEntry` struct (panel_sha, prediction_id, conformal_set_size, ood_score, signal_value, enqueued_at, language, suggested_label_method) implementing `Ord` by signal_value descending for `BinaryHeap`
      - `ActiveLearningQueueState` struct (heap: BinaryHeap<ActiveLearningQueueEntry>, entries_by_panel_sha: HashMap<[u8; 32], usize>, max_observed_outcome_set_len: usize, capacity: usize)
      - `ActiveLearningQueueState::new(capacity: usize)` constructor (max_observed_outcome_set_len starts at 4 lower bound)
      - `ActiveLearningQueueState::top_n(n, language_filter) -> Vec<ActiveLearningQueueEntry>` (read-only; the enqueue + remove + load_from_storage methods land in TASK-EVAL-009 once RocksDB store types exist)
      - `LabelProvenance` struct (corpus_sha, code_version, calibration_version)
      - `ActiveLearningLabel` struct (panel_sha, oracle_outcome, label_method, operator_id, labelled_at, provenance)
    - Create `crates/context-graph-mejepa/src/eval/task_graph.rs` with:
      - `PatchGraphEdge` struct (source_task_sha, target_task_sha, cosine)
      - `PatchGraph` struct (edges, adjacency: BTreeMap<[u8; 32], Vec<([u8; 32], f32)>>, corpus_sha, built_at)
      - `PatchGraph::new(corpus_sha)` constructor (empty graph)
      - `PatchGraph::similar_tasks(&self, source: &[u8; 32], cosine_threshold: f32, limit: usize) -> Vec<([u8; 32], f32)>` read-only query (the build logic lands in TASK-EVAL-010)
    - Create `crates/context-graph-mejepa/src/eval/error.rs` with:
      - `EvalError` enum (14 variants per TECH-EVALUATION data_models): EmptyHoldout, HoldoutLeakage, InsufficientSamples, ShipGateRegression, LanguageOodDrift, LanguageConformalDrift, ReportPersistFail, NoReportAvailable, TaskGraphRebuildFail, ActiveLearningBelowTarget, NanDetected, EmptyHoldoutLanguage, InsufficientLiveData, TaskGraphSkip, MissingProvenance, ReportTooLarge, PanelNotInQueue (extras to support EvalReport::try_new strict returns + label apply)
      - `QueueError` enum (4 variants): OodPanelRejected, NotUncertain, DuplicatePanel, NanDetected
      - `GraphError` enum (5 variants): EmptyCorpus, EmbedderUnavailable, DimMismatch, CorpusShaUnchanged, TaskSkip
      - `impl EvalError { pub fn code(&self) -> &'static str { /* match on variants */ } }` returning SCREAMING_SNAKE per failure_taxonomy
      - `impl EvalError { pub fn log_context(&self, log_target: &str) { tracing::error!(target = log_target, code = self.code(), payload = ?self, "..."); } }`
      - Same `.code()` + `.log_context()` for QueueError + GraphError
      - `From<...>` conversions where natural (e.g., `From<rocksdb::Error> for EvalError::ReportPersistFail`)
      - Upgrade `EvalReport::try_new` (and `serialize_canonical`) signatures from `Result<_, &'static str>` to `Result<_, EvalError>` (in `eval/report.rs`)
    - Create `crates/context-graph-mejepa/src/eval/config.rs` with:
      - `EvalConfig` struct with the 18 knobs from TECH-EVALUATION data_models (alpha, thresholds: ShipGateThresholds, min_samples_per_cell, min_samples_per_language, enable_bayesian_shrinkage, active_learning_queue_capacity, task_graph_top_k, task_graph_cosine_threshold, state_transfer_lambda, state_transfer_wasserstein_samples, ood_drift_window_weeks, ood_drift_threshold, m_t_window_steps, auto_route_docker_rerun, auto_route_top_n_per_day, active_learning_corpus_growth_target_pct, aux_head_divergence_threshold)
      - `EvalConfig::default()` returning constitution-pinned defaults (alpha=0.10, thresholds=ShipGateThresholds::default(), min_samples_per_cell=30, min_samples_per_language=30, enable_bayesian_shrinkage=true, active_learning_queue_capacity=10_000, task_graph_top_k=32, task_graph_cosine_threshold=0.7, state_transfer_lambda=1.0, state_transfer_wasserstein_samples=5_000, ood_drift_window_weeks=4, ood_drift_threshold=0.10, m_t_window_steps=1_000, auto_route_docker_rerun=false, auto_route_top_n_per_day=5, active_learning_corpus_growth_target_pct=1.0, aux_head_divergence_threshold=0.20)
      - `EvalConfig::validate(&self) -> Result<(), EvalError>` checking finite + ranges (alpha ∈ [0.01, 0.50]; lambda > 0; ood_auc_min ≥ 0; etc.)
    - Update `crates/context-graph-mejepa/src/eval/mod.rs` to declare `pub mod queue; pub mod task_graph; pub mod error; pub mod config;` and re-export the public types
    - Add unit tests in each new file `#[cfg(test)] mod tests` for:
      - `ActiveLearningQueueEntry::cmp` orders by signal_value descending (BinaryHeap pops max-signal first)
      - `ActiveLearningQueueState::new` initializes max_observed_outcome_set_len to 4 lower bound
      - `PatchGraph::similar_tasks` respects cosine_threshold + limit
      - `EvalError::code` returns the expected SCREAMING_SNAKE tag for every variant
      - `QueueError::code` covers all 4 variants
      - `GraphError::code` covers all 5 variants
      - `EvalConfig::default` matches constitution-pinned values
      - `EvalConfig::validate` rejects alpha=0.0 (out of [0.01, 0.50])
      - `EvalConfig::validate` rejects state_transfer_lambda=0.0 (must be positive)
  </in_scope>
  <out_of_scope>
    - `EvalRunner::new` + run_holdout_eval / check_ship_gate / enqueue_uncertain / build_patch_similarity_graph (TASK-EVAL-004 onward)
    - `ActiveLearningQueueState::enqueue` / `remove` / `load_from_storage` (TASK-EVAL-009 — needs RocksDB store types)
    - `PatchGraph::build` (TASK-EVAL-010 — needs E_Diff embedder)
    - RocksDB CFs (TASK-EVAL-003)
    - MCP handlers (TASK-EVAL-013, TASK-EVAL-014)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/queue.rs">
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use serde::{Deserialize, Serialize};
use crate::types::{Language, OracleOutcome};
use super::types::LabelMethod;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningQueueEntry {
    pub panel_sha: [u8; 32],
    pub prediction_id: u64,
    pub conformal_set_size: usize,
    pub ood_score: f32,
    pub signal_value: f32,
    pub enqueued_at: i64,
    pub language: Language,
    pub suggested_label_method: LabelMethod,
}

impl Eq for ActiveLearningQueueEntry {}
impl PartialOrd for ActiveLearningQueueEntry {
    fn partial_cmp(&self, other: &Self) -&gt; Option&lt;Ordering&gt; { Some(self.cmp(other)) }
}
impl Ord for ActiveLearningQueueEntry {
    fn cmp(&self, other: &Self) -&gt; Ordering {
        // BinaryHeap is max-heap; sort by signal_value descending
        self.signal_value.partial_cmp(&other.signal_value).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug)]
pub struct ActiveLearningQueueState {
    pub heap: BinaryHeap&lt;ActiveLearningQueueEntry&gt;,
    pub entries_by_panel_sha: HashMap&lt;[u8; 32], ActiveLearningQueueEntry&gt;,
    pub max_observed_outcome_set_len: usize,
    pub capacity: usize,
}

impl ActiveLearningQueueState {
    pub fn new(capacity: usize) -&gt; Self;
    pub fn top_n(&self, n: usize, language_filter: Option&lt;Language&gt;) -&gt; Vec&lt;ActiveLearningQueueEntry&gt;;
    pub fn len(&self) -&gt; usize;
    pub fn contains(&self, panel_sha: &[u8; 32]) -&gt; bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LabelProvenance {
    pub corpus_sha: [u8; 32],
    pub code_version: String,
    pub calibration_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningLabel {
    pub panel_sha: [u8; 32],
    pub oracle_outcome: OracleOutcome,
    pub label_method: LabelMethod,
    pub operator_id: String,
    pub labelled_at: i64,
    pub provenance: LabelProvenance,
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/task_graph.rs">
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PatchGraphEdge {
    pub source_task_sha: [u8; 32],
    pub target_task_sha: [u8; 32],
    pub cosine: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PatchGraph {
    pub edges: Vec&lt;PatchGraphEdge&gt;,
    pub adjacency: BTreeMap&lt;[u8; 32], Vec&lt;([u8; 32], f32)&gt;&gt;,
    pub corpus_sha: [u8; 32],
    pub built_at: i64,
}

impl PatchGraph {
    pub fn new(corpus_sha: [u8; 32]) -&gt; Self;
    pub fn similar_tasks(&self, source: &[u8; 32], cosine_threshold: f32, limit: usize) -&gt; Vec&lt;([u8; 32], f32)&gt;;
    pub fn node_count(&self) -&gt; usize;
    pub fn edge_count(&self) -&gt; usize;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/error.rs">
use std::path::PathBuf;
use thiserror::Error;
use crate::types::{Language, TaskId};
use super::types::MutationCategory;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum EvalError {
    #[error("MEJEPA_EVAL_EMPTY_HOLDOUT: holdout split is empty")]
    EmptyHoldout,

    #[error("MEJEPA_EVAL_HOLDOUT_LEAKAGE: task_id {task_id:?} found in {found_in:?}")]
    HoldoutLeakage { task_id: TaskId, found_in: Vec&lt;String&gt; },

    #[error("MEJEPA_EVAL_INSUFFICIENT_SAMPLES: cell {cell:?} has {observed} samples, required {required}")]
    InsufficientSamples { cell: (MutationCategory, Language), required: usize, observed: usize },

    #[error("MEJEPA_EVAL_SHIP_GATE_REGRESSION: cell {cell:?} dropped {prior} → {current} (delta {delta_pct}%)")]
    ShipGateRegression { cell: (MutationCategory, Language), prior: f32, current: f32, delta_pct: f32 },

    #[error("MEJEPA_EVAL_LANGUAGE_OOD_DRIFT: language {language:?} AUC drifted {prior_auc} → {current_auc} (Δ {delta}) over {window_weeks} weeks")]
    LanguageOodDrift { language: Language, prior_auc: f32, current_auc: f32, delta: f32, window_weeks: usize },

    #[error("MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT: language {language:?} stated {stated} empirical {empirical} (delta {delta_pct}%)")]
    LanguageConformalDrift { language: Language, stated: f32, empirical: f32, delta_pct: f32 },

    #[error("MEJEPA_EVAL_REPORT_PERSIST_FAIL: cf={cf} backend_error={backend_error}")]
    ReportPersistFail { cf: String, backend_error: String },

    #[error("MEJEPA_EVAL_NO_REPORT_AVAILABLE")]
    NoReportAvailable,

    #[error("MEJEPA_EVAL_TASK_GRAPH_REBUILD_FAIL: corpus_sha={corpus_sha:?} reason={reason}")]
    TaskGraphRebuildFail { corpus_sha: [u8; 32], failed_at_task: Option&lt;TaskId&gt;, reason: String },

    #[error("MEJEPA_EVAL_ACTIVE_LEARNING_BELOW_TARGET: growth_pct={growth_pct} target_pct={target_pct}")]
    ActiveLearningBelowTarget { growth_pct: f32, target_pct: f32 },

    #[error("MEJEPA_EVAL_NAN_DETECTED: field={field} cell={cell:?}")]
    NanDetected { field: String, cell: Option&lt;String&gt; },

    #[error("MEJEPA_EVAL_EMPTY_HOLDOUT_LANGUAGE: language={language:?}")]
    EmptyHoldoutLanguage { language: Language },

    #[error("MEJEPA_EVAL_INSUFFICIENT_LIVE_DATA: count={count} required={required}")]
    InsufficientLiveData { count: usize, required: usize },

    #[error("MEJEPA_EVAL_TASK_GRAPH_SKIP: task_id={task_id:?} reason={reason}")]
    TaskGraphSkip { task_id: TaskId, reason: String },

    #[error("MEJEPA_EVAL_MISSING_PROVENANCE: field={field}")]
    MissingProvenance { field: String },

    #[error("MEJEPA_EVAL_REPORT_TOO_LARGE: size_bytes={size_bytes} limit_bytes={limit_bytes}")]
    ReportTooLarge { size_bytes: usize, limit_bytes: usize },

    #[error("MEJEPA_EVAL_PANEL_NOT_IN_QUEUE: panel_sha={panel_sha:?}")]
    PanelNotInQueue { panel_sha: [u8; 32] },
}

impl EvalError {
    pub fn code(&self) -&gt; &amp;'static str;
    pub fn log_context(&self, log_target: &amp;str);
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum QueueError {
    #[error("MEJEPA_EVAL_OOD_PANEL_REJECTED: ood_score={ood_score}")]
    OodPanelRejected { ood_score: f32 },
    #[error("MEJEPA_EVAL_NOT_UNCERTAIN: outcome_set_size={outcome_set_size}")]
    NotUncertain { outcome_set_size: usize },
    #[error("MEJEPA_EVAL_DUPLICATE_PANEL: panel_sha={panel_sha:?}")]
    DuplicatePanel { panel_sha: [u8; 32] },
    #[error("MEJEPA_EVAL_QUEUE_NAN_DETECTED: field={field}")]
    NanDetected { field: String },
}

impl QueueError {
    pub fn code(&self) -&gt; &amp;'static str;
    pub fn log_context(&self, log_target: &amp;str);
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum GraphError {
    #[error("MEJEPA_EVAL_EMPTY_CORPUS")]
    EmptyCorpus,
    #[error("MEJEPA_EVAL_EMBEDDER_UNAVAILABLE: embedder={embedder}")]
    EmbedderUnavailable { embedder: String },
    #[error("MEJEPA_EVAL_DIM_MISMATCH: expected={expected} actual={actual}")]
    DimMismatch { expected: usize, actual: usize },
    #[error("MEJEPA_EVAL_CORPUS_SHA_UNCHANGED: cached_at={cached_at}")]
    CorpusShaUnchanged { cached_at: i64 },
    #[error("MEJEPA_EVAL_TASK_SKIP: task_id={task_id:?} reason={reason}")]
    TaskSkip { task_id: TaskId, reason: String },
}

impl GraphError {
    pub fn code(&self) -&gt; &amp;'static str;
    pub fn log_context(&self, log_target: &amp;str);
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/config.rs">
use serde::{Deserialize, Serialize};
use super::types::ShipGateThresholds;
use super::error::EvalError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvalConfig {
    pub alpha: f32,
    pub thresholds: ShipGateThresholds,
    pub min_samples_per_cell: usize,
    pub min_samples_per_language: usize,
    pub enable_bayesian_shrinkage: bool,
    pub active_learning_queue_capacity: usize,
    pub task_graph_top_k: usize,
    pub task_graph_cosine_threshold: f32,
    pub state_transfer_lambda: f32,
    pub state_transfer_wasserstein_samples: usize,
    pub ood_drift_window_weeks: usize,
    pub ood_drift_threshold: f32,
    pub m_t_window_steps: usize,
    pub auto_route_docker_rerun: bool,
    pub auto_route_top_n_per_day: usize,
    pub active_learning_corpus_growth_target_pct: f32,
    pub aux_head_divergence_threshold: f32,
}

impl Default for EvalConfig {
    fn default() -&gt; Self;
}

impl EvalConfig {
    pub fn validate(&self) -&gt; Result&lt;(), EvalError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - All public structs derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq` where natural
    - `#[serde(deny_unknown_fields)]` on every cross-boundary struct
    - `EvalError` uses `thiserror` with the `#[error(...)]` message embedding the SCREAMING_SNAKE code as the FIRST WORD of the message, AND `.code()` returns the same tag string
    - `QueueError` and `GraphError` follow the same `MEJEPA_EVAL_*` namespace per `<mcp_tool_namespace_lock>` derived patterns
    - `.log_context(target)` emits a `tracing::error!` event with `code = self.code()` and `payload = ?self`; never panics, never allocates beyond the format
    - `ActiveLearningQueueEntry::cmp` MUST handle NaN signal_value (use `partial_cmp(...).unwrap_or(Ordering::Equal)`); never panic on weird floats
    - `EvalConfig::validate` rejects:
      - alpha outside [0.01, 0.50]
      - state_transfer_lambda <= 0.0 or non-finite
      - any threshold value outside [-1.0, 1.0] (correlation thresholds) or [0.0, 1.0] (rates) or [0.0, 100.0] (percentages)
      - active_learning_queue_capacity == 0
      - task_graph_top_k == 0
      - task_graph_cosine_threshold outside [-1.0, 1.0]
    - Upgrade `EvalReport::try_new` and `EvalReport::serialize_canonical` (in `eval/report.rs`) signatures from `Result<_, &'static str>` to `Result<_, EvalError>`; map old string errors to `EvalError::NanDetected{...}` / `MissingProvenance{...}` / `ReportTooLarge{...}` as appropriate
    - No `unwrap_or` on confidence-related fields anywhere
    - `ActiveLearningQueueState::entries_by_panel_sha` stores the `ActiveLearningQueueEntry` clone (not just an index) so `top_n` and `contains` are O(1) without heap traversal — note: TECH-EVALUATION's `usize` index is a viable alternative; in this task we choose the `Entry` clone for read-side simplicity (TASK-EVAL-009 may revise if memory bound becomes an issue)
    - Rust 2021 edition; Rust 1.84+ per constitution
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa eval::queue` shows ≥ 3 passing tests
    - `cargo test -p context-graph-mejepa eval::task_graph` shows ≥ 3 passing tests
    - `cargo test -p context-graph-mejepa eval::error` shows ≥ 4 passing tests (one per error type + one per .code() round-trip)
    - `cargo test -p context-graph-mejepa eval::config` shows ≥ 3 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `active_learning_queue_entry_orders_by_signal_value_desc` passes (BinaryHeap pops max first)
    - Test `eval_error_code_covers_all_variants` passes (every variant has a unique SCREAMING_SNAKE tag)
    - Test `eval_config_default_constitution_pinned` passes
    - Test `eval_config_validate_rejects_alpha_zero` passes
    - Test `eval_config_validate_rejects_lambda_zero` passes
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/queue.rs:
  impl ActiveLearningQueueState {
      pub fn new(capacity: usize) -> Self {
          Self {
              heap: BinaryHeap::with_capacity(capacity),
              entries_by_panel_sha: HashMap::new(),
              max_observed_outcome_set_len: 4,
              capacity,
          }
      }

      pub fn top_n(&self, n: usize, language_filter: Option<Language>) -> Vec<ActiveLearningQueueEntry> {
          // BinaryHeap doesn't preserve order on iter; clone to Vec, sort by signal_value desc, take n
          let mut items: Vec<_> = self.heap.iter().cloned().collect();
          items.sort_by(|a, b| b.signal_value.partial_cmp(&a.signal_value).unwrap_or(Ordering::Equal));
          items.into_iter()
              .filter(|e| language_filter.map_or(true, |l| e.language == l))
              .take(n)
              .collect()
      }

      pub fn len(&self) -> usize { self.heap.len() }
      pub fn contains(&self, panel_sha: &[u8; 32]) -> bool { self.entries_by_panel_sha.contains_key(panel_sha) }
  }

crates/context-graph-mejepa/src/eval/error.rs:
  impl EvalError {
      pub fn code(&self) -> &'static str {
          match self {
              EvalError::EmptyHoldout => "MEJEPA_EVAL_EMPTY_HOLDOUT",
              EvalError::HoldoutLeakage{..} => "MEJEPA_EVAL_HOLDOUT_LEAKAGE",
              EvalError::InsufficientSamples{..} => "MEJEPA_EVAL_INSUFFICIENT_SAMPLES",
              EvalError::ShipGateRegression{..} => "MEJEPA_EVAL_SHIP_GATE_REGRESSION",
              EvalError::LanguageOodDrift{..} => "MEJEPA_EVAL_LANGUAGE_OOD_DRIFT",
              EvalError::LanguageConformalDrift{..} => "MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT",
              EvalError::ReportPersistFail{..} => "MEJEPA_EVAL_REPORT_PERSIST_FAIL",
              EvalError::NoReportAvailable => "MEJEPA_EVAL_NO_REPORT_AVAILABLE",
              EvalError::TaskGraphRebuildFail{..} => "MEJEPA_EVAL_TASK_GRAPH_REBUILD_FAIL",
              EvalError::ActiveLearningBelowTarget{..} => "MEJEPA_EVAL_ACTIVE_LEARNING_BELOW_TARGET",
              EvalError::NanDetected{..} => "MEJEPA_EVAL_NAN_DETECTED",
              EvalError::EmptyHoldoutLanguage{..} => "MEJEPA_EVAL_EMPTY_HOLDOUT_LANGUAGE",
              EvalError::InsufficientLiveData{..} => "MEJEPA_EVAL_INSUFFICIENT_LIVE_DATA",
              EvalError::TaskGraphSkip{..} => "MEJEPA_EVAL_TASK_GRAPH_SKIP",
              EvalError::MissingProvenance{..} => "MEJEPA_EVAL_MISSING_PROVENANCE",
              EvalError::ReportTooLarge{..} => "MEJEPA_EVAL_REPORT_TOO_LARGE",
              EvalError::PanelNotInQueue{..} => "MEJEPA_EVAL_PANEL_NOT_IN_QUEUE",
          }
      }
      pub fn log_context(&self, log_target: &str) {
          tracing::error!(target: log_target, code = self.code(), payload = ?self, "eval error");
      }
  }

crates/context-graph-mejepa/src/eval/config.rs:
  impl Default for EvalConfig { /* constitution-pinned values */ }
  impl EvalConfig {
      pub fn validate(&self) -> Result<(), EvalError> {
          if !self.alpha.is_finite() || !(0.01..=0.50).contains(&self.alpha) {
              return Err(EvalError::NanDetected { field: "alpha".into(), cell: None });
          }
          if !self.state_transfer_lambda.is_finite() || self.state_transfer_lambda <= 0.0 {
              return Err(EvalError::NanDetected { field: "state_transfer_lambda".into(), cell: None });
          }
          if self.active_learning_queue_capacity == 0 {
              return Err(EvalError::NanDetected { field: "active_learning_queue_capacity".into(), cell: None });
          }
          if self.task_graph_top_k == 0 {
              return Err(EvalError::NanDetected { field: "task_graph_top_k".into(), cell: None });
          }
          if !self.task_graph_cosine_threshold.is_finite() || !(-1.0..=1.0).contains(&self.task_graph_cosine_threshold) {
              return Err(EvalError::NanDetected { field: "task_graph_cosine_threshold".into(), cell: None });
          }
          Ok(())
      }
  }

crates/context-graph-mejepa/src/eval/report.rs (UPGRADE):
  impl EvalReport {
      pub fn try_new(b: EvalReportBuilder) -> Result<Self, EvalError> {
          if !b.overall_correlation.is_finite() {
              return Err(EvalError::NanDetected { field: "overall_correlation".into(), cell: None });
          }
          // ... per validations from TASK-EVAL-001 pseudo_code, replacing `&'static str` with EvalError variants
          if b.calibration_version.is_empty() {
              return Err(EvalError::MissingProvenance { field: "calibration_version".into() });
          }
          Ok(Self { /* fields */ })
      }
      pub fn serialize_canonical(&self) -> Result<Vec<u8>, EvalError> {
          let bytes = serde_json::to_vec(self).map_err(|e| EvalError::ReportPersistFail {
              cf: "canonical_serialize".into(),
              backend_error: e.to_string(),
          })?;
          if bytes.len() > 1024 * 1024 {
              return Err(EvalError::ReportTooLarge { size_bytes: bytes.len(), limit_bytes: 1024 * 1024 });
          }
          Ok(bytes)
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/queue.rs">ActiveLearningQueueEntry + ActiveLearningQueueState + ActiveLearningLabel + LabelProvenance + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/task_graph.rs">PatchGraph + PatchGraphEdge + similar_tasks read-side query + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/error.rs">EvalError (17 variants) + QueueError (4 variants) + GraphError (5 variants) + .code() + .log_context() + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/config.rs">EvalConfig + Default + validate + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">add `pub mod queue; pub mod task_graph; pub mod error; pub mod config;` and re-export public types</file>
  <file path="crates/context-graph-mejepa/src/eval/report.rs">UPGRADE `EvalReport::try_new` / `serialize_canonical` signatures from `Result<_, &'static str>` to `Result<_, EvalError>`; replace placeholder errors with structured EvalError variants</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add deps `thiserror = { workspace = true }`, `tracing = { workspace = true }`, `parking_lot = { workspace = true }` if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa eval shows ≥ 25 passing tests across all 4 new files (cumulative with TASK-EVAL-001 tests)</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>EvalError has 17 variants and every .code() returns a unique SCREAMING_SNAKE tag</criterion>
  <criterion>QueueError has 4 variants; GraphError has 5 variants; both have .code() coverage tests</criterion>
  <criterion>ActiveLearningQueueEntry::cmp orders by signal_value descending (BinaryHeap pops max first)</criterion>
  <criterion>ActiveLearningQueueEntry::cmp handles NaN signal_value without panicking</criterion>
  <criterion>EvalConfig::default returns the 18 constitution-pinned values</criterion>
  <criterion>EvalConfig::validate rejects alpha=0.0, lambda=0.0, capacity=0, top_k=0, cosine_threshold=2.0</criterion>
  <criterion>EvalReport::try_new now returns Result<Self, EvalError> (not &'static str); compile-time verified via _assert_signatures fn</criterion>
  <criterion>.log_context() emits a tracing event without panicking on every variant</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
