# Configuration

Source files covered:

- `config/default.toml`, `config/development.toml`, `config/production.toml`, `config/test.toml`
- `.cargo/config.toml`, `rust-toolchain.toml`
- `crates/context-graph-core/src/config/{mod.rs,sub_configs.rs,embedder_config.rs,constants.rs}`
- `crates/context-graph-core/src/weights/mod.rs`
- `crates/context-graph-core/src/retrieval/config.rs`
- `crates/context-graph-core/src/similarity/config.rs`
- `crates/context-graph-core/src/index/config.rs`
- `crates/context-graph-embeddings/src/config/{mod.rs,batch.rs,cache.rs,gpu.rs,models.rs}`
- `crates/context-graph-embeddings/src/warm/config.rs`
- `crates/context-graph-embeddings/src/models/registry/config.rs`
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs`
- `crates/context-graph-causal-agent/src/service/mod.rs`
- `crates/context-graph-mcp/src/main.rs`
- `crates/context-graph-cli/src/mcp_client.rs`, `crates/context-graph-cli/src/commands/watch.rs`

---

## 1. File Layout and Precedence

TOML config files live in `config/` at the repo root.

| File | Role |
|------|------|
| `config/default.toml` | Base, always loaded first |
| `config/development.toml` | Loaded when `CONTEXT_GRAPH_ENV=development` (default) |
| `config/production.toml` | Loaded when `CONTEXT_GRAPH_ENV=production` |
| `config/test.toml` | Loaded when `CONTEXT_GRAPH_ENV=test` |

Precedence (lowest to highest):

1. `Config::default_config()` in `sub_configs.rs` (built-in struct defaults)
2. `config/default.toml`
3. `config/{CONTEXT_GRAPH_ENV}.toml`
4. Env vars prefixed `CONTEXT_GRAPH_` (nested separator `__`)
5. CLI `apply_overrides()` in `crates/context-graph-mcp/src/main.rs`

All file sources are `required(false)` — missing files are silently skipped.

---

## 2. Loader Flow

Entry point: `Config::load()` in `crates/context-graph-core/src/config/mod.rs:66`.

```
CONTEXT_GRAPH_ENV     -> selects env-specific file (default: "development")
config::Config::builder()
  .add_source(File::with_name("config/default").required(false))
  .add_source(File::with_name(format!("config/{env}")).required(false))
  .add_source(Environment::with_prefix("CONTEXT_GRAPH").separator("__"))
  .build()
  .try_deserialize::<Config>()
config.validate()   # MCP validation + phase-safety check
```

Alternate loaders:

| Function | Source | Purpose |
|----------|--------|---------|
| `Config::load()` | default+env TOML + env vars | Normal startup |
| `Config::from_file(path)` | single TOML file | Test / `--config <PATH>` |
| `Config::default_config()` | hard-coded struct defaults | Unit tests, fallback |

The `embeddings` crate has an independent loader: `EmbeddingConfig::from_file`, `EmbeddingConfig::from_toml_str`, `EmbeddingConfig::default().with_env_overrides()` (reads `EMBEDDING_*` vars). This config is not merged into the core `Config`; it is consumed by the embedding subsystem directly.

The warm-loader has its own `WarmConfig::from_env()` reading `WARM_*` and `CUDA_VISIBLE_DEVICES`.

The causal discovery service has its own `CausalDiscoveryConfig::with_env_overrides()` reading `CAUSAL_DISCOVERY_*`.

---

## 3. Core `Config` Keys (core/config)

Root struct: `Config` (`mod.rs`). Sections below list the TOML section, the Rust field → type, its serde default, and validation.

### 3.1 Top-level / `[phase]`

| TOML key | Type | Default | Valid values | Description |
|----------|------|---------|--------------|-------------|
| `phase` | enum (lowercase) | `ghost` | `ghost` \| `development` \| `production` | Controls stub-safety check. `production` fails startup if any stub backends are active. |

### 3.2 `[server]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `server.name` | String | `"context-graph"` | Server identity. |
| `server.version` | String | `"0.1.0-ghost"` | Version string. |

### 3.3 `[mcp]`

All fields have serde defaults so the section may be omitted.

