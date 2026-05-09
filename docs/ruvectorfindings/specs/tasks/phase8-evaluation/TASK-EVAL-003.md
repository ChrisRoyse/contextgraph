# TASK-EVAL-003 — RocksDB column-family descriptors for evaluation (CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_TASK_GRAPH(+_HISTORY), CF_MEJEPA_ACTIVE_LEARNING_QUEUE/LABELS/EVICTIONS, CF_MEJEPA_OOD_ESCALATIONS, CF_MEJEPA_RESEARCH_QUESTIONS, CF_MEJEPA_MODEL_PROMOTIONS)

```xml
<task_spec id="TASK-EVAL-003" version="1.0">

<metadata>
  <title>Define and register RocksDB column families for the evaluation domain in `crates/context-graph-mejepa/src/eval/storage.rs` (and update workspace-wide CF registry)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-EVAL-18">Provenance binding — CF_MEJEPA_EVAL_REPORTS keyed by (report_date_iso8601 | report_ulid)</item>
    <item ref="REQ-EVAL-07">CF_MEJEPA_TASK_GRAPH + CF_MEJEPA_TASK_GRAPH_HISTORY for cross-task transfer graph</item>
    <item ref="REQ-EVAL-05">CF_MEJEPA_ACTIVE_LEARNING_QUEUE + CF_MEJEPA_ACTIVE_LEARNING_LABELS</item>
    <item ref="EC-EVAL-04">CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS for capacity overflow audit</item>
    <item ref="BR-EVAL-03">CF_MEJEPA_OOD_ESCALATIONS for OOD-uncertain panels</item>
    <item ref="REQ-EVAL-19">CF_MEJEPA_RESEARCH_QUESTIONS (8-entry persistent state)</item>
    <item ref="REQ-EVAL-02">CF_MEJEPA_MODEL_PROMOTIONS for model_promotion_history rolling 12-month audit</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-002</task_ref><!-- EvalError, ActiveLearningQueueEntry, ActiveLearningLabel, LabelProvenance, PatchGraphEdge -->
    <task_ref>TASK-EVAL-001</task_ref><!-- EvalReport, ModelPromotionEntry, OpenResearchQuestionStatus -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 created (eval/storage.rs, eval/storage_keys.rs); 1 modified (cgreality CF registry / mejepa lib)</estimated_files>
</metadata>

<context>
This task lands the persistent surface for the eval domain. Eight new RocksDB column families are
declared with their key formats, value formats (bincode for binary efficiency per NFR-EVAL-09),
retention policies (Phase 6 hygiene-driven), and SCREAMING_SNAKE-tagged `cf_name()` constants.
After this task, TASK-EVAL-004 (run_holdout_eval), TASK-EVAL-009 (active learning enqueue), and
TASK-EVAL-010 (graph build) can persist their outputs without inventing CF naming or key layout
ad hoc. CF lifecycle (open/create-if-missing) integrates with the existing
`crates/context-graph-storage/` CF registry pattern.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (rocksdb_cfs naming convention "SCREAMING_SNAKE column-family names with CF_ prefix"; storage budget 115 GB)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-05, REQ-EVAL-07, REQ-EVAL-18, REQ-EVAL-19; EC-EVAL-04 eviction audit)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (rocksdb_column_families block: 11 CFs total — 8 new owned by EVALUATION + 3 read-only borrowed from other domains)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase8-evaluation/TASK-EVAL-002.md (EvalError variants, queue/graph types)</file>
  <file purpose="cross_domain_pattern">crates/context-graph-mejepa-corpus/src/oracle_store.rs (CF descriptor pattern from Phase 0)</file>
  <file purpose="cross_domain_pattern">crates/context-graph-storage/src/lib.rs (workspace-wide RocksDB CF registry conventions)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-001 + TASK-EVAL-002 shipped: eval types + error + config compile</check>
  <check>`rocksdb`, `bincode`, `chrono`, `ulid` available as workspace deps</check>
  <check>`crates/context-graph-storage` exposes CF descriptor / open helpers (existing pattern)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/storage_keys.rs` with the canonical key builders (zero-copy, deterministic, big-endian where applicable):
      - `eval_report_key(report_date: NaiveDate, report_ulid: &Ulid) -> [u8; 24]` (8-byte i64 ISO-8601-day-since-epoch big-endian + 16-byte ULID)
      - `task_graph_edge_key(source_task_sha: &[u8; 32], target_task_sha: &[u8; 32]) -> [u8; 64]`
      - `task_graph_history_key(corpus_sha: &[u8; 32], source_task_sha: &[u8; 32], target_task_sha: &[u8; 32]) -> [u8; 96]`
      - `active_learning_queue_key(panel_sha: &[u8; 32]) -> [u8; 32]` (transparent passthrough)
      - `active_learning_label_key(label_ulid: &Ulid) -> [u8; 16]` (transparent ULID bytes)
      - `active_learning_eviction_key(evicted_at_unix: i64, panel_sha: &[u8; 32]) -> [u8; 40]`
      - `ood_escalation_key(panel_sha: &[u8; 32]) -> [u8; 32]`
      - `research_question_key(qid: ResearchQuestionId) -> [u8; 1]` (single byte: Q1=1, Q2=2, ..., Q8=8)
      - `model_promotion_key(promoted_at_unix: i64, weights_sha: &[u8; 32]) -> [u8; 40]`
    - Create `crates/context-graph-mejepa/src/eval/storage.rs` with:
      - `pub const CF_MEJEPA_EVAL_REPORTS: &str = "CF_MEJEPA_EVAL_REPORTS";` (and 7 more)
      - `pub fn cf_descriptors() -> Vec<rocksdb::ColumnFamilyDescriptor>` returning all 8 new CF descriptors with default options (no custom compression yet — Phase 6 HYGIENE owns tier-compression integration)
      - `pub const ALL_EVAL_CFS: [&str; 8]` enumerating the 8 new CF names for batch open
      - Per-CF value-bytes `serde::{Serialize, Deserialize}` round-trip helpers via `bincode`:
        - `serialize_eval_report(&EvalReport) -> Result<Vec<u8>, EvalError>`
        - `deserialize_eval_report(&[u8]) -> Result<EvalReport, EvalError>`
        - `serialize_queue_entry(&ActiveLearningQueueEntry) -> Result<Vec<u8>, EvalError>`
        - `deserialize_queue_entry(&[u8]) -> Result<ActiveLearningQueueEntry, EvalError>`
        - same pattern for `ActiveLearningLabel`, `OodEscalation` (new struct), `OpenResearchQuestionStatus`, `ModelPromotionEntry`
      - `OodEscalation` struct (panel_sha, ood_score, language, reason: String, escalated_at: i64) — per TECH-EVALUATION rocksdb_column_families CF_MEJEPA_OOD_ESCALATIONS value_format
      - `EvalReportPersist` wrapper struct: `{ report: EvalReport, ulid: Ulid }` for the bincode value (since the key already includes ulid, we redundantly include for cross-validation)
      - Read-only consts referencing borrowed CFs (NOT created here — owned by other domains; declared as constants for type-safe reads):
        - `pub const CF_MEJEPA_LIVE_PREDICTIONS: &str = "CF_MEJEPA_LIVE_PREDICTIONS";` (owned by SPEC-INFERENCE)
        - `pub const CF_MEJEPA_ORACLE_VERDICTS: &str = "CF_MEJEPA_ORACLE_VERDICTS";` (owned by SPEC-CORPUS)
        - `pub const CF_MEJEPA_TRAIN_CERTS: &str = "CF_MEJEPA_TRAIN_CERTS";` (owned by SPEC-TRAINING)
        - `pub const CF_MEJEPA_CONSTELLATION: &str = "CF_MEJEPA_CONSTELLATION";` (owned by SPEC-TCT)
        - `pub const CF_MEJEPA_CALIBRATION_HISTORY: &str = "CF_MEJEPA_CALIBRATION_HISTORY";` (owned by SPEC-INFERENCE)
    - Update `crates/context-graph-mejepa/src/eval/mod.rs` to declare `pub mod storage; pub mod storage_keys;` and re-export the public CF constants + serializers
    - Add unit tests in `eval/storage.rs` and `eval/storage_keys.rs` `#[cfg(test)] mod tests` for:
      - Round-trip a synthetic `EvalReport` through `serialize_eval_report` + `deserialize_eval_report` and assert byte-equality on second serialize
      - Round-trip an `ActiveLearningQueueEntry` through serialize + deserialize
      - Round-trip an `ActiveLearningLabel` (with `LabelProvenance`)
      - Round-trip an `OodEscalation`
      - Round-trip `OpenResearchQuestionStatus` (8 entries)
      - Round-trip `ModelPromotionEntry`
      - `eval_report_key` produces deterministic 24-byte output
      - `task_graph_edge_key` produces 64 bytes (32 source + 32 target)
      - `research_question_key(Q1)` == [0x01]; `research_question_key(Q8)` == [0x08]
      - `cf_descriptors().len() == 8`; all names match `ALL_EVAL_CFS`
      - Open a temporary RocksDB instance with `cf_descriptors()`, write + read one row per CF, drop, reopen, verify durable
  </in_scope>
  <out_of_scope>
    - `EvalRunner` reads from these CFs (TASK-EVAL-004 onward)
    - Writes during `enqueue_uncertain` (TASK-EVAL-009)
    - Writes during `build_patch_similarity_graph` (TASK-EVAL-010)
    - Tier-compression policy (Phase 6 HYGIENE owns)
    - MCP handler reads (TASK-EVAL-013, TASK-EVAL-014)
    - Migration tooling (no backwards compat per `memory/feedback_no_backwards_compat.md`)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/storage.rs">
