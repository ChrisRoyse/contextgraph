# System Overview

**Source files covered:**
- `Cargo.toml` (workspace manifest, default-members)
- `crates/*/Cargo.toml` (per-crate manifests, features, deps)
- `crates/context-graph-mcp/src/main.rs` (entry, daemon spawn, dual flock guard, reality-loop singleton)
- `crates/context-graph-mcp/src/lib.rs`
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` (tool registry assertion: 145 with `llm`, 141 without)
- `crates/context-graph-mcp/src/tools/mod.rs` (`set_reality_loop_mode` / `is_reality_loop_mode`)
- `crates/context-graph-mcp/src/handlers/core/dispatch.rs` (method dispatch)
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` (tool dispatch — 303 lines)
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/{alter,analyze,autoresearch,errors,helpers,interact,lock,mod,optimizer,path_policy,schema_validator,shift_log}.rs` (cgreality MCP family — 5,627 lines)
- `crates/context-graph-storage/src/column_families.rs:408` (`TOTAL_COLUMN_FAMILIES = 101`)
- `crates/context-graph-storage/src/teleological/column_families.rs:657,746,1887,2057` (`TELEOLOGICAL_CF_COUNT = 41`, `QUANTIZED_EMBEDDER_CF_COUNT = 14`, `CODE_CF_COUNT = 5`, `CAUSAL_CF_COUNT = 2`)
- `crates/context-graph-storage/src/dynamicjepa/column_families.rs:74` (`DJ_CF_COUNT = DJ_CF_NAMES.len() == 28`)
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs:81` (`NUM_EMBEDDERS = 14`)
- `crates/context-graph-embeddings/src/types/model_id/core.rs:200,234` (`ModelId::all()` returns 15, `production()` returns 14)
- `crates/context-graph-cli/Cargo.toml` (3 declared `[[bin]]` blocks; `reality-loop` auto-discovered)
- `crates/context-graph-cli/src/bin/reality-loop.rs` (engine binary, 18,137 LOC, in-process llama-cpp-2)
- `config/ccreality_*.json`, `configs/dynamicjepa/{domain_packs,verification}/`
- `Makefile`, `rust-toolchain.toml`, `CLAUDE.md`

## 1. Summary

ContextGraph is an 11-crate Rust workspace (1,361 `.rs` files, ~582K LOC across `crates/` and `tests/`, excluding `target/`) that stores text/code/learner memories as 14-embedder semantic fingerprints in a single RocksDB database (101 column families) and exposes them through 145 / 141 Model Context Protocol tools (count with / without the `llm` feature; asserted by `crates/context-graph-mcp/src/tools/definitions/mod.rs::test_total_tool_count_and_no_duplicates`). When the MCP server is launched with `--mode reality-loop`, the dispatcher additionally registers the 36-tool `cgreality` family, transforming the same binary into a thin facade over the reality engine.

The MCP server (`crates/context-graph-mcp`) speaks JSON-RPC 2.0 over stdio or TCP, optionally running as a long-lived daemon. Each memory is fingerprinted by up to 14 production embedders (`ModelId::production()` per `crates/context-graph-embeddings/src/types/model_id/core.rs:234`) and indexed by per-embedder HNSW (usearch), sparse inverted indexes (E6/E13), or MaxSim token store (E12). Causal (E5) and graph (E8) discovery uses a Hermes-2-Pro-Mistral-7B GGUF loaded in-process through `llama-cpp-2`. A 7-slot learner namespace (E15-E21) plus the `context-graph-dynamicjepa` crate ship the 5090jepa instrument/panel/predictor/planner/guard pipeline, persisted across 28 DynamicJEPA column families and exposed as 35 MCP tools.

The active workstream — **ccreality** — drives SWE-bench Lite repair under one generic harness, using **Claude Code as the outer optimizer** and **Qwen3-Coder-30B-A3B (Q4_K_XL GGUF) as the inner repair actor** loaded in-process through `llama-cpp-2` inside `target/release/reality-loop`. The reality engine binary is the single source of truth for harness writes; the MCP `cgreality` tool family shells out to `target/release/reality-loop harness <subcommand>` per call (the **MCP→CLI shim**) so cargo rebuilds propagate without `/mcp reconnect`. Stdio singleton enforcement is via a flock on `~/.cache/cgreality/reality-loop-stdio.lock`, with takeover-style eviction of stale holders.

## 2. Architecture

