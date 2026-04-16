# Configuration

Source files covered:

- `crates/context-graph-core/src/config/mod.rs`
- `crates/context-graph-core/src/config/constants.rs`
- `crates/context-graph-core/src/config/embedder_config.rs`
- `crates/context-graph-core/src/config/sub_configs.rs`
- `crates/context-graph-core/src/config/tests/`
- `crates/context-graph-core/src/weights/mod.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`
- `crates/context-graph-core/src/traits/teleological_memory_store/options.rs`
- `crates/context-graph-core/src/teleological/matrix_search/config.rs`
- `crates/context-graph-core/src/teleological/matrix_search/weights.rs`
- `crates/context-graph-core/src/teleological/matrix_search/types.rs`
- `crates/context-graph-mcp/src/main.rs`
- `crates/context-graph-mcp/src/server/mod.rs`
- `crates/context-graph-mcp/src/server/transport.rs`
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs`
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs`
- `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/causal_hnsw_index.rs`
- `crates/context-graph-storage/src/graph_edges/builder.rs`
- `crates/context-graph-embeddings/src/config/gpu.rs`
- `crates/context-graph-embeddings/src/models/registry/config.rs`
- `crates/context-graph-causal-agent/src/service/mod.rs`
- `config/default.toml`
- `config/development.toml`
- `config/production.toml`
- `config/test.toml`

---

## 1. Config File Location and Format

Configuration uses TOML format. Config files live in the `config/` directory at the project root:

| File | Purpose |
|------|---------|
| `config/default.toml` | Base settings loaded first |
| `config/development.toml` | Development overrides (loaded when `CONTEXT_GRAPH_ENV=development`) |
| `config/production.toml` | Production overrides (loaded when `CONTEXT_GRAPH_ENV=production`) |
| `config/test.toml` | Test overrides (loaded when `CONTEXT_GRAPH_ENV=test`) |

The main config struct is `Config` in `crates/context-graph-core/src/config/mod.rs`. Configuration can also be loaded from an arbitrary file path using `Config::from_file(path)`.

---

## 2. Config Loading and Merge Order

Configuration is loaded and merged in a strict priority order. Later sources override earlier ones.

### 2.1 Load Order

1. **`config/default.toml`** -- Base settings (required: false, missing is not an error)
2. **`config/{CONTEXT_GRAPH_ENV}.toml`** -- Environment-specific overrides (required: false)
3. **Environment variables** with `CONTEXT_GRAPH_` prefix (separator: `__` for nested keys)
4. **CLI argument overrides** -- Applied in `apply_overrides()` after config load

The `CONTEXT_GRAPH_ENV` variable defaults to `"development"` if not set.

### 2.2 CLI Argument Priority

For the MCP server binary, the override priority is:

```
CLI arguments > Environment variables > Config file > Struct defaults
```

This is explicitly documented and enforced in `determine_transport_mode()` and `apply_overrides()` in `crates/context-graph-mcp/src/main.rs`.

### 2.3 Environment Variable Nesting

Nested config keys use double-underscore (`__`) as separator. For example:

```bash
CONTEXT_GRAPH_MCP__TRANSPORT=tcp        # sets config.mcp.transport
CONTEXT_GRAPH_STORAGE__BACKEND=rocksdb  # sets config.storage.backend
```

### 2.4 Automatic Backend Promotion

When certain environment variables are set, `apply_overrides()` automatically promotes stub backends to real implementations:

| Trigger | Automatic Change |
|---------|-----------------|
| `CONTEXT_GRAPH_STORAGE_PATH` is set | `storage.backend` changes from `"memory"` to `"rocksdb"` |
| `CONTEXT_GRAPH_MODELS_PATH` is set | `embedding.model` changes from `"stub"` to `"e5-large-v2"` |
| `storage.backend` becomes `"rocksdb"` | `index.backend` changes from `"memory"` to `"hnsw"` |
| `storage.backend` becomes `"rocksdb"` | `utl.mode` changes from `"stub"` to `"production"` |

---

## 3. Top-Level Config Structure

The root `Config` struct contains the following sections:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `phase` | `Phase` enum | `Ghost` | System development phase |
| `server` | `ServerConfig` | see Section 4 | Server identity |
| `mcp` | `McpConfig` | see Section 5 | MCP transport and protocol |
| `logging` | `LoggingConfig` | see Section 6 | Logging behavior |
| `storage` | `StorageConfig` | see Section 7 | Storage backend |
| `embedding` | `EmbeddingConfig` | see Section 8 | Embedding model selection |
| `index` | `IndexConfig` | see Section 9 | HNSW index parameters |
| `utl` | `UtlConfig` | see Section 10 | Unified Theory of Learning |
| `cuda` | `CudaConfig` | see Section 11 | GPU device selection |
| `watcher` | `WatcherConfig` | see Section 12 | File watcher |

### 3.1 Phase

| Value | Description |
|-------|-------------|
| `ghost` | Scaffolding phase. Stubs allowed. Warnings printed to stderr. |
| `development` | Active implementation. Stubs allowed with warnings. |
| `production` | Fully operational. Stubs cause startup failure. |

Default: `ghost`

---

## 4. Server Configuration (`[server]`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `name` | String | `"context-graph"` | Any string | Server name for identification |
| `version` | String | `"0.1.0-ghost"` | Any string | Server version string |

