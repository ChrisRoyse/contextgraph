# Configuration

**Source files covered:**
- `crates/context-graph-core/src/config/mod.rs` — root `Config`, `Phase` enum, load order, validation.
- `crates/context-graph-core/src/config/sub_configs.rs` — `ServerConfig`, `McpConfig`, `LoggingConfig`, `StorageConfig`, `EmbeddingConfig`, `IndexConfig`, `UtlConfig`, `CudaConfig`, `WatcherConfig`.
- `crates/context-graph-core/src/config/embedder_config.rs` — `EmbedderConfig`, `EmbedderWeightsConfig`.
- `crates/context-graph-embeddings/src/config/{mod.rs,models.rs,batch.rs,cache.rs,gpu.rs}` — embedding subsystem root config.
- `crates/context-graph-embeddings/src/warm/config.rs` — `WarmConfig`, `QuantizationMode`.
- `crates/context-graph-mcp/src/main.rs` — MCP server CLI args, env overrides, override chain.
- `crates/context-graph-cli/src/bin/reality-loop.rs` — reality-loop binary CLI args (clap derive at `:76-350`).
- `config/{default,development,production,test}.toml` — checked-in TOML overlays.
- `config/ccreality_model_pins.json` — sha256-pinned inner-actor GGUF manifest.
- `config/ccreality_recommendation_schema.json` — JSON-Schema for `mcp__cgreality__optimizer_record_recommendation`.
- `config/ccreality_governed_paths.json` — legacy allow/deny path policy (no longer enforced at runtime; see §10).
- `config/reward_spec.verified_outcome.json` — reward spec read by `blackbox reward compile`.
- `configs/dynamicjepa/domain_packs/*.toml` — DynamicJEPA strict domain-pack manifests.
- `configs/dynamicjepa/verification/*.{json,jsonl}` — DynamicJEPA training configs and verification fixtures.

Cross-references:
- [01_system_overview.md](01_system_overview.md) — high-level architecture.
- [10_cli_reference.md](10_cli_reference.md) — CLI binaries that read these configs.
- [04_database_schema.md](04_database_schema.md) — physical CF/key/value layout governed by these configs.
- [09_storage_layer.md](09_storage_layer.md) — storage repository + HNSW lifecycle that consumes `storage.*`/`index.*`.
- [05_embedding_subsystem.md](05_embedding_subsystem.md) — what `embedding.*` and `EmbeddingConfig` drive.
- [11_cuda_acceleration.md](11_cuda_acceleration.md) — what `cuda.*` and `WarmConfig` drive.
- [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) — DynamicJEPA configs.
- [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md) — reality-loop measurement contract + model pins.

## 1. Load Order

`Config::load()` (`mod.rs:66-77`) layers sources, later overriding earlier:

| Step | Source | Required? |
| --- | --- | --- |
| 1 | `config/default.toml` | optional (`required(false)`) |
| 2 | `config/{CONTEXT_GRAPH_ENV}.toml` | optional; `CONTEXT_GRAPH_ENV` defaults to `"development"` (`mod.rs:67`) |
| 3 | `CONTEXT_GRAPH__*` env vars (double-underscore separator) | optional; e.g. `CONTEXT_GRAPH__MCP__BIND_ADDRESS=0.0.0.0` (`mod.rs:72`) |
| 4 | `apply_overrides` (`crates/context-graph-mcp/src/main.rs:379-478`) | runtime; CLI flags + dedicated env vars |
| 5 | `Config::validate()` (`mod.rs:126-147`) | enforced; calls `mcp.validate()`, parent-dir check, `validate_phase_safety()` |

`Config::from_file(path)` (`mod.rs:96-110`) reads exactly one TOML via `toml::from_str` (no env layering) and validates. The checked-in TOML files in `config/` reference legacy section names (`[general]`, `[server.transport]`, `[storage].sqlite_path`, `[embeddings]`, `[graph]`) that do NOT match the current Rust `Config` shape — fields not present in the Rust types are silently ignored by serde. The Rust types in `sub_configs.rs` are authoritative.

## 2. Top-Level `Config` Keys

