# Target Module And Crate Layout

DynamicJEPA should be implemented as an additive subsystem with clear ownership boundaries. The goal is to keep schema, persistence, model execution, MCP, and CLI testability separate.

## Workspace Additions

Add one new workspace crate:

- `crates/context-graph-dynamicjepa`

Purpose:

- JEPA model definitions;
- training loops;
- inference services;
- planner services;
- skill policy execution;
- runtime orchestration over storage and embeddings.

Do not put Candle training code in `context-graph-core`. Core should remain the contract crate.

Update root `Cargo.toml`:

- add `crates/context-graph-dynamicjepa` to `members`;
- add it to `default-members` only after the initial data-plane phase compiles;
- add workspace dependency alias if needed.

## Core Module Layout

Add:

```text
crates/context-graph-core/src/dynamicjepa/
  mod.rs
  ids.rs
  error.rs
  record_header.rs
  domain_pack.rs
  schema.rs
  instrument.rs
  adapter.rs
  event.rs
  state_action_outcome.rs
  panel.rs
  binding.rs
  trajectory.rs
  dataset.rs
  predictor.rs
  artifact.rs
  planner.rs
  skill.rs
  guard.rs
  verification.rs
  validation.rs
  tests.rs
```

Core responsibilities:

- define serializable records and ids;
- validate record invariants;
- expose deterministic key helpers where storage needs shared behavior;
- expose typed errors convertible to `ContextGraphError`;
- no RocksDB handles;
- no CUDA/Candle model execution.

Add to `crates/context-graph-core/src/lib.rs`:

```rust
pub mod dynamicjepa;
```

## Storage Module Layout

Add:

```text
crates/context-graph-storage/src/dynamicjepa/
  mod.rs
  column_families.rs
  error.rs
  keys.rs
  encode.rs
  domain_pack_store.rs
  event_store.rs
  panel_store.rs
  binding_store.rs
  trajectory_store.rs
  dataset_store.rs
  artifact_store.rs
  prediction_store.rs
  planning_store.rs
  verification_store.rs
  inspection.rs
  tests.rs
```

Storage responsibilities:

- define DynamicJEPA CF names and options;
- open all DynamicJEPA CFs at DB startup;
- encode/decode versioned bincode records;
- write atomic multi-CF batches;
- expose direct read/list/count APIs for FSV;
- never synthesize missing values on read.

Integration options:

- Preferred: integrate DynamicJEPA CFs into the existing RocksDB database open path so one physical DB is the system source of truth.
- Avoid: a separate database unless there is a measured operational reason, because it complicates atomic writes and verification.

## DynamicJEPA Service Crate Layout

Add:

```text
crates/context-graph-dynamicjepa/src/
  lib.rs
  config.rs
  service.rs
  adapters/
    mod.rs
    json_event.rs
    memory_event.rs
    graph_event.rs
  instruments/
    mod.rs
    scalar.rs
    categorical.rs
    text_embedding.rs
    graph_neighborhood.rs
    time.rs
  panel/
    mod.rs
    materializer.rs
  binding/
    mod.rs
    resolver.rs
  dataset/
    mod.rs
    compiler.rs
    splitter.rs
  model/
    mod.rs
    encoder.rs
    predictor.rs
    losses.rs
    trainer.rs
    inference.rs
  planner/
    mod.rs
    rollout.rs
    skill_policy.rs
    guard.rs
  verification/
    mod.rs
    harness.rs
```

Service responsibilities:

- execute adapter and instrument pipelines;
- call existing embedders for text/code/entity/graph instruments;
- train and evaluate DynamicJEPA predictors;
- register artifacts through storage;
- run prediction and planning workflows;
- write verification run records.

## MCP Additions

Add:

```text
crates/context-graph-mcp/src/tools/definitions/dynamicjepa.rs
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_dtos.rs
```

Modify:

- `tools/definitions/mod.rs`;
- `tools/names.rs`;
- `handlers/tools/mod.rs`;
- `handlers/tools/dispatch.rs`;
- capability matrix output;
- tool count tests.

## CLI Additions

Add:

```text
crates/context-graph-cli/src/commands/dynamicjepa.rs
```

Modify:

- `crates/context-graph-cli/src/commands/mod.rs`;
- root CLI command registration file after locating the current CLI entry point;
- CLI integration tests using a temporary real RocksDB database.

## Config And Fixtures

Add:

```text
configs/dynamicjepa/
  README.md
  domain_packs/
    counter_world.v1.toml
    context_graph_memory.v1.toml
    learner_state.v1.toml
  verification/
    counter_world_happy_path.jsonl
    counter_world_edge_empty_input.json
    counter_world_edge_invalid_action.json
    counter_world_edge_max_panel.json
```

These are not mock data. They are deterministic synthetic real input files processed through the actual pipeline and persisted to RocksDB.