---

## 5. MCP Configuration (`[mcp]`)

| Key | Type | Default | Valid Range | Description |
|-----|------|---------|-------------|-------------|
| `transport` | String | `"stdio"` | `"stdio"`, `"tcp"`, `"sse"` | Transport protocol. Note: `"sse"` passes config validation but the MCP server binary rejects it at runtime with a fatal error. Only `"stdio"` and `"tcp"` are supported operationally. |
| `max_payload_size` | usize | `10485760` (10 MB) | > 0 | Maximum JSON-RPC payload size in bytes |
| `request_timeout` | u64 | `30` | > 0 | Per-request timeout in seconds |
| `bind_address` | String | `"127.0.0.1"` | Non-empty when transport is `"tcp"` or `"sse"` | TCP/SSE bind address |
| `tcp_port` | u16 | `3100` | 1-65535 | TCP transport port |
| `sse_port` | u16 | `3101` | 1-65535 | SSE transport port |
| `max_connections` | usize | `32` | > 0 when transport is `"tcp"` or `"sse"` | Maximum concurrent connections |

### 5.1 MCP Validation Rules

Validation runs in `McpConfig::validate()`:

- `transport` must be one of `"stdio"`, `"tcp"`, `"sse"` (case-insensitive)
- `max_payload_size` must be > 0
- `request_timeout` must be > 0
- When `transport = "tcp"`:
  - `bind_address` must be non-empty
  - `tcp_port` must be 1-65535
  - `max_connections` must be > 0
- When `transport = "sse"`:
  - `bind_address` must be non-empty
  - `sse_port` must be 1-65535
  - `max_connections` must be > 0

### 5.2 TCP Transport Limits

Defined in `crates/context-graph-mcp/src/server/transport.rs`:

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_LINE_BYTES` | `10485760` (10 MB) | Maximum size of a single NDJSON line. Lines exceeding this are rejected and drained. |
| `MAX_BATCH_SIZE` | `100` | Maximum requests in a JSON-RPC 2.0 batch array. Prevents DoS via oversized batches. |

---

## 6. Logging Configuration (`[logging]`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `level` | String | `"info"` | `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"` | Log level filter |
| `format` | String | `"pretty"` | `"pretty"`, `"json"`, `"compact"` | Output format |
| `include_location` | bool | `false` | `true`, `false` | Include file/line in log output |

Note: The `RUST_LOG` environment variable also controls log level at runtime and takes precedence through the `tracing_subscriber` `EnvFilter`.

---

## 7. Storage Configuration (`[storage]`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `backend` | String | `"memory"` | `"memory"`, `"rocksdb"` | Storage backend. `"memory"` is ephemeral (data lost on restart). `"rocksdb"` provides persistent storage. |
| `path` | String | `"./data/storage"` | Valid filesystem path | RocksDB database directory. Parent directory must exist when backend is not `"memory"`. |
| `compression` | bool | `true` | `true`, `false` | Enable RocksDB compression |

### 7.1 Storage Validation

- When `backend` is not `"memory"`, the parent directory of `path` must exist on disk.
- In `Production` phase, `backend = "memory"` causes a startup failure.

---

## 8. Embedding Configuration (`[embedding]`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `model` | String | `"stub"` | `"stub"`, `"e5-large-v2"`, `"multi_array_13"` | Embedding model. `"stub"` returns deterministic fake vectors. Real models require `CONTEXT_GRAPH_MODELS_PATH`. |

In `Production` phase, `model = "stub"` causes a startup failure.

---

## 9. HNSW Index Configuration (`[index]`)

### 9.1 Core Config (`IndexConfig`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `backend` | String | `"memory"` | `"memory"`, `"hnsw"` | Index backend. `"memory"` is ephemeral. `"hnsw"` provides persistent vector search. |
| `hnsw_m` | usize | `16` | >= 2 | HNSW connectivity parameter M (bi-directional links per node). Higher = better recall, more memory. |
| `hnsw_ef_construction` | usize | `200` | >= M | Dynamic candidate list size during index construction. Higher = better quality, slower build. |

In `Production` phase, `backend = "memory"` causes a startup failure.

### 9.2 Per-Embedder HNSW Configuration

The `HnswConfig` struct in `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs` defines per-index HNSW parameters:

| Parameter | Default | Matryoshka 128D | Causal E11 Index | Description |
|-----------|---------|-----------------|------------------|-------------|
| `m` | 16 | 32 | 16 | Connectivity parameter |
| `ef_construction` | 200 | 256 | 200 | Construction expansion factor |
| `ef_search` | 100 | 128 | 100 | Search expansion factor |
| `metric` | Per-embedder | Cosine | Cosine | Distance metric |
| `dimension` | Per-embedder | 128 | 768 | Vector dimensionality |

**Default factory method**: `HnswConfig::default_for_dimension(dim, metric)` creates `m=16, ef_construction=200, ef_search=100` for any given dimension and metric.

### 9.3 HNSW Validation Rules

Enforced in `HnswConfig::new()` (panics on violation):

- `m >= 2`
- `ef_construction >= m`
- `ef_search >= 1`
- `dimension >= 1`

### 9.4 HNSW Capacity Limits