Defined in `mod.rs:42-57`. All sections except `phase` and `watcher` are required at the type level (their `Default` impls supply values).

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `phase` | `Phase` enum | `Phase::Ghost` | Serde `lowercase`: `ghost` \| `development` \| `production` (mod.rs:28-38) |
| `server` | `ServerConfig` | §3 | none |
| `mcp` | `McpConfig` | §4 | `McpConfig::validate()` (mod.rs:129) |
| `logging` | `LoggingConfig` | §5 | none |
| `storage` | `StorageConfig` | §6 | parent of `path` must exist when `backend != "memory"` (mod.rs:131-141) |
| `embedding` | `EmbeddingConfig` (core) | §7 | phase safety (mod.rs:159-260) |
| `index` | `IndexConfig` | §8 | phase safety |
| `utl` | `UtlConfig` | §9 | phase safety |
| `cuda` | `CudaConfig` | §10 | none |
| `watcher` | `WatcherConfig` | disabled | none |

## 3. `[server]` — `ServerConfig`

Source: `sub_configs.rs:9-22`.

| Key | Type | Default |
| --- | --- | --- |
| `name` | `String` | `"context-graph"` |
| `version` | `String` | `"0.1.0-ghost"` |

## 4. `[mcp]` — `McpConfig`

Source: `sub_configs.rs:24-214`. All fields have serde defaults; all are validated by `McpConfig::validate()`.

| Key | Type | Default | Valid range | Validation |
| --- | --- | --- | --- | --- |
| `transport` | `String` | `"stdio"` | `"stdio"` \| `"tcp"` \| `"sse"` (case-insensitive) | Other values: `ConfigError` (`sub_configs.rs:137`). Note: `crates/context-graph-mcp/src/main.rs:307,331,355` REJECTS `sse` even if config validation accepted it. |
| `max_payload_size` | `usize` | `10_485_760` (10 MB) | `> 0` | Zero ⇒ `ConfigError` (`sub_configs.rs:144-148`) |
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
| `backend` | `String` | `"memory"` | `Phase::Production` REJECTS `"memory"` (`mod.rs:170-176`). `apply_overrides` flips `"memory"` → `"rocksdb"` when `CONTEXT_GRAPH_STORAGE_PATH` is set (`main.rs:434-442`). |
| `path` | `String` | `"./data/storage"` | When `backend != "memory"`, parent of path must exist (`mod.rs:131-141`). Single-writer flock at `<path>/mcp.pid`. |
| `compression` | `bool` | `true` | none |

## 7. `[embedding]` — Core `EmbeddingConfig`

Source: `sub_configs.rs:252-264`. This is a separate, smaller type than the embeddings-crate `EmbeddingConfig` in §13.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `model` | `String` | `"stub"` | `Phase::Production` REJECTS `"stub"` (`mod.rs:163-167`). `apply_overrides` flips `"stub"` → `"e5-large-v2"` when `CONTEXT_GRAPH_MODELS_PATH` is set (`main.rs:446-455`). |

## 8. `[index]` — `IndexConfig`

Source: `sub_configs.rs:266-282`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `backend` | `String` | `"memory"` | `Phase::Production` REJECTS `"memory"` (`mod.rs:177-182`). `apply_overrides` flips `"memory"` → `"hnsw"` when `storage.backend == "rocksdb"` (`main.rs:459-462`). |
| `hnsw_m` | `usize` | `16` | none enforced at config layer |
| `hnsw_ef_construction` | `usize` | `200` | none enforced at config layer |

Per-embedder HNSW M/ef defaults are NOT taken from `index.hnsw_m`/`index.hnsw_ef_construction`. They are hard-coded per `EmbedderIndex` in `crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs` and consumed by the storage layer.

## 9. `[utl]` — `UtlConfig`

Source: `sub_configs.rs:284-298`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `mode` | `String` | `"stub"` | `Phase::Production` REJECTS `"stub"` (`mod.rs:184-189`). `apply_overrides` flips `"stub"` → `"production"` when `storage.backend == "rocksdb"` (`main.rs:465-468`). |
| `consolidation_threshold` | `f32` | `0.7` | none enforced |

## 10. `[cuda]` — `CudaConfig`

Source: `sub_configs.rs:300-314`. The embeddings crate has its own `GpuConfig` (§17) and `WarmConfig` (§18).

| Key | Type | Default |
| --- | --- | --- |
| `device_id` | `u32` | `0` |
| `memory_limit_gb` | `f32` | `4.0` |

## 11. `[watcher]` — `WatcherConfig`

Source: `sub_configs.rs:330-363`. Entire section is `#[serde(default)]`.

| Key | Type | Default |
| --- | --- | --- |
| `enabled` | `bool` | `false` |
| `watch_paths` | `Vec<String>` | `["./docs"]` |
| `session_id` | `String` | `"docs-watcher"` |

`config/default.toml` exposes additional `[watcher.code]` sub-table fields. These are NOT modeled in the current Rust `WatcherConfig` and are silently ignored.

