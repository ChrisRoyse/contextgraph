# Context Graph

A 13-dimensional embedded knowledge graph and semantic memory system, exposed as an MCP (Model Context Protocol) server. Context Graph gives AI assistants persistent, searchable, multi-perspective memory with causal reasoning, entity linking, code understanding, and temporal awareness.

## What It Does

Context Graph stores memories as rich multi-dimensional fingerprints — each memory is embedded simultaneously across 13 specialized embedding spaces. When you search, the system fuses results from multiple perspectives using Reciprocal Rank Fusion (RRF), finding matches that no single embedder could surface alone.

**Core capabilities:**

- **Semantic search** across 13 embedding dimensions with configurable weight profiles
- **Causal reasoning** — find causes, effects, and build causal chains with asymmetric embeddings
- **Entity extraction and linking** — TransE knowledge graph predictions
- **Code-aware search** — AST-aware code embeddings (1536D Qodo-Embed)
- **Temporal navigation** — freshness decay, periodic patterns, sequence ordering
- **Typo tolerance** — hyperdimensional computing for noise-robust search
- **Topic discovery** — emergent clustering via HDBSCAN across embedder agreement
- **File watching** — automatic ingestion of documentation and source code
- **Provenance tracking** — full audit trail for all memory operations
- **Claude Code integration** — hooks for session start, tool use, compaction, and task completion

## Architecture Overview

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
| `context-graph-mcp` | MCP server, transport layer, tool handlers |
| `context-graph-core` | Domain types, config, traits, weight profiles |
| `context-graph-storage` | RocksDB persistence, column families, HNSW indexes |
| `context-graph-embeddings` | 13-embedder pipeline (HuggingFace models via candle) |
| `context-graph-graph` | Knowledge graph with vector search |
| `context-graph-cuda` | GPU acceleration (CUDA) |
| `context-graph-cli` | CLI tools, Claude Code hooks |
| `context-graph-causal-agent` | LLM-based causal discovery (Qwen2.5-3B) |
| `context-graph-graph-agent` | LLM-based graph relationship discovery |
| `context-graph-benchmark` | Performance benchmarking suite |
| `context-graph-test-utils` | Shared test utilities |

---

## The 13 Embedders

Every memory is embedded simultaneously across 13 specialized spaces. Each embedder acts as an independent "knowledge lens" — what looks similar in one space may be distant in another.

| # | Name | Model | Dim | Type | Purpose |
|---|------|-------|-----|------|---------|
| **E1** | Semantic | e5-large-v2 | 1024D | Dense | Primary semantic similarity |
| **E2** | Freshness | Custom temporal | 512D | Dense | Exponential recency decay |
| **E3** | Periodic | Fourier-based | 512D | Dense | Time-of-day / day-of-week patterns |
| **E4** | Sequence | Sinusoidal positional | 512D | Dense | Conversation ordering |
| **E5** | Causal | Longformer SCM | 768D | Dense, Asymmetric | Cause-effect relationships |
| **E6** | Keyword | SPLADE v2 | ~30K | Sparse | BM25-style keyword matching |
| **E7** | Code | Qodo-Embed-1.5B | 1536D | Dense | Source code understanding |
| **E8** | Graph | e5-large-v2 | 1024D | Dense, Asymmetric | Directional graph connections |
| **E9** | HDC | Hyperdimensional | 1024D | Dense | Character-level typo tolerance |
| **E10** | Paraphrase | e5-base | 768D | Dense, Asymmetric | Rephrase-invariant matching |
| **E11** | Entity | KEPLER | 768D | Dense | Named entity & TransE linking |
| **E12** | ColBERT | ColBERT | 128D/token | Token-level | Late interaction precision (pipeline only) |
| **E13** | SPLADE | SPLADE v3 | ~30K | Sparse | Learned sparse expansion (pipeline only) |

### Asymmetric Embedders

Three embedders store **dual vectors** for directional reasoning:

- **E5 (Causal)**: `cause_vector` and `effect_vector` — querying for causes searches the effect index with a cause query, applying a 1.2x boost for cause→effect and 0.8x for effect→cause
- **E8 (Graph)**: `source_vector` and `target_vector` — "what points to X?" differs from "what does X point to?"
- **E10 (Paraphrase)**: `doc_vector` and `query_vector` — asymmetric document-query similarity

### Embedder Categories

| Category | Embedders | Topic Weight | Notes |
|----------|-----------|-------------|-------|
| **Semantic** | E1, E5, E6, E7, E10, E12, E13 | 1.0 | Core similarity signals |
| **Temporal** | E2, E3, E4 | 0.0 | Excluded from topic detection; applied post-retrieval |
| **Relational** | E8, E11 | 0.5 | Graph structure and entity relationships |
| **Structural** | E9 | 0.5 | Noise-robust character patterns |

