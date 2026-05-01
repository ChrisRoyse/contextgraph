# 07 — Storage Schema Changes (RocksDB Column Families)

This document specifies the precise RocksDB-side changes for v2.0:
new column families, key formats, descriptors, write paths, read
paths, and how the schema version is signalled.

## 1. CF inventory after upgrade

| # | CF Name | Status | Purpose |
| ---: | --- | --- | --- |
| 1 | `dj_domain_packs` | unchanged | Domain pack registry by uuid |
| 2 | `dj_domain_pack_by_nv` | unchanged | (name, version) → uuid index |
| 3 | `dj_instrument_registry` | unchanged | Instrument metadata + artifact hash |
| 4 | `dj_adapter_registry` | unchanged | Adapter metadata |
| 5 | `dj_raw_events` | unchanged | Raw ingested event payloads |
| 6 | `dj_normalized_states` | unchanged | Adapter-normalised state rows |
| 7 | `dj_actions` | unchanged | Normalised action rows |
| 8 | `dj_outcomes` | unchanged | Normalised outcome rows |
| 9 | `dj_transitions` | unchanged | $(s_t, a, o, s_{t+1})$ links |
| 10 | `dj_adapter_runs` | unchanged | Adapter execution audit |
| 11 | `dj_instrument_readings` | unchanged | Per-event per-instrument vector |
| 12 | `dj_latent_panels` | extended | Now references pairwise rows |
| 13 | `dj_bindings` | unchanged | |
| 14 | `dj_bindings_by_entity` | unchanged | |
| 15 | `dj_trajectories` | unchanged | |
| 16 | `dj_dataset_shards` | unchanged | |
| 17 | `dj_training_runs` | unchanged | |
| 18 | `dj_model_artifacts` | unchanged | |
| 19 | `dj_predictions` | unchanged | |
| 20 | `dj_skill_policies` | unchanged | |
| 21 | `dj_plan_traces` | extended | Adds `no_accepted_candidate` |
| 22 | `dj_guard_decisions` | extended | Adds $G_\tau$ fields |
| 23 | `dj_surprise_events` | unchanged | |
| 24 | `dj_verification_runs` | unchanged | |
| 25 | `dj_audit_log` | extended | Adds `signal_yield` |
| **26** | **`dj_pairwise_readings`** | **new** | $\binom{N_d}{2}$ pairwise feature rows |
| **27** | **`dj_constellations`** | **new** | Frozen centroids per (domain, subject, modality) |
| **28** | **`dj_threshold_calibrations`** | **new** | Per-modality $\tau_m$ persistence |

Total: 28 CFs (was 25). `DJ_CF_COUNT` becomes 28 after the upgrade.

## 2. New CF specs

### 2.1 `dj_pairwise_readings`

| Field | Spec |
| --- | --- |
| **Key** | `[event_uuid (16 B)][instrument_j_id (4 B LE)][instrument_k_id (4 B LE)]` |
| **Key length** | 24 bytes (fixed) |
| **Prefix** | first 16 bytes (event uuid) → enables prefix-extractor lookup of all pair rows for an event |
| **Value** | JSON-serialised `PairwiseReading` (see `05_pairwise_features_design.md` §2) |
| **Bloom filter** | yes (point lookups dominate) |
| **CF Options** | based on existing `dj_uuid_point_cf_options(cache)` with prefix length 16 |

Read patterns:
- All pair rows for an event: prefix scan with the event uuid as
  prefix. Cost: O(N²) reads per event.
- A specific (event, j, k) row: point lookup with full 24-byte key.
- Bulk count: `inspect-counts` reads CF `dj_pairwise_readings`.

Write pattern: batch write of $\binom{N_d}{2}$ rows per event in the
same WriteBatch as the panel; transactional with the panel write.

### 2.2 `dj_constellations`

| Field | Spec |
| --- | --- |
| **Key** | `[domain_pack_uuid (16 B)][subject_id_hash (16 B)][modality_id (4 B LE)]` |
| **Key length** | 36 bytes (fixed) |
| **Prefix** | first 32 bytes (domain + subject) |
| **Value** | JSON-serialised `ConstellationCentroid` (see `04_constellation_design.md` §3) |
| **Bloom filter** | yes |
| **CF Options** | based on existing `dj_composite_index_cf_options(cache)` |

`subject_id_hash` is `sha256(subject_id_string)[..16]` so subject ids
are stable-length keys without storing the full string per row.

Read patterns:
- All constellations for (domain, subject): prefix scan with 32-byte
  prefix.
- A specific modality's centroid: point lookup with full 36-byte key.
- All constellations for domain: prefix scan with 16-byte prefix.

Cardinality on a typical career bundle: 5 constellations × 10
modalities = 50 rows. Storage: ~40 KB (each row ~800 B with fp32
centroid of typical $d_m \le 64$ + JSON overhead). Negligible.

### 2.3 `dj_threshold_calibrations`