## 12. `EmbedderConfig` and `EmbedderWeightsConfig` (per-embedder slot)

Source: `crates/context-graph-core/src/config/embedder_config.rs`. NOT mounted on the root `Config`; consumed by callers (search, training, weight profiles).

`EmbedderConfig` (one slot):

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `embedder` | `Embedder` | `Embedder::Semantic` | Deserialized via `Embedder::from_name` (canonical, e.g. `"E8_Graph"`, `"E8"`, `"graph"`). Unknown ⇒ deserialization error. |
| `weight` | `f32` | `1.0` | Range not enforced here; downstream fusion clamps. |
| `enabled` | `bool` | `true` | none |
| `model_path` | `Option<String>` | `None` | none |

`check_deprecated_names` returns a `DeprecatedNameUsage` warning summary for legacy aliases.

## 13. `EmbeddingConfig` (embedding subsystem root)

Source: `crates/context-graph-embeddings/src/config/mod.rs`. Loaded via `EmbeddingConfig::from_file`, validated by `validate()`, env-overridable via `with_env_overrides()` (`mod.rs:223-265`). Distinct from `Config.embedding` (§7).

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
| `models_dir` | `String` | `"./models"` | non-empty (`models.rs:80-84`) |
| `lazy_loading` | `bool` | `true` | none |
| `preload_models` | `Vec<String>` | `[]` | each name normalized via lowercase + `-→_`, must match a `ModelId::all()` name; otherwise `ConfigError`. Valid names: `semantic`, `temporal_recent`, `temporal_periodic`, `temporal_positional`, `causal`, `sparse`, `code`, `graph`, `hdc`, `contextual`, `entity`, `kepler`, `late_interaction`, `splade`, `bge_m3_dense`. |
| `max_loaded_models` | `usize` | `14` | `0` documented as "unlimited". |

## 15. `[batch]` — `BatchConfig`

Source: `crates/context-graph-embeddings/src/config/batch.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `max_batch_size` | `usize` | `32` | `> 0` |
| `min_batch_size` | `usize` | `1` | `<= max_batch_size` |
| `max_wait_ms` | `u64` | `50` | `> 0` when `min_batch_size > 1` |
| `dynamic_batching` | `bool` | `true` | none |
| `padding_strategy` | `PaddingStrategy` | `DynamicMax` | enum: `max_length`, `dynamic_max`, `power_of_two`, `bucket` |
| `sort_by_length` | `bool` | `true` | none |

## 16. `[cache]` — `CacheConfig`

Source: `crates/context-graph-embeddings/src/config/cache.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | none |
| `max_entries` | `usize` | `100_000` | `> 0` when `enabled` |
| `max_bytes` | `usize` | `1_073_741_824` (1 GB) | `> 0` when `enabled` |
| `ttl_seconds` | `Option<u64>` | `None` | none |
| `eviction_policy` | `EvictionPolicy` | `Lru` | enum: `lru`, `lfu`, `ttl_lru`, `arc` |
| `persist_to_disk` | `bool` | `false` | none |
| `disk_path` | `Option<PathBuf>` | `None` | required when `persist_to_disk = true` |

## 17. `[gpu]` — `GpuConfig`

Source: `crates/context-graph-embeddings/src/config/gpu.rs`.

| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | none |
| `device_ids` | `Vec<u32>` | `[0]` | non-empty when `enabled` |
| `memory_fraction` | `f32` | `0.9` | `(0.0, 1.0]`, finite |
| `use_cuda_graphs` | `bool` | `true` | none |
| `mixed_precision` | `bool` | `true` | none |
| `green_contexts` | `bool` | `false` | none |
| `gds_enabled` | `bool` | `false` | none |

## 18. `WarmConfig` (warm-loading subsystem)

Source: `crates/context-graph-embeddings/src/warm/config.rs`. Loaded via `WarmConfig::from_env()` (`:155`) or `WarmConfig::default()` (`:134-145`). Validated by `validate()` (`:215-232`).

| Field | Type | Default | Env var |
| --- | --- | --- | --- |
| `vram_budget_bytes` | `usize` | `25_769_803_776` (24 GB) | `WARM_VRAM_BUDGET_BYTES` |
| `vram_headroom_bytes` | `usize` | `8_589_934_592` (8 GB) | `WARM_VRAM_HEADROOM_BYTES` |
| `model_weights_path` | `PathBuf` | `"./models"` | `WARM_MODEL_WEIGHTS_PATH` |
| `diagnostic_dump_path` | `PathBuf` | `"/var/log/context-graph"` | `WARM_DIAGNOSTIC_DUMP_PATH` |
| `cuda_device_id` | `u32` | `0` | `CUDA_VISIBLE_DEVICES` (first comma-segment) |
| `enable_test_inference` | `bool` | `true` | `WARM_ENABLE_TEST_INFERENCE` |
| `max_load_time_per_model_ms` | `u64` | `30_000` | `WARM_MAX_LOAD_TIME_MS` |
| `quantization` | `QuantizationMode` | `Fp16` | not env-controlled |

