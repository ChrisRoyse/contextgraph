# Source Code Map

**Source files covered:**
- `Cargo.toml` (workspace manifest)
- `crates/*/Cargo.toml` (per-crate manifests, features, internal deps)
- `crates/*/src/lib.rs` (public surface) and key sub-`mod.rs` files
- `crates/context-graph-mcp/src/main.rs` (MCP entry point)
- `crates/context-graph-cli/src/main.rs` (CLI entry point) and `crates/context-graph-cli/src/bin/{context-graph,blackbox,reality-loop}.rs`
- `crates/context-graph-mcp/src/tools/definitions/mod.rs`
- `gemmafrontend/` (Python evidence console + Python reality loop)
- `scripts/v4_2/` (34 files), `scripts/5090jepa/`, `scripts/exp2/`, `scripts/shakespeare_*` (~30 files)
- `Makefile`, `rust-toolchain.toml`

## 1. Workspace Package Dependencies
Source: `[dependencies]` of each crate's `Cargo.toml`.

| Package | Workspace path dependencies |
| --- | --- |
| context-graph-core | (root — no internal deps) |
| context-graph-cuda | context-graph-core |
| context-graph-embeddings | context-graph-core, context-graph-cuda |
| context-graph-test-utils | context-graph-core, context-graph-storage |
| context-graph-storage | context-graph-core, context-graph-embeddings (`candle` + `cuda` features), context-graph-cuda (optional, gated by `cuda` feature) |
| context-graph-causal-agent | context-graph-core, context-graph-storage, context-graph-embeddings |
| context-graph-graph-agent | context-graph-core, context-graph-storage, context-graph-embeddings, context-graph-causal-agent |
| context-graph-dynamicjepa | context-graph-core, context-graph-storage |
| context-graph-cli | context-graph-core, context-graph-storage, context-graph-dynamicjepa, context-graph-embeddings; (dev) context-graph-mcp |
| context-graph-mcp | context-graph-core, context-graph-cuda, context-graph-embeddings, context-graph-storage, context-graph-cli (default-features off), context-graph-causal-agent, context-graph-graph-agent |
| context-graph-benchmark | context-graph-core (`test-utils`), context-graph-storage; optional: context-graph-embeddings, context-graph-mcp, context-graph-graph-agent, context-graph-causal-agent |

Build order (Rust honours this automatically): core → cuda → embeddings → test-utils → storage → dynamicjepa → causal-agent → graph-agent → cli → mcp → benchmark. The MCP crate sits above CLI because MCP brings CLI in for embedded helper logic; CLI's dev-deps then re-add MCP for cross-tests, breaking the strict tree only in dev cycles.

Per-crate `.rs` file counts (source only; excludes `target/`):

| Crate | `.rs` files |
| --- | --- |
| context-graph-core | 303 |
| context-graph-embeddings | 441 |
| context-graph-storage | 130 |
| context-graph-benchmark | 140 |
| context-graph-mcp | 105 |
| context-graph-cli | 40 |
| context-graph-cuda | 18 |
| context-graph-graph-agent | 9 |
| context-graph-causal-agent | 8 |
| context-graph-dynamicjepa | 7 |
| context-graph-test-utils | 3 |
| **Workspace total** (incl. `tests/` and root) | **1,346** |

## 2. context-graph-core (`crates/context-graph-core/src/`)
Public surface declared in `lib.rs` (no `cuda` or `llm` features; only optional `test-utils`).