| Field | Spec |
| --- | --- |
| **Key** | `[domain_pack_uuid (16 B)][subject_id_hash (16 B)][modality_id (4 B LE)][supersede_seq (4 B LE)]` |
| **Key length** | 40 bytes (fixed) |
| **Prefix** | first 36 bytes (domain + subject + modality) |
| **Value** | JSON-serialised `ThresholdCalibration` |
| **Bloom filter** | yes |
| **CF Options** | based on existing `dj_composite_index_cf_options(cache)` |

`supersede_seq` increments on each `re-calibrate-threshold`. The
"current" row is the highest `supersede_seq` for the prefix; older
rows are kept for audit (per CLAUDE.md: do not delete history).

Read patterns:
- "Current" calibration for (domain, subject, modality): prefix scan
  with 36-byte prefix, take the highest `supersede_seq`.
- All calibration history for a modality: prefix scan with 36-byte
  prefix; ordered by sequence number due to LE encoding.

Cardinality on a typical career bundle: 50 rows initially (one per
constellation-modality pair); grows on re-calibration.

## 3. Extended CF specs

### 3.1 `dj_guard_decisions` (extended)

The `Value` JSON gains the fields specified in
`06_guard_upgrade_g_tau.md` §2. Reading a v1 row in a v2 binary fails
closed (typed `BUNDLE_SCHEMA_VERSION_MISMATCH` from the bundle's
`run_manifest.json` rather than mid-stream JSON parse error).

The CF descriptor itself (block options, prefix length) is unchanged:
the row is still keyed by `guard_decision_id` uuid, append-only.

### 3.2 `dj_audit_log` (extended)

The `DjAuditRecord` value JSON gains:

```rust
pub struct DjAuditRecord {
    // existing fields ...
    pub signal_yield: u32,            // new in v2
}
```

The signal-yield value for each `operation_kind` is a fixed-table
lookup (specified in `03_signal_yield_math.md` §7); the audit record
writer fills it in at write time. CF descriptor unchanged.

### 3.3 `dj_latent_panels` (extended)

The `LatentPanel` value JSON gains:

```rust
pub struct LatentPanel {
    // existing fields ...
    pub pairwise_reading_ids: Vec<PairwiseReadingId>,  // new in v2; len = C(N_d, 2)
}
```

This makes the panel ↔ pairwise relationship discoverable from the
panel side alone, which is convenient for FSV-9 and for the
predictor's optional pairwise-features input mode.

### 3.4 `dj_plan_traces` (extended)

```rust
pub struct PlanTrace {
    // existing fields ...
    pub no_accepted_candidate: bool,  // new in v2
    pub constellation_uuid_used: Option<ConstellationId>,
}
```

When `no_accepted_candidate = true`, downstream consumers
(`compute-mc-ratio`, paper-table generators) treat the row as an
operational signal but not a guard pass; the corresponding
`dj_audit_log` row of kind `plan_no_accepted_candidate` is what counts
in the bundle-level "guard rejection" rate.

## 4. RocksDB descriptor wiring

`crates/context-graph-storage/src/dynamicjepa/column_families.rs`
gains:

```rust
pub const CF_DJ_PAIRWISE_READINGS: &str        = "dj_pairwise_readings";
pub const CF_DJ_CONSTELLATIONS: &str           = "dj_constellations";
pub const CF_DJ_THRESHOLD_CALIBRATIONS: &str   = "dj_threshold_calibrations";

pub const DJ_CF_NAMES: &[&str] = &[
    // ... existing 25 ...
    CF_DJ_PAIRWISE_READINGS,
    CF_DJ_CONSTELLATIONS,
    CF_DJ_THRESHOLD_CALIBRATIONS,
];

pub const DJ_CF_COUNT: usize = DJ_CF_NAMES.len();  // 28

pub fn dj_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    // ... existing 25 ...
    descriptors.push(ColumnFamilyDescriptor::new(
        CF_DJ_PAIRWISE_READINGS,
        dj_uuid_point_cf_options(cache),  // 16-byte prefix
    ));
    descriptors.push(ColumnFamilyDescriptor::new(
        CF_DJ_CONSTELLATIONS,
        dj_composite_index_cf_options(cache),  // 16-byte prefix
    ));
    descriptors.push(ColumnFamilyDescriptor::new(
        CF_DJ_THRESHOLD_CALIBRATIONS,
        dj_composite_index_cf_options(cache),  // 16-byte prefix
    ));
    descriptors
}
```

Per memory lesson `RocksDB requires ALL on-disk CFs to be opened — delete
old DBs when removing CFs`, the v2 binary's first task on startup with a
v1 bundle is to detect the schema mismatch *before* opening RocksDB
(via the `run_manifest.json` `bundle_schema_version` field) and refuse
the open with a typed `BUNDLE_SCHEMA_VERSION_MISMATCH` error. There is
no migration path; the v1 bundle remains v1, and a fresh v2 bundle is
generated by the v2 binary on the same source data (per CLAUDE.md no-
backwards-compat rule).

