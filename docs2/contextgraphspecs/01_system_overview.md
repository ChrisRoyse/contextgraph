# System Overview

Source files covered:
- `Cargo.toml` (workspace manifest)
- `rust-toolchain.toml`
- `config/default.toml`
- `crates/context-graph-mcp/Cargo.toml`, `src/main.rs`, `src/lib.rs`, `src/protocol.rs`, `src/server/mod.rs`, `src/server/transport.rs`, `src/server/watchers.rs`
- `crates/context-graph-mcp/src/tools/names.rs`, `src/tools/definitions/mod.rs` and 19 per-category definition submodules
- `crates/context-graph-mcp/src/handlers/mod.rs`, `src/handlers/tools/mod.rs`, `src/handlers/tools/dispatch.rs`
- `crates/context-graph-core/Cargo.toml`, `src/lib.rs`, `src/error/unified.rs`, `src/error/sub_errors.rs`
- `crates/context-graph-embeddings/Cargo.toml`, `src/lib.rs`, `src/bin/train_causal.rs`
- `crates/context-graph-storage/Cargo.toml`, `src/lib.rs`, `src/column_families.rs`, `src/teleological/mod.rs`, `src/teleological/column_families.rs`
- `crates/context-graph-cli/Cargo.toml`, `src/main.rs`
- `crates/context-graph-cuda/Cargo.toml`
- `crates/context-graph-causal-agent/Cargo.toml`
- `crates/context-graph-graph-agent/Cargo.toml`
- `crates/context-graph-benchmark/Cargo.toml`

---

## 1. Summary

Context Graph is a Rust-based semantic memory retrieval system that encodes every stored memory through 13 independent embedding models (E1-E13), persists all vectors in a RocksDB database with 59 column families (TOTAL_COLUMN_FAMILIES constant; `crates/context-graph-storage/src/column_families.rs:397`), and exposes up to 75 MCP (Model Context Protocol) tools over a JSON-RPC 2.0 server (stdio or TCP transport). Clients -- primarily Claude Code and other AI agents -- store memories, search across all 13 embedding spaces simultaneously via Weighted Reciprocal Rank Fusion (RRF), navigate a K-NN knowledge graph with typed edges, discover causal relationships using a local LLM (Hermes-2-Pro-Mistral-7B via llama-cpp-2), compile constellations (per-embedder centroids + spread stats for a selected memory set), mine contrastive pairs (cross-embedder anomaly triples for downstream metric-learning), export fully-labeled training records (13 embeddings + 78 cross-correlations + 6D group alignments + typed edges + causal labels + temporal labels) to a dedicated column family, export typed-edge training corpora with per-embedder similarity profiles and optional LLM validation verdicts, derive anomaly pairs directly from typed-edge embedder scores, and manage topic portfolios, session timelines, and provenance audit trails. The system is GPU-first, targeting NVIDIA RTX 5090 (Blackwell) with CUDA 13.x through the HuggingFace Candle framework for embedding inference and Candle/llama-cpp-2 for LLM inference.

---

## 2. Architecture Overview

### 2.1 Process / Port / Transport Table

| Process | Binary | Default Transport | Default Port | Purpose |
|---------|--------|-------------------|--------------|---------|
| MCP Server | `context-graph-mcp` | stdio | N/A (stdin/stdout) | Primary JSON-RPC 2.0 server for all MCP tool calls |
| MCP Server (TCP) | `context-graph-mcp --transport tcp` | TCP | 8080 | Network-based MCP for remote deployments |
| MCP Daemon | `context-graph-mcp --daemon` | TCP | 3100 | Shared server across multiple Claude Code terminals; models loaded once into VRAM |
| CLI | `context-graph-cli` | N/A (command-line) | N/A | Session management, hooks setup, memory capture, topic/divergence commands, file watching, warmup |
| Causal Trainer | `train-causal` | N/A | N/A | Offline E5 causal embedder fine-tuning (3-stage progressive pipeline) |

### 2.2 Crate Dependency Diagram (Simplified)

```
context-graph-mcp (server + handlers)
  |-- context-graph-core (domain types, traits, business logic)
  |-- context-graph-embeddings (GPU model inference)
  |     |-- context-graph-core
  |     |-- context-graph-cuda
  |-- context-graph-storage (RocksDB persistence + HNSW indexes)
  |     |-- context-graph-core
  |     |-- context-graph-embeddings
  |     |-- context-graph-cuda
  |-- context-graph-causal-agent (LLM causal discovery)
  |     |-- context-graph-core
  |     |-- context-graph-storage
  |     |-- context-graph-embeddings
  |-- context-graph-graph-agent (LLM graph discovery)
  |     |-- context-graph-causal-agent (shares CausalDiscoveryLLM)
  |     |-- context-graph-core
  |     |-- context-graph-storage
  |     |-- context-graph-embeddings
  |-- context-graph-cuda (GPU acceleration abstractions)

context-graph-cli
  |-- context-graph-mcp
  |-- context-graph-core
  |-- context-graph-storage
  |-- context-graph-embeddings

context-graph-benchmark (excluded from default build)
  |-- context-graph-core
  |-- context-graph-storage
```

---

## 3. Technology Stack

