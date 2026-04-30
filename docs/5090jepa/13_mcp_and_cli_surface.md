# 13 — MCP & CLI Surface (v1)

The DynamicJEPA tools are *additive* on the existing 110-tool MCP with `llm` enabled (106 without) and existing CLI. Every mutating operation returns its source-of-truth ids and verification summaries; every read tool decodes records by id.

Current implementation status as of the Phase 7/8 review on 2026-04-30: the CLI implements and FSV-accepts Phases 3-8 in the working tree, including dataset shard compilation/read/inspection, CUDA training, training-run reads, artifact hash verification, prediction, planning, guard decisions, and surprise recording. DynamicJEPA MCP tools are still Phase 9 and are not present in `context-graph-mcp` yet. Bundled `verify-*` CLI commands remain future Phase 10 work.

## 1. Tool Name Constants

Append to `crates/context-graph-mcp/src/tools/names.rs`:

```rust
pub const DYNAMICJEPA_REGISTER_DOMAIN_PACK:    &str = "dynamicjepa_register_domain_pack";
pub const DYNAMICJEPA_LIST_DOMAIN_PACKS:       &str = "dynamicjepa_list_domain_packs";
pub const DYNAMICJEPA_GET_DOMAIN_PACK:         &str = "dynamicjepa_get_domain_pack";
pub const DYNAMICJEPA_INGEST_EVENT:            &str = "dynamicjepa_ingest_event";
pub const DYNAMICJEPA_RUN_ADAPTER:             &str = "dynamicjepa_run_adapter";
pub const DYNAMICJEPA_MATERIALIZE_PANEL:       &str = "dynamicjepa_materialize_panel";
pub const DYNAMICJEPA_GET_PANEL:               &str = "dynamicjepa_get_panel";
pub const DYNAMICJEPA_LIST_INSTRUMENT_READINGS: &str = "dynamicjepa_list_instrument_readings";
pub const DYNAMICJEPA_CREATE_BINDING:          &str = "dynamicjepa_create_binding";
pub const DYNAMICJEPA_LIST_BINDINGS:           &str = "dynamicjepa_list_bindings";
pub const DYNAMICJEPA_COMPILE_TRAJECTORIES:    &str = "dynamicjepa_compile_trajectories";
pub const DYNAMICJEPA_GET_TRAJECTORY:          &str = "dynamicjepa_get_trajectory";
pub const DYNAMICJEPA_LIST_TRAJECTORIES:       &str = "dynamicjepa_list_trajectories";
pub const DYNAMICJEPA_COMPILE_DATASET:         &str = "dynamicjepa_compile_dataset";
pub const DYNAMICJEPA_GET_DATASET_SHARD:       &str = "dynamicjepa_get_dataset_shard";
pub const DYNAMICJEPA_INSPECT_DATASET_ROW:     &str = "dynamicjepa_inspect_dataset_row";
pub const DYNAMICJEPA_TRAIN:                   &str = "dynamicjepa_train";
pub const DYNAMICJEPA_GET_TRAINING_RUN:        &str = "dynamicjepa_get_training_run";
pub const DYNAMICJEPA_GET_ARTIFACT:            &str = "dynamicjepa_get_artifact";
pub const DYNAMICJEPA_PREDICT:                 &str = "dynamicjepa_predict";
pub const DYNAMICJEPA_PLAN:                    &str = "dynamicjepa_plan";
pub const DYNAMICJEPA_RECORD_SURPRISE:         &str = "dynamicjepa_record_surprise";
pub const DYNAMICJEPA_VERIFY_COUNTER_WORLD:    &str = "dynamicjepa_verify_counter_world";
pub const DYNAMICJEPA_VERIFY_GRIDWORLD:        &str = "dynamicjepa_verify_gridworld";
pub const DYNAMICJEPA_INSPECT_COUNTS:          &str = "dynamicjepa_inspect_counts";
pub const DYNAMICJEPA_INSPECT_CF:              &str = "dynamicjepa_inspect_cf";
```

That is **26** new tools. Update the tool count test:

