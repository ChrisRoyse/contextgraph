<task_spec id="TASK-SHIFT-002" version="1.0">
<metadata>
  <title>Foundation — WatermarkRecord + ShiftLogTail/InstrumentCache/SlotKey + SubscriberMetrics + UtmlFactorBundle + CaptureAuditBundle + LiveSubscriberStatus + PanicInfo data models</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-SHIFT-02">WatermarkRecord schema</item>
    <item ref="REQ-SHIFT-03">SubscriberMetrics + lag_alert_state</item>
    <item ref="REQ-SHIFT-08">InstrumentCache + SlotKey</item>
    <item ref="REQ-SHIFT-10">CaptureAuditBundle</item>
    <item ref="REQ-SHIFT-12">LiveSubscriberStatus + PanicInfo</item>
    <item ref="REQ-SHIFT-18">UtmlFactorBundle (provenance carrier)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[WatermarkWriter]">struct shape (constructor + write/read methods land in TASK-SHIFT-008)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[WatermarkRecord]">on-disk JSON schema</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[ShiftLogTail]">struct shape (behaviour lands in TASK-SHIFT-005)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[InstrumentCache]">struct shape (behaviour lands in TASK-SHIFT-006)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[SlotKey]">cache key struct</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[SubscriberMetrics]">atomic counters + lag-alert state machine</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[UtmlFactorBundle]">cross-cutting carrier from SPEC-UTML</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[CaptureAuditBundle]">audit response shape</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[LiveSubscriberStatus]">status response shape</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[PanicInfo]">supervisor panic capture</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>6 files</estimated_files>
</metadata>

<context>
Second foundation task. Adds the remaining typed data shapes that downstream logic (TASK-SHIFT-005..012) and the sister phase (phase7-mcp-tools/) consume. Still no behaviour — only data declarations + constructors that fail-closed on invariant violations. The `LagAlertState` enum is declared here and consumed by TASK-SHIFT-009's state machine. `InstrumentSlot`, `AuxHeadId`, `EmbedderId`, `RealityPrediction`, `VerifyVerdict`, `OracleOutcome`, `CounterfactualResult`, `AuxHeadPrediction`, `PanelSnapshot`, `TimeStep` are imported from the existing `context-graph-mejepa-instruments` crate or stubbed via `serde_json::Value` placeholder when those crates haven't shipped yet — see `<constraints>` for the exact stub policy.
</context>

<input_context_files>
  <file purpose="data_model_field_specs">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#data_models</file>
  <file purpose="instrument_slot_definition">crates/context-graph-mejepa-instruments/src/lib.rs (InstrumentSlot enum import)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-001.md</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards</file>
  <file purpose="provenance_rule">docs/ruvectorfindings/specs/constitution.md#coding_standards#provenance</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001 complete and crate compiles</check>
  <check>`context-graph-mejepa-instruments` crate exists (InstrumentSlot enum is importable; declared a workspace dep)</check>
  <check>`lru` crate (≥ 0.12) listed as a workspace dependency (or added in this task)</check>
  <check>`parking_lot` (≥ 0.12) listed as a workspace dependency</check>
  <check>`dashmap` (≥ 5.5) listed as a workspace dependency</check>
</prerequisites>