---

## Search Strategies

### 1. E1 Only (`e1_only`)

Uses only the E1 semantic HNSW index. Fastest option (~1ms), backward compatible.

### 2. Multi-Space (`multi_space`) — Default

Fuses rankings from 6 active embedders (E1, E5, E7, E8, E10, E11) using **Weighted Reciprocal Rank Fusion**:

```
score(doc) = Sum[ weight_i / (rank_i + 60) ]
```

Temporal embedders (E2-E4) are applied as **post-retrieval boosts**, not during retrieval. This prevents temporal proximity from overriding topical relevance.

### 3. Pipeline (`pipeline`)

Three-stage retrieval for maximum precision:

1. **Stage 1 — Recall**: E13 (SPLADE) + E1 for broad candidate retrieval (~1000s)
2. **Stage 2 — Scoring**: Multi-space fusion on top candidates (~100s)
3. **Stage 3 — Rerank**: E12 (ColBERT MaxSim) for precision on final top-k

### 4. Embedder-First (`embedder_first`)

Forces retrieval through a single embedder's perspective. Useful for specialized queries:
```
search_by_embedder(embedder="E7", query="async fn handle_request")  → code-focused
search_by_embedder(embedder="E5", query="what caused the outage")   → causal-focused
```

### Weight Profiles

14 predefined profiles control how embedders are weighted during multi-space search:

| Profile | Primary Embedder(s) | Best For |
|---------|-------------------|----------|
| `semantic_search` | E1 (0.33) | General queries |
| `causal_reasoning` | E1 (0.40), E5 (0.10) | "Why" questions |
| `code_search` | E7 (0.40), E1 (0.20) | Programming queries |
| `fact_checking` | E11 (0.40), E6 (0.15) | Entity/fact validation |
| `graph_reasoning` | E8 (0.40), E11 (0.20) | Connection traversal |
| `temporal_navigation` | E2/E3/E4 (0.22 each) | Time-based queries |
| `sequence_navigation` | E4 (0.55), E1 (0.20) | Conversation flow |
| `conversation_history` | E4 (0.35), E1 (0.30) | Multi-turn reconstruction |
| `typo_tolerant` | E1 (0.30), E9 (0.15) | Misspellings |
| `category_weighted` | Constitution-compliant | Architecture-neutral |
| `pipeline_stage1_recall` | E6/E13 (0.25 each) | Sparse recall |
| `pipeline_stage2_scoring` | E1 (0.50) | Dense scoring |
| `pipeline_full` | E1 (0.40) | End-to-end pipeline |
| `balanced` | ~0.077 each | Comparison baseline |

Custom weight profiles can be created per session via `create_weight_profile`.

---

## MCP Tools Reference

### Core Memory (4 tools)

| Tool | Description |
|------|-------------|
| `store_memory` | Store a memory with content, rationale, importance, tags, and session tracking |
| `search_graph` | Multi-space semantic search with configurable strategy and weight profile |
| `get_memetic_status` | System status: fingerprint count, embedder health, storage info |
| `trigger_consolidation` | Merge similar memories using similarity, temporal, or semantic strategies |

### Memory Curation (3 tools)

| Tool | Description |
|------|-------------|
| `merge_concepts` | Merge related memories with union/intersection/weighted_average strategies |
| `forget_concept` | Soft-delete a memory (30-day recovery window) |
| `boost_importance` | Adjust memory importance score (clamped 0.0–1.0) |

### Causal Reasoning (4 tools)

| Tool | Description |
|------|-------------|
| `search_causal_relationships` | Search for causal relationships with provenance |
| `search_causes` | Abductive reasoning — find likely causes of an observed effect |
| `search_effects` | Forward causal reasoning — predict effects of a cause |
| `get_causal_chain` | Build transitive causal chains with hop attenuation |

### Causal Discovery — LLM (2 tools)

| Tool | Description |
|------|-------------|
| `trigger_causal_discovery` | Run LLM-based causal discovery (Qwen2.5-3B) |
| `get_causal_discovery_status` | Agent status, VRAM usage, statistics |

### Entity & Knowledge Graph (6 tools)

| Tool | Description |
|------|-------------|
| `extract_entities` | Extract named entities via pattern matching and knowledge base lookup |
| `search_by_entities` | Find memories by entity names with hybrid E11 ranking |
| `infer_relationship` | TransE knowledge graph relationship prediction |
| `find_related_entities` | Find entities connected via specific relationships |
| `validate_knowledge` | Score (subject, predicate, object) triples using TransE |
| `get_entity_graph` | Build and visualize entity relationship graph |

### Session & Conversation (4 tools)