use std::path::Path;
use bincode;
use rocksdb::{ColumnFamilyDescriptor, Options};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use crate::types::Language;
use super::error::EvalError;
use super::queue::{ActiveLearningQueueEntry, ActiveLearningLabel};
use super::report::EvalReport;
use super::types::{ModelPromotionEntry, OpenResearchQuestionStatus};

// ===== EVALUATION-OWNED CFs (8) =====
pub const CF_MEJEPA_EVAL_REPORTS: &amp;str               = "CF_MEJEPA_EVAL_REPORTS";
pub const CF_MEJEPA_TASK_GRAPH: &amp;str                  = "CF_MEJEPA_TASK_GRAPH";
pub const CF_MEJEPA_TASK_GRAPH_HISTORY: &amp;str          = "CF_MEJEPA_TASK_GRAPH_HISTORY";
pub const CF_MEJEPA_ACTIVE_LEARNING_QUEUE: &amp;str       = "CF_MEJEPA_ACTIVE_LEARNING_QUEUE";
pub const CF_MEJEPA_ACTIVE_LEARNING_LABELS: &amp;str      = "CF_MEJEPA_ACTIVE_LEARNING_LABELS";
pub const CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS: &amp;str   = "CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS";
pub const CF_MEJEPA_OOD_ESCALATIONS: &amp;str             = "CF_MEJEPA_OOD_ESCALATIONS";
pub const CF_MEJEPA_RESEARCH_QUESTIONS: &amp;str          = "CF_MEJEPA_RESEARCH_QUESTIONS";
pub const CF_MEJEPA_MODEL_PROMOTIONS: &amp;str            = "CF_MEJEPA_MODEL_PROMOTIONS";