| Process / surface | Address | Technology | Purpose |
| --- | --- | --- | --- |
| `context-graph-mcp` (stdio) | inherited stdin/stdout | tokio + JSON-RPC 2.0 | Default MCP transport; `--mode reality-loop` registers 36 cgreality tools on top of the standard registry |
| `context-graph-mcp` (tcp) | `bind_address`:`tcp_port` (default `127.0.0.1:3100`) | tokio TCP, line-delimited JSON | Networked MCP transport |
| `context-graph-mcp --daemon` (server side) | TCP `:3100` (or `--daemon-port`) | Detached child via `spawn_daemon_process` | Single shared server holding GPU-resident models |
| `context-graph-mcp --daemon` (client side) | stdio ↔ TCP relay | `crate::adapters::DaemonProxy` | Forwards stdio JSON-RPC into the running daemon |
| RocksDB database | `storage.path` (typ. `<repo>/contextgraph_data/`) | `rocksdb 0.22`, 101 CFs, shared 256 MB block cache | Sacred durable store for memories, edges, learning, DynamicJEPA |
| HNSW indexes | in-memory (`usearch 2`) | one index per dense embedder + asymmetric duals (E5/E8/E10) | K-NN over 14-embedder fingerprint vectors; rebuilt from CFs on startup |
| Sparse inverted indexes | RocksDB CFs `e6_sparse_inverted`, `e13_splade_inverted` | Custom inverted index (`teleological/indexes`) | Stage-1 recall for E6/E13 |
| Late-interaction MaxSim | RocksDB CF `e12_late_interaction` | E12 token vectors (128D × tokens) | Stage-3 reranking |
| Embedder warm provider | in-process, GPU 0 | Candle 0.10.2 (CUDA), `crates/context-graph-embeddings/src/warm/` | Loads E1-E14 + 7 learner heads into VRAM lazily / eagerly |
| Causal/graph LLM agent | in-process, GPU | `llama-cpp-2 0.1` + GBNF grammars in `models/hermes-2-pro/` | Hermes-2-Pro-Mistral-7B Q5_K_M (~5 GB) for relationship discovery |
| **Inner SWE actor (reality-loop engine)** | in-process, GPU 0 | `llama-cpp-2 0.1` (CUDA) | **Qwen3-Coder-30B-A3B-Instruct UD-Q4_K_XL (~17.67 GB) loaded directly inside `target/release/reality-loop`**; pin key `qwen3_coder_30b_a3b_q4_k_xl` from `config/ccreality_model_pins.json` |
| File watcher background task | in-process tokio task | `crates/context-graph-mcp/src/server/watchers.rs`, `glob 0.3` | Re-ingests files matching watched globs |
| K-NN graph builder | in-process tokio task | `crates/context-graph-storage/src/graph_edges/builder.rs` | Background `BackgroundGraphBuilder` rebuilds typed/embedder edges |
| `context-graph-cli` | local process | `clap 4.5` derive | Operator commands, hooks, capture/inject, DynamicJEPA admin, ccreality helpers |
| `blackbox` (CLI bin) | local process | clap | Black-box harness; phase-13 retention dashboard, FSV |
| `context-graph` (CLI bin) | local process | clap (6-line forwarder) | Auxiliary entry preserved for hooks |
| `reality-loop` (CLI bin) | local process | clap, llama-cpp-2 (cuda), sha2, libc | **Reality engine — owns SWE-bench attempts, smoke, official-eval, targeted-test, replay, verify, AND the `harness` subcommand surface that the MCP cgreality tools shell out to**; auto-discovered from `src/bin/`, NOT declared in `[[bin]]` |
| Per-DB single-writer guard | `<storage.path>/mcp.pid` | `libc::flock` (Unix) | Prevents concurrent RocksDB opens; staleness checked against PID + port + zombie state |
| **Reality-loop stdio singleton** | `~/.cache/cgreality/reality-loop-stdio.lock` | `libc::flock` (Unix) takeover | Prevents orphaned `--mode reality-loop` MCP processes when Claude Code half-opens stdin on `/mcp reconnect`; SIGTERM (2s) → SIGKILL eviction of stale holder |

The MCP server boots in this order (`crates/context-graph-mcp/src/main.rs`): parse CLI → load TOML config (`config/default.toml` or `--config`) → call `set_reality_loop_mode(cli.mode == "reality-loop")` → acquire `mcp.pid` flock (per-DB) → if reality-loop mode, also acquire the `reality-loop-stdio.lock` singleton (with takeover eviction) → optionally spawn or proxy into a daemon → initialise tracing → construct `LazyMultiArrayProvider` (defers GPU model load) and `RocksDbTeleologicalStore` → start watchers and graph builder → enter the chosen `TransportMode`.

The `reality-loop` engine binary is independent of the MCP server: it is invoked either directly by the operator (the `attempt`/`smoke`/`official-eval`/`targeted-test`/`replay`/`verify` subcommands) OR as a fresh subprocess by the cgreality MCP tools (the `harness` subcommand surface). Process-per-call ensures `cargo build --release` propagates immediately without an `/mcp reconnect`. The engine loads the Qwen3-Coder-30B-A3B model in-process via `llama-cpp-2` (CUDA), runs the inner repair-actor agentic loop with grammar-constrained tool calling, applies governed file edits, runs the official SWE-bench Docker evaluator, and writes a fail-closed `<run_root>/measurement-contract.json` plus per-attempt artifacts before declaring success.

## 3. Technology Stack