`QuantizationMode` variants: `Fp32` (1.0×), `Fp16` (0.5×), `Fp8` (0.25×). `validate()` rejects when `vram_budget_bytes == 0` or `model_weights_path` does not exist.

## 19. Phase-Aware Validation

Implemented in `Config::validate_phase_safety` (`mod.rs:159-260`). Production REJECTS:

| Setting | Required production value |
| --- | --- |
| `embedding.model` | not `"stub"` |
| `storage.backend` | not `"memory"` |
| `index.backend` | not `"memory"` |
| `utl.mode` | not `"stub"` |

`Phase::Ghost` and `Phase::Development` allow stubs but emit a multi-line stderr banner UNLESS `CONTEXT_GRAPH_MCP_QUIET` is set (`mod.rs:193`). `Phase::Production` produces a multi-line `ConfigError` listing every violation. `Config::uses_stubs()` and `Config::is_production_safe()` are public helpers.

## 20. Override Chain Side Effects

`apply_overrides` (`crates/context-graph-mcp/src/main.rs:379-478`) introduces these CHAIN-EFFECT flips that occur AFTER initial `Config::load`:

1. `CONTEXT_GRAPH_STORAGE_PATH` set ⇒ `storage.path = $val`; if `storage.backend == "memory"` then `backend = "rocksdb"` (`main.rs:434-442`).
2. `CONTEXT_GRAPH_MODELS_PATH` set ⇒ if `embedding.model == "stub"` then `embedding.model = "e5-large-v2"` (`main.rs:446-455`).
3. `storage.backend == "rocksdb"` AND `index.backend == "memory"` ⇒ `index.backend = "hnsw"` (`main.rs:459-462`).
4. `storage.backend == "rocksdb"` AND `utl.mode == "stub"` ⇒ `utl.mode = "production"` (`main.rs:465-468`).

These flips happen BEFORE phase validation, so `Phase::Production` combined with both env vars passes the stub check.

## 21. Environment Variables (Complete Inventory)

### 21.1 Core MCP server

Read by `Config::load` and `apply_overrides`.

| Variable | Effect | Read at |
| --- | --- | --- |
| `CONTEXT_GRAPH_ENV` | Selects `config/{env}.toml` | `mod.rs:67` |
| `CONTEXT_GRAPH__*` (double-underscore) | Generic config override (e.g. `CONTEXT_GRAPH__MCP__BIND_ADDRESS`) | `mod.rs:72` |
| `CONTEXT_GRAPH_TRANSPORT` | Override `mcp.transport` (rejects `sse`) | `main.rs:326,404` |
| `CONTEXT_GRAPH_TCP_PORT` | Sets `mcp.tcp_port` (parses `u16`) | `main.rs:384` |
| `CONTEXT_GRAPH_BIND_ADDRESS` | Sets `mcp.bind_address` | `main.rs:395` |
| `CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT` / `CONTEXT_GRAPH__MCP__REQUEST_TIMEOUT` | Sets `mcp.request_timeout` (positive integer seconds) | `main.rs:411-430` |
| `CONTEXT_GRAPH_STORAGE_PATH` | Sets `storage.path`; flips `storage.backend = rocksdb` | `main.rs:434`, `server/mod.rs:1193` |
| `CONTEXT_GRAPH_MODELS_PATH` | Flips `embedding.model = e5-large-v2`; consumed by `ProductionMultiArrayProvider` | `main.rs:446`, `server/mod.rs:1245` |
| `CONTEXT_GRAPH_WATCHER_ENABLED` | `1`/`true` enables file watcher | `main.rs:471-475` |
| `CONTEXT_GRAPH_WARM_FIRST` | `0`/`false` disables blocking warm-up; default true | `main.rs:503-512` |
| `CONTEXT_GRAPH_DAEMON` | `1`/`true` enables daemon mode | `main.rs:532-541` |
| `CONTEXT_GRAPH_DAEMON_PORT` | Daemon listening port (parses `u16`) | `main.rs:555-559` |
| `CONTEXT_GRAPH_MCP_QUIET` | Suppresses stderr phase warnings | `mod.rs:193` |
| `CLAUDE_SESSION_ID` | Used by handlers to tag session-bound memories | `handlers/core/handlers.rs:354,367` |

