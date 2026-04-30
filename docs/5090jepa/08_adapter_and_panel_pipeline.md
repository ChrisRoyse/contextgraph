# 08 — Adapter & Panel Pipeline

This file specifies how raw JSONL fixture lines become normalized state/action/outcome/transition records, then instrument readings, then a latent panel — for the v1 demo. The pipeline is identical for both domain packs; the only difference is which TOML is registered.

Current Phase 10 status as of 2026-04-30: the adapter, instrument execution, and panel materialization path remains implemented in `crates/context-graph-cli/src/commands/dynamicjepa.rs`, backed by atomic storage helpers in `crates/context-graph-storage/src/dynamicjepa/{event_store.rs,panel_store.rs}`. The Phase 7 service crate owns training/artifacts and Phase 8 inference logic; this adapter/panel code has not been moved into the service crate.

## 1. Pipeline Stages

```
JSONL fixture line
  ─▶ RawDomainEvent              [CF_DJ_RAW_EVENTS]
  ─▶ AdapterRunRecord(started)   [CF_DJ_ADAPTER_RUNS]
  ─▶ adapter parse + map
       ├─ ok ─▶ NormalizedState/Action/Outcome/Transition + AdapterRun(completed)
       └─ err ─▶ AdapterRun(failed)  [no normalized rows]
  ─▶ instrument execution
       ├─ ok ─▶ InstrumentReading × N
       └─ err ─▶ no panel; failure recorded as audit row
  ─▶ LatentPanel                 [CF_DJ_LATENT_PANELS]
```

Every stage persists to its CF before the next stage begins. Failure at any stage halts the operation. The only allowed partially observed adapter state is the audited raw-event + failed-adapter-run pair documented for malformed JSONL rows; normalized rows and panels are never written on adapter failure.

## 2. Adapter: `json_event`

Input: a JSONL file path. Output: per-line `RawDomainEvent` + `AdapterRunRecord`. The implementation lives in `crates/context-graph-cli/src/commands/dynamicjepa.rs`; a reusable service implementation under `crates/context-graph-dynamicjepa/src/adapters/` remains a future refactor and must preserve the verified CLI behavior.

Algorithm:

1. Open the file. If the file is empty (zero bytes), return `DynamicJepaError::AdapterFailed { adapter_id: "json_event", message: "empty input file", field_path: "" }`. Write exactly one failed `dj_audit_log` row and no raw, adapter, normalized, reading, or panel rows.
2. Iterate lines with their byte offsets. For each line:
   1. Compute `payload_hash = sha256(line_bytes)`.
   2. Parse JSON. If parse fails → write the raw event row, then write an `AdapterRunRecord { status: Failed, error_message: "json parse failed: ...", field_path: ... }`. Move to next line.
   3. Apply the declared mapping: every `field_path` in `adapters[json_event].mapping` resolves a JSONPath-like expression. Demo uses `$.a.b.c` only — no filters, no wildcards.
   3a. Reject any extra JSON object key that is not consumed by the registered domain schema/mapping. Unknown fixture fields are adapter failures, not ignored metadata.
   4. Coerce each resolved value to the declared `FieldKind`. Strict: `i64` accepts JSON numbers without fractional part only; `f64` accepts numbers; `categorical` accepts strings present in `variants`; `bool` only accepts `true`/`false`; `string` only accepts strings; `unix_ms` accepts `i64` integers.
   5. Validate `min`/`max` if declared.
   6. If any of (3)–(5) fails → write the raw event row, write `AdapterRunRecord { status: Failed, field_path, expected, actual }`. **No** normalized rows.
   7. On success: build `NormalizedState`, `NormalizedAction`, `NormalizedOutcome` records. Build the `StateTransition` record linking them. Persist all four + update the AdapterRun to `completed` in **one** RocksDB write batch. Append an audit row.

The line offset is captured in `RawDomainEvent.source_offset` so an operator inspecting `dj_raw_events` knows exactly which line of which file produced any row.

JSON path resolution (demo subset):

