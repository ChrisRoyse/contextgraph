# Configuration

**Source files covered:**
- `crates/context-graph-core/src/config/mod.rs` — root `Config`, `Phase`, load order, validation
- `crates/context-graph-core/src/config/sub_configs.rs` — `ServerConfig`, `McpConfig`, `LoggingConfig`, `StorageConfig`, `EmbeddingConfig`, `IndexConfig`, `UtlConfig`, `CudaConfig`, `WatcherConfig`
- `crates/context-graph-core/src/config/embedder_config.rs` — `EmbedderConfig`, `EmbedderWeightsConfig`, name-based deserialization
- `crates/context-graph-embeddings/src/config/mod.rs` — `EmbeddingConfig` (root for embedding subsystem)
- `crates/context-graph-embeddings/src/config/models.rs` — `ModelPathConfig`
- `crates/context-graph-embeddings/src/config/batch.rs` — `BatchConfig`, `PaddingStrategy`
- `crates/context-graph-embeddings/src/config/cache.rs` — `CacheConfig`, `EvictionPolicy`
- `crates/context-graph-embeddings/src/config/gpu.rs` — `GpuConfig`
- `crates/context-graph-embeddings/src/warm/config.rs` — `WarmConfig`, `QuantizationMode`
- `crates/context-graph-mcp/src/main.rs` — CLI args, env overrides, transport selection, daemon mode
- `crates/context-graph-cli/src/bin/reality-loop.rs:25-71` — CLI args for the SWE-bench reality loop (no env vars)
- `config/default.toml`, `config/development.toml`, `config/production.toml`, `config/test.toml`
- `configs/dynamicjepa/{domain_packs,verification}/*` — DynamicJEPA bundles (TOML domain packs + JSONL fixtures + `tiny_train_config.json`, `*_train_config.json`, `career_taxonomy_data_manifest.json`)

Cross-references:
- [04_database_schema.md](04_database_schema.md) — physical CF/key/value layout governed by these configs
- [09_storage_layer.md](09_storage_layer.md) — runtime storage repository + HNSW lifecycle that consumes `storage.*`/`index.*`
- [05_embedding_subsystem.md](05_embedding_subsystem.md) — what `embedding.*` and `EmbeddingConfig.gpu/batch/cache/models` actually drive
- [11_cuda_acceleration.md](11_cuda_acceleration.md) — what `cuda.*` and `WarmConfig.*` drive

## 1. Load Order and Resolution

`Config::load` (`crates/context-graph-core/src/config/mod.rs:66-77`) loads in the following order; later sources OVERRIDE earlier ones:

| Step | Source | Required? | Notes |
| --- | --- | --- | --- |
| 1 | `config/default.toml` | optional (`required(false)`) | Base settings |
| 2 | `config/{CONTEXT_GRAPH_ENV}.toml` | optional (`required(false)`) | `CONTEXT_GRAPH_ENV` defaults to `"development"` (`mod.rs:67`) |
| 3 | Environment variables prefixed `CONTEXT_GRAPH` with `__` separator | optional | E.g. `CONTEXT_GRAPH__MCP__TRANSPORT=tcp` maps to `mcp.transport` (`mod.rs:72`) |
| 4 | `apply_overrides` (CLI flags + dedicated env vars) in `crates/context-graph-mcp/src/main.rs:363-462` | runtime | Applied AFTER load, BEFORE phase validation is re-verified by handlers |
| 5 | `Config::validate()` (`mod.rs:126-147`) | enforced | Calls `mcp.validate()`, parent-dir check on `storage.path`, then `validate_phase_safety()` |

`Config::from_file(path)` (`mod.rs:96-110`) reads exactly one TOML file via `std::fs::read_to_string` + `toml::from_str` (no env layering), then validates.