| Key | Type | Default | Valid range | Description |
|-----|------|---------|-------------|-------------|
| `mcp.transport` | String | `"stdio"` | `stdio` \| `tcp` \| `sse` (case-insensitive) | JSON-RPC transport. SSE is parsed but rejected by `determine_transport_mode` at runtime. |
| `mcp.max_payload_size` | usize | `10485760` (10 MiB) | >0 | Max JSON-RPC request bytes. |
| `mcp.request_timeout` | u64 | `30` | >0 | Request timeout in seconds. |
| `mcp.bind_address` | String | `"127.0.0.1"` | non-empty for tcp/sse | TCP/SSE bind host. |
| `mcp.tcp_port` | u16 | `3100` | 1-65535 (>0 for tcp) | TCP port when `transport=tcp`. |
| `mcp.sse_port` | u16 | `3101` | 1-65535 (>0 for sse) | SSE port (transport rejected at runtime). |
| `mcp.max_connections` | usize | `32` | >0 for tcp/sse | Max concurrent connections. |

Validation: `McpConfig::validate()` — transport ∈ {stdio,tcp,sse}; payload/timeout > 0; TCP/SSE-specific fields validated only when transport matches.

### 3.4 `[logging]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `logging.level` | String | `"info"` | trace/debug/info/warn/error |
| `logging.format` | String | `"pretty"` | pretty \| json |
| `logging.include_location` | bool | `false` | Include source file/line. |

### 3.5 `[storage]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `storage.backend` | String | `"memory"` | `memory` (stub) \| `rocksdb` (prod) \| `sqlite` / `postgres` (scaffold only). |
| `storage.path` | String | `"./data/storage"` | Filesystem path when backend ≠ memory. Parent dir must exist (validated). |
| `storage.compression` | bool | `true` | Enable RocksDB compression. |

### 3.6 `[embedding]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `embedding.model` | String | `"stub"` | `stub` \| `multi_array_13` \| specific model id (e.g. `e5-large-v2`). |

Triggers stub-warning in Ghost/Development phase; aborts startup in Production.

### 3.7 `[index]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `index.backend` | String | `"memory"` | `memory` (ephemeral) \| `hnsw` (persisted). |
| `index.hnsw_m` | usize | `16` | HNSW M parameter. |
| `index.hnsw_ef_construction` | usize | `200` | HNSW ef_construction. |

### 3.8 `[utl]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `utl.mode` | String | `"stub"` | `stub` \| `production`. |
| `utl.consolidation_threshold` | f32 | `0.7` | Threshold for consolidation decisions. |

### 3.9 `[cuda]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `cuda.device_id` | u32 | `0` | Primary CUDA device. |
| `cuda.memory_limit_gb` | f32 | `4.0` | Memory cap in GB. |

### 3.10 `[watcher]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `watcher.enabled` | bool | `false` | Enable file watcher (default.toml overrides to `true`). |
| `watcher.watch_paths` | Vec\<String\> | `["./docs"]` | Recursive directories. |
| `watcher.session_id` | String | `"docs-watcher"` | Session tag for captured memories. |

Keys present in `default.toml` but not deserialized by the core `WatcherConfig` struct (parsed but ignored by core loader; consumed elsewhere by the file-watcher subsystem): `watcher.extensions`, `watcher.code.*` (enabled, watch_paths, session_id, extensions, use_ast_chunker, target_chunk_size, min_chunk_size, max_chunk_size).

### 3.11 `[graph]` (config files only)

Present in `default.toml`, `production.toml`, `test.toml` but not bound to any field in the core `Config`. Current effect: ignored by `Config::load()`. Preserved for downstream tooling.

| Key | Type | default.toml | production.toml | test.toml |
|-----|------|-------------|-----------------|-----------|
| `graph.max_edges_per_node` | usize | 100 | 1000 | 10 |
| `graph.default_edge_weight` | f32 | 0.5 | 0.5 | — |

### 3.12 `[general]` (config files only)

`default.toml` exposes `general.phase` and `general.log_level` as convenience duplicates; neither is bound to the core `Config` struct (which uses top-level `phase` and `[logging]`). Effective values come from `phase` and `logging.level`.

---

