# 05 — Storage Schema (v1)

The 5090 demo persists everything to the existing physical RocksDB database opened by `RocksDbTeleologicalStore`. New CFs are *additive* — the existing 73 stay untouched. Migration policy: **wipe the DB and rebuild.** Do not write a migrator.

This file specifies CFs, key formats, batch boundaries, and the inspection API. Implementation lives under `crates/context-graph-storage/src/dynamicjepa/`.

Current implementation status as of the Phase 10 review on 2026-04-30: all 25 CFs and the inspection API are implemented; Phase 4 atomic helpers are implemented for raw event + started adapter run, adapter success/failure terminalization, and panel + readings writes. Phase 5 atomic helpers are implemented for binding + secondary index + audit writes and trajectory + audit writes. Phase 6 adds dataset shard + audit atomic writes. Phase 7 adds training-run start/fail/complete and model-artifact completion batches with audit rows. Phase 8 adds prediction + audit, plan batch, and surprise + audit write paths. Phase 10 adds verification-run + audit atomic writes through `put_verification_run_with_audit_batch`.

## 1. Module Layout

```
crates/context-graph-storage/src/dynamicjepa/
  mod.rs                      # public facade, opens CFs, hands out CF handles
  column_families.rs          # all CF name constants + DJ_CF_COUNT
  error.rs                    # storage-side errors that map to DynamicJepaError
  keys.rs                     # all key encoders (UUID prefix, name/version, composite)
  encode.rs                   # versioned bincode helpers (uses existing `versioned_bincode.rs` pattern)
  domain_pack_store.rs
  event_store.rs              # raw events, normalized state/action/outcome/transition
  panel_store.rs              # readings + panels
  binding_store.rs            # primary + by_entity index
  trajectory_store.rs
  dataset_store.rs            # shards
  artifact_store.rs           # training runs + artifacts
  prediction_store.rs
  planning_store.rs           # plan traces + guard decisions + skills
  surprise_store.rs
  verification_store.rs       # verification runs + audit
  inspection.rs               # count_*, list_*, get_*, inspect_cf for FSV/CLI
  tests.rs
```

## 2. New CF Constants

Add 25 CFs (vs the existing 73). Total after demo = 98.

```rust
// crates/context-graph-storage/src/dynamicjepa/column_families.rs

pub const CF_DJ_DOMAIN_PACKS:                 &str = "dj_domain_packs";
pub const CF_DJ_DOMAIN_PACK_BY_NAME_VERSION:  &str = "dj_domain_pack_by_nv";
pub const CF_DJ_INSTRUMENT_REGISTRY:          &str = "dj_instrument_registry";
pub const CF_DJ_ADAPTER_REGISTRY:             &str = "dj_adapter_registry";

pub const CF_DJ_RAW_EVENTS:                   &str = "dj_raw_events";
pub const CF_DJ_NORMALIZED_STATES:            &str = "dj_normalized_states";
pub const CF_DJ_ACTIONS:                      &str = "dj_actions";
pub const CF_DJ_OUTCOMES:                     &str = "dj_outcomes";
pub const CF_DJ_TRANSITIONS:                  &str = "dj_transitions";
pub const CF_DJ_ADAPTER_RUNS:                 &str = "dj_adapter_runs";

pub const CF_DJ_INSTRUMENT_READINGS:          &str = "dj_instrument_readings";
pub const CF_DJ_LATENT_PANELS:                &str = "dj_latent_panels";

pub const CF_DJ_BINDINGS:                     &str = "dj_bindings";
pub const CF_DJ_BINDINGS_BY_ENTITY:           &str = "dj_bindings_by_entity";

pub const CF_DJ_TRAJECTORIES:                 &str = "dj_trajectories";
pub const CF_DJ_DATASET_SHARDS:               &str = "dj_dataset_shards";

pub const CF_DJ_TRAINING_RUNS:                &str = "dj_training_runs";
pub const CF_DJ_MODEL_ARTIFACTS:              &str = "dj_model_artifacts";

pub const CF_DJ_PREDICTIONS:                  &str = "dj_predictions";
pub const CF_DJ_SKILL_POLICIES:               &str = "dj_skill_policies";
pub const CF_DJ_PLAN_TRACES:                  &str = "dj_plan_traces";
pub const CF_DJ_GUARD_DECISIONS:              &str = "dj_guard_decisions";
pub const CF_DJ_SURPRISE_EVENTS:              &str = "dj_surprise_events";

pub const CF_DJ_VERIFICATION_RUNS:            &str = "dj_verification_runs";
pub const CF_DJ_AUDIT_LOG:                    &str = "dj_audit_log";

pub const DJ_CF_NAMES: &[&str] = &[
    CF_DJ_DOMAIN_PACKS, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION,
    CF_DJ_INSTRUMENT_REGISTRY, CF_DJ_ADAPTER_REGISTRY,
    CF_DJ_RAW_EVENTS, CF_DJ_NORMALIZED_STATES, CF_DJ_ACTIONS, CF_DJ_OUTCOMES,
    CF_DJ_TRANSITIONS, CF_DJ_ADAPTER_RUNS,
    CF_DJ_INSTRUMENT_READINGS, CF_DJ_LATENT_PANELS,
    CF_DJ_BINDINGS, CF_DJ_BINDINGS_BY_ENTITY,
    CF_DJ_TRAJECTORIES, CF_DJ_DATASET_SHARDS,
    CF_DJ_TRAINING_RUNS, CF_DJ_MODEL_ARTIFACTS,
    CF_DJ_PREDICTIONS, CF_DJ_SKILL_POLICIES, CF_DJ_PLAN_TRACES,
    CF_DJ_GUARD_DECISIONS, CF_DJ_SURPRISE_EVENTS,
    CF_DJ_VERIFICATION_RUNS, CF_DJ_AUDIT_LOG,
];
pub const DJ_CF_COUNT: usize = DJ_CF_NAMES.len();   // = 25
```

