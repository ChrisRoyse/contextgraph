# Context Graph

**Persistent, multi-dimensional semantic memory for AI assistants.**

[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)

Context Graph is an MCP server that gives AI assistants like Claude long-term memory with 13 specialized embedding dimensions. Every memory is embedded simultaneously across semantic, causal, temporal, code, entity, and structural spaces — then fused at search time using Reciprocal Rank Fusion to surface results that no single perspective could find alone.

```
Store a memory  ──►  13 embedders fire in parallel  ──►  RocksDB + HNSW indexes
Search a query  ──►  6 embedders retrieve candidates ──►  RRF fusion  ──►  ranked results
```

---

## Why Context Graph?

Most memory systems for AI use a single embedding model and basic vector search. Context Graph takes a fundamentally different approach:

**Multi-perspective retrieval.** A query like *"Why does auth fail under load?"* searches simultaneously through semantic similarity (E1), causal reasoning (E5), code patterns (E7), entity linking (E11), graph structure (E8), and paraphrase matching (E10). Each perspective catches what the others miss.

**Asymmetric causal reasoning.** Three embedders store dual vectors for directional queries. "What caused X?" and "What did X cause?" return different results because cause and effect are embedded separately with directional boosting.

**Temporal awareness without temporal bias.** Time-based embedders (freshness, periodicity, sequence) are applied as post-retrieval boosts, not during retrieval. This prevents recent memories from drowning out relevant older ones.

**55 MCP tools.** Not just store-and-search — full causal chain building, entity extraction with TransE predictions, topic discovery via HDBSCAN, code-aware search with AST chunking, file watching, provenance tracking, and LLM-powered relationship discovery.

**Production-grade storage.** RocksDB with 51 column families, HNSW indexes for O(log n) K-NN search, soft-delete with 30-day recovery, background compaction, and graceful degradation when components fail.

---

## Quick Start

### Prerequisites

- **Rust** 1.75+ (stable)
- **CUDA** toolkit (for GPU-accelerated embeddings via candle)
- **RocksDB** system library

### Build

```bash
git clone https://github.com/contextgraph/contextgraph.git
cd contextgraph
make build
```

### Run

```bash
# Stdio mode (default — for Claude Code / Claude Desktop)
context-graph-mcp

# TCP mode (remote clients)
context-graph-mcp --transport tcp --port 3100

# Daemon mode (shared server, load models once)
context-graph-mcp --daemon

# Fast startup (models load in background)
context-graph-mcp --no-warm
```

