# TASK-INFER-010 — CalibrationStore (CF_MEJEPA_CALIBRATION_HISTORY persist + load_active + recompute_sliding_window)

```xml
<task_spec id="TASK-INFER-010" version="1.0">

<metadata>
  <title>CalibrationStore RocksDB integration in `crates/context-graph-mejepa/src/calibration.rs`</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-INFER-06">Calibration record persistence with corpus_sha + embedder_versions + frozen_at provenance</item>
    <item ref="REQ-INFER-19">CF_MEJEPA_CALIBRATION_HISTORY column family declaration</item>
    <item ref="BR-INFER-04">CalibrationStale on age > max_calibration_age_days</item>
    <item ref="EC-INFER-03">ConformalInsufficientSamples fallback to last-known calibration with degraded</item>
    <item ref="component:CalibrationStore.calibrate">orchestrator pulling 005+006 together</item>
    <item ref="component:CalibrationStore.load_active">read most-recent record + age check</item>
    <item ref="component:CalibrationStore.recompute_sliding_window">continuous calibration per doc 09 §5.5</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (calibration.rs)</estimated_files>
</metadata>

<context>
CalibrationStore is the RocksDB-backed orchestrator that pulls together TASK-INFER-005 (τ
calibration) + TASK-INFER-006 (σ² tuning) and persists the resulting CalibrationRecord to
CF_MEJEPA_CALIBRATION_HISTORY. It implements `load_active()` (read most-recent + check age) and
`recompute_sliding_window()` (per doc 09 §5.5 continuous calibration; reads recent
CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_ORACLE_VERDICTS pairs). compile() reads the active record
on every call; if stale → CalibrationStale fail-closed.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (rocksdb_cfs naming; failure_taxonomy MEJEPA_INFER_CALIBRATION_STALE)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-06; BR-INFER-04; EC-INFER-03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (rocksdb_column_families CF_MEJEPA_CALIBRATION_HISTORY; component_contracts CalibrationStore.{calibrate, load_active, recompute_sliding_window})</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/{calibration_types.rs, conformal.rs, ood.rs, error.rs, config.rs} (TASK-INFER-002/005/006)</file>
  <file purpose="storage_dep">crates/context-graph-storage/src/lib.rs (existing CF registration patterns)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-002 complete; CalibrationRecord exists</check>
  <check>TASK-INFER-005 complete; conformal::calibrate exists</check>
  <check>TASK-INFER-006 complete; ood::tune_sigma_squared exists</check>
  <check>`rocksdb = "0.21"` workspace dep; `bincode = "1"` workspace dep</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/calibration.rs` with:
      - `pub struct CalibrationStore { db: Arc<rocksdb::DB>, cf_name: &'static str, max_age_days: u32 }`
      - `pub const CF_MEJEPA_CALIBRATION_HISTORY: &str = "CF_MEJEPA_CALIBRATION_HISTORY";`
      - `pub fn new(db: Arc<rocksdb::DB>, max_age_days: u32) -> Self` (validates CF exists; creates if missing)
      - `pub fn calibrate(&self, examples: &[CalibrationExample], norm_sq_per_example: &[f32], alpha: f32, min_samples_per_stratum: usize, target_mean_ood: f32, corpus_sha: [u8; 32], embedder_versions: BTreeMap<EmbedderId, String>) -> Result<CalibrationRecord, MejepaInferError>` — calls `conformal::calibrate(...)` (with sigma_squared placeholder=1.0) THEN `ood::tune_sigma_squared(norm_sq_per_example, target_mean_ood)` to produce final record THEN `persist(&record)`; returns the final record
      - `pub fn persist(&self, record: &CalibrationRecord) -> Result<(), MejepaInferError>` — bincode-serialize; key = `frozen_at_be(8) || version_ulid(16)` (24-byte composite); put_cf
      - `pub fn load_active(&self) -> Result<Arc<CalibrationRecord>, MejepaInferError>` — reverse-iterate CF; deserialize most-recent; check `age = now - frozen_at`; if `age > max_age_days * 86400` return `Err(CalibrationStale { version, age_days })`; return Arc
      - `pub fn recompute_sliding_window(&self, recent_attempts: usize, store: &dyn MejepaStore, alpha: f32, target_mean_ood: f32, corpus_sha: [u8; 32], embedder_versions: BTreeMap<EmbedderId, String>) -> Result<CalibrationRecord, MejepaInferError>` — reads `recent_attempts` recent (panel, oracle_outcome) pairs from CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_ORACLE_VERDICTS via the store trait; constructs CalibrationExample list; calls calibrate(); persists with new ULID
      - `pub fn list_history(&self, limit: usize) -> Result<Vec<CalibrationRecord>, MejepaInferError>` — for operator inspection
    - Add integration tests using `tempfile::TempDir` + `rocksdb::DB::open_default`:
      - `calibration_store_persist_load_round_trip`: persist a CalibrationRecord; load_active returns identical record (within bincode roundtrip)
      - `calibration_store_load_active_no_records_returns_stale`: empty CF → CalibrationStale (or specific InsufficientSamples)
      - `calibration_store_load_active_stale_returns_err`: synthesize record with frozen_at=2 years ago → CalibrationStale
      - `calibration_store_load_active_picks_newest_by_frozen_at`: persist 3 records → load_active returns one with max frozen_at
      - `calibrate_end_to_end_with_synthetic_examples`: 100 stratified examples → record persisted with empirical_coverage in [0.88, 0.92]
      - `recompute_sliding_window_uses_mocked_store`: MockedStore returns 100 (panel, oracle_outcome) pairs → new CalibrationRecord persisted
    - Document key encoding: `frozen_at_be(8) || version_ulid(16)` for time-ordered iteration
  </in_scope>
  <out_of_scope>
    - Forward-pass over calibration corpus to produce predicted_test_pass / norm_sq — TASK-INFER-013 CLI driver
    - σ² staleness check separate from τ — single record holds both per TECH-INFERENCE
    - Integration with MeJepaCompiler::new() field — that's TASK-INFER-003
    - Sliding-window auto-refresh trigger — Phase 5 SPEC-SELF-HEALING
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/calibration.rs">
use std::sync::Arc;
use std::collections::BTreeMap;
use rocksdb::DB;
use crate::calibration_types::CalibrationRecord;
use crate::conformal::CalibrationExample;
use crate::error::MejepaInferError;
use crate::types::EmbedderId;
use crate::compiler::MejepaStore;