### 21.2 Embedding subsystem

`EmbeddingConfig::with_env_overrides` (`crates/context-graph-embeddings/src/config/mod.rs:223-265`).

| Variable | Field overridden | Parse |
| --- | --- | --- |
| `EMBEDDING_MODELS_DIR` | `models.models_dir` | string |
| `EMBEDDING_LAZY_LOADING` | `models.lazy_loading` | `bool` |
| `EMBEDDING_GPU_ENABLED` | `gpu.enabled` | `bool` |
| `EMBEDDING_CACHE_ENABLED` | `cache.enabled` | `bool` |
| `EMBEDDING_CACHE_MAX_ENTRIES` | `cache.max_entries` | `usize` |
| `EMBEDDING_BATCH_MAX_SIZE` | `batch.max_batch_size` | `usize` |
| `EMBEDDING_BATCH_MAX_WAIT_MS` | `batch.max_wait_ms` | `u64` |

### 21.3 Warm-loading

See §18.

### 21.4 ccreality / reality-loop

| Variable | Effect | Read at |
| --- | --- | --- |
| `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` | Per-attempt wallclock deadline; default `7200` (120 min) | `crates/context-graph-cli/src/bin/reality-loop.rs:4334` |
| `CGREALITY_GOVERNED_PATHS` | Path to allow/deny policy JSON; if missing, the MCP path-policy module rejects with a structured error. The deny gate's enforcement was removed 2026-05-07 (operator directive); the file is still parsed for non-empty/no-control-character path validation. | `crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs:40`; `crates/context-graph-cli/src/governed_edit.rs:565` |
| `CGREALITY_RECOMMENDATION_SCHEMA` | Path to `ccreality_recommendation_schema.json`; defaults to `config/ccreality_recommendation_schema.json` if unset. | `crates/context-graph-mcp/src/handlers/tools/reality_loop/schema_validator.rs:13` |
| `CGREALITY_REALITY_LOOP_BIN` | Override path to `target/release/reality-loop` for the MCP→CLI shim. | `crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs:534` |
| `CGREALITY_MCP_BIN` | Path to `context-graph-mcp` binary for `context-graph-cli ccreality record-harness-transition`. | `crates/context-graph-cli/src/commands/ccreality/mod.rs:553` |
| `CGREALITY_CACHE_DIR` | Override the cache directory used by ccreality client commands. | `crates/context-graph-cli/src/commands/ccreality/mod.rs:571` |
| `CONTEXTGRAPH_ROOT` | Project root override read by both the MCP reality-loop helpers and the ccreality CLI. | `crates/context-graph-mcp/src/handlers/tools/reality_loop/helpers.rs:536`, `crates/context-graph-cli/src/commands/ccreality/mod.rs:508` |

The reality-loop binary itself reads ONLY `CGREALITY_PER_ATTEMPT_DEADLINE_SECS`. All other reality-loop configuration is via CLI flags (see [10_cli_reference.md §3](10_cli_reference.md)).

### 21.5 BlackBoxFix

| Variable | Effect | Read at |
| --- | --- | --- |
| `BLACKBOX_LEDGER` | Default value for `--ledger` flag on every blackbox subcommand | `crates/context-graph-cli/src/commands/blackbox.rs` (clap `env=` attribute on each `ledger: PathBuf` field) |
| `BBX_GITHUB_TOKEN` / `GITHUB_TOKEN` | GitHub auth fallback chain | `commands/blackbox.rs:27276,27280` |
| `BBX_GITLAB_TOKEN` / `GITLAB_TOKEN` | GitLab auth fallback chain | `commands/blackbox.rs:27661,27665` |

### 21.6 CLI helpers

| Variable | Effect | Read at |
| --- | --- | --- |
| `CONTEXT_GRAPH_MCP_HOST` / `CONTEXT_GRAPH_MCP_PORT` | TCP target for `mcp_client.rs`; consumed by `context-graph-cli memory`/`hooks` | `crates/context-graph-cli/src/mcp_client.rs:228-230` |
| `CONTEXT_GRAPH_DATA_DIR` | Storage path used by `context-graph-cli watch` | `commands/watch.rs:106` |
| `USER` | Used by `dynamicjepa` to tag operator id | `commands/dynamicjepa.rs:10376` |
| `CUDA_COMPUTE_CAP` / `CUDA_HOME` / `LD_LIBRARY_PATH` | Reflected in `dynamicjepa research-smoke` provenance | `commands/dynamicjepa.rs:12276-12278` |
| `RUST_LOG` | Tracing-subscriber filter (default for all bins) | universal |