| Constant | Value | Source File | Description |
|----------|-------|-------------|-------------|
| `MAX_HNSW_VECTORS_PER_INDEX` | `2,000,000` | `hnsw_impl/types.rs` | Maximum vectors per HNSW index. At 1024D x 4B = 4 KB/vector, 2M vectors uses ~8 GB raw + ~16 GB graph overhead. |
| `INITIAL_CAPACITY` | `1,024` | `causal_hnsw_index.rs` | Initial reserved capacity for the causal E11 HNSW index. Grows by doubling when exceeded. |

### 9.5 HNSW Index Map (15 Indexes)

Each HNSW-enabled embedder has its own index. The full set of 15 HNSW indexes:

| Index | Embedder | Dimension | Metric | Notes |
|-------|----------|-----------|--------|-------|
| E1 | E1_Semantic | 1024 | Cosine | Primary semantic |
| E1 Matryoshka | E1_Matryoshka_128 | 128 | Cosine | Stage 2 fast filtering (M=32, ef_c=256, ef_s=128) |
| E2 | E2_Temporal_Recent | 512 | Cosine | Sinusoidal PE of absolute Unix timestamp (production) |
| E3 | E3_Temporal_Periodic | 512 | Cosine | Fourier periodic encoding (cyclic calendar patterns) |
| E4 | E4_Temporal_Positional | 512 | Cosine | Sinusoidal PE of session turn index |
| E5 | E5_Causal (active vector) | 768 | AsymmetricCosine | Legacy active vector |
| E5 Cause | E5_CausalCause | 768 | AsymmetricCosine | Cause direction |
| E5 Effect | E5_CausalEffect | 768 | AsymmetricCosine | Effect direction |
| E7 | E7_Code | 1536 | Cosine | Code similarity |
| E8 | E8_Graph | 1024 | Cosine | Structural |
| E9 | E9_HDC | 1024 | Cosine | Hyperdimensional |
| E10 | E10_Multimodal (active) | 768 | AsymmetricCosine | Legacy active vector |
| E10 Paraphrase | E10_MultimodalParaphrase | 768 | AsymmetricCosine | Paraphrase direction |
| E10 Context | E10_MultimodalContext | 768 | AsymmetricCosine | Context direction |
| E11 | E11_Entity | 768 | Cosine | Entity (currently disabled) |

The following embedders do NOT use HNSW:

- **E6** (Sparse Lexical) -- uses inverted index
- **E12** (Late Interaction) -- uses MaxSim token-level computation
- **E13** (SPLADE) -- uses inverted index

---

## 10. UTL Configuration (`[utl]`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `mode` | String | `"stub"` | `"stub"`, `"real"`, `"production"` | UTL computation mode. `"stub"` returns fake learning/consolidation scores. |
| `consolidation_threshold` | f32 | `0.7` | 0.0 - 1.0 | Threshold for memory consolidation |

In `Production` phase, `mode = "stub"` causes a startup failure.

---

## 11. CUDA / GPU Configuration

### 11.1 Core Config (`CudaConfig` in `[cuda]`)

Defined in `crates/context-graph-core/src/config/sub_configs.rs`:

| Key | Type | Default | Valid Range | Description |
|-----|------|---------|-------------|-------------|
| `device_id` | u32 | `0` | 0+ | Target CUDA device ID |
| `memory_limit_gb` | f32 | `4.0` | > 0 | GPU memory limit in GB |

### 11.2 Embeddings GPU Config (`GpuConfig`)

Defined in `crates/context-graph-embeddings/src/config/gpu.rs`. This is the GPU config used by the embedding model system (target: RTX 5090, 32GB GDDR7):

| Key | Type | Default | Valid Range | Description |
|-----|------|---------|-------------|-------------|
| `enabled` | bool | `true` | -- | Whether GPU acceleration is enabled |
| `device_ids` | Vec\<u32\> | `[0]` | Non-empty when enabled | CUDA device IDs to use |
| `memory_fraction` | f32 | `0.9` | (0.0, 1.0] | Fraction of GPU memory to use. Constitution spec: <24GB of 32GB. |
| `use_cuda_graphs` | bool | `true` | -- | CUDA kernel fusion for reduced launch overhead |
| `mixed_precision` | bool | `true` | -- | FP16/BF16 inference for 2x throughput |
| `green_contexts` | bool | `false` | -- | CUDA 13.1 green contexts (static SM partitioning). Requires Blackwell architecture. |
| `gds_enabled` | bool | `false` | -- | GPU Direct Storage for 25+ GB/s model loading (requires GDS driver + NVMe SSD) |

**GpuConfig Validation** (`GpuConfig::validate()`):

- When `enabled = true`, `device_ids` must not be empty
- `memory_fraction` must be in `(0.0, 1.0]` and must not be NaN

### 11.3 Model Registry Config (`ModelRegistryConfig`)

