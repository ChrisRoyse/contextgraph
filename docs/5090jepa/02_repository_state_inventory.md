# 02 — Repository State Inventory

This file is the anti‑hallucination anchor. Re‑run the audit in Section 3 before every remaining phase, starting with Phase 7. If anything below is wrong relative to the live tree, the live tree wins; update this file *before* writing code that depends on a stale fact.

## 1. Branch & Commit Snapshot (Phase 4 Double-Check: 2026-04-30)

| Property | Value |
| --- | --- |
| Branch | `main` |
| Latest commits at audit start | `5c907530 feat(dynamicjepa): add Phase 4 adapter/panel materialization CLI` → `7bc9d6e1 docs(5090jepa): refresh docs to reflect Phase 4 completion` |
| Working tree at audit start | Clean before the Phase 5 implementation work |
| `data/contextgraph_data` | Real RocksDB DB exists with operator data; **never** point demo runs at it |

## 2. Workspace Layout (Verified)

`Cargo.toml` workspace members (10):

```
crates/context-graph-mcp
crates/context-graph-core
crates/context-graph-cuda
crates/context-graph-embeddings
crates/context-graph-storage
crates/context-graph-cli
crates/context-graph-benchmark        # excluded from default-members
crates/context-graph-causal-agent
crates/context-graph-graph-agent
crates/context-graph-test-utils
```

Default members exclude `context-graph-benchmark`. Build order (per `docs2/contextgraphspecs/02_source_code_map.md`): `core → cuda → embeddings → test-utils → storage → causal-agent → graph-agent → mcp → cli → benchmark`.

`docs/5090jepa` adds **one** new crate:

```
crates/context-graph-dynamicjepa
```

To be added to `[workspace] members` and (after Phase 7 stabilizes) to `default-members`. Phase 1‑6 land before the new crate exists; those phases live in `context-graph-core` (records) and `context-graph-storage` (CFs + stores) and `context-graph-cli` (CLI).

## 3. Audit Commands (Run Before The Next Phase)

```bash
cd /home/cabdru/contextgraph

# crate roster
ls crates/

# core lib.rs current modules
sed -n '30,70p' crates/context-graph-core/src/lib.rs

# storage modules
ls crates/context-graph-storage/src/
ls crates/context-graph-storage/src/teleological/

# MCP tool name constants count
grep -c "^pub const " crates/context-graph-mcp/src/tools/names.rs

# CLI commands directory
ls crates/context-graph-cli/src/commands/

# DynamicJEPA current phase state
test ! -d crates/context-graph-dynamicjepa
test -d crates/context-graph-core/src/dynamicjepa
test -d crates/context-graph-storage/src/dynamicjepa
test ! -f crates/context-graph-mcp/src/tools/definitions/dynamicjepa.rs
test -f crates/context-graph-cli/src/commands/dynamicjepa.rs
test -f crates/context-graph-cli/tests/dynamicjepa_phase4_fsv.rs
test -f crates/context-graph-cli/tests/dynamicjepa_phase6_fsv.rs
test -f crates/context-graph-cli/src/bin/context-graph.rs

# Configs root
ls config/                                # operator config (existing)
test -d configs/dynamicjepa              # Phase 0 input fixtures
```

If those checks fail, the implementation phase has drifted. Read the live files before adding parallel work.

## 4. Phase 2 Through Phase 6 Verification Status (2026-04-30)

Phase 2 is implemented and manually verified as the storage data plane. Phase 3 is implemented and manually verified as the CLI domain registry / inspection layer. Phase 4 is implemented and manually verified as the CLI `json_event` adapter, instrument execution, and latent panel materialization layer. Phase 5 is implemented and manually verified as the deterministic binding plus trajectory compiler layer. Phase 6 is implemented and manually verified as the deterministic one-step dataset shard compiler. Phase 7+ service/model/MCP work is still not present.

Source of truth verified:

- Physical RocksDB DB: `tmp/5090jepa_evidence/phase2/storage_fsv/rocksdb_happy`
- DynamicJEPA CF list: all 25 names in `DJ_CF_NAMES`
- Decoded evidence log: `tmp/5090jepa_evidence/phase2/storage_fsv.json`
- Independent physical count read: `ldb --db tmp/5090jepa_evidence/phase2/storage_fsv/rocksdb_happy --column_family=<cf> dump --count_only`