pub const ALL_EVAL_CFS: [&amp;str; 9] = [
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
];

// ===== READ-ONLY borrowed CFs (declared for type-safe reads only; created by other domains) =====
pub const CF_MEJEPA_LIVE_PREDICTIONS: &amp;str           = "CF_MEJEPA_LIVE_PREDICTIONS";
pub const CF_MEJEPA_ORACLE_VERDICTS: &amp;str            = "CF_MEJEPA_ORACLE_VERDICTS";
pub const CF_MEJEPA_TRAIN_CERTS: &amp;str                = "CF_MEJEPA_TRAIN_CERTS";
pub const CF_MEJEPA_CONSTELLATION: &amp;str              = "CF_MEJEPA_CONSTELLATION";
pub const CF_MEJEPA_CALIBRATION_HISTORY: &amp;str        = "CF_MEJEPA_CALIBRATION_HISTORY";

pub fn cf_descriptors() -&gt; Vec&lt;ColumnFamilyDescriptor&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OodEscalation {
    pub panel_sha: [u8; 32],
    pub ood_score: f32,
    pub language: Language,
    pub reason: String,
    pub escalated_at: i64,
}

pub fn serialize_eval_report(report: &amp;EvalReport) -&gt; Result&lt;Vec&lt;u8&gt;, EvalError&gt;;
pub fn deserialize_eval_report(bytes: &amp;[u8]) -&gt; Result&lt;EvalReport, EvalError&gt;;

