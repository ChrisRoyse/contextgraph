# 01 — System Overview

**Source files covered:**
- `Cargo.toml`, `rust-toolchain.toml`, `config/default.toml`
- `crates/context-graph-mcp/Cargo.toml`, `src/main.rs`, `src/lib.rs`, `src/protocol.rs`, `src/server/mod.rs`, `src/server/transport.rs`, `src/server/watchers.rs`
- `crates/context-graph-mcp/src/tools/names.rs`, `src/tools/definitions/mod.rs` (+ 24 per-category definition modules)
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` (+ 25 per-category handler modules)
- `crates/context-graph-core/src/lib.rs`, `src/error/unified.rs`, `src/error/sub_errors.rs`
- `crates/context-graph-storage/src/lib.rs`, `src/column_families.rs`, `src/teleological/column_families.rs`
- `crates/context-graph-embeddings/src/lib.rs`, `src/bin/train_causal.rs`
- `crates/context-graph-cli/src/main.rs`
- `crates/context-graph-cuda/Cargo.toml`, `crates/context-graph-causal-agent/Cargo.toml`, `crates/context-graph-graph-agent/Cargo.toml`
- `crates/context-graph-benchmark/Cargo.toml`, `crates/context-graph-test-utils/Cargo.toml`
- `scripts/v4_2/` (15 Python modules)

---

## 1. Summary

Context Graph is a Rust workspace that encodes every stored memory through 13 independent embedding spaces (E1-E13) plus derived cross-embedding tensors, persists everything in a single RocksDB database with 59 column families, and exposes 75 MCP (Model Context Protocol) tools over JSON-RPC 2.0 for retrieval, curation, causal/graph reasoning, topic/constellation analytics, and training-data export. The stack targets NVIDIA RTX 5090 (Blackwell) with CUDA 13.x, runs embeddings on HuggingFace Candle 0.9.2-alpha, and runs local LLM inference (Hermes-2-Pro-Mistral-7B) through `llama-cpp-2` with GBNF-constrained decoding. A Python-based CAMD-DPO training-data subsystem lives under `scripts/v4_2/` and reads directly from the daemon's RPC surface + an LMDB sidecar store.

---

## 2. Architecture

### 2.1 Processes, Binaries, Transport, Ports

| Process | Binary | Transport | Port | Purpose |
|---|---|---|---|---|
| MCP stdio server | `context-graph-mcp` | stdio | - | Single-client JSON-RPC over stdin/stdout (default) |
| MCP TCP server | `context-graph-mcp --transport tcp` | TCP | 8080 (default via config) | Networked JSON-RPC, multi-client |
| MCP daemon | `context-graph-mcp --daemon` | TCP | 3100 | Shared server across terminals; single VRAM load; stdio-to-TCP proxy for child clients |
| CLI | `context-graph-cli` | - | - | Hooks, session persistence, memory capture, topic/divergence queries, warmup, directory watch |
| Causal trainer | `train-causal` | - | - | Offline E5 causal-embedder fine-tuning (3-stage LoRA pipeline) |
| CAMD-DPO scripts | `scripts/v4_2/*.py` | TCP (to daemon) + LMDB | 3100 | Python pair-mining + reward + extended-ensemble + DPO training (see [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md)) |

### 2.2 Crate Dependency Diagram

```
context-graph-cli
  └── context-graph-mcp
       ├── context-graph-core
       ├── context-graph-cuda        ──┐
       ├── context-graph-embeddings  ──┼──► context-graph-core
       ├── context-graph-storage     ──┼──► context-graph-core
       │                                └──► context-graph-embeddings
       ├── context-graph-causal-agent ──► core + storage + embeddings
       └── context-graph-graph-agent  ──► core + storage + embeddings + causal-agent

context-graph-benchmark (default-members = NO)
  └── core + storage (+ embeddings/mcp/graph-agent/causal-agent behind features)

context-graph-test-utils
  └── core + storage
```

Dependency layers (bottom → top):

| Layer | Crate | Depends on |
|---|---|---|
| 0 | `context-graph-core` | - |
| 1 | `context-graph-cuda` | core |
| 2 | `context-graph-embeddings` | core, cuda |
| 3 | `context-graph-storage` | core, embeddings, cuda (opt) |
| 4 | `context-graph-causal-agent` | core, storage, embeddings |
| 5 | `context-graph-graph-agent` | core, storage, embeddings, causal-agent |
| 6 | `context-graph-mcp` | all of the above |
| 7 | `context-graph-cli` | core, mcp, storage, embeddings |
| util | `context-graph-test-utils` | core, storage |

### 2.3 Workspace Members

Declared in `Cargo.toml`:

| Crate | default-members | Role |
|---|---|---|
| `context-graph-mcp` | yes | JSON-RPC server binary |
| `context-graph-core` | yes | Domain types, traits, business logic |
| `context-graph-cuda` | yes | GPU abstractions (CUDA 13.x / Blackwell) |
| `context-graph-embeddings` | yes | 13-embedder pipeline, warm loader, quantization, E5 trainer |
| `context-graph-storage` | yes | RocksDB + HNSW + inverted indexes |
| `context-graph-cli` | yes | CLI tool (`context-graph-cli`) |
| `context-graph-causal-agent` | yes | LLM causal discovery service |
| `context-graph-graph-agent` | yes | LLM graph discovery service |
| `context-graph-test-utils` | yes | Shared test fixtures |
| `context-graph-benchmark` | **no** | Benchmark suite, build explicitly via `-p` |

---

## 3. Technology Stack

| Layer | Technology | Version / Constraint |
|---|---|---|
| Language | Rust | edition 2021, MSRV 1.75, stable channel |
| Target triple | `x86_64-unknown-linux-gnu` | `rust-toolchain.toml` |
| Async runtime | tokio | 1.35 (full features) |
| Serialization | serde 1.0, serde_json 1.0, bincode 1.3, toml 0.8 | workspace |
| Error | thiserror 1.0, anyhow 1.0 | workspace |
| Logging | tracing 0.1, tracing-subscriber 0.3 (json + env-filter) | workspace |
| IDs / time | uuid 1.6 (v4, v5, serde), chrono 0.4 (serde) | workspace |
| GPU ML | candle-core / candle-nn 0.9.2-alpha (cuda) | workspace (RTX 5090 Blackwell / CUDA 13.x) |
| LLM runtime | llama-cpp-2 0.1 | GGUF + GBNF grammar, optional via `llm` feature |
| Storage | rocksdb 0.22 | core crate |
| ANN (core) | hnsw_rs 0.3 | core crate |
| ANN (storage) | usearch 2 | storage crate |
| Sparse ops | custom inverted index + SPLADE in-memory impl | core/storage |
| Concurrency | parking_lot 0.12, dashmap 5.5, rayon 1.10 | direct |
| Concurrent cache | moka 0.12 (sync) | embeddings |
| Tokenization | tokenizers 0.22 (HuggingFace) | embeddings, candle-gated |
| Tensors | safetensors 0.4, bytemuck 1.14, half 2.3 | embeddings |
| HDC | bitvec 1.0 | embeddings |
| AST parsing | tree-sitter 0.23 + tree-sitter-rust 0.23 | core |
| Hashing | sha2 0.10, xxhash-rust 0.8 | multi |
| Unix IPC | libc 0.2 (target-gated) | mcp (flock PID guard) |
| CLI parsing | manual (no clap) in mcp; clap 4.5 in cli | - |
| Release profile | `lto = "thin"`, `codegen-units = 1`, `strip = true` | Cargo.toml |

### 3.1 Feature Flags

| Feature | Crate | Default | Effect |
|---|---|---|---|
| `cuda` | mcp, cuda, embeddings, storage, causal-agent, candle | **yes** | Enable CUDA path across the stack; fail-fast if GPU missing |
| `llm` | mcp, causal-agent, graph-agent | **yes** | Enable llama-cpp-2 LLM agents (+~500MB binary). Gates 4 MCP tools |
| `candle` | embeddings | transitively yes | Mandates Candle — `compile_error!` fires otherwise |
| `test-utils` | core, graph-agent | no | Stub/zero constructors for tests only |
| `real-embeddings` | benchmark | no | Use real GPU embeddings in benchmarks |
| `benchmark-tests` | benchmark | no | Benchmark-only test suite |

---

## 4. MCP Tool Categories (75 tools total)

Dispatched by `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`; name constants in `crates/context-graph-mcp/src/tools/names.rs`; JSON schemas in `crates/context-graph-mcp/src/tools/definitions/`. Four tools are gated behind `--features llm` (disabled → `-32001 FEATURE_DISABLED`). Full per-tool reference lives in [08_mcp_tools_reference.md](08_mcp_tools_reference.md).

| # | Category | Tools | Purpose | Definitions file |
|---|---|---:|---|---|
| 1 | Core | 4 | `store_memory`, `search_graph`, `get_memetic_status`, `trigger_consolidation` | `definitions/core.rs` |
| 2 | Curation | 2 | forget/boost (merge lives in its own category) | `definitions/curation.rs` |
| 3 | Merge | 1 | `merge_concepts` (union/intersection/weighted) | `definitions/merge.rs` |
| 4 | Topic | 4 | Portfolio, stability, detect, divergence alerts | `definitions/topic.rs` |
| 5 | File watcher | 4 | List / stats / delete / reconcile watched files | `definitions/file_watcher.rs` |
| 6 | Sequence (E4) | 4 | Conversation context, timeline, chain, state compare | `definitions/sequence.rs` |
| 7 | Causal (E5) | 4 | search_causes/effects/relationships, causal chain | `definitions/causal.rs` |
| 8 | Causal discovery (LLM) | 2 | trigger + status (gated) | `definitions/causal_discovery.rs` |
| 9 | Keyword (E6) | 1 | `search_by_keywords` | `definitions/keyword.rs` |
| 10 | Code (E7) | 1 | `search_code` | `definitions/code.rs` |
| 11 | Graph (E8) | 4 | search_connections, get_graph_path, discover/validate (LLM-gated) | `definitions/graph.rs` |
| 12 | Robustness (E9) | 1 | `search_robust` HDC | `definitions/robustness.rs` |
| 13 | Entity (E11) | 6 | extract, search, infer, related, validate, entity_graph | `definitions/entity.rs` |
| 14 | Embedder-first | 9 | search_by_embedder, clusters, compare_views, list_indexes, fingerprint, weight_profile, cross_anomalies, by_tokens (E12), by_expansion (E13) | `definitions/embedder.rs` |
| 15 | Temporal (E2/E3) | 2 | search_recent, search_periodic | `definitions/temporal.rs` |
| 16 | Graph linking / K-NN | 4 | get_memory_neighbors, get_typed_edges, traverse_graph, get_unified_neighbors | `definitions/graph_link.rs` |
| 17 | Provenance | 3 | audit_trail, merge_history, provenance_chain | `definitions/provenance.rs` |
| 18 | Maintenance | 1 | `repair_causal_relationships` | `definitions/maintenance.rs` |
| 19 | Daemon | 1 | `daemon_status` | `definitions/daemon.rs` |
| 20 | Training export | 4 | export/list/get/count_training_records → `CF_TRAINING_RECORDS` | `definitions/training.rs` |
| 21 | Constellation | 5 | compile/list/get/score/delete_constellation | `definitions/constellation.rs` |
| 22 | Contrastive pairs | 4 | mine/list/get/count_contrastive_pairs | `definitions/contrastive.rs` |
| 23 | Typed-edge training | 4 | export/derive/validate_low_confidence/list | `definitions/typed_edges.rs` |

LLM-gated tools (4): `trigger_causal_discovery`, `get_causal_discovery_status`, `discover_graph_relationships`, `validate_graph_link`, plus `validate_low_confidence_edges` which contains an LLM-only inner branch.

---

## 5. Entry Points

### 5.1 Binaries

| Binary | Crate | Path |
|---|---|---|
| `context-graph-mcp` | context-graph-mcp | `crates/context-graph-mcp/src/main.rs` |
| `context-graph-cli` | context-graph-cli | `crates/context-graph-cli/src/main.rs` |
| `train-causal` | context-graph-embeddings | `crates/context-graph-embeddings/src/bin/train_causal.rs` |
| `benchmark-report` (+ ~40 bench bins) | context-graph-benchmark | `crates/context-graph-benchmark/src/bin/*.rs` (built only via `cargo -p`) |

### 5.2 `context-graph-mcp` CLI flags

| Flag | Purpose |
|---|---|
| `--config <PATH>` | Load TOML config file |
| `--transport {stdio\|tcp}` | Override transport (SSE rejected, fail-fast) |
| `--port <u16>` | TCP port override |
| `--bind <addr>` | TCP bind address |
| `--warm-first` / `--no-warm` | Block on VRAM warmup (default) vs. background-load |
| `--daemon` | Spawn or connect to shared TCP daemon |
| `--daemon-port <u16>` | Override daemon port (default 3100) |
| `--daemon-server-only` | Internal — marks the spawned headless daemon process |
| `--help, -h` | Help |

Priority: CLI > env (`CONTEXT_GRAPH_*`) > config file > defaults.

### 5.3 `context-graph-cli` command tree

| Group | Subcommands |
|---|---|
| `session` | `restore-identity`, `persist-identity` |
| `hooks` | `session-start`, `session-end`, `pre-tool-use`, `post-tool-use`, `user-prompt-submit`, `task-completed`, `pre-compact` |
| `memory` | `capture`, `inject` |
| `topic` | Portfolio + stability queries |
| `divergence` | Divergence detection |
| `setup` | Write `.claude/settings.json` + hook scripts |
| `warmup` | Pre-load all 13 embedding models into VRAM |
| `watch` | Watch directory for markdown/rust changes → auto-index |

---

## 6. Runtime Directory Layout

```
/home/cabdru/contextgraph/
├── Cargo.toml                          # Workspace (10 members, benchmark excluded from default)
├── rust-toolchain.toml                 # stable, x86_64-unknown-linux-gnu
├── CLAUDE.md                           # Project instructions
├── config/
│   ├── default.toml                    # Base config
│   ├── development.toml                # Dev overrides
│   ├── production.toml                 # Production overrides
│   └── test.toml                       # Test overrides
├── crates/                             # 10 Rust crates (see 02_source_code_map.md)
├── scripts/
│   ├── v4_2/                           # Python CAMD-DPO pipeline (see 19_camd_dpo_subsystem.md)
│   ├── *.sh                            # Build/benchmark/daemon shell helpers
│   └── shakespeare_*.py                # Domain-specific training pipeline
├── models/
│   ├── semantic/                       # E1: e5-large-v2 (1024D)
│   ├── temporal/                       # E2/E3/E4 encoder params (512D)
│   ├── causal/                         # E5: nomic-embed fine-tuned (768D)
│   ├── sparse/                         # E6: sparse keyword (~30K vocab)
│   ├── code-1536/                      # E7: Qodo-Embed-1-1.5B (1536D)
│   ├── graph/                          # E8: e5-large-v2 asymmetric (1024D)
│   ├── hdc/                            # E9: HDC character trigram hypervectors (1024D)
│   ├── multimodal/                     # E10: paraphrase (768D)
│   ├── contextual/                     # E10 contextual variant
│   ├── entity/                         # E11 legacy
│   ├── kepler/                         # E11 KEPLER/TransE (768D; disabled by default)
│   ├── late-interaction/               # E12: ColBERT (128D/token)
│   ├── splade-v3/                      # E13: SPLADE-v3 (~30K vocab)
│   ├── hermes-2-pro/                   # Hermes-2-Pro-Mistral-7B GGUF + GBNF grammars
│   ├── gnn/, graph_weights/            # Graph model auxiliary weights
│   ├── hyperbolic/                     # Legacy hyperbolic artifacts (retained)
│   └── models_config.toml              # Model-path mapping
├── contextgraph_data/                  # RocksDB root (created on first run)
│   ├── *.sst                           # Sorted String Tables
│   ├── MANIFEST-*                      # RocksDB manifest
│   ├── CURRENT, LOG, OPTIONS-*
│   └── mcp.pid                         # PID + flock (prevents multi-process DB corruption)
├── data/                               # Benchmark / training datasets
├── docs/, docs2/                       # Documentation (specs live in docs2/contextgraphspecs/)
├── tests/                              # Workspace-level integration tests
├── tmp/                                # SITREPs, scratch output
├── target/, target-test/               # Cargo build output
└── Makefile, README.md, LICENSE
```

RocksDB path is overridable via `CONTEXT_GRAPH_STORAGE_PATH`. Models path via `CONTEXT_GRAPH_MODELS_PATH`.

---

## 7. Storage Tier Classification

### 7.1 Sacred (irreplaceable)

| Artifact | Path | Rationale |
|---|---|---|
| RocksDB SSTs | `contextgraph_data/*.sst` | User memories, embeddings, provenance, audit |
| RocksDB MANIFEST / WAL | `contextgraph_data/MANIFEST-*`, `*.log` | Consistency / crash recovery |
| Embedder weights | `models/*/model.safetensors` | Pre-trained, re-download non-trivial |
| LLM GGUF | `models/hermes-2-pro/*.gguf` | ~6-14 GB download |
| LMDB sidecar (v4.2) | `~/.contextgraph/v4_2/extended_store.lmdb` | Extended-5 embedder vectors persisted for CAMD mining |

### 7.2 Regenerable (rebuild from sacred)

| Artifact | CF / location | Rebuild |
|---|---|---|
| HNSW graphs | `CF_HNSW_GRAPHS` (+ in-memory) | O(n) scan of `CF_FINGERPRINTS` on startup |
| K-NN edges | `embedder_edges` CF | `BackgroundGraphBuilder` cycle |
| Typed edges | `typed_edges`, `typed_edges_by_type` | Agreement over K-NN edges |
| Topic portfolio | `CF_TOPIC_PORTFOLIO` | `detect_topics` |
| E13 SPLADE inverted | `CF_E13_SPLADE_INVERTED` | Rebuild from fingerprints |
| E6 sparse inverted | `CF_E6_SPARSE_INVERTED` | Rebuild from fingerprints |
| E1 Matryoshka 128 | `CF_E1_MATRYOSHKA_128` | Truncate full 1024D |
| Quantized embedder CFs | `emb_0` … `emb_12` | Quantize full-precision vectors |
| Consolidation recs | `CF_CONSOLIDATION_RECOMMENDATIONS` | `trigger_consolidation` |
| Training records | `CF_TRAINING_RECORDS` | `export_training_corpus` |
| Constellations | `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` | `compile_constellation` |
| Contrastive pairs | `CF_CONTRASTIVE_PAIRS` + by-kind + by-anchor | `mine_contrastive_pairs` |
| Typed-edge records / validations | `CF_TYPED_EDGE_RECORDS`, `CF_TYPED_EDGE_VALIDATIONS` | `export_typed_edges_corpus`, `validate_low_confidence_edges` |

### 7.3 Ephemeral (lost on restart, acceptable)

| Artifact | Location | Lifetime |
|---|---|---|
| Session cache | In-memory `SessionCache` | Process |
| Custom weight profiles | `CF_CUSTOM_WEIGHT_PROFILES` | Session-scoped |
| Embedding cache | moka | Process |
| `mcp.pid` + flock | `contextgraph_data/mcp.pid` | Process (stale-lock auto-cleaned) |
| Background task state | Atomics | Process |

---

## 8. Error Hierarchy

Top-level unified enum: `ContextGraphError` (`crates/context-graph-core/src/error/unified.rs`). Every crate error converts into this enum; the MCP layer maps variants onto JSON-RPC error codes.

### 8.1 Top-level variants

| Variant | JSON-RPC code | Recoverable | Critical | Source |
|---|---|---|---|---|
| `Embedding(EmbeddingError)` | -32005 | if `ModelNotLoaded` | no | `error/sub_errors.rs` |
| `Storage(StorageError)` | -32004 | if `Transaction` | if `Corruption` | `error/sub_errors.rs` |
| `Index(IndexError)` | -32008 | if `Timeout` | if `Corruption` | `error/sub_errors.rs` |
| `Config(ConfigError)` | -32603 | no | no | `error/sub_errors.rs` |
| `Gpu(GpuError)` | -32009 | if `OutOfMemory` | if `NotAvailable` | `error/sub_errors.rs` |
| `Mcp(McpError)` | varies | if `RateLimited` | no | `error/sub_errors.rs` |
| `Validation(String)` | -32602 | no | no | `error/unified.rs` |
| `Internal(String)` | -32603 | no | yes | `error/unified.rs` |

### 8.2 JSON-RPC codes (`crates/context-graph-mcp/src/protocol.rs`)

| Code | Name |
|---|---|
| -32700 | PARSE_ERROR |
| -32600 | INVALID_REQUEST |
| -32601 | METHOD_NOT_FOUND |
| -32602 | INVALID_PARAMS |
| -32603 | INTERNAL_ERROR |
| -32001 | FEATURE_DISABLED |
| -32002 | NODE_NOT_FOUND |
| -32004 | STORAGE_ERROR |
| -32005 | EMBEDDING_ERROR |
| -32006 | TOOL_NOT_FOUND |
| -32007 | LAYER_TIMEOUT |
| -32008 | INDEX_ERROR |
| -32009 | GPU_ERROR |
| -32021 | INSUFFICIENT_MEMORIES |
| -32110 | TCP_BIND_FAILED |
| -32111 | TCP_CONNECTION_ERROR |
| -32112 | TCP_MAX_CONNECTIONS_REACHED |

### 8.3 Sub-error catalogue

| Type | Crate | File |
|---|---|---|
| `EmbeddingError`, `StorageError`, `IndexError`, `ConfigError`, `GpuError`, `McpError` | core | `src/error/sub_errors.rs` |
| `CoreError` (legacy) | core | `src/error/legacy.rs` |
| `RetrieverError` | core | `src/retrieval/retriever.rs` |
| `SimilarityError` | core | `src/similarity/error.rs` |
| `ClusterError` | core | `src/clustering/error.rs` |
| `EdgeError` | core | `src/graph_linking/error.rs` |
| `ValidationError` | core | `src/types/memory_node/validation.rs` |
| `WeightProfileError` | core | `src/weights/mod.rs` |
| `InjectionError` | core | `src/injection/pipeline.rs` |
| `WatcherError` | core | `src/memory/watcher.rs` |
| `SessionError` | core | `src/memory/manager.rs` |
| `CudaError`, `GpuHdbscanError` | cuda | `src/error.rs`, `src/hdbscan/error.rs` |
| `EmbeddingError`, `ModelLoadError`, `UnifiedLoaderError`, `MemoryError`, `PQ8QuantizationError`, `Float8QuantizationError`, `BinaryQuantizationError` | embeddings | `src/error/types.rs`, `src/gpu/model_loader/error.rs`, etc. |
| `StorageError` | storage | `src/error.rs` |
| `GraphAgentError` | graph-agent | `src/error.rs` |
| `CausalAgentError` | causal-agent | `src/error.rs` |

---

## 9. Subsystem Summaries

### 9.1 Embeddings — `context-graph-embeddings`
13 embedders (E1-E13, E11 KEPLER disabled by default) implemented on Candle. `ProductionMultiArrayProvider` orchestrates forward passes, the warm loader pins all weights into VRAM up-front (~20-30 s on RTX 5090), and a moka cache dedupes repeated inputs. Quantization modules handle PQ-8, Float8 (E4M3) and binary compression for storage. Pre-trained models (E1, E5, E7, E8, E10, E11, E12, E13 + R-GCN + weight-projection MLP) live under `models/pretrained/`; custom encoders (E2 recent, E3 periodic, E4 positional, E9 HDC) under `models/custom/`. Deep dive: [05_embedding_subsystem.md](05_embedding_subsystem.md).

### 9.2 Storage — `context-graph-storage`
RocksDB engine with 59 column families (`TOTAL_COLUMN_FAMILIES` in `crates/context-graph-storage/src/column_families.rs:397`): 11 base (8 original + 3 graph-linking) + 28 teleological (`TELEOLOGICAL_CF_COUNT` in `src/teleological/column_families.rs:551`) + 13 quantized embedder (`emb_0` … `emb_12`) + 5 code + 2 causal. `RocksDbTeleologicalStore` implements `TeleologicalMemoryStore`; `usearch`-based HNSW indexes live in `CF_HNSW_GRAPHS`; E6 and E13 use custom inverted indexes; E12 uses MaxSim reranking. Deep dive: [09_storage_layer.md](09_storage_layer.md) and [04_database_schema.md](04_database_schema.md).

### 9.3 Search / Fusion — `context-graph-core::retrieval` + `context-graph-storage::search`
Four strategies: `e1_only`, `multi_space` (default), `pipeline` (E13→E1→E12), `embedder_first`. Weighted Reciprocal Rank Fusion across all active embedders, per-query weight profiles (14 built-in), optional synergy matrix (13×13), optional quantized prefilter, optional teleological prefilter. Asymmetric cosine for E5 (cause→effect 1.2×, effect→cause 0.8×) and E8 (source→target 1.2×, target→source 0.8×). Cosine normalized to `[0,1]` by SRC-3 convention `(raw+1)/2`. Deep dive: [06_search_and_fusion.md](06_search_and_fusion.md).

### 9.4 Graph — `context-graph-core::graph_linking` + `context-graph-storage::graph_edges`
Per-embedder K-NN graphs built by NN-Descent (`BackgroundGraphBuilder` cycles ~60 s), agreement over K-NN neighborhoods produces 8 typed edge kinds (semantic_similar, code_related, causal_chain, etc.), and `get_unified_neighbors` does weighted RRF across the 13 per-embedder neighborhoods. Edges stored as `[source:16][target:16]` (K-NN) and `[type:1][source:16][target:16]` (typed-by-type). Deep dive: [07_graph_and_causal.md](07_graph_and_causal.md).

### 9.5 Causal — `context-graph-core::causal` + `context-graph-causal-agent`
`CausalDiscoveryLLM` (Hermes-2-Pro-Mistral-7B via llama-cpp-2 + GBNF) extracts cause/effect pairs; `E5EmbedderActivator` triggers dual E5 embedding for each confirmed relationship; `CausalHnswIndex` indexes cause/effect vectors separately so abduction (`search_causes`) and forward (`search_effects`) are direction-aware. Causal gate (`CAUSAL_THRESHOLD=0.04`, `NON_CAUSAL_THRESHOLD=0.008`) boosts/demotes retrieval scores. Deep dive: [07_graph_and_causal.md](07_graph_and_causal.md).

### 9.6 Clustering / Topics — `context-graph-core::clustering`
HDBSCAN on multi-space fingerprints (optional GPU via `context-graph-cuda::hdbscan`), BIRCH for incremental updates, `TopicSynthesizer` merges per-space clusters into cross-space topics, `ClusteringManager` enforces caps and tracks stability (churn, drift, phase transitions). Detection threshold 2.5 agreement. Deep dive: [12_clustering_and_topics.md](12_clustering_and_topics.md).

### 9.7 Teleological — `context-graph-core::teleological`
13×13 `SynergyMatrix`, 5-stage retrieval pipeline, `TeleologicalProfile` weight profiles per task type, `TeleologicalFingerprint` (~63 KB full) with 13D `TopicProfile` + 78 cross-correlations + 6D group alignments. `MultiSpaceRetriever`, `FusionEngine`, and `MeaningPipeline` compose into the production retrieval stack. Deep dive: [13_teleological_subsystem.md](13_teleological_subsystem.md).

### 9.8 Training (export) — `context-graph-core::training` + `handlers/tools/training_tools.rs`
`TrainingRecord` v1 bincode struct assembling 13 embeddings + 78 cross-correlations + 6D group alignments + outgoing/incoming typed edges + K-NN neighbors + LLM causal labels + Phase 5 temporal labels (`PeriodicBucket`, hour/day/month, age, session position). Persisted row-at-a-time to `CF_TRAINING_RECORDS` with `TRAINING_RECORD_VERSION=1` prefix byte. Deep dive: [16_training_subsystem.md](16_training_subsystem.md).

### 9.9 Constellation — `context-graph-core::constellation`
Compiled summary of a caller-selected memory set: `WelfordVector` centroids (dense), `ReservoirSample` percentiles (Vitter R, `RESERVOIR_SAMPLE_SIZE=1024`), E12 pooled 128D, E6/E13 sparse top-K terms, aggregate 13D topic profile + 78 correlations + 6D group alignments. Persisted to `CF_CONSTELLATIONS` keyed by fresh UUID, with `CF_CONSTELLATION_BY_SELECTOR` ([`selector_kind:u8`][sha256-16] → UUID) for idempotent existence check. `MIN_CONSTELLATION_MEMBERS=3`. Deep dive: [17_constellation_subsystem.md](17_constellation_subsystem.md).

### 9.10 Contrastive / Typed-Edges — `context-graph-core::contrastive` + `context-graph-core::typed_edge_export` + `context-graph-core::llm_edge_validation`
Six `AnomalyKind` classifications over 13-slot similarity profiles (SRC-3 `[0,1]`), persisted as `(anchor, negative, profile)` triples across three CFs. Typed-edge export copies K-NN typed edges into `CF_TYPED_EDGE_RECORDS` keyed `[source:16][target:16][edge_type:1]` (shared format in `typed_edge_keys.rs`), LLM validations into `CF_TYPED_EDGE_VALIDATIONS` with Valid/Invalid/Reclassify verdicts + prompt hash + validator version. Anomaly derivation scans typed-edge embedder scores directly to produce 5 of 6 anomaly kinds without running the miner. Deep dive: [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

### 9.11 CAMD-DPO (v4.2) — `scripts/v4_2/`
Python subsystem that turns `CF_CONTRASTIVE_PAIRS` and the Sonnets constellation into DPO training pairs via the **Constellation-Anchored Multi-View DPO** reward. Core modules: `camd_lib.py` (ConstellationScorer, CAMDPairFilter), `mine_camd_pairs.py` / `mine_camd_pairs_extended.py` (CLI miners), `extended_ensemble.py` (4 dense + 1 reranker BGE/E5-Mistral/Jina/Qwen3-Reranker), `extended_store.py` (LMDB sidecar), `extended_constellation.py` + `extended_camd.py` (extended-5 reward), `backfill_extended.py`, `analyze_correlations.py`, `smoke_dpo_v4_2.py`, `train_camd_dpo.py` (TRL-based DPO trainer), `validate_camd.py` (3-part sanity). Daemon on `127.0.0.1:3100` is a hard prerequisite. Deep dive: [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).

### 9.12 Other subsystems (pointers only)
- **Configuration** → [03_configuration.md](03_configuration.md)
- **Database schema (all 59 CFs with key layouts)** → [04_database_schema.md](04_database_schema.md)
- **CLI reference** → [10_cli_reference.md](10_cli_reference.md)
- **CUDA acceleration** → [11_cuda_acceleration.md](11_cuda_acceleration.md)
- **Test suite** → [14_test_suite.md](14_test_suite.md)
- **Verification** → [15_verification_report.md](15_verification_report.md)

---

## 10. Key Constants Quick Reference

| Constant | Value | Location |
|---|---|---|
| `TOTAL_COLUMN_FAMILIES` | 59 | `context-graph-storage/src/column_families.rs:397` |
| `TELEOLOGICAL_CF_COUNT` | 28 | `context-graph-storage/src/teleological/column_families.rs:551` |
| Base CFs | 11 | `context-graph-storage/src/column_families.rs` (`cf_names::ALL`) |
| Quantized embedder CFs | 13 | `emb_0` … `emb_12` |
| Code CFs | 5 | `context-graph-storage/src/code/` |
| Causal CFs | 2 | `CF_CAUSAL_RELATIONSHIPS`, `CF_CAUSAL_BY_SOURCE` |
| MCP tool constants | 75 | `context-graph-mcp/src/tools/names.rs` |
| Active embedders | 13 (E1-E13) | `context-graph-embeddings` models/pretrained + models/custom |
| Default active set | 11 dense | E2/E3/E4 weight-gated; E11 disabled |
| `TRAINING_RECORD_VERSION` | 1 | `context-graph-core/src/training/mod.rs` |
| `CONSTELLATION_VERSION` | 1 | `context-graph-core/src/constellation/mod.rs` |
| `CONTRASTIVE_PAIR_VERSION` | 1 | `context-graph-core/src/contrastive/mod.rs` |
| `TYPED_EDGE_RECORD_VERSION` | 1 | `context-graph-core/src/typed_edge_export/mod.rs` |
| `LLM_EDGE_VALIDATION_VERSION` | 1 | `context-graph-core/src/llm_edge_validation/mod.rs` |
| `MIN_CONSTELLATION_MEMBERS` | 3 | `context-graph-core/src/constellation/mod.rs` |
| `RESERVOIR_SAMPLE_SIZE` | 1024 | `context-graph-core/src/constellation/mod.rs` |
| Causal gate | 0.04 / 0.008 / 1.10× / 0.85× | `context-graph-core/src/causal/` |
| Default daemon port | 3100 | `context-graph-mcp/src/main.rs` |
| Default TCP port | 8080 | `config/default.toml` |

---

*Cross-references:* [02_source_code_map.md](02_source_code_map.md) · [03_configuration.md](03_configuration.md) · [04_database_schema.md](04_database_schema.md) · [05_embedding_subsystem.md](05_embedding_subsystem.md) · [06_search_and_fusion.md](06_search_and_fusion.md) · [07_graph_and_causal.md](07_graph_and_causal.md) · [08_mcp_tools_reference.md](08_mcp_tools_reference.md) · [09_storage_layer.md](09_storage_layer.md) · [10_cli_reference.md](10_cli_reference.md) · [11_cuda_acceleration.md](11_cuda_acceleration.md) · [12_clustering_and_topics.md](12_clustering_and_topics.md) · [13_teleological_subsystem.md](13_teleological_subsystem.md) · [14_test_suite.md](14_test_suite.md) · [15_verification_report.md](15_verification_report.md) · [16_training_subsystem.md](16_training_subsystem.md) · [17_constellation_subsystem.md](17_constellation_subsystem.md) · [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) · [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md)