Verification results:

- `cargo test -p context-graph-core dynamicjepa --all-features -- --nocapture`: 3 passed, 0 failed.
- `cargo test -p context-graph-storage dynamicjepa --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-storage --all-features`: passed after correcting stale count invariants; 508 lib tests passed, 1 non-DynamicJEPA performance test ignored, integration tests passed, doctests passed.
- `ldb list_column_families` on the Phase 2 FSV DB shows 98 total CFs, including all 25 DynamicJEPA CFs.
- `ldb dump --count_only` on the Phase 2 FSV DB matches the evidence counts: one row in every DynamicJEPA data CF touched by the synthetic happy path, two instrument readings, two audit records, and one persisted `VerificationRunRecord`.
- Edge cases manually exercised in the FSV DB: empty payload, wrong version byte, and corrupt bincode body. Each leaves a physical corrupt source-of-truth row and fails decode with `CODEC`; no fallback/default decode path is present.

Root-cause fixes made during this audit:

- `crates/context-graph-storage/src/teleological/tests/column_family.rs` now derives the expected all-teleological descriptor count from `TELEOLOGICAL_CF_COUNT + QUANTIZED_EMBEDDER_CF_COUNT` (41 + 14 = 55) instead of the stale hard-coded 54.
- HNSW doctest/comment examples now assert the current 16 HNSW-capable embedders after E14 BGE-M3 Dense, matching the runtime registry.

Phase 3 source of truth verified:

- Physical RocksDB DB: `/tmp/5090jepa-phase3-manual`
- Decoded CLI evidence: `tmp/5090jepa_evidence/phase3/manual_fsv/`
- Automated CLI FSV evidence: `tmp/5090jepa_evidence/phase3/cli_fsv.json`
- Independent physical count read: `ldb --db /tmp/5090jepa-phase3-manual --column_family=<cf> dump --count_only`

Phase 3 verification results:

- `cargo test -p context-graph-cli --test dynamicjepa_phase3_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-cli dynamicjepa --all-features --no-run`: every CLI test target compiled after correcting stale non-DynamicJEPA test code.
- Manual happy path registered `counter_world.v1.toml` and `gridworld_5x5.v1.toml` into one fresh DB. Final Phase 3 source-of-truth counts: `dj_domain_packs=2`, `dj_domain_pack_by_nv=2`, `dj_instrument_registry=5`, `dj_adapter_registry=2`, `dj_audit_log=2`.
- `ldb list_column_families` confirms all 25 DynamicJEPA CFs are physically present in `/tmp/5090jepa-phase3-manual`.
- `ldb dump --count_only` independently confirms the same Phase 3 counts above; `ldb dump --hex --max_keys=1` shows physical versioned bincode rows in `dj_domain_packs` and `dj_audit_log`, plus the 16-byte UUID value in `dj_domain_pack_by_nv`.
- Edge cases manually exercised: empty TOML, duplicate instrument id, unknown TOML key, and invalid CF name. All failed with structured JSON errors and left before/after counts unchanged.

Phase 3 re-audit results:

- Fresh DB used for second-pass proof: `/tmp/5090jepa-phase3-doublecheck`.
- Evidence path: `tmp/5090jepa_evidence/phase3/doublecheck/`.
- CLI help exposes exactly the implemented Phase 3 commands: `register-domain`, `list-domains`, `get-domain`, `inspect-counts`, and `inspect-cf`.
- Registering both checked-in domain packs again produced the expected final source-of-truth counts: `dj_domain_packs=2`, `dj_domain_pack_by_nv=2`, `dj_instrument_registry=5`, `dj_adapter_registry=2`, `dj_audit_log=2`.
- Independent `ldb dump --count_only` against the same DB confirmed those five counts exactly.
- Duplicate-instrument registration failed with `VALIDATION` on `DomainPack.instrument_specs.instrument_id` and left all 25 DynamicJEPA CF counts unchanged.
- Phase 4 supersedes this stale re-audit finding: the `ingest-event`, `run-adapter`, `materialize-panel`, `get-panel`, and `list-instrument-readings` commands are now implemented and verified.

Root-cause fixes made during the Phase 3 audit:

- `get-domain` now uses `--domain-version` because Clap's propagated global `--version` flag reserves the `version` argument id.
- The CLI package exposes both `context-graph-cli` and the documented `context-graph` binary without Cargo duplicate-target warnings.
- Phase 3 `count_delta` now treats missing or unexpected CF names in before/after snapshots as `StorageInvariantViolation` instead of substituting zero.
- `crates/context-graph-cli/tests/export_training_integration.rs` now initializes the current `TrainingRecord::e14_bge_m3_dense` field.
- `crates/context-graph-cli/tests/e14_phase_b_fsv.rs` now calls `EmbedderId::parse`, matching the live DTO API.

Phase 4 source of truth verified:

- Physical RocksDB DBs: `/tmp/5090jepa-phase4-manual-cw` and `/tmp/5090jepa-phase4-manual-gw`.
- Manual decoded evidence: `tmp/5090jepa_evidence/phase4/manual_counter/` and `tmp/5090jepa_evidence/phase4/manual_gridworld/`.
- Automated CLI FSV evidence: `tmp/5090jepa_evidence/phase4/cli_phase4_fsv.json`.
- Independent physical count read: `ldb --db <phase4-db> --column_family=<cf> dump --count_only`.

Phase 4 verification results:

- `cargo test -p context-graph-cli --test dynamicjepa_phase4_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-cli --test dynamicjepa_phase3_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-core dynamicjepa --all-features`: 3 passed, 0 failed.
- `cargo test -p context-graph-storage dynamicjepa --all-features`: 1 passed, 0 failed.
- `cargo build -p context-graph-cli --release --all-features`: passed.
- Manual counter_world happy path registered one pack, ingested three events, materialized three panels, and ended with `dj_domain_packs=1`, `dj_domain_pack_by_nv=1`, `dj_instrument_registry=2`, `dj_adapter_registry=1`, `dj_raw_events=3`, `dj_adapter_runs=3`, `dj_normalized_states=3`, `dj_actions=3`, `dj_outcomes=3`, `dj_transitions=3`, `dj_instrument_readings=6`, `dj_latent_panels=3`, `dj_audit_log=10`.
- Manual counter_world after edge cases ended with `dj_raw_events=5`, `dj_adapter_runs=5`, `dj_normalized_states=3`, `dj_actions=3`, `dj_outcomes=3`, `dj_transitions=3`, `dj_instrument_readings=6`, `dj_latent_panels=3`, `dj_audit_log=15`.
- Manual gridworld happy path registered one pack, ingested one eight-event trajectory, materialized eight panels, and ended with `dj_domain_packs=1`, `dj_domain_pack_by_nv=1`, `dj_instrument_registry=3`, `dj_adapter_registry=1`, `dj_raw_events=8`, `dj_adapter_runs=8`, `dj_normalized_states=8`, `dj_actions=8`, `dj_outcomes=8`, `dj_transitions=8`, `dj_instrument_readings=24`, `dj_latent_panels=8`, `dj_audit_log=25`.
- Manual gridworld after edge cases ended with `dj_raw_events=10`, `dj_adapter_runs=10`, `dj_normalized_states=8`, `dj_actions=8`, `dj_outcomes=8`, `dj_transitions=8`, `dj_instrument_readings=24`, `dj_latent_panels=8`, `dj_audit_log=30`.
- Edge cases verified with before/after snapshots: empty input writes only a failed audit row; invalid action writes one raw event, one failed adapter run, and no normalized rows; counter_world max outcome and gridworld out-of-bounds state write one raw event, one failed adapter run, and no normalized rows.

Root-cause fixes made during the Phase 4 audit:

- Adapter validation now uses schema field order before adapter mapping order, so gridworld out-of-bounds state fails at `state.x` before the outcome fields are considered.
- Optional failed panel slots are valid only with `mask=false` and an empty vector; required slots still require a present vector of the declared dimension.
- Event and panel storage paths gained atomic batch helpers for raw+started, adapter success/failure, and readings+panel writes.

Phase 5 source of truth verified:

- Physical RocksDB DBs: `/tmp/5090jepa-phase5-manual-cw`, `/tmp/5090jepa-phase5-manual-gw`, and `/tmp/5090jepa-phase5-manual-missing-panel`.
- Manual decoded evidence: `tmp/5090jepa_evidence/phase5/manual/`.
- Automated CLI FSV evidence: `tmp/5090jepa_evidence/phase5/cli_phase5_fsv.json`.
- Independent physical count read: `ldb --db <phase5-db> --column_family=<cf> dump --count_only`.