pub fn serialize_queue_entry(e: &amp;ActiveLearningQueueEntry) -&gt; Result&lt;Vec&lt;u8&gt;, EvalError&gt;;
pub fn deserialize_queue_entry(bytes: &amp;[u8]) -&gt; Result&lt;ActiveLearningQueueEntry, EvalError&gt;;

pub fn serialize_label(l: &amp;ActiveLearningLabel) -&gt; Result&lt;Vec&lt;u8&gt;, EvalError&gt;;
pub fn deserialize_label(bytes: &amp;[u8]) -&gt; Result&lt;ActiveLearningLabel, EvalError&gt;;

pub fn serialize_ood_escalation(e: &amp;OodEscalation) -&gt; Result&lt;Vec&lt;u8&gt;, EvalError&gt;;
pub fn deserialize_ood_escalation(bytes: &amp;[u8]) -&gt; Result&lt;OodEscalation, EvalError&gt;;

pub fn serialize_research_question(q: &amp;OpenResearchQuestionStatus) -&gt; Result&lt;Vec&lt;u8&gt;, EvalError&gt;;
pub fn deserialize_research_question(bytes: &amp;[u8]) -&gt; Result&lt;OpenResearchQuestionStatus, EvalError&gt;;

pub fn serialize_model_promotion(p: &amp;ModelPromotionEntry) -&gt; Result&lt;Vec&lt;u8&gt;, EvalError&gt;;
pub fn deserialize_model_promotion(bytes: &amp;[u8]) -&gt; Result&lt;ModelPromotionEntry, EvalError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/storage_keys.rs">
use chrono::NaiveDate;
use ulid::Ulid;
use super::types::ResearchQuestionId;