## 4. Phase-Safety Validation

`Config::validate()` → `validate_phase_safety()` flags these as "dangerous":

| Field | Dangerous value |
|-------|-----------------|
| `embedding.model` | `"stub"` |
| `storage.backend` | `"memory"` |
| `index.backend` | `"memory"` |
| `utl.mode` | `"stub"` |

Behavior by phase:

| Phase | Action on dangerous config |
|-------|----------------------------|
| `ghost` | Warn to stderr (suppressed by `CONTEXT_GRAPH_MCP_QUIET`). |
| `development` | Warn to stderr (suppressed by `CONTEXT_GRAPH_MCP_QUIET`). |
| `production` | **Return `CoreError::ConfigError`; startup aborts.** |

Additional validation:

- If `storage.backend != "memory"` and `storage.path` parent dir does not exist → error.
- `McpConfig::validate()` checks transport, payload, timeout, and transport-specific fields.

---

## 5. Per-Environment Overrides (actual contents)

Only keys present in each file:

### `default.toml`

`general.phase=ghost`, `general.log_level=info`, `server.transport=stdio`, `server.tcp_port=8080`, `storage.backend=memory`, `storage.sqlite_path="data/context-graph.db"`, `embeddings.provider=stub`, `cuda.device_id=0`, `utl.consolidation_threshold=0.7`, `graph.max_edges_per_node=100`, `graph.default_edge_weight=0.5`, `watcher.enabled=true`, `watcher.watch_paths=["./docs"]`, `watcher.session_id="docs-watcher"`, `watcher.extensions=["md"]`, `watcher.code.enabled=false`, `watcher.code.watch_paths=["./crates"]`, `watcher.code.session_id="code-watcher"`, `watcher.code.extensions=["rs"]`, `watcher.code.use_ast_chunker=true`, `watcher.code.target_chunk_size=500`, `watcher.code.min_chunk_size=100`, `watcher.code.max_chunk_size=1000`.

### `development.toml`

`general.phase=ghost`, `general.log_level=debug`, `utl.consolidation_threshold=0.5`. Others inherit from default.

### `production.toml`

`general.phase=nervous` (legacy label; core enum rejects — production startup requires `phase=production` set elsewhere), `server.transport=tcp`, `server.tcp_port=8080`, `storage.backend=postgres`, `storage.postgres_url="${POSTGRES_URL}"`, `embeddings.provider=local`, `embeddings.model_path="models/all-MiniLM-L6-v2"`, `cuda.device_id=0`, `utl.consolidation_threshold=0.7`, `graph.max_edges_per_node=1000`.

### `test.toml`

`general.phase=ghost`, `general.log_level=warn`, `utl.consolidation_threshold=0.5`, `graph.max_edges_per_node=10`.

Note: `server.transport`, `server.tcp_port`, `storage.sqlite_path`, `storage.postgres_url`, `embeddings.provider`, `embeddings.model_path` match TOML keys the core loader does not bind (it expects `mcp.transport`, `mcp.tcp_port`, `storage.path`, `embedding.model`). These values in the TOML files are informational for deployment tooling rather than consumed by `Config::load()`.

---

## 6. Environment Variables

### 6.1 `CONTEXT_GRAPH_*` (MCP/core)