A test in `tests.rs` asserts `DJ_CF_COUNT == 25` and that `DJ_CF_NAMES.iter().all_unique()`. Bumping the count requires updating both the slice and the assertion intentionally.

## 3. Key Formats

| CF | Key | Bytes | Notes |
| --- | --- | --- | --- |
| `dj_domain_packs` | `[domain_pack_uuid:16]` | 16 | derived deterministically from `(id, version)` via UUIDv5 namespace; lookup by id reads `dj_domain_pack_by_nv` first then this |
| `dj_domain_pack_by_nv` | `[name_blake3_128:16][version_blake3_128:16]` | 32 | value = `domain_pack_uuid:16` |
| `dj_instrument_registry` | `[domain_pack_uuid:16][instrument_blake3_128:16]` | 32 | |
| `dj_adapter_registry` | `[domain_pack_uuid:16][adapter_blake3_128:16]` | 32 | |
| `dj_raw_events` | `[event_uuid:16]` | 16 | |
| `dj_normalized_states` | `[state_uuid:16]` | 16 | |
| `dj_actions` | `[action_uuid:16]` | 16 | |
| `dj_outcomes` | `[outcome_uuid:16]` | 16 | |
| `dj_transitions` | `[transition_uuid:16]` | 16 | |
| `dj_adapter_runs` | `[adapter_run_uuid:16]` | 16 | |
| `dj_instrument_readings` | `[event_uuid:16][instrument_blake3_128:16]` | 32 | prefix scan by event |
| `dj_latent_panels` | `[panel_uuid:16]` | 16 | |
| `dj_bindings` | `[binding_uuid:16]` | 16 | |
| `dj_bindings_by_entity` | `[entity_blake3_128:16][binding_uuid:16]` | 32 | empty value |
| `dj_trajectories` | `[trajectory_uuid:16]` | 16 | |
| `dj_dataset_shards` | `[dataset_uuid:16][shard_uuid:16]` | 32 | |
| `dj_training_runs` | `[training_run_uuid:16]` | 16 | |
| `dj_model_artifacts` | `[artifact_uuid:16]` | 16 | |
| `dj_predictions` | `[prediction_uuid:16]` | 16 | |
| `dj_skill_policies` | `[skill_uuid:16]` | 16 | |
| `dj_plan_traces` | `[plan_trace_uuid:16]` | 16 | |
| `dj_guard_decisions` | `[guard_decision_uuid:16]` | 16 | |
| `dj_surprise_events` | `[surprise_uuid:16]` | 16 | |
| `dj_verification_runs` | `[verification_run_uuid:16]` | 16 | |
| `dj_audit_log` | `[timestamp_nanos_BE:8][audit_uuid:16]` | 24 | append-only; big-endian timestamp prefix for time scans |