pub fn eval_report_key(report_date: NaiveDate, report_ulid: &amp;Ulid) -&gt; [u8; 24];
pub fn task_graph_edge_key(source: &amp;[u8; 32], target: &amp;[u8; 32]) -&gt; [u8; 64];
pub fn task_graph_history_key(corpus_sha: &amp;[u8; 32], source: &amp;[u8; 32], target: &amp;[u8; 32]) -&gt; [u8; 96];
pub fn active_learning_queue_key(panel_sha: &amp;[u8; 32]) -&gt; [u8; 32];
pub fn active_learning_label_key(label_ulid: &amp;Ulid) -&gt; [u8; 16];
pub fn active_learning_eviction_key(evicted_at_unix: i64, panel_sha: &amp;[u8; 32]) -&gt; [u8; 40];
pub fn ood_escalation_key(panel_sha: &amp;[u8; 32]) -&gt; [u8; 32];
pub fn research_question_key(qid: ResearchQuestionId) -&gt; [u8; 1];
pub fn model_promotion_key(promoted_at_unix: i64, weights_sha: &amp;[u8; 32]) -&gt; [u8; 40];
    </signature>
  </signatures>
  <constraints>
    - Every CF name has the `CF_MEJEPA_` prefix per `<rocksdb_cfs>` naming rule
    - All multi-byte integer fields in keys are big-endian (preserves lexicographic == numeric ordering for RocksDB iteration)
    - `eval_report_key` packs ISO-8601-day-since-epoch (i64 BE) followed by ULID bytes — enables ordered prefix scan by date for "latest within window" queries
    - `research_question_key` is exactly 1 byte; ResearchQuestionId Q1..Q8 maps to 0x01..0x08; reserve 0x00 for future "uncategorized"
    - All serializers use `bincode::serialize` / `bincode::deserialize` (NOT serde_json) for binary efficiency per NFR-EVAL-09 storage budget
    - `serialize_eval_report` rejects when bincode output > 1 MiB (per NFR-EVAL-09 + REQ-EVAL-16) → return `EvalError::ReportTooLarge`
    - `deserialize_*` errors map bincode errors to `EvalError::ReportPersistFail{cf, backend_error: e.to_string()}` with appropriate `cf` tag
    - `cf_descriptors()` uses `Options::default()` for now; column-family-specific tuning (compression, block size) is out of scope — Phase 6 HYGIENE owns
    - The 5 borrowed CFs (CF_MEJEPA_LIVE_PREDICTIONS, _ORACLE_VERDICTS, _TRAIN_CERTS, _CONSTELLATION, _CALIBRATION_HISTORY) are declared as `pub const &str` ONLY — they are NOT included in `cf_descriptors()` and are NOT created by this task; SPEC-INFERENCE / SPEC-CORPUS / SPEC-TRAINING / SPEC-TCT own creation
    - All bincode-serialized values are versioned via the surrounding struct's serde derive (no manual version field per `memory/feedback_no_backwards_compat.md` "wipe DB on schema change")
    - All keys are deterministic given identical inputs (no clock reads, no random bytes)
    - Rust 2021 edition; Rust 1.84+ per constitution
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa eval::storage` shows ≥ 8 passing tests
    - `cargo test -p context-graph-mejepa eval::storage_keys` shows ≥ 5 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `eval_report_round_trip_byte_equal` passes (serialize → deserialize → re-serialize byte-equal)
    - Test `queue_entry_round_trip` passes
    - Test `label_round_trip_with_provenance` passes
    - Test `ood_escalation_round_trip` passes
    - Test `research_question_round_trip_eight_entries` passes
    - Test `model_promotion_round_trip` passes
    - Test `cf_descriptors_returns_eight` passes; verifies `ALL_EVAL_CFS.len() == 9` (oops should be 8 — spec sanity check happens here; the 9th const above is wrong; fix on actual implementation: ALL_EVAL_CFS has exactly 8 entries; CF_MEJEPA_TASK_GRAPH and CF_MEJEPA_TASK_GRAPH_HISTORY are TWO separate CFs counted as 2 of 8; 8 = REPORTS, GRAPH, GRAPH_HISTORY, AL_QUEUE, AL_LABELS, AL_EVICTIONS, OOD, RESEARCH, PROMOTIONS = 9. Constitution-pinned CF count is 9 owned + 5 borrowed = 14 declared in this file)
    - Test `rocksdb_open_close_reopen_durable` passes (write 1 row per CF, close, reopen with same descriptors, read back)
    - Test `research_question_key_q1_eq_0x01` passes
    - Test `eval_report_key_24_bytes` passes
    - Test `task_graph_edge_key_64_bytes` passes
    - Test `serialize_eval_report_rejects_oversize` passes (synthesize 2 MiB-bound report, expect `ReportTooLarge`)
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/storage.rs:
  // ALL_EVAL_CFS counted: REPORTS(1) + TASK_GRAPH(2) + TASK_GRAPH_HISTORY(3) +
  // AL_QUEUE(4) + AL_LABELS(5) + AL_EVICTIONS(6) + OOD(7) + RESEARCH(8) + MODEL_PROMOTIONS(9) = 9 owned CFs
  // (the spec text "8 new CFs" includes the 8 from TECH-EVALUATION rocksdb_column_families
  // PLUS CF_MEJEPA_MODEL_PROMOTIONS — which IS the 9th evaluator-owned CF per data_models)
  pub const ALL_EVAL_CFS: [&str; 9] = [/* 9 entries */];

  pub fn cf_descriptors() -> Vec<ColumnFamilyDescriptor> {
      ALL_EVAL_CFS.iter()
          .map(|&name| ColumnFamilyDescriptor::new(name, Options::default()))
          .collect()
  }

  pub fn serialize_eval_report(report: &EvalReport) -> Result<Vec<u8>, EvalError> {
      let bytes = bincode::serialize(report).map_err(|e| EvalError::ReportPersistFail {
          cf: CF_MEJEPA_EVAL_REPORTS.into(),
          backend_error: e.to_string(),
      })?;
      if bytes.len() > 1024 * 1024 {
          return Err(EvalError::ReportTooLarge { size_bytes: bytes.len(), limit_bytes: 1024 * 1024 });
      }
      Ok(bytes)
  }

  pub fn deserialize_eval_report(bytes: &[u8]) -> Result<EvalReport, EvalError> {
      bincode::deserialize(bytes).map_err(|e| EvalError::ReportPersistFail {
          cf: CF_MEJEPA_EVAL_REPORTS.into(),
          backend_error: e.to_string(),
      })
  }
  // pattern repeats for queue_entry, label, ood_escalation, research_question, model_promotion

crates/context-graph-mejepa/src/eval/storage_keys.rs:
  pub fn eval_report_key(report_date: NaiveDate, report_ulid: &Ulid) -> [u8; 24] {
      let day_since_epoch = report_date.and_hms_opt(0, 0, 0)
          .unwrap()
          .and_utc()
          .timestamp() / 86400;
      let mut out = [0u8; 24];
      out[..8].copy_from_slice(&day_since_epoch.to_be_bytes());
      out[8..].copy_from_slice(&report_ulid.0.to_be_bytes()); // ULID is 128-bit; 16 bytes BE
      out
  }

  pub fn task_graph_edge_key(source: &[u8; 32], target: &[u8; 32]) -> [u8; 64] {
      let mut out = [0u8; 64];
      out[..32].copy_from_slice(source);
      out[32..].copy_from_slice(target);
      out
  }

  pub fn research_question_key(qid: ResearchQuestionId) -> [u8; 1] {
      use ResearchQuestionId::*;
      match qid {
          Q1 => [1], Q2 => [2], Q3 => [3], Q4 => [4],
          Q5 => [5], Q6 => [6], Q7 => [7], Q8 => [8],
      }
  }
  // pattern repeats for the rest
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/storage_keys.rs">9 deterministic key builders + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/storage.rs">9 owned CF constants + 5 borrowed CF constants + cf_descriptors() + 6 serialize/deserialize pairs + OodEscalation struct + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">add `pub mod storage; pub mod storage_keys;` and re-export public CF constants + serializers</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add deps `rocksdb = { workspace = true }`, `bincode = { workspace = true }`, `ulid = { workspace = true }` if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa eval::storage shows ≥ 8 passing tests</criterion>
  <criterion>cargo test -p context-graph-mejepa eval::storage_keys shows ≥ 5 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>ALL_EVAL_CFS contains exactly 9 owned CF constants, all matching `CF_MEJEPA_*`</criterion>
  <criterion>cf_descriptors() returns exactly 9 ColumnFamilyDescriptor entries</criterion>
  <criterion>EvalReport bincode round-trip is byte-equal across two serialize calls</criterion>
  <criterion>serialize_eval_report rejects when bincode output exceeds 1 MiB</criterion>
  <criterion>research_question_key(Q1) == [0x01]; research_question_key(Q8) == [0x08]</criterion>
  <criterion>eval_report_key returns exactly 24 bytes; first 8 bytes are big-endian i64 day-since-epoch</criterion>
  <criterion>task_graph_edge_key returns exactly 64 bytes (source[..32] | target[32..])</criterion>
  <criterion>RocksDB open + write 1 row per CF + close + reopen + read durably (integration test using TempDir)</criterion>
  <criterion>The 5 borrowed CF consts (LIVE_PREDICTIONS, ORACLE_VERDICTS, TRAIN_CERTS, CONSTELLATION, CALIBRATION_HISTORY) are NOT included in ALL_EVAL_CFS</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::storage</command>
  <command>cargo test -p context-graph-mejepa eval::storage_keys</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