| Layer | Technology | Version Constraint | Source |
|-------|------------|-------------------|--------|
| **Language** | Rust | Edition 2021, MSRV 1.75 | `Cargo.toml` workspace.package |
| **Toolchain** | Stable Rust | stable channel | `rust-toolchain.toml` |
| **Target** | x86_64-unknown-linux-gnu | Required | `rust-toolchain.toml` |
| **Async Runtime** | Tokio | 1.35, features=["full"] | `Cargo.toml` workspace.dependencies |
| **GPU/ML Framework** | Candle (HuggingFace) | 0.9.2-alpha, features=["cuda"] | `Cargo.toml` workspace.dependencies |
| **LLM Inference** | llama-cpp-2 (llama.cpp bindings) | 0.1, optional, features=["cuda"] | `context-graph-causal-agent/Cargo.toml` |
| **Storage Engine** | RocksDB | 0.22 | `context-graph-core/Cargo.toml` |
| **HNSW (core)** | hnsw_rs | 0.3 | `context-graph-core/Cargo.toml` |
| **HNSW (storage)** | usearch | 2 | `context-graph-storage/Cargo.toml` |
| **Serialization** | serde 1.0, serde_json 1.0, bincode 1.3, toml 0.8 | Workspace | `Cargo.toml` |
| **Error Handling** | thiserror 1.0, anyhow 1.0 | Workspace | `Cargo.toml` |
| **Logging** | tracing 0.1, tracing-subscriber 0.3 | Workspace | `Cargo.toml` |
| **CLI Framework** | clap 4.5, features=["derive","env"] | Direct | `context-graph-cli/Cargo.toml` |
| **Concurrency** | rayon 1.10, parking_lot 0.12, dashmap 5.5 | Direct | Various Cargo.toml |
| **Concurrent Cache** | moka 0.12, features=["sync"] | Direct | `context-graph-embeddings/Cargo.toml` |
| **Tokenization** | tokenizers 0.22 (HuggingFace) | Optional (candle feature) | `context-graph-embeddings/Cargo.toml` |
| **SafeTensors** | safetensors 0.4 | Direct | `context-graph-embeddings/Cargo.toml` |
| **AST Parsing** | tree-sitter 0.23, tree-sitter-rust 0.23 | Direct | `context-graph-core/Cargo.toml` |
| **FP16** | half 2.3 | Direct | `context-graph-core/Cargo.toml` |
| **Bit Vectors (HDC)** | bitvec 1.0 | Direct | `context-graph-embeddings/Cargo.toml` |
| **Zero-copy** | bytemuck 1.14 | Direct | `context-graph-embeddings/Cargo.toml` |
| **Hashing** | sha2 0.10, xxhash-rust 0.8 | Direct | Various |
| **IDs** | uuid 1.6, features=["v4","v5","serde"] | Workspace | `Cargo.toml` |
| **Time** | chrono 0.4, features=["serde"] | Workspace | `Cargo.toml` |
| **Regex** | regex 1 | Direct | `context-graph-causal-agent/Cargo.toml` |
| **Unix IPC** | libc 0.2 | Unix-only | `context-graph-mcp/Cargo.toml` |
| **Build Profile** | Release: thin LTO, codegen-units=1, strip=true | N/A | `Cargo.toml` [profile.release] |

### 3.1 Feature Flags

| Feature | Crate | Default | Effect |
|---------|-------|---------|--------|
| `cuda` | context-graph-mcp | Yes | Enables CUDA across all crates; required for production |
| `llm` | context-graph-mcp | Yes | Enables CausalDiscoveryLLM + GraphDiscoveryService (~500MB); without this 4 tools are unavailable |
| `candle` | context-graph-embeddings | Yes (via cuda) | Mandates Candle framework; compile_error! if missing |
| `test-utils` | context-graph-core | No | Enables stub/zeroed constructors for downstream tests; never for production |

---

## 4. MCP Tools (67 without LLM, 71 with LLM)