Phase 5 verification results:

- `cargo test -p context-graph-cli --test dynamicjepa_phase5_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-cli --test dynamicjepa_phase4_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-cli --test dynamicjepa_phase3_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- `cargo test -p context-graph-core dynamicjepa --all-features`: 3 passed, 0 failed.
- `cargo test -p context-graph-storage dynamicjepa --all-features`: 1 passed, 0 failed.
- `cargo build -p context-graph-cli --release --all-features`: passed.
- Manual counter_world happy path compiled one trajectory with three ordered transitions and three ordered panels; derived timestamp proof is `[1700000000000, 1700000001000, 1700000002000]`.
- Manual counter_world binding wrote both deterministic methods: explicit mapping (`dj_raw_events` → `dj_trajectories`) and id equality (`dj_raw_events` → same `dj_raw_events` key). Final binding counts are `dj_bindings=2` and `dj_bindings_by_entity=2`; decoded binding evidence references keys that were read from RocksDB.
- Manual gridworld one-trajectory happy path compiled one trajectory with eight ordered transitions and eight ordered panels; derived timestamp proof is `[0,1,2,3,4,5,6,7]` and `entity_refs` carries the source `trajectory_id`.
- Edge cases verified with before/after snapshots: unsupported trajectory policy, duplicate trajectory compile, missing binding evidence, and missing panel source. Each failed with a structured error and left the relevant primary/index/audit counts unchanged.
- Independent `ldb dump --count_only` confirmed final manual counts: counter_world `dj_raw_events=3`, `dj_transitions=3`, `dj_latent_panels=3`, `dj_bindings=2`, `dj_bindings_by_entity=2`, `dj_trajectories=1`, `dj_audit_log=13`; gridworld `dj_raw_events=8`, `dj_transitions=8`, `dj_latent_panels=8`, `dj_trajectories=1`, `dj_audit_log=26`.

Phase 6 verification result:

- `cargo test -p context-graph-cli --test dynamicjepa_phase6_fsv --all-features -- --nocapture`: 1 passed, 0 failed.
- Manual counter_world happy path compiled two dataset shards: train row `t0->t1` and val row `t1->t2`; `get-dataset-shard` and `inspect-dataset` read back the persisted rows from `dj_dataset_shards` and linked panels/actions/outcomes.
- Manual counter_world edge cases verified before/after source-of-truth counts for duplicate split, invalid policy, no trajectories, and insufficient negatives. Each failed with structured JSON and left `dj_dataset_shards`/`dj_audit_log` unchanged.
- Manual full gridworld happy path ingested 8000 events, materialized 8000 panels, compiled 1000 trajectories, and compiled 3 dataset shards with row counts `train=4977`, `val=1330`, `test=693`, totaling 7000 one-step rows.
- Independent `ldb dump --count_only` confirmed final Phase 6 gridworld counts: `dj_dataset_shards=3`, `dj_trajectories=1000`, `dj_latent_panels=8000`, `dj_actions=8000`, `dj_outcomes=8000`, `dj_audit_log=25004`.
- Manual Phase 6 evidence paths: `tmp/5090jepa_evidence/phase6/manual_counter/`, `tmp/5090jepa_evidence/phase6/manual_gridworld/`, and `tmp/5090jepa_evidence/phase6/cli_phase6_fsv.json`.

Root-cause fixes made during the Phase 5 and Phase 6 audits:

- Binding and trajectory storage gained atomic `*_with_audit_batch` helpers, so the primary rows, secondary index rows, and audit rows are persisted together.
- CLI binding validates and decodes both referenced source-of-truth rows before writing; missing evidence fails with `BINDING_EVIDENCE_MISSING` and no row writes.
- Trajectory compilation groups gridworld by the registered entity mapping from raw event payloads while counter_world uses one domain-session group; every transition must have a decoded latent panel before a trajectory row is written.
- Adapter terminal timestamps now clamp `finished_at_unix_ms` to at least the persisted `started_at_unix_ms` before writing terminal run rows, preserving the storage invariant even if the host clock is sampled non-monotonically within the same millisecond.
- Dataset shard storage gained an atomic `put_dataset_shard_with_audit_batch` helper, so the shard row and audit row are written together after leakage and source-hash checks pass.

## 5. Existing Surfaces You Will Reuse

### 5.1 Core (`crates/context-graph-core/src/`)

Modules currently re‑exported in `lib.rs`:

```
causal, clustering, code, config, constellation, contrastive, embeddings, entity,
error, fusion, graph, graph_linking, index, injection, learner, learner_training,
learning, llm_edge_validation, memory, monitoring, quantization, retrieval,
similarity, stubs, teleological, training, traits, typed_edge_export, types, weights
```

Phase 1 already added `pub mod dynamicjepa;` and re‑exports the record, id, validation, and error contracts needed by storage.

Reusable error pattern: `error::ContextGraphError`, `CoreError`, `Result`. Add `DynamicJepaError` in `dynamicjepa::error` and implement `From<DynamicJepaError> for ContextGraphError`.

Reusable record pattern: `learner_training::*` and `training::TrainingRecord` — both use versioned bincode with explicit version bytes. Mirror that pattern for every DynamicJEPA record.

### 5.2 Storage (`crates/context-graph-storage/src/`)

Existing layout:

```
column_families.rs            # 11 base CFs
graph_edges/                  # 3 base graph edge CFs
code/                         # 5 code CFs
teleological/
  column_families.rs          # 41 teleological CFs + 14 quantized = 55
  rocksdb_store/              # all read/write paths
  quantized/                  # per-embedder codecs
  indexes/, search/, schema.rs, serialization.rs