| Path | Purpose |
| --- | --- |
| `lib.rs` | Crate root; re-exports config, error hierarchy, types, traits, fusion, weights, graph linking, monitoring, memory, clustering, injection helpers |
| `config/{mod,sub_configs,constants,embedder_config}.rs`, `config/tests/` | TOML+env configuration (`Config`, `McpConfig`, `StorageConfig`, embedder config, validation) |
| `types/mod.rs` | `EdgeType`, `NodeId`, `ValidationError`; child modules below |
| `types/fingerprint/semantic/` (constants, model-id-bound dimensions) | `SemanticFingerprint`, `NUM_EMBEDDERS = 14` (`constants.rs:81`) |
| `types/fingerprint/teleological/types.rs` | `TeleologicalFingerprint`, `TELEOLOGICAL_VECTOR_DIM = 13`, `teleological_vector()` |
| `types/fingerprint/sparse.rs` | `SparseVector` |
| `types/memory_node/{mod,node,metadata,validation}.rs` | `Memory`, `Session`, content/metadata validators |
| `types/graph_edge/{edge,modulation,traversal}.rs` | Graph edge primitives |
| `types/{audit,causal_relationship,code_entity,discovery,file_index,source_metadata}.rs` | Audit trail, causal records, code entities, discovery records, file index entries, source metadata |
| `traits/` | Repository / store / embedding traits |
| `error/{mod,unified,sub_errors,legacy,conversions,tests}.rs` | `ContextGraphError` (unified), per-domain sub-errors, JSON-RPC code mapping |
| `embeddings/` | `Embedder` trait, provider abstraction (no model code) |
| `retrieval/{mod,query,result,executor,in_memory_executor,multi_space,aggregation,sparse_index,distance,similarity,detector,divergence,insight_annotation,teleological_query,teleological_result,config}.rs` | Search strategies, query DTOs, multi-space + RRF aggregation |
| `fusion/mod.rs` | `RRF_K`, `weighted_rrf`, `fuse_rankings`, `score_weighted_rrf`, `weighted_sum`, `normalize_minmax`, `FusionStrategy`, `EmbedderRanking`, `FusedResult` |
| `weights/mod.rs` | 20 hard-coded `WEIGHT_PROFILES` (semantic_search, causal_reasoning, code_search, fact_checking, graph_reasoning, temporal_navigation, sequence_navigation, conversation_history, category_weighted, typo_tolerant, pipeline_stage1_recall, pipeline_stage2_scoring, pipeline_full, balanced, multilingual_search, long_context, translation_finder, affect_repair, affect_priming, affect_neutral); profile validation (`weights/mod.rs:171,178,200,220,240,265,291,315,335,357,383,407,427,447,467,490,513,536,562,583,604`) |
| `clustering/` | `HDBSCANClusterer`, `BIRCHParams`, `Topic`, `TopicProfile`, `TopicStability`, `ClusterMembership` |
| `teleological/{mod,types,vector,resolution,meaning,embedder,groups,tucker}.rs` plus `comparator/`, `matrix_search/`, `profile/`, `services/`, `synergy_matrix/` | Teleological abstractions, Tucker decomposition, group/embedder mask types |
| `graph_linking/` | K-NN edges (`EmbedderEdge`, `KnnGraph`), 20 typed-edge types (`GraphLinkEdgeType`), thresholds (`KNN_K`, `MIN_KNN_SIMILARITY`, `NN_DESCENT_*`) |
| `graph/` | E8 asymmetric graph similarity (`GraphDirection`, `compute_graph_asymmetric_*`, `compute_e8_asymmetric_full`) |
| `causal/` | E5 causal direction (`CausalDirection`), causal types |
| `code/` | E7 cAST chunking, query-type detection (`CodeQueryType`, `compute_e7_similarity_with_query_type`) |
| `entity/` | E11 entity store, TransE-style relation prediction |
| `constellation/{mod,types,compiler,reservoir,welford,tests}.rs` | Streaming centroid compiler |
| `contrastive/{mod,types,mining,tests}.rs` | Contrastive pair mining + `AnomalyKind` |
| `typed_edge_export/{mod,tests}.rs` | `TypedEdgeTrainingRecord` builder |
| `learner/mod.rs` | Learner profile, modalities, state, M(t) traces, goals (core-side) |
| `learner_training.rs` | Learner training dataset shard types |
| `learning/mod.rs` | Learning-as-UTL events, signals, ΔS / ΔC / ΔE / L scoring |
| `injection/` | Memory injection candidates, badges, token budget |
| `memory/` | In-memory store stub, capture types |
| `quantization/` | PQ-8, Float8E4M3, Binary, SparseNative, TokenPruning encoders |
| `index/` | HNSW abstraction (`hnsw_impl`) |
| `similarity/` | Cosine, dense engine |
| `llm_edge_validation/` | LLMEdgeValidation request/response types |
| `dynamicjepa/` (~25 files) | Domain pack, instrument, panel, predictor, planner, guard, surprise, validation, verification, schema, dataset, trajectory, binding, pair_kinds, features, meaning_compression, IDs, record header, error |
| `monitoring.rs` | `LayerStatus`, `LayerStatusProvider`, `SystemMonitor` traits |
| `stubs/` | Test-only stubs (gated behind `test-utils` feature) |
| `examples/tree_sitter_test.rs` | tree-sitter chunking demo |

## 3. context-graph-embeddings (`crates/context-graph-embeddings/src/`)
Compile-error if the `candle` feature is missing. Default features: `candle`. `cuda` is an alias for `candle`.

| Path | Purpose |
| --- | --- |
| `lib.rs` | Public surface (re-exports `ModelId`, `EmbeddingProvider`, `ProductionMultiArrayProvider`, warm provider accessors, learner namespace, quantization, pruning, storage helpers, adapters) |
| `types/model_id/{mod,core,conversions,display,repository,tokenizer,tests}.rs` | `ModelId` enum (15 variants per `core.rs:40-77`; `production()` returns 14: `Semantic..BgeM3Dense` per `core.rs:234-251`); native + projected dims; tokenizer family; `Entity` (legacy 384D MiniLM) coexists with `Kepler` (production 768D RoBERTa+TransE) |
| `types/dimensions/` | Native + projected dim helpers |
| `types/embedding/` | `EmbeddingArray`, sparse, token-level types |
| `types/concatenated/` | Concatenated 14-embedder vector |
| `types/input/` | Input batching, modality tags |
| `models/mod.rs`, `models/registry/`, `models/factory/`, `models/memory_tracker/`, `models/attention/` | `ModelRegistry`, `ModelFactory`, `MemoryTracker`, attention helpers |
| `models/pretrained/` (semantic, causal, code, contextual, graph, kepler, late_interaction, sparse, rgcn, weight_projection, bge_m3_dense, shared.rs) | Pretrained model wrappers (E1, E5, E7, E10, E8, E11, E12, E6/E13, E14) |
| `models/custom/` (hdc, temporal_periodic, temporal_positional, temporal_recent) | Custom (no-weight) models for E2/E3/E4/E9 |
| `warm/{mod,config,error,handle,state,tests}.rs` plus `loader/`, `registry/`, `cuda_alloc/`, `memory_pool/`, `inference/`, `validation/`, `health/`, `diagnostics/`, `integration/` | GPU warm provider — model loading, registry, CUDA allocation, memory pool, inference, validation, health checks, diagnostics |
| `provider/` | `EmbeddingProvider`, `ProductionMultiArrayProvider` |
| `traits/embedding_model/`, `traits/model_factory/` | `EmbeddingModel`, `ModelFactory`, `MEMORY_ESTIMATES`, `TOTAL_MEMORY_ESTIMATE` |
| `config/` | `EmbeddingConfig` (models, batch, cache, gpu, eviction policy, padding strategy) |
| `cache/` | `EmbeddingCache` (moka-backed) with `CacheKey`, `CacheStats` |
| `batch/` | Batched embedding flow |
| `quantization/` | `BinaryEncoder`, `Float8Encoder`, `PQ8Codebook`, `QuantizationRouter`, `QuantizedEmbedding`, `QuantizationMetadata` |
| `pruning/` | `TokenPruningQuantizer`, `ImportanceScoringMethod`, `PrunedEmbeddings` |
| `storage/` | `IndexEntry`, `MultiSpaceQueryResult`, `EmbedderQueryResult`, `StoredQuantizedFingerprint`, `EXPECTED_QUANTIZED_SIZE_BYTES`, `MAX_QUANTIZED_SIZE_BYTES`, `RRF_K`, `STORAGE_VERSION` |
| `training/` | E5 LoRA training driver helpers |
| `gpu/` | GPU device helpers |
| `learner/mod.rs` | E15-E21 learner namespace: `LearnerEmbedderSlot`, `LearnerEmbedderSpec`, `LEARNER_EMBEDDER_COUNT = 7`, `UTL_PLANNED_TOTAL_EMBEDDERS = 21`, calibration fixture, asset preflight |
| `adapters/` | `E7CodeEmbeddingProvider` |
| `error/` | `EmbeddingError`, `EmbeddingResult` |
| `global_provider.rs` | `get_warm_provider`, `initialize_global_warm_provider`, `is_warm_initialized`, `warm_status_message`, `get_warm_causal_model`, `get_warm_graph_model` |
| `bin/train_causal.rs` | E5 LoRA training binary (`train-causal`) |

