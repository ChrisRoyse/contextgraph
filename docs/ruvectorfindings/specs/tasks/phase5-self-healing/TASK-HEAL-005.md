# TASK-HEAL-005 — RocksDB column families (CF_MEJEPA_FISHER_SNAPSHOTS, CF_MEJEPA_DRIFT_WINDOW, CF_MEJEPA_DRIFT_HISTORY, CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_CALIBRATION_HISTORY)

```xml
<task_spec id="TASK-HEAL-005" version="1.0">

<metadata>
  <title>RocksDB CF descriptors + key/value codecs in `crates/context-graph-mejepa/src/heal/cf.rs`</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-HEAL-02">CF_MEJEPA_FISHER_SNAPSHOTS for TaskSnapshot persistence</item>
    <item ref="REQ-HEAL-03">CF_MEJEPA_DRIFT_WINDOW + CF_MEJEPA_DRIFT_HISTORY</item>
    <item ref="REQ-HEAL-04">CF_MEJEPA_ACTIVE_POINTERS for atomic active-weight pointer updates; CF_MEJEPA_HEAL_REPORTS</item>
    <item ref="REQ-HEAL-06">CF_MEJEPA_CALIBRATION_HISTORY</item>
    <item ref="REQ-HEAL-12">CF_MEJEPA_WEIGHT_BLOBS already shipped (referenced, not redefined)</item>
    <item ref="REQ-HEAL-20">CF_MEJEPA_SHIFT_WATERMARK already shipped (referenced)</item>
    <item ref="constitution:rocksdb_cfs">SCREAMING_SNAKE CF_ prefix per coding_standards naming_conventions</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-001</task_ref>
    <task_ref>TASK-HEAL-002</task_ref>
    <task_ref>TASK-HEAL-003</task_ref>
    <task_ref>TASK-HEAL-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/cf.rs)</estimated_files>
</metadata>

<context>
Foundation 5 of 5. Lands the RocksDB column-family descriptor names + key/value codecs for the
six new SELF-HEALING CFs. The two already-shipped CFs (`CF_MEJEPA_WEIGHT_BLOBS` from Phase 3
training, `CF_MEJEPA_SHIFT_WATERMARK` from Phase 7 shift-subscriber) are imported by name —
not redefined here per the no-backwards-compat rule. Each CF has an explicit key encoder + bincode
value codec + retention policy comment matching `<rocksdb_column_families>` in TECH-SELF-HEALING.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<coding_standards>` rocksdb_cfs naming; `<performance_budgets>` rocksdb_disk_total_quota ≤ 115 GB)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (rocksdb_column_families block — six new CFs + retention policies)</file>
  <file purpose="prior_task">TASK-HEAL-001 (TaskSnapshot)</file>
  <file purpose="prior_task">TASK-HEAL-002 (DriftSample, SeverityHistoryEntry)</file>
  <file purpose="prior_task">TASK-HEAL-003 (HoldoutEval, ModelHandle)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealReport, CalibrationRecord)</file>
  <file purpose="cross_domain">crates/context-graph-storage/src/lib.rs (RocksDbStore + ColumnFamilyDescriptor wrapper) — read for the existing CF-registration pattern</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-001..004 complete</check>
  <check>`crates/context-graph-storage` exposes a `ColumnFamilyName(&'static str)` newtype or equivalent registration API</check>
  <check>`bincode = "1"` with serde feature enabled</check>
</prerequisites>