Defined in `crates/context-graph-embeddings/src/models/registry/config.rs`:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_concurrent_loads` | usize | `4` | Maximum concurrent model loads |
| `memory_budget_bytes` | usize | `32,000,000,000` (32 GB) | Total VRAM budget |
| `preload_models` | Vec\<ModelId\> | `[]` | Models to preload at startup |
| `enable_debug_logging` | bool | `false` | Debug logging for model operations |

---

## 12. File Watcher Configuration (`[watcher]`)

| Key | Type | Default | Valid Values | Description |
|-----|------|---------|--------------|-------------|
| `enabled` | bool | `false` | `true`, `false` | Enable file watching. Also controllable via `CONTEXT_GRAPH_WATCHER_ENABLED`. |
| `watch_paths` | Vec\<String\> | `["./docs"]` | Directory paths | Directories to watch recursively for file changes |
| `session_id` | String | `"docs-watcher"` | Any string | Session ID for auto-indexed memories |

### 12.1 Code Watcher (`[watcher.code]`)

Defined in `config/default.toml` (not in the struct defaults):

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable code file watching |
| `watch_paths` | Vec\<String\> | `["./crates"]` | Directories for code files |
| `session_id` | String | `"code-watcher"` | Session ID for code memories |
| `extensions` | Vec\<String\> | `["rs"]` | File extensions to watch |
| `use_ast_chunker` | bool | `true` | AST-based chunking (vs line-based) |
| `target_chunk_size` | usize | `500` | Target chunk size in non-whitespace characters |
| `min_chunk_size` | usize | `100` | Minimum chunk size before merging with siblings |
| `max_chunk_size` | usize | `1000` | Maximum chunk size before recursive splitting |

---

## 13. Environment Variables

### 13.1 Core Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CONTEXT_GRAPH_ENV` | `"development"` | Selects environment-specific config file (`config/{value}.toml`) |
| `CONTEXT_GRAPH_TRANSPORT` | from config | Transport mode override (`stdio` or `tcp`) |
| `CONTEXT_GRAPH_TCP_PORT` | from config | TCP port override |
| `CONTEXT_GRAPH_BIND_ADDRESS` | from config | TCP bind address override |
| `CONTEXT_GRAPH_STORAGE_PATH` | none | Storage directory path. When set, auto-promotes `storage.backend` to `"rocksdb"`. |
| `CONTEXT_GRAPH_MODELS_PATH` | none | Path to downloaded model files. When set, auto-promotes `embedding.model` to `"e5-large-v2"`. |
| `CONTEXT_GRAPH_WATCHER_ENABLED` | `"false"` | Enable file watcher (`"1"` or `"true"` to enable) |
| `CONTEXT_GRAPH_MCP_QUIET` | unset | When set (any value), suppresses phase safety warnings to stderr. Used for clean MCP stdio protocol. |
| `CONTEXT_GRAPH_WARM_FIRST` | `"1"` | `"0"` or `"false"` disables blocking model warmup at startup |
| `CONTEXT_GRAPH_DAEMON` | `"0"` | `"1"` or `"true"` enables daemon mode for shared MCP server |
| `CONTEXT_GRAPH_DAEMON_PORT` | `3100` | Port for daemon-mode TCP listener |
| `RUST_LOG` | none | Log level override via `tracing_subscriber::EnvFilter` |

### 13.2 CLI-Only Environment Variables

Used by the CLI tools in `crates/context-graph-cli/`:

| Variable | Default | Description |
|----------|---------|-------------|
| `CONTEXT_GRAPH_MCP_HOST` | `"127.0.0.1"` | MCP server hostname for CLI client connection |
| `CONTEXT_GRAPH_MCP_PORT` | `3100` | MCP server TCP port for CLI client connection |
| `CONTEXT_GRAPH_DB_PATH` | none | Database path for CLI session persist/restore and hook operations |
| `CONTEXT_GRAPH_DATA_DIR` | none | Data directory for CLI watch command |
| `CONTEXT_GRAPH_CLI` | none | Path to CLI binary (used in e2e test helpers) |
| `CLAUDE_SESSION_ID` | none | Session ID for MCP operations. Used as fallback when `sessionId` is not passed as a parameter. |

### 13.3 Causal Discovery Environment Variables

Used by the causal discovery background service in `crates/context-graph-causal-agent/`:

| Variable | Default | Type | Description |
|----------|---------|------|-------------|
| `CAUSAL_DISCOVERY_ENABLED` | `"false"` | `"true"` or `"1"` | Enable background causal discovery loop |
| `CAUSAL_DISCOVERY_INTERVAL_SECS` | `120` | u64 | Interval between discovery cycles in seconds |
| `CAUSAL_DISCOVERY_BATCH_SIZE` | `100` | usize | Maximum memory pairs to analyze per cycle |
| `CAUSAL_DISCOVERY_MIN_CONFIDENCE` | `0.7` | f32 | Minimum LLM confidence threshold for accepting a relationship |

### 13.4 Environment Variable to Config Mapping (Nested Separator)

The config library uses `CONTEXT_GRAPH_` prefix with `__` (double underscore) as the nesting separator:

```bash
CONTEXT_GRAPH_SERVER__NAME="my-server"     # -> config.server.name
CONTEXT_GRAPH_LOGGING__LEVEL="debug"       # -> config.logging.level
CONTEXT_GRAPH_CUDA__DEVICE_ID=1            # -> config.cuda.device_id
```

---

## 14. Phase-Aware Validation

Validation runs automatically in `Config::validate()` and `Config::from_file()`. The `validate_phase_safety()` method enforces phase-specific rules.

### 14.1 Dangerous Configurations Detected

| Config | Danger |
|--------|--------|
| `embedding.model = "stub"` | Fake embeddings; all similarity computations are meaningless |
| `storage.backend = "memory"` | All data lost on restart; no persistence or durability |
| `index.backend = "memory"` | HNSW indexes are ephemeral; no persistent vector search |
| `utl.mode = "stub"` | Fake UTL processing; learning scores and consolidation are meaningless |