The checked-in TOML files in `config/` (default.toml, development.toml, production.toml, test.toml) reference legacy section names (`[general]`, `[server.transport]`, `[storage].sqlite_path`, `[embeddings]`, `[graph]`) that DO NOT match the current Rust `Config` shape — fields not present in the Rust types are silently ignored by serde. The Rust types in `crates/context-graph-core/src/config/sub_configs.rs` are authoritative.

## 2. Top-Level `Config` Keys

Defined in `crates/context-graph-core/src/config/mod.rs:42-57`. All sections except `phase` and `watcher` are required at the type level (their `Default` impls supply values).

| Key | Type | Default (`Config::default_config`) | Validation |
| --- | --- | --- | --- |
| `phase` | `Phase` enum | `Phase::Ghost` | Serde `lowercase`: `ghost`/`development`/`production` (mod.rs:28-38) |
| `server` | `ServerConfig` | see §3 | n/a |
| `mcp` | `McpConfig` | see §4 | `McpConfig::validate()` (mod.rs:129) |
| `logging` | `LoggingConfig` | see §5 | none |
| `storage` | `StorageConfig` | see §6 | parent of `path` must exist when `backend != "memory"` (mod.rs:131-141) |
| `embedding` | `EmbeddingConfig` (core) | see §7 | phase safety (mod.rs:159-260) |
| `index` | `IndexConfig` | see §8 | phase safety |
| `utl` | `UtlConfig` | see §9 | phase safety |
| `cuda` | `CudaConfig` | see §10 | none in `Config::validate` |
| `watcher` | `WatcherConfig` | disabled | none in `Config::validate` |

## 3. `[server]` — `ServerConfig`

Source: `sub_configs.rs:9-22`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `name` | `String` | `"context-graph"` | none |
| `version` | `String` | `"0.1.0-ghost"` | none |

## 4. `[mcp]` — `McpConfig`

Source: `sub_configs.rs:24-214`. All fields have serde defaults; all are validated by `McpConfig::validate()`.

| Key | Type | Default | Valid range | Validation behavior |
| --- | --- | --- | --- | --- |
| `transport` | `String` | `"stdio"` | `"stdio"` \| `"tcp"` \| `"sse"` (case-insensitive) | Other values: `ConfigError` (`sub_configs.rs:137`). Note: `crates/context-graph-mcp/src/main.rs:290-295,315-320,339-344` REJECTS `sse` even if config validation accepted it. |
| `max_payload_size` | `usize` | `10_485_760` (10MB) | `> 0` | Zero ⇒ `ConfigError` (`sub_configs.rs:144-148`) |
| `request_timeout` | `u64` | `30` (seconds) | `> 0` | Zero ⇒ `ConfigError` (`sub_configs.rs:151-155`) |
| `bind_address` | `String` | `"127.0.0.1"` | non-empty when `transport ∈ {tcp, sse}` | Empty ⇒ `ConfigError` (`sub_configs.rs:160-165, 187-191`) |
| `tcp_port` | `u16` | `3100` | `1..=65535` when `transport == "tcp"` | `0` ⇒ `ConfigError` (`sub_configs.rs:168-173`) |
| `sse_port` | `u16` | `3101` | `1..=65535` when `transport == "sse"` | `0` ⇒ `ConfigError` (`sub_configs.rs:194-200`). NOTE: SSE rejected at runtime regardless. |
| `max_connections` | `usize` | `32` | `> 0` when transport is tcp/sse | Zero ⇒ `ConfigError` (`sub_configs.rs:176-181, 202-208`) |

## 5. `[logging]` — `LoggingConfig`

Source: `sub_configs.rs:217-232`. None are validated.

| Key | Type | Default |
| --- | --- | --- |
| `level` | `String` | `"info"` |
| `format` | `String` | `"pretty"` |
| `include_location` | `bool` | `false` |

## 6. `[storage]` — `StorageConfig`

