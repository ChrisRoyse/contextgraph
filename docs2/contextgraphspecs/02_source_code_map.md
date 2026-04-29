# Source Code Map

**Source files covered:**
All `crates/*/src/**/*.rs`, `scripts/v4_2/*.py`, workspace manifests.

## 1. Workspace Package Dependencies
| Package | Workspace path dependencies |
| --- | --- |
| context-graph-core | (root — no internal deps) |
| context-graph-cuda | context-graph-core |
| context-graph-embeddings | context-graph-core, context-graph-cuda |
| context-graph-test-utils | context-graph-core, context-graph-storage |
| context-graph-storage | context-graph-core, context-graph-cuda, context-graph-embeddings, context-graph-test-utils |
| context-graph-causal-agent | context-graph-core, context-graph-embeddings, context-graph-storage |
| context-graph-graph-agent | context-graph-causal-agent, context-graph-core, context-graph-embeddings, context-graph-storage, context-graph-test-utils |
| context-graph-mcp | context-graph-causal-agent, context-graph-core, context-graph-cuda, context-graph-embeddings, context-graph-graph-agent, context-graph-storage |
| context-graph-cli | context-graph-core, context-graph-embeddings, context-graph-mcp, context-graph-storage, context-graph-test-utils |
| context-graph-benchmark | context-graph-causal-agent, context-graph-core, context-graph-embeddings, context-graph-graph-agent, context-graph-mcp, context-graph-storage |

Build order: core → cuda → embeddings → test-utils → storage → causal-agent → graph-agent → mcp → cli → benchmark.

## 2. context-graph-core (root crate)
| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Crate root |
| `src/config/` | TOML+env config; sub_configs.rs holds Config struct |
| `src/types/` | Core types incl. fingerprint/, audit/, learner/, etc. |
| `src/types/fingerprint/semantic/` | SemanticFingerprint, ModelId-bound dimensions, constants |
| `src/types/fingerprint/teleological/` | TeleologicalFingerprint wrapper |
| `src/embeddings/` | Embedder trait + provider |
| `src/retrieval/` | Search strategies, aggregation (RRF/Weighted/Max), config |
| `src/fusion/` | RRF_K constant + storage-side fusion helpers |
| `src/weights/` | 17 weight profiles, state-conditioned routing |
| `src/clustering/` | HDBSCAN driver + topic emergence |
| `src/teleological/` | Pipeline, comparator, matrix_search, profile, services, synergy_matrix |
| `src/graph_linking/` | Embedder edge / typed edge / weight projector |
| `src/graph/` | Causal-graph types |
| `src/causal/` | Asymmetric E5, CausalDirection, causal types |
| `src/code/` | E7 code chunking + AST |
| `src/entity/` | E11 entity store, TransE prediction |
| `src/constellation/` | Compile, derive, score |
| `src/contrastive/` | Mining + AnomalyKind |
| `src/typed_edge_export/` | TypedEdgeTrainingRecord builder |
| `src/learner/` | Learner profile, modality, state, M(t), goals |
| `src/learning/` | Learning-as-UTL events, signals, ΔS/ΔC/ΔE/L |
| `src/training/` | TrainingRecord v3 |
| `src/injection/` | Memory injection candidate scoring |
| `src/memory/` | In-memory store stub |
| `src/quantization/` | PQ-8, Float8E4M3, Binary, SparseNative, TokenPruning |
| `src/index/` | HNSW abstraction (hnsw_impl) |
| `src/similarity/` | Cosine, dense engine |
| `src/llm_edge_validation/` | LLMEdgeValidation types |
| `src/traits/` | Repository / store / embedding traits |
| `src/error/` | Error types |
| `src/stubs/` | Test stubs |

## 3. context-graph-embeddings
| Path | Purpose |
| --- | --- |
| `src/types/model_id/` | ModelId enum (15 variants; production = 14) |
| `src/types/dimensions/` | Native + projected dim helpers |
| `src/types/embedding/` | EmbeddingArray, sparse, token-level types |
| `src/types/concatenated/` | Concatenated vector |
| `src/types/input/` | Input batching |
| `src/models/` | E1 (e5), E5 (nomic), E7 (Qodo), E10 (e5-base), E11 (Kepler), E12 (ColBERT), E14 (BGE-M3), HDC, Splade |
| `src/warm/` | GPU warm provider — loader, registry, cuda_alloc, memory_pool, inference, validation, health, diagnostics |
| `src/config/` | EmbeddingConfig (models, batch, cache, gpu) |
| `src/traits/embedding_model/` | EmbeddingModel trait |
| `src/traits/model_factory/` | ModelFactory |
| `src/bin/train_causal.rs` | Causal training (E5 LoRA) bin |

## 4. context-graph-storage
| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Crate root |
| `src/column_families.rs` | 11 base CFs |
| `src/repository.rs` | Top-level CRUD, K-NN edge writes |
| `src/code/` | CF_CODE_* (5 CFs) |
| `src/graph_edges/` | CF_TYPED_EDGES, CF_TYPED_EDGES_BY_TYPE, CF_EMBEDDER_EDGES |
| `src/teleological/column_families.rs` | 41 teleological CFs + 14 quantized |
| `src/teleological/rocksdb_store/` | All write/read paths: search.rs, fusion.rs, export.rs, constellation.rs, contrastive.rs, learner_*.rs, learning_events.rs, causal_relationships.rs, typed_edges*.rs |
| `src/teleological/quantized/` | Per-embedder quantization codec |
| `src/teleological/indexes/` | Sparse inverted (E6/E13), late-interaction (E12) |
| `src/teleological/search/` | Multi-space search engine |