| Layer | Technology | Source / version |
| --- | --- | --- |
| Toolchain | Rust stable, edition 2021 | `rust-toolchain.toml` (channel = `stable`, components rustfmt/clippy/rust-src), `Cargo.toml` (`rust-version = "1.95"`) |
| Async runtime | `tokio = "1.35"` (full features) | workspace dep |
| Serialization | `serde = "1.0"` derive, `serde_json = "1.0"`, `bincode = "1.3"`, `toml = "0.8"` | workspace deps |
| Errors | `thiserror = "1.0"`, `anyhow = "1.0"` | workspace deps |
| Logging | `tracing = "0.1"`, `tracing-subscriber = "0.3"` (json, env-filter) | workspace deps |
| Config | `config = "0.14"`, `toml`, env override | `crates/context-graph-core/src/config/` |
| Storage | `rocksdb = "0.22"`, `usearch = "2"`, `parking_lot = "0.12"`, `dashmap = "5.5"`, `blake3 = "1.5"` | `crates/context-graph-storage/Cargo.toml` |
| GPU / ML (embeddings) | `candle-core 0.10.2` + `candle-nn 0.10.2` (cuda), `tokenizers 0.22`, `safetensors 0.4`, `half 2.3` | `crates/context-graph-embeddings/Cargo.toml` |
| GPU / ML (causal/graph LLM agents) | `llama-cpp-2 0.1` (optional `llm` and `cuda` features) | `crates/context-graph-causal-agent/Cargo.toml` |
| GPU / ML (inner SWE actor) | `llama-cpp-2 0.1` (cuda, mandatory) loaded by `target/release/reality-loop` | `crates/context-graph-cli/Cargo.toml:59` |
| CUDA target | RTX 5090 / Blackwell, sm_120a, CUDA 13.2 (via cudarc 0.19.x in candle 0.10.2) | `Cargo.toml` workspace comment, `crates/context-graph-cuda` |
| Search & fusion | RRF (`RRF_K`), Weighted RRF, MaxPooling, MaxSim | `crates/context-graph-core/src/fusion/`, `crates/context-graph-storage/src/teleological/search/` |
| HNSW (dense) | `hnsw_rs 0.3` (core abstraction) and `usearch 2` (storage runtime) | `crates/context-graph-core/src/index/`, `crates/context-graph-storage/src/teleological/indexes/` |
| Parsing | `tree-sitter 0.23`, `tree-sitter-rust/python/javascript/typescript 0.23.x` | `crates/context-graph-cli/Cargo.toml`, `crates/context-graph-core/Cargo.toml` |
| Hashing | `sha2 0.10`, `blake3 1.5`, `xxhash-rust 0.8` | per-crate Cargo.toml |
| Caching | `moka 0.12` (sync) for embedding cache | `crates/context-graph-embeddings/Cargo.toml` |
| Parquet export | `parquet 58`, `arrow-array 58`, `arrow-schema 58` | `crates/context-graph-cli/Cargo.toml` |
| HTTP / network | `reqwest 0.12` (rustls), `zip 7.2.0`, `rusqlite 0.32` (bundled) | `crates/context-graph-cli/Cargo.toml` (model download, share/import flows, BlackBoxFix ledger) |
| MCP CLI parsing | hand-rolled (`crates/context-graph-mcp/src/main.rs`); CLI uses `clap 4.5` | per main.rs |
| FFI / kernels | `crates/context-graph-cuda/src/ffi/` (CUDA driver API; FAISS bindings gated behind `faiss-working`) | `crates/context-graph-cuda/Cargo.toml` |
| JSON Schema | `jsonschema 0.18` (draft202012) for cgreality recommendation validation | `crates/context-graph-mcp/Cargo.toml` |
| Test utilities | `tempfile 3.10`, `tokio-test 0.4`, `serial_test 3`, `criterion 0.5`, `futures 0.3` | per-crate `[dev-dependencies]` |

## 4. Workspace Crates

Source: `Cargo.toml` `[workspace.members]` (11 crates) and `default-members` (10 — benchmark excluded).

| Package | `default` features | Purpose | Default-members? |
| --- | --- | --- | --- |
| `context-graph-mcp` | `cuda`, `llm` | MCP JSON-RPC server; 145/141 standard tools + 36 cgreality tools when `--mode reality-loop` | Yes |
| `context-graph-core` | — | Domain types, fingerprints, retrieval, learner, learning, teleological, error hierarchy | Yes |
| `context-graph-cuda` | `cuda` | CUDA driver API kernels, similarity, HDBSCAN, allocation, FAISS gate | Yes |
| `context-graph-embeddings` | `candle` (mandatory) | 14 production + 7 learner embedders, warm provider, quantization, training | Yes |
| `context-graph-storage` | `cuda` | RocksDB facade, 101 CFs, HNSW/inverted/MaxSim search engines, edge repo | Yes |
| `context-graph-dynamicjepa` | — | DynamicJEPA service: tiny-model training, artifacts, predictions, MI audit, transfer | Yes |
| `context-graph-cli` | `cuda` | Operator + hook CLI; 3 declared bins (`context-graph-cli`, `context-graph`, `blackbox`) + `reality-loop` (auto-discovered, 18,137 LOC) | Yes |
| `context-graph-causal-agent` | — (opt-in `llm`, `cuda`) | LLM-gated causal discovery (`CausalDiscoveryLLM`) | Yes |
| `context-graph-graph-agent` | — (opt-in `llm`, `cuda`, `test-utils`) | LLM-gated graph relationship discovery; reuses causal-agent LLM | Yes |
| `context-graph-test-utils` | — | Shared real-data test fixtures (no mocks) | Yes |
| `context-graph-benchmark` | — (opt-in `real-embeddings`, `benchmark-tests`) | 45 benchmark binaries; excluded from default builds | **No** |

## 5. MCP Tool Domains

Source: `crates/context-graph-mcp/src/tools/definitions/mod.rs::get_tool_definitions`.
Standard registry totals (asserted by `test_total_tool_count_and_no_duplicates`, lines 144-149): **145 with `llm`, 141 without**. When `crate::tools::is_reality_loop_mode() == true`, an additional **36 cgreality tools** are appended (`reality::definitions()`). Per-category counts asserted by `test_submodule_counts` (lines 172-209).

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
| **reality (cgreality)** | **36** (mode-gated; not part of 145/141 base) | `definitions/reality.rs` | `handlers/tools/reality_loop/{alter,analyze,autoresearch,interact,lock,optimizer,path_policy,schema_validator,shift_log}.rs` (5,627 LOC across 12 files) |