## 5. Bundle JSON manifest schema bump

`run_manifest.json` and `aggregate_manifest.json` gain
`bundle_schema_version: 2`. Aggregator readers refuse to mix v1 and
v2 bundles in the same `multiseed-aggregate` invocation; FSV check
`bundle_schema_consistency` is added to the FSV-12 harness.

The release reference (`reference/reference_release_v2.0.0.json`) is
versioned alongside the bundle schema; the v1 reference
(`reference_release_v1.0.0.json`) is preserved for v1 binaries.

## 6. Estimated disk footprint per career bundle

| CF | v1 rows | v2 rows | v1 bytes (approx) | v2 bytes (approx) |
| --- | ---: | ---: | ---: | ---: |
| `dj_raw_events` | 1{,}004 | 1{,}004 | 600 KB | 600 KB |
| `dj_normalized_states` | 1{,}000 | 1{,}000 | 1.2 MB | 1.2 MB |
| `dj_instrument_readings` | 10{,}000 | 10{,}000 | 1.5 MB | 1.5 MB |
| `dj_latent_panels` | 1{,}000 | 1{,}000 | 800 KB | 850 KB (added uuids) |
| `dj_pairwise_readings` | 0 | 45{,}000 | 0 | 4.5 MB (LZ4) |
| `dj_constellations` | 0 | 50 | 0 | 35 KB |
| `dj_threshold_calibrations` | 0 | 50 | 0 | 25 KB |
| `dj_guard_decisions` | 5 | 5 | 4 KB | 6 KB |
| `dj_audit_log` | 3{,}019 | 3{,}019 | 700 KB | 720 KB |
| Other 19 CFs | (unchanged) | (unchanged) | $\sim$8 MB | $\sim$8 MB |
| **Total** | $\sim$19{,}050 | $\sim$64{,}120 | $\sim$13 MB | $\sim$17.5 MB |

Per-bundle disk overhead: $\sim$4.5 MB additional, dominated by
pairwise readings. For a 5-seed release-C aggregate
(5 × 17.5 + aggregate output) the total bundles directory grows from
$\sim$70 MB to $\sim$95 MB, well within the 7.4 GiB existing artifact
archive's headroom.

## 7. Per-bundle file additions

Beyond RocksDB, the bundle adds these JSON / CSV files:

```
bundles/<run-id>/
  paper_tables/
    table_pairwise_mi_summary.csv          new
    table_mc_ratio.csv                     new
    table_constellation_guard.csv          new
    table_threshold_calibration.csv        new
    table_edge_cases.csv                   extended (11 → 26 rows)
  plots/
    pairwise_mi_heatmap.svg                new
    signal_density_per_event_distribution.svg  new
    g_tau_acceptance_curve.svg             new
  fsv_evidence/
    fsv9_pairwise_readings_integrity.json  new
    fsv10_constellation_integrity.json     new
    fsv11_gtau_verifiability.json          new
    fsv12_signal_yield_audit.json          new
  constellations/                          new directory
    <constellation-uuid>.json              one per constellation
  thresholds/                              new directory
    <calibration-uuid>.json                one per threshold row
```

The aggregate bundle adds the multi-seed versions of the four new
paper tables.

## 8. Read-only inspection paths

All three new CFs are inspectable via the existing `inspect-counts`
and `inspect-cf` commands:

```bash
context-graph dynamicjepa inspect-counts --db <path>
# now emits 28 CF counts including the three new ones

context-graph dynamicjepa inspect-cf --db <path> \
  --cf dj_pairwise_readings --limit 5
# decodes 5 PairwiseReading rows

context-graph dynamicjepa inspect-cf --db <path> \
  --cf dj_constellations --limit 50
# decodes all constellation centroids in the bundle
```

The read-only-open hardening from the v1 smoke-B fix
(`create_missing_column_families = false`) carries forward; opening a
v1 RocksDB with a v2 binary fails closed at the manifest-version check
*before* hitting RocksDB.

## 9. Backwards-compat compliance

Per CLAUDE.md "No backwards compatibility required":

- v1 bundles are NOT migrated; they remain readable by v1 binaries.
- v2 binaries refuse to read v1 bundles (typed error).
- v1 binaries refuse to read v2 bundles (typed error from the JSON
  manifest's `bundle_schema_version` field).
- The v1 release artifact (`mejepa_5090_artifact_v1.0.0_rc5_sif_verified`)
  remains valid. The v2 artifact is published as a *new* release
  candidate `mejepa_5090_artifact_v2.0.0_rcX`.
- Generated reproductions of v1 numbers from v1 binaries continue to
  work bit-equivalently with v1 bundles.
- Generated v2 bundles produce v2 numbers (different, larger MC
  ratio; same headline cosines because the predictor is unchanged).

The first-version bump preserves a clean v1 baseline for paper-section
deltas.