All tools are exposed via JSON-RPC 2.0 `tools/list` and invoked via `tools/call`. Tool name constants live in `crates/context-graph-mcp/src/tools/names.rs` (75 constants); dispatch arms live in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`.

### 4.1 Core Tools (4)

| Tool | Description | Source |
|------|-------------|--------|
| `store_memory` | Store a memory node in the knowledge graph (inject_context merged into this tool) | `definitions/core.rs` |
| `get_memetic_status` | Get system status: fingerprint count, embedder count (13), storage backend/size, layer status | `definitions/core.rs` |
| `search_graph` | Multi-space semantic search across 13 embedders with configurable strategies (e1_only, multi_space [default], pipeline [E13→E1→E12], embedder_first), weight profiles (14 built-in), temporal/causal/sequence parameters, synergy_weights, use_quantized_prefilter, enable_teleological_prefilter | `definitions/core.rs` |
| `trigger_consolidation` | Analyze memories for merge candidates using similarity/temporal/semantic strategies (does NOT auto-merge) | `definitions/core.rs` |

### 4.2 Curation Tools (3)

| Tool | Description | Source |
|------|-------------|--------|
| `merge_concepts` | Merge 2-10 concept nodes with union/intersection/weighted_average strategy; 30-day undo via reversal_hash | `definitions/merge.rs` |
| `forget_concept` | Soft-delete a memory with 30-day recovery (SEC-06); optional hard delete | `definitions/curation.rs` |
| `boost_importance` | Adjust memory importance by delta [-1.0, 1.0]; clamped to [0.0, 1.0] | `definitions/curation.rs` |

### 4.3 Topic Tools (4)

| Tool | Description | Source |
|------|-------------|--------|
| `get_topic_portfolio` | List all discovered topics with profiles, stability metrics, tier info (threshold >= 2.5) | `definitions/topic.rs` |
| `get_topic_stability` | Portfolio-level stability: churn rate, entropy, phase breakdown | `definitions/topic.rs` |
| `detect_topics` | Force HDBSCAN topic detection recalculation (min 3 memories) | `definitions/topic.rs` |
| `get_divergence_alerts` | Check for divergence from recent activity using semantic embedders only | `definitions/topic.rs` |

### 4.4 File Watcher Tools (4)

| Tool | Description | Source |
|------|-------------|--------|
| `list_watched_files` | List files with embeddings; optional glob filter | `definitions/file_watcher.rs` |
| `get_file_watcher_stats` | Statistics: total files, chunks, averages | `definitions/file_watcher.rs` |
| `delete_file_content` | Delete all embeddings for a file path (soft delete with 30-day recovery) | `definitions/file_watcher.rs` |
| `reconcile_files` | Find orphaned file embeddings (file deleted from disk); optional cleanup | `definitions/file_watcher.rs` |

### 4.5 Sequence / Session Tools (4) -- E4 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `get_conversation_context` | Get memories around current conversation turn using E4 V_ordering | `definitions/sequence.rs` |
| `get_session_timeline` | Ordered timeline of session memories with sequence numbers and position labels | `definitions/sequence.rs` |
| `traverse_memory_chain` | Multi-hop navigation from an anchor point through conversation flow | `definitions/sequence.rs` |
| `compare_session_states` | Compare memory state at different sequence points; topics, counts, differences | `definitions/sequence.rs` |

### 4.6 Causal Tools (4) -- E5 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `search_causal_relationships` | Search LLM-generated causal descriptions with E5 asymmetric similarity and provenance | `definitions/causal.rs` |
| `search_causes` | Abductive reasoning: find causes of effects (0.8x effect-to-cause dampening per AP-77) | `definitions/causal.rs` |
| `search_effects` | Forward causal reasoning: find effects of causes (1.2x cause-to-effect boost per AP-77) | `definitions/causal.rs` |
| `get_causal_chain` | Build transitive causal chains from anchor with hop attenuation (0.9^hop) | `definitions/causal.rs` |

### 4.7 Causal Discovery Tools (2) -- LLM-only

| Tool | Description | Source |
|------|-------------|--------|
| `trigger_causal_discovery` | Trigger LLM (Hermes-2-Pro-Mistral-7B via llama-cpp-2 with GBNF grammar; tool description text still references "Qwen2.5" as legacy name) to analyze memories for cause-effect relationships; embeds via E5 | `definitions/causal_discovery.rs` |
| `get_causal_discovery_status` | Status/statistics of causal discovery agent: running state, VRAM, cumulative stats | `definitions/causal_discovery.rs` |

### 4.8 Keyword Tools (1) -- E6 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `search_by_keywords` | Sparse keyword search via E6; optional E13 SPLADE term expansion; blended with E1 semantic | `definitions/keyword.rs` |

### 4.9 Code Tools (1) -- E7 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `search_code` | Code pattern search via E7 1536D dense embeddings; language detection; hybrid/e7Only/rerank modes | `definitions/code.rs` |

### 4.10 Graph Tools (2 base + 2 LLM-only) -- E8 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `search_connections` | Find connected memories using asymmetric E8 (source/target 1.2x/0.8x) | `definitions/graph.rs` |
| `get_graph_path` | Multi-hop graph path with hop attenuation (0.9^hop) | `definitions/graph.rs` |
| `discover_graph_relationships` | LLM-based relationship discovery across 20 types in 4 domains (LLM feature required) | `definitions/graph.rs` |
| `validate_graph_link` | LLM validation of proposed graph link between two memories (LLM feature required) | `definitions/graph.rs` |

### 4.11 Robustness Tools (1) -- E9 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `search_robust` | Typo-tolerant search via E9 HDC character trigram hypervectors; blind-spot detection vs E1 | `definitions/robustness.rs` |

### 4.12 Entity Tools (6) -- E11 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `extract_entities` | Extract and canonicalize entities from text (pattern matching + knowledge base) | `definitions/entity.rs` |
| `search_by_entities` | Find memories by entities with Jaccard + E11 hybrid scoring | `definitions/entity.rs` |
| `infer_relationship` | Infer relationship between two entities using TransE (r = t - h) | `definitions/entity.rs` |
| `find_related_entities` | Find entities with given relationship via TransE prediction | `definitions/entity.rs` |
| `validate_knowledge` | Score a (subject, predicate, object) triple via TransE distance | `definitions/entity.rs` |
| `get_entity_graph` | Visualize entity relationship graph with TransE edge weights | `definitions/entity.rs` |

### 4.13 Embedder-First Search Tools (7) + Standalone (2)

| Tool | Description | Source |
|------|-------------|--------|
| `search_by_embedder` | Search using any HNSW-indexed embedder (E1-E5, E7-E11) as primary | `definitions/embedder.rs` |
| `get_embedder_clusters` | Explore clusters in a specific embedder's HNSW space | `definitions/embedder.rs` |
| `compare_embedder_views` | Side-by-side comparison of rankings from 2-5 embedders on same query | `definitions/embedder.rs` |
| `list_embedder_indexes` | List all 13 embedder indexes with dimension, type, count, GPU residency | `definitions/embedder.rs` |
| `get_memory_fingerprint` | Per-embedder vector details for a specific memory (norms, presence, dual vectors) | `definitions/embedder.rs` |
| `create_weight_profile` | Create session-scoped custom weight profile for reuse in search_graph | `definitions/embedder.rs` |
| `search_cross_embedder_anomalies` | Find memories scoring high in one embedder but low in another (blind spots) | `definitions/embedder.rs` |
| `search_by_tokens` | E12 ColBERT token-level MaxSim scoring for precise phrase matching | `definitions/embedder.rs` |
| `search_by_expansion` | E13 SPLADE learned term expansion for enhanced keyword recall | `definitions/embedder.rs` |

### 4.14 Temporal Tools (2) -- E2/E3 Integration

| Tool | Description | Source |
|------|-------------|--------|
| `search_recent` | Temporal search with E2 freshness boost (exponential/linear/step decay) | `definitions/temporal.rs` |
| `search_periodic` | Periodic pattern search with E3 time-of-day/day-of-week matching | `definitions/temporal.rs` |

### 4.15 Graph Linking / K-NN Tools (4)

| Tool | Description | Source |
|------|-------------|--------|
| `get_memory_neighbors` | K nearest neighbors in a single embedder space via pre-computed K-NN edges | `definitions/graph_link.rs` |
| `get_typed_edges` | Typed edges (semantic_similar, code_related, causal_chain, etc.) from a memory | `definitions/graph_link.rs` |
| `traverse_graph` | Multi-hop traversal following typed edges up to depth 5 | `definitions/graph_link.rs` |
| `get_unified_neighbors` | Weighted RRF fusion across all 13 embedders for consensus-based neighbors (ARCH-21) | `definitions/graph_link.rs` |

### 4.16 Provenance Tools (3)

| Tool | Description | Source |
|------|-------------|--------|
| `get_audit_trail` | Query audit log by target memory or time range; chronological operation records | `definitions/provenance.rs` |
| `get_merge_history` | Merge lineage for a memory: source IDs, strategy, operator | `definitions/provenance.rs` |
| `get_provenance_chain` | Full provenance chain: source type, file path, chunk info, operator, timestamps | `definitions/provenance.rs` |

### 4.17 Maintenance Tools (1)

| Tool | Description | Source |
|------|-------------|--------|
| `repair_causal_relationships` | Scan and remove corrupted causal relationship entries that fail deserialization | `definitions/maintenance.rs` |

### 4.18 Daemon Tools (1)

| Tool | Description | Source |
|------|-------------|--------|
| `daemon_status` | Daemon health: active connections, model state, background tasks (GC, HNSW, graph builder), uptime, PID | `definitions/daemon.rs` |

### 4.19 Training Tools (4) -- Training corpus export (Phase 1)

Iterates the teleological store, assembles a full `TrainingRecord` per memory (13 embeddings, 13D topic profile, 78 cross-correlations, 6D group alignments, outgoing + optional incoming typed edges, K-NN neighbors, LLM causal labels, temporal labels), and persists each record to `CF_TRAINING_RECORDS`. Output is RocksDB-only (no JSON/JSONL/Parquet files).

| Tool | Description | Source |
|------|-------------|--------|
| `export_training_corpus` | Iterate memories, build `TrainingRecord`s, persist to `CF_TRAINING_RECORDS`; returns count + bytes + timing summary | `definitions/training.rs` |
| `list_training_records` | Paginated UUID listing of rows in `CF_TRAINING_RECORDS` with optional shape stats | `definitions/training.rs` |
| `get_training_record` | Fetch one `TrainingRecord` by memory UUID, optionally stripping heavy vectors | `definitions/training.rs` |
| `count_training_records` | Row count in `CF_TRAINING_RECORDS` | `definitions/training.rs` |

### 4.20 Constellation Tools (5) -- Constellation compiler (Phase 2)

A *constellation* is a compiled summary of a caller-selected set of memories (topic / session / tag / time range / explicit id list). It captures per-embedder centroids (dense for symmetric/asymmetric embedders; sparse top-K terms for E6/E13; pooled 128D for E12), Welford-online spread stats, reservoir-sampled percentiles, and aggregate topic / group / cross-correlation centroids. Records live in `CF_CONSTELLATIONS` keyed by a fresh UUID; `CF_CONSTELLATION_BY_SELECTOR` indexes `[selector_kind:u8][sha256(canonical_selector)[..16]:16B]` → constellation UUID for O(1) existence checks.

| Tool | Description | Source |
|------|-------------|--------|
| `compile_constellation` | Resolve members from selector, stream into `ConstellationAccumulator`, persist | `definitions/constellation.rs` |
| `list_constellations` | Paginated UUID listing with optional full-shape hydration | `definitions/constellation.rs` |
| `get_constellation` | Point lookup by UUID with optional centroid arrays | `definitions/constellation.rs` |
| `score_against_constellation` | Score a candidate memory against an existing constellation (classification-style scoring) | `definitions/constellation.rs` |
| `delete_constellation` | Remove primary record + selector-index entry in a single `WriteBatch` | `definitions/constellation.rs` |

### 4.21 Contrastive Pair Tools (4) -- Cross-embedder anomaly miner (Phase 3)

Mines `(anchor, negative, similarity_profile)` triples from the corpus: pairs that agree strongly on one embedder axis while disagreeing on another. Each pair is classified into one of six `AnomalyKind` categories and persisted atomically across three CFs (`CF_CONTRASTIVE_PAIRS` primary + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR` secondary indexes). Similarity profiles follow the SRC-3 `[0,1]` convention.