## 4. context-graph-cuda (`crates/context-graph-cuda/src/`)
Default features: `cuda`. `faiss-working` enables FAISS bindings (requires rebuilt `libfaiss_c.so`).

| Path | Purpose |
| --- | --- |
| `lib.rs` | Re-exports `CudaError`, FFI surface, `GpuDevice`, `gpu_memory_usage_percent`, `VectorOps`, similarity helpers, HDBSCAN clusterer, stub |
| `ffi/{mod,cuda_driver,faiss,knn}.rs` | CUDA driver API bindings, FAISS bindings, KNN kernels |
| `context/{mod,green_contexts}.rs` | Green-context partitioning for VRAM isolation |
| `safe/{mod,device}.rs` | Safe wrappers for CUDA primitives |
| `ops.rs` | `VectorOps` (batched ops over GPU memory) |
| `similarity.rs` | GPU cosine / dot kernels |
| `hdbscan/{mod,clusterer,error,gpu_knn,tests}.rs` | GPU HDBSCAN with KNN graph build |
| `error.rs` | `CudaError` / `CudaResult` |
| `stub.rs` | `StubVectorOps` (test-only, AP-007 compliant) |

## 5. context-graph-storage (`crates/context-graph-storage/src/`)
Default features: `cuda`. Forces `context-graph-embeddings` candle+cuda features in deps.

| Path | Purpose |
| --- | --- |
| `lib.rs` | Re-exports `RocksDbTeleologicalStore`, `EdgeRepository`, `CodeStore`, all CF constants, all DynamicJEPA stores (`dynamicjepa::*`) |
| `column_families.rs` | 11 base CFs (8 originals: `nodes`, `edges`, `embeddings`, `metadata`, `temporal`, `tags`, `sources`, `system` + 3 graph linking: `embedder_edges`, `typed_edges`, `typed_edges_by_type`); `TOTAL_COLUMN_FAMILIES = 101` (`column_families.rs:408`); shared 256MB block cache |
| `error.rs` | `StorageError` (`OpenFailed`, `ColumnFamilyNotFound`, `WriteFailed`, `ReadFailed`, `FlushFailed`, `NotFound`, `Serialization`, `ValidationFailed`, `IndexCorrupted`, `Internal`) |
| `indexes.rs` | Secondary index helpers (tags, temporal, sources) |
| `code/{mod,store,error}.rs` | `CodeStore` over CFs `code_entities`, `code_e7_embeddings`, `code_file_index`, `code_name_index`, `code_signature_index` (`CODE_CF_COUNT = 5`) |
| `graph_edges/{mod,types,builder,repository,serialization}.rs` | `EdgeRepository`, `BackgroundGraphBuilder`, `GraphBuilderConfig`, K-NN write paths over `embedder_edges`, `typed_edges`, `typed_edges_by_type` |
| `teleological/mod.rs`, `schema.rs`, `serialization.rs`, `tests/` | Teleological re-export hub |
| `teleological/column_families.rs` | `TELEOLOGICAL_CF_COUNT = 41` (`:657`) + `QUANTIZED_EMBEDDER_CF_COUNT = 14` (`:746`, `emb_0..emb_13`) + `CODE_CF_COUNT = 5` (`:1887`) + `CAUSAL_CF_COUNT = 2` (`:2057`, `causal_relationships`, `causal_by_source`); per-CF options builders; key-format functions |
| `teleological/quantized/{mod,trait_def,helpers,rocksdb_impl,error,tests}.rs` | `QuantizedFingerprintStorage`, per-embedder quantized read/write |
| `teleological/indexes/{mod,embedder_index,registry,metrics}.rs` plus `hnsw_config/`, `hnsw_impl/` | usearch-backed HNSW index registry + sparse inverted index + late-interaction MaxSim configs |
| `teleological/search/{mod,result,error,maxsim,temporal_boost,token_storage}.rs` plus `multi/`, `pipeline/`, `single/`, `matrix/` | Multi-space search engine, pipeline (E13→E1→E12), single-embedder, matrix variants |
| `teleological/rocksdb_store/mod.rs`, `store.rs`, `crud.rs`, `trait_impl.rs`, `persistence.rs`, `index_ops.rs`, `helpers.rs`, `types.rs`, `versioned_bincode.rs`, `tests.rs` | Top-level `RocksDbTeleologicalStore` + CRUD + trait impls |
| `teleological/rocksdb_store/{search,fusion,export,constellation,contrastive}.rs` | Search/fusion paths and read/write surfaces for constellation, contrastive |
| `teleological/rocksdb_store/{learner,learner_training,learning,causal_relationships,causal_hnsw_index,typed_edge_export,typed_edge_keys,llm_validation}.rs` | Learner state, learning events, causal relationships and dedicated HNSW, typed edge export, LLM edge validation |
| `teleological/rocksdb_store/{audit_log,content,file_index,source_metadata,inverted_index,provenance_storage,anomaly_derivation}.rs` | Audit log, content, file index, source metadata, inverted index, provenance, anomaly derivation |
| `dynamicjepa/{mod,common,keys,column_families,encode,audit,inspection}.rs` plus `{prediction,event,binding,domain_pack,artifact,trajectory,planning,pairwise,threshold,surprise,dataset,constellation,verification}_store.rs`, `tests.rs`, `bridge.rs` | DynamicJEPA storage data plane: 28 CFs (`DJ_CF_COUNT = DJ_CF_NAMES.len()` per `column_families.rs:74`; asserted `== 28` in `tests.rs:1126`), per-domain stores |
| `examples/` | Storage usage examples |