The cgreality tool family covers: read artifacts (`reality_latest_root`, `reality_attempt_summary`, `reality_official_report`, `reality_problem_packet`, `reality_signal`, `dynamicjepa_reality_for_attempt`, `reality_failure`, `reality_trigger_decision`, `reality_harness_transitions`, `reality_compare_attempts`, `reality_audit_trail`, `reality_replay_artifact`); spawn engine work (`reality_run_attempt`, `reality_run_smoke`, `reality_run_official_eval`, `reality_run_targeted_test`, `reality_recompile_reward`, `reality_query_ledger`); harness writes via MCP→CLI shim (`harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_git_diff`, `harness_git_status`, `harness_verify_state`); optimizer record (`optimizer_record_decision`, `optimizer_record_recommendation`, `optimizer_record_harness_transition`); shift detection (`reality_shift_log`, `reality_shift_compare_to_my_view`); autoresearch (`experiment_registry_list/get/propose/update_outcome`, `champion_state_get/promote`, `attempts_history_query`).

Method dispatch is restricted to MCP `initialize`, `notifications/initialized`, `shutdown`, `tools/list`, `tools/call` (`crates/context-graph-mcp/src/handlers/core/dispatch.rs`); all tool access is via `tools/call`.

See [08_mcp_tools_reference.md](08_mcp_tools_reference.md) for the full inventory and DTOs.

## 6. Entry Points

| Entry | Kind | Source |
| --- | --- | --- |
| `context-graph-mcp` | binary | `crates/context-graph-mcp/src/main.rs` (3,016 LOC) |
| `context-graph-cli` | binary | `crates/context-graph-cli/src/main.rs` |
| `context-graph` | binary (auxiliary, 6 LOC stub) | `crates/context-graph-cli/src/bin/context-graph.rs` |
| `blackbox` | binary (24 LOC) | `crates/context-graph-cli/src/bin/blackbox.rs` |
| `reality-loop` | binary (auto-discovered, NOT in `[[bin]]`) | `crates/context-graph-cli/src/bin/reality-loop.rs` (18,137 LOC); `tokio::main`; subcommands `attempt`, `smoke`, `official-eval`, `targeted-test`, `replay`, `verify`, `harness {open-window, apply-line-window-edit, run-command, verify-state}` |
| `train-causal` | binary | `crates/context-graph-embeddings/src/bin/train_causal.rs` |
| `benchmark-report`, `realdata-bench`, `gpu-bench`, `mcp-bench`, etc. | 45 binaries | `crates/context-graph-benchmark/src/bin/*.rs` (build with `cargo build -p context-graph-benchmark`) |

MCP server CLI flags (parsed in `main.rs::CliArgs`): `--config <path>`, `--transport <stdio|tcp>`, `--mode <default|reality-loop>`, `--port <u16>`, `--bind <addr>`, `--warm-first`, `--no-warm`, `--daemon`, `--daemon-port <u16>`, hidden `--daemon-server-only`. Environment overrides: `CONTEXT_GRAPH_TRANSPORT`, `CONTEXT_GRAPH_TCP_PORT`, `CONTEXT_GRAPH_BIND_ADDRESS`, `CONTEXT_GRAPH_DAEMON`, `CONTEXT_GRAPH_DAEMON_PORT`, `CONTEXT_GRAPH_MCP_QUIET`, `CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT`, `CONTEXT_GRAPH_STORAGE_PATH`, `CONTEXT_GRAPH_MODELS_PATH`, `CONTEXT_GRAPH_WATCHER_ENABLED`, `CONTEXT_GRAPH_WARM_FIRST`. Priority: CLI > env > config file > defaults. See [10_cli_reference.md](10_cli_reference.md).

`reality-loop attempt` defaults (`crates/context-graph-cli/src/bin/reality-loop.rs:26-75`):
- `runtime_root = /tmp/contextgraph-rust-reality-loop`
- `model = qwen3-coder:30b-a3b` (display name; bytes loaded from `inner_model_path`)
- `inner_model_path = models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf`
- `inner_model_pin_path = config/ccreality_model_pins.json`, `inner_model_pin_key = qwen3_coder_30b_a3b_q4_k_xl`
- `gpu_min_free_mb = 1536` (steady-state); `gpu_preload_min_free_mb = 18432` (~18 GB pre-load headroom)
- `inner_min_context_tokens = 128_000`, `inner_max_new_tokens = 8192`, `inner_temperature = 0.0`, `inner_top_p = 0.9`, `inner_batch_size = 2048`, `inner_seed = 1`
- `agent_tool_rounds = 24`, `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP = 20`, `REPEATED_ERROR_DEADLOCK_LEN = 5`
- `dataset = princeton-nlp/SWE-bench_Lite`, `official_docker_host = unix:///var/run/docker.sock`, `official_docker_config = /home/cabdru/.cache/contextgraph-docker-config`
- `dynamicjepa_reality_domain_pack = contextgraph_swe_reality_loop_v1`

Makefile targets: `build` (`build-rust` + `build-python`), `build-rust` (release), `build-debug`, `test` / `test-rust` / `test-python-smoke`, `test-mcp`, `test-e2e`, `check`, `lint` / `clippy`, `trim`, `clean`, `clean-all`, `clean-deep`, `disk-check`, `release-candidate-archive`, `release-candidate-archive-v2`. Every build/test target invokes `scripts/trim-stale-deps.sh` to keep `target/` bounded.

## 7. Project Directory Structure