### 14.2 Phase Behavior

| Phase | Behavior with Dangerous Configs |
|-------|-------------------------------|
| `Ghost` | Allowed. Warning printed to stderr (unless `CONTEXT_GRAPH_MCP_QUIET` is set). |
| `Development` | Allowed. Warning printed to stderr (unless `CONTEXT_GRAPH_MCP_QUIET` is set). |
| `Production` | **FAILS HARD**. Returns `CoreError::ConfigError` with remediation steps. Server refuses to start. |

### 14.3 Helper Methods

| Method | Returns |
|--------|---------|
| `Config::uses_stubs()` | `true` if any stub configuration is detected |
| `Config::is_production_safe()` | `true` only if `phase == Production` AND no stubs |

---

## 15. Embedder-Specific Configuration

### 15.1 Embedder Dimensions

| Embedder | Canonical Name | Model | Dimension | Index Type |
|----------|---------------|-------|-----------|------------|
| E1 | `E1_Semantic` | e5-large-v2 | 1024 | HNSW (Cosine) |
| E2 | `E2_Temporal_Recent` | Sinusoidal PE of Unix timestamp (production) / legacy multi-scale decay (tests) via `compute_decay_embedding` in `crates/context-graph-embeddings/src/models/custom/temporal_recent/compute.rs` | 512 | HNSW (Cosine) |
| E3 | `E3_Temporal_Periodic` | Fourier periodic encoding (day, week, month, year cycles) | 512 | HNSW (Cosine) |
| E4 | `E4_Temporal_Positional` | Sinusoidal PE of session position / turn index | 512 | HNSW (Cosine) |
| E5 | `E5_Causal` | Longformer SCM (asymmetric cause/effect vectors) | 768 | HNSW (AsymmetricCosine) |
| E6 | `E6_Sparse_Lexical` | Sparse BERT-vocab keyword frequency (legacy sparse slot) | 30,522 vocab, max 1,500 non-zeros | Inverted index (no BM25) |
| E7 | `E7_Code` | Qodo-Embed-1-1.5B | 1536 | HNSW (Cosine) |
| E8 | `E8_Graph` | e5-large-v2 (structure/graph view) | 1024 | HNSW (Cosine) |
| E9 | `E9_HDC` | HDC projected from 10K-bit hypervector | 1024 | HNSW (Cosine) |
| E10 | `E10_Multimodal` | CLIP (paraphrase/context asymmetric) | 768 | HNSW (AsymmetricCosine) |
| E11 | `E11_Entity` | KEPLER (RoBERTa-base + TransE) | 768 | HNSW (Cosine) -- **disabled by default** (`E11_ENTITY_ENABLED = false`) |
| E12 | `E12_Late_Interaction` | ColBERT token-level late interaction | 128/token | MaxSim (no HNSW) |
| E13 | `E13_SPLADE` | SPLADE v3 (BM25 + sparse expansion) | 30,522 vocab, max 1,500 non-zeros | Inverted index (BM25) |

### 15.2 Embedder Name Resolution

The `EmbedderConfig` system supports multiple name formats via `Embedder::from_name()`:

| Format | Example | Result |
|--------|---------|--------|
| Canonical | `E8_Graph` | `Embedder::Graph` |
| Short | `E8` | `Embedder::Graph` |
| Lowercase | `graph` | `Embedder::Graph` |

Deprecated name `E8_Emotional` is no longer accepted and fails deserialization.

### 15.3 EmbedderConfig Struct (Per-Embedder)

```toml
[[embedders.configs]]
embedder = "E8_Graph"
weight = 0.5
enabled = false
model_path = "/path/to/custom/model"
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `embedder` | String | `"E1_Semantic"` | Embedder name (resolved via `Embedder::from_name()`) |
| `weight` | f32 | `1.0` | Fusion weight [0.0, 1.0] |
| `enabled` | bool | `true` | Whether this embedder is enabled |
| `model_path` | Option\<String\> | `None` | Custom model path override |

### 15.4 EmbedderWeightsConfig (Weight Map)

Two TOML formats are supported:

**Map format:**

```toml
[embedders.weights]
E1_Semantic = 1.0
E8_Graph = 0.5
```

**List format:**

```toml
[[embedders.configs]]
embedder = "E1_Semantic"
weight = 1.0

