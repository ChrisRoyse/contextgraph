# Verification Report

**Source files covered:**
- `Cargo.toml` (workspace + per-crate)
- `crates/**/*.rs`
- `crates/context-graph-storage/src/column_families.rs` — base CF declarations + `TOTAL_COLUMN_FAMILIES`
- `crates/context-graph-storage/src/teleological/column_families.rs` — teleological + learner/training/learning CFs
- `crates/context-graph-storage/src/dynamicjepa/column_families.rs` — DynamicJEPA CFs
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — tool count assertions
- `crates/context-graph-cli/src/commands/blackbox.rs` — `LEDGER_SCHEMA_VERSION`
- `Makefile` — build/test/lint targets
- `scripts/**/*.py` — Python FSV harnesses

Cross-references: `10_cli_reference.md`, `14_test_suite.md`, `08_mcp_tools_reference.md`, `09_storage_layer.md`.

## 1. Codebase Metrics (counted at refresh)

| Metric | Value | Source |
| --- | --- | --- |
| Workspace crates | 11 | `Cargo.toml` `[workspace.members]` |
| Default-members crates | 10 (benchmark excluded — 87K lines / 22%) | `Cargo.toml` `default-members` |
| All `.rs` files in repo | 1,332 | `find . -name '*.rs' -not -path '*/target/*'` |
| `.rs` files under `crates/*/src` | 1,203 | sources |
| `.rs` files under `crates/*/tests` | 233 | integration / FSV |
| Total Rust LOC (workspace) | ~528,400 | `find . -name '*.rs' -exec cat {} \\| wc -l` |
| Source LOC under `crates/*/src` | ~477,100 | sources only |
| Files containing `#[test]` / `#[tokio::test]` | 586 | grep |
| `#[test]` / `#[tokio::test]` attributes | 5,415 | grep |
| FSV-tagged test files | 47 | filename grep |
| Python scripts under `scripts/` | 123 (32 in `scripts/v4_2/`) | filesystem |
| Benchmark binaries declared | 44 `[[bin]]` + 2 `[[bench]]` | `crates/context-graph-benchmark/Cargo.toml` |
| MCP tools (`llm` feature on) | 145 | `tools/definitions/mod.rs:135` |
| MCP tools (`llm` feature off) | 141 | `tools/definitions/mod.rs:137` |
| Total RocksDB column families | 101 | `column_families.rs:408` (`TOTAL_COLUMN_FAMILIES = 101`) |
| Production embedder slots (E1–E14) | 14 | `semantic/constants.rs` (`NUM_EMBEDDERS = 14`) |
| Learner-state slots (E15–E21) | 7 | `learner/mod.rs` (`LEARNER_MODALITY_COUNT = 7`) |
| ModelId variants | 15 (14 production + legacy `Entity`) | enum |
| Schema version (BlackBoxFix ledger) | 8 (`LEDGER_SCHEMA_VERSION`) | `commands/blackbox.rs:25` — Phase 13 ships v7→v8 in source |
| `EPISODE_SCHEMA_VERSION` (DynamicJEPA episode export) | 1 | `commands/export_dynamicjepa.rs:32` |
| `REQUIRED_BUNDLE_SCHEMA_VERSION` (DynamicJEPA bundle) | 2 | `commands/export_dynamicjepa.rs:33` |
| `TRAINING_RECORD_VERSION` | 3 | `core/training/mod.rs:67` |
| `LEARNER_RECORD_VERSION` | 1 | `core/learner/mod.rs:17` |
| `LEARNING_EVENT_VERSION` | 1 | `core/learning/mod.rs:18` |

## 2. Crate Inventory

Source: `Cargo.toml`.

| # | Crate | In default-members | Test files |
| --- | --- | --- | --- |
| 1 | `context-graph-mcp` | yes | 11 |
| 2 | `context-graph-core` | yes | 31 |
| 3 | `context-graph-cuda` | yes | 1 |
| 4 | `context-graph-embeddings` | yes | 113 |
| 5 | `context-graph-storage` | yes | 34 |
| 6 | `context-graph-dynamicjepa` | yes | 1 |
| 7 | `context-graph-cli` | yes | 39 |
| 8 | `context-graph-causal-agent` | yes | 2 |
| 9 | `context-graph-graph-agent` | yes | 1 |
| 10 | `context-graph-test-utils` | yes | 0 (library only) |
| 11 | `context-graph-benchmark` | **no** | (benches + bins) |