Source: `sub_configs.rs:234-250`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `backend` | `String` | `"memory"` | `Phase::Production` REJECTS `"memory"` (`mod.rs:170-176`). `apply_overrides` flips `"memory"` → `"rocksdb"` when `CONTEXT_GRAPH_STORAGE_PATH` is set (`main.rs:418-426`). |
| `path` | `String` | `"./data/storage"` | When `backend != "memory"`, parent of path must exist (`mod.rs:131-141`). Single-writer flock at `<path>/mcp.pid`. |
| `compression` | `bool` | `true` | none |

## 7. `[embedding]` — Core `EmbeddingConfig` (root config)

Source: `sub_configs.rs:252-264`. This is a separate, smaller type than the `EmbeddingConfig` in `context-graph-embeddings`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `model` | `String` | `"stub"` | `Phase::Production` REJECTS `"stub"` (`mod.rs:163-167`). `apply_overrides` flips `"stub"` → `"e5-large-v2"` when `CONTEXT_GRAPH_MODELS_PATH` is set (`main.rs:430-439`). |

## 8. `[index]` — `IndexConfig`

Source: `sub_configs.rs:266-282`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `backend` | `String` | `"memory"` | `Phase::Production` REJECTS `"memory"` (`mod.rs:177-182`). `apply_overrides` flips `"memory"` → `"hnsw"` when `storage.backend == "rocksdb"` (`main.rs:443-446`). |
| `hnsw_m` | `usize` | `16` | none enforced at config layer |
| `hnsw_ef_construction` | `usize` | `200` | none enforced at config layer |

Per-embedder HNSW M/ef defaults are NOT taken from `index.hnsw_m`/`index.hnsw_ef_construction`. They are hard-coded per `EmbedderIndex` in `crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs` and only consumed by the storage layer, not by `Config`.

## 9. `[utl]` — `UtlConfig`

Source: `sub_configs.rs:284-298`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `mode` | `String` | `"stub"` | `Phase::Production` REJECTS `"stub"` (`mod.rs:184-189`). `apply_overrides` flips `"stub"` → `"production"` when `storage.backend == "rocksdb"` (`main.rs:449-451`). |
| `consolidation_threshold` | `f32` | `0.7` | none enforced at config layer |

## 10. `[cuda]` — `CudaConfig`

Source: `sub_configs.rs:300-314`. Used by the core crate only; the embeddings crate has its own `GpuConfig` (§17) and `WarmConfig` (§18).

| Key | Type | Default |
| --- | --- | --- |
| `device_id` | `u32` | `0` |
| `memory_limit_gb` | `f32` | `4.0` |

## 11. `[watcher]` — `WatcherConfig`

Source: `sub_configs.rs:330-363`. All fields default; entire section is `#[serde(default)]`.

| Key | Type | Default |
| --- | --- | --- |
| `enabled` | `bool` | `false` (see `Default` impl, `sub_configs.rs:357`) |
| `watch_paths` | `Vec<String>` | `["./docs"]` |
| `session_id` | `String` | `"docs-watcher"` |

`config/default.toml` exposes additional `[watcher.code]` sub-table fields (`enabled`, `watch_paths`, `session_id`, `extensions`, `use_ast_chunker`, `target_chunk_size`, `min_chunk_size`, `max_chunk_size`). These are NOT modeled in the current Rust `WatcherConfig` and are ignored by the loader; the actual code-watcher wiring is constructed elsewhere in the MCP server with hardcoded defaults.

## 12. `EmbedderConfig` and `EmbedderWeightsConfig` (per-embedder slot config)

Source: `crates/context-graph-core/src/config/embedder_config.rs`. These types are NOT mounted on the root `Config`; they are consumed by callers (search, training, weight profiles) that take an embedder list directly.

`EmbedderConfig` (one slot):

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `embedder` | `Embedder` | `Embedder::Semantic` | Deserialized via `Embedder::from_name` (accepts canonical name, e.g. `"E8_Graph"`, `"E8"`, `"graph"`, per `embedder_config.rs:22-28`). Unknown names ⇒ deserialization error. |
| `weight` | `f32` | `1.0` | Range not enforced here; downstream fusion clamps. |
| `enabled` | `bool` | `true` | none |
| `model_path` | `Option<String>` | `None` | none |