[[embedders.configs]]
embedder = "E8_Graph"
weight = 0.5
```

Weight resolution priority: detailed config > weight map > 0.0 (unknown embedders excluded from scoring).

### 15.5 Embedder Enable Flags

| Constant | Value | Source | Description |
|----------|-------|--------|-------------|
| `TEMPORAL_EMBEDDERS_ENABLED` | `true` | `weights/mod.rs` | E2/E3/E4 generate real temporal vectors during store |
| `E11_ENTITY_ENABLED` | `false` | `weights/mod.rs` | E11 KEPLER is disabled (near-identical vectors for all inputs). When false, `get_effective_weight_profile()` redistributes E11 weight to remaining embedders. |

### 15.6 Model Subdirectory Layout

When `CONTEXT_GRAPH_MODELS_PATH` is set, the embedding factory looks for model files in subdirectories:

| Model ID | Subdirectory |
|----------|-------------|
| Semantic (E1) | `semantic/` |
| Causal (E5) | `causal/` |
| Sparse (E6) | `sparse/` |
| Code (E7) | `code-1536/` |
| Graph (E8) | `graph/` |
| Contextual (E10) | `contextual/` |
| Entity (E11) | `entity/` |
| Kepler | `kepler/` |
| LateInteraction (E12) | `late-interaction/` |
| Splade (E13) | `splade-v3/` |
| TemporalRecent (E2) | None (custom, no model files) |
| TemporalPeriodic (E3) | None (custom, no model files) |
| TemporalPositional (E4) | None (custom, no model files) |
| HDC (E9) | None (custom, no model files) |

---

## 16. Search Strategy Configuration

### 16.1 MCP Search Strategies

Used in `TeleologicalSearchOptions` (the search API exposed via MCP tools):

| Strategy | Description |
|----------|-------------|
| `e1_only` | E1 HNSW only. Fastest. Backward compatible. Default. |
| `multi_space` | Weighted fusion of semantic embedders (E1, E5, E7, E8, E10, E11). E2-E4 have weight 0.0 by default. |
| `pipeline` | 3-stage pipeline: E13 SPLADE + E1 recall, multi-space scoring, optional E12 ColBERT re-ranking. |

### 16.2 Matrix Search Strategies

Used in `MatrixSearchConfig` for teleological vector comparison:

| Strategy | Description |
|----------|-------------|
| `Cosine` | Standard cosine similarity across all components. Default. |
| `Euclidean` | Euclidean distance inverted to similarity. |
| `SynergyWeighted` | Synergy matrix modulates importance of each pair. |
| `GroupHierarchical` | Compare at group level only (6D). |
| `CrossCorrelationDominant` | Prioritize cross-correlation patterns (78D). |
| `TuckerCompressed` | Tucker core compressed comparison. |
| `Adaptive` | Auto-select best strategy based on vector characteristics. |

### 16.3 Matrix Search Config Defaults

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `strategy` | SearchStrategy | `Cosine` | Comparison strategy |
| `scope` | ComparisonScope | `Full` | Which components to compare |
| `weights` | ComponentWeights | see below | Component weights for Full scope |
| `synergy_matrix` | Option\<SynergyMatrix\> | `None` | Optional synergy matrix |
| `min_similarity` | f32 | `0.0` | Minimum result similarity threshold |
| `max_results` | usize | `100` | Maximum number of results |
| `compute_breakdown` | bool | `false` | Include per-component breakdown |

### 16.4 Normalization Strategies

| Strategy | Description |
|----------|-------------|
| `none` | No normalization; use raw similarity scores |
| `min_max` | Min-max normalization to [0, 1]. Default. |
| `z_score` | Z-score normalization (mean=0, std=1), scaled to [0, 1] |
| `convex` | Convex combination (research-backed best practice) |

### 16.5 Decay Functions (Temporal Post-Retrieval)

| Function | Formula | Description |
|----------|---------|-------------|
| `linear` | `1.0 - (age / max_age)` | Simple predictable decay. Default. |
| `exponential` | `exp(-age * 0.693 / half_life)` | Natural forgetting curve. |
| `step` | Bucketed: <5m=1.0, <1h=0.8, <1d=0.5, >=1d=0.1 | Time-bucket step function. |
| `no_decay` | `1.0` | All memories have equal recency. |

---

## 17. Weight Profile Configuration

### 17.1 Predefined Weight Profiles

Each profile is a 13-element array `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]`. Weights sum to approximately 1.0.

| Profile Name | Primary Embedder | E2/E3/E4 | E12/E13 | Use Case |
|-------------|-----------------|----------|---------|----------|
| `semantic_search` | E1 (0.33) | 0.0 | 0.0 | General semantic queries |
| `causal_reasoning` | E1 (0.40) | 0.0 | 0.0 | "Why" questions. E5 demoted to 0.10. |
| `code_search` | E7 (0.40) | 0.0 | 0.0 | Programming queries |
| `fact_checking` | E11 (0.40) | 0.0 | 0.0 | Entity/fact queries |
| `graph_reasoning` | E8 (0.40) | 0.0 | 0.0 | Structural/connectivity queries |
| `temporal_navigation` | E2/E3/E4 (0.23 each) | 0.23 | 0.0 | Explicit time-based queries |
| `sequence_navigation` | E4 (0.55) | E2=0.05 | 0.0 | Explicit sequence traversal |
| `conversation_history` | E4 (0.35) + E1 (0.30) | E2=0.05 | 0.0 | Contextual conversation recall |
| `category_weighted` | Constitution-compliant | 0.0 | 0.0 | Semantic=1.0, Relational=0.5, Structural=0.5 weights |
| `typo_tolerant` | E9 (0.18) | 0.0 | 0.0 | Queries with potential spelling errors |
| `pipeline_stage1_recall` | E13 (0.25) + E6 (0.25) | 0.0 | E13=0.25 | Sparse retrieval recall stage |
| `pipeline_stage2_scoring` | E1 (0.50) | 0.0 | 0.0 | Dense candidate scoring stage |
| `pipeline_full` | E1 (0.40) | 0.0 | E13=0.07 | Complete pipeline execution |
| `balanced` | Equal (0.077 each) | 0.077 | 0.077 | Testing/comparison baseline |

### 17.2 E12/E13 Weight Rule

Per architecture document ARCH-13:

- **E12** (Late Interaction / ColBERT) has weight `0.0` in all scoring profiles. It is used only for Stage 3 MaxSim re-ranking.
- **E13** (SPLADE) has weight `0.0` in all scoring profiles except `pipeline_stage1_recall` and `pipeline_full`. It is used primarily for Stage 1 inverted-index recall.

### 17.3 E11 Disable Redistribution

When `E11_ENTITY_ENABLED = false` (the current default), calling `get_effective_weight_profile(name)` zeroes out E11's weight and redistributes it proportionally to remaining active embedders so the total still sums to approximately 1.0.

### 17.4 ComponentWeights (Teleological Matrix)

For teleological vector comparison (not MCP search), the `ComponentWeights` struct controls how comparison components are weighted:

| Key | Default | Correlation-Focused | Topic-Focused | Group-Focused | Valid Range |
|-----|---------|-------------------|--------------|--------------|-------------|
| `topic_profile` | 0.4 | 0.25 | 0.6 | 0.25 | [0.0, 1.0] |
| `cross_correlations` | 0.35 | 0.55 | 0.2 | 0.25 | [0.0, 1.0] |
| `group_alignments` | 0.15 | 0.15 | 0.15 | 0.45 | [0.0, 1.0] |
| `confidence` | 0.1 | 0.05 | 0.05 | 0.05 | [0.0, 1.0] |

Validation requires all weights in [0.0, 1.0] and sum within 0.001 of 1.0.

---

## 18. Graph Builder Configuration

The `GraphBuilderConfig` in `crates/context-graph-storage/src/graph_edges/builder.rs` controls the background K-NN graph linking worker:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `batch_interval_secs` | u64 | `60` | Interval between batch processing cycles |
| `max_batch_size` | usize | `1000` | Maximum fingerprints to process in one batch |
| `min_batch_size` | usize | `10` | Minimum batch size before processing starts |
| `active_embedders` | Vec\<u8\> | `[0, 4, 6, 7, 9, 10]` | Embedder indices for K-NN graph building: E1, E5, E7, E8, E10, E11 |
| `k` | usize | `20` | K neighbors per node in K-NN graph |
| `min_weighted_agreement` | f32 | `2.5` | Minimum weighted agreement for creating typed edges (same as topic threshold per ARCH-09) |
| `process_on_max` | bool | `true` | Process immediately when `max_batch_size` is reached |

---

## 19. Causal Discovery Configuration

The `CausalDiscoveryConfig` in `crates/context-graph-causal-agent/src/service/mod.rs` controls the LLM-based background causal discovery service:

| Key | Type | Default | Env Override | Description |
|-----|------|---------|-------------|-------------|
| `interval` | Duration | 120s (2 min) | `CAUSAL_DISCOVERY_INTERVAL_SECS` | Interval between discovery cycles |
| `batch_size` | usize | `100` | `CAUSAL_DISCOVERY_BATCH_SIZE` | Max memory pairs per cycle |
| `min_confidence` | f32 | `0.7` | `CAUSAL_DISCOVERY_MIN_CONFIDENCE` | Minimum LLM confidence threshold |
| `skip_analyzed` | bool | `true` | -- | Skip already-analyzed pairs |
| `enable_background` | bool | `false` | `CAUSAL_DISCOVERY_ENABLED` | Explicit opt-in for background loop |
| `min_interval` | Duration | 30s | -- | Adaptive floor for interval |
| `max_interval` | Duration | 600s (10 min) | -- | Adaptive ceiling for interval |
| `max_consecutive_errors` | u32 | `3` | -- | Max consecutive errors before long pause |

---

## 20. Constants from Constitution

Defined in `crates/context-graph-core/src/config/constants.rs`:

| Constant | Value | Source | Description |
|----------|-------|--------|-------------|
| `similarity::RRF_K` | `60.0` | `constitution.yaml embeddings.similarity.rrf_constant` | Reciprocal Rank Fusion constant k. Used in formula: RRF(d) = sum(1/(k + rank_i(d))) |
| `pipeline::DEFAULT_RRF_K` | `60.0` | Same as above | Pipeline default RRF k |

---

## 21. MCP Server CLI Arguments

The MCP server binary (`context-graph-mcp`) supports the following CLI arguments:

| Argument | Type | Description |
|----------|------|-------------|
| `--config <PATH>` | PathBuf | Path to configuration TOML file |
| `--transport <MODE>` | String | Transport mode: `stdio` (default) or `tcp` |
| `--port <PORT>` | u16 | TCP port (only with `--transport tcp`) |
| `--bind <ADDRESS>` | String | TCP bind address (default: 127.0.0.1) |
| `--warm-first` | flag | Block startup until embedding models are loaded into VRAM (default behavior) |
| `--no-warm` | flag | Skip blocking warmup (fast startup, embeddings fail until background load completes) |
| `--daemon` | flag | Enable daemon mode: share one MCP server across multiple terminals |
| `--daemon-port <PORT>` | u16 | Daemon TCP port (default: 3100) |
| `--help`, `-h` | flag | Show help |

Internal flag `--daemon-server-only` is set automatically by the daemon spawner and is not intended for direct use.

---

## 22. Config File Examples

> **Warning**: The checked-in files at `config/default.toml`, `config/development.toml`, `config/production.toml`, and `config/test.toml` use a legacy schema (`[general]`, `[embeddings]`, `[graph]`, `[server].transport`, `[server].tcp_port`, `[storage].sqlite_path`, `[storage].postgres_url`, `[embeddings].provider`) that does NOT match the current `Config` struct in `crates/context-graph-core/src/config/mod.rs`. The current struct expects top-level tables `[server]` (name, version), `[mcp]`, `[logging]`, `[storage]` (backend/path/compression), `[embedding]` (model), `[index]`, `[utl]`, `[cuda]`, `[watcher]`, and a top-level `phase` key. Because `Config::load()` registers these files with `required(false)` and every field that is not listed has a serde default, the stale sections are effectively ignored at runtime (the config library routes unknown keys to defaults). Treat the checked-in files as informational only; the examples below reflect the struct schema actually deserialized by `Config::try_deserialize()`.

### 22.1 Environment-Specific Overrides (Current Schema)

**Struct-accurate `config/development.toml`** (aligned with `Config` struct):

```toml
phase = "ghost"

