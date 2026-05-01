# Current Context Graph Inventory

This inventory anchors DynamicJEPA to the current repository rather than an abstract greenfield architecture.

## Workspace Crates

The current workspace members are:

- `crates/context-graph-core`: core domain types, traits, validation, errors, teleological types, constellations, learning, learner state, graph linking, and training record types.
- `crates/context-graph-storage`: RocksDB-backed teleological storage, column-family definitions, encode/decode helpers, training export storage, learner storage, constellation storage, contrastive storage, typed-edge records, and source metadata.
- `crates/context-graph-embeddings`: embedding pipeline and Candle/CUDA model loading.
- `crates/context-graph-cuda`: GPU acceleration support.
- `crates/context-graph-mcp`: JSON-RPC/MCP server, tool definitions, tool names, aliases, handlers, and dispatch.
- `crates/context-graph-cli`: operator CLI command modules.
- `crates/context-graph-benchmark`: explicit benchmark crate excluded from default builds.
- `crates/context-graph-causal-agent` and `crates/context-graph-graph-agent`: LLM-backed causal/graph services.
- `crates/context-graph-test-utils`: shared real test utilities.

DynamicJEPA should add a model/service crate instead of loading training code into `context-graph-core`. Core stays mostly pure schema and validation; storage stays source-of-truth persistence; the new crate owns JEPA training/inference and depends on storage, embeddings, and CUDA where needed.

## Existing Core Surfaces To Reuse

Relevant core modules already exported from `crates/context-graph-core/src/lib.rs`:

- `constellation`: compiled latent centroids and spread statistics.
- `contrastive`: contrastive pair types and anomaly categories.
- `embeddings`, `teleological`, `types`: existing multi-embedder fingerprint model.
- `learning`, `learner`, `learner_training`: before/after learning events and learner-state export patterns.
- `graph_linking`, `typed_edge_export`: graph and relationship training surfaces.
- `error`: `ContextGraphError`, `CoreError`, and sub-errors.

DynamicJEPA should add `pub mod dynamicjepa;` in `context-graph-core/src/lib.rs` once core records are created.

## Existing Storage Patterns To Reuse

`crates/context-graph-storage/src/teleological/column_families.rs` currently declares teleological CFs and quantized embedder CFs. It already uses:

- named CF constants;
- documented key/value formats;
- explicit count constants;
- custom options per CF;
- 16-byte UUID prefix extractors for point lookups;
- composite keys for typed edge and contrastive records;
- versioned bincode payloads;
- append-oriented audit and history CFs;
- no-fallback comments and fail-fast RocksDB open behavior.

`crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs` is the right pattern for DynamicJEPA persisted records: `[version: u8][bincode payload]`, reject empty bytes, reject wrong versions, include payload type names and remediation hints.

DynamicJEPA should add a separate storage module and CF block rather than overloading existing memory CFs. Existing CFs remain usable as inputs and joins.

## Existing MCP Patterns To Reuse

MCP tool definitions are split by module under `crates/context-graph-mcp/src/tools/definitions/`. Tool names live in `crates/context-graph-mcp/src/tools/names.rs`. Dispatch is centralized in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`.

To add DynamicJEPA:

- add `tools/definitions/dynamicjepa.rs`;
- add `handlers/tools/dynamicjepa_tools.rs`;
- add tool-name constants in `tools/names.rs`;
- extend `tools/definitions/mod.rs`;
- extend dispatch in `handlers/tools/dispatch.rs`;
- update tool count tests.

## Existing CLI Patterns To Reuse

CLI commands live under `crates/context-graph-cli/src/commands/`. DynamicJEPA should add:

- `commands/dynamicjepa.rs`;
- subcommands for register, ingest, materialize, bind, compile, train, predict, plan, verify, inspect, and count;
- machine-readable JSON output for manual FSV logs.

The CLI must expose direct source-of-truth readbacks, not just high-level success wrappers.

## Existing Documentation To Reuse

The following docs define relevant prior planning:

- `docs/dynamicjepa/*.md`: high-level DynamicJEPA architecture, storage, FSV, and references.
- `docs/mejepa/*.md`: specific ME-JEPA plan for the recruiting/career-trajectory application.
- `docs/bigunlock/07_full_state_verification.md`: FSV mentality and manual verification style.
- `docs/bigunlock/08_implementation_roadmap.md`: phase-oriented implementation planning pattern.

## Main Gaps

The current repo does not yet have:

- a generic DynamicJEPA core module;
- domain pack registry records;
- universal state/action/outcome records;
- adapter run records;
- latent panel records independent of memory fingerprints;
- cross-domain binding records;
- DynamicJEPA trajectory and dataset shard records;
- DynamicJEPA training run and model artifact registry;
- prediction, plan trace, skill, guard, and surprise-event source-of-truth records;
- DynamicJEPA MCP/CLI surfaces;
- end-to-end manual FSV harness for the full DynamicJEPA loop.