`check_deprecated_names` returns a `DeprecatedNameUsage` warning summary for any legacy aliases.

## 13. `EmbeddingConfig` (embedding subsystem root, NOT the `embedding.*` table)

Source: `crates/context-graph-embeddings/src/config/mod.rs`. Loaded by `EmbeddingConfig::from_file` (`:138`) or `from_toml_str` (`:272`), validated by `validate()`, env-overridable via `with_env_overrides()` (`:223-265`). NOTE: this root is distinct from `Config.embedding` (§7). The MCP server constructs both independently.

| Section | Type | Default |
| --- | --- | --- |
| `models` | `ModelPathConfig` | §14 |
| `batch` | `BatchConfig` | §15 |
| `cache` | `CacheConfig` | §16 |
| `gpu` | `GpuConfig` | §17 |

## 14. `[models]` — `ModelPathConfig`

Source: `crates/context-graph-embeddings/src/config/models.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `models_dir` | `String` | `"./models"` (`models.rs:14-16`) | non-empty (`models.rs:80-84`) |
| `lazy_loading` | `bool` | `true` (`models.rs:18-20`) | none |
| `preload_models` | `Vec<String>` | `[]` | each name normalized via lowercase + `-→_`, must match a `ModelId::all()` name; otherwise `ConfigError` (`models.rs:89-101`). Valid names (per `models.rs:49-51`): `semantic`, `temporal_recent`, `temporal_periodic`, `temporal_positional`, `causal`, `sparse`, `code`, `graph`, `hdc`, `contextual`, `entity`, `kepler`, `late_interaction`, `splade`, `bge_m3_dense`. |
| `max_loaded_models` | `usize` | `14` (`models.rs:22-24`) | `0` documented as "unlimited (all 14 production models can be loaded)" (`models.rs:57`). |

## 15. `[batch]` — `BatchConfig`

Source: `crates/context-graph-embeddings/src/config/batch.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `max_batch_size` | `usize` | `32` (`batch.rs:64-66`) | `> 0` (`batch.rs:157-161`) |
| `min_batch_size` | `usize` | `1` (`batch.rs:68-70`) | `<= max_batch_size` |
| `max_wait_ms` | `u64` | `50` (`batch.rs:72-74`) | `> 0` when `min_batch_size > 1` |
| `dynamic_batching` | `bool` | `true` (`batch.rs:76-78`) | none |
| `padding_strategy` | `PaddingStrategy` | `DynamicMax` (`batch.rs:26-27`) | enum: `max_length`, `dynamic_max`, `power_of_two`, `bucket` (`batch.rs:18-36`) |
| `sort_by_length` | `bool` | `true` (`batch.rs:80-82`) | none |

## 16. `[cache]` — `CacheConfig`

Source: `crates/context-graph-embeddings/src/config/cache.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` (`cache.rs:66-68`) | none |
| `max_entries` | `usize` | `100_000` (`cache.rs:70-72`) | `> 0` when `enabled` (`cache.rs:158-162`) |
| `max_bytes` | `usize` | `1_073_741_824` (1GB) (`cache.rs:74-76`) | `> 0` when `enabled` (`cache.rs:163-167`) |
| `ttl_seconds` | `Option<u64>` | `None` | none |
| `eviction_policy` | `EvictionPolicy` | `Lru` (`cache.rs:24-25`) | enum: `lru`, `lfu`, `ttl_lru`, `arc` (`cache.rs:21-38`) |
| `persist_to_disk` | `bool` | `false` | none |
| `disk_path` | `Option<PathBuf>` | `None` | required when `persist_to_disk` is true (`cache.rs:168-172`) |