<scope>
  <in_scope>
    - Fill `crates/context-graph-mejepa/src/heal/cf.rs` with:
      - Six `pub const` CF name strings: `CF_MEJEPA_FISHER_SNAPSHOTS`, `CF_MEJEPA_DRIFT_WINDOW`, `CF_MEJEPA_DRIFT_HISTORY`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_ACTIVE_POINTERS`, `CF_MEJEPA_CALIBRATION_HISTORY`. Each is a `&'static str` with the literal SCREAMING_SNAKE name.
      - Two re-exports referencing already-shipped CFs (DO NOT redefine): `pub use crate::cf_already_shipped::{CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_SHIFT_WATERMARK};` (or, if the storage crate exposes them, `pub use context_graph_storage::cfs::{CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_SHIFT_WATERMARK};`). The exact import path depends on the storage crate's surface — falling-back to documenting them as `"CF_MEJEPA_WEIGHT_BLOBS"` consts when not yet exported.
      - `pub fn all_self_healing_cf_names() -> [&'static str; 6]` returning the six new CFs.
      - `pub fn all_referenced_cf_names() -> Vec<&'static str>` returning the six new + two already-shipped (8 total).
      - Key encoders (one per CF):
        - `pub fn encode_fisher_snapshot_key(boundary_step: u64) -> [u8; 8]` (big-endian)
        - `pub fn decode_fisher_snapshot_key(bytes: &[u8]) -> Result<u64, HealError>` (rejects len != 8)
        - `pub fn encode_drift_window_key(ring_buffer_offset: u64) -> [u8; 8]` (big-endian)
        - `pub fn encode_drift_history_key(timestamp_seconds: i64) -> [u8; 8]` (big-endian; signed-as-unsigned via `as u64` is fine for sort order on positive timestamps)
        - `pub fn encode_heal_report_key(timestamp_seconds: i64) -> [u8; 8]` (big-endian)
        - `pub fn encode_active_pointer_key(name: &str) -> Vec<u8>` returning UTF-8 bytes; valid `name` ∈ `{"active_weights", "active_calibration", "active_constellation"}` validated via `validate_active_pointer_name(name) -> Result<(), HealError>`
        - `pub fn encode_calibration_history_key(frozen_at_seconds: i64) -> [u8; 8]` (big-endian)
      - Value codecs: per-CF helper `pub fn encode_value(v: &T) -> Result<Vec<u8>, HealError>` and `pub fn decode_value(bytes: &[u8]) -> Result<T, HealError>` for each of:
        - TaskSnapshot (CF_MEJEPA_FISHER_SNAPSHOTS)
        - DriftSample (CF_MEJEPA_DRIFT_WINDOW)
        - SeverityHistoryEntry (CF_MEJEPA_DRIFT_HISTORY) wrapped as `DriftHistoryRecord { severity, empirical_coverage, window_start_offset, window_end_offset }` per data_models
        - HealReport (CF_MEJEPA_HEAL_REPORTS)
        - ActivePointerValue { theta_sha_or_version: Vec<u8>, frozen_at: i64 } (CF_MEJEPA_ACTIVE_POINTERS) — Vec<u8> covers both `[u8; 32]` (theta_sha) and `String` (version)
        - CalibrationRecord (CF_MEJEPA_CALIBRATION_HISTORY)
      - Retention-policy doc-comments on each CF constant per data_models block (e.g., `/// Retention: bounded by per-embedder cap (1 base + ≤4 deltas); SVD merge older snapshots into base.`).
      - Helper: `pub fn cf_retention_descriptor(cf_name: &str) -> Option<RetentionPolicy>` returning a structured RetentionPolicy `{ time_capped_days: Option<u32>, count_capped: Option<u64>, byte_capped_gb: Option<u32>, single_row_per_key: bool }`.
    - Module-level test set in `#[cfg(test)] mod tests`:
      - `cf_names_are_screaming_snake_with_cf_prefix` (regex check on all 6 + 2 referenced)
      - `cf_names_are_unique` (no duplicates)
      - `all_self_healing_cf_names_returns_six` (count check)
      - `encode_decode_fisher_snapshot_key_round_trip`
      - `decode_fisher_snapshot_key_rejects_seven_byte_input`
      - `encode_drift_window_key_big_endian` (asserts known input → known output)
      - `encode_active_pointer_key_validates_known_names` (rejects unknown)
      - `encode_active_pointer_key_accepts_active_weights`
      - `encode_active_pointer_key_accepts_active_calibration`
      - `encode_active_pointer_key_accepts_active_constellation`
      - `encode_decode_task_snapshot_value_round_trip` (real TaskSnapshot from TASK-HEAL-001)
      - `encode_decode_drift_sample_value_round_trip`
      - `encode_decode_heal_report_value_round_trip`
      - `encode_decode_calibration_record_value_round_trip`
      - `cf_retention_descriptor_for_drift_history_is_90_days`
      - `cf_retention_descriptor_for_calibration_history_is_30_records`
      - `cf_retention_descriptor_for_active_pointers_is_single_row_per_key`
  </in_scope>
  <out_of_scope>
    - Live RocksDB CF registration (Phase 7 wires this; this task only ships descriptors)
    - Compaction filters / TTL policies (Phase 6 hygiene owns)
    - Backup/restore (Phase 6)
    - The actual RocksDbStore::open_cfs() invocation (deferred to TASK-HEAL-008 SelfHealingPipeline::new)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/cf.rs">