```rust
// crates/context-graph-mcp/src/tools/definitions/tests.rs
#[test]
fn test_total_tool_count_and_no_duplicates() {
    let tools = get_tool_definitions();
    let names: HashSet<_> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.len(), tools.len(), "duplicate tool names");
    let expected = if cfg!(feature = "llm") { 110 + 26 } else { 106 + 26 };
    assert_eq!(tools.len(), expected, "expected {expected} tools, got {}", tools.len());
}
```

(Numbers match the spec doc baseline. If the baseline drifts before Phase 9, update this assertion at audit time.)

## 2. Tool Definitions Module

```
crates/context-graph-mcp/src/tools/definitions/dynamicjepa.rs
```

One `pub fn dynamicjepa_tools() -> Vec<ToolDefinition>` aggregating every name above. Add to `definitions/mod.rs::get_tool_definitions()`.

## 3. Handler Module

```
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_dtos.rs
```

DTOs use `serde_json::Value` for typed input/output. Every DTO has a strict schema (additionalProperties: false). Every mutating handler:

1. Reads `before_counts` from `snapshot_dj_counts`.
2. Performs the operation against storage.
3. Reads `after_counts`.
4. Decodes one or more new records as evidence.
5. Returns the response shape in Section 4.

Add dispatch arms in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`.

## 4. MCP Response Shape

Mutating tool success:

```json
{
  "operation": "dynamicjepa_ingest_event",
  "status": "ok",
  "created_ids": {
    "raw_event_ids": ["..."],
    "adapter_run_ids": ["..."],
    "state_ids": ["..."],
    "action_ids": ["..."],
    "outcome_ids": ["..."],
    "transition_ids": ["..."]
  },
  "source_of_truth_cfs": [
    "dj_raw_events", "dj_adapter_runs",
    "dj_normalized_states", "dj_actions", "dj_outcomes", "dj_transitions"
  ],
  "before_counts": { "dj_raw_events": 0, ... },
  "after_counts":  { "dj_raw_events": 3, ... },
  "audit_log_ids": ["..."]
}
```

Mutating tool failure:

```json
{
  "operation": "dynamicjepa_ingest_event",
  "status": "error",
  "error_code": "ADAPTER_FAILED",
  "error_message": "value 'two' is not an i64 at action.delta",
  "field_path": "action.delta",
  "domain_pack_id": "counter_world",
  "remediation": "fix fixture or extend domain pack",
  "wrote_raw_event": true,
  "wrote_adapter_run_failed": true,
  "wrote_normalized": false,
  "after_counts": { ... }
}
```

Read tool success returns the decoded record(s) verbatim plus a `source_of_truth_cf` field. Read tool miss returns `status: "not_found"` with the queried id and CF.

## 5. CLI Subcommand Module

```
crates/context-graph-cli/src/commands/dynamicjepa.rs
```

Wire to `commands/mod.rs` and the top-level clap-derive enum in `main.rs`.

Current CLI status: the registry/inspection subset is implemented and verified:
`register-domain`, `list-domains`, `get-domain`, `inspect-counts`, and
`inspect-cf`. The adapter/panel subset is also implemented and verified:
`ingest-event`, `run-adapter`, `materialize-panel`, `get-panel`, and
`list-instrument-readings`. The Phase 5 binding/trajectory subset is implemented
and verified: `bind`, `list-bindings`, `compile-trajectories`, `get-trajectory`,
and `list-trajectories`. The Phase 6 dataset subset is implemented and verified:
`compile-dataset`, `get-dataset-shard`, and `inspect-dataset`. The Phase 7
training/artifact subset is implemented and verified for counter_world:
`train`, `get-training-run`, and `get-artifact --verify-files`. The Phase 8
prediction/planning/surprise subset is implemented and FSV-tested:
`predict`, `plan`, `record-surprise`, `get-prediction`, `get-plan-trace`, and
`get-surprise`. Bundled verification commands and MCP tools remain unimplemented.

Subcommands (all accept `--db PATH` and `--json`):

```
context-graph dynamicjepa register-domain --db PATH --file PATH --json
context-graph dynamicjepa list-domains --db PATH --json
context-graph dynamicjepa get-domain --db PATH --id ID --json
context-graph dynamicjepa get-domain --db PATH --id ID --domain-version 1.0.0 --json
context-graph dynamicjepa inspect-counts --db PATH --json
context-graph dynamicjepa inspect-cf --db PATH --cf dj_domain_packs --limit N --json

