# System Overview

**Source files covered:**
- `Cargo.toml` (workspace manifest, default-members)
- `crates/*/Cargo.toml` (per-crate manifests, features, deps)
- `crates/context-graph-mcp/src/main.rs` (entry, CLI, daemon spawn)
- `crates/context-graph-mcp/src/lib.rs` (lib surface)
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` (tool registry, `get_tool_definitions`)
- `crates/context-graph-mcp/src/handlers/core/dispatch.rs` (method dispatch)
- `crates/context-graph-mcp/src/server/mod.rs` (`TransportMode`)
- `crates/context-graph-storage/src/lib.rs` (storage public surface)
- `crates/context-graph-storage/src/column_families.rs:408` (`TOTAL_COLUMN_FAMILIES = 101`)
- `crates/context-graph-storage/src/teleological/column_families.rs:657,746,1887,2057` (`TELEOLOGICAL_CF_COUNT = 41`, `QUANTIZED_EMBEDDER_CF_COUNT = 14`, `CODE_CF_COUNT = 5`, `CAUSAL_CF_COUNT = 2`)
- `crates/context-graph-storage/src/dynamicjepa/column_families.rs:74` (`DJ_CF_COUNT = 28`)
- `crates/context-graph-core/src/lib.rs`, `crates/context-graph-core/src/error/{unified,sub_errors}.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs:81` (`NUM_EMBEDDERS = 14`)
- `crates/context-graph-core/src/types/fingerprint/teleological/types.rs:12` (`TELEOLOGICAL_VECTOR_DIM = 13`)
- `crates/context-graph-core/src/weights/mod.rs:171` (`WEIGHT_PROFILES`, 20 entries)
- `crates/context-graph-embeddings/src/lib.rs`, `src/types/model_id/core.rs:200,234` (`ModelId::all()` returns 15, `production()` returns 14)
- `crates/context-graph-embeddings/src/learner/mod.rs:25,28` (`LEARNER_EMBEDDER_COUNT = 7`, `UTL_PLANNED_TOTAL_EMBEDDERS = 21`)
- `crates/context-graph-causal-agent/src/llm/mod.rs` (Hermes-2-Pro-Mistral-7B GGUF + GBNF grammars)
- `crates/context-graph-cli/src/bin/reality-loop.rs` (Rust SWE-bench refinement loop)
- `gemmafrontend/server.py` (Python evidence console)
- `Makefile`, `rust-toolchain.toml`, `config/*.toml`

## 1. Summary
Context Graph is an 11-crate Rust workspace (1,346 `.rs` files across `crates/` and `tests/`, excluding `target/`) that stores text/code/learner memories as 14-embedder semantic fingerprints in a single RocksDB database (101 column families) and exposes them through 141/145 Model Context Protocol tools (count without/with `llm` feature; `crates/context-graph-mcp/src/tools/definitions/mod.rs:131-198`). The MCP server (`crates/context-graph-mcp/src/main.rs`) accepts JSON-RPC 2.0 over stdio or TCP, optionally running as a long-lived daemon that other terminals proxy into. Each memory is fingerprinted by up to 14 production embedders (E1-E14 per `ModelId::production()`), indexed by per-embedder HNSW (usearch) or sparse inverted indexes, and linked through K-NN edges, typed edges, and causal/graph relationships discovered by a local Hermes-2-Pro-Mistral-7B GGUF model via `llama-cpp-2`. A 7-slot learner namespace (E15-E21) plus the `context-graph-dynamicjepa` crate ship Phase 2-8 of the 5090jepa learning programme: instrument readings, latent panels, predictions, plans, surprise events, and tiny-model artifacts persisted in 28 DynamicJEPA column families. A Rust-owned SWE-bench reality loop (`reality-loop` binary) and a Python evidence console (`gemmafrontend/`) live alongside the workspace and exercise the MCP/CLI surface end-to-end against local Ollama models with official SWE-bench Docker readback.

## 2. Architecture
| Process / surface | Address | Technology | Purpose |
| --- | --- | --- | --- |
| `context-graph-mcp` (stdio mode) | inherited stdin/stdout | tokio + JSON-RPC 2.0 | Default MCP transport for editor/agent clients |
| `context-graph-mcp` (tcp mode) | `bind_address`:`tcp_port` (default `127.0.0.1:3100`) | tokio TCP, line-delimited JSON | Networked MCP transport |
| `context-graph-mcp --daemon` (server side) | TCP `:3100` (or `--daemon-port`) | Detached child via `spawn_daemon_process` | Single shared server holding GPU-resident models |
| `context-graph-mcp --daemon` (client side) | stdio ↔ TCP relay | `crate::adapters::DaemonProxy` (referenced in main.rs) | Forwards stdio JSON-RPC into the running daemon |
| RocksDB database | `storage.path` from config (typ. `<repo>/contextgraph_data/`) | `rocksdb 0.22`, 101 CFs, shared 256MB block cache | Sacred durable store for memories, edges, learning, DynamicJEPA |
| HNSW indexes | in-memory (`usearch 2`) | one index per dense embedder + asymmetric duals (E5/E8/E10) | K-NN over 14-embedder fingerprint vectors; rebuilt from CFs on startup |
| Sparse inverted indexes | RocksDB CFs `e6_sparse_inverted`, `e13_splade_inverted` | Custom inverted index (`crates/context-graph-storage/src/teleological/indexes`) | Stage-1 recall for E6/E13 |
| Late-interaction MaxSim | RocksDB CF `e12_late_interaction` + `compute_maxsim_direct` | E12 token vectors (128D × tokens) | Stage-3 reranking |
| Embedder warm provider | in-process, GPU 0 | Candle 0.10.2 (CUDA), `crates/context-graph-embeddings/src/warm/` | Loads E1-E14 + 7 learner heads into VRAM lazily/eagerly |
| Causal/graph LLM agent | in-process, GPU | `llama-cpp-2 0.1` + GBNF grammars in `models/hermes-2-pro/` | Hermes-2-Pro-Mistral-7B Q5_K_M (~5GB) for relationship discovery |
| File watcher background task | in-process tokio task | `crates/context-graph-mcp/src/server/watchers.rs`, `glob 0.3` | Re-ingests files matching watched globs |
| K-NN graph builder | in-process tokio task | `crates/context-graph-storage/src/graph_edges/builder.rs` | Background `BackgroundGraphBuilder` rebuilds typed/embedder edges |
| `context-graph-cli` | local process | `clap 4.5` derive | Operator commands, hooks, capture/inject, DynamicJEPA admin |
| `blackbox` (CLI bin) | local process | clap | Black-box harness, defined in `crates/context-graph-cli/src/bin/blackbox.rs` |
| `context-graph` (CLI bin) | local process | clap | Auxiliary CLI binary (`crates/context-graph-cli/src/bin/context-graph.rs`) |
| `reality-loop` (CLI bin) | local process | clap, reqwest, sha2 | Rust-owned SWE-bench refinement loop driving local Ollama + official SWE-bench Docker eval (`crates/context-graph-cli/src/bin/reality-loop.rs`); auto-discovered (no `[[bin]]` entry) |
| Single-writer guard | `<storage.path>/mcp.pid` | `libc` flock (Unix) | Prevents concurrent RocksDB opens; staleness checked against PID + port |
| `gemmafrontend/server.py` | local Python (uv) HTTP `:8787` | `python3` + Ollama HTTP API | Evidence console exposing 10 model tools (`run_proof_pipeline`, `inspect_latest_run`, `compile_reward_for_run`, ...) and persisting prevalidated tool-call artifacts under `/tmp/gemmafrontend-runs/` |

The MCP server boots in this order (see `crates/context-graph-mcp/src/main.rs`): parse CLI → load TOML config (`config/default.toml` or `--config`) → acquire `mcp.pid` flock → optionally spawn or proxy into a daemon → initialise tracing → construct `LazyMultiArrayProvider` (defers GPU model load) and `RocksDbTeleologicalStore` → start watchers and graph builder → enter the chosen `TransportMode`.

The `reality-loop` binary boots independently and DOES NOT load the MCP server: it shells out to `gemma4:e4b` via Ollama `/api/chat`, drives local git workspace control, calls the official SWE-bench evaluator as a Docker subprocess (`princeton-nlp/SWE-bench_Lite` only), and writes a fail-closed measurement contract (`<run_root>/measurement-contract.json`) before any execution. Hard rejections include missing `--official-eval-each-attempt`, malformed manifest, base_commit ≠ git HEAD, dirty worktree, and patches that touch benchmark/test/fixture/harness paths.

## 3. Technology Stack
| Layer | Technology | Source / version |
| --- | --- | --- |
| Toolchain | Rust stable, edition 2021 | `rust-toolchain.toml` (channel = "stable", components rustfmt/clippy/rust-src), `Cargo.toml` (`rust-version = "1.95"`) |
| Async runtime | `tokio = "1.35"` (full features) | workspace dep `Cargo.toml` |
| Serialization | `serde = "1.0"` derive, `serde_json = "1.0"`, `bincode = "1.3"`, `toml = "0.8"` | workspace deps |
| Errors | `thiserror = "1.0"`, `anyhow = "1.0"` | workspace deps |
| Logging | `tracing = "0.1"`, `tracing-subscriber = "0.3"` (json, env-filter) | workspace deps |
| Config | `config = "0.14"`, `toml`, env override | `crates/context-graph-core/src/config/` |
| Storage | `rocksdb = "0.22"`, `usearch = "2"`, `parking_lot = "0.12"`, `dashmap = "5.5"`, `blake3 = "1.5"` | `crates/context-graph-storage/Cargo.toml` |
| Sparse | `bitvec = "1.0"` (HDC); custom inverted indexes for E6/E13 | `crates/context-graph-embeddings`, `crates/context-graph-storage/src/teleological/indexes/` |
| GPU / ML (embeddings) | `candle-core = "0.10.2"` + `candle-nn = "0.10.2"` (cuda), `tokenizers = "0.22"`, `safetensors = "0.4"`, `half = "2.3"` | `crates/context-graph-embeddings/Cargo.toml` |
| GPU / ML (LLM agents) | `llama-cpp-2 = "0.1"` (optional `llm` and `cuda` features) | `crates/context-graph-causal-agent/Cargo.toml` |
| CUDA target | RTX 5090 / Blackwell, sm_120a, CUDA 13.2; via `cudarc` 0.19.x in candle 0.10.2 | `Cargo.toml` workspace comment, `crates/context-graph-cuda` |
| Search & fusion | RRF (`RRF_K` constant in `crates/context-graph-core/src/fusion/mod.rs`), Weighted RRF, MaxPooling, MaxSim | `crates/context-graph-core/src/fusion/`, `crates/context-graph-storage/src/teleological/search/` |
| HNSW (dense) | `hnsw_rs = "0.3"` (in core abstraction) and `usearch = "2"` (storage runtime) | `crates/context-graph-core/src/index/`, `crates/context-graph-storage/src/teleological/indexes/` |
| Parsing | `tree-sitter = "0.23"`, `tree-sitter-rust = "0.23"` for E7 cAST chunking | `crates/context-graph-core/Cargo.toml` |
| Hashing | `sha2 = "0.10"`, `blake3 = "1.5"`, `xxhash-rust 0.8` | per-crate Cargo.toml |
| Caching | `moka = "0.12"` (sync) for embedding cache | `crates/context-graph-embeddings/Cargo.toml` |
| Parquet export | `parquet = "58"`, `arrow-array 58`, `arrow-schema 58` | `crates/context-graph-cli/Cargo.toml` |
| HTTP / network | `reqwest 0.12` (rustls), `zip 7.2.0`, `rusqlite 0.32` (bundled) | `crates/context-graph-cli/Cargo.toml` (model download, share/import flows, reality-loop Ollama HTTP) |
| MCP CLI parsing | hand-rolled (`crates/context-graph-mcp/src/main.rs`); CLI uses `clap = "4.5"` | per main.rs |
| FFI / kernels | `crates/context-graph-cuda/src/ffi/` (CUDA driver API, FAISS bindings gated behind `faiss-working`) | `crates/context-graph-cuda/Cargo.toml` |
| Test utilities | `tempfile = "3.10"`, `tokio-test = "0.4"`, `serial_test = "3"`, `criterion 0.5` | per-crate `[dev-dependencies]` |

## 4. Workspace Crates
Source: `Cargo.toml` `[workspace.members]` (11 crates) and `default-members` (10 crates — benchmark excluded).

| Package | `default` features | Purpose | Default-members? |
| --- | --- | --- | --- |
| `context-graph-mcp` | `cuda`, `llm` | MCP JSON-RPC server; 141/145 tools | Yes |
| `context-graph-core` | — | Domain types, fingerprints, retrieval, learner, learning, teleological, error hierarchy | Yes |
| `context-graph-cuda` | `cuda` | CUDA driver API kernels, similarity, HDBSCAN, allocation, FAISS gate | Yes |
| `context-graph-embeddings` | `candle` | 14 production + 7 learner embedders, warm provider, quantization, training | Yes |
| `context-graph-storage` | `cuda` | RocksDB facade, 101 CFs, HNSW/inverted/MaxSim search engines, edge repo | Yes |
| `context-graph-dynamicjepa` | — | DynamicJEPA service: tiny-model training, artifacts, predictions, MI audit, transfer | Yes |
| `context-graph-cli` | `cuda` | Operator + hook CLI; 4 binaries (`context-graph-cli`, `context-graph`, `blackbox`, `reality-loop`) | Yes |
| `context-graph-causal-agent` | — (opt-in `llm`, `cuda`) | LLM-gated causal discovery (`CausalDiscoveryLLM`) | Yes |
| `context-graph-graph-agent` | — (opt-in `llm`, `cuda`, `test-utils`) | LLM-gated graph relationship discovery; reuses causal-agent LLM | Yes |
| `context-graph-test-utils` | — | Shared, real-data test fixtures (no mocks) | Yes |
| `context-graph-benchmark` | — (opt-in `real-embeddings`, `benchmark-tests`) | 45 benchmark binaries (per `cargo metadata`); excluded from default builds | **No** |

## 5. MCP Tool Domains
Source: `crates/context-graph-mcp/src/tools/definitions/mod.rs` (`get_tool_definitions`).
Total: **141 without `llm`, 145 with `llm`** (asserted by `test_total_tool_count_and_no_duplicates`, lines 131-157). Per-category counts asserted by `test_submodule_counts` (lines 159-198):

| Category | Count | Definition source | Handler module |
| --- | --- | --- | --- |
| capability | 1 | `definitions/capability.rs` | `handlers/tools/capability_tools.rs` |
| core | 5 | `definitions/core.rs` | `handlers/tools/memory_tools.rs`, `consolidation.rs`, `status_tools.rs` |
| merge | 1 | `definitions/merge.rs` | `handlers/merge.rs` |
| curation | 2 | `definitions/curation.rs` | `handlers/tools/curation_tools.rs` |
| topic | 4 | `definitions/topic.rs` | `handlers/tools/topic_tools.rs` |
| file_watcher | 4 | `definitions/file_watcher.rs` | `handlers/tools/file_watcher_tools.rs` |
| sequence | 4 | `definitions/sequence.rs` | `handlers/tools/sequence_tools.rs` |
| causal | 4 | `definitions/causal.rs` | `handlers/tools/causal_tools.rs`, `causal_relationship_tools.rs` |
| causal_discovery | 0 base / 2 with `llm` | `definitions/causal_discovery.rs` | `handlers/tools/causal_discovery_tools.rs` |
| keyword | 1 | `definitions/keyword.rs` | `handlers/tools/keyword_tools.rs` |
| code | 1 | `definitions/code.rs` | `handlers/tools/code_tools.rs` |
| graph | 2 base / 4 with `llm` | `definitions/graph.rs` | `handlers/tools/graph_tools.rs`, `graph_learning_tools.rs` |
| robustness | 1 | `definitions/robustness.rs` | `handlers/tools/robustness_tools.rs` |
| entity | 6 | `definitions/entity.rs` | `handlers/tools/entity_tools.rs` |
| embedder | 7 + 2 standalone | `definitions/embedder.rs` (`definitions()` + `standalone_definitions()`) | `handlers/tools/embedder_tools.rs` |
| temporal | 2 | `definitions/temporal.rs` | `handlers/tools/temporal_tools.rs` |
| graph_link | 4 | `definitions/graph_link.rs` | `handlers/tools/graph_link_tools.rs` |
| maintenance | 1 | `definitions/maintenance.rs` | `handlers/tools/maintenance_tools.rs` |
| provenance | 3 | `definitions/provenance.rs` | `handlers/tools/provenance_tools.rs` |
| daemon | 1 | `definitions/daemon.rs` | `handlers/tools/daemon_tools.rs` |
| training | 4 | `definitions/training.rs` | `handlers/tools/training_tools.rs` |
| learning | 14 | `definitions/learning.rs` | `handlers/tools/learning_tools.rs` |
| utl | 18 | `definitions/utl.rs` | `handlers/tools/utl_tools.rs` |
| constellation | 6 | `definitions/constellation.rs` | `handlers/tools/constellation_tools.rs` |
| contrastive | 4 | `definitions/contrastive.rs` | `handlers/tools/contrastive_tools.rs` |
| typed_edges | 4 | `definitions/typed_edges.rs` | `handlers/tools/typed_edges_tools.rs` |
| dynamicjepa | 35 | `definitions/dynamicjepa.rs` | `handlers/tools/dynamicjepa_tools.rs` |

Method dispatch is restricted to MCP `initialize`, `notifications/initialized`, `shutdown`, `tools/list`, `tools/call` (`crates/context-graph-mcp/src/handlers/core/dispatch.rs`); all tool access is via `tools/call`.

See [08_mcp_tools_reference.md](08_mcp_tools_reference.md) for the full tool inventory and DTOs.

## 6. Entry Points
| Entry | Kind | Source |
| --- | --- | --- |
| `context-graph-mcp` | binary | `crates/context-graph-mcp/src/main.rs` |
| `context-graph-cli` | binary | `crates/context-graph-cli/src/main.rs` |
| `context-graph` | binary (auxiliary) | `crates/context-graph-cli/src/bin/context-graph.rs` |
| `blackbox` | binary | `crates/context-graph-cli/src/bin/blackbox.rs` |
| `reality-loop` | binary | `crates/context-graph-cli/src/bin/reality-loop.rs` (auto-discovered, not declared in `[[bin]]`); 2,379 LOC; runs SWE-bench refinement against Ollama-hosted local models |
| `train-causal` | binary | `crates/context-graph-embeddings/src/bin/train_causal.rs` |
| `benchmark-report`, `realdata-bench`, `gpu-bench`, `mcp-bench`, etc. | 45 binaries | `crates/context-graph-benchmark/src/bin/*.rs` (build with `cargo build -p context-graph-benchmark`); enumerated in §12 of [02_source_code_map.md](02_source_code_map.md) |
| Examples | `crates/context-graph-{causal-agent,core,graph-agent,storage,cuda}/examples/` |

MCP server CLI flags (parsed in `crates/context-graph-mcp/src/main.rs::CliArgs`): `--config <path>`, `--transport <stdio|tcp>`, `--port <u16>`, `--bind <addr>`, `--warm-first`, `--no-warm`, `--daemon`, `--daemon-port <u16>`, hidden `--daemon-server-only`. Environment overrides: `CONTEXT_GRAPH_TRANSPORT`, `CONTEXT_GRAPH_TCP_PORT`, `CONTEXT_GRAPH_BIND_ADDRESS`, `CONTEXT_GRAPH_DAEMON`. Priority is CLI > env > config file > defaults. See [10_cli_reference.md](10_cli_reference.md).

`reality-loop` CLI flags (`crates/context-graph-cli/src/bin/reality-loop.rs:25-71`): `--task-manifest <path>` (required), `--model gemma4:e4b` (default), `--runtime-root /tmp/contextgraph-rust-reality-loop` (default), `--ollama-url http://127.0.0.1:11434`, `--context-graph-bin target/debug/context-graph`, `--max-attempts <u64>`, `--run-until-resolved`, `--official-eval-each-attempt`, `--swebench-python <path>`, `--official-timeout 900`, `--official-max-workers 1`, `--official-cache-level env`, `--official-docker-host unix:///var/run/docker.sock`, `--official-docker-config <path>`, `--gpu-index 0`, `--gpu-min-free-mb 8192`, `--gpu-max-used-mb`, `--ollama-keep-alive 30m`, `--model-timeout 300`, `--git-timeout 600`, `--sleep-between-attempts-ms 0`. Hard-rejects `--run-until-resolved` without `--official-eval-each-attempt`.

Makefile targets (top-level `Makefile`):
| Target | Action |
| --- | --- |
| `make build` | `build-rust` (cargo build --release --workspace + trim) and `build-python` (uv sync inside `clipcannon/`) |
| `make build-rust` | release build of all default-members |
| `make build-debug` | debug build of all default-members |
| `make test` / `make test-rust` / `make test-python-smoke` | `cargo test --workspace` and Python smoke tests |
| `make test-mcp` | MCP-only integration suite |
| `make test-e2e` | E2E hook tests |
| `make check`, `make lint`, `make clippy` | quick check, lint, clippy |
| `make trim`, `make clean`, `make clean-all`, `make clean-deep` | disk maintenance |
| `make disk-check` | disk usage report |
| `make release-candidate-archive` | bundles `release-candidate-archive` artifact |

## 7. Project Directory Structure
| Path | Role |
| --- | --- |
| `Cargo.toml` | Workspace manifest (11 members, 10 default-members, profiles) |
| `Cargo.lock` | Locked dep versions |
| `Makefile` | Build/test/lint orchestration with auto-trim |
| `rust-toolchain.toml` | Pinned toolchain (`stable`, rustfmt/clippy/rust-src) |
| `Dockerfile` | Container build |
| `crates/` | 11 workspace crates (see Section 4) |
| `config/{default,development,production,test}.toml` | TOML configs consumed by `Config::from_path` |
| `configs/dynamicjepa/{domain_packs,verification}/` | DynamicJEPA-specific TOML/JSONL bundles (4 domain packs: counter_to_grid, career_taxonomy, counter_world, gridworld_5x5; 14 verification fixtures) |
| `contextgraph_data/` | Default RocksDB database (SST + WAL + `mcp.pid` flock) |
| `models/` | Pretrained weights (E1 semantic, E5 causal, E7 code-1536, E10 contextual, E11 kepler, E12 late-interaction, E13 splade-v3, E14 bge-m3-dense, learner heads, `hermes-2-pro/*.gguf` + `*.gbnf` grammars) |
| `gemmafrontend/` | Python evidence console (`server.py` 3,879 LOC, runs on `:8787`); `manual_fsv.py`, `model_benchmark.py`, `model_prompt_matrix.py`; SWE-bench refinement is Rust-only in `reality-loop` |
| `scripts/` | 78 shell + Python helpers; includes `scripts/v4_2/` (34 files: CAMD-DPO, FSV), `scripts/5090jepa/` (corpus prep), `scripts/exp2/` (Phase-2 experimental cycle), `scripts/shakespeare_*.py` (~30 files), `scripts/download_models.py`, daemon helpers |
| `examples/` | Standalone usage examples |
| `tests/` | Workspace-level integration scaffolds |
| `tasks/`, `docs/`, `docs2/`, `backupdocs/`, `reference/` | Planning + documentation (this directory is `docs2/contextgraphspecs/`) |
| `tmp/`, `target/`, `data/`, `artefacts/`, `benchmarks/` | Build outputs and scratch |
| `clipcannon/` | Python media subproject (uv-managed) |
| `shakespear/` | Shakespeare empirical pipeline working dir |
| `pyproject.toml` | Top-level Python project shim |

## 8. Storage Tier Classification
| Path | Class | Behavior |
| --- | --- | --- |
| RocksDB at `storage.path` (typ. `contextgraph_data/`) | **Sacred durable** | All 101 CFs (11 base + 41 teleological + 14 quantized + 5 code + 2 causal + 28 DynamicJEPA); single-writer enforced by `mcp.pid` flock; no auto-migrations (pre-production project) |
| usearch HNSW indexes (per dense embedder) | **Regenerable runtime** | Rebuilt from `fingerprints` CF on startup; `try_load_hnsw_indexes()` returns Ok(true) when ANY persisted index loads |
| Sparse inverted indexes for E6/E13 | **Regenerable** | CFs `e6_sparse_inverted`, `e13_splade_inverted` rebuilt from fingerprints |
| MaxSim store for E12 | **Regenerable** | CF `e12_late_interaction` |
| `models/` | Regenerable runtime asset | Weights downloaded by `scripts/download_models.py` and embedder loaders |
| `target/` | Regenerable | Cargo artifacts; `Makefile` calls `scripts/trim-stale-deps.sh` after each build |
| `<storage.path>/mcp.pid` | Ephemeral | Unix flock guard with stale-lock detection (zombie-aware) |
| `~/.contextgraph/extended_embeddings.lmdb` (if used) | Regenerable sidecar | Extended CAMD cache used by Python harnesses |
| `tmp/`, `/tmp/contextgraph-rust-reality-loop/`, `/tmp/gemmafrontend-runs/` | Ephemeral | Reality-loop run roots, gemmafrontend snapshots |

See [04_database_schema.md](04_database_schema.md) for per-CF key/value schemas and [09_storage_layer.md](09_storage_layer.md) for read/write paths.

## 9. Error Hierarchy
Source: `crates/context-graph-core/src/error/{mod,unified,sub_errors,legacy,conversions}.rs` and `crates/context-graph-storage/src/error.rs`.

| Error type | Location | Notes |
| --- | --- | --- |
| `ContextGraphError` | `core/src/error/unified.rs` | Top-level enum with `#[from]` conversions; provides `error_code()` mapping JSON-RPC codes (-32004 storage, -32005 embedding, -32602 validation, etc.) |
| `EmbeddingError` | `core/src/error/sub_errors.rs` | Model loading, generation, quantization, dimension issues |
| `StorageError` (core) | `core/src/error/sub_errors.rs` | High-level storage failures wrapped from `context_graph_storage::StorageError` |
| `IndexError` | `core/src/error/sub_errors.rs` | HNSW search and corruption |
| `ConfigError` | `core/src/error/sub_errors.rs` | Missing config, invalid values, parse failures |
| `GpuError` | `core/src/error/sub_errors.rs` | Device init, memory, kernel failures |
| `McpError` | `core/src/error/sub_errors.rs` | Request validation, authorization, protocol violations |
| `CoreError` / `CoreResult` | `core/src/error/legacy.rs` | Retained for backwards compatibility (still re-exported) |
| `Result<T>` | `core/src/error/unified.rs` | `type Result<T> = std::result::Result<T, ContextGraphError>` |
| `StorageError` (storage crate) | `storage/src/error.rs` | RocksDB-level: `OpenFailed`, `ColumnFamilyNotFound`, `WriteFailed`, `ReadFailed`, `FlushFailed`, `NotFound`, `Serialization`, `ValidationFailed`, `IndexCorrupted`, `Internal`. `From<rocksdb::Error>` conversion present. |
| `TeleologicalStoreError`, `QuantizedStorageError`, `CodeStorageError`, `GraphEdgeStorageError` | `crates/context-graph-storage/src/teleological`, `code`, `graph_edges` | Per-subsystem typed errors |
| `EdgeError` | `core/src/graph_linking/` | K-NN graph edge construction errors |
| `DynamicJepaError` | `core/src/dynamicjepa/error.rs` | DynamicJEPA pipeline errors |
| `WeightProfileError` | `core/src/weights/mod.rs` | Profile validation |
| `RealityError` | `crates/context-graph-cli/src/bin/reality-loop.rs:73-103` | Reality-loop typed failure: `error_code` (e.g. `RUST_REALITY_OFFICIAL_EVAL_REQUIRED`, `RUST_REALITY_PATCH_TOUCHES_BENCHMARK`), `field_path`, `remediation`, `details`, `source_of_truth`; serialised to `<runtime_root>/failures/*.json` |

`ContextGraphError::error_code()` returns negative i32 codes per JSON-RPC 2.0 spec; `crates/context-graph-mcp/src/protocol.rs` defines error_codes constants used by handlers.

## 10. Key Subsystems

**MCP server.** `crates/context-graph-mcp/src/main.rs` parses CLI, manages the `mcp.pid` flock, optionally spawns/proxies into a daemon, then enters either stdio or TCP transport. `server/transport.rs` implements `read_line_bounded` framing; `server/watchers.rs` runs file-watcher and graph-builder background tasks. Method dispatch is in `handlers/core/dispatch.rs`; `tools/call` flows through `handlers/tools/dispatch.rs`. The `LazyMultiArrayProvider` adapter (`adapters/lazy_provider.rs`) lets the server come up before all 14 GPU embedders finish loading. See [08_mcp_tools_reference.md](08_mcp_tools_reference.md).

**Storage (RocksDB + 101 CFs).** `crates/context-graph-storage/src/lib.rs` exposes `RocksDbTeleologicalStore`, `EdgeRepository`, `CodeStore`, and DynamicJEPA stores. Column families are split across four registries: `column_families.rs` (11 base: 8 originals + `embedder_edges`, `typed_edges`, `typed_edges_by_type`), `teleological/column_families.rs` (41 teleological + 14 quantized + 5 code + 2 causal), and `dynamicjepa/column_families.rs` (28). All CFs share a 256 MB block cache; per-CF options live alongside their constants. `TOTAL_COLUMN_FAMILIES = 101`. See [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md).

**Embeddings (14 production + 7 learner).** `crates/context-graph-embeddings/src/lib.rs` requires the `candle` feature (compile_error otherwise). `ModelId::production()` returns 14 variants `Semantic, TemporalRecent, TemporalPeriodic, TemporalPositional, Causal, Sparse, Code, Graph, Hdc, Contextual, Kepler, LateInteraction, Splade, BgeM3Dense`. `Entity = 10` (legacy 384D MiniLM) coexists with `Kepler = 13` (768D KEPLER) and is excluded from `production()` (kept in `all()`, which returns 15). Native dims: E1=1024, E2/E3/E4=512, E5=768 (asymmetric cause/effect), E6=30,522 SPLADE vocab, E7=1536, E8=1024 (asymmetric source/target), E9=10,000 binary, E10=768 (asymmetric doc/query), E11=768 (Kepler), E12=128/token, E13=30,522 SPLADE vocab, E14=1024 (BGE-M3 dense). Warm provider in `src/warm/` handles loader, registry, CUDA allocation, memory pool, inference, validation, health, diagnostics. Learner namespace (`src/learner/mod.rs`) defines E15-E21 (`LEARNER_EMBEDDER_COUNT = 7`, `UTL_PLANNED_TOTAL_EMBEDDERS = 21`). See [05_embedding_subsystem.md](05_embedding_subsystem.md).

**Search & fusion.** `crates/context-graph-core/src/retrieval/` defines query, executor, multi-space, divergence, and aggregation. `crates/context-graph-core/src/fusion/` provides `RRF_K`, `weighted_rrf`, `fuse_rankings`, `score_weighted_rrf`, `weighted_sum`, `normalize_minmax`, `FusionStrategy`, `EmbedderRanking`, `FusedResult`. Pipeline strategy stages: E13 (sparse recall) → E1 (dense recall + scoring) → E12 (MaxSim rerank). 20 weight profiles in `crates/context-graph-core/src/weights/mod.rs::WEIGHT_PROFILES` (verified line-by-line at lines 178, 200, 220, 240, 265, 291, 315, 335, 357, 383, 407, 427, 447, 467, 490, 513, 536, 562, 583, 604): `semantic_search, causal_reasoning, code_search, fact_checking, graph_reasoning, temporal_navigation, sequence_navigation, conversation_history, category_weighted, typo_tolerant, pipeline_stage1_recall, pipeline_stage2_scoring, pipeline_full, balanced, multilingual_search, long_context, translation_finder, affect_repair, affect_priming, affect_neutral`. See [06_search_and_fusion.md](06_search_and_fusion.md).

**Graph & causal.** `crates/context-graph-core/src/graph_linking/` defines K-NN edges (`EmbedderEdge`, `KnnGraph`, `KNN_K`, `MIN_KNN_SIMILARITY`, `NN_DESCENT_*`) and 20 typed edge relations (`GraphLinkEdgeType`, `TypedEdge`). `crates/context-graph-core/src/graph/` and `src/causal/` implement E5 asymmetric similarity (`compute_e5_asymmetric_full`, `CausalDirection::Cause/Effect`) and E8 asymmetric similarity (`compute_e8_asymmetric_*`, `GraphDirection`). Edge writes live in `crates/context-graph-storage/src/graph_edges/` and `teleological/rocksdb_store/typed_edge_*.rs`. See [07_graph_and_causal.md](07_graph_and_causal.md).

**LLM agents.** `crates/context-graph-causal-agent/src/llm/mod.rs` loads `models/hermes-2-pro/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf` (~5GB) via `llama-cpp-2` with three GBNF grammars (`causal_analysis.gbnf`, `graph_relationship.gbnf`, `validation.gbnf`) for 100% valid JSON output. ChatML prompt format (`prompt.rs`). `context-graph-graph-agent` reuses `CausalDiscoveryLLM` rather than instantiating its own model. Both crates gate the LLM path behind a `llm` feature; `graph-agent/src/stubs.rs` provides no-op replacements when disabled. See [07_graph_and_causal.md](07_graph_and_causal.md).

**Teleological.** `crates/context-graph-core/src/teleological/` holds the comparator, embedder/group definitions, matrix search, profile, services, synergy matrix, Tucker core, and `TELEOLOGICAL_VECTOR_DIM = 13` (E14 excluded from the compact teleological vector). `TeleologicalFingerprint` wraps `SemanticFingerprint` with metadata, importance, and access tracking. See [13_teleological_subsystem.md](13_teleological_subsystem.md).

**Constellation.** `crates/context-graph-core/src/constellation/` provides reservoir sampling, Welford streaming statistics, compiler, and types. Storage in `teleological/rocksdb_store/constellation.rs` with CFs `constellations` and `constellation_by_selector`. See [17_constellation_subsystem.md](17_constellation_subsystem.md).

**Contrastive & typed edges.** `crates/context-graph-core/src/contrastive/` mines disagreement pairs across embedders (`AnomalyKind`); `crates/context-graph-core/src/typed_edge_export/` builds `TypedEdgeTrainingRecord`s. Storage in `teleological/rocksdb_store/{contrastive,typed_edge_export,llm_validation}.rs` across CFs `contrastive_pairs`, `contrastive_by_kind`, `contrastive_by_anchor`, `typed_edge_records`, `typed_edge_validations`. See [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

**Training.** `crates/context-graph-core/src/training/` defines `TrainingRecord` v3 (memory + 14 embeddings + 91 cross-correlations) with computation, edge_signature, temporal modules. CF `training_records`. `crates/context-graph-embeddings/src/training/` and `bin/train_causal.rs` provide the E5 LoRA training driver. See [16_training_subsystem.md](16_training_subsystem.md).

**Learning subsystem (UTL).** `crates/context-graph-core/src/learning/`, `src/learner/`, `src/learner_training.rs` implement learner profiles, modalities, S/C/E deltas, L = ΔS·ΔC·ΔE, M(t) memory traces, and goal centroids. 13 learner-state CFs are present (`learning_events`, `learner_profile`, `learner_constellations`, `fingerprints_learner`, `learner_m_per_trace`, `learner_state_history`, `learner_goal_states`, `learner_retrieval_log`, `learner_k_sleep`, `goal_centroids`, `learner_delta_log`, `learner_audit`, `learner_training_datasets`). See [20_learning_subsystem.md](20_learning_subsystem.md).

**DynamicJEPA.** `crates/context-graph-dynamicjepa/` (service, artifact, transfer, MI audit, model, config) plus `crates/context-graph-core/src/dynamicjepa/` (domain pack, instrument, panel, planner, predictor, guard, surprise, validation, verification, schema, dataset, trajectory, binding, pair_kinds, features, meaning_compression, etc.) implement the 5090jepa programme. Storage data plane in `crates/context-graph-storage/src/dynamicjepa/` (28 CFs) covers domain packs, raw events, normalized states, actions, outcomes, transitions, instrument readings, latent panels, pairwise readings, constellations, threshold calibrations, bindings, trajectories, dataset shards, training runs, model artifacts, predictions, skill policies, plan traces, guard decisions, surprise events, verification runs, audit log. Exposed via 35 MCP tools. See [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md).

**Reality loop (SWE-bench).** `crates/context-graph-cli/src/bin/reality-loop.rs` implements the fail-closed SWE-bench Lite repair loop. The loop validates a task manifest against an immutable `contextgraph_reality_measurement_contract_v1`, drives a local Ollama model under a GPU VRAM budget, applies the candidate patch to the real repository at the exact base commit, runs the official Docker SWE-bench harness, and only declares success when `resolved_ids` contains the task's `instance_id`. Every step is byte-equal-readback verified; every artefact is persisted into BlackBoxFix (`LEDGER_SCHEMA_VERSION = 14`) and a `reality-events.jsonl` event stream. See [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md).

**File watcher.** `crates/context-graph-core/src/types/file_index.rs`, file-watcher MCP tools in `definitions/file_watcher.rs`, background task in `crates/context-graph-mcp/src/server/watchers.rs`. CF `file_index`. Reconciliation tool removes orphans.

**Topic clustering.** `crates/context-graph-core/src/clustering/` implements HDBSCAN (`hdbscan_defaults`, `HDBSCANClusterer`, `HDBSCANParams`, `ClusterSelectionMethod`) and BIRCH (`birch_defaults`, `BIRCHParams`, `ClusteringFeature`). `Topic`, `TopicProfile`, `TopicStability`, `ClusterMembership`. Storage CFs `topic_profiles`, `topic_portfolio`. See [12_clustering_and_topics.md](12_clustering_and_topics.md).

**Memetic system.** `get_memetic_status` MCP tool reports system meme/concept activity (`handlers/tools/status_tools.rs`). Backed by `crates/context-graph-core/src/memory/`.

**Privacy & retention (Phase 13).** Recent work (commits `316e54a8`, `1ed0eb95`, `9ba0a9dd`) wired Phase-13 retention/erasure decisions into the dashboard timeline (schema v7) plus the FSV suite — referenced from `crates/context-graph-cli/src/bin/blackbox.rs` and DynamicJEPA audit log.

**Reality loop (Rust SWE-bench).** `crates/context-graph-cli/src/bin/reality-loop.rs` (2,379 LOC, added in commits `56d9026b` and hardened in `cc8e6c33`) is a self-contained `tokio::main` binary that drives a single local Ollama model (`gemma4:e4b` by default) through SWE-bench_Lite repair attempts. It owns: task-manifest readback with hard contract validation (`validate_task_manifest_contract`), git clone/reset/clean workspace control, repository context selection from real `git ls-files`/`grep`, prompt/request/response/parse-report SHA-256 read-after-write, real `/api/chat` HTTP calls, `nvidia-smi` GPU VRAM budget enforcement (`--gpu-min-free-mb`/`--gpu-max-used-mb`), BlackBoxFix run/model/tool/verifier/score/reward records with SQLite ledger readback, an append-only `reality-events.jsonl`, typed `RealityError` failure artifacts under `<runtime_root>/failures/`, and a `<run_root>/measurement-contract.json` written before execution. Hard rejections: missing `--official-eval-each-attempt`, dataset ≠ `princeton-nlp/SWE-bench_Lite`, malformed instance id, repo not `owner/repo`, base_commit not 40-char SHA, empty problem statement, missing FAIL_TO_PASS/PASS_TO_PASS metadata, dirty worktree, base_commit ≠ git HEAD, candidate patch touching benchmark/test/fixture/harness paths (`RUST_REALITY_PATCH_TOUCHES_BENCHMARK`).

**Gemma frontend (Python).** `gemmafrontend/server.py` exposes a Python HTTP UI (`:8787`) with 10 governed model tools (`run_proof_pipeline`, `inspect_latest_run`, `compile_reward_for_run`, `explain_scorecard`, `analyze_incident`, `suggest_reward_improvements`, `compare_recent_runs`, `export_buyer_report`, `evaluate_tool_policy`, `build_investor_category_pack`). It validates Ollama tool-call selections, persists prevalidated tool-call artifacts under `/tmp/gemmafrontend-runs/prevalidated-tool-calls/`, runs the BlackBoxFix harness, and persists frontend snapshots under `/tmp/gemmafrontend-runs/frontend-runs/*/system-snapshot.json`. `manual_fsv.py`, `model_benchmark.py`, and `model_prompt_matrix.py` provide complementary UI/evidence harnesses. SWE-bench refinement is not implemented in Python anymore.

**Capability matrix.** `get_capability_matrix` (`handlers/tools/capability_tools.rs`) returns embedders, CFs, tools, and runtime state in a compact JSON map for agents to discover the surface.

## 11. Documented Source-Level Inconsistencies
| Item | Source |
| --- | --- |
| `TELEOLOGICAL_VECTOR_DIM = 13` while `NUM_EMBEDDERS = 14` (E14 excluded from compact vector) | `crates/context-graph-core/src/types/fingerprint/teleological/types.rs:12`; `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs:81` |
| Config validates `sse` transport but MCP `main.rs` rejects it | `crates/context-graph-core/src/config/sub_configs.rs:137`; `crates/context-graph-mcp/src/main.rs` |
| `LEARNER_MODALITY_COUNT` < `LearnerModality` variant count (validation permits +1 slack) | `crates/context-graph-core/src/learner/mod.rs` |
| `ModelId::all()` returns 15 variants, `production()` returns 14, `NUM_EMBEDDERS = 14`. `Entity` = legacy 384D MiniLM, `Kepler` = production 768D | `crates/context-graph-embeddings/src/types/model_id/core.rs:200,234` |
| `crates/context-graph-storage/src/column_families.rs` doc comment context lists 8 base CFs; constant `TOTAL_COLUMN_FAMILIES = 101` includes 11 base after graph-linking additions (8 originals + `embedder_edges`/`typed_edges`/`typed_edges_by_type`) | `crates/context-graph-storage/src/column_families.rs:41-83,408` |
| `Cargo.toml` `rust-version = "1.95"` but `rust-toolchain.toml` channel = "stable" — toolchain selection driven by stable channel | `Cargo.toml`, `rust-toolchain.toml` |
| Quantized embedder CF dimension docstrings (e.g. "PQ-8 8 bytes from 1024D") describe target compression, not necessarily current write path; CFs were "reserved" historically and only partially wired | `crates/context-graph-storage/src/teleological/column_families.rs` lines ~660-665 |
| `config/default.toml` uses legacy section names (`[general]`, `[server.transport]`, `[storage].sqlite_path`, `[embeddings]`, `[graph]`) that do NOT match the current Rust `Config` shape — fields not present in the Rust types are silently ignored by serde; the Rust types in `crates/context-graph-core/src/config/sub_configs.rs` are authoritative | `config/default.toml`; `crates/context-graph-core/src/config/mod.rs:42` |
| `reality-loop` binary is auto-discovered by Cargo (file at `src/bin/reality-loop.rs`) but is NOT declared in `crates/context-graph-cli/Cargo.toml` `[[bin]]` blocks alongside `context-graph-cli`, `context-graph`, `blackbox` | `crates/context-graph-cli/Cargo.toml:11-21`; `cargo metadata` confirms 4 bins |
