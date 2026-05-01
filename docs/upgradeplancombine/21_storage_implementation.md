# 21 — Storage Implementation Plan

This document is the file-level diff plan for the storage layer
changes (PRs 1, 2, 4 of `20_implementation_plan_overview.md`).

## 1. Files touched (Phase 1, PR 1)

### 1.1 `crates/context-graph-storage/src/dynamicjepa/column_families.rs`

Changes:

- Add three new constants (matching the existing snake-case
  convention):
  ```rust
  pub const CF_DJ_PAIRWISE_READINGS: &str = "dj_pairwise_readings";
  pub const CF_DJ_CONSTELLATIONS: &str = "dj_constellations";
  pub const CF_DJ_THRESHOLD_CALIBRATIONS: &str = "dj_threshold_calibrations";
  ```
- Append all three to `DJ_CF_NAMES` array.
- Update `DJ_CF_COUNT` (currently 25) to **28**.
- Extend `dj_cf_descriptors(cache: &Cache)` to register the three
  new CFs:
  ```rust
  ColumnFamilyDescriptor::new(
      CF_DJ_PAIRWISE_READINGS,
      dj_uuid_point_cf_options(cache),
  ),
  ColumnFamilyDescriptor::new(
      CF_DJ_CONSTELLATIONS,
      dj_composite_index_cf_options(cache),
  ),
  ColumnFamilyDescriptor::new(
      CF_DJ_THRESHOLD_CALIBRATIONS,
      dj_composite_index_cf_options(cache),
  ),
  ```

### 1.2 `crates/context-graph-storage/src/dynamicjepa/mod.rs`

Add three module declarations:

```rust
pub mod pairwise_store;
pub mod constellation_store;
pub mod threshold_store;
```

And re-export the public API:

```rust
pub use pairwise_store::{
    get_pairwise_reading,
    list_pairwise_readings_for_event,
    put_panel_with_readings_and_pairwise_batch,
};
pub use constellation_store::{
    get_constellation,
    list_constellations,
    put_constellation_with_audit_batch,
};
pub use threshold_store::{
    get_threshold_calibration,
    list_threshold_calibrations,
    put_threshold_calibration_with_audit_batch,
};
```

### 1.3 `crates/context-graph-storage/src/dynamicjepa/keys.rs`

Add three new key constructor functions:

```rust
pub fn pairwise_reading_key(
    event_uuid: &Uuid,
    instrument_j: u32,
    instrument_k: u32,
) -> [u8; 24] {
    let mut key = [0u8; 24];
    key[0..16].copy_from_slice(event_uuid.as_bytes());
    key[16..20].copy_from_slice(&instrument_j.to_le_bytes());
    key[20..24].copy_from_slice(&instrument_k.to_le_bytes());
    key
}

pub fn constellation_key(
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
) -> [u8; 36] {
    let mut key = [0u8; 36];
    key[0..16].copy_from_slice(domain_uuid.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(subject_id.as_bytes());
    let hash = hasher.finalize();
    key[16..32].copy_from_slice(&hash[..16]);
    key[32..36].copy_from_slice(&modality_id.to_le_bytes());
    key
}

pub fn threshold_calibration_key(
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
    supersede_seq: u32,
) -> [u8; 40] {
    let mut key = [0u8; 40];
    key[0..36].copy_from_slice(&constellation_key(domain_uuid, subject_id, modality_id));
    key[36..40].copy_from_slice(&supersede_seq.to_le_bytes());
    key
}
```

Tests in `crates/context-graph-storage/src/dynamicjepa/keys.rs`:

- `test_pairwise_reading_key_format_24_bytes_fixed`
- `test_constellation_key_format_36_bytes_fixed`
- `test_threshold_calibration_key_format_40_bytes_fixed`
- `test_subject_id_hash_stable_across_calls`

### 1.4 `crates/context-graph-storage/src/dynamicjepa/pairwise_store.rs` (NEW)

Skeleton (full implementation in PR 4):