| Path | Role |
| --- | --- |
| `Cargo.toml` | Workspace manifest (11 members, 10 default-members, profiles) |
| `Cargo.lock` | Locked dep versions |
| `Makefile` | Build/test/lint orchestration with auto-trim |
| `rust-toolchain.toml` | Pinned toolchain (`stable`, rustfmt/clippy/rust-src) |
| `Dockerfile` | Container build |
| `CLAUDE.md` | Project-level conventions and ccreality optimizer contract (loaded automatically each Claude Code session) |
| `crates/` | 11 workspace crates (see Section 4) |
| `config/{default,development,production,test}.toml` | TOML configs consumed by `Config::from_path` |
| `config/ccreality_model_pins.json` | Inner-actor model pin registry (currently `qwen3_coder_30b_a3b_q4_k_xl`) |
| `config/ccreality_recommendation_schema.json` | JSON Schema validating optimizer recommendation artifacts |
| `config/ccreality_governed_paths.json` | Phase-5 allow/deny policy file (KEPT on disk; the gate has been REMOVED from `path_policy::ensure_path_allowed` as of 2026-05-07 — the file is retained as documentation/library surface, the unit tests still load it) |
| `config/reward_spec.verified_outcome.json` | Reward spec for verified-outcome mode |
| `configs/dynamicjepa/domain_packs/` | 5 domain packs: `counter_to_grid.v1.toml`, `career_taxonomy.v1.toml`, `counter_world.v1.toml`, `gridworld_5x5.v1.toml`, `contextgraph_swe_reality_loop.v1.toml` (the latter ties DynamicJEPA to the reality loop) |
| `configs/dynamicjepa/verification/` | 19 fixture files (manifests, JSONL corpora, train configs) |
| `contextgraph_data/` | Default RocksDB database (SST + WAL + `mcp.pid` flock) |
| `models/` | Pretrained weights (E1 semantic, E5 causal, E7 code-1536, E10 contextual, E11 kepler, E12 late-interaction, E13 splade-v3, E14 bge-m3-dense, learner heads, `hermes-2-pro/*.gguf` + `*.gbnf` grammars, `qwen3-coder-30b-a3b/*.gguf` for the inner SWE actor) |
| `gemmafrontend/` | Python evidence console (`server.py`, `manual_fsv.py`, `model_benchmark.py`, `model_prompt_matrix.py`); SWE-bench refinement is Rust-only — Python is observability/inspection |
| `scripts/` | 75 top-level entries (62 Python); `scripts/v4_2/` (34 files: CAMD-DPO, FSV), `scripts/5090jepa/` (release helpers), `scripts/exp2/`, `scripts/shakespeare_*.py` (~30 files), `scripts/download_models.py`, daemon helpers |
| `tasks/swebench-lite/*.json` | 300-instance SWE-bench Lite manifest queue (the ccreality work queue) |
| `docs/ccreality/` | 21 ccreality design docs (canonical entry: `00_INDEX.md`; runbook: `19_MIGRATION_RUNBOOK.md`) |
| `docs/`, `docs2/`, `backupdocs/`, `reference/` | Planning + documentation (this directory is `docs2/contextgraphspecs/`) |
| `.claude/` | Claude Code config: `settings.json`, `agents/cc-reality-*.md`, `loop.md`, `skills/` |
| `memory/` | SMAP shared mind: `context/`, `decisions/`, `discoveries/`, `handoff/`, `journal/`, `blockers/`, `patterns/`, `reports/` |
| `agents/` | SMAP agent roster + role definitions |
| `tmp/`, `target/`, `data/`, `artefacts/`, `benchmarks/` | Build outputs and scratch |
| `clipcannon/` | Python media subproject (uv-managed) |
| `shakespear/` | Shakespeare empirical pipeline working dir |
| `pyproject.toml` | Top-level Python project shim |

## 8. Storage Tier Classification

| Path | Class | Behavior |
| --- | --- | --- |
| RocksDB at `storage.path` (typ. `contextgraph_data/`) | **Sacred durable** | All 101 CFs (11 base + 41 teleological + 14 quantized + 5 code + 2 causal + 28 DynamicJEPA); single-writer enforced by `mcp.pid` flock; pre-production project, no auto-migrations |
| usearch HNSW indexes (per dense embedder) | **Regenerable runtime** | Rebuilt from `fingerprints` CF on startup; `try_load_hnsw_indexes()` returns `Ok(true)` when ANY persisted index loads |
| Sparse inverted indexes for E6/E13 | **Regenerable** | CFs `e6_sparse_inverted`, `e13_splade_inverted` rebuilt from fingerprints |
| MaxSim store for E12 | **Regenerable** | CF `e12_late_interaction` |
| `models/` | Regenerable runtime asset | Weights downloaded by `scripts/download_models.py` and embedder loaders |
| `target/` | Regenerable | Cargo artifacts; `Makefile` calls `scripts/trim-stale-deps.sh` after each build |
| `<storage.path>/mcp.pid` | Ephemeral | Unix flock guard with stale-lock detection (zombie-aware) |
| `~/.cache/cgreality/reality-loop-stdio.lock` | Ephemeral | Reality-loop MCP stdio singleton flock; takeover-evicts orphans when Claude Code half-opens stdin on `/mcp reconnect` |
| `~/.cache/cgreality/active_runtime_root` | Ephemeral hint | Records the most recent reality-loop runtime root |
| `tmp/`, `/tmp/contextgraph-rust-reality-loop/`, `/tmp/cgreality-smoke/`, `/tmp/gemmafrontend-runs/` | Ephemeral | Reality-loop run roots, smoke roots, gemmafrontend snapshots |

See [04_database_schema.md](04_database_schema.md) for per-CF schemas and [09_storage_layer.md](09_storage_layer.md) for read/write paths.

## 9. Error Hierarchy

Source: `crates/context-graph-core/src/error/{mod,unified,sub_errors,legacy,conversions}.rs`, `crates/context-graph-storage/src/error.rs`, and `crates/context-graph-mcp/src/handlers/tools/reality_loop/errors.rs`.