## 17. `[gpu]` — `GpuConfig`

Source: `crates/context-graph-embeddings/src/config/gpu.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` (`gpu.rs:13-15`) | none |
| `device_ids` | `Vec<u32>` | `[0]` (`gpu.rs:17-19`) | non-empty when `enabled` (`gpu.rs:115-119`) |
| `memory_fraction` | `f32` | `0.9` (`gpu.rs:21-23`) | `(0.0, 1.0]`, finite (`gpu.rs:120-130`) |
| `use_cuda_graphs` | `bool` | `true` (`gpu.rs:25-27`) | none |
| `mixed_precision` | `bool` | `true` (`gpu.rs:29-31`) | none |
| `green_contexts` | `bool` | `false` (default via `#[serde(default)]`, `gpu.rs:82`) | none |
| `gds_enabled` | `bool` | `false` (default via `#[serde(default)]`, `gpu.rs:89`) | none |

## 18. `WarmConfig` (warm-loading subsystem)

Source: `crates/context-graph-embeddings/src/warm/config.rs`. Loaded via `WarmConfig::from_env()` (`:155`) or `WarmConfig::default()` (`:134-145`). Validated by `validate()` (`:215-232`).

| Field | Type | Default | Env var |
| --- | --- | --- | --- |
| `vram_budget_bytes` | `usize` | `25_769_803_776` (24GB; `24 * GB` at `config.rs:137`) | `WARM_VRAM_BUDGET_BYTES` |
| `vram_headroom_bytes` | `usize` | `8_589_934_592` (8GB; `8 * GB`) | `WARM_VRAM_HEADROOM_BYTES` |
| `model_weights_path` | `PathBuf` | `"./models"` | `WARM_MODEL_WEIGHTS_PATH` |
| `diagnostic_dump_path` | `PathBuf` | `"/var/log/context-graph"` | `WARM_DIAGNOSTIC_DUMP_PATH` |
| `cuda_device_id` | `u32` | `0` | `CUDA_VISIBLE_DEVICES` (first comma-segment, `config.rs:178-185`) |
| `enable_test_inference` | `bool` | `true` | `WARM_ENABLE_TEST_INFERENCE` (`true`/`1`/`yes`/`on`) |
| `max_load_time_per_model_ms` | `u64` | `30_000` | `WARM_MAX_LOAD_TIME_MS` |
| `quantization` | `QuantizationMode` | `Fp16` | not env-controlled |

`QuantizationMode` variants: `Fp32` (1.0×), `Fp16` (0.5×), `Fp8` (0.25×).

`validate()` rejects when `vram_budget_bytes == 0` or when `model_weights_path` does not exist (`config.rs:218-230`).

## 19. Phase-Aware Validation

Implemented in `Config::validate_phase_safety` (`mod.rs:159-260`). Production REJECTS:

| Setting | Required production value |
| --- | --- |
| `embedding.model` | not `"stub"` |
| `storage.backend` | not `"memory"` |
| `index.backend` | not `"memory"` |
| `utl.mode` | not `"stub"` |

`Phase::Ghost` and `Phase::Development` allow stubs but emit a multi-line stderr banner UNLESS `CONTEXT_GRAPH_MCP_QUIET` is set (`mod.rs:193`). `Phase::Production` produces a multi-line `ConfigError` listing every violation and remediation steps (`mod.rs:234-256`).

`Config::uses_stubs()` (`mod.rs:265-270`) and `Config::is_production_safe()` (`mod.rs:275-277`) provide helpers used elsewhere.

## 20. Environment Variables (Complete)

### 20.1 Core (read by `Config::load` + MCP main)

