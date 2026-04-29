# MCP And CLI API Spec

MCP is for tool consumers. CLI is for operators and Full State Verification. Both must expose decoded source-of-truth reads, not just write commands.

## MCP Tool Additions

Add tool constants in `crates/context-graph-mcp/src/tools/names.rs`:

```text
DYNAMICJEPA_REGISTER_DOMAIN_PACK
DYNAMICJEPA_LIST_DOMAIN_PACKS
DYNAMICJEPA_GET_DOMAIN_PACK
DYNAMICJEPA_INGEST_EVENT
DYNAMICJEPA_RUN_ADAPTER
DYNAMICJEPA_MATERIALIZE_PANEL
DYNAMICJEPA_GET_PANEL
DYNAMICJEPA_CREATE_BINDING
DYNAMICJEPA_LIST_BINDINGS
DYNAMICJEPA_COMPILE_TRAJECTORIES
DYNAMICJEPA_GET_TRAJECTORY
DYNAMICJEPA_COMPILE_DATASET
DYNAMICJEPA_GET_DATASET_SHARD
DYNAMICJEPA_TRAIN
DYNAMICJEPA_REGISTER_ARTIFACT
DYNAMICJEPA_PREDICT
DYNAMICJEPA_PLAN
DYNAMICJEPA_RECORD_SURPRISE
DYNAMICJEPA_VERIFY_STATE
DYNAMICJEPA_INSPECT_COUNTS
```

Tool definitions go in:

```text
crates/context-graph-mcp/src/tools/definitions/dynamicjepa.rs
```

Handlers go in:

```text
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs
```

DTOs go in:

```text
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_dtos.rs
```

## MCP Response Contract

Every mutating tool response includes:

- operation;
- status;
- created or updated ids;
- source-of-truth CF names;
- before counts when requested;
- after counts when requested;
- verification readback summary;
- audit id;

Errors include:

- operation;
- tool name;
- domain pack id if known;
- target id if known;
- source-of-truth CF if relevant;
- remediation hint;
- structured error code.

## CLI Command Additions

Add `crates/context-graph-cli/src/commands/dynamicjepa.rs`.

Commands:

```text
context-graph dynamicjepa register-domain --file PATH --db PATH
context-graph dynamicjepa list-domains --db PATH --json
context-graph dynamicjepa get-domain --id ID --db PATH --json
context-graph dynamicjepa ingest-event --domain ID --adapter ID --file PATH --db PATH
context-graph dynamicjepa run-adapter --event-id UUID --db PATH
context-graph dynamicjepa materialize-panel --transition-id UUID --db PATH
context-graph dynamicjepa inspect-panel --panel-id UUID --include-readings --db PATH --json
context-graph dynamicjepa bind --left REF --right REF --method METHOD --evidence REF --db PATH
context-graph dynamicjepa list-bindings --entity ENTITY --db PATH --json
context-graph dynamicjepa compile-trajectories --domain ID --policy ID --db PATH
context-graph dynamicjepa get-trajectory --id UUID --db PATH --json
context-graph dynamicjepa compile-dataset --domain ID --policy ID --split train --db PATH
context-graph dynamicjepa get-dataset-shard --dataset-id UUID --shard-id UUID --db PATH --json
context-graph dynamicjepa train --dataset-id UUID --config PATH --db PATH --artifact-root PATH
context-graph dynamicjepa register-artifact --training-run-id UUID --path PATH --db PATH
context-graph dynamicjepa predict --artifact-id UUID --panel-id UUID --action-id UUID --db PATH
context-graph dynamicjepa plan --artifact-id UUID --panel-id UUID --skill-id UUID --db PATH
context-graph dynamicjepa record-surprise --prediction-id UUID --observed-outcome-id UUID --observed-panel-id UUID --db PATH
context-graph dynamicjepa inspect-counts --db PATH --json
context-graph dynamicjepa inspect-cf --cf NAME --limit N --db PATH --json
context-graph dynamicjepa verify-counter-world --db PATH --fixtures PATH --artifact-root PATH
```

## CLI Output Contract

All FSV-supporting commands must support `--json`.

JSON output includes:

- `operation`;
- `db_path`;
- `source_of_truth`;
- `before_counts`;
- `after_counts`;
- `created_ids`;
- `decoded_records`;
- `artifact_hashes`;
- `error` if failed.

Human-readable output can be added, but JSON output is required for evidence logs.

## Capability Matrix

Extend `get_capability_matrix` output with:

- DynamicJEPA enabled status;
- domain pack count;
- event count;
- panel count;
- trajectory count;
- dataset shard count;
- model artifact count;
- prediction count;
- plan trace count;
- surprise event count;
- DynamicJEPA CF list;
- registered built-in adapters and instruments.

## Tool Count Tests

When adding MCP tools:

- update `get_tool_definitions()` capacity;
- update total tool count tests;
- update submodule count tests;
- assert no duplicate tool names;
- assert every DynamicJEPA tool has a schema and description;
- include tests for alias behavior only if aliases are intentionally added.

## Full State Verification CLI Requirement

Do not rely on MCP write responses for verification. For every mutating MCP tool, there must be a CLI or MCP read tool that can independently inspect the source-of-truth CF and decode the row.