```rust
// Current implementation: crates/context-graph-cli/src/commands/dynamicjepa.rs
// Future refactor target: crates/context-graph-dynamicjepa/src/adapters/jsonpath.rs
pub fn resolve(root: &serde_json::Value, path: &str) -> Result<serde_json::Value, DynamicJepaError>;
```

Supports `$.<key>(\.<key>)*` only. Reject `[*]`, `[?(...)]`, recursive descent (`..`), and any non-dotted segment. The full plan's `$.a.b.c` syntax is sufficient for both demo packs.

## 3. Adapter Run Record

```rust
pub struct AdapterRunRecord {
    pub header: DjRecordHeader,
    pub adapter_run_id: Uuid,
    pub adapter_id: AdapterId,
    pub domain_pack_id: DomainPackId,
    pub event_id: EventId,
    pub started_at_unix_ms: i64,
    pub finished_at_unix_ms: Option<i64>,
    pub status: AdapterRunStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub field_path: Option<String>,
    pub expected_kind: Option<String>,
    pub actual_kind: Option<String>,
    pub output_state_id: Option<StateId>,
    pub output_action_id: Option<ActionId>,
    pub output_outcome_id: Option<OutcomeId>,
    pub output_transition_id: Option<TransitionId>,
}

pub enum AdapterRunStatus { Started, Completed, Failed }
```

Audit invariant: every `event_id` in `dj_raw_events` has exactly one `dj_adapter_runs` row whose terminal status is `Completed` xor `Failed`. (`Started` is transient — the same row is updated to `Completed`/`Failed` in the same batch as the normalized writes.) The FSV harness asserts this with a count diff.

## 4. Instrument Execution

Current implementation is in `crates/context-graph-cli/src/commands/dynamicjepa.rs`. Moving this into `crates/context-graph-dynamicjepa/src/instruments/` remains a future refactor and must not change the verified behavior.

Per `InstrumentSpec`, for an event with normalized records:

1. Resolve every `input_field` against the typed normalized records (`state.counter` → `NormalizedState.fields["counter"]`).
2. If a required input field is missing → `RequiredInstrumentMissing` and the panel is not materialized for that event. Emit an audit row.
3. Run the kind-specific transform:
   - **scalar**: pull the scalar value (`I64`/`F64`), cast to `f32`, apply `normalization.standard_score`, return `[1]`-shape vec. NaN/Inf checks per validation.
   - **categorical**: look up the variant index; return one-hot of length `variants.len()`. Unknown variant → `InstrumentFailed`.
   - **onehot**: for compound input (e.g. `(x, y)` with declared dim 25), compute `idx = x * GRID + y`; return one-hot of length `dim`. Out-of-range → `InstrumentFailed`.
   - **time**: `unix_ms` → `f32` divided by `1000.0` (seconds since epoch) for `UnixSeconds` encoding; or modular fractional day for `FractionalDayOfWeek`.
4. Compute `input_hash = sha256(canonical_bytes_of(resolved_inputs))`.
5. Compute `instrument_hash = blake3_128((instrument_id, version, output_shape))`.
6. Persist `InstrumentReading` to `CF_DJ_INSTRUMENT_READINGS` with key `[event_uuid][instrument_hash]`.

A single batch persists all readings for one event plus the panel record (Section 5).

## 5. Panel Materialization

```rust
pub fn materialize_panel(
    db: &Database,
    domain_pack: &DomainPack,
    transition_id: TransitionId,
) -> Result<PanelId, DynamicJepaError>;
```

Algorithm:

1. Load `StateTransition` by id (`get_transition`). Missing → `SourceOfTruthMissing`.
2. Load the linked normalized state, action, outcome. Missing any → `SourceOfTruthMissing`.
3. Iterate `domain_pack.instrument_specs` in declared order. For each spec:
   - Run the instrument (Section 4) and produce a reading.
   - If the spec is `required` and the reading is `Failed`, abort the entire panel materialization with `RequiredInstrumentMissing`. Discard any readings already accumulated in the batch — none have been written to RocksDB yet.
   - If the spec is optional and the reading is `Failed`, append the reading with `Failed` status; mark the slot mask `false`.