| Tool | Description |
|------|-------------|
| `get_conversation_context` | Get memories around current conversation turn |
| `get_session_timeline` | Ordered timeline of session memories with sequence numbers |
| `traverse_memory_chain` | Multi-hop traversal starting from an anchor memory |
| `compare_session_states` | Compare memory state at different sequence points |

### Topic Detection (4 tools)

| Tool | Description |
|------|-------------|
| `get_topic_portfolio` | All discovered topics with profiles and stability metrics |
| `get_topic_stability` | Portfolio-level stability (churn, entropy, phase breakdown) |
| `detect_topics` | Force topic detection using HDBSCAN |
| `get_divergence_alerts` | Check for divergence from recent activity |

### Embedder-First Search (7 tools)

| Tool | Description |
|------|-------------|
| `search_by_embedder` | Search using any single embedder as primary perspective |
| `get_embedder_clusters` | Explore HDBSCAN clusters in a specific embedder space |
| `compare_embedder_views` | Side-by-side comparison of embedder rankings for a query |
| `list_embedder_indexes` | Statistics for all 13 embedder indexes |
| `get_memory_fingerprint` | Retrieve per-embedder vectors for a memory |
| `create_weight_profile` | Create session-scoped custom weight profiles |
| `search_cross_embedder_anomalies` | Find blind spots (high in one embedder, low in another) |

### Specialized Search (5 tools)

| Tool | Description |
|------|-------------|
| `search_by_keywords` | E6 sparse keyword search with term expansion |
| `search_code` | E7 code-specific search with AST context and language detection |
| `search_robust` | E9 typo-tolerant search using character trigram hypervectors |
| `search_recent` | E2 freshness-decayed search (exponential/linear/step) |
| `search_periodic` | E3 time-pattern matching (similar times of day/week) |

### Graph Navigation (6 tools)

| Tool | Description |
|------|-------------|
| `search_connections` | Find memories connected via asymmetric E8 similarity |
| `get_graph_path` | Multi-hop graph traversal with hop attenuation (0.9^hop) |
| `get_memory_neighbors` | K-NN neighbors in a specific embedder space |
| `get_typed_edges` | Explore typed edges derived from embedder agreement |
| `traverse_graph` | Multi-hop traversal following typed edges |
| `get_unified_neighbors` | Unified neighbors via Weighted RRF across all embedders |

### Graph Discovery — LLM (2 tools)

| Tool | Description |
|------|-------------|
| `discover_graph_relationships` | LLM-based relationship discovery across 20 types (4 domains) |
| `validate_graph_link` | Validate proposed graph links with confidence scoring |

### File Watcher (4 tools)

| Tool | Description |
|------|-------------|
| `list_watched_files` | List files with embeddings in the knowledge graph |
| `get_file_watcher_stats` | Statistics on watched file content |
| `delete_file_content` | Delete embeddings for a file path (soft-delete) |
| `reconcile_files` | Find and clean up orphaned file embeddings |

### Provenance & Audit (3 tools)

| Tool | Description |
|------|-------------|
| `get_audit_trail` | Query append-only audit log for memory operations |
| `get_merge_history` | Show merge lineage and history for fingerprints |
| `get_provenance_chain` | Full provenance from embedding to source |

### Maintenance (1 tool)

| Tool | Description |
|------|-------------|
| `repair_causal_relationships` | Repair corrupted causal relationship entries |

---

## Storage

### RocksDB Column Families

Context Graph uses 51 RocksDB column families organized across several layers:

**Core (11 CFs):** `nodes`, `edges`, `embeddings`, `metadata`, `temporal`, `tags`, `sources`, `system`, `embedder_edges`, `typed_edges`, `typed_edges_by_type`

**Teleological (20 CFs):** Fingerprints, topic profiles, synergy matrix, causal relationships, weight profiles, inverted indexes (E6/E13), Matryoshka truncations

**Quantized (13 CFs):** `CF_EMB_0` through `CF_EMB_12` — quantized vectors per embedder (PQ-8 or Float8)

**Code (5 CFs):** AST chunks, language indexes, symbol tables

**Causal (2 CFs):** Causal relationship metadata and indexes

### HNSW Indexing

