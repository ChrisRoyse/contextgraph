# System Overview

**Source files covered:**
- `Cargo.toml`
- `crates/*/Cargo.toml`
- `crates/context-graph-mcp/src/main.rs`
- `crates/context-graph-mcp/src/tools/definitions/mod.rs`
- `crates/context-graph-storage/src/column_families.rs`
- `crates/context-graph-storage/src/teleological/column_families.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`
- `scripts/v4_2/*.py`

## 1. Summary
Context Graph is a Rust workspace that stores memories as 14-embedder fingerprints in RocksDB and exposes MCP JSON-RPC tools plus a local CLI. Source defines 14 production content embedders (E1-E14), 7 learner-state slots (E15-E21), 73 RocksDB column families, and **108 MCP tools with `llm` (104 without)**. Python scripts under `scripts/v4_2` mine CAMD-DPO pairs, run FSVs, and execute Shakespeare empirical cycles.

## 2. Architecture
| Process | Port/address | Technology | Purpose |
| --- | --- | --- | --- |
| context-graph-mcp | stdio or TCP 3100 (sse rejected by main) | tokio JSON-RPC | tools/list, tools/call |
| context-graph-cli | local process | clap | Operator commands, hooks, capture/inject |
| RocksDB | `storage.path` or `CONTEXT_GRAPH_STORAGE_PATH` | 73 CFs | Sacred durable storage |
| Embedding warm provider | in-process GPU | Candle/CUDA | Loads E1-E14 (lazy) |
| CAMD/Shakespeare scripts | local Python | Python RPC + optional LMDB | DPO mining, empirical cycle |

## 3. Technology Stack
| Layer | Technology | Source |
| --- | --- | --- |
| Rust | edition 2021, rust-version 1.75 | Cargo.toml |
| Async/MCP | tokio 1.35 full, JSON-RPC 2.0 | Cargo.toml |
| Storage | rocksdb 0.22, usearch 2 | Cargo.toml |
| Embeddings | candle 0.9.2-alpha (CUDA 13/Blackwell), llama-cpp-2 (llm feature) | Cargo.toml |
| Search | hnsw_rs 0.3, RRF k=60 | retrieval/aggregation.rs |
| Parsing | tree-sitter 0.23, tree-sitter-rust 0.23 | Cargo.toml |
| Serialization | serde, serde_json, bincode, toml | Cargo.toml |

## 4. Workspace Crates
| Package | Default features | Notes |
| --- | --- | --- |
| context-graph-mcp | cuda, llm | MCP server + 108 tools |
| context-graph-core | — | Fingerprints, weights, retrieval, learning, teleological |
| context-graph-embeddings | candle | 14 embedder providers + train-causal bin |
| context-graph-cuda | cuda | FFI, similarity, hdbscan, allocation |
| context-graph-storage | cuda | 73 CFs, search, fusion, repository |
| context-graph-causal-agent | — | Causal discovery (LLM-gated) |
| context-graph-graph-agent | — | Graph link discovery (LLM-gated) |
| context-graph-cli | cuda | Operator CLI, hooks, watcher |
| context-graph-test-utils | — | Shared fixtures |
| context-graph-benchmark | — | Excluded from default-members; build with `-p context-graph-benchmark` |

## 5. MCP Tool Domains (108 total with llm, 104 without)
| Category | Count | Source module |
| --- | --- | --- |
| capability | 1 | tools/definitions/capability.rs |
| core | 5 | core.rs (store_memory, store_memories, get_memetic_status, search_graph, trigger_consolidation) |
| merge | 1 | merge.rs |
| curation | 2 | curation.rs |
| topic | 4 | topic.rs |
| file_watcher | 4 | file_watcher.rs |
| sequence | 4 | sequence.rs |
| causal | 4 | causal.rs |
| causal_discovery | 0 (default) / 2 (llm) | causal_discovery.rs |
| keyword | 1 | keyword.rs |
| code | 1 | code.rs |
| graph | 2 (default) / 4 (llm) | graph.rs |
| robustness | 1 | robustness.rs |
| entity | 6 | entity.rs |
| embedder | 7 + 2 standalone | embedder.rs |
| temporal | 2 | temporal.rs |
| graph_link | 4 | graph_link.rs |
| maintenance | 1 | maintenance.rs |
| provenance | 3 | provenance.rs |
| daemon | 1 | daemon.rs |
| training | 4 | training.rs |
| learning | 12 | learning.rs |
| utl | 18 | utl.rs |
| constellation | 6 | constellation.rs |
| contrastive | 4 | contrastive.rs |
| typed_edges | 4 | typed_edges.rs |