| Error type | Location | Notes |
| --- | --- | --- |
| `ContextGraphError` | `core/src/error/unified.rs` | Top-level enum with `#[from]` conversions; `error_code()` mapping to JSON-RPC codes (-32004 storage, -32005 embedding, -32602 validation, etc.) |
| `EmbeddingError` | `core/src/error/sub_errors.rs` | Model loading, generation, quantization, dimension issues |
| `StorageError` (core) | `core/src/error/sub_errors.rs` | High-level storage failures wrapped from `context_graph_storage::StorageError` |
| `IndexError` | `core/src/error/sub_errors.rs` | HNSW search and corruption |
| `ConfigError` | `core/src/error/sub_errors.rs` | Missing config, invalid values, parse failures |
| `GpuError` | `core/src/error/sub_errors.rs` | Device init, memory, kernel failures |
| `McpError` | `core/src/error/sub_errors.rs` | Request validation, authorization, protocol violations |
| `CoreError` / `CoreResult` | `core/src/error/legacy.rs` | Retained for backwards compatibility |
| `Result<T>` | `core/src/error/unified.rs` | `type Result<T> = std::result::Result<T, ContextGraphError>` |
| `StorageError` (storage crate) | `storage/src/error.rs` | RocksDB-level: `OpenFailed`, `ColumnFamilyNotFound`, `WriteFailed`, `ReadFailed`, `FlushFailed`, `NotFound`, `Serialization`, `ValidationFailed`, `IndexCorrupted`, `Internal`. `From<rocksdb::Error>` conversion. |
| `TeleologicalStoreError`, `QuantizedStorageError`, `CodeStorageError`, `GraphEdgeStorageError` | `crates/context-graph-storage/src/teleological`, `code`, `graph_edges` | Per-subsystem typed errors |
| `EdgeError` | `core/src/graph_linking/` | K-NN graph edge construction errors |
| `DynamicJepaError` | `core/src/dynamicjepa/error.rs` | DynamicJEPA pipeline errors |
| `WeightProfileError` | `core/src/weights/mod.rs` | Profile validation |
| `RealityError` | `crates/context-graph-cli/src/bin/reality-loop.rs` | Reality-engine typed failure: `error_code` (e.g. `RUST_REALITY_OFFICIAL_EVAL_REQUIRED`, `RUST_REALITY_PATCH_TOUCHES_BENCHMARK`, `RUST_REALITY_REPEATED_ERROR_DEADLOCK`), `field_path`, `remediation`, `details`, `source_of_truth`; serialized to `<runtime_root>/failures/*.json` |
| `CCRealityError` | `crates/context-graph-mcp/src/handlers/tools/reality_loop/errors.rs` | cgreality MCP tool error: `error_code` prefix `CCREALITY_*`, `message`, `field_path`, `remediation`, `details`, `source_of_truth`; surfaced as `isError: true` JSON-RPC responses with structured content |

`ContextGraphError::error_code()` returns negative i32 codes per JSON-RPC 2.0 spec; `crates/context-graph-mcp/src/protocol.rs` defines the `error_codes` constants used by handlers.

## 10. Key Subsystems

**MCP server.** `crates/context-graph-mcp/src/main.rs` (3,016 lines) parses CLI, manages the per-DB `mcp.pid` flock and (in reality-loop mode) the `~/.cache/cgreality/reality-loop-stdio.lock` singleton, optionally spawns/proxies into a daemon, then enters either stdio or TCP transport. `server/transport.rs` implements `read_line_bounded` framing; `server/watchers.rs` runs file-watcher and graph-builder background tasks. Method dispatch in `handlers/core/dispatch.rs`; `tools/call` flows through `handlers/tools/dispatch.rs` (303 lines). `LazyMultiArrayProvider` defers GPU model load. See [08_mcp_tools_reference.md](08_mcp_tools_reference.md).

**cgreality MCP tool family.** `crates/context-graph-mcp/src/handlers/tools/reality_loop/` (12 files, 5,627 lines) implements the 36-tool surface. The `alter.rs` module hosts the **MCP→CLI shim**: `harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_git_diff`, `harness_git_status`, `harness_verify_state` all spawn `target/release/reality-loop harness <subcommand>` as a fresh subprocess (`spawn_harness_subcommand`), so the on-disk binary is the source of truth and `cargo build --release` propagates without `/mcp reconnect`. `lock.rs` (174 lines) owns the per-attempt edit-lock; `path_policy.rs` (356 lines) keeps the Phase-5 allow/deny library surface but the runtime gate (`ensure_path_allowed`) only rejects empty / NUL / newline paths now (operator directive 2026-05-07: "the AI needs to be able to do anything it needs to do"). `optimizer.rs` writes recommendation/decision/transition artifacts; `schema_validator.rs` validates against `config/ccreality_recommendation_schema.json`; `shift_log.rs` appends per-session shift records; `autoresearch.rs` (761 lines) implements experiment registry + champion state.

**Storage (RocksDB + 101 CFs).** `crates/context-graph-storage/src/lib.rs` exposes `RocksDbTeleologicalStore`, `EdgeRepository`, `CodeStore`, and DynamicJEPA stores. CFs split across four registries: `column_families.rs` (11 base: 8 originals + `embedder_edges`/`typed_edges`/`typed_edges_by_type`), `teleological/column_families.rs` (41 + 14 + 5 + 2 = 62 teleological CFs), and `dynamicjepa/column_families.rs` (28). All CFs share a 256 MB block cache. `TOTAL_COLUMN_FAMILIES = 101`. See [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md).

