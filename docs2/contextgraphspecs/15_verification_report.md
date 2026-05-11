# Verification Report

**Source files covered:**
- `Cargo.toml` (workspace + per-crate)
- `crates/**/*.rs`
- `crates/context-graph-storage/src/column_families.rs` — base CF declarations + `TOTAL_COLUMN_FAMILIES`
- `crates/context-graph-storage/src/teleological/column_families.rs` — `TELEOLOGICAL_CF_COUNT`, `QUANTIZED_EMBEDDER_CF_COUNT`, `CODE_CF_COUNT`, `CAUSAL_CF_COUNT`
- `crates/context-graph-storage/src/dynamicjepa/column_families.rs` — `DJ_CF_COUNT` (28)
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — tool-count assertions (lines 138-224)
- `crates/context-graph-cli/src/commands/blackbox.rs` — `LEDGER_SCHEMA_VERSION`
- `crates/context-graph-cli/src/commands/export_dynamicjepa.rs` — `EPISODE_SCHEMA_VERSION`, `REQUIRED_BUNDLE_SCHEMA_VERSION`
- `crates/context-graph-core/src/{learner,learning,training}/mod.rs` — record version constants
- `crates/context-graph-core/src/dynamicjepa/*.rs` — DJ record version constants (16 distinct versioned record kinds)
- `crates/context-graph-cli/src/bin/reality-loop.rs` — Rust reality-loop binary (see [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md))
- `Makefile` — build/test/lint targets
- `rust-toolchain.toml` — toolchain pin
- `scripts/v4_2/*.py` — Python FSV harnesses