## 5. context-graph-mcp
| Path | Purpose |
| --- | --- |
| `src/main.rs` | MCP server entry, transport, daemon, PidFileGuard |
| `src/server/` | Transport (stdio, tcp), connection handling |
| `src/protocol/` | JSON-RPC types, errors |
| `src/handlers/` | Method handlers |
| `src/handlers/tools/` | Per-category dispatchers (`*_tools.rs`, `*_dtos.rs`) |
| `src/handlers/tools/dispatch.rs` | tools/call routing |
| `src/handlers/tools/capability_tools.rs` | get_capability_matrix |
| `src/handlers/tools/learning_tools.rs` | Learning + learner-training-dataset endpoints |
| `src/handlers/core/` | initialize / list_resources |
| `src/handlers/tests/mcp_protocol_e2e_test.rs` | Integration |
| `src/tools/names.rs` | 108 tool name constants |
| `src/tools/types.rs` | ToolDefinition |
| `src/tools/definitions/` | One module per category (capability, core, ..., utl) |
| `src/adapters/` | LayerStatusProvider, daemon proxy |

## 6. context-graph-cli
| Path | Purpose |
| --- | --- |
| `src/main.rs` | clap derive, top-level subcommand dispatch |
| `src/commands/setup.rs` | Bootstrap |
| `src/commands/warmup.rs` | Wait for warm provider |
| `src/commands/watch.rs` | Path watcher → ingestion |
| `src/commands/topic.rs` | Topic ops |
| `src/commands/divergence.rs` | Divergence alerts |
| `src/commands/learning.rs` | Learning events + datasets |
| `src/commands/utl.rs` | Learner state ops |
| `src/commands/export_training.rs` | TrainingRecord export |
| `src/commands/backfill_e14.rs` | E14 backfill |
| `src/commands/memory/{capture,inject}.rs` | Hook helpers |
| `src/commands/session/{persist,restore}.rs` | Session lifecycle |
| `src/commands/hooks/` | Claude Code lifecycle: pre/post tool, session start/end, user prompt, task completed, pre_compact |

## 7. context-graph-causal-agent
| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Crate root |
| `src/types/` | CausalRelationship, mechanism types |
| `src/scanner/` | Pair scanning, batching |
| `src/llm/` | CausalDiscoveryLLM (Qwen2.5 via Candle, FP16 BL) |
| `src/activator/` | Background loop trigger |
| `src/service/` | MCP integration glue |

## 8. context-graph-graph-agent
Same shape as causal-agent: types/scanner/llm/activator/service. Provides graph relationship discovery (20 types) + edge validation. Uses shared `CausalDiscoveryLLM`. Crate gates LLM features behind `llm`; `stubs.rs` provides no-op replacements.

## 9. context-graph-cuda
`src/{ffi,context,similarity,hdbscan,allocation,memory,test}/`. FFI to CUDA driver + faiss; green-context partitioning; HDBSCAN on GPU; tensor allocation.

## 10. context-graph-test-utils
Shared fixtures: `corpus.rs`, `embeddings.rs`, `temporal.rs`, deterministic data builders.

## 11. context-graph-benchmark (excluded from default-members)
~40 binary targets under `src/bin/` (one per benchmark scenario), `src/causal_bench/`, `src/datasets/`, `src/metrics/`, `src/realdata/`, `src/runners/`, `src/scaling/`, `src/tuning/`, `src/validation/`, plus `src/baseline/{single_embedder,single_hnsw}.rs`. Build with `cargo build -p context-graph-benchmark`.

## 12. Entry Points
| Entry | Source |
| --- | --- |
| `context-graph-mcp` (bin) | crates/context-graph-mcp/src/main.rs |
| `context-graph-cli` (bin) | crates/context-graph-cli/src/main.rs |
| `train-causal` (bin) | crates/context-graph-embeddings/src/bin/train_causal.rs |
| Examples | crates/context-graph-causal-agent/examples/, context-graph-core/examples/, context-graph-graph-agent/examples/, context-graph-storage/examples/ |
| Benches | criterion-style benches under `*/benches/` |

## 13. Python Scripts (`scripts/v4_2/`, 32 files)
Roles: CAMD-DPO mining, Shakespeare empirical cycle, FSV harnesses, CUDA helpers, agent workflow learning, evidence inventory. See [19](19_camd_dpo_subsystem.md), [20](20_learning_subsystem.md).

## 14. Build Configuration
| Item | Value |
| --- | --- |
| Workspace packages | 10 |
| Default workspace members | 9 (benchmark excluded) |
| Edition / rust-version | 2021 / 1.75 |
| Release profile | thin LTO, codegen-units=1, strip=true |
| MCP defaults | `cuda`, `llm` |
| CLI defaults | `cuda` |
| `train-causal` features | candle |

## 15. Top-Level Public APIs Per Crate
- **context-graph-core**: `weights::{get_weight_profile, get_effective_weight_profile, select_state_conditioned_weight_profile}`, `retrieval::aggregation::*`, `learning::*`, `learner::*`, `constellation::*`, `contrastive::*`.
- **context-graph-storage**: `Repository::*`, `teleological::rocksdb_store::*` (full read/write surface).
- **context-graph-embeddings**: `WarmProvider`, `ModelId`, `EmbeddingModel` trait.
- **context-graph-mcp**: `tools::definitions::get_tool_definitions()` (108 tools), `handlers::tools::dispatch::dispatch_tool_call`.