## 6. context-graph-mcp (`crates/context-graph-mcp/src/`)
Default features: `cuda`, `llm`. Tool count varies by `llm`: 141 base / 145 with `llm`.

| Path | Purpose |
| --- | --- |
| `main.rs` | MCP server entry: `CliArgs` parser, daemon spawn/proxy, `mcp.pid` flock guard (Unix), tracing init, transport selection. Env vars: `CONTEXT_GRAPH_TRANSPORT`, `CONTEXT_GRAPH_TCP_PORT`, `CONTEXT_GRAPH_BIND_ADDRESS`, `CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT`, `CONTEXT_GRAPH_STORAGE_PATH`, `CONTEXT_GRAPH_MODELS_PATH`, `CONTEXT_GRAPH_WATCHER_ENABLED`, `CONTEXT_GRAPH_WARM_FIRST`, `CONTEXT_GRAPH_DAEMON`, `CONTEXT_GRAPH_DAEMON_PORT`, `CONTEXT_GRAPH_MCP_QUIET` |
| `lib.rs` | Public lib surface (`adapters`, `handlers`, `monitoring`, `protocol`, `server`, `tools`, `weights`) |
| `protocol.rs` | `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcId` (String/Number/Null), `JsonRpcError`, `error_codes` |
| `monitoring.rs` | `LiveLayerStatusProvider` — real component health checks (replaces stub) |
| `weights.rs` | MCP-side weight DTOs |
| `server/mod.rs` | `McpServer` constructor, `TransportMode::{Stdio, Tcp}`, stdio handler, request dispatch glue, path resolution |
| `server/transport.rs` | TCP transport (`run_tcp`, `handle_tcp_client`), `read_line_bounded` line framing |
| `server/watchers.rs` | File watcher, code watcher, K-NN graph builder background tasks |
| `tools/mod.rs`, `types.rs`, `names.rs`, `aliases.rs` | `ToolDefinition`, tool name constants for dispatch matching, alias resolution |
| `tools/definitions/mod.rs` | `get_tool_definitions()` — registers 141/145 tools across 27 categories |
| `tools/definitions/{capability,causal,causal_discovery,code,constellation,contrastive,core,curation,daemon,dynamicjepa,embedder,entity,file_watcher,graph,graph_link,keyword,learning,maintenance,merge,provenance,robustness,sequence,temporal,topic,training,typed_edges,utl}.rs` | One module per tool category — schemas + descriptions |
| `handlers/mod.rs` | Re-exports `Handlers`, `DaemonState` |
| `handlers/core/{mod,handlers,dispatch}.rs` | Method dispatch: `initialize`, `notifications/initialized`, `shutdown`, `tools/list`, `tools/call`; handles topic portfolio restore on init |
| `handlers/merge.rs` | `merge_concepts` handler |
| `handlers/tools/{capability,causal,causal_discovery,causal_relationship,code,consolidation,constellation,contrastive,curation,daemon,dispatch,dynamicjepa,embedder,entity,file_watcher,graph,graph_learning,graph_link,helpers,keyword,learning,maintenance,memory,provenance,robustness,sequence,status,temporal,topic,training,typed_edges,utl,validate}_tools.rs` | Per-category tool handlers (counted: 33+ files in `handlers/tools/`) |
| `handlers/tools/{causal,code,curation,dynamicjepa,embedder,entity,graph,graph_link,keyword,provenance,robustness,temporal,topic}_dtos.rs` | Per-category serde DTOs for request/response |
| `handlers/tests/{mod,initialize,error_codes,tools_list,tools_call,topic_tools,search_periodic_test,tcp_transport_integration,dynamicjepa_tools,mcp_protocol_e2e_test}.rs` | Integration tests for protocol + tools |
| `adapters/{mod,lazy_provider,causal_hint}.rs` | `LazyMultiArrayProvider` (defers GPU model load), `LlmCausalHintProvider` (gated by `llm`) |