### 21.7 Diagnostics

| Variable | Effect |
| --- | --- |
| `HOSTNAME` / `COMPUTERNAME` | `WarmDiagnostics::hostname` |

## 22. ccreality Model Pin Schema

Source: `config/ccreality_model_pins.json`. Loaded by `crates/context-graph-cli/src/bin/reality-loop.rs::load_model_pin` (`:3350`) and validated by `validate_inner_model_pin` (`:3399`) and `validate_inner_model_pin_identity_substrings` (`:3446`).

```json
{
  "$schema_version": "1",
  "_doc": "Cryptographic pins for the ccreality inner-actor coding-harness model files. The reality-loop binary loads this file at start-up and refuses to load any GGUF whose size or sha256 does not match the active pin entry.",
  "models": {
    "<pin_key>": {
      "description": "...",
      "repo_id": "<huggingface repo id>",
      "filename": "<gguf filename>",
      "expected_sha256": "sha256:<hex>",
      "expected_size_bytes": 17665334432,
      "expected_min_context_tokens": 128000,
      "expected_identity_substrings": ["qwen3"]
    }
  }
}
```

| Field | Type | Required | Purpose |
| --- | --- | --- | --- |
| `$schema_version` | string `"1"` | yes | Manifest schema version |
| `models.<key>.description` | string | yes | Human-readable description |
| `models.<key>.repo_id` | string | yes | HuggingFace repo id (provenance only) |
| `models.<key>.filename` | string | yes | GGUF file basename (provenance only) |
| `models.<key>.expected_sha256` | string with `sha256:` prefix | yes | sha256 fail-closed gate; recompute deliberately when upgrading |
| `models.<key>.expected_size_bytes` | integer | yes | Byte size fail-closed gate |
| `models.<key>.expected_min_context_tokens` | integer | yes | Minimum trainable context (verified against `LlamaModel::n_ctx_train`) |
| `models.<key>.expected_identity_substrings` | array of strings | yes | Substrings that MUST appear in the model's identity readback |

The active pin key on `main` (2026-05-08) is `qwen3_coder_30b_a3b_q4_k_xl` (Qwen3-Coder-30B-A3B-Instruct, Unsloth Dynamic Q4_K_XL GGUF, ~17.67 GB). It replaced the dense Qwen3.6-27B-Q6_K_XL pin on 2026-05-08 for the CCReality cutover. Selection at runtime is via `--inner-model-pin-key` (default `qwen3_coder_30b_a3b_q4_k_xl`).

## 23. ccreality Recommendation Schema

Source: `config/ccreality_recommendation_schema.json` — JSON-Schema (draft 2020-12) at `$id` `https://contextgraph.dev/schemas/ccreality_recommendation.v1.json`. Loaded by `crates/context-graph-mcp/src/handlers/tools/reality_loop/schema_validator.rs` and enforced by `mcp__cgreality__optimizer_record_recommendation`.

Required top-level fields: `schema_version` (const `1`), `record_kind` (const `"ccreality_optimizer_recommendation"`), `claude_session_id`, `claude_model` (one of `claude-opus-4-7`, `claude-sonnet-4-6`, `claude-haiku-4-5-20251001`, `opus`, `sonnet`, `haiku`, `inherit`), `run_id`, `attempt`, `turn_number`, `status` (one of `changed`, `no_change`, `blocked`, `error`), `reason`, `source_files_changed`, `verification_commands`, `source_of_truth_readbacks` (each `^(file:|sqlite:|git:)`), `next_inner_prompt_delta`, `expected_reward_signal_delta`, `remaining_risks`, `memories_created` (uuid array), `memories_boosted` (uuid array), `harness_transitions_minted` (each `^file:`), `shift_log_excerpt`, `created_at_unix`, `source_of_truth` (`^file:`).

Conditional schema constraints (`allOf`):
- `status == "changed"` ⇒ `source_files_changed.minItems: 1`, `harness_transitions_minted.minItems: 1`, `shift_log_excerpt.minItems: 1`.
- `status == "no_change"` ⇒ `source_files_changed.maxItems: 0`, `harness_transitions_minted.maxItems: 0`.

Optional fields: `diagnosis_summary` (`failure_class`, `root_cause_hypothesis`, `evidence_paths`, `intervention_surface`), `drift_detected.files`.