| Variable | Effect | Read by |
| --- | --- | --- |
| `CONTEXT_GRAPH_ENV` | Selects `config/{env}.toml` | `mod.rs:67` |
| `CONTEXT_GRAPH__*` (double-underscore separator) | Generic config override (e.g. `CONTEXT_GRAPH__MCP__BIND_ADDRESS`) | `config` crate `Environment::with_prefix` (`mod.rs:72`) |
| `CONTEXT_GRAPH_TRANSPORT` | Override `mcp.transport` (rejects `sse` at `main.rs:315`) | `main.rs:310, 388` |
| `CONTEXT_GRAPH_TCP_PORT` | Parses `u16`, sets `mcp.tcp_port` | `main.rs:368-373` |
| `CONTEXT_GRAPH_BIND_ADDRESS` | Sets `mcp.bind_address` | `main.rs:379-382` |
| `CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT` / `CONTEXT_GRAPH__MCP__REQUEST_TIMEOUT` | Sets `mcp.request_timeout` (positive integer seconds) | `main.rs:395-414` |
| `CONTEXT_GRAPH_STORAGE_PATH` | Sets `storage.path`; flips `storage.backend` from `memory` to `rocksdb` | `main.rs:418-426`, `server/mod.rs:1193` |
| `CONTEXT_GRAPH_MODELS_PATH` | Flips `embedding.model` from `stub` to `e5-large-v2`; the actual path is consumed by `ProductionMultiArrayProvider` | `main.rs:430-439`, `server/mod.rs:1245`, `global_provider.rs:88` |
| `CONTEXT_GRAPH_WATCHER_ENABLED` | `1`/`true` enables watcher | `main.rs:455-459` |
| `CONTEXT_GRAPH_WARM_FIRST` | `0`/`false` disables blocking warm-up; default true | `main.rs:487-496` |
| `CONTEXT_GRAPH_DAEMON` | `1`/`true` enables daemon mode (otherwise standalone) | `main.rs:516-525` |
| `CONTEXT_GRAPH_DAEMON_PORT` | Daemon listening port | `main.rs:539-543` |
| `CONTEXT_GRAPH_MCP_QUIET` | Suppresses stderr phase warnings | `mod.rs:193` |
| `CLAUDE_SESSION_ID` | Used by handlers to tag session-bound memories | `handlers/core/handlers.rs:354,367` |

### 20.2 Embedding subsystem (`EmbeddingConfig::with_env_overrides`)

Source: `crates/context-graph-embeddings/src/config/mod.rs:223-265`.

| Variable | Field overridden | Parse |
| --- | --- | --- |
| `EMBEDDING_MODELS_DIR` | `models.models_dir` | string |
| `EMBEDDING_LAZY_LOADING` | `models.lazy_loading` | `bool` |
| `EMBEDDING_GPU_ENABLED` | `gpu.enabled` | `bool` |
| `EMBEDDING_CACHE_ENABLED` | `cache.enabled` | `bool` |
| `EMBEDDING_CACHE_MAX_ENTRIES` | `cache.max_entries` | `usize` |
| `EMBEDDING_BATCH_MAX_SIZE` | `batch.max_batch_size` | `usize` |
| `EMBEDDING_BATCH_MAX_WAIT_MS` | `batch.max_wait_ms` | `u64` |

### 20.3 Warm-loading subsystem

See §18.

### 20.4 Diagnostics

| Variable | Effect |
| --- | --- |
| `HOSTNAME` / `COMPUTERNAME` | `WarmDiagnostics::hostname` |

### 20.5 Reality-loop binary

`crates/context-graph-cli/src/bin/reality-loop.rs` reads NO environment variables. All configuration is via CLI flags (see §22).

## 21. CLI Args (`crates/context-graph-mcp/src/main.rs`)

`apply_overrides` (`main.rs:363-462`) lets these CLI flags supersede env and config:

| Flag | Field overridden |
| --- | --- |
| `--port <N>` | `mcp.tcp_port` |
| `--bind-address <ADDR>` | `mcp.bind_address` |
| `--transport <MODE>` | `mcp.transport` (stdio/tcp; sse rejected) |
| `--no-warm` | sets `warm_first = false` |
| `--warm-first` | sets `warm_first = true` (default) |
| `--daemon` | enables daemon mode |
| `--daemon-port <N>` | sets daemon TCP port (default `3100`) |
| `--config <PATH>` | path to alternate config TOML |
| `--daemon-server-only` | hidden flag — current process IS the headless daemon |