| Name | Default | Purpose | Consumer |
|------|---------|---------|----------|
| `CONTEXT_GRAPH_ENV` | `development` | Selects env-specific TOML file. | `core/config/mod.rs:67` |
| `CONTEXT_GRAPH_MCP_QUIET` | unset | Suppresses phase-safety stderr warnings. | `core/config/mod.rs:193` |
| `CONTEXT_GRAPH_TRANSPORT` | — | Overrides `mcp.transport` (stdio/tcp). | `mcp/main.rs:309,391` |
| `CONTEXT_GRAPH_TCP_PORT` | — | Overrides `mcp.tcp_port`. | `mcp/main.rs:371` |
| `CONTEXT_GRAPH_BIND_ADDRESS` | — | Overrides `mcp.bind_address`. | `mcp/main.rs:382` |
| `CONTEXT_GRAPH_STORAGE_PATH` | — | Sets `storage.path`; auto-flips `backend` to `rocksdb`. | `mcp/main.rs:398`, `mcp/server/mod.rs:1188` |
| `CONTEXT_GRAPH_MODELS_PATH` | — | Sets model dir; auto-flips `embedding.model` from `stub` to `e5-large-v2`. | `mcp/main.rs:410`, `mcp/server/mod.rs:1240`, `embeddings/global_provider.rs:88` |
| `CONTEXT_GRAPH_WATCHER_ENABLED` | — | `"1"`/`"true"` enables, else disables `watcher.enabled`. | `mcp/main.rs:435` |
| `CONTEXT_GRAPH_WARM_FIRST` | `1` | `"0"`/`"false"` disables blocking GPU warmup. | `mcp/main.rs:465` |
| `CONTEXT_GRAPH_DAEMON` | `0` | `"1"`/`"true"` enables daemon mode. | `mcp/main.rs:494` |
| `CONTEXT_GRAPH_DAEMON_PORT` | `3100` | Daemon TCP port. | `mcp/main.rs:517` |
| `CONTEXT_GRAPH_DATA_DIR` | — | Data dir for `cg watch` CLI. | `cli/commands/watch.rs:104` |
| `CONTEXT_GRAPH_MCP_HOST` | `127.0.0.1` | CLI → MCP daemon host. | `cli/mcp_client.rs:226` |
| `CONTEXT_GRAPH_MCP_PORT` | `3100` | CLI → MCP daemon port. | `cli/mcp_client.rs:228` |

Cascading overrides (applied in `apply_overrides()` after `CONTEXT_GRAPH_STORAGE_PATH` is set):

- `storage.backend=memory` + storage path set → `backend=rocksdb`.
- `storage.backend=rocksdb` + `index.backend=memory` → `index.backend=hnsw`.
- `storage.backend=rocksdb` + `utl.mode=stub` → `utl.mode=production`.
- `embedding.model=stub` + `CONTEXT_GRAPH_MODELS_PATH` set → `embedding.model=e5-large-v2`.

### 6.2 `EMBEDDING_*` (embeddings crate, applied via `with_env_overrides()`)

| Name | Field overridden | Type |
|------|------------------|------|
| `EMBEDDING_MODELS_DIR` | `models.models_dir` | String |
| `EMBEDDING_LAZY_LOADING` | `models.lazy_loading` | bool |
| `EMBEDDING_GPU_ENABLED` | `gpu.enabled` | bool |
| `EMBEDDING_CACHE_ENABLED` | `cache.enabled` | bool |
| `EMBEDDING_CACHE_MAX_ENTRIES` | `cache.max_entries` | usize |
| `EMBEDDING_BATCH_MAX_SIZE` | `batch.max_batch_size` | usize |
| `EMBEDDING_BATCH_MAX_WAIT_MS` | `batch.max_wait_ms` | u64 |
| `MODELS_DIR` | Diagnostic-only fallback for models dir. | `embeddings/provider/diagnostic_test.rs:18` |

### 6.3 `WARM_*` / CUDA (warm loader)

| Name | Default | Field |
|------|---------|-------|
| `WARM_VRAM_BUDGET_BYTES` | `24*GB` (25,769,803,776) | `vram_budget_bytes` |
| `WARM_VRAM_HEADROOM_BYTES` | `8*GB` (8,589,934,592) | `vram_headroom_bytes` |
| `WARM_MODEL_WEIGHTS_PATH` | `./models` | `model_weights_path` |
| `WARM_DIAGNOSTIC_DUMP_PATH` | `/var/log/context-graph` | `diagnostic_dump_path` |
| `CUDA_VISIBLE_DEVICES` | `0` | First CSV element → `cuda_device_id` |
| `WARM_ENABLE_TEST_INFERENCE` | `true` | `enable_test_inference` (accepts true/1/yes/on) |
| `WARM_MAX_LOAD_TIME_MS` | `30000` | `max_load_time_per_model_ms` |

### 6.4 `CAUSAL_DISCOVERY_*` (causal agent service)