| Tool | Description | Source |
|------|-------------|--------|
| `mine_contrastive_pairs` | Sample candidate pool, score 13-slot similarity profiles, classify, persist triples | `definitions/contrastive.rs` |
| `list_contrastive_pairs` | Paginated `(anchorId, negativeId)` listing with optional kind / anchor filters | `definitions/contrastive.rs` |
| `get_contrastive_pair` | Composite-key `(anchor, negative)` point lookup | `definitions/contrastive.rs` |
| `count_contrastive_pairs` | Total or per-kind row count | `definitions/contrastive.rs` |

---

## 5. Entry Points

### 5.1 Binaries

| Binary | Crate | Path | Purpose |
|--------|-------|------|---------|
| `context-graph-mcp` | context-graph-mcp | `crates/context-graph-mcp/src/main.rs` | MCP JSON-RPC server (stdio, TCP, daemon modes) |
| `context-graph-cli` | context-graph-cli | `crates/context-graph-cli/src/main.rs` | CLI tools for memory management and hooks |
| `train-causal` | context-graph-embeddings | `crates/context-graph-embeddings/src/bin/train_causal.rs` | Offline E5 causal embedder fine-tuning |

### 5.2 CLI Commands (`context-graph-cli`)

| Command | Subcommand | Description |
|---------|------------|-------------|
| `session` | `restore-identity`, `persist-identity` | Session state persistence to/from storage |
| `hooks` | (subcommands) | Claude Code native hooks integration |
| `memory` | (subcommands) | Memory capture and context injection |
| `topic` | (subcommands) | Topic portfolio and stability queries |
| `divergence` | (subcommands) | Divergence detection from recent activity |
| `setup` | N/A | Initialize .claude/settings.json and hooks for Claude Code |
| `warmup` | N/A | Pre-load all 13 embedding models into VRAM (~20-30s on RTX 5090) |
| `watch` | N/A | Watch a directory for markdown file changes and auto-index |