## 7. context-graph-cli (`crates/context-graph-cli/src/`)
Default features: `cuda`. Four binaries (per `cargo metadata`): `context-graph-cli`, `context-graph`, `blackbox`, `reality-loop`. Note: `reality-loop` is auto-discovered (no `[[bin]]` entry in `Cargo.toml`).

| Path | Purpose |
| --- | --- |
| `main.rs` | clap derive root for `context-graph-cli` binary |
| `lib.rs` | `commands`, `error`, `mcp_client`, `mcp_helpers`; `CliExitCode`, `is_corruption_indicator`, `exit_code_for_error` |
| `bin/context-graph.rs` | Auxiliary binary (6 LOC stub forwarding to lib) |
| `bin/blackbox.rs` | Black-box harness entry (24 LOC; phase-13 retention dashboard, FSV) |
| `bin/reality-loop.rs` | Rust-owned SWE-bench reality loop (2,379 LOC). `tokio::main` async binary. Drives one local Ollama model (default `qwen3.6:27b`) through SWE-bench_Lite repair attempts: task-manifest readback, git workspace control, repository context selection, prompt/request/response/parse-report SHA-256 read-after-write, real `/api/chat` calls, `nvidia-smi` GPU VRAM budget enforcement, BlackBoxFix run/model/tool/verifier/score/reward records with SQLite ledger readback, append-only `reality-events.jsonl`, typed `RealityError` failure artifacts, fail-closed `<run_root>/measurement-contract.json` written before execution. Hard rejects `--run-until-resolved` without `--official-eval-each-attempt`, dataset ≠ `princeton-nlp/SWE-bench_Lite`, malformed instance/repo/SHA, dirty worktree, base_commit ≠ git HEAD, candidate patch touching benchmark/test/fixture/harness paths. |
| `error.rs` | CLI exit-code mapping |
| `mcp_client.rs`, `mcp_helpers.rs` | Local MCP client for hook scripts |
| `commands/setup.rs` | Bootstrap (`init`, model download orchestration) |
| `commands/warmup.rs` | Wait for warm provider readiness |
| `commands/watch.rs` | Path-watch ingestion |
| `commands/topic.rs` | Topic ops |
| `commands/divergence.rs` | Divergence alerts |
| `commands/learning.rs` | Learning event + dataset commands |
| `commands/utl.rs` | Learner-state UTL ops |
| `commands/export_training.rs` | TrainingRecord v3 export (Parquet) |
| `commands/export_dynamicjepa.rs` | DynamicJEPA dataset export |
| `commands/dynamicjepa.rs` | DynamicJEPA admin (~21K LOC; domain-pack registry, instrument/adapter/panel materialization, binding, trajectory, dataset) |
| `commands/backfill_e14.rs` | E14 backfill |
| `commands/blackbox.rs` | Black-box harness commands |
| `commands/memory/{mod,capture,inject}.rs` | Hook helpers (capture user prompt, inject context) |
| `commands/session/{mod,persist,restore}.rs` | Session lifecycle |
| `commands/hooks/{mod,args,error,types,pre_tool_use,post_tool_use,user_prompt_submit,task_completed,session_start,session_end,session_state,memory_cache,pre_compact}.rs` | Claude Code hook lifecycle handlers |

## 8. context-graph-causal-agent (`crates/context-graph-causal-agent/src/`)
Default features: empty. Optional: `llm` (adds `llama-cpp-2`), `cuda` (implies `llm` + `llama-cpp-2/cuda`).

| Path | Purpose |
| --- | --- |
| `lib.rs` | Crate root |
| `error.rs` | Causal agent error types |
| `types/mod.rs` | `CausalRelationship`, mechanism types |
| `scanner/mod.rs` | Pair scanning, batching |
| `llm/mod.rs` | `CausalDiscoveryLLM` — Hermes-2-Pro-Mistral-7B Q5_K_M via `llama-cpp-2`; loads `models/hermes-2-pro/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf` and three GBNF grammars (`causal_analysis.gbnf`, `graph_relationship.gbnf`, `validation.gbnf`); ChatML format; ~5GB model + ~1GB KV cache |
| `llm/prompt.rs` | ChatML prompt construction with system prompts optimized for Hermes 2 Pro function calling |
| `activator/mod.rs` | Background loop trigger |
| `service/mod.rs` | MCP integration glue |
| `examples/` | LLM-gated examples (require `llm` feature) |

## 9. context-graph-graph-agent (`crates/context-graph-graph-agent/src/`)
Default features: empty. `llm` propagates to `context-graph-causal-agent/llm`. `cuda` implies `llm` + `causal-agent/cuda`. `test-utils` requires `llm` to construct LLM-backed service.

| Path | Purpose |
| --- | --- |
| `lib.rs` | Crate root |
| `stubs.rs` | No-op replacements compiled when `llm` is disabled |
| `error.rs` | Graph agent error types |
| `types/mod.rs` | Graph relationship types (20 edge kinds) |
| `scanner/mod.rs` | Pair scanning |
| `llm/{mod,prompt}.rs` | Wraps `CausalDiscoveryLLM` from causal-agent — no direct `llama-cpp-2` dep |
| `activator/mod.rs` | Background trigger |
| `service/mod.rs` | Graph relationship discovery + edge validation service |
| `examples/{benchmark_graph,benchmark_graph_code,benchmark_graph_multi}.rs` | Require `llm` feature |