use crate::heal::ewc::TaskSnapshot;
use crate::heal::drift::{DriftSample, SeverityHistoryEntry};
use crate::heal::status::HealReport;
use crate::heal::calibration::CalibrationRecord;
use crate::heal::errors::HealError;
use serde::{Deserialize, Serialize};

pub const CF_MEJEPA_FISHER_SNAPSHOTS: &amp;str = "CF_MEJEPA_FISHER_SNAPSHOTS";
pub const CF_MEJEPA_DRIFT_WINDOW: &amp;str = "CF_MEJEPA_DRIFT_WINDOW";
pub const CF_MEJEPA_DRIFT_HISTORY: &amp;str = "CF_MEJEPA_DRIFT_HISTORY";
pub const CF_MEJEPA_HEAL_REPORTS: &amp;str = "CF_MEJEPA_HEAL_REPORTS";
pub const CF_MEJEPA_ACTIVE_POINTERS: &amp;str = "CF_MEJEPA_ACTIVE_POINTERS";
pub const CF_MEJEPA_CALIBRATION_HISTORY: &amp;str = "CF_MEJEPA_CALIBRATION_HISTORY";

/// Already-shipped (Phase 3 training) — do NOT redefine.
pub const CF_MEJEPA_WEIGHT_BLOBS: &amp;str = "CF_MEJEPA_WEIGHT_BLOBS";
/// Already-shipped (Phase 7 shift-subscriber).
pub const CF_MEJEPA_SHIFT_WATERMARK: &amp;str = "CF_MEJEPA_SHIFT_WATERMARK";