**Embeddings (14 production + 7 learner).** `crates/context-graph-embeddings/src/lib.rs` mandates the `candle` feature. `ModelId::production()` returns 14 variants `Semantic, TemporalRecent, TemporalPeriodic, TemporalPositional, Causal, Sparse, Code, Graph, Hdc, Contextual, Kepler, LateInteraction, Splade, BgeM3Dense`. `Entity = 10` (legacy 384D MiniLM) coexists with `Kepler = 13` (768D KEPLER) and is excluded from `production()` (kept in `all()`, which returns 15). Native dims: E1=1024, E2/E3/E4=512, E5=768 asymmetric, E6=30,522 SPLADE vocab, E7=1536, E8=1024 asymmetric, E9=10,000 binary, E10=768 asymmetric, E11=768 (Kepler), E12=128/token, E13=30,522 SPLADE vocab, E14=1024. Warm provider in `src/warm/`. Learner namespace (`src/learner/mod.rs`) defines E15-E21 (`LEARNER_EMBEDDER_COUNT = 7`, `UTL_PLANNED_TOTAL_EMBEDDERS = 21`). See [05_embedding_subsystem.md](05_embedding_subsystem.md).

**Search & fusion.** `crates/context-graph-core/src/retrieval/` and `src/fusion/`. Pipeline strategy: E13 sparse recall → E1 dense scoring → E12 MaxSim rerank. 20 weight profiles in `crates/context-graph-core/src/weights/mod.rs::WEIGHT_PROFILES`. See [06_search_and_fusion.md](06_search_and_fusion.md).

**Graph & causal.** `crates/context-graph-core/src/graph_linking/` defines K-NN edges and 20 typed edge relations. `src/graph/` and `src/causal/` implement E5/E8 asymmetric similarity. Edge writes in `crates/context-graph-storage/src/graph_edges/` and `teleological/rocksdb_store/typed_edge_*.rs`. See [07_graph_and_causal.md](07_graph_and_causal.md).

**LLM agents (Hermes-2-Pro).** `crates/context-graph-causal-agent/src/llm/mod.rs` loads `models/hermes-2-pro/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf` (~5 GB) via `llama-cpp-2` with three GBNF grammars (`causal_analysis.gbnf`, `graph_relationship.gbnf`, `validation.gbnf`). ChatML prompt (`prompt.rs`). `context-graph-graph-agent` reuses `CausalDiscoveryLLM` rather than instantiating its own model. Both crates gate the LLM path behind a `llm` feature; `graph-agent/src/stubs.rs` provides no-op replacements when disabled. See [07_graph_and_causal.md](07_graph_and_causal.md).

**Inner SWE actor (Qwen3-Coder-30B-A3B).** Loaded **directly inside `target/release/reality-loop`** through `llama-cpp-2 0.1` (cuda) — there is no Ollama hop, no HTTP indirection. Pin: `qwen3_coder_30b_a3b_q4_k_xl` resolved from `config/ccreality_model_pins.json`. Steady-state VRAM budget 1,536 MB free; pre-load budget 18,432 MB. Context up to 128k tokens, max 8,192 new tokens per turn, temperature 0.0, top_p 0.9, batch 2,048, seed 1. Tool calling is grammar-constrained via `LlamaSampler` + `GrammarTriggerType`. Hard caps: 20 rounds per attempt, 5 consecutive identical errors triggers `RUST_REALITY_REPEATED_ERROR_DEADLOCK`. See [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md).

**Teleological.** `crates/context-graph-core/src/teleological/` holds the comparator, embedder/group definitions, matrix search, profile, services, synergy matrix, Tucker core, and `TELEOLOGICAL_VECTOR_DIM = 13` (E14 excluded from the compact teleological vector). See [13_teleological_subsystem.md](13_teleological_subsystem.md).