## 10. context-graph-dynamicjepa (`crates/context-graph-dynamicjepa/src/`)
Service layer for the 5090jepa DynamicJEPA programme.

| Path | Purpose |
| --- | --- |
| `lib.rs` | Re-exports `artifact`, `config`, `mi_audit`, `model`, `service`, `transfer` |
| `artifact.rs` | Tiny-model artifact writer + hash-checked loader |
| `config.rs` | DynamicJEPA service config |
| `mi_audit.rs` | Mutual information audit (training-data leakage check) |
| `model/mod.rs` | Tiny-model definition + Candle training loop |
| `service.rs` | Phase 7/8 service orchestration: hash-checked prediction, planning, guard decisions, surprise events |
| `transfer.rs` | Cross-domain transfer helpers |

## 11. context-graph-test-utils (`crates/context-graph-test-utils/src/`)
Real-data fixtures (no mocks per workspace rule).

| Path | Purpose |
| --- | --- |
| `lib.rs` | Public surface |
| `fingerprints.rs` | Real `SemanticFingerprint`/`TeleologicalFingerprint` builders |
| `stores.rs` | RocksDB-backed temp-dir store fixtures |

## 12. context-graph-benchmark (excluded from default-members)
Located under `crates/context-graph-benchmark/src/`. Build with `cargo build -p context-graph-benchmark` (or with `--features real-embeddings` for GPU-required benches). Per `cargo metadata`, **45 binaries** are exposed as `[[bin]]` entries (`Cargo.toml` declares 44; one auto-discovered via `src/bin/`).

| Path | Purpose |
| --- | --- |
| `lib.rs`, `config.rs`, `util.rs`, `ablation.rs`, `stress_corpus.rs` | Shared infra |
| `baseline/{single_embedder,single_hnsw}.rs` | Baseline RAG comparators |
| `causal_bench/`, `validation/`, `tuning/`, `scaling/`, `runners/`, `metrics/`, `realdata/`, `datasets/`, `reports/` | Per-area benchmarks |
| `bin/*.rs` (45 files) | Per-scenario binaries: `benchmark-report`, `causal-accuracy-bench`, `causal-discovery-mcp-bench`, `causal-e2e-bench`, `causal-embedding-bench`, `causal-perf-bench`, `causal-prompt-experiment`, `causal-provenance-bench`, `causal-provenance-e2e-bench`, `causal-provenance-llm-bench`, `causal-realdata-bench`, `causal-span-fix-bench`, `code-fingerprint-bench`, `disk-space-bench`, `e1-semantic-bench`, `e10_modifier_tuning`, `e11-entity-bench`, `e2-manual-verification`, `e4-hybrid-session-bench`, `e7-bench`, `e7-realdata-bench`, `embedder-impact-bench`, `embedder-impact-realdata-bench`, `embedder-stress`, `gpu-bench`, `graph-bench`, `graph-linking-bench`, `graph-linking-gpu-bench`, `graph-linking-real-bench`, `graph-realdata-bench`, `hf-bench`, `mcp-bench`, `model-injection-bench`, `model-injection-rigorous-bench`, `multimodal-bench`, `multimodal-realdata-bench`, `realdata-bench`, `sparse-bench`, `spawn-blocking-bench`, `temporal-bench`, `temporal-realdata-bench`, `unified-realdata-bench`, `validation-bench`, `validation-realdata-bench`, `weight-profile-bench` |
| `[[bench]]` `comparative_suite`, `scaling_analysis` | Criterion benches |

## 13. Entry-Point Trace (MCP Path)
1. `crates/context-graph-mcp/src/main.rs::main` — parse `CliArgs`, init tracing, decide daemon mode, optionally spawn child process or proxy stdio↔TCP.
2. Acquire `<storage.path>/mcp.pid` Unix flock with stale-lock detection (uses `kill(pid, 0)` + `/proc/pid/status` zombie check + port health).
3. Load TOML config (`Config::from_path`) → resolve `storage.path`, `mcp.transport`, `mcp.tcp_port`, `mcp.bind_address`.
4. Build `RocksDbTeleologicalStore` over 101 CFs with shared 256MB block cache.
5. Build `LazyMultiArrayProvider` wrapping `ProductionMultiArrayProvider`; either await first warm completion (`--warm-first`) or skip warm entirely (`--no-warm`).
6. Construct `Handlers` (`crates/context-graph-mcp/src/handlers/core/handlers.rs`) holding store, provider, optional `LlmCausalHintProvider` (when `llm`).
7. Spawn background tasks via `crates/context-graph-mcp/src/server/watchers.rs` — file watcher, code watcher, `BackgroundGraphBuilder`.
8. Enter `TransportMode::Stdio` → `read_line_bounded` loop; or `TransportMode::Tcp` → `run_tcp` accepting concurrent clients.
9. Each request lands in `handlers/core/dispatch.rs::dispatch`; `tools/call` is forwarded to `handlers/tools/dispatch.rs::dispatch_tool_call`, which matches `tool_names::*` to the per-category handler module.
10. Tool handlers read/write through `RocksDbTeleologicalStore`, `EdgeRepository`, `CodeStore`, DynamicJEPA stores, and the warm embedding provider, returning JSON-RPC responses serialised by `protocol.rs`.