pub fn all_self_healing_cf_names() -&gt; [&amp;'static str; 6];
pub fn all_referenced_cf_names() -&gt; Vec&lt;&amp;'static str&gt;;

pub fn encode_fisher_snapshot_key(boundary_step: u64) -&gt; [u8; 8];
pub fn decode_fisher_snapshot_key(bytes: &amp;[u8]) -&gt; Result&lt;u64, HealError&gt;;
pub fn encode_drift_window_key(ring_buffer_offset: u64) -&gt; [u8; 8];
pub fn encode_drift_history_key(timestamp_seconds: i64) -&gt; [u8; 8];
pub fn encode_heal_report_key(timestamp_seconds: i64) -&gt; [u8; 8];
pub fn encode_active_pointer_key(name: &amp;str) -&gt; Result&lt;Vec&lt;u8&gt;, HealError&gt;;
pub fn encode_calibration_history_key(frozen_at_seconds: i64) -&gt; [u8; 8];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivePointerValue {
    pub theta_sha_or_version: Vec&lt;u8&gt;,
    pub frozen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftHistoryRecord {
    pub severity: crate::heal::drift::DriftSeverity,
    pub empirical_coverage: f32,
    pub window_start_offset: u64,
    pub window_end_offset: u64,
}

pub fn encode_value&lt;T: Serialize&gt;(v: &amp;T) -&gt; Result&lt;Vec&lt;u8&gt;, HealError&gt;;
pub fn decode_value&lt;T: for&lt;'de&gt; Deserialize&lt;'de&gt;&gt;(bytes: &amp;[u8]) -&gt; Result&lt;T, HealError&gt;;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RetentionPolicy {
    pub time_capped_days: Option&lt;u32&gt;,
    pub count_capped: Option&lt;u64&gt;,
    pub byte_capped_gb: Option&lt;u32&gt;,
    pub single_row_per_key: bool,
}

pub fn cf_retention_descriptor(cf_name: &amp;str) -&gt; Option&lt;RetentionPolicy&gt;;
    </signature>
  </signatures>
  <constraints>
    - File &lt; 500 lines
    - All CF names match SCREAMING_SNAKE with `CF_MEJEPA_` prefix
    - Big-endian u64/i64 encoding for sort-order keys
    - `bincode::serialize` / `bincode::deserialize` for value codecs (mapped to HealError)
    - `validate_active_pointer_name` accepts exactly `{"active_weights", "active_calibration", "active_constellation"}` — anything else returns `HealError::FeatureDisabled`-style structured rejection
    - Retention policies match data_models exactly:
      - FISHER_SNAPSHOTS: count_capped=Some(N) where N=PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS × num_embedders (use a runtime computation, NOT a hardcoded value; default `Some(105)` for 21 embedders × 5 cap)
      - DRIFT_WINDOW: count_capped=Some(1000), single_row_per_key=false
      - DRIFT_HISTORY: time_capped_days=Some(90)
      - HEAL_REPORTS: time_capped_days=Some(365)
      - ACTIVE_POINTERS: single_row_per_key=true
      - CALIBRATION_HISTORY: count_capped=Some(30)
    - All public types `Debug + Clone + Serialize + Deserialize + PartialEq`
    - File begins with `// RocksDB CF descriptors for SELF-HEALING domain (per TECH-SELF-HEALING <rocksdb_column_families>). Two CFs (CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_SHIFT_WATERMARK) are already-shipped and re-exported by name only.`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` zero warnings
    - `cargo test -p context-graph-mejepa heal::cf` shows ≥ 17 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - `all_self_healing_cf_names().len() == 6`
    - `all_referenced_cf_names().len() == 8`
    - `encode_active_pointer_key("foo")` returns Err
    - `encode_active_pointer_key("active_weights")` returns Ok with bytes == "active_weights".as_bytes()
    - Round-trip codec works for every value type
    - cf_retention_descriptor matches data_models block byte-for-byte for each of the 6 new CFs
  </verification>
</definition_of_done>

<pseudo_code>
pub fn encode_fisher_snapshot_key(boundary_step: u64) -> [u8; 8] {
    boundary_step.to_be_bytes()
}

pub fn decode_fisher_snapshot_key(bytes: &[u8]) -> Result<u64, HealError> {
    if bytes.len() != 8 {
        return Err(HealError::FeatureDisabled); // placeholder; real variant added once we wire StorageError.
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(bytes);
    Ok(u64::from_be_bytes(buf))
}

pub fn encode_active_pointer_key(name: &str) -> Result<Vec<u8>, HealError> {
    match name {
        "active_weights" | "active_calibration" | "active_constellation" => Ok(name.as_bytes().to_vec()),
        _ => Err(HealError::FeatureDisabled), // structured rejection
    }
}

pub fn cf_retention_descriptor(cf_name: &str) -> Option<RetentionPolicy> {
    match cf_name {
        CF_MEJEPA_FISHER_SNAPSHOTS => Some(RetentionPolicy {
            time_capped_days: None,
            count_capped: Some(105), // 21 embedders × (1 base + 4 deltas)
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_DRIFT_WINDOW => Some(RetentionPolicy {
            time_capped_days: None,
            count_capped: Some(1000),
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_DRIFT_HISTORY => Some(RetentionPolicy {
            time_capped_days: Some(90), count_capped: None,
            byte_capped_gb: None, single_row_per_key: false,
        }),
        CF_MEJEPA_HEAL_REPORTS => Some(RetentionPolicy {
            time_capped_days: Some(365), count_capped: None,
            byte_capped_gb: None, single_row_per_key: false,
        }),
        CF_MEJEPA_ACTIVE_POINTERS => Some(RetentionPolicy {
            time_capped_days: None, count_capped: None,
            byte_capped_gb: None, single_row_per_key: true,
        }),
        CF_MEJEPA_CALIBRATION_HISTORY => Some(RetentionPolicy {
            time_capped_days: None, count_capped: Some(30),
            byte_capped_gb: None, single_row_per_key: false,
        }),
        _ => None,
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/cf.rs">CF name constants + key encoders + value codecs + RetentionPolicy + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod cf;` to existing scaffold</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa heal::cf shows ≥ 17 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>All six new CF names match SCREAMING_SNAKE pattern with CF_MEJEPA_ prefix</criterion>
  <criterion>encode/decode round-trip for all six value types</criterion>
  <criterion>encode_active_pointer_key rejects unknown names</criterion>
  <criterion>cf_retention_descriptor matches data_models block exactly</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::cf</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