```

98 CFs total today. Phase 2 added a sibling `dynamicjepa/` module under `crates/context-graph-storage/src/` that:

- declares 25 DynamicJEPA CFs (full list in `05_storage_schema_v1.md`);
- contributes its CF descriptors to the existing DB open path so one physical RocksDB stays the source of truth;
- exposes typed `put_*` / `get_*` / `list_*` / `count_*` and inspection/count APIs for every Phase 2 record class.

Reusable codec helper: `crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs`. Use it verbatim for `[version: u8][bincode payload]` framing.

Single‑writer enforcement: `<db_path>/mcp.pid` + Unix flock via `PidFileGuard`. The demo's CLI commands operate against a `/tmp/` DB so they cannot race with the operator's real MCP server.

### 5.3 MCP (`crates/context-graph-mcp/src/`)

Tool registry surface:

- `tools/names.rs` — 110 `pub const` tool names with `llm` enabled (106 without). Append DynamicJEPA names; do not renumber.
- `tools/types.rs` — `ToolDefinition` struct.
- `tools/definitions/<group>.rs` — one file per group; aggregated in `tools/definitions/mod.rs`. Add `dynamicjepa.rs`.
- `handlers/tools/<group>_tools.rs` + `<group>_dtos.rs` — per‑group handler. Add `dynamicjepa_tools.rs` + `dynamicjepa_dtos.rs`.
- `handlers/tools/dispatch.rs` — central tools/call switch. Add a match arm per DynamicJEPA tool.
- Tool count tests in `tools/definitions/tests` — bump expected count by the number of DynamicJEPA tools added.
- Capability matrix handler in `handlers/tools/capability_tools.rs` — extend to include DynamicJEPA enabled flag, CFs, and counts (see `13_mcp_and_cli_surface.md`).

### 5.4 CLI (`crates/context-graph-cli/src/`)

Existing top‑level subcommands (in `commands/`): `setup`, `warmup`, `watch`, `topic`, `divergence`, `learning`, `utl`, `export_training`, `backfill_e14`, `memory`, `session`, `hooks`, and now `dynamicjepa`.

Phase 3 implemented subcommands: `register-domain`, `list-domains`, `get-domain`, `inspect-counts`, and `inspect-cf`. Phase 4 implemented subcommands: `ingest-event`, `run-adapter`, `materialize-panel`, `get-panel`, and `list-instrument-readings`. Phase 5 implemented subcommands: `bind`, `list-bindings`, `compile-trajectories`, `get-trajectory`, and `list-trajectories`. Phase 6 dataset subcommands now present: `compile-dataset`, `get-dataset-shard`, and `inspect-dataset`. All accept `--db <path>` and `--json`; `get-domain` accepts `--domain-version` with default `1.0.0` to avoid colliding with Clap's global `--version` flag. Phase 7+ model/MCP CLI subcommands remain unimplemented.

All FSV‑supporting subcommands accept `--db <path>` and `--json` (machine‑readable JSON output is a hard requirement — operator evidence logs depend on it).

### 5.5 Embeddings & CUDA Crates

The DynamicJEPA crate depends on `context-graph-core`, `context-graph-storage`, and `candle-core` / `candle-nn` directly. It does **not** depend on `context-graph-embeddings` for v1 (the demo's instruments are scalar/categorical/onehot/time only — no text embedders). It does **not** depend on `context-graph-cuda` directly; CUDA goes through candle.

If a later phase wants `text_embedding` instruments (binding panels into the existing 14‑embedder space), that adds a `context-graph-embeddings` dep and is out of demo scope.

## 6. Existing Documentation You Will Use

| Path | Use |
| --- | --- |
| `docs/dynamicjepa/implementationplan/00_master_implementation_plan.md` | Source‑of‑truth contract |
| `docs/dynamicjepa/implementationplan/03_core_data_contracts.md` | Records you trim into v1 |
| `docs/dynamicjepa/implementationplan/04_storage_schema_and_migrations.md` | CF list and key formats |
| `docs/dynamicjepa/implementationplan/12_full_state_verification_manual_tests.md` | FSV template you specialize |
| `docs/dynamicjepa/implementationplan/14_phase_task_breakdown.md` | Phase order |
| `docs2/contextgraphspecs/02_source_code_map.md` | Crate dependency graph |
| `docs2/contextgraphspecs/04_database_schema.md` | Existing CF roster (73 CFs) |
| `docs2/contextgraphspecs/06_search_and_fusion.md` | Reusable retrieval/fusion patterns |
| `docs2/contextgraphspecs/09_storage_layer.md` | Versioning + codec patterns |
| `docs2/contextgraphspecs/14_test_suite.md` | FSV test naming + locations |
| `docs/bigunlock/07_full_state_verification.md` | FSV mentality (older but still authoritative on style) |

You do not need to read the full DynamicJEPA architecture deck (`docs/dynamicjepa/03_*.md` etc) for the demo, but it answers any "why is this contract shaped this way?" questions.

## 7. Existing Models You Will *Not* Use

`models/` contains 14 production embedder weights for the existing Context Graph system. The 5090jepa demo trains a tiny encoder/predictor from scratch in candle and writes new artifacts under `artifacts/dynamicjepa/<domain>/<run_id>/`. Do not reuse existing embedder weights for the demo. Two independent reasons:

1. The demo's purpose is to prove the JEPA loop trains and converges; reusing pre‑trained weights would conflate "did our pipeline learn something" with "are the existing weights any good."
2. Loading a 1.5B parameter `Qodo/Qodo-Embed-1-1.5B` (E7) just to embed a 5×5 grid coordinate is the opposite of Occam's razor.

## 8. Existing Configs You Will Not Edit

`config/default.toml`, `config/development.toml`, `config/production.toml`, `config/test.toml` — these configure the existing MCP server. Do not change them. The demo's domain packs and fixtures live under a *new* directory `configs/dynamicjepa/…` (note plural) — operator‑authored TOML inputs that get hashed and registered into RocksDB.

## 9. Pre‑Existing FSV Patterns To Mirror

These tests in the current tree are good reference shape:

- `crates/context-graph-storage/tests/full_14_embedder_fsv.rs` — full pipeline FSV harness style
- `crates/context-graph-storage/tests/e14_fsv.rs` — single‑embedder end‑to‑end with state diff
- `crates/context-graph-cli/tests/e14_phase_b_full_state_verification.rs` — CLI‑level FSV with JSON evidence
- `crates/context-graph-cli/tests/e2e/full_session_test.rs` — session lifecycle, useful for demo command sequencing

Place new DynamicJEPA FSV tests under each crate's `tests/` directory using `dynamicjepa_*_fsv.rs` naming.

## 10. Drift Audit Reminder

Re‑run Section 3 commands at the start of every phase. If a single command exits non‑zero or returns a different value, halt and resolve before continuing. The cost of one redundant audit is much less than the cost of building on an incorrect mental model.