## 13a. Entry-Point Trace (Reality-Loop Path)
1. `crates/context-graph-cli/src/bin/reality-loop.rs::main` — `tokio::main`. Parses `Args` via clap; calls `run(args)`.
2. `run()` validates `--max-attempts >= 1`, hard-rejects `--run-until-resolved && !--official-eval-each-attempt`, resolves `--context-graph-bin` and `--task-manifest` paths, parses task manifest as `TaskManifest` via `serde_json`.
3. `validate_task_manifest_contract` enforces dataset = `princeton-nlp/SWE-bench_Lite`, repo `owner/repo` shape, base_commit 40-char hex, non-empty problem_statement, populated `fail_to_pass` and `pass_to_pass_count`, and `source_of_truth` binding dataset+instance.
4. `<runtime_root>/measurement-contract.json` written with claim scope, success predicate, non-claims, immutable boundaries, mandatory readbacks, model-editable scope, model-forbidden scope, fail-closed policy. SHA-256 read back before execution.
5. Per attempt: `nvidia-smi` GPU VRAM check (`--gpu-min-free-mb`/`--gpu-max-used-mb`) → git workspace clone/reset/clean → repo context selection (real `git ls-files`, `grep`) → ChatML prompt construction → real Ollama `/api/chat` POST (`reqwest`, `--model-timeout`) → SHA-256 of prompt/response → parse model JSON to candidate patch → reject patches touching benchmark/test/fixture/harness (`RUST_REALITY_PATCH_TOUCHES_BENCHMARK`) → `git apply` → if `--official-eval-each-attempt`, shell out to `--swebench-python` running official Docker evaluator (`--official-docker-host`, `--official-docker-config`, `--official-cache-level`, `--official-timeout`, `--official-max-workers`) → readback `official_report.resolved_ids` → write per-attempt `AttemptRecord` to `reality-events.jsonl`.
6. Failure path: typed `RealityError` (status, error_code, message, field_path, remediation, details, source_of_truth) serialised to `<runtime_root>/failures/<error_code>.json`; `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<path>` printed to stderr; exit 1.
7. Success path: `RUST_REALITY_LOOP_SUMMARY=<text>` printed to stdout; exit 0.

## 14. Python Scripts
Located under `scripts/` (78 entries) and `gemmafrontend/` (7 Python files). The Python harnesses are not built by Cargo but are exercised by tests, hooks, and operator workflows.

| Path | Role |
| --- | --- |
| `scripts/v4_2/` (34 files) | CAMD-DPO mining, Shakespeare empirical cycle, FSV harnesses, evidence inventory; consumed by [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md), [20_learning_subsystem.md](20_learning_subsystem.md) |
| `scripts/5090jepa/` | Counter-world / gridworld_5x5 corpus prep, dry-run, release-candidate archive helpers |
| `scripts/exp2/` | Phase-2 experimental cycle (`run_exp2.py`, `audit_chain.py`, `effective_rank.py`, `estimators/`, `strata/`, FSV) |
| `scripts/shakespeare_*.py` (~30 files) | Shakespeare ingestion, DPO datasets, RLHF, inference server, FSV, V4/V5 training |
| `scripts/download_models.py`, `download_utl_assets.py` | Model + UTL asset downloaders |
| `scripts/upload_to_huggingface.py`, `upload_v2_1_to_huggingface.py`, `upload_v4_1_to_huggingface.py` | Model release upload |
| `scripts/train_utl_eda_head.py`, `train_utl_eda_stress_head.py` | UTL EDA head training |
| `scripts/mcp_full_verify.py` | End-to-end MCP verification harness |
| `scripts/ensure_daemon.sh`, `context-graph-mcp-wrapper.sh` | Daemon lifecycle helpers |
| `scripts/benchmark-*.sh`, `disk-guard.sh`, `trim-stale-deps.sh` | Build-size + disk hygiene |
| `scripts/check-embedder-count.sh`, `check-ffi-consolidation.sh` | CI guards |
| `scripts/restart_pipeline_services.sh`, `vram_watchdog.py` | Runtime ops |
| `scripts/blackboxfix_*.py` | BlackBoxFix demos and intelligence reports |
| `gemmafrontend/server.py` (3,879 LOC) | Python evidence console; HTTP `:8787`; loads tools from backend; targets the single local Ollama model (`qwen3.6:27b`); persists frontend snapshots under `/tmp/gemmafrontend-runs/frontend-runs/*/system-snapshot.json` |
| `gemmafrontend/manual_fsv.py` (382 LOC) | Manual FSV starter for the inner-actor frontend server, ledger, snapshot, product-category pack, signed attestation, tamper detection |
| `gemmafrontend/model_benchmark.py` (248 LOC) | Single-model benchmark runner against the local `qwen3.6:27b` Ollama model |
| `gemmafrontend/model_prompt_matrix.py` (497 LOC) | Prompt × model evidence matrix (currently single-model: `qwen3.6:27b`) |
| `crates/context-graph-cli/src/bin/reality-loop.rs` | Rust-only SWE-bench Lite refinement loop and prompt compiler |

## 15. Build Configuration
Source: `Cargo.toml` (root), `rust-toolchain.toml`, per-crate `[features]`.

