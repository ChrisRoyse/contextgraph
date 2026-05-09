# TASK-INFER-002 — Internal data models + error taxonomy (CalibrationRecord, ProvenanceBundle, MejepaInferError, InstrumentCache, SlotKey, MeJepaInferConfig)

```xml
<task_spec id="TASK-INFER-002" version="1.0">

<metadata>
  <title>Define internal inference types + structured error in `crates/context-graph-mejepa/src/{error.rs, config.rs, cache.rs, calibration_types.rs, provenance.rs}`</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-INFER-17">Fail-closed error taxonomy + `.code()` SCREAMING_SNAKE + log_context</item>
    <item ref="component:MejepaInferError">9 variants per failure_taxonomy</item>
    <item ref="component:InstrumentCache">LruCache(SlotKey -&gt; Arc&lt;Vec&lt;f32&gt;&gt;) cap 4_096</item>
    <item ref="component:MeJepaInferConfig">alpha, p_threshold, max_age, p_test_threshold, etc.</item>
    <item ref="data_model:CalibrationRecord">version + alpha + tau + sigma_squared + provenance metadata</item>
    <item ref="data_model:ProvenanceBundle">panel_sha + embedder_versions + calibration_version + ...</item>
    <item ref="data_model:SlotKey">file_sha + slot_id + code_version</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>5 created (error.rs, config.rs, cache.rs, calibration_types.rs, provenance.rs)</estimated_files>
</metadata>

<context>
This task lands the internal types that downstream tasks (003 compile, 004 verify, 005 calibration,
006 OOD, 010 store, 011 MCP, 014 FSV) all consume. The error type is THE single failure surface; its
`.code()` method must return SCREAMING_SNAKE tags listed verbatim in constitution
&lt;failure_taxonomy&gt;. The InstrumentCache + SlotKey are the latency hinge per
&lt;performance_budgets&gt; single_prediction_latency_p50 &lt; 100ms.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy, performance_budgets, error_handling rules)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-17, EC-INFER-09, BR-INFER-04)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (data_models for MejepaInferError, MeJepaInferConfig, CalibrationRecord, ProvenanceBundle, InstrumentCache, SlotKey; component_contracts MejepaInferError::code + log_context)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/types.rs (TASK-INFER-001 — Language, EmbedderId, ChunkId)</file>
  <file purpose="instruments">crates/context-graph-mejepa-instruments/src/lib.rs (InstrumentSlot for SlotKey)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (SCREAMING_SNAKE error codes; CF_ prefix on RocksDB CFs)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-001 complete; `crates/context-graph-mejepa/src/types.rs` ships Language, EmbedderId, ChunkId, RealityPrediction</check>
  <check>`thiserror = "1"` available as workspace dep</check>
  <check>`lru = "0.12"` available as workspace dep</check>
  <check>`parking_lot = "0.12"` available as workspace dep</check>
  <check>`tracing = "0.1"` available as workspace dep</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/error.rs` with `MejepaInferError` enum (9 variants from REQ-INFER-17 verbatim) + `.code()` returning SCREAMING_SNAKE + `.log_context()` emitting structured tracing event with payload
    - Replace TASK-INFER-001's placeholder `Result&lt;_, &amp;'static str&gt;` returns in `RealityPrediction::try_new` and `ConformalSet::try_new` with `Result&lt;_, MejepaInferError&gt;` (wired via NanDetected variant)
    - Create `src/config.rs` with `MeJepaInferConfig` carrying all knobs from TECH-INFERENCE data_models block (alpha, p_threshold, max_calibration_age_days, max_constellation_age_days, p_test_threshold, ood_refuse_threshold, outcome_set_max, bootstrap_delta_omega, bootstrap_delta_xi, train_cert_window_steps, instrument_cache_capacity); `Default` impl matches the constitution defaults
    - Create `src/cache.rs` with `InstrumentCache` (RwLock&lt;LruCache&lt;SlotKey, Arc&lt;Vec&lt;f32&gt;&gt;&gt;&gt;) cap 4_096 + `SlotKey { file_sha: [u8; 32], slot_id: InstrumentSlot, code_version: String }` (deserialized to a stable byte rep before LruCache key); `get` (read-lock) + `insert` (write-lock); singleflight stub (per-key Tokio semaphore wired in TASK-INFER-003)
    - Create `src/calibration_types.rs` with `CalibrationRecord` (version: String ULID, alpha: f32, tau: f32, sigma_squared: f32, calibration_corpus_sha: [u8; 32], embedder_versions: BTreeMap&lt;EmbedderId, String&gt;, frozen_at: i64 unix_seconds, num_samples: usize, empirical_coverage: f32) — bincode + serde_json round-trippable
    - Create `src/provenance.rs` with `ProvenanceBundle { panel_sha: [u8; 32], embedder_versions, calibration_version: Option&lt;String&gt;, training_cert_chain_offset: u64, constellation_version: Option&lt;String&gt;, code_version: String, provenance_warnings: Vec&lt;String&gt; }`
    - Update `src/lib.rs`: `pub mod {error, config, cache, calibration_types, provenance};` + re-export
    - Add unit tests: (a) every MejepaInferError variant `.code()` returns the canonical SCREAMING_SNAKE tag from constitution; (b) Default config matches constitution defaults; (c) InstrumentCache hit/miss/evict with synthetic LRU pressure; (d) CalibrationRecord serde round-trip (bincode + JSON); (e) ProvenanceBundle accepts None for unloadable components and emits provenance_warnings (per EC-INFER-10)
  </in_scope>
  <out_of_scope>
    - `MeJepaCompiler` impl (TASK-INFER-003)
    - `CalibrationStore` (RocksDB-backed; TASK-INFER-010)
    - Verify-ladder logic (TASK-INFER-004)
    - Granger F-test wiring (consumer of `granger.rs`; TASK-INFER-003)
    - Singleflight per-key semaphore implementation (TASK-INFER-003)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/error.rs">
use std::path::PathBuf;
use crate::types::{Language, EmbedderId, ChunkId};

#[derive(Debug, thiserror::Error)]
pub enum MejepaInferError {
    #[error("ood refuse: score={score:.4} panel_sha={panel_sha:?}")]
    OodRefuse { score: f32, panel_sha: [u8; 32] },
    #[error("dim mismatch: expected={expected} actual={actual}")]
    DimMismatch { expected: usize, actual: usize },
    #[error("instrument missing: instrument={instrument} language={language:?}")]
    InstrumentMissing { instrument: String, language: Language },
    #[error("nan detected: field={field} witness_chain_offset={witness_chain_offset}")]
    NanDetected { field: String, witness_chain_offset: u64 },
    #[error("calibration stale: version={version} age_days={age_days}")]
    CalibrationStale { version: String, age_days: u32 },
    #[error("conformal insufficient samples: required={required} observed={observed} language={language:?}")]
    ConformalInsufficientSamples { required: usize, observed: usize, language: Language },
    #[error("witness chain broken at offset={offset}")]
    WitnessChainBroken { offset: u64 },
    #[error("source sha drift at path={path:?} claimed={claimed:?} observed={observed:?}")]
    SourceShaDrift { path: PathBuf, claimed: [u8; 32], observed: [u8; 32] },
    #[error("gtau stale constellation: version={version} age_days={age_days}")]
    GtauStaleConstellation { version: String, age_days: u32 },
    #[error("feature disabled: {feature}")]
    FeatureDisabled { feature: &amp;'static str },
}

impl MejepaInferError {
    pub fn code(&amp;self) -&gt; &amp;'static str {
        match self {
            Self::OodRefuse { .. } =&gt; "MEJEPA_INFER_OOD_REFUSE",
            Self::DimMismatch { .. } =&gt; "MEJEPA_INFER_DIM_MISMATCH",
            Self::InstrumentMissing { .. } =&gt; "MEJEPA_INFER_INSTRUMENT_MISSING",
            Self::NanDetected { .. } =&gt; "MEJEPA_INFER_NAN_DETECTED",
            Self::CalibrationStale { .. } =&gt; "MEJEPA_INFER_CALIBRATION_STALE",
            Self::ConformalInsufficientSamples { .. } =&gt; "MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES",
            Self::WitnessChainBroken { .. } =&gt; "MEJEPA_INFER_WITNESS_CHAIN_BROKEN",
            Self::SourceShaDrift { .. } =&gt; "MEJEPA_INFER_SOURCE_SHA_DRIFT",
            Self::GtauStaleConstellation { .. } =&gt; "MEJEPA_INFER_GTAU_STALE_CONSTELLATION",
            Self::FeatureDisabled { .. } =&gt; "MEJEPA_INFER_FEATURE_DISABLED",
        }
    }
    pub fn log_context(&amp;self, target: &amp;str);
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/config.rs">
#[derive(Debug, Clone)]
pub struct MeJepaInferConfig {
    pub alpha: f32,
    pub p_threshold: f32,
    pub max_calibration_age_days: u32,
    pub max_constellation_age_days: u32,
    pub p_test_threshold: f32,
    pub ood_refuse_threshold: f32,
    pub outcome_set_max: usize,
    pub bootstrap_delta_omega: f32,
    pub bootstrap_delta_xi: f32,
    pub train_cert_window_steps: usize,
    pub instrument_cache_capacity: usize,
}

impl Default for MeJepaInferConfig {
    fn default() -&gt; Self {
        Self { alpha: 0.10, p_threshold: 0.01, max_calibration_age_days: 30,
               max_constellation_age_days: 90, p_test_threshold: 0.80,
               ood_refuse_threshold: 0.5, outcome_set_max: 2,
               bootstrap_delta_omega: 0.5, bootstrap_delta_xi: 0.5,
               train_cert_window_steps: 128, instrument_cache_capacity: 4_096 }
    }
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/cache.rs">
use std::sync::Arc;
use std::num::NonZeroUsize;
use lru::LruCache;
use parking_lot::RwLock;
use crate::error::MejepaInferError;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SlotKey {
    pub file_sha: [u8; 32],
    pub slot_id: context_graph_mejepa_instruments::InstrumentSlot,
    pub code_version: String,
}

pub struct InstrumentCache {
    lru: RwLock&lt;LruCache&lt;SlotKey, Arc&lt;Vec&lt;f32&gt;&gt;&gt;&gt;,
    capacity: usize,
}

impl InstrumentCache {
    pub fn new(capacity: usize) -&gt; Self;
    pub fn get(&amp;self, key: &amp;SlotKey) -&gt; Option&lt;Arc&lt;Vec&lt;f32&gt;&gt;&gt;;
    pub fn insert(&amp;self, key: SlotKey, value: Arc&lt;Vec&lt;f32&gt;&gt;);
    pub fn capacity(&amp;self) -&gt; usize;
    pub fn len(&amp;self) -&gt; usize;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/calibration_types.rs">
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::types::EmbedderId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationRecord {
    pub version: String,
    pub alpha: f32,
    pub tau: f32,
    pub sigma_squared: f32,
    pub calibration_corpus_sha: [u8; 32],
    pub embedder_versions: BTreeMap&lt;EmbedderId, String&gt;,
    pub frozen_at: i64,
    pub num_samples: usize,
    pub empirical_coverage: f32,
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/provenance.rs">
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::types::EmbedderId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceBundle {
    pub panel_sha: [u8; 32],
    pub embedder_versions: BTreeMap&lt;EmbedderId, String&gt;,
    pub calibration_version: Option&lt;String&gt;,
    pub training_cert_chain_offset: u64,
    pub constellation_version: Option&lt;String&gt;,
    pub code_version: String,
    pub provenance_warnings: Vec&lt;String&gt;,
}
    </signature>
  </signatures>
  <constraints>
    - `.code()` returns ONLY values listed in constitution &lt;failure_taxonomy&gt;: MEJEPA_INFER_OOD_REFUSE, MEJEPA_INFER_DIM_MISMATCH, MEJEPA_INFER_INSTRUMENT_MISSING, MEJEPA_INFER_NAN_DETECTED, MEJEPA_INFER_CALIBRATION_STALE, MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES, MEJEPA_INFER_WITNESS_CHAIN_BROKEN, MEJEPA_INFER_SOURCE_SHA_DRIFT, MEJEPA_INFER_GTAU_STALE_CONSTELLATION, MEJEPA_INFER_FEATURE_DISABLED
    - `log_context` uses `tracing::error!` macro with structured fields (path, score, dim_expected, dim_actual, embedder, language, witness_chain_offset, etc.) per constitution &lt;error_handling&gt; "robust error logging"
    - `MeJepaInferConfig::default()` matches the constitution / TECH-INFERENCE defaults verbatim (alpha=0.10, p_threshold=0.01, max_calibration_age_days=30, max_constellation_age_days=90, p_test_threshold=0.80, ood_refuse_threshold=0.5, outcome_set_max=2, train_cert_window_steps=128, instrument_cache_capacity=4_096, bootstrap_delta_omega=0.5, bootstrap_delta_xi=0.5)
    - `MeJepaInferConfig` validation: alpha ∈ [0.01, 0.50]; p_threshold ∈ [0.0001, 0.05] (cannot be loosened above 0.05 per anti-overfit)
    - `InstrumentCache::new` panics if `capacity == 0` (NonZeroUsize wrapper)
    - SlotKey deserialization is byte-stable; iteration order across hash equality preserved
    - `RealityPrediction::try_new` and `ConformalSet::try_new` updated to return `Result&lt;_, MejepaInferError&gt;` (NanDetected variant); existing tests in TASK-INFER-001 updated to match
    - `parking_lot::RwLock` chosen because RwLockReadGuard from std is !Send across `.await` (per memory key lesson)
    - `bincode` + `serde_json` BOTH supported on CalibrationRecord (bincode for CF storage, JSON for FSV)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds (no warnings)
    - `cargo test -p context-graph-mejepa error config cache calibration_types provenance` shows ≥ 12 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Each of 10 MejepaInferError variants `.code()` returns the canonical tag from constitution
    - `MeJepaInferConfig::default().outcome_set_max == 2`
    - InstrumentCache evicts LRU when capacity exceeded (synthetic test with cap=4 + 5 inserts)
    - CalibrationRecord round-trips bincode AND serde_json
    - ProvenanceBundle with `Option::None` for calibration_version + warning array round-trips
  </verification>
</definition_of_done>

<pseudo_code>
src/error.rs:
  pub fn log_context(&self, target: &str) {
      use tracing::error;
      match self {
          Self::OodRefuse { score, panel_sha } =>
              error!(target = target, code = self.code(), score = score, panel_sha = ?panel_sha, "ood refuse"),
          Self::DimMismatch { expected, actual } =>
              error!(target = target, code = self.code(), expected, actual, "dim mismatch"),
          // ... per-variant structured emit ...
      }
  }

src/cache.rs:
  pub fn new(capacity: usize) -> Self {
      let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
      Self { lru: RwLock::new(LruCache::new(cap)), capacity }
  }
  pub fn get(&self, key: &SlotKey) -> Option<Arc<Vec<f32>>> {
      self.lru.write().get(key).cloned()  // LruCache::get takes &mut for LRU bookkeeping
  }
  pub fn insert(&self, key: SlotKey, value: Arc<Vec<f32>>) {
      self.lru.write().put(key, value);
  }

# update src/types.rs: replace `Result<_, &'static str>` with `Result<_, MejepaInferError>` using NanDetected.
# All TASK-INFER-001 tests updated to match.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/error.rs">MejepaInferError + .code() + log_context</file>
  <file path="crates/context-graph-mejepa/src/config.rs">MeJepaInferConfig + Default + validate</file>
  <file path="crates/context-graph-mejepa/src/cache.rs">InstrumentCache + SlotKey</file>
  <file path="crates/context-graph-mejepa/src/calibration_types.rs">CalibrationRecord</file>
  <file path="crates/context-graph-mejepa/src/provenance.rs">ProvenanceBundle</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod error; pub mod config; pub mod cache; pub mod calibration_types; pub mod provenance; pub use {error::*, config::*, cache::*, calibration_types::*, provenance::*};`</file>
  <file path="crates/context-graph-mejepa/src/types.rs">replace `Result&lt;_, &amp;'static str&gt;` with `Result&lt;_, MejepaInferError&gt;`; update tests accordingly</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `thiserror`, `lru`, `parking_lot`, `tracing`, `bincode`, `chrono` (or use raw i64 unix), `ulid` deps</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows ≥ 12 passing tests across error/config/cache/calibration_types/provenance</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>error_codes_match_constitution_taxonomy passes (10 .code() values)</criterion>
  <criterion>config_default_matches_constitution_defaults passes</criterion>
  <criterion>instrument_cache_evicts_lru passes (cap=4, 5 inserts → first key absent)</criterion>
  <criterion>calibration_record_bincode_round_trip passes</criterion>
  <criterion>calibration_record_json_round_trip passes</criterion>
  <criterion>provenance_bundle_with_none_components_round_trips passes (EC-INFER-10)</criterion>
  <criterion>RealityPrediction::try_new now returns MejepaInferError::NanDetected on NaN</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