## 3. Build Status

| Check | Result | Notes |
| --- | --- | --- |
| `cargo build --release --workspace` | zero errors, zero warnings (per audits #9–#15) | `make build-rust` |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | `make clippy` / `make lint-rust` enforces `-D warnings` |
| Release profile | `lto = "thin"`, `codegen-units = 1`, `strip = true` | `Cargo.toml` `[profile.release]` |
| Rust toolchain | edition 2021, `rust-version = "1.95"` | `Cargo.toml` `[workspace.package]` |

## 4. Most Recent Test Verifications

Source: `/home/cabdru/.claude/projects/-home-cabdru-contextgraph/memory/MEMORY.md`.

| Date | Suite | Tests passed | Failed | Notes |
| --- | --- | --- | --- | --- |
| 2026-02-26 | MCP + Storage (`unified_neighbors` FSV) | 1,179 | 0 | 3 typed-edge bugs found + fixed during the session |
| 2026-02-26 | Full Audit #15 (workspace) | 6,527 | 0 | Build clean, clippy clean |
| 2026-02-22 | Audit #9 + MCP testing | 6,724 | 0 | 28/28 MCP tool tests pass |
| 2026-02-21 | Audits #7/#8 | passing | 0 | E5 / E10 / merge fixes |
| 2026-02-20 | Audits #4–#6 | passing | 0 | 33 + 9 findings fixed |

Approximate total runnable Rust tests: 5,415 attributes (`#[test]` + `#[tokio::test]`); not all are exercised in every audit because some are gated by features (`llm`), CUDA availability, or `#[ignore]`.

## 5. Verification Commands

| Command | Notes |
| --- | --- |
| `cargo build --release --workspace` | Builds default-members; benchmark crate excluded |
| `cargo build -p context-graph-benchmark` | Explicit build of the 87K-line benchmark crate |
| `cargo test --workspace` | Full default-members suite (long-running) |
| `cargo test --release` | Same, release profile |
| `make test` / `make test-rust` / `make test-mcp` / `make test-e2e` | Make wrappers |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Asserts 145 (llm) / 141 (no-llm) |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_submodule_counts` | Per-submodule counts |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint gate |

## 6. Notable Constants

Source: enumerated `pub const` in the listed files.

| Constant | Value | Source |
| --- | --- | --- |
| `TOTAL_COLUMN_FAMILIES` | 101 | `column_families.rs:408` |
| `NUM_EMBEDDERS` | 14 | `semantic/constants.rs`, replicated in retrieval/clustering/teleological |
| `RRF_K` (core / fusion) | `60.0_f32` | `retrieval/config.rs`, `fusion/mod.rs` |
| `RRF_K` (storage) | `60_i32` | `rocksdb_store/fusion.rs` |
| `TOTAL_WEIGHT` | 9.5 | `retrieval/config.rs` |
| `MAX_WEIGHTED_AGREEMENT` | 9.5 | `clustering/manager.rs` |
| `MAX_HNSW_VECTORS_PER_INDEX` | 2,000,000 | `teleological/indexes/hnsw_impl/types.rs:18` |
| `MAX_CAUSAL_NODES` | 500,000 | `causal/scm.rs:195` |
| `MAX_CAUSAL_EDGES` | 2,000,000 | `causal/scm.rs:198` |
| `DEFAULT_MAX_SPARSE_DOCS` | 500,000 | `retrieval/sparse_index.rs:46` |
| `MAX_BATCH_SIZE` (JSON-RPC) | 100 | `mcp/server/transport.rs:36` |
| `MAX_SNAPSHOTS` | 1,440 | `clustering/stability.rs:38` |
| `CAUSAL_THRESHOLD` | 0.04 | causal gate |
| `NON_CAUSAL_THRESHOLD` | 0.008 | causal gate |
| `MAX_LEARNER_STATE_VECTOR_VALUES` | 16,384 | `learner/mod.rs:26` |
| `MAX_LEARNER_LABEL_CHARS` | 256 | `learner/mod.rs:35` |
| `LEARNER_MODALITY_COUNT` | 7 | `learner/mod.rs:20` |
| `MAX_LEARNING_EVENT_MEMORY_IDS` | 1,024 | `learning/mod.rs:21` |
| `MAX_LEARNING_EVENT_TEXT_CHARS` | 65,536 | `learning/mod.rs:24` |
| `MAX_LEARNING_EVENT_LABEL_CHARS` | 256 | `learning/mod.rs:27` |
| `TEMPORAL_EMBEDDERS_ENABLED` | `true` | `weights/mod.rs:42` |
| `E11_ENTITY_ENABLED` | `true` | `weights/mod.rs:48` (re-enabled — older docs said `false`) |
| `TRAINING_RECORD_VERSION` | 3 | `core/training/mod.rs:67` |
| `LEARNER_RECORD_VERSION` | 1 | `core/learner/mod.rs:17` |
| `LEARNING_EVENT_VERSION` | 1 | `core/learning/mod.rs:18` |
| `LEDGER_SCHEMA_VERSION` (BlackBoxFix) | 8 | `commands/blackbox.rs:25` |
| `EPISODE_SCHEMA_VERSION` | 1 | `commands/export_dynamicjepa.rs:32` |
| `REQUIRED_BUNDLE_SCHEMA_VERSION` | 2 | `commands/export_dynamicjepa.rs:33` |
| Daemon default port | 3,100 | `mcp/main.rs:99` |
| `DEFAULT_EXPORT_MAX_EDGES` | 10,000 | typed-edge tools |
| `DEFAULT_VALIDATE_MAX_EDGES` | 100 | typed-edge tools |

## 7. MCP Tool Composition

Source: `crates/context-graph-mcp/src/tools/definitions/mod.rs` `test_submodule_counts`.

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
| Clippy gate | `cargo clippy --workspace --all-targets -- -D warnings` | `Makefile` `lint-rust`/`clippy` |

## 9. 3-Tier Model Routing (ADR-026)

| Tier | Handler | Latency | Cost | Use cases |
| --- | --- | --- | --- | --- |
| 1 | Agent Booster (WASM) | <1 ms | $0 | Simple transforms (var→const, add types) — skip LLM |
| 2 | Haiku | ~500 ms | $0.0002 | Simple tasks, complexity <30% |
| 3 | Sonnet / Opus | 2–5 s | $0.003–$0.015 | Complex reasoning, architecture, security |

## 10. Source-Level Inconsistencies (Tracked)

1. `TELEOLOGICAL_VECTOR_DIM = 13` while `NUM_EMBEDDERS = 14` (E14 deliberately excluded from compact teleological vector).
2. Config schema accepts `sse` transport while `context-graph-mcp` main rejects it (`server::TransportMode` validation).
3. `LEARNER_MODALITY_COUNT = 7` while `LearnerModality` enum has 14 variants; validation permits `LEARNER_MODALITY_COUNT + 1`.
4. CAMD `camd_lib.py` is 13-view; E14 is exposed via extended sidecar scripts only.
5. Some inline comments still reference older counts (13/100/96); current authoritative numbers are above.
6. `E11_ENTITY_ENABLED` was `false` in earlier docs; current source has it `true` (`weights/mod.rs:48`).
7. `LEDGER_SCHEMA_VERSION = 8` in code while older docs / project plan refer to "schema v7" for Phase 13 — Phase 13 lifecycle bumped to v8 in the source-of-truth file.
8. `tests/chaos/mod.rs` is a redirect-only stub; the actual chaos tests previously lived in a `context-graph-graph` crate that is no longer present in the workspace (workspace is now 11 crates). Effective coverage of the chaos suite is currently zero — flagged for follow-up.

## 11. Build Hygiene Tooling

| Tool | Purpose |
| --- | --- |
| `scripts/trim-stale-deps.sh` | Run after every `make build*`/`test*` to bound `target/` |
| `scripts/disk-guard.sh` | Disk usage guardrail invoked by `make disk-check` |
| `scripts/clean-build-artifacts.sh` | `--check`/`--aggressive` cleanup |
| `scripts/5090jepa/prepare_release_candidate_archive.sh` (+ `_v2`) | 5090 JEPA release archive |

## 12. What Is *Not* Verified Here

- Coverage percentages (no instrumentation in this workspace; counts above are static + audit-reported).
- Live runtime CUDA availability (RTX 5090 / CUDA 13.2 documented; not exercised by docs build).
- Python tests under `clipcannon/` (run separately via `make test-python` / `make test-python-smoke`).
- External script behaviour (Python FSV scripts under `scripts/v4_2/`).
- MCP tools' end-to-end behaviour beyond their schema/count assertions (covered separately by `tools::definitions::tests::test_submodule_counts`, MCP+Storage FSV, and the audits enumerated in `docs/HIDDEN_BROKENNESS_AUDIT_*.md`).