## 24. ccreality Governed Paths (legacy)

Source: `config/ccreality_governed_paths.json` (allow `["**"]`, deny set covering `.env`, `swebench/**`, `tests/oracle_**`, `crates/*/src/{reward_specs,verifiers}/**`, `*.pem`, `*.key`, `**/credentials.json`, `/etc/**`, `/root/**`).

The hook gate (`scripts/cc/hook_pre_tool_use_edit.sh`) and the MCP-server path-policy gate (`crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs::ensure_path_allowed`) had their deny-list enforcement REMOVED on 2026-05-07 (operator directive: the gate was blocking legitimate edits). The MCP path check now only rejects empty paths or paths containing control characters; the deny list is no longer consulted at runtime. The file is still loaded so the parser stays in lockstep with the schema, but the deny set is now a BEHAVIORAL convention enforced by Claude Code per the rules in `CLAUDE.md §8`.

## 25. Reward Spec Schema

Source: `config/reward_spec.verified_outcome.json`. Read by `blackbox reward compile`; referenced by reality-loop `measurement_contract.immutable_boundaries.reward_spec`.

| Field | Type | Notes |
| --- | --- | --- |
| `reward_spec_schema_version` | integer (currently `1`) | schema version |
| `reward_id` | string | stable id (currently `contextgraph_verified_outcome_demo_v1`) |
| `objective` | string | human-readable objective |
| `bounds.min` / `bounds.max` | float | reward output bounds (`-10.0` … `10.0`) |
| `components` | array | each entry: `{id, feature, weight}` (signed float weight) |

Active components on main:
- `correctness` (4.0), `verification_confidence` (2.0), `risk_penalty` (-2.0), `incident_penalty` (-3.0), `policy_missing_penalty` (-1.0), `training_value` (1.0).
- DynamicJEPA augmentation: `dj_surprise_rate_penalty` (`feature: dynamicjepa_trained_surprise_rate`, weight `-0.5`), `dj_prediction_accuracy_bonus` (`feature: dynamicjepa_prediction_accuracy`, weight `1.0`), `dj_plan_trace_bonus` (`feature: dynamicjepa_trained_plan_trace_coverage`, weight `0.5`).

## 26. DynamicJEPA Domain Pack TOML Schema

Located under `configs/dynamicjepa/domain_packs/`. Each TOML file is a strict domain-pack manifest registered into the `dj_domain_packs` column family by `context-graph-cli dynamicjepa register-domain --file <path>`. Active packs: `counter_world.v1.toml`, `gridworld_5x5.v1.toml`, `career_taxonomy.v1.toml`, `counter_to_grid.v1.toml`, `contextgraph_swe_reality_loop.v1.toml`.

Top-level keys (per `contextgraph_swe_reality_loop.v1.toml`):

| Key | Required | Purpose |
| --- | --- | --- |
| `[domain]` | yes | `id`, `version`, `title`, `schema_version` (currently `1`) |
| `[time]` | yes | Time field descriptor: `field = { name, kind, required, min }` |
| `[[state.fields]]` | yes | One row per state field: `name`, `kind` (`{type=...}` either `i64` or `categorical { variants = [...] }`), `required`, optional `min`/`max` |
| `[[action.fields]]` | yes | Same shape as state.fields; describes the action vector |
| `[[outcome.fields]]` | yes | Same shape; describes the observed outcome |
| `[[entity.fields]]` | yes | Entity identifier columns (e.g. `trajectory_id`, `run_id`) |
| `[[instruments]]` | yes | Per-instrument schema: `id`, `kind` (`scalar`, `categorical`, `time`), `input_fields`, `output_shape`, `normalization`, `required`, `pair_kinds`, `version`, `[instruments.validation]` (`require_finite`, `reject_nan`, `reject_inf`, optional `min`, `max`) |
| `[[adapters]]` | yes | Adapter binding: `id`, `kind` (e.g. `json_event`), `version`, `[adapters.mapping]` of state/action/outcome/entity field names ⇒ JSONPath expressions |
| `[[objectives]]` | yes | Training objective: `id`, `kind` (e.g. `latent_delta`), `input_panel`, `target`, `loss_weight`, `required_dataset_fields` |
| `[[invariants]]` | yes | Logical invariants over fields: `id`, `expression`, `severity` (`fatal`/`warn`) |
| `[dataset_policy]` | yes | `default_split`, `default_objective`, `negative_sampling`, `min_negatives_per_row` |
| `[planner_policy]` | yes | `candidate_actions = { kind="enumerated", records = [...] }`, `guards`, `surprise_threshold` |
| `[constellation.global]` | yes | `modalities`, `require_loo_min`, `calibration_percentile`, `calibration_set_size` |
| `[verification_policy]` | yes | `required_cfs`, `expected_trajectory_count`, `expected_event_count`, `expected_panel_count`, `expected_dataset_row_count`, `expected_train_rows_min`/`max`, `expected_val_rows_min`/`max`, `expected_test_rows_min`/`max` |