context-graph dynamicjepa ingest-event --domain ID --adapter ID --file PATH
context-graph dynamicjepa run-adapter --event-id UUID
context-graph dynamicjepa materialize-panel --transition-id UUID
context-graph dynamicjepa materialize-panel --all-pending --domain ID
context-graph dynamicjepa get-panel --panel-id UUID --include-readings
context-graph dynamicjepa list-instrument-readings --event-id UUID

context-graph dynamicjepa bind --left-cf NAME --left-key BYTES --right-cf NAME --right-key BYTES --method id_equality
context-graph dynamicjepa list-bindings --entity ENTITY

context-graph dynamicjepa compile-trajectories --domain ID --policy by_domain_session
context-graph dynamicjepa get-trajectory --id UUID
context-graph dynamicjepa list-trajectories --domain ID

context-graph dynamicjepa compile-dataset --domain ID --policy one_step --split train|val|test
context-graph dynamicjepa get-dataset-shard --dataset-id UUID --shard-id UUID
context-graph dynamicjepa inspect-dataset --dataset-id UUID --shard-id UUID --row N

context-graph dynamicjepa train --dataset-id UUID --config PATH --artifact-root PATH
context-graph dynamicjepa get-training-run --id UUID
context-graph dynamicjepa get-artifact --id UUID --verify-files

context-graph dynamicjepa predict --artifact-id UUID --panel-id UUID --action-id UUID
context-graph dynamicjepa plan --artifact-id UUID --panel-id UUID --skill-id UUID
context-graph dynamicjepa record-surprise --prediction-id UUID --observed-outcome-id UUID --observed-panel-id UUID
context-graph dynamicjepa get-prediction --id UUID
context-graph dynamicjepa get-plan-trace --id UUID --include-predictions --include-guards
context-graph dynamicjepa get-surprise --id UUID

context-graph dynamicjepa inspect-counts
context-graph dynamicjepa inspect-cf --cf NAME --limit N