Priority for transport selection (`main.rs::determine_transport_mode`, `:283-357`): CLI > `CONTEXT_GRAPH_TRANSPORT` > `mcp.transport` config field. Priority for warm-first (`:479-501`): CLI `--no-warm` > `CONTEXT_GRAPH_WARM_FIRST` > default `true`. Priority for daemon mode (`:508-529`): CLI `--daemon` > `CONTEXT_GRAPH_DAEMON` > default `false`. Priority for daemon port (`:532-547`): CLI `--daemon-port` (only when explicitly set) > `CONTEXT_GRAPH_DAEMON_PORT` > default `3100`.

## 22. CLI Args (`crates/context-graph-cli/src/bin/reality-loop.rs`)

clap-derived `Args` struct (`reality-loop.rs:22-71`). All defaults are constants at the top of the file (`:12-20`). The binary takes no env vars.

| Flag | Type | Default | Required? |
| --- | --- | --- | --- |
| `--task-manifest <PATH>` | `PathBuf` | — | **Required** |
| `--model <NAME>` | `String` | `"gemma4:e4b"` | no |
| `--runtime-root <PATH>` | `PathBuf` | `"/tmp/contextgraph-rust-reality-loop"` | no |
| `--ollama-url <URL>` | `String` | `"http://127.0.0.1:11434"` | no |
| `--context-graph-bin <PATH>` | `PathBuf` | `"target/debug/context-graph"` | no |
| `--max-attempts <N>` | `u64` | `1` | no; must be ≥ 1 |
| `--run-until-resolved` | `bool` flag | `false` | hard-rejected unless `--official-eval-each-attempt` is also set |
| `--official-eval-each-attempt` | `bool` flag | `false` | required for any SWE reality run |
| `--swebench-python <PATH>` | `PathBuf` | `"/home/cabdru/.cache/contextgraph-swebench/venv/bin/python"` | no |
| `--official-timeout <SEC>` | `u64` | `900` | no |
| `--official-max-workers <N>` | `u64` | `1` | no |
| `--official-cache-level <STR>` | `String` | `"env"` | no |
| `--official-docker-host <URI>` | `String` | `"unix:///var/run/docker.sock"` | no |
| `--official-docker-config <PATH>` | `PathBuf` | `"/home/cabdru/.cache/contextgraph-docker-config"` | no |
| `--gpu-index <N>` | `u64` | `0` | no |
| `--gpu-min-free-mb <N>` | `u64` | `8192` | no |
| `--gpu-max-used-mb <N>` | `Option<u64>` | none | no |
| `--ollama-keep-alive <DUR>` | `String` | `"30m"` | no |
| `--model-timeout <SEC>` | `u64` | `300` | no |
| `--git-timeout <SEC>` | `u64` | `600` | no |
| `--sleep-between-attempts-ms <MS>` | `u64` | `0` | no |

Hard contract checks (typed `RealityError` exit code 1):

| Error code | Condition |
| --- | --- |
| `RUST_REALITY_MAX_ATTEMPTS_INVALID` | `--max-attempts < 1` |
| `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL` | `--run-until-resolved` without `--official-eval-each-attempt` |
| `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` | any SWE run without `--official-eval-each-attempt` |
| `RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING` | `--context-graph-bin` does not exist |
| `RUST_REALITY_TASK_MANIFEST_MISSING` | `--task-manifest` does not exist |
| `RUST_REALITY_PATCH_TOUCHES_BENCHMARK` | candidate patch touches benchmark/test/fixture/harness paths |