The `contextgraph_swe_reality_loop_v1` pack defines 13 state fields, 17 action fields, 16 outcome fields, 38 instruments, 8 enumerated planner actions, and a `surprise_threshold = 0.85` for the SWE-bench reality-loop domain.

## 27. DynamicJEPA Training-Config JSON Schema

Located under `configs/dynamicjepa/verification/`. Consumed by `context-graph-cli dynamicjepa train --train-config <file>`. Active configs: `tiny_train_config.json`, `gridworld_5x5_train_config.json`, `career_taxonomy_train_config.json`, `swe_reality_loop_train_config.json`.

| Top-level key | Sub-fields | Notes |
| --- | --- | --- |
| `model.encoder` | `kind` (`mlp`), `hidden` (array of widths), `out_dim` | Latent dimensions for the encoder MLP |
| `model.predictor` | `kind`, `hidden`, `in_action_dim`, `out_dim` | Action conditioning MLP |
| `model.target_architecture` | optional string (e.g. `frozen_instrument_projection`) | Target-network architecture (newer SWE-loop config) |
| `model.ema_momentum` | float in `[0, 1]` | EMA momentum for target encoder; `0.0` ⇒ frozen-projection mode |
| `loss.latent_mse_weight` | float | Latent prediction MSE weight |
| `loss.vicreg_variance_weight` | float | VICReg variance regularizer weight |
| `loss.vicreg_covariance_weight` | float | VICReg covariance regularizer weight |
| `loss.vicreg_target_std` | float | Target stddev per latent dimension |
| `optim.kind` | `adamw` | Optimizer family |
| `optim.lr` | float | Learning rate |
| `optim.weight_decay` | float | AdamW weight decay |
| `optim.warmup_steps` | integer | Linear warmup |
| `schedule.epochs` | integer | Total epochs |
| `schedule.batch_size` | integer | Mini-batch size |
| `schedule.device` | `cuda` \| `cpu` | Backend |
| `stopping.metric` | string (e.g. `val_latent_mse`) | Early-stopping metric |
| `stopping.target` | float | Stop when metric ≤ target |
| `stopping.max_seconds` | integer | Wallclock cap |
| `seed` | integer | Deterministic seed |

## 28. DynamicJEPA Verification Fixtures

Located under `configs/dynamicjepa/verification/`:

| Path | Format | Role |
| --- | --- | --- |
| `counter_world_happy_path.jsonl` | JSONL | Counter-world happy path (`verify-counter-world`) |
| `counter_world_edge_*.jsonl` | JSONL | Counter-world edge cases (empty input, invalid action, max panel) |
| `gridworld_5x5_corpus.jsonl` | JSONL | Gridworld training corpus (`verify-gridworld`) |
| `gridworld_5x5_edge_*.jsonl` | JSONL | Gridworld edge cases (empty, invalid action, OOB state) |
| `career_taxonomy_smoke.jsonl` | JSONL | Career taxonomy happy path (`verify-career-taxonomy`) |
| `career_taxonomy_edge_*.jsonl` | JSONL | Career taxonomy edge cases (empty, invalid action, missing license, OOB job zone, wrong vector dim) |
| `career_taxonomy_data_manifest.json` | JSON | Career taxonomy data manifest |

## 29. What Is NOT Covered Here

- Per-CF RocksDB write-buffer / block-size / prefix-extractor profiles: hard-coded per CF. See [04_database_schema.md](04_database_schema.md).
- `LearnerProfile`, `ConstellationSelector`, `WeightProfile` runtime records that look like config but live in the database. See [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md).
- Causal/topic gate thresholds (`CAUSAL_THRESHOLD`, `NON_CAUSAL_THRESHOLD`, `CAUSAL_BOOST`, `CAUSAL_DEMOTION`, topic emergence ≥ 2.5): hard-coded constants. See [06_search_and_fusion.md](06_search_and_fusion.md), [07_graph_and_causal.md](07_graph_and_causal.md).
- Test-only config files under `crates/*/tests`: scope-limited fixtures, not part of the production load chain.