```rust
use crate::dynamicjepa::{column_families::*, keys::*};
use crate::dynamicjepa::audit::DjAuditRecord;
use context_graph_core::dynamicjepa::PairwiseReading;
use rocksdb::WriteBatch;

pub fn put_pairwise_reading_with_audit_batch(
    store: &RocksDbTeleologicalStore,
    batch: &mut WriteBatch,
    reading: &PairwiseReading,
    audit: &DjAuditRecord,
) -> Result<(), DjError> {
    // implementation
}

pub fn get_pairwise_reading(
    store: &RocksDbTeleologicalStore,
    event_uuid: &Uuid,
    instrument_j: u32,
    instrument_k: u32,
) -> Result<Option<PairwiseReading>, DjError> {
    // implementation
}

pub fn list_pairwise_readings_for_event(
    store: &RocksDbTeleologicalStore,
    event_uuid: &Uuid,
) -> Result<Vec<PairwiseReading>, DjError> {
    // prefix scan with event_uuid as prefix
}

pub fn put_panel_with_readings_and_pairwise_batch(
    store: &RocksDbTeleologicalStore,
    panel: LatentPanel,
    readings: Vec<InstrumentReading>,
    pairwise_readings: Vec<PairwiseReading>,
    audit: DjAuditRecord,
) -> Result<(), DjError> {
    // single-WriteBatch atomic write of panel + all readings + all pair rows
}
```

### 1.5 `crates/context-graph-storage/src/dynamicjepa/constellation_store.rs` (NEW)

Skeleton (full implementation in PR 7):

```rust
use crate::dynamicjepa::{column_families::*, keys::*};
use context_graph_core::dynamicjepa::ConstellationCentroid;

pub fn put_constellation_with_audit_batch(
    store: &RocksDbTeleologicalStore,
    centroid: &ConstellationCentroid,
    audit: &DjAuditRecord,
) -> Result<(), DjError> {
    // implementation
}

pub fn get_constellation(
    store: &RocksDbTeleologicalStore,
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
) -> Result<Option<ConstellationCentroid>, DjError> {
    // implementation
}

pub fn list_constellations(
    store: &RocksDbTeleologicalStore,
    filter: ConstellationFilter,
    limit: usize,
    offset: usize,
) -> Result<Vec<ConstellationCentroid>, DjError> {
    // prefix scan
}

pub struct ConstellationFilter {
    pub domain_uuid: Option<Uuid>,
    pub subject_id: Option<String>,
    pub modality_id: Option<u32>,
}
```

### 1.6 `crates/context-graph-storage/src/dynamicjepa/threshold_store.rs` (NEW)

Skeleton (full implementation in PR 8):

```rust
use crate::dynamicjepa::{column_families::*, keys::*};
use context_graph_core::dynamicjepa::ThresholdCalibration;

pub fn put_threshold_calibration_with_audit_batch(
    store: &RocksDbTeleologicalStore,
    calibration: &ThresholdCalibration,
    audit: &DjAuditRecord,
) -> Result<(), DjError> {
    // implementation
}

pub fn get_threshold_calibration(
    store: &RocksDbTeleologicalStore,
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
) -> Result<Option<ThresholdCalibration>, DjError> {
    // returns the row with the highest supersede_seq for the given prefix
}

pub fn list_threshold_calibrations(
    store: &RocksDbTeleologicalStore,
    filter: ThresholdCalibrationFilter,
) -> Result<Vec<ThresholdCalibration>, DjError> {
    // prefix scan; returns all rows ordered by supersede_seq
}

pub fn next_supersede_seq(
    store: &RocksDbTeleologicalStore,
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
) -> Result<u32, DjError> {
    // computes max(supersede_seq) + 1 by prefix scan
}
```

### 1.7 `crates/context-graph-storage/src/dynamicjepa/inspection.rs`

Add three decoder branches to the existing `inspect_cf` and
`inspect_cf_key` dispatch:

```rust
match cf_name {
    // ... existing 25 branches ...
    CF_DJ_PAIRWISE_READINGS => {
        // decode 24-byte key + JSON value into PairwiseReading
    }
    CF_DJ_CONSTELLATIONS => {
        // decode 36-byte key + JSON value into ConstellationCentroid
    }
    CF_DJ_THRESHOLD_CALIBRATIONS => {
        // decode 40-byte key + JSON value into ThresholdCalibration
    }
    _ => DjError::UnknownColumnFamily(cf_name.to_string()),
}
```

Test:

- `test_inspect_cf_dispatches_pairwise_readings`
- `test_inspect_cf_dispatches_constellations`
- `test_inspect_cf_dispatches_threshold_calibrations`

## 2. Files touched (Phase 1, PR 2 — Core records)

### 2.1 `crates/context-graph-core/src/dynamicjepa/records.rs`