### 5.3 MCP Server Modes

| Mode | Flag | Behavior |
|------|------|----------|
| Stdio (default) | (none) | Reads JSON-RPC from stdin, writes to stdout; single client |
| TCP | `--transport tcp` | Listens on port (default 8080); concurrent clients via tokio tasks |
| Daemon | `--daemon` | Spawns headless TCP server on port 3100; clients proxy stdin through TCP; models loaded once |

### 5.4 Configuration Priority

CLI arguments > Environment variables > Config file (`config/default.toml`) > Hardcoded defaults

Key environment variables:
- `CONTEXT_GRAPH_STORAGE_PATH`: Override RocksDB data directory
- `CONTEXT_GRAPH_TRANSPORT`: Override transport mode (`stdio` or `tcp`)
- `CONTEXT_GRAPH_TCP_PORT`: Override TCP port
- `CONTEXT_GRAPH_BIND_ADDRESS`: Override TCP bind address
- `RUST_LOG`: Control log verbosity
- `CLAUDE_SESSION_ID`: Default session ID for store_memory

---

## 6. Project Directory Structure

### 6.1 Source Layout

```
contextgraph/
  Cargo.toml                  # Workspace manifest (10 members, benchmark excluded from default)
  rust-toolchain.toml         # Stable Rust, x86_64-unknown-linux-gnu
  config/
    default.toml              # Default configuration
    development.toml          # Development overrides
    production.toml           # Production overrides
    test.toml                 # Test overrides
  crates/
    context-graph-core/       # 99K lines - Domain types, traits, business logic
    context-graph-benchmark/  # 87K lines - Benchmarking suite (excluded from default build)
    context-graph-embeddings/ # 76K lines - GPU model inference pipeline
    context-graph-mcp/        # 42K lines - MCP server, handlers, tool definitions
    context-graph-storage/    # 36K lines - RocksDB persistence, HNSW indexes
    context-graph-cli/        # 16K lines - CLI tools
    context-graph-cuda/       #  5K lines - CUDA abstractions
    context-graph-causal-agent/ # 5K lines - LLM causal discovery
    context-graph-graph-agent/  # 4K lines - LLM graph discovery
    context-graph-test-utils/   # <1K lines - Shared test utilities
  models/                     # Pre-trained model weights (SafeTensors + GGUF)
    semantic/                 # E1: e5-large-v2 (1024D)
    causal/                   # E5: nomic-embed fine-tuned for causal (768D)
    code-1536/                # E7: Qodo-Embed-1-1.5B (1536D)
    graph/                    # E8: nomic-embed fine-tuned for graph (1024D)
    entity/                   # E11: KEPLER/TransE (768D)
    multimodal/               # E10: paraphrase model (768D)
    contextual/               # E10 contextual variant
    late-interaction/         # E12: ColBERT (128D/token)
    sparse/                   # E6: sparse keyword model (30K)
    splade-v3/                # E13: SPLADE-v3 (30K)
    temporal/                 # E2/E3/E4: temporal encoding parameters
    hdc/                      # E9: HDC binary hypervector parameters
    kepler/                   # E11: KEPLER TransE embeddings (E11 disabled by default; weights retained)
    gnn/                      # Graph neural network weights
    graph_weights/            # Graph model weight matrices
    hyperbolic/               # Legacy hyperbolic embedding artifacts (retained on disk)
    hermes-2-pro/             # Hermes-2-Pro-Mistral-7B GGUF + GBNF grammars for LLM inference
    models_config.toml        # Model path and configuration mapping
  data/                       # Benchmark and training data
  scripts/                    # Utility scripts
  tests/                      # Integration tests
  examples/                   # Example code
  docs/                       # Documentation and audit reports
```

### 6.2 Runtime Data (Created at Runtime)

```
contextgraph_data/            # Default RocksDB directory (relative to executable)
  *.sst                       # RocksDB SST files (sorted string tables)
  MANIFEST-*                  # RocksDB manifest
  CURRENT                     # RocksDB current manifest pointer
  LOG                         # RocksDB write-ahead log info
  mcp.pid                     # PID file with flock guard (prevents multi-process DB corruption)
```

Override location via `CONTEXT_GRAPH_STORAGE_PATH` env var or `config.storage.path`.

---

## 7. Storage Tier Classification

### 7.1 Sacred (Must Never Be Lost)

| Data | Location | Rationale |
|------|----------|-----------|
| RocksDB SST files | `contextgraph_data/*.sst` | Contains all memories, embeddings, provenance, audit logs; irreplaceable user data |
| RocksDB MANIFEST/WAL | `contextgraph_data/MANIFEST-*`, `*.log` | Required for database consistency and crash recovery |
| Model weights (SafeTensors) | `models/*/model.safetensors` | Pre-trained weights; re-downloading is expensive and may not be available offline |
| GGUF LLM model | `models/hermes-2-pro/*.gguf` | LLM for causal/graph discovery; large download (~6-14GB) |

### 7.2 Regenerable (Can Be Rebuilt from Sacred Data)

| Data | Location | Rebuild Method |
|------|----------|----------------|
| HNSW indexes (in-memory + CF_HNSW_GRAPHS) | RocksDB `hnsw_graphs` CF | Rebuilt on startup if missing; O(n) scan of fingerprints CF |
| K-NN graph edges | RocksDB `embedder_edges` CF | Rebuilt by BackgroundGraphBuilder (~60s cycle) |
| Typed edges | RocksDB `typed_edges` CF | Derived from K-NN edge agreement patterns |
| Topic portfolios | RocksDB `topic_portfolio` CF | Recomputed via `detect_topics` tool |
| E13 SPLADE inverted index | RocksDB `e13_splade_inverted` CF | Rebuilt from fingerprints |
| E6 sparse inverted index | RocksDB `e6_sparse_inverted` CF | Rebuilt from fingerprints |
| E1 Matryoshka 128D vectors | RocksDB `e1_matryoshka_128` CF | Truncated from full E1 1024D vectors |
| Quantized embedder CFs (emb_0..emb_12) | RocksDB | Quantized from full-precision vectors in fingerprints |
| Consolidation recommendations | RocksDB `consolidation_recommendations` CF | Recomputed via `trigger_consolidation` |