See [08_mcp_tools_reference.md](08_mcp_tools_reference.md) for full tool inventory.

## 6. Entry Points
| Entry | Kind | Source |
| --- | --- | --- |
| context-graph-mcp | bin | crates/context-graph-mcp/src/main.rs |
| context-graph-cli | bin | crates/context-graph-cli/src/main.rs |
| train-causal | bin | crates/context-graph-embeddings/src/bin/train_causal.rs |
| benchmark binaries | bin × ~40 | crates/context-graph-benchmark/src/bin/*.rs |
| examples × ~12 | example | causal-agent, core, graph-agent, storage examples |

MCP server CLI flags: `--config`, `--transport`, `--port`, `--bind`, `--warm-first`, `--no-warm`, `--daemon`, `--daemon-port`, hidden `--daemon-server-only`. See [10_cli_reference.md](10_cli_reference.md).

## 7. Storage Tier Classification
| Path | Class | Behavior |
| --- | --- | --- |
| RocksDB at `storage.path` | sacred durable | All 73 CFs; `mcp.pid` flock guards single-writer |
| `models/` | regenerable runtime asset | Embedder weights downloaded by providers |
| `target/` | regenerable | Cargo artifacts |
| `<db_path>/mcp.pid` | ephemeral | Unix flock |
| `~/.contextgraph/extended_embeddings.lmdb` | regenerable sidecar | Extended CAMD cache |

## 8. Key Subsystems (One-Line Each)
- **Embedding** ([05](05_embedding_subsystem.md)) — 14 production embedders + 7 learner slots; SemanticFingerprint stores all without fusion.
- **Search & Fusion** ([06](06_search_and_fusion.md)) — RRF/Weighted/MaxPooling fusion across 14 dense+sparse+token spaces; 17 weight profiles.
- **Graph & Causal** ([07](07_graph_and_causal.md)) — K-NN embedder edges, typed edges, causal relationships; LLM-gated discovery.
- **Storage** ([09](09_storage_layer.md)) — RocksDB with 73 CFs; bloom filters, prefix extractors, LZ4.
- **Teleological** ([13](13_teleological_subsystem.md)) — TeleologicalFingerprint wraps SemanticFingerprint with metadata, importance, access tracking.
- **Training** ([16](16_training_subsystem.md)) — TrainingRecord v3 exports memory + 14 embeddings + 91 cross-correlations.
- **Constellation** ([17](17_constellation_subsystem.md)) — Aggregate centroids over selected memory sets.
- **Contrastive/Typed-Edges** ([18](18_contrastive_and_typed_edges.md)) — Disagreement mining + LLM edge validation.
- **CAMD/DPO** ([19](19_camd_dpo_subsystem.md)) — Python-side DPO pair mining, retrieval policy benchmarks.
- **Learning/UTL** ([20](20_learning_subsystem.md)) — Learner profiles, deltas (S/C/E), L = ΔS·ΔC·ΔE, M(t) traces.
- **Capability Matrix** — `get_capability_matrix` tool returns embedders, CFs, tools, runtime state in compact form.

## 9. Source-Level Inconsistencies (Documented As-Is)
| Item | Source |
| --- | --- |
| `TELEOLOGICAL_VECTOR_DIM = 13` while `NUM_EMBEDDERS = 14` (E14 excluded from compact vector) | teleological/types.rs; semantic/constants.rs |
| Config validates `sse` transport but MCP main rejects it | config/sub_configs.rs; mcp/main.rs |
| `LEARNER_MODALITY_COUNT = 7` while `LearnerModality` has more variants; validation permits `+1` | learner/mod.rs |
| Some comments mention older 13/100/96 counts; constants in this doc are current | various files |