**Constellation / Contrastive / Typed edges.** Reservoir sampling + Welford streaming statistics in `core/src/constellation/`. Disagreement pair mining in `core/src/contrastive/`. `TypedEdgeTrainingRecord` builders in `core/src/typed_edge_export/`. Storage in `teleological/rocksdb_store/{constellation,contrastive,typed_edge_export,llm_validation}.rs`. See [17_constellation_subsystem.md](17_constellation_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

**Training.** `crates/context-graph-core/src/training/` defines `TrainingRecord` v3 (memory + 14 embeddings + 91 cross-correlations + 6D group alignments + edges + causal labels). CF `training_records`. `crates/context-graph-embeddings/src/training/` and `bin/train_causal.rs` provide the E5 LoRA training driver. See [16_training_subsystem.md](16_training_subsystem.md).

**Learning subsystem (UTL).** `core/src/learning/`, `src/learner/`, `src/learner_training.rs` implement learner profiles, modalities, S/C/E deltas, L = ΔS·ΔC·ΔE, M(t) memory traces, and goal centroids. 13 learner-state CFs. See [20_learning_subsystem.md](20_learning_subsystem.md).

**DynamicJEPA.** `crates/context-graph-dynamicjepa/` (service, artifact, transfer, MI audit, model, config) plus `crates/context-graph-core/src/dynamicjepa/` (~25 files: domain pack, instrument, panel, planner, predictor, guard, surprise, validation, verification, schema, dataset, trajectory, binding, pair_kinds, features, meaning_compression). Storage data plane in `crates/context-graph-storage/src/dynamicjepa/` (28 CFs). 35 MCP tools. The `contextgraph_swe_reality_loop_v1` domain pack ties the DJ pipeline to reality-loop attempts (`DEFAULT_DYNAMICJEPA_SKILL_ID = d91e5e0c-3557-57a9-8b48-0fa2acb429a0`). See [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md).

**Reality loop (SWE-bench).** `crates/context-graph-cli/src/bin/reality-loop.rs` (18,137 LOC). Subcommands: `attempt` (run inner-actor on one task), `smoke` (synthetic, no model, no Docker), `official-eval` (re-run Docker eval on existing patch), `targeted-test` (one fail_to_pass pytest), `replay` (rebuild signal packet), `verify` (full-state verify all artifacts), `harness {open-window, apply-line-window-edit, run-command, verify-state}` (called from MCP shim). Validates task manifest against immutable `contextgraph_reality_measurement_contract_v1`, drives Qwen3-Coder-30B-A3B in-process, applies governed line-window edits, runs official Docker SWE-bench evaluator, only declares success when `resolved_ids` contains the instance. Every step SHA-256 read-after-write; every artifact persisted into BlackBoxFix (`LEDGER_SCHEMA_VERSION = 14`) and a `reality-events.jsonl` event stream. Hard rejections: missing `--official-eval-each-attempt`, dataset ≠ `princeton-nlp/SWE-bench_Lite`, malformed instance/repo/SHA, dirty worktree, base_commit ≠ git HEAD, candidate patch touching benchmark/test/fixture/harness paths. See [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md).

**File watcher.** `core/src/types/file_index.rs`, definitions in `definitions/file_watcher.rs`, background task in `crates/context-graph-mcp/src/server/watchers.rs`. CF `file_index`. Reconciliation tool removes orphans.

**Topic clustering.** `core/src/clustering/` implements HDBSCAN and BIRCH. Storage CFs `topic_profiles`, `topic_portfolio`. See [12_clustering_and_topics.md](12_clustering_and_topics.md).

**Capability matrix.** `get_capability_matrix` (`handlers/tools/capability_tools.rs`) returns embedders, CFs, tools, and runtime state in a compact JSON map for agents to discover the surface.

## 11. Documented Source-Level Inconsistencies

| Item | Source |
| --- | --- |
| `TELEOLOGICAL_VECTOR_DIM = 13` while `NUM_EMBEDDERS = 14` (E14 excluded from compact vector) | `crates/context-graph-core/src/types/fingerprint/teleological/types.rs:12`; `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs:81` |
| `ModelId::all()` returns 15 variants, `production()` returns 14, `NUM_EMBEDDERS = 14`. `Entity` is legacy 384D MiniLM, `Kepler` is production 768D | `crates/context-graph-embeddings/src/types/model_id/core.rs:200,234` |
| Standard test asserts 145/141 tools but the registry CAN expose 181/177 when `--mode reality-loop` flips `is_reality_loop_mode()` to true. The tests do not exercise the reality-loop branch beyond `test_reality_loop_lightweight_registry_only_lists_callable_tools` | `crates/context-graph-mcp/src/tools/definitions/mod.rs:125-130, 144-149, 213-223` |
| `crates/context-graph-storage/src/column_families.rs` doc comment (`:41-83`) lists 8 base CFs; constant `TOTAL_COLUMN_FAMILIES = 101` accounts for 11 base after graph-linking additions (`embedder_edges`, `typed_edges`, `typed_edges_by_type`) | `crates/context-graph-storage/src/column_families.rs:41-83,408` |
| `Cargo.toml` `rust-version = "1.95"` but `rust-toolchain.toml` channel = `stable` — toolchain selection driven by stable channel; rust-version is a floor, not a pin | `Cargo.toml`, `rust-toolchain.toml` |
| Quantized embedder CF dimension docstrings (e.g. "PQ-8 8 bytes from 1024D") describe target compression, not necessarily current write path; CFs were "reserved" historically and only partially wired | `crates/context-graph-storage/src/teleological/column_families.rs:660-665` |
| `config/default.toml` uses legacy section names (`[general]`, `[server.transport]`, `[storage].sqlite_path`, `[embeddings]`, `[graph]`) that do NOT match the current Rust `Config` shape — fields not present in the Rust types are silently ignored by serde; the Rust types in `crates/context-graph-core/src/config/sub_configs.rs` are authoritative | `config/default.toml`; `crates/context-graph-core/src/config/mod.rs:42` |
| `reality-loop` binary is auto-discovered by Cargo (file at `src/bin/reality-loop.rs`) but is NOT declared in `crates/context-graph-cli/Cargo.toml` `[[bin]]` blocks alongside the other three. `cargo metadata` still enumerates it, but the manifest is silent. | `crates/context-graph-cli/Cargo.toml:15-25` |
| `config/ccreality_governed_paths.json` is referenced by tests in `path_policy.rs` and is loaded via `GovernedPathsPolicy::load_from`, but the runtime gate in `path_policy::ensure_path_allowed` (line 268) DOES NOT consult it — only rejects empty / NUL / newline paths. The file exists as documentation-of-intent and a unit-test fixture; the AI is BEHAVIORALLY trusted to honor the deny set per `CLAUDE.md §8`. | `crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs:244-280` |
| `path_policy.rs:268` doc comment explicitly says "**GATE FULLY REMOVED 2026-05-07**" and lists the four rules removed (allow-list match, deny-list, require-relative-path, no-traversal). The Phase-5 policy file's allow/deny structure remains usable as a library, just not invoked. | `path_policy.rs:244-280` |
| `crates/context-graph-cli/Cargo.toml` declares 3 `[[bin]]` blocks (`context-graph-cli`, `context-graph`, `blackbox`), but the crate ships 4 binaries — `reality-loop` is auto-discovered. This deviates from the explicit-declaration pattern used by the other three. | `crates/context-graph-cli/Cargo.toml:15-25`, `crates/context-graph-cli/src/bin/reality-loop.rs` |
| Cost-cap machinery: previously the codebase had `cost_cap_reached` blocked status and `cost_estimate_usd` schema fields. These are GONE per operator directive (no per-turn or per-day spend caps; the system runs locally). `CLAUDE.md §11.10` codifies this. | `CLAUDE.md`, current source absence |