### 7.3 Ephemeral (Lost on Restart, Acceptable)

| Data | Location | Lifetime |
|------|----------|----------|
| Session cache | In-memory `SessionCache` | Process lifetime; not persisted to RocksDB |
| Custom weight profiles | RocksDB `custom_weight_profiles` CF (session-scoped) | Session lifetime |
| Embedding cache | In-memory moka cache | Process lifetime |
| PID file + flock | `contextgraph_data/mcp.pid` | Process lifetime; stale locks auto-cleaned |
| Background task state | In-memory atomics | Process lifetime |

---

## 8. Error Hierarchy

### 8.1 Top-Level Unified Error: `ContextGraphError`

Defined in `crates/context-graph-core/src/error/unified.rs`. All crate errors convert into this type. Maps to JSON-RPC error codes for MCP responses.

| Variant | JSON-RPC Code | Recoverable | Critical | Description |
|---------|---------------|-------------|----------|-------------|
| `Embedding(EmbeddingError)` | -32005 | If `ModelNotLoaded` | No | Model loading, generation, quantization, dimension mismatch |
| `Storage(StorageError)` | -32004 | If `Transaction` | If `Corruption` | Database ops, serialization, not found, corruption |
| `Index(IndexError)` | -32008 | If `Timeout` | If `Corruption` | HNSW/inverted index ops, search, construction |
| `Config(ConfigError)` | -32603 | No | No | Missing config, invalid values, env vars, parse errors |
| `Gpu(GpuError)` | -32009 | If `OutOfMemory` | If `NotAvailable` | Device init, VRAM, kernel failures |
| `Mcp(McpError)` | Varies | If `RateLimited` | No | Invalid request/params, method not found, auth, PII detection |
| `Validation(String)` | -32602 | No | No | Input validation failures, NaN/Infinity, range violations |
| `Internal(String)` | -32603 | No | Yes | Invariant violations, unrecoverable state |

### 8.2 Sub-Error Types

| Error Type | Crate | Variants | Source |
|------------|-------|----------|--------|
| `EmbeddingError` | core | ModelNotLoaded, GenerationFailed, Quantization, DimensionMismatch, BatchTooLarge, EmptyInput, WarmupFailed, ModelNotFound, TensorError, LegacyUnknownEmbedder | `error/sub_errors.rs` |
| `StorageError` | core | Database, Serialization, NotFound, AlreadyExists, IncompleteArray, Migration, Corruption, Transaction, WriteFailed, ReadFailed | `error/sub_errors.rs` |
| `IndexError` | core | Hnsw, Inverted, NotFound, RebuildRequired, Corruption, Timeout, ConstructionFailed, InsertionFailed | `error/sub_errors.rs` |
| `ConfigError` | core | Missing, Invalid, EnvNotSet, FileNotFound, ParseError | `error/sub_errors.rs` |
| `GpuError` | core | NotAvailable, OutOfMemory, CudaError, InitFailed, KernelFailed | `error/sub_errors.rs` |
| `McpError` | core | InvalidRequest, MethodNotFound, InvalidParams, RateLimited, Unauthorized, SessionExpired, PiiDetected | `error/sub_errors.rs` |
| `CoreError` | core | Legacy error type (backwards compatibility) | `error/legacy.rs` |
| `CudaError` | cuda | GPU-specific errors | `crates/context-graph-cuda/src/error.rs` |
| `GpuHdbscanError` | cuda | HDBSCAN clustering on GPU | `crates/context-graph-cuda/src/hdbscan/error.rs` |
| `EmbeddingError` | embeddings | Separate from core; embedding pipeline errors | `crates/context-graph-embeddings/src/error/types.rs` |
| `ModelLoadError` | embeddings | Model file loading failures | `crates/context-graph-embeddings/src/gpu/model_loader/error.rs` |
| `UnifiedLoaderError` | embeddings | Unified model loader failures | `crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` |
| `MemoryError` | embeddings | GPU memory allocation errors | `crates/context-graph-embeddings/src/gpu/memory/error.rs` |
| `PQ8QuantizationError` | embeddings | PQ-8 quantization failures | `crates/context-graph-embeddings/src/quantization/pq8/types.rs` |
| `Float8QuantizationError` | embeddings | Float8 quantization failures | `crates/context-graph-embeddings/src/quantization/float8.rs` |
| `BinaryQuantizationError` | embeddings | Binary quantization failures | `crates/context-graph-embeddings/src/quantization/binary.rs` |
| `StorageError` | storage | RocksDB storage layer errors | `crates/context-graph-storage/src/error.rs` |
| `GraphAgentError` | graph-agent | Graph discovery agent errors | `crates/context-graph-graph-agent/src/error.rs` |
| `RetrieverError` | core | Retrieval pipeline errors | `crates/context-graph-core/src/retrieval/retriever.rs` |
| `SimilarityError` | core | Similarity computation errors | `crates/context-graph-core/src/similarity/error.rs` |
| `ClusterError` | core | Clustering operation errors | `crates/context-graph-core/src/clustering/error.rs` |
| `EdgeError` | core | Graph linking edge errors | `crates/context-graph-core/src/graph_linking/error.rs` |
| `ValidationError` | core | Memory node input validation | `crates/context-graph-core/src/types/memory_node/validation.rs` |
| `WeightProfileError` | core | Weight profile creation/lookup errors | `crates/context-graph-core/src/weights/mod.rs` |
| `InjectionError` | core | Memory injection pipeline errors | `crates/context-graph-core/src/injection/pipeline.rs` |
| `WatcherError` | core | File watcher errors | `crates/context-graph-core/src/memory/watcher.rs` |
| `SessionError` | core | Session management errors | `crates/context-graph-core/src/memory/manager.rs` |