Add three new record structs (full body in
`22_core_records_implementation.md`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PairwiseReading {
    pub pairwise_id: PairwiseReadingId,
    pub event_id: EventId,
    pub instrument_j: InstrumentId,
    pub instrument_k: InstrumentId,
    pub instrument_j_artifact_hash: [u8; 32],
    pub instrument_k_artifact_hash: [u8; 32],
    pub kinds_emitted: PairKindBitset,
    pub cosine_agreement: f32,
    pub rank_disagreement: Option<f32>,
    pub modality_contradiction: Option<bool>,
    pub sparse_dense_mismatch: Option<f32>,
    pub temporal_surprise: Option<f32>,
    pub causal_direction_disagreement: Option<bool>,
    pub safety_proximity: Option<f32>,
    pub created_at_unix_ms: u64,
    pub validation_status: ReadingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ConstellationCentroid {
    pub constellation_id: ConstellationId,
    pub domain_pack_id: DomainPackId,
    pub subject_id: String,
    pub modality_id: InstrumentId,
    pub centroid: Vec<f32>,
    pub instrument_artifact_hash: [u8; 32],
    pub reference_set_count: u32,
    pub kept_count: u32,
    pub dropped_zero_norm: u32,
    pub loo_stability: f32,
    pub calibration_percentile: u8,
    pub calibration_set_size: u32,
    pub built_at_unix_ms: u64,
    pub built_by_run_id: Option<RunId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ThresholdCalibration {
    pub calibration_id: ThresholdCalibrationId,
    pub domain_pack_id: DomainPackId,
    pub subject_id: String,
    pub modality_id: InstrumentId,
    pub tau: f32,
    pub percentile: u8,
    pub calibration_set_count: u32,
    pub calibration_event_uuids_sample: Vec<EventId>,
    pub calibration_set_disjoint_proof: bool,
    pub calibration_min: f32,
    pub calibration_max: f32,
    pub calibration_p10: f32,
    pub supersede_seq: u32,
    pub supersedes_uuid: Option<ThresholdCalibrationId>,
    pub reason: Option<String>,
    pub calibrated_at_unix_ms: u64,
}
```

### 2.2 `crates/context-graph-core/src/dynamicjepa/ids.rs`

Add three new ID newtypes following the existing
`AuditId(Uuid)`-style pattern:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PairwiseReadingId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConstellationId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThresholdCalibrationId(pub Uuid);
```

### 2.3 `crates/context-graph-core/src/dynamicjepa/pair_kinds.rs` (NEW)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct PairKindBitset(pub u8);

impl PairKindBitset {
    pub const COSINE_AGREEMENT: u8         = 0b0000_0001;
    pub const RANK_DISAGREEMENT: u8        = 0b0000_0010;
    pub const MODALITY_CONTRADICTION: u8   = 0b0000_0100;
    pub const SPARSE_DENSE_MISMATCH: u8    = 0b0000_1000;
    pub const TEMPORAL_SURPRISE: u8        = 0b0001_0000;
    pub const CAUSAL_DIRECTION: u8         = 0b0010_0000;
    pub const SAFETY_PROXIMITY: u8         = 0b0100_0000;

    pub fn has(&self, kind: u8) -> bool { (self.0 & kind) != 0 }
    pub fn set(&mut self, kind: u8) { self.0 |= kind; }
    pub fn clear(&mut self, kind: u8) { self.0 &= !kind; }
}
```

### 2.4 `crates/context-graph-core/src/dynamicjepa/audit_log.rs`

Extend the existing `DjAuditRecord` struct with one new field:

```rust
pub struct DjAuditRecord {
    // existing fields ...
    pub signal_yield: u32,
}

impl Default for DjAuditRecord {
    fn default() -> Self {
        Self {
            // existing defaults ...
            signal_yield: 0,
        }
    }
}
```

The existing serde derive handles the v1↔v2 case via `#[serde(default)]`
on `signal_yield`.

### 2.5 `crates/context-graph-core/src/dynamicjepa/audit_log.rs` (cont.)

Add the signal-yield lookup table:

```rust
pub fn signal_yield_for_operation(
    operation_kind: &str,
    domain: &DomainPackInfo,
) -> u32 {
    match operation_kind {
        "ingest_event" => 1,
        "run_adapter_success" => 1,
        "run_adapter_failure" => 0,
        "materialize_panel" => domain.n_panel_slots as u32,
        "materialize_pairwise" => {
            let n = domain.n_panel_slots as u32;
            n * (n - 1) / 2
        },
        "compile_transition" => 2,
        "train_predictor" => 0,
        "predict" => domain.n_panel_slots as u32 + domain.n_active_modalities as u32,
        "plan" => {
            let n = domain.n_panel_slots as u32;
            let m = domain.n_active_modalities as u32;
            domain.k_candidates as u32 * (n + m + 1)
        },
        "record_surprise" => 1,
        "verify_artifact_files" => 0,
        "build_constellation" => 0,
        "calibrate_threshold" => 0,
        "audit_pairwise_mi" => 0,
        // unknown kinds: 0; FSV-12 catches the discrepancy
        _ => 0,
    }
}
```

### 2.6 `crates/context-graph-core/src/dynamicjepa/types.rs`

Add helper struct:

```rust
pub struct DomainPackInfo {
    pub n_panel_slots: usize,
    pub n_active_modalities: usize,
    pub k_candidates: usize,
}
```

### 2.7 `crates/context-graph-core/src/dynamicjepa/mod.rs`

Re-export the new types:

```rust
pub use ids::{PairwiseReadingId, ConstellationId, ThresholdCalibrationId};
pub use pair_kinds::PairKindBitset;
pub use records::{PairwiseReading, ConstellationCentroid, ThresholdCalibration};
pub use audit_log::signal_yield_for_operation;
pub use types::DomainPackInfo;
```

### 2.8 `crates/context-graph-core/src/dynamicjepa/lib.rs`

Add module declarations.

## 3. Files touched (Phase 2, PR 4 — Pairwise compute-on-write)

### 3.1 `crates/context-graph-storage/src/dynamicjepa/pairwise_store.rs` (full body)

The full implementation, building on the skeleton from PR 1:

```rust
pub fn put_panel_with_readings_and_pairwise_batch(
    store: &RocksDbTeleologicalStore,
    panel: LatentPanel,
    readings: Vec<InstrumentReading>,
    pairwise_readings: Vec<PairwiseReading>,
    audit: DjAuditRecord,
) -> Result<(), DjError> {
    let mut batch = WriteBatch::default();

    // Write panel
    let panel_key = panel.panel_id.0.as_bytes();
    let panel_value = serde_json::to_vec(&panel)
        .map_err(DjError::SerializationFailed)?;
    batch.put_cf(
        &store.cf_handle(CF_DJ_LATENT_PANELS)?,
        panel_key,
        &panel_value,
    );

    // Write readings
    for reading in &readings {
        let key = instrument_reading_key(&reading.event_id.0, reading.instrument_id.0);
        let value = serde_json::to_vec(reading)?;
        batch.put_cf(
            &store.cf_handle(CF_DJ_INSTRUMENT_READINGS)?,
            key,
            &value,
        );
    }

    // Write pairwise readings (new)
    for pair in &pairwise_readings {
        let key = pairwise_reading_key(
            &pair.event_id.0,
            pair.instrument_j.0,
            pair.instrument_k.0,
        );
        let value = serde_json::to_vec(pair)?;
        batch.put_cf(
            &store.cf_handle(CF_DJ_PAIRWISE_READINGS)?,
            key,
            &value,
        );
    }

    // Write audit
    batch.put_cf(
        &store.cf_handle(CF_DJ_AUDIT_LOG)?,
        audit_log_key(&audit),
        &serde_json::to_vec(&audit)?,
    );

    // Atomic commit
    store.db().write(batch).map_err(DjError::WriteBatchFailed)?;

    Ok(())
}
```

### 3.2 `crates/context-graph-storage/src/dynamicjepa/pair_compute.rs` (NEW)

Computes the 7 pair-kinds:

```rust
pub fn compute_pairwise_kinds(
    j: &InstrumentReading,
    k: &InstrumentReading,
    j_pair_kinds: &[String],
    k_pair_kinds: &[String],
) -> PairwiseReading {
    let mut bitset = PairKindBitset(0);

    // Cosine is always emitted
    let cosine = cosine_similarity(&j.vector, &k.vector);
    bitset.set(PairKindBitset::COSINE_AGREEMENT);

    let rank_disagreement = if j_pair_kinds.contains(&"rank_disagreement".to_string())
        && k_pair_kinds.contains(&"rank_disagreement".to_string())
    {
        bitset.set(PairKindBitset::RANK_DISAGREEMENT);
        Some(rank_disagreement_score(j, k))
    } else {
        None
    };

    // ... etc for other 5 kinds ...

    PairwiseReading {
        pairwise_id: PairwiseReadingId(Uuid::new_v4()),
        event_id: j.event_id,
        instrument_j: j.instrument_id,
        instrument_k: k.instrument_id,
        instrument_j_artifact_hash: j.artifact_hash,
        instrument_k_artifact_hash: k.artifact_hash,
        kinds_emitted: bitset,
        cosine_agreement: cosine,
        rank_disagreement,
        modality_contradiction: None,    // populated when both declare it
        sparse_dense_mismatch: None,
        temporal_surprise: None,
        causal_direction_disagreement: None,
        safety_proximity: None,
        created_at_unix_ms: now_unix_ms(),
        validation_status: ReadingStatus::Valid,
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-6 || nb < 1e-6 {
        0.0
    } else {
        dot / (na * nb)
    }
}
```

## 4. Files touched (Phase 3 — Constellation + Threshold storage write paths)

PR 7 (build-constellation) and PR 8 (calibrate-threshold) populate
the full bodies of `constellation_store.rs` and `threshold_store.rs`
created in PR 1. The skeletons from §1.5 and §1.6 above are the
contract; PRs 7 and 8 fill in the bodies and add the write-path
construction logic specified in `04_constellation_design.md` §3 and
§6.

## 5. Test additions (per PR)

Per `14_test_plan.md`:

- PR 1 adds 4 tests for key formats.
- PR 2 adds 12 tests for typed records (round-trip serde, Validate
  derive, default values).
- PR 4 adds 12 tests for `pairwise_store` and 8 for `pair_compute`.
- PR 7 adds 11 tests for `constellation_store`.
- PR 8 adds 8 tests for `threshold_store`.
- PR 11 (FSV) adds integration tests for FSV-10 / 11.

All tests follow the existing v1 `#[cfg(test)] mod tests` pattern
within the same source file.

## 6. RocksDB block options

The three new CFs reuse existing helper functions:

- `dj_uuid_point_cf_options(cache)` — for `dj_pairwise_readings`
  (16-byte event-uuid prefix; bloom filter on; LZ4 compression).
- `dj_composite_index_cf_options(cache)` — for `dj_constellations`
  and `dj_threshold_calibrations` (16-byte domain-uuid prefix;
  bloom filter on).

No new helper function is needed; the existing helpers are
appropriate.

## 7. Migration semantics

**Per CLAUDE.md, no migration shims.** A v1 RocksDB store has 25
CFs. Opening it with a v2 binary fails closed at the manifest
pre-check (which runs *before* RocksDB) — the v2 binary never gets
to discover the missing 3 CFs. A v2 RocksDB has 28 CFs from
inception.

There is no merge / migration / upgrade-in-place path; v2 bundles
are produced fresh.

## 8. Schema-version pre-check (PR 1, in CLI dispatcher)

Code added to `crates/context-graph-cli/src/main.rs`:

```rust
fn validate_bundle_schema_version(
    bundle_root: &Path,
    expected_schema_version: u32,
) -> Result<(), CliError> {
    let manifest_path = bundle_root.join("run_manifest.json");
    if !manifest_path.exists() {
        return Ok(()); // greenfield bundle; v2 will create it
    }

    let manifest: serde_json::Value = serde_json::from_reader(
        File::open(&manifest_path)?
    )?;

    let bundle_schema_version = manifest
        .get("bundle_schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32; // v1 manifests don't have this field

    if bundle_schema_version != expected_schema_version {
        return Err(CliError::BundleSchemaVersionMismatch {
            expected: expected_schema_version,
            found: bundle_schema_version,
            bundle_root: bundle_root.to_path_buf(),
        });
    }

    Ok(())
}
```

This pre-check is invoked for every dispatched subcommand that
operates on a bundle (everything except `register-domain` and a few
low-level ops).

## 9. PR 1 → PR 2 → PR 4 dependency

PR 1 lands without write paths; the new CFs exist but are empty.
PR 2 lands typed records and the audit-log signal-yield extension.
PR 4 connects the two by populating the new CFs from the panel
materialisation entrypoint.

After PR 4 lands, a research-smoke v2 invocation populates
`dj_pairwise_readings` correctly; subsequent PRs build on top.