<scope>
  <in_scope>
    - Add data model files:
      - `watermark.rs` — `WatermarkRecord` (JSON-serializable per-session row); `WatermarkWriter` struct skeleton (no methods yet)
      - `tail.rs` — `ShiftLogTail` struct skeleton; `RotationDetector` struct
      - `cache.rs` — `InstrumentCache` struct skeleton; `SlotKey` struct
      - `metrics.rs` — `SubscriberMetrics` (atomic counters + `LagAlertState` enum) + `ErrorEvent` + `CircularBuffer<T, const N: usize>`
      - `audit.rs` — `UtmlFactorBundle`, `CaptureAuditBundle`, `LiveSubscriberStatus`, `PanicInfo`
    - Wire `pub mod` + `pub use` re-exports in `lib.rs`
    - Add `lru`, `parking_lot`, `dashmap`, `procfs` to crate Cargo.toml
    - Add `context-graph-mejepa-instruments = { path = "../context-graph-mejepa-instruments" }` so `InstrumentSlot` is importable
    - Unit tests: `LagAlertState` transitions; `WatermarkRecord` JSON round-trip; `SlotKey` Hash + Eq invariants; `CaptureAuditBundle` JSON serializes deterministically
  </in_scope>
  <out_of_scope>
    - `ShiftLogTail::poll_next_line` (TASK-SHIFT-005)
    - `InstrumentCache::get_or_insert_with` (TASK-SHIFT-006)
    - `WatermarkWriter::write_watermark` / `read_watermark` (TASK-SHIFT-008)
    - `SubscriberError` taxonomy (TASK-SHIFT-003)
    - RocksDB CF descriptors (TASK-SHIFT-004)
    - Cross-cutting types from SPEC-INFERENCE/SPEC-PREDICTOR — stubbed via `serde_json::Value` here, replaced with concrete types when those crates ship
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/watermark.rs">
      use serde::{Serialize, Deserialize};
      use crate::entry::{SessionId, ShiftId};

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct WatermarkRecord {
          pub session_id: SessionId,
          pub last_consumed_shift_id: ShiftId,
          pub last_consumed_byte_offset: u64,
          pub last_advanced_at_unix_seconds: i64,
          pub provenance_corpus_sha: [u8; 32],
          pub provenance_code_version: String,
      }
      impl WatermarkRecord {
          pub fn rocksdb_key(session_id: &amp;SessionId) -&gt; Vec&lt;u8&gt;; // `wm:{session_id}` UTF-8
      }

      pub struct WatermarkWriter {
          // RocksDB Arc&lt;DB&gt; + ColumnFamilyHandle live here; methods land in TASK-SHIFT-008.
          // Skeleton struct only; constructor stub returns a placeholder error.
      }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/tail.rs">
      use std::collections::HashMap;
      use std::path::PathBuf;
      use std::time::Duration;
      use crate::entry::SessionId;

      pub struct ShiftLogTail {
          pub(crate) root_dir: PathBuf,
          pub(crate) readers: HashMap&lt;SessionId, ReaderHandle&gt;,
          pub(crate) poll_interval: Duration,
          pub(crate) rotation_detector: RotationDetector,
      }

      pub struct RotationDetector {
          pub(crate) inode_by_session: HashMap&lt;SessionId, u64&gt;,
          pub(crate) size_by_session: HashMap&lt;SessionId, u64&gt;,
      }

      // ReaderHandle is opaque here; concrete tokio::fs::File wrapping happens in TASK-SHIFT-005
      pub struct ReaderHandle { pub(crate) _opaque: () }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/cache.rs">
      use std::sync::atomic::{AtomicU64, AtomicUsize};
      use parking_lot::RwLock;
      use lru::LruCache;
      use std::sync::Arc;
      use serde::{Serialize, Deserialize};
      use context_graph_mejepa_instruments::InstrumentSlot;

      #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
      pub struct SlotKey {
          pub file_sha: [u8; 32],
          pub slot_id: InstrumentSlot,
          pub code_version: String,
      }

      pub struct InstrumentCache {
          pub(crate) lru: RwLock&lt;LruCache&lt;SlotKey, Arc&lt;Vec&lt;f32&gt;&gt;&gt;&gt;,
          pub(crate) capacity: AtomicUsize,
          pub(crate) ram_budget_bytes: u64,
          pub(crate) ram_used_bytes: AtomicU64,
          pub(crate) hits: AtomicU64,
          pub(crate) misses: AtomicU64,
      }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/metrics.rs">
      use std::sync::atomic::{AtomicU64, AtomicU8};
      use std::time::Instant;
      use dashmap::DashMap;
      use parking_lot::RwLock;
      use crate::entry::{SessionId, ShiftId};

      #[repr(u8)]
      #[derive(Debug, Clone, Copy, PartialEq, Eq)]
      pub enum LagAlertStateTag {
          Idle = 0,
          Watching = 1,
          Trigger = 2,
          AlertActive = 3,
          AlertClearing = 4,
      }

      #[derive(Debug, Clone)]
      pub enum LagAlertState {
          Idle,
          Watching,
          Trigger { since: Instant },
          AlertActive,
          AlertClearing { since: Instant },
      }

      #[derive(Debug, Clone)]
      pub struct ErrorEvent {
          pub timestamp_unix_seconds: i64,
          pub error_code: String,
      }

      pub struct CircularBuffer&lt;T, const N: usize&gt; {
          pub(crate) buf: [Option&lt;T&gt;; N],
          pub(crate) write_head: usize,
      }

      pub struct SubscriberMetrics {
          pub lag_total: AtomicU64,
          pub lag_per_session: DashMap&lt;SessionId, AtomicU64&gt;,
          pub lag_alert_state_tag: AtomicU8, // matches LagAlertStateTag
          pub lag_alert_state_full: RwLock&lt;LagAlertState&gt;, // for since: Instant fields
          pub error_rate_window: RwLock&lt;CircularBuffer&lt;ErrorEvent, 1000&gt;&gt;,
          pub dropped_l_step_count: AtomicU64,
          pub rss_high_water: AtomicU64,
          pub task_alive_since: RwLock&lt;Option&lt;i64&gt;&gt;,
          pub last_panic: RwLock&lt;Option&lt;crate::audit::PanicInfo&gt;&gt;,
      }
      impl SubscriberMetrics { pub fn new() -&gt; Self }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/audit.rs">
      use std::collections::HashMap;
      use serde::{Serialize, Deserialize};
      use context_graph_mejepa_instruments::InstrumentSlot;
      use crate::entry::ShiftId;

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct UtmlFactorBundle {
          pub delta_p: f32,
          pub delta_k: f32,
          pub delta_omega: f32,
          pub delta_xi: f32,
          pub l_step: f32,
      }
      impl UtmlFactorBundle {
          pub fn assert_finite_and_clamped(&amp;self) -&gt; Result&lt;(), UtmlFactorError&gt;;
      }
      #[derive(Debug, thiserror::Error)] pub enum UtmlFactorError { ... }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct PanicInfo {
          pub message: String,
          pub backtrace: String,
          pub captured_at_unix_seconds: i64,
          pub last_in_flight_shift: Option&lt;ShiftId&gt;,
      }

      // Time-step + cross-cutting type stubs (replaced when SPEC-PREDICTOR/SPEC-INFERENCE ship)
      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
      pub struct TimeStep(pub u8); // 0, 1, 2

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct PanelSnapshot { pub slots: HashMap&lt;InstrumentSlot, Vec&lt;f32&gt;&gt; }

      pub type AuxHeadId = String;
      pub type EmbedderId = String;
      pub type AttemptId = String;

      // Stubbed cross-cutting types — replaced by concrete imports when SPEC-INFERENCE/SPEC-PREDICTOR/SPEC-CORPUS ship.
      // Stub here intentionally uses serde_json::Value to keep this crate compilable in isolation.
      pub type RealityPrediction = serde_json::Value;
      pub type VerifyVerdict = serde_json::Value;
      pub type OracleOutcome = serde_json::Value;
      pub type AuxHeadPrediction = serde_json::Value;
      pub type CounterfactualResult = serde_json::Value;

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct CaptureAuditBundle {
          pub attempt_id: AttemptId,
          pub attempt_found: bool,
          pub panels: HashMap&lt;TimeStep, PanelSnapshot&gt;,
          pub missing_slots: Vec&lt;InstrumentSlot&gt;,
          pub aux_head_predictions: HashMap&lt;AuxHeadId, AuxHeadPrediction&gt;,
          pub reality_prediction: Option&lt;RealityPrediction&gt;,
          pub verify_verdict: Option&lt;VerifyVerdict&gt;,
          pub oracle_outcome: Option&lt;OracleOutcome&gt;,
          pub l_step: Option&lt;f32&gt;,
          pub utml_factors: Option&lt;UtmlFactorBundle&gt;,
          pub counterfactual_perturbations: Vec&lt;CounterfactualResult&gt;,
          pub bracketing_witness_offsets: (u64, u64),
          pub cache_hit_flags: HashMap&lt;InstrumentSlot, bool&gt;,
          pub panel_shas: HashMap&lt;TimeStep, [u8; 32]&gt;,
          pub embedder_versions: HashMap&lt;EmbedderId, String&gt;,
          pub nearest_neighbors: Vec&lt;AttemptId&gt;,
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct LiveSubscriberStatus {
          pub last_watermark_per_session: HashMap&lt;String, ShiftId&gt;, // SessionId.as_str() keys for JSON-friendly serialization
          pub lag: u64,
          pub lag_alert_active: bool,
          pub error_rate_per_1000: f32,
          pub instrument_cache_hit_rate: f32,
          pub instrument_cache_capacity: usize,
          pub instrument_cache_entries: usize,
          pub rss_bytes: u64,
          pub task_alive_since: Option&lt;i64&gt;,
          pub dropped_l_step_below_threshold_count: u64,
          pub last_panic: Option&lt;PanicInfo&gt;,
      }
    </signature>
  </signatures>

  <constraints>
    - All struct fields exactly match TECH-SHIFT-SUBSCRIBER `<data_models>`; no liberties
    - `WatermarkRecord` JSON serialization MUST be byte-stable (sort keys via `serde_json::to_string` deterministic mode where applicable; field order on the struct is the canonical order)
    - `SlotKey` `Eq + Hash` derives; `code_version` participates in equality (so a redeploy invalidates cache automatically per provenance rule)
    - `LagAlertState` is `Clone + Debug` but NOT `Serialize` (Instant is not serializable); `LagAlertStateTag` IS `Serialize` and is what surfaces to JSON
    - `CircularBuffer<T, const N: usize>` SHALL panic-free `push` (overwrite oldest); no unbounded growth
    - `UtmlFactorBundle::assert_finite_and_clamped` SHALL fail-closed on NaN/Inf or value outside `[0, 1]` (constitution `<utml_v1_invariants>`)
    - Cross-cutting types (`RealityPrediction`, `VerifyVerdict`, `OracleOutcome`, `AuxHeadPrediction`, `CounterfactualResult`) are typedef'd to `serde_json::Value` HERE only; a code comment cites the owning crate so future migration is automated. NO other defaulting.
    - `procfs` dependency is gated to Linux (`[target.'cfg(unix)'.dependencies]` or feature flag) so the crate still compiles on non-Linux dev boxes
    - Files all under 500 lines
    - NO `unwrap()`, NO `expect()` outside test modules
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-shift-subscriber` succeeds with zero warnings
    - JSON round-trip on `WatermarkRecord` is byte-identical for the same input
    - `SlotKey` with same (sha, slot, version) but constructed independently are `==` and have equal `hash`
    - `UtmlFactorBundle::assert_finite_and_clamped` rejects NaN, Inf, -0.001, 1.001
    - `CircularBuffer` overflow at N+1 elements overwrites position 0
    - `LagAlertStateTag` serializes to `0..4` integers
  </verification>
</definition_of_done>

<pseudo_code>
WatermarkRecord::rocksdb_key(sid) -> Vec<u8>:
  let mut k = Vec::with_capacity(3 + sid.as_str().len());
  k.extend_from_slice(b"wm:");
  k.extend_from_slice(sid.as_str().as_bytes());
  k

UtmlFactorBundle::assert_finite_and_clamped:
  for f in [delta_p, delta_k, delta_omega, delta_xi, l_step]:
    if !f.is_finite() || f < 0.0 || f > 1.0:
      return Err(UtmlFactorError::OutOfBounds { factor, value: f })
  Ok(())

CircularBuffer::push(item):
  self.buf[self.write_head] = Some(item);
  self.write_head = (self.write_head + 1) % N;
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/watermark.rs">WatermarkRecord + WatermarkWriter struct skeleton</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/tail.rs">ShiftLogTail + RotationDetector + ReaderHandle skeleton</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/cache.rs">SlotKey + InstrumentCache skeleton</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/metrics.rs">SubscriberMetrics + LagAlertState/Tag + ErrorEvent + CircularBuffer</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/audit.rs">UtmlFactorBundle + PanicInfo + CaptureAuditBundle + LiveSubscriberStatus + cross-cutting type stubs</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add lru, parking_lot, dashmap, procfs (cfg(unix)), context-graph-mejepa-instruments path-dep</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">add `pub mod watermark; pub mod tail; pub mod cache; pub mod metrics; pub mod audit;` + matching `pub use`</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 5 new modules compile and re-exports resolve from outside the crate</criterion>
  <criterion>WatermarkRecord JSON serializes deterministically (test asserts byte equality across two serializations)</criterion>
  <criterion>SlotKey equality + hash is consistent under repeated construction</criterion>
  <criterion>LagAlertState transitions Idle→Watching→Trigger→AlertActive→AlertClearing→Watching covered by unit test</criterion>
  <criterion>UtmlFactorBundle rejects NaN, Inf, 1.001, -1e-6</criterion>
  <criterion>CaptureAuditBundle JSON serializes with all 16 fields populated; deserializes byte-identical</criterion>
  <criterion>No file exceeds 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