### 8.3 JSON-RPC Error Codes

| Code | Name | Scope |
|------|------|-------|
| -32700 | PARSE_ERROR | Standard JSON-RPC |
| -32600 | INVALID_REQUEST | Standard JSON-RPC |
| -32601 | METHOD_NOT_FOUND | Standard JSON-RPC |
| -32602 | INVALID_PARAMS | Standard JSON-RPC |
| -32603 | INTERNAL_ERROR | Standard JSON-RPC |
| -32001 | FEATURE_DISABLED | Context Graph |
| -32002 | NODE_NOT_FOUND | Context Graph |
| -32004 | STORAGE_ERROR | Context Graph |
| -32005 | EMBEDDING_ERROR | Context Graph |
| -32006 | TOOL_NOT_FOUND | Context Graph |
| -32007 | LAYER_TIMEOUT | Context Graph |
| -32008 | INDEX_ERROR | Context Graph |
| -32009 | GPU_ERROR | Context Graph |
| -32021 | INSUFFICIENT_MEMORIES | Context Graph (topic detection) |
| -32110 | TCP_BIND_FAILED | TCP Transport |
| -32111 | TCP_CONNECTION_ERROR | TCP Transport |
| -32112 | TCP_MAX_CONNECTIONS_REACHED | TCP Transport |

Source: `crates/context-graph-mcp/src/protocol.rs`

---

## 9. Major Subsystem Summaries

### 9.1 Core Domain (`context-graph-core`, 99K lines)