[logging]
level = "debug"
format = "pretty"
include_location = false

[storage]
backend = "memory"
path = "./data/storage"
compression = true

[embedding]
model = "stub"

[utl]
mode = "stub"
consolidation_threshold = 0.5
```

**Struct-accurate `config/production.toml`**:

```toml
phase = "production"

[logging]
level = "info"
format = "json"
include_location = false

[mcp]
transport = "tcp"
tcp_port = 3100
bind_address = "127.0.0.1"
max_connections = 32

[storage]
backend = "rocksdb"
path = "./data/rocksdb"
compression = true

[embedding]
model = "e5-large-v2"

[index]
backend = "hnsw"
hnsw_m = 16
hnsw_ef_construction = 200

[utl]
mode = "production"
consolidation_threshold = 0.7
```

**Struct-accurate `config/test.toml`**:

```toml
phase = "ghost"

[logging]
level = "warn"

[storage]
backend = "memory"
path = "./data/test"

[embedding]
model = "stub"

[utl]
mode = "stub"
consolidation_threshold = 0.5
```

### 22.2 Actual Checked-In Default (`config/default.toml`) -- Legacy Schema

The current `config/default.toml` (verified against the file on disk) contains the following legacy sections. The `[general]`, `[embeddings]`, and `[graph]` sections have no matching fields in the current `Config` struct and are silently ignored; only `[watcher]` and `[watcher.code]` keys still map to real configuration fields (via `WatcherConfig` and the `[watcher.code]` subtable handling in `crates/context-graph-mcp`).

```toml
[general]              # IGNORED (no [general] in Config)
phase = "ghost"
log_level = "info"