Hashes are BLAKE3 truncated to 16 bytes (`blake3::Hasher::finalize()` then take first 16 bytes). All key encoders live in `keys.rs` with byte‑exact round‑trip tests.

## 4. Value Format

```
[record_version: u8][bincode payload]
```

- `record_version` is a per‑record‑type constant in `crates/context-graph-core/src/dynamicjepa/`.
- Empty payload, wrong version, bincode failure, or hash mismatch → `DynamicJepaError::Codec`.
- Use the existing `crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs` helper. Do not invent a new framing.

## 5. CF Options

| Profile | CFs | Settings |
| --- | --- | --- |
| `dj_uuid_point` | all `[uuid:16]` keys | LZ4, bloom, shared cache, 16‑byte prefix extractor, 4 MB write buffer |
| `dj_composite_index` | `[uuid:16][hash:16]` keys (instrument readings, dataset shards, registries) | LZ4, bloom, 16‑byte prefix extractor (the *first* 16 bytes — supports prefix scans by domain pack/event) |
| `dj_secondary_index` | `dj_bindings_by_entity` (empty values) | LZ4, no bloom, 16‑byte prefix extractor, 1 MB write buffer |
| `dj_append_only` | `dj_audit_log`, `dj_training_runs`, `dj_predictions`, `dj_plan_traces`, `dj_surprise_events`, `dj_verification_runs` | LZ4, bloom, 8‑byte prefix on audit only, 4 MB write buffer; never delete except on full DB wipe |
| `dj_large_blob` | `dj_latent_panels`, `dj_dataset_shards` | LZ4, 64 KB blocks, 8 MB write buffer (panels can reach ~10 KB; dataset shard rows reference panel ids, not inline panels — but shard summaries can grow) |

CF descriptors are produced by a single helper `dj_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor>` that the existing DB open path appends to the descriptor list before calling `DB::open_cf_descriptors`.

## 6. Atomic Write Batches

Required batch boundaries (every operation listed below must be one `WriteBatch`):

1. **Register domain pack**: primary `dj_domain_packs` row + `dj_domain_pack_by_nv` row + N `dj_instrument_registry` rows + M `dj_adapter_registry` rows + 1 `dj_audit_log` row.
2. **Ingest raw event**: 1 `dj_raw_events` row + 1 `dj_adapter_runs` row with status=`started` + 1 audit row. Implemented by `put_raw_event_and_adapter_run_started`.
3. **Successful adapter output**: update `dj_adapter_runs` row to `completed` + 1 each of `dj_normalized_states`, `dj_actions`, `dj_outcomes`, `dj_transitions` + 1 audit row. Implemented by `put_adapter_run_success_batch`.
4. **Failed adapter output (raw event already persisted)**: update `dj_adapter_runs` to `failed` + 1 audit row. **No** normalized rows. Implemented by `put_adapter_run_failure_batch`.
5. **Materialize panel**: K `dj_instrument_readings` rows + 1 `dj_latent_panels` row + 1 audit row. Implemented by `put_panel_with_readings_batch`.
6. **Write binding**: 1 `dj_bindings` row + 1 `dj_bindings_by_entity` row + 1 audit row.
7. **Compile trajectory**: 1 `dj_trajectories` row + 1 audit row.
8. **Compile dataset shard**: 1 `dj_dataset_shards` row + 1 audit row. (Shard references panel ids; panels themselves are unchanged.)
9. **Start training run**: 1 `dj_training_runs` row with status=`started` + 1 audit row.
10. **Register artifact**: 1 `dj_model_artifacts` row + update `dj_training_runs` row to `completed` with new `artifact_ids` + 1 audit row. (Files must already be on disk and hashes computed before this batch fires.)
11. **Run prediction**: 1 `dj_predictions` row + 1 audit row.
12. **Plan run**: K hypothetical `dj_actions` rows (with `action_origin = Hypothetical`) + K `dj_predictions` rows + K `dj_guard_decisions` rows + 1 `dj_plan_traces` row + 1 audit row. **All in one batch.** Partial plans are not allowed.
13. **Surprise event**: 1 `dj_surprise_events` row + 1 audit row.
14. **Verification run**: 1 `dj_verification_runs` row + 1 audit row. Implemented by `put_verification_run_with_audit_batch`.