10 of 13 embedders use HNSW (Hierarchical Navigable Small World) graphs for logarithmic-time K-NN search via [usearch](https://github.com/unum-cloud/usearch). E6 and E13 (sparse) use inverted indexes, and E12 (ColBERT) uses MaxSim token-level scoring.

HNSW graphs are persisted to RocksDB and compacted on a 10-minute background interval.

---

## Configuration

### Priority Order

1. **CLI arguments** (highest)
2. **Environment variables** (`CONTEXT_GRAPH_` prefix)
3. **Config files** (`config/default.toml`, `config/{env}.toml`)
4. **Defaults** (lowest)

### Key Environment Variables

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

### Config File Example

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
watch_paths = ["./crates"]
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
| **stdio+tcp** | Both simultaneously | stdio for Claude Code + TCP for hooks |
| **daemon** | Shared TCP server | Single server across multiple terminals |

---

## Setup

### Prerequisites

- **Rust** 1.75+ (stable)
- **CUDA** toolkit (for GPU-accelerated embeddings)
- **RocksDB** system library

### Build

```bash
# Release build
make build

# Quick check (no linking)
make check

# Run tests
make test

# Lint
make clippy
```

### Run the MCP Server

```bash
# Stdio mode (default — for Claude Code)
context-graph-mcp

# TCP mode
context-graph-mcp --transport tcp --port 3100

# Daemon mode (shared server, load models once)
context-graph-mcp --daemon

# Fast startup (models load in background)
context-graph-mcp --no-warm
```

### Claude Code Integration

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

### CLI & Hooks

The CLI provides Claude Code hooks for automatic memory capture:

```bash
# Set up hooks for Claude Code
context-graph-cli setup

# Manual operations
context-graph-cli memory capture --content "learned something" --rationale "important pattern"
context-graph-cli memory inject --query "authentication patterns"
context-graph-cli topic portfolio
context-graph-cli warmup   # Pre-load embeddings into VRAM
```

**Hook timeouts:**

| Hook | Timeout | Trigger |
|------|---------|---------|
| `session-start` | 5s | Session begins |
| `pre-tool-use` | 500ms | Before each tool call |
| `post-tool-use` | 3s | After each tool call |
| `user-prompt-submit` | 2s | User sends a message |
| `pre-compact` | 20s | Before context compaction |
| `task-completed` | 20s | Task finishes |
| `session-end` | 30s | Session ends |

---

## How It Works — Example Flow

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
4. **RRF fusion** — rankings merged with causal weights: `0.40/(rank_E1+60) + 0.10/(rank_E5+60) + ...`
5. **Post-retrieval boosts** — E2 freshness decay prioritizes recent memories
6. **Causal gate** — scores above 0.04 threshold get 1.10x boost, below 0.008 get 0.85x demotion
7. **Return** — top-k results with per-embedder breakdown and provenance

---

## Graceful Degradation

- **LLM unavailable**: 52 of 55 tools work normally. Only `trigger_causal_discovery`, `discover_graph_relationships`, and `validate_graph_link` return errors.
- **Embedder failure**: LazyMultiArrayProvider handles loading failures. Search falls back to available embedders with degraded tracking.
- **Soft-delete**: All deletions are soft with a 30-day recovery window. A background GC task runs every 5 minutes.

---

## Performance Targets

| Operation | Target |
|-----------|--------|
| `store_memory` | < 5ms p95 |
| `get_node` | < 1ms p95 |
| Context injection | < 25ms p95, < 50ms p99 |
| Embedding validation | < 1ms |
| Health check | < 1ms |
| HNSW K-NN search | O(log n) |

---

## Project Structure

```
contextgraph/
├── crates/
│   ├── context-graph-mcp/          # MCP server
│   │   ├── src/
│   │   │   ├── main.rs             # Entry point, CLI args
│   │   │   ├── server/             # Server core, transport
│   │   │   ├── handlers/           # Tool dispatch and handlers
│   │   │   └── tools/              # Tool definitions and schemas
│   ├── context-graph-core/         # Domain types, config, traits
│   │   ├── src/
│   │   │   ├── config/             # Configuration system
│   │   │   ├── embeddings/         # Embedder config and categories
│   │   │   ├── teleological/       # 13-embedder fingerprints
│   │   │   ├── weights/            # 14 weight profiles
│   │   │   └── traits/             # Store traits and search options
│   ├── context-graph-storage/      # RocksDB persistence
│   │   ├── src/
│   │   │   ├── column_families.rs  # 51 column family definitions
│   │   │   └── teleological/       # HNSW indexes, quantization
│   ├── context-graph-embeddings/   # 13-model embedding pipeline
│   ├── context-graph-graph/        # Knowledge graph
│   ├── context-graph-cuda/         # GPU acceleration
│   ├── context-graph-cli/          # CLI and Claude Code hooks
│   ├── context-graph-causal-agent/ # LLM causal discovery
│   ├── context-graph-graph-agent/  # LLM graph discovery
│   ├── context-graph-benchmark/    # Benchmarking suite
│   └── context-graph-test-utils/   # Shared test helpers
├── config/                         # Configuration files
├── scripts/                        # Build and maintenance scripts
└── Makefile                        # Build targets
```

---

## Protocol

- **MCP Version**: 2024-11-05
- **Message Format**: Newline-delimited JSON (NDJSON)
- **RPC**: JSON-RPC 2.0
- **Serialization**: JSON for all provenance and metadata (bincode has known issues with `skip_serializing_if`)

## License

All rights reserved.