| Item | Value |
| --- | --- |
| Workspace packages | 11 (`Cargo.toml` `[workspace.members]`) |
| Default workspace members | 10 (benchmark excluded — never compiled into MCP server) |
| Edition / rust-version | edition `2021` / `rust-version = "1.95"` (in workspace.package); `rust-toolchain.toml` channel `stable` |
| Toolchain components | rustfmt, clippy, rust-src |
| Toolchain target | `x86_64-unknown-linux-gnu` |
| Release profile | `lto = "thin"`, `codegen-units = 1`, `strip = true` |
| Dev profile | `opt-level = 0`, `debug = 1` (line info only), `incremental = true`, `split-debuginfo = "unpacked"`; external deps compile with `debug = false` |
| MCP defaults | `cuda`, `llm` |
| MCP `llm` feature | Adds `llama-cpp-2` (~500MB binary growth) via `causal-agent/llm` + `graph-agent/llm` |
| MCP `cuda` feature | Cascades to `cuda/cuda`, `embeddings/cuda`, `storage/cuda`, `causal-agent/cuda`, `candle` |
| CLI defaults | `cuda` (cascades to `embeddings/cuda`) |
| Embeddings defaults | `candle` (mandatory — `compile_error!` otherwise); `cuda` aliases `candle` |
| CUDA defaults | `cuda`; opt-in `faiss-working` (requires rebuilt `libfaiss_c.so`) |
| Storage defaults | `cuda` (cascades to `embeddings/cuda` and brings in `context-graph-cuda` dep) |
| Causal-agent defaults | empty; opt-in `llm` (`dep:llama-cpp-2`), `cuda` (implies `llm` + `llama-cpp-2/cuda`) |
| Graph-agent defaults | empty; opt-in `llm` (propagates to `causal-agent/llm`), `cuda` (implies `llm` + `causal-agent/cuda`), `test-utils` |
| Core defaults | empty; opt-in `test-utils` (test-only stubs) |
| `train-causal` features | inherits embeddings `candle` |
| Benchmark features | empty; opt-in `real-embeddings` (adds embeddings + mcp + graph-agent + causal-agent), `benchmark-tests` |

## 16. Top-Level Public APIs Per Crate
Source: `lib.rs` re-exports.

- **context-graph-core**: `Config`, `ContextGraphError` + sub-errors, `Result`, `EdgeType`, `LayerStatusProvider`, `SystemMonitor`, teleological types (`DomainAlignments`, `Embedder`, `EmbedderMask`, `GroupAlignments`, `MultiResolutionHierarchy`, `ProfileId`, `SynergyMatrix`, `TaskType`, `TeleologicalProfile`, `TeleologicalVector`, `TuckerCore`), memory capture (`Memory`, `Session`, `MAX_CONTENT_LENGTH`), clustering (`HDBSCANClusterer`, `Topic`, `TopicProfile`), injection (`InjectionCandidate`, `TokenBudget`, `BRIEF_BUDGET`), code (`compute_e7_similarity_with_query_type`, `CodeQueryType`), fusion (`fuse_rankings`, `weighted_rrf`, `RRF_K`, `FusionStrategy`), graph (`compute_graph_asymmetric_similarity`, `GraphDirection`), graph_linking (`KnnGraph`, `EmbedderEdge`, `TypedEdge`, `GraphLinkEdgeType`, `KNN_K`), weights (`get_weight_profile`, `WEIGHT_PROFILES`).
- **context-graph-storage**: `RocksDbTeleologicalStore`, `EdgeRepository`, `BackgroundGraphBuilder`, `CodeStore`, all CF constants (`CF_FINGERPRINTS`, `CF_EMB_0..CF_EMB_13`, `CF_LEARNER_*`, `CF_CONSTELLATIONS`, `CF_CONTRASTIVE_*`, `CF_TYPED_EDGE_*`, `CF_LEARNING_EVENTS`, `CF_GOAL_CENTROIDS`, etc.), `TOTAL_COLUMN_FAMILIES`, dimension constants `E1_DIM..E13_SPLADE_VOCAB`, `NUM_EMBEDDERS`, `compute_maxsim_direct`, `QuantizedFingerprintStorage`, `TELEOLOGICAL_VERSION`, all DynamicJEPA stores (`DJ_CF_NAMES`, `DJ_CF_COUNT`).
- **context-graph-embeddings**: `WarmProvider`, `ModelId` (15 variants), `ModelRegistry`, `EmbeddingModel`, `ModelFactory`, `ProductionMultiArrayProvider`, `MultiArrayEmbedding`, learner namespace (`LearnerEmbedderSpec`, `embed_learner_signal`, `LEARNER_EMBEDDER_COUNT`, `UTL_PLANNED_TOTAL_EMBEDDERS`), quantization, pruning, storage helpers.
- **context-graph-cuda**: `CudaError`, FFI surface, `GpuDevice`, `gpu_memory_usage_percent`, `VectorOps`, similarity helpers, `HDBSCANClusterer`, `StubVectorOps`.
- **context-graph-mcp**: `tools::definitions::get_tool_definitions()` returning 141/145 `ToolDefinition`s, `handlers::core::Handlers`, `handlers::tools::dispatch::dispatch_tool_call`, `protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcId, JsonRpcError}`, `server::TransportMode`, `monitoring::LiveLayerStatusProvider`.
- **context-graph-cli**: `commands::*`, `mcp_client`, `mcp_helpers`, `error::{CliExitCode, exit_code_for_error, is_corruption_indicator}`. The `reality-loop` binary is a self-contained module that does NOT import from `lib.rs`.
- **context-graph-causal-agent**: `CausalDiscoveryLLM` (gated on `llm`), service + scanner + activator types, `CausalRelationship`.
- **context-graph-graph-agent**: graph relationship service (gated on `llm`), 20 edge kinds, `stubs` (no-LLM fallback), edge validation.
- **context-graph-dynamicjepa**: `service::*`, `artifact::*`, `transfer::*`, `mi_audit::*`, `model::*`, `config::*`.
- **context-graph-test-utils**: real-data fingerprint and store fixtures.