### Connect to Claude Code

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "context-graph": {
      "command": "context-graph-mcp",
      "args": ["--transport", "stdio"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Once connected, Claude has access to all 55 MCP tools — persistent memory, causal reasoning, entity linking, code search, and more — with no further configuration.

---

## Key Features

### 13 Specialized Embedders

Every memory is embedded across 13 spaces simultaneously. Each acts as an independent "knowledge lens."

| # | Name | Model | Dim | Purpose |
|---|------|-------|-----|---------|
| **E1** | Semantic | e5-large-v2 | 1024D | Primary semantic similarity |
| **E2** | Freshness | Custom temporal | 512D | Exponential recency decay |
| **E3** | Periodic | Fourier-based | 512D | Time-of-day / day-of-week patterns |
| **E4** | Sequence | Sinusoidal positional | 512D | Conversation ordering |
| **E5** | Causal | nomic-embed-v1.5 + LoRA | 768D | Cause-effect relationships (asymmetric) |
| **E6** | Keyword | SPLADE cocondenser | ~30K | BM25-style sparse keyword matching |
| **E7** | Code | Qodo-Embed-1.5B | 1536D | Source code understanding (AST-aware) |
| **E8** | Graph | e5-large-v2 | 1024D | Directional graph connections (asymmetric) |
| **E9** | HDC | Hyperdimensional | 1024D | Character-level typo tolerance |
| **E10** | Paraphrase | e5-base-v2 | 768D | Rephrase-invariant matching (asymmetric) |
| **E11** | Entity | KEPLER | 768D | Named entity & TransE linking |
| **E12** | ColBERT | ColBERT | 128D/tok | Late interaction precision (pipeline stage) |
| **E13** | SPLADE | SPLADE v3 | ~30K | Learned sparse expansion (pipeline stage) |

### 3 Search Strategies

| Strategy | How it works | Best for |
|----------|-------------|----------|
| **e1_only** (default) | Single E1 HNSW search (~1ms) | Simple similarity, lowest latency |
| **multi_space** | Weighted RRF across 6 active embedders (E1, E5, E7, E8, E10, E11) | General-purpose queries |
| **pipeline** | E13 recall → multi-space scoring → E12 ColBERT rerank | Maximum precision |

Individual embedders can also be queried directly via `search_by_embedder` for specialized single-perspective queries (code, causal, entity).

### 14 Weight Profiles

Predefined profiles control how embedders are weighted during multi-space search:

| Profile | Primary Focus | Best For |
|---------|--------------|----------|
| `semantic_search` | E1 semantic | General queries |
| `causal_reasoning` | E1 + E5 causal | "Why" questions, root cause analysis |
| `code_search` | E7 code | Programming queries, function lookup |
| `fact_checking` | E11 entity + E6 keyword | Entity/fact validation |
| `graph_reasoning` | E8 graph + E11 entity | Connection traversal |
| `temporal_navigation` | E2/E3/E4 temporal | Time-based queries |
| `sequence_navigation` | E4 positional | Conversation sequence traversal |
| `conversation_history` | E4 + E1 balanced | Contextual recall from conversation |
| `category_weighted` | Constitution-compliant | Balanced across embedder categories |
| `typo_tolerant` | E1 + E9 HDC | Misspelled queries |
| `pipeline_stage1_recall` | E13 + E6 sparse | Pipeline recall stage |
| `pipeline_stage2_scoring` | E1 semantic | Pipeline scoring stage |
| `pipeline_full` | E13 → E1 → E12 | End-to-end precision pipeline |
| `balanced` | Equal across all 13 | Testing and comparison |

Custom profiles can be created per-session via `create_weight_profile`.

---

## How It Works

A query like *"Why does the authentication service fail under load?"*:

1. **Intent detection** — classified as causal (seeking effects of load)
2. **Strategy selection** — `multi_space` with `causal_reasoning` weight profile
3. **Parallel retrieval** across 6 active embedders:
   - **E1**: Semantic HNSW search for "authentication service fail load"
   - **E5**: Asymmetric causal search (query as cause, searching effect index, 1.2x boost)
   - **E7**: Code embeddings catch relevant auth service implementations
   - **E8**: Graph connections find structurally related memories
   - **E10**: Paraphrase matching catches rephrasings of the same concept
   - **E11**: Entity linking identifies "authentication service" as a known entity
4. **RRF fusion** — rankings merged: `weight_i / (rank_i + 60)` across all embedders
5. **Post-retrieval boosts** — E2 freshness decay prioritizes recent memories
6. **Causal gate** — high-confidence causal scores get 1.10x boost, low-confidence get 0.85x demotion
7. **Return** — top-k results with per-embedder breakdown and full provenance

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   MCP Clients                       │
│          (Claude Code, Claude Desktop)              │
└──────────────────────┬──────────────────────────────┘
                       │ JSON-RPC 2.0
                       │ (stdio / TCP / SSE)
┌──────────────────────▼──────────────────────────────┐
│              Context Graph MCP Server               │
│                   55 MCP Tools                      │
├─────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌──────────┐  ┌────────────────────┐ │
│  │ Handlers│  │ Transport│  │  Background Tasks  │ │
│  │ (tools) │  │ Layer    │  │  - HNSW compaction │ │
│  │         │  │          │  │  - Soft-delete GC  │ │
│  │         │  │          │  │  - Graph builder   │ │
│  │         │  │          │  │  - File watcher    │ │
│  └────┬────┘  └──────────┘  └────────────────────┘ │
├───────┼─────────────────────────────────────────────┤
│  ┌────▼──────────────────────────────────────────┐  │
│  │          13-Embedder Pipeline                 │  │
│  │  E1 Semantic    E5 Causal     E9  HDC        │  │
│  │  E2 Freshness   E6 Keyword    E10 Paraphrase │  │
│  │  E3 Periodic    E7 Code       E11 Entity     │  │
│  │  E4 Sequence    E8 Graph      E12 ColBERT    │  │
│  │                               E13 SPLADE     │  │
│  └────┬──────────────────────────────────────────┘  │
│  ┌────▼──────────────────────────────────────────┐  │
│  │          RocksDB + HNSW Indexes               │  │
│  │  51 Column Families  │  usearch K-NN Graphs   │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### Workspace Crates

| Crate | Purpose |
|-------|---------|
| `context-graph-mcp` | MCP server, transport layer, 55 tool handlers |
| `context-graph-core` | Domain types, config, traits, 14 weight profiles |
| `context-graph-storage` | RocksDB persistence, 51 column families, HNSW indexes |
| `context-graph-embeddings` | 13-model embedding pipeline (HuggingFace candle) |
| `context-graph-graph` | Knowledge graph with vector search |
| `context-graph-cuda` | GPU acceleration (CUDA / candle) |
| `context-graph-cli` | CLI tools, Claude Code hooks |
| `context-graph-causal-agent` | LLM-based causal discovery |
| `context-graph-graph-agent` | LLM-based graph relationship discovery |
| `context-graph-benchmark` | Performance benchmarking suite |
| `context-graph-test-utils` | Shared test utilities |

---

## MCP Tools

### Core Memory

| Tool | Description |
|------|-------------|
| `store_memory` | Store a memory with content, rationale, importance, tags, and session tracking |
| `search_graph` | Multi-space semantic search with configurable strategy and weight profile |
| `get_memetic_status` | System status: fingerprint count, embedder health, storage info |
| `trigger_consolidation` | Merge similar memories using similarity, temporal, or semantic strategies |

### Memory Curation

| Tool | Description |
|------|-------------|
| `merge_concepts` | Merge related memories with union/intersection/weighted_average strategies |
| `forget_concept` | Soft-delete a memory (30-day recovery window) |
| `boost_importance` | Adjust memory importance score (clamped 0.0-1.0) |

### Causal Reasoning

| Tool | Description |
|------|-------------|
| `search_causal_relationships` | Search for causal relationships with provenance |
| `search_causes` | Abductive reasoning — find likely causes of an observed effect |
| `search_effects` | Forward causal reasoning — predict effects of a cause |
| `get_causal_chain` | Build transitive causal chains with hop attenuation |
| `trigger_causal_discovery` | Run LLM-based causal discovery (requires LLM feature) |
| `get_causal_discovery_status` | Agent status, VRAM usage, statistics |

### Entity & Knowledge Graph

| Tool | Description |
|------|-------------|
| `extract_entities` | Extract named entities via pattern matching and knowledge base lookup |
| `search_by_entities` | Find memories by entity names with hybrid E11 ranking |
| `infer_relationship` | TransE knowledge graph relationship prediction |
| `find_related_entities` | Find entities connected via specific relationships |
| `validate_knowledge` | Score (subject, predicate, object) triples using TransE |
| `get_entity_graph` | Build and visualize entity relationship graph |

### Session & Conversation

| Tool | Description |
|------|-------------|
| `get_conversation_context` | Get memories around current conversation turn |
| `get_session_timeline` | Ordered timeline of session memories with sequence numbers |
| `traverse_memory_chain` | Multi-hop traversal starting from an anchor memory |
| `compare_session_states` | Compare memory state at different sequence points |

### Topic Detection

| Tool | Description |
|------|-------------|
| `get_topic_portfolio` | All discovered topics with profiles and stability metrics |
| `get_topic_stability` | Portfolio-level stability (churn, entropy, phase breakdown) |
| `detect_topics` | Force topic detection using HDBSCAN |
| `get_divergence_alerts` | Check for divergence from recent activity |

### Embedder-First Search

| Tool | Description |
|------|-------------|
| `search_by_embedder` | Search using any single embedder as primary perspective |
| `get_embedder_clusters` | Explore HDBSCAN clusters in a specific embedder space |
| `compare_embedder_views` | Side-by-side comparison of embedder rankings for a query |
| `list_embedder_indexes` | Statistics for all 13 embedder indexes |
| `get_memory_fingerprint` | Retrieve per-embedder vectors for a memory |
| `create_weight_profile` | Create session-scoped custom weight profiles |
| `search_cross_embedder_anomalies` | Find blind spots (high in one embedder, low in another) |

### Specialized Search

| Tool | Description |
|------|-------------|
| `search_by_keywords` | E6 sparse keyword search with term expansion |
| `search_code` | E7 code-specific search with AST context and language detection |
| `search_robust` | E9 typo-tolerant search using character trigram hypervectors |
| `search_recent` | E2 freshness-decayed search (exponential/linear/step) |
| `search_periodic` | E3 time-pattern matching (similar times of day/week) |

### Graph Navigation

| Tool | Description |
|------|-------------|
| `search_connections` | Find memories connected via asymmetric E8 similarity |
| `get_graph_path` | Multi-hop graph traversal with hop attenuation (0.9^hop) |
| `get_memory_neighbors` | K-NN neighbors in a specific embedder space |
| `get_typed_edges` | Explore typed edges derived from embedder agreement |
| `traverse_graph` | Multi-hop traversal following typed edges |
| `get_unified_neighbors` | Unified neighbors via Weighted RRF across all embedders |
| `discover_graph_relationships` | LLM-based relationship discovery across 20 types |
| `validate_graph_link` | Validate proposed graph links with confidence scoring |

### File Watcher

| Tool | Description |
|------|-------------|
| `list_watched_files` | List files with embeddings in the knowledge graph |
| `get_file_watcher_stats` | Statistics on watched file content |
| `delete_file_content` | Delete embeddings for a file path (soft-delete) |
| `reconcile_files` | Find and clean up orphaned file embeddings |

### Provenance & Audit

| Tool | Description |
|------|-------------|
| `get_audit_trail` | Query append-only audit log for memory operations |
| `get_merge_history` | Show merge lineage and history for fingerprints |
| `get_provenance_chain` | Full provenance from embedding to source |

### Maintenance

| Tool | Description |
|------|-------------|
| `repair_causal_relationships` | Repair corrupted causal relationship entries |

---

## Storage

### RocksDB Column Families (51 total)

| Layer | CFs | Contents |
|-------|-----|----------|
| **Core** | 11 | Nodes, edges, embeddings, metadata, temporal, tags, sources, system, typed edges |
| **Teleological** | 20 | Fingerprints, topic profiles, synergy matrix, causal relationships, weight profiles, inverted indexes |
| **Quantized** | 13 | `CF_EMB_0` through `CF_EMB_12` — quantized vectors per embedder (PQ-8 or Float8) |
| **Code** | 5 | AST chunks, language indexes, symbol tables |
| **Causal** | 2 | Causal relationship metadata and indexes |

### HNSW Indexing

10 of 13 embedders use [usearch](https://github.com/unum-cloud/usearch) HNSW graphs for O(log n) K-NN search. E6/E13 (sparse) use inverted indexes. E12 (ColBERT) uses MaxSim token-level scoring. HNSW graphs are persisted to RocksDB and compacted on a 10-minute background interval.

---

## Configuration

### Priority Order

1. **CLI arguments** (highest)
2. **Environment variables** (`CONTEXT_GRAPH_` prefix)
3. **Config files** (`config/default.toml`, `config/{env}.toml`)
4. **Defaults** (lowest)

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CONTEXT_GRAPH_TRANSPORT` | `stdio` | Transport: `stdio`, `tcp`, `sse`, `stdio+tcp` |
| `CONTEXT_GRAPH_TCP_PORT` | `3100` | TCP port |
| `CONTEXT_GRAPH_SSE_PORT` | `3101` | SSE port |
| `CONTEXT_GRAPH_BIND_ADDRESS` | `127.0.0.1` | Bind address |
| `CONTEXT_GRAPH_DAEMON` | `false` | Enable daemon mode |
| `CONTEXT_GRAPH_WARM_FIRST` | `true` | Block until models load |
| `CONTEXT_GRAPH_STORAGE_PATH` | — | RocksDB database path |
| `CONTEXT_GRAPH_MODELS_PATH` | — | Embedding models path |
| `CONTEXT_GRAPH_ENV` | `development` | Config environment |
| `RUST_LOG` | `info` | Log level |

### Config File

```toml
[mcp]
transport = "stdio"
tcp_port = 3100
sse_port = 3101
bind_address = "127.0.0.1"
max_payload_size = 10485760
request_timeout = 30
max_connections = 32

[storage]
backend = "rocksdb"

[watcher]
enabled = true
watch_paths = ["./docs"]
extensions = ["md"]

[watcher.code]
enabled = false
watch_paths = ["./src"]
extensions = ["rs"]
use_ast_chunker = true
target_chunk_size = 500
```

---

## Transport Modes

| Mode | Protocol | Use Case |
|------|----------|----------|
| **stdio** | Newline-delimited JSON over stdin/stdout | Claude Code, Claude Desktop |
| **tcp** | JSON-RPC over TCP socket | Remote deployments, multiple clients |
| **sse** | Server-Sent Events over HTTP | Web clients, real-time streaming |
| **stdio+tcp** | Both simultaneously | stdio for Claude Code + TCP for hooks/CLI |
| **daemon** | Shared TCP server | Single server instance across multiple terminals |

---

## CLI & Hooks

The CLI provides Claude Code hooks for automatic memory capture during sessions:

```bash
# Set up hooks for Claude Code
context-graph-cli setup

# Manual memory operations
context-graph-cli memory capture --content "learned something" --rationale "important pattern"
context-graph-cli memory inject --query "authentication patterns"
context-graph-cli topic portfolio
context-graph-cli warmup   # Pre-load embeddings into VRAM
```

### Hook Timeouts

| Hook | Timeout | Trigger |
|------|---------|---------|
| `session-start` | 5s | Session begins — injects previous session context |
| `pre-tool-use` | 500ms | Before each tool call |
| `post-tool-use` | 3s | After each tool call |
| `user-prompt-submit` | 2s | User sends a message |
| `pre-compact` | 20s | Before context compaction — preserves important context |
| `task-completed` | 20s | Task finishes — captures learnings |
| `session-end` | 30s | Session ends — persists session summary |

---

## Graceful Degradation

Context Graph is designed to keep working when components fail:

- **LLM unavailable**: 52 of 55 tools work normally. Only `trigger_causal_discovery`, `discover_graph_relationships`, and `validate_graph_link` return errors. Build without the `llm` feature to skip LLM dependencies entirely (~500MB smaller binary).
- **Embedder failure**: The pipeline handles individual embedder loading failures. Search falls back to available embedders with degraded-mode tracking.
- **Soft-delete**: All deletions are soft with a 30-day recovery window. A background GC task runs every 5 minutes to clean up expired deletions.
- **HNSW compaction**: Background task rebuilds HNSW indexes every 10 minutes. Safe concurrent reads during rebuild.

---

## Performance

| Operation | Target |
|-----------|--------|
| `store_memory` | < 5ms p95 |
| `get_node` | < 1ms p95 |
| Context injection | < 25ms p95, < 50ms p99 |
| HNSW K-NN search | O(log n) |
| Embedding validation | < 1ms |
| Health check | < 1ms |

---

## Building Without GPU

To build without CUDA (CPU-only embeddings):

```bash
cargo build --release --no-default-features --features llm
```

To build without LLM support (smaller binary, 51 tools):

```bash
cargo build --release --no-default-features --features cuda
```

---

## Development

```bash
# Run all tests
make test

# Run E2E hook tests
make test-e2e

# Run MCP server tests
make test-mcp

# Quick check (no linking)
make check

# Lint
make clippy

# Disk usage report
make disk-check
```

### Project Structure

```
contextgraph/
├── crates/
│   ├── context-graph-mcp/          # MCP server, transport, tool handlers
│   ├── context-graph-core/         # Domain types, config, traits, weight profiles
│   ├── context-graph-storage/      # RocksDB persistence, 51 column families
│   ├── context-graph-embeddings/   # 13-model embedding pipeline (candle)
│   ├── context-graph-graph/        # Knowledge graph with vector search
│   ├── context-graph-cuda/         # GPU acceleration
│   ├── context-graph-cli/          # CLI and Claude Code hooks
│   ├── context-graph-causal-agent/ # LLM-based causal discovery
│   ├── context-graph-graph-agent/  # LLM-based graph relationship discovery
│   ├── context-graph-benchmark/    # Performance benchmarking suite
│   └── context-graph-test-utils/   # Shared test helpers
├── config/                         # Configuration files (TOML)
├── scripts/                        # Build and maintenance scripts
└── Makefile                        # Build targets
```

---

## Protocol

- **MCP Version**: 2024-11-05
- **Message Format**: Newline-delimited JSON (NDJSON)
- **RPC**: JSON-RPC 2.0

## License

Licensed under either of:

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT License](http://opensource.org/licenses/MIT)

at your option.
