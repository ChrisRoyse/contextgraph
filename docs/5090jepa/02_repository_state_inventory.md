# 02 — Repository State Inventory

This file is the anti‑hallucination anchor. Re‑run the audit in Section 3 before Phase 0 and again before Phase 7. If anything below is wrong relative to the live tree, the live tree wins; update this file *before* writing code that depends on a stale fact.

## 1. Branch & Commit Snapshot (Phase 3 Live Audit: 2026-04-30)

| Property | Value |
| --- | --- |
| Branch | `main` |
| Latest commits at audit start | `a43c962f docs(5090jepa): add master context guide and AI prompt library` → `bcf6a297 feat(dynamicjepa): add Phase 2 storage data plane (25 RocksDB CFs)` → `d47b0e92 docs(5090jepa): add game-theory analysis of commercial opportunity` |
| Working tree at audit start | Clean before Phase 2 verification fixes; local changes now include stale CF/HNSW count fixes, Phase 3 CLI implementation/tests, stale CLI test fixes, and this status documentation |
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
test -f crates/context-graph-cli/src/bin/context-graph.rs

# Configs root
ls config/                                # operator config (existing)
test -d configs/dynamicjepa              # Phase 0 input fixtures
```

If those checks fail, the implementation phase has drifted. Read the live files before adding parallel work.

## 4. Phase 2 And Phase 3 Verification Status (2026-04-30)

Phase 2 is implemented and manually verified as the storage data plane. Phase 3 is implemented and manually verified as the CLI domain registry / inspection layer. Phase 4+ adapter/panel/MCP/model pipeline work is still not present.

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
- No Phase 4+ CLI commands are present yet; the next implementation phase starts at `ingest-event`, adapter runs, normalized records, readings, and panels.

Root-cause fixes made during the Phase 3 audit:

- `get-domain` now uses `--domain-version` because Clap's propagated global `--version` flag reserves the `version` argument id.
- The CLI package exposes both `context-graph-cli` and the documented `context-graph` binary without Cargo duplicate-target warnings.
- Phase 3 `count_delta` now treats missing or unexpected CF names in before/after snapshots as `StorageInvariantViolation` instead of substituting zero.
- `crates/context-graph-cli/tests/export_training_integration.rs` now initializes the current `TrainingRecord::e14_bge_m3_dense` field.
- `crates/context-graph-cli/tests/e14_phase_b_fsv.rs` now calls `EmbedderId::parse`, matching the live DTO API.

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

Phase 3 implemented subcommands: `register-domain`, `list-domains`, `get-domain`, `inspect-counts`, and `inspect-cf`. All accept `--db <path>` and `--json`; `get-domain` accepts `--domain-version` with default `1.0.0` to avoid colliding with Clap's global `--version` flag. Phase 4+ CLI subcommands remain unimplemented.

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