4. Build the `LatentPanel`:
   - `ordered_slots` = the spec order.
   - `slot_vectors` = the reading dense vectors in the same order.
   - `slot_masks` = `true` for required+ok and optional+ok; `false` for optional+failed.
   - `panel_hash = sha256(canonical_bytes_of(domain_pack_id, sorted reading_ids, ordered_slots, slot_vectors))`.
   - `materializer_version = 1`.
5. **One** RocksDB batch writes all readings + the panel + an audit row.
6. Independently read back the panel and every reading using `get_latent_panel` + `list_instrument_readings_for_event`. Verify counts. If any mismatch, return `StorageInvariantViolation`.

The materialization is deterministic — running it twice produces identical bytes (via the same `panel_hash`). A second `materialize_panel` call on the same transition is idempotent and rejects duplicate panel writes.

## 6. Pending Panel Discovery

`dynamicjepa materialize-panel --all-pending --domain <id>`:

1. List every `StateTransition` for `domain_pack_id`.
2. List every `LatentPanel`. Compute the set difference.
3. For each missing transition, call `materialize_panel`.
4. Print a summary: total transitions, materialized, skipped, failed.

This is how the demo bulk-materializes 8000 gridworld panels after `ingest-event`.

## 7. Reading-Back APIs

These are exercised by every FSV harness:

```rust
pub fn get_raw_event(db: &Database, id: EventId) -> Result<Option<RawDomainEvent>>;
pub fn get_adapter_run(db: &Database, id: Uuid) -> Result<Option<AdapterRunRecord>>;
pub fn get_normalized_state(db: &Database, id: StateId) -> Result<Option<NormalizedState>>;
pub fn get_action(db: &Database, id: ActionId) -> Result<Option<NormalizedAction>>;
pub fn get_outcome(db: &Database, id: OutcomeId) -> Result<Option<NormalizedOutcome>>;
pub fn get_transition(db: &Database, id: TransitionId) -> Result<Option<StateTransition>>;
pub fn list_instrument_readings_for_event(db: &Database, id: EventId) -> Result<Vec<InstrumentReading>>;
pub fn get_latent_panel(db: &Database, id: PanelId) -> Result<Option<LatentPanel>>;
```

Phase 4 CLI subcommands wrap `get_latent_panel` (`get-panel`) and `list_instrument_readings_for_event` (`list-instrument-readings`); other readbacks are currently available through `inspect-cf` and become dedicated `get-*` commands in later phases. JSON output is the canonical machine-readable form for evidence logs.

## 8. Failure Examples (Expected Outputs)

For the gridworld `edge_invalid_action` fixture:

```json
{
  "operation": "ingest_event",
  "status": "failed",
  "error_code": "ADAPTER_FAILED",
  "error_message": "value 'diagonal' is not a declared variant of action.move",
  "field_path": "action.move",
  "expected_kind": "categorical{up,down,left,right,noop}",
  "actual_kind": "string",
  "remediation": "fix fixture to use a declared variant; or extend domain pack action.move variants and bump version",
  "raw_event_id": "<uuid>",
  "adapter_run_id": "<uuid>",
  "wrote_normalized": false
}
```

The CLI exits non-zero. The operator sees the exact failing line and field. The DB ends up with the raw event + failed adapter run row only — exactly the documented edge-case state.

## 9. No-Fallback Patterns To Reject

If any of these appear in a PR, reject:

- Missing field defaulted to `0`/`""`/`null`.
- Categorical value mapped to "unknown" bucket when it is not a declared variant.
- Adapter ignoring extra JSON keys silently. (Demo's strict mode rejects unknown keys at the line level — log every extra key as a structured error before failing.)
- Returning a placeholder vector when an instrument fails.
- A panel materialized with `slot_masks` all `true` when any reading was actually `Failed`.
- An `AdapterRunRecord` written without a corresponding `RawDomainEvent`.
- A `StateTransition` written without all four prior/action/outcome/next_state references being valid `Uuid`s.