# Future Phase 10:
context-graph dynamicjepa verify-counter-world --fixtures-root PATH --artifact-root PATH
context-graph dynamicjepa verify-gridworld     --fixtures-root PATH --artifact-root PATH
```

CLI exit codes:

- `0` on success.
- `1` on any structured `DynamicJepaError`.
- `2` on argument validation errors.
- `3` on environment errors (CUDA missing, GPU unavailable, lock file present).

## 6. Capability Matrix Updates

Extend `crates/context-graph-mcp/src/handlers/tools/capability_tools.rs::get_capability_matrix` to include:

```json
"dynamicjepa": {
  "enabled": true,
  "tool_count": 26,
  "tools": ["dynamicjepa_register_domain_pack", ...],
  "column_families": ["dj_domain_packs", ...],
  "cf_counts": { "dj_domain_packs": 1, "dj_latent_panels": 8003, ... },
  "registered_domain_packs": [
    { "id": "counter_world", "version": "1.0.0", "instrument_count": 2, "adapter_count": 1, "objective_count": 1 },
    { "id": "gridworld_5x5", "version": "1.0.0", "instrument_count": 3, "adapter_count": 1, "objective_count": 1 }
  ],
  "model_artifact_count": 2,
  "training_run_count": 2,
  "verification_run_count": 2,
  "latest_verification": { "test_name": "verify-gridworld", "status": "Passed", "completed_at_unix_ms": ... }
}
```

The capability matrix is the single MCP call an operator (or another agent) uses to confirm DynamicJEPA is alive, configured, and has run.

## 7. JSON Output Contract

Every CLI subcommand with `--json` emits a single JSON object (no streaming, no NDJSON). The schema is:

```json
{
  "operation": "<command name>",
  "status": "ok" | "error" | "not_found",
  "db_path": "/absolute/path",
  "source_of_truth": ["dj_..."],
  "before_counts": { "..." },
  "after_counts": { "..." },
  "created_ids": { "...": ["..."] },
  "decoded_records": { "...": <json> },
  "artifact_hashes": { "<file>": { "expected": "...", "actual": "...", "equal": true } },
  "error_code": "...",
  "error_message": "...",
  "remediation": "..."
}
```

Empty fields are omitted. The harness pipes this output into `tmp/5090jepa_evidence/<domain>/<run_id>/` files.

## 8. MCP Schema Strictness

Every DynamicJEPA tool definition sets `additionalProperties: false` in its JSON schema. Unknown params must surface as `INVALID_PARAMS` errors, not be silently dropped. (This was a real bug in the wider Context Graph project; do not repeat it.)

## 9. Daemon Vs Standalone

The demo runs CLI commands against `/tmp/5090jepa-cw-fsv` and `/tmp/5090jepa-gw-fsv`, which the operator's daemon does not own. CLI commands open the DB directly, hold the flock for the duration of the command, and release it on exit. The MCP daemon never touches the demo paths.

If the operator wants to call DynamicJEPA tools through the MCP server later, the daemon’s storage path would need to be the demo path (set via `CONTEXT_GRAPH_STORAGE_PATH=/tmp/5090jepa-cw-fsv` before launching the MCP server). The demo's verification harness explicitly does **not** require the MCP server to be running — every CLI subcommand is self-sufficient.

## 10. Test Surface

Add or keep current:

- Current Phase 3: `crates/context-graph-cli/tests/dynamicjepa_phase3_fsv.rs` — real CLI binary, real temp RocksDB, both checked-in domain TOMLs, decoded CF inspection, and empty TOML / duplicate instrument / unknown TOML key / invalid CF edge cases.
- Current Phase 4: `crates/context-graph-cli/tests/dynamicjepa_phase4_fsv.rs` — real CLI binary, real temp RocksDB, checked-in domain packs and fixtures, decoded CF inspection for adapter/panel rows, and empty input / invalid action / max or out-of-bounds edge cases.
- Current Phase 5: `crates/context-graph-cli/tests/dynamicjepa_phase5_fsv.rs` — real CLI binary, real temp RocksDB, trajectories, bindings, and edge-case source-of-truth count checks.
- Current Phase 6: `crates/context-graph-cli/tests/dynamicjepa_phase6_fsv.rs` — real CLI binary, real temp RocksDB, dataset shards, row inspection, and dataset edge cases.
- Current Phase 7: `crates/context-graph-cli/tests/dynamicjepa_phase7_fsv.rs` — real CLI binary, real temp RocksDB, generated counter_world training data, CUDA training, artifact hash verification, and missing dataset / invalid CPU config / corrupted artifact edge cases.
- Current Phase 8: `crates/context-graph-cli/tests/dynamicjepa_phase8_fsv.rs` — real CLI binary, real temp RocksDB, generated counter_world training data, CUDA training, prediction, plan, guard decision, surprise, readback, missing artifact, and corrupt artifact edge cases.
- `crates/context-graph-mcp/tests/dynamicjepa_tools_dispatch_test.rs` — confirm every tool name maps to a handler.
- `crates/context-graph-mcp/tests/dynamicjepa_capability_matrix_test.rs` — confirm the capability matrix includes DynamicJEPA fields after a `verify-counter-world` run.
- `crates/context-graph-cli/tests/dynamicjepa_cli_smoke_test.rs` — CLI argument parsing, exit codes, JSON shape.
- `crates/context-graph-cli/tests/dynamicjepa_counter_world_fsv.rs` — full happy path + 3 edge cases against a real `/tmp/` DB. (This is the same shape as `tests/e14_phase_b_full_state_verification.rs`.)
- `crates/context-graph-cli/tests/dynamicjepa_gridworld_fsv.rs` — same for gridworld.

These are the tests the user will run to confirm the demo works.