The foundational crate defining all domain types, traits, and business logic. Key types include `TeleologicalFingerprint` (a memory's 13-vector representation across all embedders), `SemanticFingerprint`, and `TopicProfile`. Defines the `TeleologicalMemoryStore` and `MultiArrayEmbeddingProvider` traits that the storage and embeddings crates implement. Contains the fusion module (Weighted RRF across embedders), retrieval pipeline (e1_only / multi_space / pipeline / embedder_first strategies), similarity computation (dense cosine, sparse dot product, MaxSim), clustering (HDBSCAN for topic detection with hierarchical topics), graph linking (K-NN edge computation and typed edge derivation), injection pipeline, quantization (PQ-8, Float8, Binary), entity subsystem (TransE knowledge graph operations), causal subsystem (direction detection, asymmetric scoring), the weights module (14 built-in weight profiles with `E11_ENTITY_ENABLED = false` redistribution), dream-phase consolidation (DreamPhase enum), importance decay, plus three Phase-1/2/3 training-data subsystems: `training/` (TrainingRecord v1 with 78 cross-correlations and 6D group alignments; Phase 5 temporal labels in `temporal.rs`; bincode-on-disk with `TRAINING_RECORD_VERSION=1`), `constellation/` (per-embedder centroids via `WelfordVector`, percentile reservoirs, compiler two-pass accumulator; `CONSTELLATION_VERSION=1`, `RESERVOIR_SAMPLE_SIZE=1024`, `MIN_CONSTELLATION_MEMBERS=3`), and `contrastive/` (13-slot similarity profile miner with six `AnomalyKind`s; `CONTRASTIVE_PAIR_VERSION=1`). Source: `crates/context-graph-core/src/lib.rs`.

### 9.2 Embeddings Pipeline (`context-graph-embeddings`, 76K lines)

GPU-accelerated embedding generation using the HuggingFace Candle framework. Manages 13 embedding models (see 02_embedder_architecture.md) loaded into VRAM via a `ProductionMultiArrayProvider` that implements the core `MultiArrayEmbeddingProvider` trait. Includes a `UnifiedModelLoader` for lifecycle management, a moka-based embedding cache for deduplication, batch processing with configurable strategies, and quantization modules (PQ-8 for high-dimensional dense, Float8E4M3 for temporal, Binary for HDC). The `train-causal` binary provides offline E5 fine-tuning with a 3-stage progressive pipeline (projection warmup, LoRA activation, directional emphasis). The `candle` feature is mandatory -- a `compile_error!` fires if it is not enabled. Source: `crates/context-graph-embeddings/src/lib.rs`.

### 9.3 MCP Server (`context-graph-mcp`, 42K lines)

JSON-RPC 2.0 server implementing the Model Context Protocol specification (2024-11-05). Exposes 71-75 tools across 22 domains via `tools/list` and `tools/call` methods (75 constants in `tools/names.rs`, 88 dispatch arms in `handlers/tools/dispatch.rs`). Supports stdio transport (default, for Claude Code process integration), TCP transport (for networked deployments), and daemon mode (shared server across terminals with single VRAM load). Handles initialization/capabilities handshake, PID file locking with flock to prevent multi-process RocksDB corruption, bounded line reading (10MB limit per request, 100 max batch size), and graceful shutdown. Background tasks include file watcher, code watcher, and K-NN graph builder. The `llm` feature gates 4 additional LLM-dependent tools. Handler modules for training-data pipeline: `handlers/tools/training_tools.rs` (4 tools), `handlers/tools/constellation_tools.rs` (5 tools), `handlers/tools/contrastive_tools.rs` (4 tools), `handlers/tools/typed_edges_tools.rs` (4 tools); matching schemas live in `tools/definitions/{training,constellation,contrastive,typed_edges}.rs`. Source: `crates/context-graph-mcp/src/main.rs`, `src/server/mod.rs`.

### 9.4 Storage Layer (`context-graph-storage`, 36K lines)

RocksDB-backed persistence with 59 column families organized into base (11 = 8 original + 3 graph linking), teleological (28), quantized embedder (13), code (5), and causal (2) groups. The teleological group now includes 8 new CFs beyond the original 20: `CF_TRAINING_RECORDS` (Phase 1), `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` (Phase 2), `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR` (Phase 3), and `CF_TYPED_EDGE_RECORDS` + `CF_TYPED_EDGE_VALIDATIONS` (Phase 4). The `RocksDbTeleologicalStore` implements the core `TeleologicalMemoryStore` trait. Includes usearch-based HNSW indexes (persisted to `hnsw_graphs` CF), E13 SPLADE and E6 sparse inverted indexes, E12 ColBERT MaxSim scoring, secondary indexes (tags, temporal, sources, file), the `EdgeRepository` for K-NN and typed edge management, and persistence modules `teleological/rocksdb_store/export.rs` (training records), `teleological/rocksdb_store/constellation.rs` (constellation primary + selector-index), `teleological/rocksdb_store/contrastive.rs` (pair + two secondary indexes), `teleological/rocksdb_store/typed_edge_export.rs` (F1 export corpus), `teleological/rocksdb_store/llm_validation.rs` (F4 LLM verdicts), `teleological/rocksdb_store/anomaly_derivation.rs` (F2 anomaly derivation from edges), `teleological/rocksdb_store/typed_edge_keys.rs` (shared 33-byte composite key format), and `teleological/rocksdb_store/versioned_bincode.rs` (shared encode/decode with version byte). Write buffer limits are applied per-CF to prevent excessive memory usage across 59 CFs. Source: `crates/context-graph-storage/src/lib.rs`.

### 9.5 CLI (`context-graph-cli`, 16K lines)

Command-line interface built on clap 4.5 providing session persistence, Claude Code hooks integration, memory capture/injection, topic portfolio queries, divergence detection, VRAM warmup, and directory watching. Designed for integration with Claude Code's hooks system (pre-task, post-task, pre-edit, post-edit) via `.claude/settings.json`. Exit codes follow AP-26: 0=success, 1=error, 2=corruption. Source: `crates/context-graph-cli/src/main.rs`.

### 9.6 CUDA Abstractions (`context-graph-cuda`, 5K lines)

GPU acceleration layer targeting NVIDIA RTX 5090 (Blackwell, compute capability 12.0) with CUDA 13.x. Currently provides trait abstractions and HDBSCAN GPU clustering error types. Direct Candle tensor operations pending Candle 0.9.x CUDA 13.1 stabilization. Source: `crates/context-graph-cuda/Cargo.toml`.

### 9.7 Causal Discovery Agent (`context-graph-causal-agent`, 5K lines)

LLM-based causal relationship discovery using Hermes-2-Pro-Mistral-7B (or Qwen2.5) via llama-cpp-2 with GBNF grammar constraints for 100% valid JSON output. Analyzes memories to extract cause-effect relationships, generates E5 embeddings via `CausalModel.embed_dual()` for asymmetric causal vectors, and stores results in `CF_CAUSAL_RELATIONSHIPS`. VRAM: ~6GB FP16 (3B model), ~14GB FP16 (7B model). Source: `crates/context-graph-causal-agent/Cargo.toml`.

### 9.8 Graph Discovery Agent (`context-graph-graph-agent`, 4K lines)

LLM-based graph relationship discovery sharing the `CausalDiscoveryLLM` from the causal-agent crate. Supports 20 relationship types across 4 domains (Code: imports, calls, implements; Legal: cites, overrules, interprets; Academic: cites, applies, extends; General). Validates proposed links and discovers new relationships between memory pairs. Source: `crates/context-graph-graph-agent/Cargo.toml`.

### 9.9 Benchmark Suite (`context-graph-benchmark`, 87K lines)

Comprehensive benchmarking suite comparing 13-embedder multi-space fingerprinting against single-embedder RAG baselines. Excluded from default workspace build (`cargo build -p context-graph-benchmark` to compile). Includes result aggregation, validation, and statistical analysis. Source: `crates/context-graph-benchmark/Cargo.toml`.

### 9.10 RocksDB Column Family Summary

| Group | Count | Examples |
|-------|-------|---------|
| Base | 11 | 8 original (nodes, edges, embeddings, metadata, temporal, tags, sources, system) + 3 graph linking (embedder_edges, typed_edges, typed_edges_by_type) |
| Teleological | 28 | 20 core (fingerprints, content, source_metadata, file_index, topic_profiles, topic_portfolio, audit_log, audit_by_target, merge_history, importance_history, hnsw_graphs, custom_weight_profiles, e12_late_interaction, entity_provenance, tool_call_index, consolidation_recommendations, embedding_registry, ...) + 6 Phase 1-3 (training_records, constellations, constellation_by_selector, contrastive_pairs, contrastive_by_kind, contrastive_by_anchor) + 2 Phase 4 (typed_edge_records, typed_edge_validations) |
| Quantized Embedder | 13 | emb_0 (E1 PQ-8) through emb_12 (E13 SparseNative) |
| Code | 5 | code_entities, code_e7_embeddings, code_file_index, code_name_index, code_signature_index |
| Causal | 2 | causal_relationships, causal_by_source |
| **Total** | **59** | |

Source: `crates/context-graph-storage/src/column_families.rs:397` (`TOTAL_COLUMN_FAMILIES = 59`). Teleological group expanded at `crates/context-graph-storage/src/teleological/column_families.rs` — Phase 1: training_records; Phase 2: constellations, constellation_by_selector; Phase 3: contrastive_pairs, contrastive_by_kind, contrastive_by_anchor; Phase 4: typed_edge_records, typed_edge_validations; count constant at `:551` (`TELEOLOGICAL_CF_COUNT = 28`).

---

*Cross-references: For embedder architecture details see `02_embedder_architecture.md`. For search strategy internals see the retrieval/fusion modules in context-graph-core. For RocksDB schema details see the column_families modules in context-graph-storage.*
