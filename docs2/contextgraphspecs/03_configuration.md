# Configuration

**Source files covered:**
- `crates/context-graph-core/src/config/mod.rs`
- `crates/context-graph-core/src/config/sub_configs.rs`
- `crates/context-graph-embeddings/src/config/*.rs`
- `crates/context-graph-mcp/src/main.rs`

## 1. Load Order
`Config::load` reads optional `config/default.toml`, optional `config/{CONTEXT_GRAPH_ENV}.toml` (default env `development`), then `CONTEXT_GRAPH` env vars with `__` separator. `Config::from_file(path)` reads one TOML file directly and validates.

## 2. Core Config Keys
| Key | Type | Default | Validation |
| --- | --- | --- | --- |
| phase | Phase | ghost | ghost/development/production |
| server.name | String | context-graph | — |
| server.version | String | 0.1.0-ghost | — |
| mcp.transport | String | stdio | config accepts stdio/tcp/sse; main rejects sse |
| mcp.max_payload_size | usize | 10485760 | >0 |
| mcp.request_timeout | u64 | 30 | >0 |
| mcp.bind_address | String | 127.0.0.1 | non-empty for tcp/sse |
| mcp.tcp_port | u16 | 3100 | 1..=65535 for tcp |
| mcp.sse_port | u16 | 3101 | 1..=65535 for sse |
| mcp.max_connections | usize | 32 | >0 for tcp/sse |
| logging.level | String | info | — |
| logging.format | String | pretty | — |
| logging.include_location | bool | false | — |
| storage.backend | String | memory | production rejects `memory` |
| storage.path | String | ./data/storage | parent must exist if backend != memory |
| storage.compression | bool | true | — |
| embedding.model | String | stub | production rejects `stub` |
| index.backend | String | memory | production rejects `memory` |
| index.hnsw_m | usize | 16 | — |
| index.hnsw_ef_construction | usize | 200 | — |
| utl.mode | String | stub | production rejects `stub` |
| utl.consolidation_threshold | f32 | 0.7 | — |
| cuda.device_id | u32 | 0 | — |
| cuda.memory_limit_gb | f32 | 4.0 | — |
| watcher.enabled | bool | false | — |
| watcher.watch_paths | Vec<String> | ["./docs"] | — |
| watcher.session_id | String | docs-watcher | — |

## 3. Environment Variables
| Variable | Effect |
| --- | --- |
| CONTEXT_GRAPH_ENV | development config selector |
| CONTEXT_GRAPH__* | config crate env override |
| CONTEXT_GRAPH_TRANSPORT | stdio/tcp transport |
| CONTEXT_GRAPH_TCP_PORT | TCP port |
| CONTEXT_GRAPH_BIND_ADDRESS | bind address |
| CONTEXT_GRAPH_STORAGE_PATH | sets path; switches `memory` to `rocksdb` |
| CONTEXT_GRAPH_MODELS_PATH | switches `stub` to `e5-large-v2` |
| CONTEXT_GRAPH_WATCHER_ENABLED | 1/true enables watcher |
| CONTEXT_GRAPH_WARM_FIRST | 0/false disables blocking warmup |
| CONTEXT_GRAPH_DAEMON | 1/true enables daemon |
| CONTEXT_GRAPH_DAEMON_PORT | daemon port |
| CONTEXT_GRAPH_MCP_QUIET | suppresses phase warnings |
| EMBEDDING_MODELS_DIR | embedding `models_dir` |
| EMBEDDING_LAZY_LOADING | embedding `lazy_loading` |
| EMBEDDING_GPU_ENABLED | gpu.enabled |
| EMBEDDING_CACHE_ENABLED | cache.enabled |
| EMBEDDING_CACHE_MAX_ENTRIES | cache.max_entries |
| EMBEDDING_BATCH_MAX_SIZE | batch.max_batch_size |
| EMBEDDING_BATCH_MAX_WAIT_MS | batch.max_wait_ms |

## 4. Embedding Config Keys
| Key | Default | Validation |
| --- | --- | --- |
| models.models_dir | ./models | non-empty |
| models.lazy_loading | true | — |
| models.preload_models | [] | must match `ModelId::all` |
| models.max_loaded_models | 14 | 0 = unlimited (per comment) |
| batch.max_batch_size | 32 | >0 |
| batch.min_batch_size | 1 | <= max |
| batch.max_wait_ms | 50 | >0 if min > 1 |
| batch.padding_strategy | dynamic_max | max_length / dynamic_max / power_of_two / bucket |
| cache.enabled | true | — |
| cache.max_entries | 100000 | >0 if enabled |
| cache.max_bytes | 1073741824 | >0 if enabled |
| cache.eviction_policy | lru | lru / lfu / ttl_lru / arc |
| cache.persist_to_disk | false | disk_path required if true |
| gpu.enabled | true | — |
| gpu.device_ids | [0] | non-empty if enabled |
| gpu.memory_fraction | 0.9 | (0,1] finite |
| gpu.use_cuda_graphs | true | — |
| gpu.mixed_precision | true | — |
| gpu.green_contexts | false | — |
| gpu.gds_enabled | false | — |

## 5. Validation Rules
Production phase rejects: `embedding.model=stub`, `storage.backend=memory`, `index.backend=memory`, `utl.mode=stub`. Non-memory storage validates that the parent directory of `storage.path` exists. MCP config validates payload size, timeout, transport-specific address/port/connections.

## 6. What Is Not Covered
Checked-in TOML files contain older sections that no longer match the current Rust `Config`; behavior in code is authoritative.