pub const CF_MEJEPA_CALIBRATION_HISTORY: &amp;str = "CF_MEJEPA_CALIBRATION_HISTORY";

pub struct CalibrationStore {
    db: Arc&lt;DB&gt;,
    max_age_days: u32,
}

impl CalibrationStore {
    pub fn new(db: Arc&lt;DB&gt;, max_age_days: u32) -&gt; Result&lt;Self, MejepaInferError&gt;;
    pub fn calibrate(
        &amp;self,
        examples: &amp;[CalibrationExample],
        norm_sq_per_example: &amp;[f32],
        alpha: f32,
        min_samples_per_stratum: usize,
        target_mean_ood: f32,
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap&lt;EmbedderId, String&gt;,
    ) -&gt; Result&lt;CalibrationRecord, MejepaInferError&gt;;
    pub fn persist(&amp;self, record: &amp;CalibrationRecord) -&gt; Result&lt;(), MejepaInferError&gt;;
    pub fn load_active(&amp;self) -&gt; Result&lt;Arc&lt;CalibrationRecord&gt;, MejepaInferError&gt;;
    pub fn recompute_sliding_window(
        &amp;self,
        recent_attempts: usize,
        store: &amp;dyn MejepaStore,
        alpha: f32,
        target_mean_ood: f32,
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap&lt;EmbedderId, String&gt;,
    ) -&gt; Result&lt;CalibrationRecord, MejepaInferError&gt;;
    pub fn list_history(&amp;self, limit: usize) -&gt; Result&lt;Vec&lt;CalibrationRecord&gt;, MejepaInferError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - CF name MUST be `CF_MEJEPA_CALIBRATION_HISTORY` (constitution rocksdb_cfs naming)
    - Key encoding: 24 bytes total — `frozen_at_be(8) || version_ulid(16)` — enables time-ordered iteration via `iterator_cf` reverse
    - Value serialization: `bincode::serialize` per implementation_notes (fixed-int encoding, little-endian for x86_64)
    - load_active: reverse-iterate; first deserialized record is newest by frozen_at; rejects empty CF with CalibrationStale (not InsufficientSamples per BR-INFER-04 distinction); rejects stale with CalibrationStale
    - calibrate orchestration: per_example norm_sq passed alongside examples (caller computes during forward-pass)
    - recompute_sliding_window persists new record with FRESH ULID; previous remains queryable for replay
    - All `?` sites wrap `e.log_context(file!())`
    - No mock data in tests beyond the synthetic corpus required for calibration semantics
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa calibration` shows ≥ 6 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Round-trip persist/load preserves bytes
    - Stale record returns CalibrationStale
    - Empty CF returns CalibrationStale (not generic NotFound)
    - load_active picks newest by frozen_at across 3 inserted records
  </verification>
</definition_of_done>

<pseudo_code>
pub fn new(db: Arc<DB>, max_age_days: u32) -> Result<Self, MejepaInferError> {
    // Caller is responsible for opening DB with the CF; this just wraps.
    if !db.cf_handle(CF_MEJEPA_CALIBRATION_HISTORY).is_some() {
        return Err(MejepaInferError::DimMismatch { expected: 1, actual: 0 });
    }
    Ok(Self { db, max_age_days })
}

pub fn persist(&self, record: &CalibrationRecord) -> Result<(), MejepaInferError> {
    let cf = self.db.cf_handle(CF_MEJEPA_CALIBRATION_HISTORY).unwrap();
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(&record.frozen_at.to_be_bytes());
    let ulid_bytes: [u8; 16] = ulid::Ulid::from_string(&record.version)
        .map_err(|_| MejepaInferError::DimMismatch { expected: 16, actual: 0 })?.into();
    key.extend_from_slice(&ulid_bytes);
    let value = bincode::serialize(record)
        .map_err(|_| MejepaInferError::DimMismatch { expected: 1, actual: 0 })?;
    self.db.put_cf(cf, key, value).map_err(|_| MejepaInferError::DimMismatch{expected:1, actual:0})?;
    Ok(())
}

pub fn load_active(&self) -> Result<Arc<CalibrationRecord>, MejepaInferError> {
    let cf = self.db.cf_handle(CF_MEJEPA_CALIBRATION_HISTORY).unwrap();
    let mut iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::End);
    if let Some(Ok((_k, v))) = iter.next() {
        let record: CalibrationRecord = bincode::deserialize(&v)
            .map_err(|_| MejepaInferError::DimMismatch{expected:1, actual:0})?;
        let age_days = ((chrono::Utc::now().timestamp() - record.frozen_at) / 86400).max(0) as u32;
        if age_days > self.max_age_days {
            return Err(MejepaInferError::CalibrationStale { version: record.version.clone(), age_days });
        }
        return Ok(Arc::new(record));
    }
    Err(MejepaInferError::CalibrationStale { version: String::new(), age_days: u32::MAX })
}

pub fn calibrate(&self, examples, norm_sq, alpha, min_samples, target, corpus_sha, embedder_versions)
    -> Result<CalibrationRecord, MejepaInferError>
{
    let mut record = crate::conformal::calibrate(examples, alpha, min_samples,
        /*sigma_squared placeholder*/ 1.0, corpus_sha, embedder_versions.clone())?;
    let sigma2 = crate::ood::tune_sigma_squared(norm_sq, target)?;
    record.sigma_squared = sigma2;
    self.persist(&record)?;
    Ok(record)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/calibration.rs">CalibrationStore + 6 integration tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod calibration;`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `rocksdb`, `bincode`, `ulid`, `chrono`, `tempfile` (dev-dep)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa calibration shows ≥ 6 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>persist/load round-trip preserves bytes</criterion>
  <criterion>Stale record returns CalibrationStale</criterion>
  <criterion>load_active picks newest by frozen_at</criterion>
  <criterion>CF name == "CF_MEJEPA_CALIBRATION_HISTORY"</criterion>
  <criterion>recompute_sliding_window persists new ULID; previous remains</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa calibration</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