Pre‑persistence schema validation failure (for example a malformed domain TOML before any operation starts) writes **no** rows — neither to data CFs nor to audit. Once a CLI operation starts and can name a source-of-truth operation, failures are audit-visible: Phase 4 empty JSONL input writes one failed `dj_audit_log` row and no raw/adapter/normalized rows.

## 7. Inspection API (`inspection.rs`)

Required for FSV. Every CF must have:

```rust
pub fn count_<cf>(db: &Database) -> StorageResult<u64>;
pub fn list_<cf>(db: &Database, limit: usize, offset: usize) -> StorageResult<Vec<<RecordType>>>;
pub fn get_<cf>(db: &Database, id: &<IdType>) -> StorageResult<Option<<RecordType>>>;
```

Plus a generic:

```rust
pub fn inspect_cf(db: &Database, cf_name: &str, limit: usize) -> StorageResult<Vec<(Vec<u8>, serde_json::Value)>>;
pub fn inspect_cf_key(db: &Database, cf_name: &str, key: &[u8]) -> StorageResult<Option<serde_json::Value>>;
```

`inspect_cf` returns the first `limit` (key, decoded JSON) pairs. `inspect_cf_key` decodes one exact RocksDB key and is exposed as `dynamicjepa inspect-cf --key-hex` for FSV row proofs. CLI commands `dynamicjepa inspect-cf` and `dynamicjepa inspect-counts` call these directly.

## 8. Database Open Path

Modify the existing DB open code (`RocksDbTeleologicalStore::open`) to:

1. Build base + teleological + quantized + code + causal CF descriptors as today.
2. Append `dj_cf_descriptors(&cache)`.
3. Call `DB::open_cf_descriptors_as_secondary` / `open_cf_descriptors` with the union.
4. Validate that `db.cf_handle(name).is_some()` for every entry in `DJ_CF_NAMES`. Missing handles → fail open.

`create_if_missing = true` for the demo (we always run on fresh `/tmp/` DBs). Production: pre‑existing DB without DynamicJEPA CFs needs them auto‑created on open; log every CF created at info level.

## 9. Single‑Writer Guard

The existing `PidFileGuard` writes `<db_path>/mcp.pid` and holds an exclusive flock. Demo CLI commands honor it: pre‑flight in `01_environment_and_hardware.md` §5 already removes any pid file from the demo `/tmp/` paths before launch.

If you ever observe a stale pid on a demo path, the previous demo crashed — **do not** silently delete the lock; treat it as a structured error and require the operator to re‑run pre‑flight explicitly.

## 10. Audit Log Schema

```rust
pub struct DjAuditRecord {
    pub audit_id: Uuid,
    pub timestamp_unix_nanos: u64,
    pub operation: String,                // "register_domain_pack", "materialize_panel", ...
    pub actor: String,                    // "cli" | "mcp" | "fsv"
    pub input_ids: Vec<String>,
    pub output_ids: Vec<String>,
    pub cfs_touched: Vec<String>,
    pub content_hashes: Vec<[u8; 32]>,
    pub status: AuditStatus,              // Ok | Failed { error_code: String }
    pub verification_run_id: Option<Uuid>,
}
```

Append‑only. Never updated. Used by FSV harnesses to reconstruct what happened during a run when something goes sideways.

## 11. Wipe‑And‑Rebuild Migration Policy

When schema changes:

1. Bump `record_version` for the affected record types in core.
2. Old DBs must be wiped — they will fail to open with a typed codec error if any decode is attempted.
3. There is **no** migration code. Operator wipes `/tmp/5090jepa-*` dirs (pre‑flight already does this) and re‑runs `verify-*`.
4. The operator's real Context Graph DB is *not* affected because DynamicJEPA CFs are missing there until/unless deployed.

This is the policy the user has explicitly requested ("ABSOLUTELY NO BACKWARDS COMPATIBILITY").

## 12. FSV Hooks

`storage/dynamicjepa/inspection.rs` exposes:

```rust
pub fn snapshot_dj_counts(db: &Database) -> StorageResult<BTreeMap<String, u64>>;
```

Returns one entry per CF in `DJ_CF_NAMES`. CLI `dynamicjepa inspect-counts --json` uses it. Verification harnesses call it before and after every step and embed the diff into the `VerificationRunRecord.before_counts` / `after_counts` fields.

If a CF is added later (`DJ_CF_COUNT` bumps), `snapshot_dj_counts` automatically picks it up — no separate registry to maintain.