Cross-references: [10_cli_reference.md](10_cli_reference.md), [14_test_suite.md](14_test_suite.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [09_storage_layer.md](09_storage_layer.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md), [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md), [04_database_schema.md](04_database_schema.md).

---

## 1. Codebase Metrics

Counts produced by `find` / `grep` / `wc` on the working tree (2026-05-08 snapshot, `ccreality-cutover` branch).

| Metric | Value | Source command |
| --- | --- | --- |
| Workspace crates | 11 | `Cargo.toml` `[workspace.members]` |
| Default-members crates | 10 (benchmark excluded) | `Cargo.toml` `default-members` |
| `.rs` files in repo (excl. `target/`) | 1,361 | `find . -name '*.rs' -not -path '*/target/*'` |
| `.rs` files under `crates/*/src` | 1,219 | `find crates -name '*.rs' -path '*/src/*'` |
| `.rs` files under `crates/*/tests` | 246 | `find crates -name '*.rs' -path '*/tests/*'` |
| Total Rust LOC under `crates/` | 581,933 | `find crates -name '*.rs' \| xargs wc -l` |
| Source LOC (`crates/*/src`) | 523,153 | `find crates -name '*.rs' -path '*/src/*' \| xargs wc -l` |
| Integration-test LOC (`crates/*/tests`) | 68,783 | `find crates -name '*.rs' -path '*/tests/*' \| xargs wc -l` |
| Files containing `#[test]` / `#[tokio::test]` | 699 | `grep -rln '#\[test\]\|#\[tokio::test\]' crates` |
| `#[test]` / `#[tokio::test]` attributes | 5,524 | `grep -rn '#\[test\]\|#\[tokio::test\]' crates \| wc -l` |
| FSV-tagged test files (`*_fsv.rs`) | 46 | `find crates -name '*_fsv.rs'` |
| Python scripts (`scripts/`) | 126 | `find scripts -name '*.py'` |
| Python scripts (`scripts/v4_2/`) | 32 (9 are `*_fsv.py`) | `find scripts/v4_2 -maxdepth 1 -name '*.py'` |
| Benchmark crate `[[bin]]` targets | 31 | `crates/context-graph-benchmark/Cargo.toml` |
| Benchmark crate `[[bench]]` targets | 2 | `comparative_suite`, `scaling_analysis` |
| Storage `[[bench]]` targets | 4 | `maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench` |
| CLI `[[bench]]` targets | 1 | `cli_benchmarks` |
| CLI `[[test]]` declarations | 2 | `integration` (path `tests/integration/mod.rs`), `e2e` (`tests/e2e/mod.rs`) |
| MCP tools (`llm` feature on) | 145 | `tools/definitions/mod.rs:147` |
| MCP tools (`llm` feature off) | 141 | `tools/definitions/mod.rs:149` |
| Reality-loop tool registry size | 36 | `def(` count in `tools/definitions/reality.rs` |
| Total RocksDB column families | 101 | `column_families.rs:408` (`TOTAL_COLUMN_FAMILIES = 101`) |
| Composition: base + teleological + quantized + code + causal + DJ | 11 + 41 + 14 + 5 + 2 + 28 = 101 | `TOTAL_COLUMN_FAMILIES` arithmetic |
| Production embedder slots (E1–E14) | 14 | `NUM_EMBEDDERS` (replicated in 5 source files) |
| Learner-state slots (E15–E21) | 7 | `learner/mod.rs:20` (`LEARNER_MODALITY_COUNT`) |
| ModelId variants | 15 (14 production + legacy `Entity`) | enum |

LOC is dominated by `context-graph-embeddings` (per-model code + tests, 473 `.rs` files) and `context-graph-core` (312 `.rs` files including the entire dynamicjepa core domain).

---

## 2. Crate Inventory

Source: `Cargo.toml` workspace manifest.

| # | Crate | In `default-members` | `.rs` total | Test files | Notes |
| --- | --- | --- | ---: | ---: | --- |
| 1 | `context-graph-mcp` | yes | 119 | 11 | MCP server, JSON-RPC stdio + TCP, `mcp__cgreality__*` tool surface |
| 2 | `context-graph-core` | yes | 312 | 31 | Fingerprints, retrieval, RRF fusion, learning, learner, training, dynamicjepa core types |
| 3 | `context-graph-cuda` | yes | 20 | 1 | CUDA bindings, native cubin smoke |
| 4 | `context-graph-embeddings` | yes | 473 | 113 | E1–E14 embedders + warm provider + roundtrip suites |
| 5 | `context-graph-storage` | yes | 158 | 34 + embedded `src/teleological/...` test trees | RocksDB store, HNSW indexes, teleological / dynamicjepa CFs |
| 6 | `context-graph-dynamicjepa` | yes | 8 | 1 | Tiny JEPA stack (CUDA smoke) |
| 7 | `context-graph-cli` | yes | 95 | 52 | BlackBoxFix, dynamicjepa export, reality-loop, hooks |
| 8 | `context-graph-causal-agent` | yes | 14 | 2 | Causal agent integration |
| 9 | `context-graph-graph-agent` | yes | 13 | 1 | Scanner integration |
| 10 | `context-graph-test-utils` | yes | 3 | 0 | Library-only (real fingerprint generators + store factories) |
| 11 | `context-graph-benchmark` | **no** | 142 | 0 | 31 bins + 2 benches (excluded — ~87K LOC) |

CLI binaries declared by `crates/context-graph-cli/Cargo.toml`:

| `[[bin]] name` | Path | Purpose |
| --- | --- | --- |
| `context-graph-cli` | `src/main.rs` | Standard MCP/CLI entry |
| `context-graph` | `src/bin/context-graph.rs` | Multi-subcommand binary (BlackBoxFix, dynamicjepa, etc.) |
| `blackbox` | `src/bin/blackbox.rs` | Standalone BlackBoxFix binary |

Note: `reality-loop` is no longer declared as a `[[bin]]` in `crates/context-graph-cli/Cargo.toml`; the binary lives at `src/bin/reality-loop.rs` (18,137 lines) and is invoked via `cargo run --release --bin reality-loop` because Cargo auto-discovers files under `src/bin/`. See [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md) for the operator contract.

`crates/context-graph-mcp/Cargo.toml` declares one `[[bin]]`; `crates/context-graph-embeddings/Cargo.toml` declares one `[[bin]]`.

---

## 3. Build Status

| Check | Result | Notes |
| --- | --- | --- |
| `cargo build --release --workspace` | zero errors, zero warnings (per audits #9–#15) | `make build-rust` chains `scripts/trim-stale-deps.sh release` |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | `make clippy` / `make lint-rust` enforces `-D warnings` |
| Release profile | `lto = "thin"`, `codegen-units = 1`, `strip = true` | `Cargo.toml` `[profile.release]` |
| Dev profile | `opt-level = 0`, `debug = 1`, `incremental = true`, `split-debuginfo = "unpacked"`; external deps `debug = false` | `Cargo.toml` `[profile.dev]` + `[profile.dev.package."*"]` |
| Toolchain | `channel = "stable"`, components `rustfmt`, `clippy`, `rust-src`; target `x86_64-unknown-linux-gnu` | `rust-toolchain.toml` |
| Edition / MSRV | edition `2021`, `rust-version = "1.95"` | `Cargo.toml` `[workspace.package]` |

No `clippy.toml` or `rustfmt.toml` is present at the repo root or in any crate (lint configuration is the Rust default; the gate is the workspace `-D warnings` in `make clippy`).

---

## 4. Most Recent Test Verifications

Source: `~/.claude/projects/-home-cabdru-contextgraph/memory/MEMORY.md` audit history.

| Date | Suite | Tests passed | Failed | Notes |
| --- | --- | ---: | ---: | --- |
| 2026-02-26 | MCP + Storage (`unified_neighbors` FSV) | 1,179 | 0 | 3 typed-edge bugs found + fixed during the session |
| 2026-02-26 | Full Audit #15 (workspace) | 6,527 | 0 | Build clean, clippy clean |
| 2026-02-22 | Audit #9 + MCP testing | 6,724 | 0 | 28/28 MCP tool tests pass |
| 2026-02-21 | Audits #7/#8 | passing | 0 | E5 / E10 / merge fixes |
| 2026-02-20 | Audits #4–#6 | passing | 0 | 33 + 9 findings fixed |

The static `#[test]` + `#[tokio::test]` count today is 5,524; the audit-time totals diverged because (a) audits exclude `#[ignore]`-gated tests, (b) the `llm` feature toggles graph + causal-discovery tests, (c) the `cuda` feature toggles GPU-only paths, and (d) tests have been added since the most recent audit.

---

## 5. Verification Commands

| Command | Notes |
| --- | --- |
| `cargo build --release --workspace` | Builds default-members; benchmark crate excluded |
| `cargo build -p context-graph-benchmark` | Explicit build of the 87K-line benchmark crate |
| `cargo build --release -p context-graph-cli --bin reality-loop` | Build reality-loop binary ([22](22_reality_loop_subsystem.md)) |
| `cargo test --workspace` | Full default-members suite |
| `cargo test --release` | Same, release profile |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Asserts 145 (llm) / 141 (no-llm) |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_submodule_counts` | Per-submodule counts |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_reality_loop_lightweight_registry_only_lists_callable_tools` | Asserts the 36-tool reality-loop registry |
| `cargo test -p context-graph-storage dynamicjepa::tests` | Asserts `DJ_CF_COUNT == 28` |
| `cargo bench --package context-graph-storage` | Runs `maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint gate |
| `make test` / `make test-rust` / `make test-mcp` / `make test-e2e` | Make wrappers |

---

## 6. Notable Constants

Source: enumerated `pub const` / `const` definitions in the listed files.

### Column-family / embedder counts

| Constant | Value | Source |
| --- | --- | --- |
| `TOTAL_COLUMN_FAMILIES` | 101 | `column_families.rs:408` |
| `TELEOLOGICAL_CF_COUNT` | 41 | `teleological/column_families.rs:657` |
| `QUANTIZED_EMBEDDER_CF_COUNT` | 14 | `teleological/column_families.rs:746` |
| `CODE_CF_COUNT` | 5 | `teleological/column_families.rs:1887` |
| `CAUSAL_CF_COUNT` | 2 | `teleological/column_families.rs:2057` |
| `DJ_CF_COUNT` | 28 | `dynamicjepa/column_families.rs:74` |
| `NUM_EMBEDDERS` | 14 | replicated in `embeddings/storage/types/constants.rs:27`, `storage/teleological/indexes/hnsw_config/constants.rs:48`, `embeddings/models/pretrained/weight_projection/constants.rs:8`, `core/teleological/types.rs:217`, `cli/commands/hooks/session_state.rs:10` |
| `LEARNER_MODALITY_COUNT` | 7 | `core/learner/mod.rs:20` |

### Schema / record versions

| Constant | Value | Source |
| --- | --- | --- |
| `LEDGER_SCHEMA_VERSION` (BlackBoxFix SQLite ledger) | 14 | `cli/commands/blackbox.rs:40` |
| `EPISODE_SCHEMA_VERSION` (DynamicJEPA episode export) | 1 | `cli/commands/export_dynamicjepa.rs:32` |
| `REQUIRED_BUNDLE_SCHEMA_VERSION` (DynamicJEPA bundle) | 2 | `cli/commands/export_dynamicjepa.rs:33` |
| `TRAINING_RECORD_VERSION` | 3 | `core/training/mod.rs:67` |
| `LEARNER_RECORD_VERSION` | 1 | `core/learner/mod.rs:17` |
| `LEARNING_EVENT_VERSION` | 1 | `core/learning/mod.rs:18` |
| `LEARNER_TRAINING_DATASET_VERSION` | 1 | `core/learner_training.rs:20` |
| `CONSTELLATION_VERSION` | 2 | `core/constellation/mod.rs:55` |
| `CONTRASTIVE_PAIR_VERSION` | 1 | `core/contrastive/mod.rs:49` |
| `TYPED_EDGE_RECORD_VERSION` | 1 | `core/typed_edge_export/mod.rs:32` |
| `LLM_EDGE_VALIDATION_VERSION` | 1 | `core/llm_edge_validation/mod.rs:25` |
| `GRAPH_EDGE_VERSION` | 1 | `storage/graph_edges/serialization.rs:16` |
| `TELEOLOGICAL_VERSION` | 1 | `storage/teleological/serialization.rs:35` |

### DynamicJEPA record versions (`core/src/dynamicjepa/*.rs`)

| Constant | Value | Source |
| --- | --- | --- |
| `DOMAIN_PACK_RECORD_VERSION` | 1 | `domain_pack.rs:14` |
| `RAW_DOMAIN_EVENT_RECORD_VERSION` | 1 | `event.rs:7` |
| `NORMALIZED_STATE_RECORD_VERSION` | 1 | `state_action_outcome.rs:8` |
| `NORMALIZED_ACTION_RECORD_VERSION` | 1 | `state_action_outcome.rs:9` |
| `NORMALIZED_OUTCOME_RECORD_VERSION` | 1 | `state_action_outcome.rs:10` |
| `STATE_TRANSITION_RECORD_VERSION` | 1 | `state_action_outcome.rs:11` |
| `INSTRUMENT_READING_RECORD_VERSION` | 1 | `instrument.rs:10` |
| `BINDING_RECORD_VERSION` | 1 | `binding.rs:8` |
| `TRAJECTORY_RECORD_VERSION` | 1 | `trajectory.rs:8` |
| `PLAN_TRACE_RECORD_VERSION` | 2 | `planner.rs:11` |
| `SURPRISE_EVENT_RECORD_VERSION` | 1 | `planner.rs:12` |
| `PAIRWISE_READING_RECORD_VERSION` | 1 | `meaning_compression.rs:12` |
| `CONSTELLATION_CENTROID_RECORD_VERSION` | 1 | `meaning_compression.rs:13` |
| `THRESHOLD_CALIBRATION_RECORD_VERSION` | 1 | `meaning_compression.rs:14` |
| `MODEL_ARTIFACT_RECORD_VERSION` | 1 | `artifact.rs:8` |
| `LATENT_PANEL_RECORD_VERSION` | 2 | `panel.rs:10` |
| `SKILL_POLICY_RECORD_VERSION` | 1 | `skill.rs:7` |
| `TRAINING_RUN_RECORD_VERSION` | 1 | `predictor.rs:10` |
| `PREDICTION_RECORD_VERSION` | 1 | `predictor.rs:11` |
| `GUARD_DECISION_RECORD_VERSION` | 2 | `guard.rs:9` |
| `ADAPTER_RUN_RECORD_VERSION` | 1 | `adapter.rs:11` |
| `VERIFICATION_RUN_RECORD_VERSION` | 1 | `verification.rs:9` |
| `DATASET_SHARD_RECORD_VERSION` | 2 | `dataset.rs:7` |
| `DJ_AUDIT_RECORD_VERSION` | 2 | `storage/dynamicjepa/audit.rs:7` |
| `DYNAMIC_JEPA_STORAGE_VALUE_VERSION` | 1 | `storage/dynamicjepa/encode.rs:9` |
| `COUNTER_TO_GRID_DOMAIN_VERSION` | `"1.0.0"` | `storage/dynamicjepa/bridge.rs:4` |

### Reality-loop schemas and defaults (`crates/context-graph-cli/src/bin/reality-loop.rs`)

| Constant | Value | Source line |
| --- | --- | --- |
| `DEFAULT_MODEL` | `"qwen3-coder:30b-a3b"` | 28 |
| `DEFAULT_GPU_MIN_FREE_MB` | 1,536 | 40 |
| `DEFAULT_OFFICIAL_DOCKER_HOST` | `"unix:///var/run/docker.sock"` | 68 |
| `DATASET_NAME` | `"princeton-nlp/SWE-bench_Lite"` | 72 |
| `DYNAMICJEPA_REALITY_DOMAIN_PACK` | `"contextgraph_swe_reality_loop_v1"` | 74 |
| Measurement contract schema | `"contextgraph_reality_measurement_contract_v1"` | 2337 |
| Reality-loop summary schema | `"contextgraph_rust_reality_loop_summary_v1"` | 15023 |
| Problem packet record kind | `"contextgraph_swe_problem_reality_packet"` | 2482 |
| Inner-harness model-readback record kind | `"contextgraph_inner_harness_model_capability_readback"` | 3682 |
| Agent tool transcript record kind | `"contextgraph_agent_tool_transcript"` | 4769 |
| Reward spec schema version | 1 | `config/reward_spec.verified_outcome.json` |

Note: `DEFAULT_MODEL` in source is `qwen3-coder:30b-a3b`; the operator-facing model identifier referenced in `CLAUDE.md` is `qwen3.6:27b`. This is a known source-of-truth divergence — the loop's CLI accepts `--model` to override at runtime.

### Retrieval / fusion / capacity

| Constant | Value | Source |
| --- | --- | --- |
| `RRF_K` (core / fusion) | `60.0_f32` | `core/fusion/mod.rs:41` |
| `RRF_K` (storage rocksdb_store) | `60_i32` | `storage/teleological/rocksdb_store/fusion.rs:49` |
| `TOTAL_WEIGHT` | `9.5` | `core/retrieval/config.rs:61` |
| `MAX_RECENT_MEMORIES` | 50 | `core/retrieval/config.rs:35` |
| `MAX_HNSW_VECTORS_PER_INDEX` | 2,000,000 | `storage/teleological/indexes/hnsw_impl/types.rs:18` |
| `MAX_CAUSAL_NODES` | 500,000 | `core/causal/scm.rs:195` |
| `MAX_CAUSAL_EDGES` | 2,000,000 | `core/causal/scm.rs:198` |
| `DEFAULT_MAX_SPARSE_DOCS` | 500,000 | `core/retrieval/sparse_index.rs:46` |
| `MAX_BATCH_SIZE` (JSON-RPC) | 100 | `mcp/server/transport.rs:36` |
| `MAX_SNAPSHOTS` | 1,440 | `core/clustering/stability.rs:38` |
| `MAX_LEARNER_STATE_VECTOR_VALUES` | 16,384 | `core/learner/mod.rs:26` |
| `MAX_LEARNER_LABEL_CHARS` | 256 | `core/learner/mod.rs:35` |
| `MAX_LEARNING_EVENT_MEMORY_IDS` | 1,024 | `core/learning/mod.rs:21` |
| `MAX_LEARNING_EVENT_TEXT_CHARS` | 65,536 | `core/learning/mod.rs:24` |
| `MAX_LEARNING_EVENT_LABEL_CHARS` | 256 | `core/learning/mod.rs:27` |
| `TEMPORAL_EMBEDDERS_ENABLED` | `true` | `core/weights/mod.rs:42` |
| `E11_ENTITY_ENABLED` | `true` | `core/weights/mod.rs:48` |

---

## 7. MCP Tool Composition

Source: `crates/context-graph-mcp/src/tools/definitions/mod.rs:172-209` (`test_submodule_counts`).

| Submodule | Tools |
| --- | --- |
| `capability` | 1 |
| `core` | 5 |
| `merge` | 1 |
| `curation` | 2 |
| `topic` | 4 |
| `file_watcher` | 4 |
| `sequence` | 4 |
| `causal` | 4 |
| `keyword` | 1 |
| `code` | 1 |
| `robustness` | 1 |
| `entity` | 6 |
| `embedder` | 7 + 2 standalone |
| `temporal` | 2 |
| `graph_link` | 4 |
| `maintenance` | 1 |
| `provenance` | 3 |
| `daemon` | 1 |
| `training` | 4 |
| `learning` | 14 |
| `utl` | 18 |
| `constellation` | 6 |
| `contrastive` | 4 |
| `typed_edges` | 4 |
| `dynamicjepa` | 35 |
| `graph` | 4 (llm) / 2 (no-llm) — 2 base + 2 LLM-gated |
| `causal_discovery` | 2 (llm) / 0 (no-llm) |
| **Total** | **145 (llm) / 141 (no-llm)** |
| Reality-loop registry | 36 (separate, lightweight `--mode reality-loop` server) |

The reality-loop registry is asserted by `test_reality_loop_lightweight_registry_only_lists_callable_tools`; it intentionally excludes `search_graph`, `get_memory_neighbors`, and `get_audit_trail`.

---

## 8. Lint / Profile Configuration

| Setting | Value | Source |
| --- | --- | --- |
| `[profile.release].lto` | `"thin"` | root `Cargo.toml` |
| `[profile.release].codegen-units` | 1 | root |
| `[profile.release].strip` | `true` | root |
| `[profile.dev].opt-level` | 0 | root |
| `[profile.dev].debug` | 1 (line info only) | root |
| `[profile.dev].incremental` | `true` | root |
| `[profile.dev].split-debuginfo` | `"unpacked"` | root |
| `[profile.dev.package."*"].debug` | `false` | external deps |
| Toolchain channel | `stable` | `rust-toolchain.toml` |
| Toolchain components | `rustfmt`, `clippy`, `rust-src` | `rust-toolchain.toml` |
| Toolchain target | `x86_64-unknown-linux-gnu` | `rust-toolchain.toml` |
| Workspace edition | `2021` | `Cargo.toml` `[workspace.package]` |
| Workspace `rust-version` | `1.95` | `Cargo.toml` `[workspace.package]` |
| Workspace license | `PolyForm-Noncommercial-1.0.0` | `Cargo.toml` `[workspace.package]` |
| Clippy gate | `cargo clippy --workspace --all-targets -- -D warnings` | `Makefile` `lint-rust`/`clippy` |
| `clippy.toml` / `rustfmt.toml` | not present | repo defaults |

---

## 9. 3-Tier Model Routing (ADR-026, design reference)

| Tier | Handler | Latency | Cost | Use cases |
| --- | --- | --- | --- | --- |
| 1 | Agent Booster (WASM) | <1 ms | $0 | Simple transforms (var→const, add types) — skip LLM |
| 2 | Haiku | ~500 ms | $0.0002 | Simple tasks, complexity <30% |
| 3 | Sonnet / Opus | 2–5 s | $0.003–$0.015 | Complex reasoning, architecture, security |

This tiering is referenced by the design (ADR-026) but is not currently implemented in the workspace as a runtime decision; the production MCP server routes all LLM traffic through the local llama.cpp / Ollama paths.

---

## 10. Source-Level Inconsistencies (Tracked)

1. `TELEOLOGICAL_VECTOR_DIM = 13` while `NUM_EMBEDDERS = 14` (E14 deliberately excluded from the compact teleological vector).
2. Config schema accepts `sse` transport while `context-graph-mcp` main rejects it (`server::TransportMode` validation).
3. `LEARNER_MODALITY_COUNT = 7` while `LearnerModality` enum has 14 variants; validation permits `LEARNER_MODALITY_COUNT + 1`.
4. CAMD `camd_lib.py` is 13-view; E14 is exposed via extended sidecar scripts only.
5. `NUM_EMBEDDERS = 14` is declared in five separate files (`embeddings/storage/types/constants.rs`, `storage/teleological/indexes/hnsw_config/constants.rs`, `embeddings/models/pretrained/weight_projection/constants.rs`, `core/teleological/types.rs`, `cli/commands/hooks/session_state.rs`). All five agree at HEAD.
6. `RRF_K` is declared twice with different types: `f32 = 60.0` in `core/fusion/mod.rs:41` and `i32 = 60` in `storage/teleological/rocksdb_store/fusion.rs:49`. Both use the same numerical value.
7. `E11_ENTITY_ENABLED` was `false` in earlier docs; current source has it `true` (`core/weights/mod.rs:48`).
8. `LEDGER_SCHEMA_VERSION = 14` in current source. Earlier docs referenced v7/v8; the BlackBoxFix Phase 13–24 expansion bumped through v14 in the source-of-truth file.
9. The legacy root-level `tests/` tree was removed after the stale chaos redirect was found to point at a non-existent `context-graph-graph` crate. Chaos coverage must be reintroduced as real Cargo-discovered tests under the crate that owns the behavior.
10. The `gemmafrontend/` Python harness directory was removed in the `ccreality-cutover` branch. The Rust `reality-loop` binary (`crates/context-graph-cli/src/bin/reality-loop.rs`, 18,137 LOC) is now the only ContextGraph SWE-bench refinement implementation.
11. `reality-loop.rs:28` has `DEFAULT_MODEL = "qwen3-coder:30b-a3b"`; `CLAUDE.md` and `.claude/settings.json` reference `qwen3.6:27b`. The CLI `--model` flag overrides at runtime.

---

## 11. Build Hygiene Tooling

| Tool | Purpose |
| --- | --- |
| `scripts/trim-stale-deps.sh` | Run after every `make build*` / `test*` to bound `target/` |
| `scripts/disk-guard.sh` | Disk-usage guardrail invoked by `make disk-check` |
| `scripts/clean-build-artifacts.sh` | `--check` / `--aggressive` cleanup |
| `scripts/5090jepa/prepare_release_candidate_archive.sh` (+ `_v2`) | 5090 JEPA release archive |

Makefile header documents the rationale: "the 11-crate workspace produces ~600 MB binaries per build; without trimming, target/ grows to 240 GB+ within a few weeks."

---

## 12. What Is *Not* Verified Here

- Coverage percentages — there is no instrumentation in this workspace; counts above are static (`find` + `grep` + Cargo declarations) plus self-reported audit totals.
- Live runtime CUDA availability (RTX 5090 / CUDA 13.2 documented; not exercised by this docs build).
- Python tests under `clipcannon/` (run separately via `make test-python` / `make test-python-smoke`).
- External script behaviour (Python FSV scripts under `scripts/v4_2/`).
- MCP tools' end-to-end behaviour beyond schema/count assertions (covered by `tools::definitions::tests::test_submodule_counts`, MCP+Storage FSV, and the audits enumerated in `docs/HIDDEN_BROKENNESS_AUDIT_*.md`).
- Reality-loop end-to-end execution — requires a real Docker daemon and the official SWE-bench harness installed at `/home/cabdru/.cache/contextgraph-swebench/venv`; the loop refuses to run without `--official-eval-each-attempt`. See [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md) for the operator contract.