| Name | Default | Field |
|------|---------|-------|
| `CAUSAL_DISCOVERY_ENABLED` | `false` | `enable_background` (true/1 enables) |
| `CAUSAL_DISCOVERY_INTERVAL_SECS` | `120` | `interval` |
| `CAUSAL_DISCOVERY_BATCH_SIZE` | `100` | `batch_size` |
| `CAUSAL_DISCOVERY_MIN_CONFIDENCE` | `0.7` | `min_confidence` |

### 6.5 CLI helpers

| Name | Usage |
|------|-------|
| `CLAUDE_SESSION_ID` | Default session id for `cg memory inject`, `cg memory capture`, MCP core handler (`store_memory` fallback). |
| `HOME` / `USERPROFILE` | Session-persist path resolution (`cli/commands/session/persist.rs`). |
| `HOSTNAME` / `COMPUTERNAME` | Diagnostic host identity (`warm/diagnostics/types.rs`). |
| `BENCHMARK_PAIRS`, `BENCHMARK_MEMORIES` | Benchmark example scale knobs (examples only). |
| `SKIP_GPU_TESTS` | Skip GPU tests in `context-graph-cuda`. |
| `RUST_LOG` | tracing-subscriber log filter. |
| `RUST_TEST_THREADS` | Set to `4` via `.cargo/config.toml`. |

### 6.6 Build-time env (`context-graph-cuda/build.rs`, `.cargo/config.toml`)

| Name | Default | Purpose |
|------|---------|---------|
| `CUDA_HOME` / `CUDAToolkit_ROOT` | `/usr/local/cuda-13.1` | CUDA install root. |
| `CMAKE_CUDA_COMPILER` / `CUDACXX` | `/usr/local/cuda-13.1/bin/nvcc` | NVCC path. |
| `CMAKE_CUDA_ARCHITECTURES` | `120a-real` | Blackwell consumer SM arch. |
| `CUDARC_CUDA_VERSION` | `13000` | Forces cudarc 13.0 ABI. |
| `CUDA_PATH` | auto-detected | Override CUDA path in build.rs. |
| `CUDA_ARCH` | `sm_120a` | nvcc target arch in build.rs. |
| `NVCC_FLAGS` | `-Xptxas -O3 --use_fast_math --restrict` | Extra nvcc flags (build.rs). |
| `OUT_DIR` | cargo-provided | Build output dir. |
| `CARGO_MANIFEST_DIR`, `CARGO_PKG_VERSION` | cargo-provided | Used in test data lookups. |

---

## 7. Embeddings Subsystem Config (`EmbeddingConfig`)