[server]               # PARTIALLY IGNORED: `transport` and `tcp_port` belong under [mcp]
transport = "stdio"
tcp_port = 8080

[storage]              # PARTIALLY IGNORED: `sqlite_path`/`postgres_url` are not in StorageConfig
backend = "memory"
sqlite_path = "data/context-graph.db"

[embeddings]           # IGNORED: struct is singular `[embedding]` with field `model`, not `provider`
provider = "stub"

[cuda]
device_id = 0

[utl]                  # Only `consolidation_threshold` matches; current UtlConfig also has `mode`
consolidation_threshold = 0.7

[graph]                # IGNORED (no [graph] in Config)
max_edges_per_node = 100
default_edge_weight = 0.5

[watcher]              # OK: matches WatcherConfig exactly
enabled = true
watch_paths = ["./docs"]
session_id = "docs-watcher"
extensions = ["md"]    # extensions is not on WatcherConfig; handled by watcher pipeline code

[watcher.code]         # OK: consumed by CodeWatcher (see Section 12.1)
enabled = false
watch_paths = ["./crates"]
session_id = "code-watcher"
extensions = ["rs"]
use_ast_chunker = true
target_chunk_size = 500
min_chunk_size = 100
max_chunk_size = 1000
```

### 22.3 Minimal Production Setup (Environment Variables)

```bash
export CONTEXT_GRAPH_STORAGE_PATH="./data/rocksdb"
export CONTEXT_GRAPH_MODELS_PATH="./models"
export CONTEXT_GRAPH_WARM_FIRST=1
context-graph-mcp --daemon
```

Because `apply_overrides()` (in `crates/context-graph-mcp/src/main.rs:366`) promotes `storage.backend` to `"rocksdb"` when `CONTEXT_GRAPH_STORAGE_PATH` is set and `embedding.model` to `"e5-large-v2"` when `CONTEXT_GRAPH_MODELS_PATH` is set (which in turn promotes `index.backend` to `"hnsw"` and `utl.mode` to `"production"`), the environment variables alone yield a production-safe configuration regardless of what the TOML files say.

---

## 23. Cross-References

- Database schema and RocksDB column families: see [04_database_schema.md](04_database_schema.md)
- Storage layer, HNSW indexes, inverted indexes, SPLADE, ColBERT: see [09_storage_layer.md](09_storage_layer.md)
- Architecture overview and embedder details: see `01_architecture.md` (if present) or `07_embedding_pipeline.md`
- Constitution and forbidden patterns: see `docs2/constitution.yaml`