Other contract violations (per commit `cc8e6c33`): manifest dataset ≠ `princeton-nlp/SWE-bench_Lite`, malformed instance id, repo not `owner/repo`, base_commit not 40-char SHA, empty problem_statement, missing FAIL_TO_PASS or PASS_TO_PASS metadata, `task.source_of_truth` missing dataset+instance binding, dirty git worktree, base_commit ≠ git HEAD, official report does not classify the target instance.

## 23. Override Chain Side Effects

`apply_overrides` (`main.rs:363-462`) introduces these CHAIN-EFFECT flips that occur AFTER initial `Config::load`:

1. `CONTEXT_GRAPH_STORAGE_PATH` set ⇒ `storage.path = $val`; if `storage.backend == "memory"` then `backend = "rocksdb"` (`main.rs:418-426`).
2. `CONTEXT_GRAPH_MODELS_PATH` set ⇒ if `embedding.model == "stub"` then `embedding.model = "e5-large-v2"` (`main.rs:430-439`).
3. `storage.backend == "rocksdb"` AND `index.backend == "memory"` ⇒ `index.backend = "hnsw"` (`main.rs:443-446`).
4. `storage.backend == "rocksdb"` AND `utl.mode == "stub"` ⇒ `utl.mode = "production"` (`main.rs:449-451`).

These flips happen BEFORE phase validation, so a Production phase combined with `CONTEXT_GRAPH_STORAGE_PATH` and `CONTEXT_GRAPH_MODELS_PATH` will pass the stub check.

## 24. DynamicJEPA Bundles

Located under `configs/dynamicjepa/` (not loaded by `Config::load`; consumed directly by `context-graph-cli dynamicjepa` admin commands and the DynamicJEPA service):

| Path | Format | Role |
| --- | --- | --- |
| `configs/dynamicjepa/domain_packs/counter_world.v1.toml` | TOML | Counter-world domain pack v1 |
| `configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml` | TOML | Gridworld 5x5 domain pack v1 |
| `configs/dynamicjepa/domain_packs/career_taxonomy.v1.toml` | TOML | Career taxonomy domain pack v1 |
| `configs/dynamicjepa/domain_packs/counter_to_grid.v1.toml` | TOML | Cross-domain transfer pack v1 |
| `configs/dynamicjepa/verification/{counter_world,gridworld_5x5,career_taxonomy}_*.jsonl` | JSONL | Happy-path + edge-case verification fixtures |
| `configs/dynamicjepa/verification/{tiny_train_config,gridworld_5x5_train_config,career_taxonomy_train_config}.json` | JSON | Tiny-model training configs |
| `configs/dynamicjepa/verification/career_taxonomy_data_manifest.json` | JSON | Career taxonomy data manifest |

## 25. What Is NOT Covered Here

- Per-CF RocksDB write-buffer/block-size/prefix-extractor profiles: these are hard-coded per CF, see [04_database_schema.md](04_database_schema.md).
- `LearnerProfile`, `ConstellationSelector`, `WeightProfile` and other RUNTIME records that look like config but live in the database (see [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md)).
- Causal/topic gate thresholds (`CAUSAL_THRESHOLD`, `NON_CAUSAL_THRESHOLD`, `CAUSAL_BOOST`, `CAUSAL_DEMOTION`, topic emergence ≥ 2.5): hard-coded constants referenced in [06_search_and_fusion.md](06_search_and_fusion.md) and [07_graph_and_causal.md](07_graph_and_causal.md).
- Test config files under `crates/*/tests`: scope-limited fixtures, not part of the production load chain.
- `gemmafrontend/` Python configuration: the evidence console takes `--host`, `--port`, `--runtime-root` CLI args and reads `GEMMAFRONTEND_RUNTIME_ROOT` env var (per `gemmafrontend/README.md`); the harness scripts (`manual_fsv.py`, `model_benchmark.py`, `swe_lite_official_eval.py`) take their own argparse flags. None of these flow into the Rust `Config`.