Struct: `EmbeddingConfig` in `embeddings/src/config/mod.rs`. Independent of core `Config`. Load via `from_file`, `from_toml_str`, or `default().with_env_overrides()`. Validated by `EmbeddingConfig::validate()` (calls each subsystem's validate, first-error-wins).

### 7.1 `[models]` → `ModelPathConfig`

| Key | Type | Default | Validation |
|-----|------|---------|------------|
| `models.models_dir` | String | `"./models"` | non-empty |
| `models.lazy_loading` | bool | `true` | — |
| `models.preload_models` | Vec\<String\> | `[]` | each must match a `ModelId::as_str()` (case/hyphen-insensitive) |
| `models.max_loaded_models` | usize | `13` | — (0 = unlimited) |

Valid preload names: `semantic, temporal_recent, temporal_periodic, temporal_positional, causal, sparse, code, graph, hdc, contextual, entity, kepler, late_interaction, splade`.

### 7.2 `[batch]` → `BatchConfig`

| Key | Type | Default | Validation |
|-----|------|---------|------------|
| `batch.max_batch_size` | usize | `32` | >0 |
| `batch.min_batch_size` | usize | `1` | ≤ max_batch_size |
| `batch.max_wait_ms` | u64 | `50` | >0 when min_batch_size>1 |
| `batch.dynamic_batching` | bool | `true` | — |
| `batch.padding_strategy` | enum | `dynamic_max` | `max_length` \| `dynamic_max` \| `power_of_two` \| `bucket` |
| `batch.sort_by_length` | bool | `true` | — |

### 7.3 `[cache]` → `CacheConfig`

| Key | Type | Default | Validation |
|-----|------|---------|------------|
| `cache.enabled` | bool | `true` | — |
| `cache.max_entries` | usize | `100000` | >0 when enabled |
| `cache.max_bytes` | usize | `1073741824` (1 GiB) | >0 when enabled |
| `cache.ttl_seconds` | Option\<u64\> | `None` | — |
| `cache.eviction_policy` | enum | `lru` | `lru` \| `lfu` \| `ttl_lru` \| `arc` |
| `cache.persist_to_disk` | bool | `false` | — |
| `cache.disk_path` | Option\<PathBuf\> | `None` | required when persist_to_disk |

### 7.4 `[gpu]` → `GpuConfig`

| Key | Type | Default | Validation |
|-----|------|---------|------------|
| `gpu.enabled` | bool | `true` | — |
| `gpu.device_ids` | Vec\<u32\> | `[0]` | non-empty when enabled |
| `gpu.memory_fraction` | f32 | `0.9` | (0.0, 1.0], non-NaN |
| `gpu.use_cuda_graphs` | bool | `true` | — |
| `gpu.mixed_precision` | bool | `true` | — |
| `gpu.green_contexts` | bool | `false` | — |
| `gpu.gds_enabled` | bool | `false` | — |

### 7.5 `WarmConfig` (non-TOML — env/code only)

| Field | Default | Validation |
|-------|---------|------------|
| `vram_budget_bytes` | 24 GiB | >0 |
| `vram_headroom_bytes` | 8 GiB | — |
| `model_weights_path` | `./models` | must exist |
| `diagnostic_dump_path` | `/var/log/context-graph` | — |
| `cuda_device_id` | 0 | — |
| `enable_test_inference` | true | — |
| `max_load_time_per_model_ms` | 30000 | — |
| `quantization` | `Fp16` | `Fp32` \| `Fp16` \| `Fp8` |

### 7.6 `ModelRegistryConfig` (runtime, non-TOML)

| Field | Default | Validation |
|-------|---------|------------|
| `max_concurrent_loads` | 4 | >0 |
| `memory_budget_bytes` | 32 GB (32_000_000_000) | >0 |
| `preload_models` | `[]` | no duplicates |
| `enable_debug_logging` | false | — |

Convenience constructors: `rtx_5090()`, `rtx_4090()` (24 GB), `testing(budget)`.

---

## 8. Storage/Index Config (`HnswConfig`, `InvertedIndexConfig`)

Struct: `HnswConfig` in `storage/.../hnsw_config/config.rs`. Created in code (not TOML-bound).

| Field | Default (`default_for_dimension`) | Matryoshka 128D (`matryoshka_128d`) | Validation (panics with `HNSW CONFIG ERROR`) |
|-------|-----------------------------------|-------------------------------------|----------------------------------------------|
| `m` | 16 | 32 | ≥ 2 |
| `ef_construction` | 200 | 256 | ≥ m |
| `ef_search` | 100 | 128 | ≥ 1 |
| `metric` | caller-supplied | `Cosine` | enum `DistanceMetric` |
| `dimension` | caller-supplied | 128 | ≥ 1 |

`InvertedIndexConfig`: `e6_sparse()` → `{vocab_size: 30522, max_nnz: 1500, use_bm25: false}`; `e13_splade()` → same vocab, `use_bm25: true`.

Embedder dimensions (constants in `core/src/index/config.rs` and `storage/.../hnsw_config/constants.rs`):

| Embedder | Dim / Vocab |
|----------|-------------|
| E1 / E8 / E9 | 1024 |
| E2 / E3 / E4 | 512 |
| E5 / E10 / E11 | 768 |
| E7 | 1536 |
| E12 | 128 per token |
| E1 Matryoshka | 128 |
| E6 / E13 | 30522 (BERT vocab) |

---

## 9. Similarity, Retrieval, and Fusion Config

### 9.1 `CrossSpaceConfig` (`core/src/similarity/config.rs`)

| Field | Default | Values |
|-------|---------|--------|
| `weighting_strategy` | `RRF { k: 60.0 }` | `Uniform` / `Static([f32;13])` / `TopicAligned` / `RRF{k}` / `TopicWeightedRRF{k}` / `LateInteraction` |
| `min_active_spaces` | 1 | usize |
| `missing_space_handling` | `Skip` | `Skip` / `ZeroFill` / `AverageFill` / `RequireAll` |
| `include_breakdown` | false | bool |

RRF constant: `core/src/config/constants.rs::similarity::RRF_K = 60.0`, re-exported as `pipeline::DEFAULT_RRF_K`.

### 9.2 Per-space thresholds (`core/src/retrieval/config.rs`)

Fixed constants, not TOML:

| Embedder | High threshold | Low threshold |
|----------|----------------|---------------|
| E1 Semantic | 0.75 | 0.30 |
| E2 Temporal Recent | 0.70 | 0.30 |
| E3 Temporal Periodic | 0.70 | 0.30 |
| E4 Temporal Positional | 0.70 | 0.30 |
| E5 Causal | 0.70 | 0.25 |
| E6 Sparse | 0.60 | 0.20 |
| E7 Code | 0.80 | 0.35 |
| E8 Graph | 0.70 | 0.30 |
| E9 HDC | 0.70 | 0.30 |
| E10 Multimodal | 0.70 | 0.30 |
| E11 Entity | 0.70 | 0.30 |
| E12 Late Interaction | 0.70 | 0.30 |
| E13 SPLADE | 0.60 | 0.20 |

Category weights (`SPACE_WEIGHTS`, sum = `TOTAL_WEIGHT = 8.5`):

| Category | Embedders | Weight |
|----------|-----------|--------|
| Semantic | E1, E5, E6, E7, E10, E12, E13 | 1.0 |
| Temporal | E2, E3, E4 | 0.0 |
| Relational | E8, E11 | 0.5 |
| Structural | E9 | 0.5 |

Constants: `RECENT_LOOKBACK_SECS = 7200` (2h), `MAX_RECENT_MEMORIES = 50`.

### 9.3 Weight profiles (`core/src/weights/mod.rs`)

Compile-time constants:

| Constant | Value | Effect |
|----------|-------|--------|
| `TEMPORAL_EMBEDDERS_ENABLED` | `true` | E2/E3/E4 generate vectors during store_memory. |
| `E11_ENTITY_ENABLED` | `false` | E11 excluded from fusion by default. |

`WEIGHT_PROFILES` is a compile-time list of `(name, [f32;13])` weight vectors. See [06_search_and_fusion.md](06_search_and_fusion.md) for full profile table (semantic_search, code_search, causal_reasoning, graph_reasoning, temporal_navigation, category_weighted, etc.).

---

## 10. Causal Discovery Config (`CausalDiscoveryConfig`)

Runtime struct in `crates/context-graph-causal-agent/src/service/mod.rs`. Not TOML-bound; overridden via `CAUSAL_DISCOVERY_*` env vars (§6.4).

| Field | Default | Env override |
|-------|---------|--------------|
| `interval` | `Duration::from_secs(120)` | `CAUSAL_DISCOVERY_INTERVAL_SECS` |
| `batch_size` | 100 | `CAUSAL_DISCOVERY_BATCH_SIZE` |
| `min_confidence` | 0.7 | `CAUSAL_DISCOVERY_MIN_CONFIDENCE` |
| `skip_analyzed` | true | — |
| `enable_background` | false | `CAUSAL_DISCOVERY_ENABLED` |
| `min_interval` | 30 s | — |
| `max_interval` | 600 s | — |
| `max_consecutive_errors` | 3 | — |
| `llm_config` | `LlmConfig::default()` | — |
| `scanner_config` | `ScannerConfig::default()` | — |
| `activator_config` | `ActivatorConfig::default()` | — |

---

## 11. MCP CLI Flags (`context-graph-mcp`)

CLI > env > TOML > defaults. Parsed in `crates/context-graph-mcp/src/main.rs`.

| Flag | Arg | Effect |
|------|-----|--------|
| `--config <PATH>` | path | Load TOML from path. |
| `--transport <MODE>` | `stdio`\|`tcp` | Overrides `mcp.transport`; `sse` rejected with error. |
| `--port <PORT>` | u16 | Overrides `mcp.tcp_port`. |
| `--bind <ADDR>` | string | Overrides `mcp.bind_address`. |
| `--warm-first` | — | Clears `--no-warm`; block startup until GPU warm. |
| `--no-warm` | — | Disable blocking warmup. |
| `--daemon` | — | Enable daemon mode (shared server via TCP). |
| `--daemon-port <PORT>` | u16 (default 3100) | Daemon TCP port. |
| `--daemon-server-only` | — | Internal: this process is the headless daemon (set by `spawn_daemon_process`). |
| `--help` / `-h` | — | Print usage and exit. |

---

## 12. Build / Toolchain Config

`rust-toolchain.toml`:

| Key | Value |
|-----|-------|
| `toolchain.channel` | `stable` |
| `toolchain.components` | `rustfmt, clippy, rust-src` |
| `toolchain.targets` | `x86_64-unknown-linux-gnu` |

`.cargo/config.toml`:

| Key | Value |
|-----|-------|
| `build.rustflags` | `["-C", "target-cpu=native"]` |
| `target.x86_64-unknown-linux-gnu.linker` | `clang` |
| `target.x86_64-unknown-linux-gnu.rustflags` | `["-C", "link-arg=-fuse-ld=lld"]` |
| `profile.release.incremental` | `false` |
| `profile.dev.incremental` | `true` |
| `env.*` | See §6.6. |
| `alias.check-all` | `check --workspace --all-targets` |
| `alias.clippy-all` | `clippy --workspace --all-targets -- -D warnings` |
| `alias.test-all` | `test --workspace -- --test-threads=4` |
| `alias.test-llm` | `run -p context-graph-causal-agent --example test_llm --features cuda --release` |
| `alias.bench-causal` | `run -p context-graph-causal-agent --example benchmark_causal --features cuda --release` |
| `alias.bench-graph` | `run -p context-graph-graph-agent --example benchmark_graph_code --features cuda --release` |
| `alias.clean-debug` / `alias.clean-all` | Cleanup helpers. |

---

## 13. Embedder Name Resolution

`EmbedderConfig` (`core/src/config/embedder_config.rs`) uses a custom deserializer `deserialize_embedder` backed by `Embedder::from_name()`. Accepts canonical, short, or lowercase names:

| Canonical | Short | Enum variant |
|-----------|-------|--------------|
| `E1_Semantic` | `E1`, `semantic` | `Embedder::Semantic` |
| `E2_Temporal_Recent` | `E2` | `TemporalRecent` |
| `E3_Temporal_Periodic` | `E3` | `TemporalPeriodic` |
| `E4_Temporal_Positional` | `E4` | `TemporalPositional` |
| `E5_Causal` | `E5` | `Causal` |
| `E6_Sparse_Lexical` | `E6` | `Sparse` |
| `E7_Code` | `E7` | `Code` |
| `E8_Graph` | `E8` | `Graph` |
| `E9_HDC` | `E9` | `Hdc` |
| `E10_Multimodal` | `E10` | `Contextual` |
| `E11_Entity` | `E11` | `Entity` |
| `E12_Late_Interaction` | `E12` | `LateInteraction` |
| `E13_SPLADE` | `E13` | `KeywordSplade` |

`E8_Emotional` is rejected (deprecated; use `check_deprecated_names()` to lint config files).

`EmbedderWeightsConfig::get_weight` returns weight from `configs` first, then `weights` map, else `0.0` (unknown embedders do not participate in scoring).

---

## 14. Cross-References

| Topic | Document |
|-------|----------|
| Embedder models, dims, asymmetry, preload | [05_embedding_subsystem.md](05_embedding_subsystem.md) |
| Fusion strategies, RRF, weight profiles | [06_search_and_fusion.md](06_search_and_fusion.md) |
| Storage backends, RocksDB column families | [09_storage_layer.md](09_storage_layer.md) |
| Graph & causal discovery | [07_graph_and_causal.md](07_graph_and_causal.md) |
| MCP tool schemas | [08_mcp_tools_reference.md](08_mcp_tools_reference.md) |
| CLI surface | [10_cli_reference.md](10_cli_reference.md) |
| CUDA acceleration | [11_cuda_acceleration.md](11_cuda_acceleration.md) |
| Database schema | [04_database_schema.md](04_database_schema.md) |
