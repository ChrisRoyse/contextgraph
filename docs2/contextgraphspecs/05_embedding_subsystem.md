# Embedding Subsystem

**Source files covered:**
- `crates/context-graph-embeddings/src/lib.rs`
- `crates/context-graph-embeddings/src/types/model_id/core.rs`
- `crates/context-graph-embeddings/src/types/dimensions/{constants,aggregates,arrays}.rs`
- `crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs`
- `crates/context-graph-embeddings/src/provider/{mod,multi_array,causal_hint}.rs`
- `crates/context-graph-embeddings/src/global_provider.rs`
- `crates/context-graph-embeddings/src/models/pretrained/mod.rs` (and subfolders for each model)
- `crates/context-graph-embeddings/src/models/custom/{temporal_recent,temporal_periodic,temporal_positional,hdc}/`
- `crates/context-graph-embeddings/src/models/registry/{mod,core,loader,operations,stats}.rs`
- `crates/context-graph-embeddings/src/models/factory/default_factory.rs`
- `crates/context-graph-embeddings/src/quantization/{types,binary,float8,pq8,router}/`
- `crates/context-graph-embeddings/src/pruning/{config,token_pruner}.rs`
- `crates/context-graph-embeddings/src/cache/mod.rs`
- `crates/context-graph-embeddings/src/batch/{processor,types}/`
- `crates/context-graph-embeddings/src/adapters/{mod,code_provider}.rs`
- `crates/context-graph-embeddings/src/learner/mod.rs`
- `crates/context-graph-embeddings/src/warm/mod.rs` (warm provider singleton)
- `crates/context-graph-core/src/types/fingerprint/semantic/*.rs` (consumer-side fingerprint shape)
- `crates/context-graph-core/src/weights/mod.rs` (E11/temporal feature flags)
- `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:686-770` (E2 rebuild)

## 1. Purpose

`SemanticFingerprint` (in `context-graph-core`) is the persisted shape that holds every per-embedder vector for one memory: 11 dense slots (E1, E2-E4, E5 cause/effect, E7, E8 source/target, E9, E10 paraphrase/context, E11, E14), 2 sparse slots (E6, E13), and 1 token-level slot (E12). `ProductionMultiArrayProvider` is the singleton that produces a `MultiArrayEmbeddingOutput` (fingerprint + per-embedder latency) for one piece of text. `LearnerFingerprint` (separate type) holds learner-state E15-E21 signal vectors and never enters content fusion.

The `candle` Cargo feature is mandatory for `context-graph-embeddings`; the crate refuses to compile without it (`crates/context-graph-embeddings/src/lib.rs:45`). Default features: `["candle"]`. The `cuda` alias is identical to `candle`.

## 2. Embedder Inventory (E1-E14, NUM_EMBEDDERS=14)

| Slot | ModelId variant | Repository | Native Dim | Stored / Projected | Max tokens | Embedder type | Index type | Asymmetric | Quantization | Latency budget (ms) | Tokenizer family |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| E1 | `Semantic` | intfloat/e5-large-v2 | 1024 | 1024 | 512 | dense | HNSW (+ Matryoshka128 head) | no | PQ-8 (32x) | 5 | BertWordpiece |
| E2 | `TemporalRecent` | sinusoidal PE of timestamp (custom) | 512 | 512 | n/a | dense | HNSW (rebuilt from `created_at`) | no | Float8E4M3 (4x) | 2 | None |
| E3 | `TemporalPeriodic` | Fourier basis (custom) | 512 | 512 | n/a | dense | HNSW | no | Float8E4M3 (4x) | 2 | None |
| E4 | `TemporalPositional` | Sinusoidal PE on session sequence (custom) | 512 | 512 | n/a | dense | HNSW | no | Float8E4M3 (4x) | 2 | None |
| E5 | `Causal` | nomic-ai/nomic-embed-text-v1.5 | 768 | 768 | 512 | dense | HNSW × 3 (combined + cause + effect) | yes (cause/effect) | PQ-8 (32x) | 8 | BertWordpiece |
| E6 | `Sparse` | naver/splade-cocondenser-ensembledistil | 30522 sparse | sparse (1536 if ever projected) | 512 | sparse | Inverted (`CF_E6_SPARSE_INVERTED`) | no | SparseNative | 3 | BertWordpiece |
| E7 | `Code` | Qodo/Qodo-Embed-1-1.5B | 1536 | 1536 | 32768 | dense | HNSW | no | PQ-8 (32x) | 10 | BertWordpiece (loads its own Qwen2 BPE) |
| E8 | `Graph` | intfloat/e5-large-v2 (shares weights with E1) | 1024 | 1024 | 512 | dense | HNSW × 3 (combined + source + target) | yes (source/target) | Float8E4M3 (4x) | 5 | BertWordpiece |
| E9 | `Hdc` | custom hyperdimensional | 10000 binary | 1024 projected | n/a | dense | HNSW | no | Binary (32x) | 1 | None |
| E10 | `Contextual` (serde alias `Multimodal`) | intfloat/e5-base-v2 | 768 | 768 | 512 | dense | HNSW × 2 (paraphrase + context) | yes (paraphrase/context) | PQ-8 (32x) | 15 | BertWordpiece |
| E11 (legacy) | `Entity` | sentence-transformers/all-MiniLM-L6-v2 | 384 | 384 | 512 | dense | HNSW (legacy) | no | Float8E4M3 (4x) | 2 | BertWordpiece |
| E11 (production) | `Kepler` | THU-KEG/KEPLER-Wiki5M-KE (RoBERTa-base + TransE) | 768 | 768 | 512 | dense | HNSW (E11) | no | PQ-8 (32x) | 5 | RobertaBpe |
| E12 | `LateInteraction` | colbert-ir/colbertv2.0 | 128 / token | 128 / token | 512 | multivector | MaxSim (`CF_E12_LATE_INTERACTION`) | no | TokenPruning (~50%) | 8 | BertWordpiece |
| E13 | `Splade` | prithivida/Splade_PP_en_v1 (SPLADE v3) | 30522 sparse | sparse (1536 if ever projected) | 512 | sparse | Inverted (`CF_E13_SPLADE_INVERTED`) | no | SparseNative | 3 | BertWordpiece |
| E14 | `BgeM3Dense` | BAAI/bge-m3 (XLM-RoBERTa-Large dense head) | 1024 | 1024 | 8192 | dense | HNSW | no | PQ-8 (32x) | 50 | XlmRobertaSentencePiece |

### Variant-count caveats

- `ModelId::all()` returns **15** entries because `Entity` (legacy 384D) and `Kepler` (production 768D) both carry the E11 weight slot.
- `ModelId::production()` returns **14** — the production set (no `Entity`).
- `NUM_EMBEDDERS` (in `context_graph_core::types::fingerprint`) is **14**. Always use `NUM_EMBEDDERS` for array sizing — never `ModelId::all().len()`.
- `MODEL_COUNT` in `context_graph_embeddings::types::dimensions::aggregates` is **15** (counts both Entity and Kepler).
- `TOTAL_DIMENSION` = **13056** (compile-time-asserted in `aggregates.rs:48`).

### Feature flags that gate live inference

`context_graph_core::weights::mod.rs`:
- `TEMPORAL_EMBEDDERS_ENABLED: bool = true` (line 42) — when `false`, E2/E3/E4 emit `vec![0.0; 512]` and skip GPU inference.
- `E11_ENTITY_ENABLED: bool = true` (line 48) — when `false`, E11 emits `vec![0.0; 768]` and KEPLER GPU inference is skipped (`provider/multi_array.rs:1136-1144`).

### E14 zero-fallback

`E14ZeroAdapter` (`provider/multi_array.rs:719-742`) is installed when `./models/bge-m3-dense/` is missing `model.safetensors`, `tokenizer.json`, or `config.json`. It returns `Ok(Vec::new())` so the slot stays present in the fingerprint while fusion treats E14 as inactive. A `tracing::warn!` is emitted at startup naming the missing directory.

## 3. HNSW Indexes (15 total)

Even though `NUM_EMBEDDERS=14`, the HNSW layer maintains 15 separate indexes because asymmetric embedders store dual vectors and E1 has a Matryoshka-truncated head:

| # | Index | Vector source | Dim |
| --- | --- | --- | --- |
| 1 | E1 | `e1_semantic` | 1024 |
| 2 | E1Matryoshka128 | `e1_semantic[0..128]` (truncate-then-renormalize) | 128 |
| 3 | E2 | `e2_temporal_recent` | 512 |
| 4 | E3 | `e3_temporal_periodic` | 512 |
| 5 | E4 | `e4_temporal_positional` | 512 |
| 6 | E5 (combined) | concat / averaged cause+effect | 768 |
| 7 | E5CausalCause | `e5_causal_as_cause` | 768 |
| 8 | E5CausalEffect | `e5_causal_as_effect` | 768 |
| 9 | E7 | `e7_code` | 1536 |
| 10 | E8 (combined) | source+target | 1024 |
| 11 | E9 | `e9_hdc` | 1024 |
| 12 | E10MultimodalParaphrase | `e10_multimodal_paraphrase` | 768 |
| 13 | E10MultimodalContext | `e10_multimodal_as_context` | 768 |
| 14 | E11 | `e11_entity` (KEPLER) | 768 |
| 15 | E14 | `e14_bge_m3_dense` | 1024 |

E6, E12, E13 do **not** participate in HNSW — see fusion notes below.

## 4. Provider Trait API

### `EmbeddingProvider` (single-model adapter)

`crates/context-graph-embeddings/src/provider/mod.rs`:

| Method | Signature |
| --- | --- |
| `embed` | `async fn embed(&self, text: &str) -> EmbeddingResult<Vec<f32>>` |
| `embed_batch` | `async fn embed_batch(&self, texts: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>>` (default: per-item loop) |
| `dimension` | `fn dimension(&self) -> usize` |
| `model_name` | `fn model_name(&self) -> &str` |
| `max_tokens` | `fn max_tokens(&self) -> usize` |

### `EmbeddingModel` (per-model implementation trait)

`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs:43-346`:

| Method | Required? | Notes |
| --- | --- | --- |
| `model_id() -> ModelId` | yes | identity |
| `supported_input_types() -> &[InputType]` | yes | drives `validate_input` |
| `embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding>` | yes | core inference |
| `is_initialized() -> bool` | yes | readiness gate |
| `load() -> EmbeddingResult<()>` | default no-op | overridden by pretrained |
| `unload() -> EmbeddingResult<()>` | default no-op | drops GPU tensors |
| `embed_sparse(...) -> EmbeddingResult<(Vec<u16>, Vec<f32>)>` | default `UnsupportedModality` | E6/E13 only |
| `dimension`/`projected_dimension`/`latency_budget_ms`/`max_tokens`/`is_pretrained` | default delegates to `ModelId::*` | |
| `supports_input_type` / `validate_input` | default | |

### Multi-array provider

`MultiArrayEmbeddingProvider` (defined in `context-graph-core::traits`, implemented by `ProductionMultiArrayProvider`):

| Method | Purpose |
| --- | --- |
| `embed_all(content)` | All 14 embedders run in parallel via `tokio::join!`; returns `MultiArrayEmbeddingOutput` |
| `embed_all_with_metadata(content, EmbeddingMetadata)` | Adds E2/E3 timestamp instructions, E4 sequence instruction, E5 LLM causal hint |
| `embed_batch_all(contents, &[EmbeddingMetadata])` | Batch path; spawns one tokio task per content; reuses metadata-aware code (EMB-H1) |
| `model_ids() -> [String; 14]` | Identifiers used for telemetry |
| `is_ready() / health_status()` | Per-embedder readiness |

### Adapter wiring (must override every metadata-aware method)

`provider/multi_array.rs` defines four adapters, each `Send + Sync` (manual `unsafe impl` on `Arc<RwLock<Box<dyn EmbeddingModel>>>`):

| Adapter | Trait implemented | Notes |
| --- | --- | --- |
| `DenseEmbedderAdapter` | `SingleEmbedder` | Stores `model_id` and `dimension`; `embed_with_instruction` for E4 sequence/E2/E3 timestamps |
| `SparseEmbedderAdapter` | `SparseEmbedder` | Calls `model.embed_sparse(...)` (NOT `embed`) so the SPLADE 30522-vocab indices/values are returned, not a 1536D dense projection |
| `TokenEmbedderAdapter` | `TokenEmbedder` | Reshapes ColBERT's flat output `[num_tokens × 128]` into `Vec<Vec<f32>>` |
| `CausalDualEmbedderAdapter` | (concrete, not behind trait) | Holds `Arc<CausalModel>`; exposes `embed_dual()` and `embed_dual_with_hint()` |
| `GraphDualEmbedderAdapter` | (concrete) | Holds `Arc<GraphModel>`; exposes `embed_dual()` |
| `ContextualDualEmbedderAdapter` | (concrete) | Holds `Arc<ContextualModel>`; exposes `embed_dual()` for paraphrase/context |

**Lesson (load-bearing):** `LazyMultiArrayProvider` and any adapter that wraps `MultiArrayEmbeddingProvider` MUST override **both** `embed_all` and `embed_all_with_metadata`. Overriding only `embed_all` causes the metadata-aware path to fall through to the default trait impl that drops the metadata, silently losing E2/E3 timestamp prefixes, the E4 sequence number, and any E5 causal hint.

## 5. E2 Sinusoidal Position Encoding (production path)

`crates/context-graph-embeddings/src/models/custom/temporal_recent/compute.rs:42-95`:

```text
compute_decay_embedding(timestamp: DateTime<Utc>, reference_time: Option<DateTime<Utc>>, decay_rates: &[f32]) -> Vec<f32>
  reference_time = None  -> compute_sinusoidal_recency(timestamp)   // PRODUCTION
  reference_time = Some  -> compute_legacy_decay(...)               // TESTS ONLY
```

For production (`reference_time = None`) the absolute Unix timestamp `t = timestamp.timestamp()` is encoded transformer-style:

```text
for k in 0..256:
    divisor = 86400.0 ** (2k/512)         // base = 1 day in seconds
    angle   = t / divisor
    vector[2k]   = sin(angle)
    vector[2k+1] = cos(angle)
L2-normalize(vector)                      // 512D
```

- k=0: wavelength ≈ 6.28 s (second-level discrimination)
- k=127: wavelength ≈ days
- k=255: wavelength ≈ months

Why this matters: the previous decay-only encoding produced `delta ≈ 0` for every memory because embedding happens immediately after `created_at`, so all E2 vectors were nearly identical and the E2 HNSW index was useless. The sinusoidal path produces distinct, non-degenerate vectors keyed on creation time.

### E2 rebuild on store open

`crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:686-770` (`rebuild_indexes_from_store`):
- Iterates `CF_FINGERPRINTS`, skipping soft-deleted records.
- Recomputes E2 from `fp.created_at` via `compute_decay_embedding(fp.created_at, None, &DEFAULT_DECAY_RATES)`.
- Replaces `fp.semantic.e2_temporal_recent` and adds the fingerprint to all 15 HNSW indexes.
- Acquires the write side of `compaction_lock` to serialize against concurrent `store`/`delete`.
- Persisted E2 HNSW from before the fix is intentionally discarded so newly-rebuilt vectors take effect.

## 6. E5 / E8 / E10 Asymmetric Storage and Direction Bias

The three asymmetric embedders each store **two** vectors per memory and emit dual outputs from `embed_dual()`.

### E5 — Causal (`models/pretrained/causal/`)

- 768D from nomic-embed-text-v1.5; max 512 tokens (capped from upstream 8192).
- Instruction prefixes (`config.rs:21-27`):
  - `CAUSE_INSTRUCTION = "search_query: Identify the cause in: "`
  - `EFFECT_INSTRUCTION = "search_query: Identify the effect of: "`
- A trained projection head (`TrainableProjection`) plus optional LoRA layers post-process the encoder output. Loaded eagerly via `e5_causal_model.load_trained_weights(&models/causal/trained)` — the provider FAILS FAST if these are missing (no silent fallback to base weights).
- `embed_dual_with_hint()` accepts a `CausalHint` (LLM-derived). When `hint.is_useful()`:
  - direction-bias multiply: `cause_vec *= cause_bias`, `effect_vec *= effect_bias` from `hint.bias_factors()` (default 1.3 / 0.8 for `Cause`, swapped for `Effect`, `1.0/1.0` for `Neutral`).
  - LLM-supplied marker spans get injected via `embed_dual_guided`.

### E8 — Graph (`models/pretrained/graph/`)

- 1024D, shares e5-large-v2 weights with E1 (constructor at `provider/multi_array.rs:791` reads from `models_dir.join("semantic")`).
- Asymmetry comes from learned source/target projections (`W_source`, `W_target`).
- Direction multipliers applied at fusion time: source→target preferred when query is a "what does X use?" probe.

### E10 — Contextual (`models/pretrained/contextual/`)

- 768D from intfloat/e5-base-v2. Constants `INTENT_PREFIX` and `CONTEXT_PREFIX` provide asymmetric prompts (`models/pretrained/contextual/`).
- Direction multipliers per plan: paraphrase→context = 1.2x, context→paraphrase = 0.8x.

## 7. E1 Matryoshka-128 Head

E1 ships a second HNSW index that holds the first 128 dimensions of the L2-normalized E1 vector (after re-normalization). Used for cheap candidate generation; downstream stages re-rank using full 1024D. No additional inference call — the head is a slice of the existing E1 vector.

## 8. Custom (no-weights) embedders

| Slot | Implementation | Output |
| --- | --- | --- |
| E2 | `models/custom/temporal_recent/` | 512D sinusoidal PE of `created_at` (production) or exponential decay (tests via `with_reference_time`) |
| E3 | `models/custom/temporal_periodic/` | 512D Fourier basis of timestamp (hour-of-day, day-of-week, month-of-year, etc.) |
| E4 | `models/custom/temporal_positional/` | 512D mixed signature — first 256D session-signature, last 256D sinusoidal PE of `sequence:N` or `epoch:N` instruction |
| E9 | `models/custom/hdc/` | 10000-bit hypervector projected to 1024D via fixed projection |

E2/E3/E4 honor `TEMPORAL_EMBEDDERS_ENABLED`. The provider passes `metadata.e{2,3,4}_instruction()` (e.g., `"timestamp:1714358400"`, `"sequence:42"`) when invoked through `embed_all_with_metadata`.

## 9. Pretrained model loading

`models/pretrained/mod.rs` re-exports concrete model types and constants:

| Model file | Type | Constants |
| --- | --- | --- |
| `bge_m3_dense.rs` | `BgeM3DenseModel` | `BGE_M3_DENSE_DIMENSION=1024`, `BGE_M3_DENSE_MAX_TOKENS=8192`, `BGE_M3_DENSE_LATENCY_BUDGET_MS=50`, `XLM_R_BOS_TOKEN_ID`, `XLM_R_PAD_TOKEN_ID`, `XLM_R_POSITION_OFFSET`, `XLM_R_WEIGHT_PREFIX` |
| `causal/` | `CausalModel`, `TrainableProjection`, `NomicWeights` | `CAUSAL_DIMENSION=768`, `CAUSAL_MAX_TOKENS=512`, `CAUSAL_LATENCY_BUDGET_MS=8`, `CAUSE_INSTRUCTION`, `EFFECT_INSTRUCTION` |
| `code/` | `CodeModel` | `CODE_NATIVE_DIMENSION=1536`, `CODE_PROJECTED_DIMENSION=1536`, `CODE_MAX_TOKENS=32768`, `CODE_LATENCY_BUDGET_MS=10`, `CODE_MODEL_NAME` |
| `contextual/` | `ContextualModel` | `CONTEXTUAL_DIMENSION=768`, `INTENT_PREFIX`, `CONTEXT_PREFIX`, `CONTEXTUAL_HIDDEN_SIZE`, `CONTEXTUAL_INTERMEDIATE_SIZE`, `CONTEXTUAL_NUM_HEADS`, `CONTEXTUAL_NUM_LAYERS`, `CONTEXTUAL_LAYER_NORM_EPS`, `CONTEXTUAL_VOCAB_SIZE`, `CONTEXTUAL_MAX_TOKENS=512` |
| `graph/` | `GraphModel` | `GRAPH_DIMENSION=1024`, `MAX_CONTEXT_NEIGHBORS`, `GRAPH_MAX_TOKENS=512`, `GRAPH_LATENCY_BUDGET_MS=5` |
| `kepler/` | `KeplerModel` | `KEPLER_DIMENSION=768`, `KEPLER_MAX_TOKENS=512`, `KEPLER_LATENCY_BUDGET_MS=5`, `KEPLER_MODEL_NAME` |
| `late_interaction/` | `LateInteractionModel`, `TokenEmbeddings` | `LATE_INTERACTION_DIMENSION=128`, `LATE_INTERACTION_MAX_TOKENS=512`, `LATE_INTERACTION_LATENCY_BUDGET_MS=8` |
| `rgcn/` | `RelationalGCN` | `INPUT_DIM`, `HIDDEN_DIM`, `RGCN_OUTPUT_DIM`, `NUM_BASES`, `NUM_RELATIONS`, `RELATION_NAMES`, `RGCN_WEIGHTS_PATH`, `DEFAULT_CONFIG_PATH` |
| `semantic/` | `SemanticModel` | `SEMANTIC_DIMENSION=1024`, `QUERY_PREFIX="query: "`, `PASSAGE_PREFIX="passage: "`, `SEMANTIC_MAX_TOKENS=512` |
| `sparse/` | `SparseModel`, `SparseVector` | `SPARSE_NATIVE_DIMENSION=30522`, `SPARSE_PROJECTED_DIMENSION=1536`, `SPARSE_EXPECTED_SPARSITY`, `SPARSE_MAX_TOKENS=512`, `SPARSE_LATENCY_BUDGET_MS=3` |
| `weight_projection/` | `LearnedWeightProjection` | `NUM_EMBEDDERS=14`, `HIDDEN_DIM_1`, `HIDDEN_DIM_2`, `OUTPUT_DIM`, `MAX_WEIGHTED_AGREEMENT`, `WEIGHTED_AGREEMENT_THRESHOLD`, `DEFAULT_CATEGORY_WEIGHTS`, `DEFAULT_WEIGHTS_PATH` |

Construction order in `ProductionMultiArrayProvider::new` (`provider/multi_array.rs:771-952`):
1. Build `DefaultModelFactory` with `models_dir` + `gpu_config`.
2. Use `SingleModelConfig::cuda_fp16()` for all pretrained models.
3. Construct E1, E2, E3, E4 via factory; E5 directly (`CausalModel::new(&models_dir.join("causal"), …)`); E6, E7 via factory; E8 directly (`GraphModel::new(&models_dir.join("semantic"), …)` — shares e5-large-v2 with E1); E9 via factory; E10 directly (`ContextualModel::new(&models_dir.join("contextual"), …)`); E11 = `factory.create_model(ModelId::Kepler, …)`; E12, E13 via factory.
4. **Eagerly call `model.load().await?`** on every model in order (E1→E13).
5. After loading the base causal model, FAIL FAST if `models/causal/trained/{lora,projection}.safetensors` is missing — the search gate depends on those weights.
6. E14 is conditional: probe `models/bge-m3-dense/{model.safetensors,tokenizer.json,config.json}`. If all three exist, load the full BGE-M3 dense head; otherwise install `E14ZeroAdapter` and emit `tracing::warn!`.
7. Wrap loaded models in adapters and store in struct fields.

### Lazy / on-demand path: `ModelRegistry`

`crates/context-graph-embeddings/src/models/registry/`:
- Lazy loading: models loaded on first `get_model(ModelId)` call.
- Per-model load locks (serialize concurrent requests for the same model).
- Memory budget tracking via `MemoryTracker` — refuses to load when total VRAM would exceed `ModelRegistryConfig::memory_budget`.
- LRU-ish unload via `max_loaded_models` (default 14 — the entire production set).
- Used primarily by tests and `BatchProcessor`; production runtime path uses the warm singleton.

### Warm singleton

`crates/context-graph-embeddings/src/global_provider.rs`:
- `static GLOBAL_WARM_PROVIDER: OnceLock<Arc<RwLock<ProviderState>>>`.
- `initialize_global_warm_provider()` is called once at MCP/CLI startup. Subsequent calls are no-ops on success; on failure the error is cached and never retried.
- `get_warm_provider()` returns the trait object; `get_warm_causal_model()` / `get_warm_graph_model()` return concrete `Arc<CausalModel>` / `Arc<GraphModel>` for the discovery services.
- `CONTEXT_GRAPH_MODELS_PATH` env var overrides `./models`.

The dedicated warm subsystem under `src/warm/` (loader, memory_pool, validation, diagnostics, health, inference, integration, cuda_alloc, registry) provides the lower-level allocator/health-check machinery — see [11_cuda_acceleration.md](11_cuda_acceleration.md) for the VRAM allocation lifecycle.

## 10. BatchProcessor and dynamic batching

`src/batch/processor/{core,worker,submit,config,stats}.rs`:
- One `BatchQueue` per `ModelId` (initialised for **all 15 variants** including legacy `Entity`).
- `mpsc::channel(request_buffer_size)` feeds a single worker task that:
  - polls per-model queues
  - flushes when `batch.len() >= max_batch_size` or `oldest_age >= max_wait`
  - bounded concurrency via `Semaphore::new(max_concurrent_batches)`
- Stats: requests submitted/completed/failed/timed-out, batches flushed by trigger, queue depths.
- Shutdown drains in-flight batches before joining the worker.

Batch types (`src/batch/types/`): `BatchRequest`, `BatchQueue`, `Batch`, `BatchStats`.

## 11. Cache layer

`src/cache/mod.rs`:
- Backed by `moka::sync::Cache<CacheKey, MultiArrayEmbedding>`.
- `CacheKey(u64)` = `xxhash_rust::xxh64(content.as_bytes(), 0)`.
- Tracked via atomic `CacheStats { hits, misses, ... }`.
- Eviction: LRU / LFU / Tiny-LFU controlled by `CacheConfig` (`crates/context-graph-embeddings/src/config/cache.rs`).
- TTL configurable; no fallback on cache failure (errors propagate).
- Roughly ~6 KB per entry (13 native-dim embeddings packed inline).

## 12. Quantization (`src/quantization/`)

`QuantizationMethod::for_model_id(ModelId)` is the routing table (no fallbacks):

| Method | Compression | Max recall loss | Embedders |
| --- | --- | --- | --- |
| `PQ8` | 32x | <5% | E1, E5, E7, E10, E11 (Kepler), E14 |
| `Float8E4M3` | 4x | <0.3% | E2, E3, E4, E8, legacy E11 (Entity) |
| `Binary` | 32x | 5–10% | E9 |
| `SparseNative` | native (depends on sparsity) | 0% | E6, E13 |
| `TokenPruning` | ~2x | <2% | E12 |

Implementations:
- `binary.rs` — sign-bit pack for E9.
- `float8.rs` — E4M3 scalar quantize/dequantize for temporal/graph slots.
- `pq8/` — Product Quantization with 8 subvectors × 256 centroids per subvector. The codebook (`PQ8Codebook`) stores actual per-subvector min/max from training data — hardcoding `[-1, 1]` corrupts dequantization for any model whose normalized output is asymmetric (load-bearing lesson). Submodules: `encoder.rs`, `training.rs`, `persistence.rs`, `types.rs`.
- `router/` — `QuantizationRouter` dispatches `(ModelId, &[f32]) -> QuantizedEmbedding` and reverses on read.

`QuantizedEmbedding` shape: `{ method: QuantizationMethod, original_dim: usize, data: Vec<u8>, metadata: QuantizationMetadata }`.

**Asymmetric quantization (E5/E8/E10):** both vectors must be quantized independently — cloning cause→effect destroys the dual-vector signal entirely.

## 13. Token pruning (E12)

`src/pruning/token_pruner.rs` + `src/pruning/config.rs`:
- `TokenPruningQuantizer::prune(&[Vec<f32>], optional_attention_weights)` keeps the top fraction of tokens by importance.
- Importance scoring methods (`ImportanceScoringMethod`): `EmbeddingMagnitude`, `AttentionWeights`, `Combined`.
- Default config: `target_compression=0.5`, `min_tokens=10`, `scoring_method=EmbeddingMagnitude`.
- Output: `PrunedEmbeddings { embeddings, kept_indices, original_count }`.

## 14. Adapters (cross-crate trait bridges)

`src/adapters/code_provider.rs`: `E7CodeEmbeddingProvider` wraps `CodeModel` so it implements the `CodeEmbeddingProvider` trait defined in `context-graph-core`. Used by code-search ingestion paths.

## 15. Learner-state E15-E21

`src/learner/mod.rs`:

| Slot | `LearnerEmbedderSlot` | Modality | Native dim | Model name | Path |
| --- | --- | --- | --- | --- | --- |
| E15 | `E15AffectSpeech` | AffectSpeech | 1024 | Wav2Vec2-large-MSP-DIM | `affect-speech` |
| E16 | `E16AffectFace` | AffectFace | 512 | OpenFace 3.0 + MTL emotion/AU/gaze | `affect-face` |
| E17 | `E17AffectText` | AffectText | 384 | MiniLM-L6-v2 + EmoBank regressor | `affect-text` |
| E18 | `E18Ppg` | Ppg | 512 | PaPaGei PPG foundation model | `ppg` |
| E19 | `E19Eda` | Eda | 64 | WESAD chest physiology stress head | `eda` |
| E20 | `E20Eeg` | Eeg | 768 | LaBraM EEG foundation model | `eeg` |
| E21 | `E21EegArtifactRobust` | EegArtifactRobust | 768 | EEGPT artifact-robust EEG model | `eeg-robust` |

Constants:
- `LEARNER_EMBEDDER_COUNT = 7`
- `UTL_PLANNED_TOTAL_EMBEDDERS = 21` (14 content + 7 learner)
- `UTL_CONTENT_EMBEDDERS_BEFORE_LEARNER_STATE = 14`

These slots back `LearnerStateVector` and goal-distance computations only; they never enter content fusion. See [20_learning_subsystem.md](20_learning_subsystem.md).

## 16. SemanticFingerprint Field Layout

`crates/context-graph-core/src/types/fingerprint/semantic/`:

| Field | Representation | Notes |
| --- | --- | --- |
| `e1_semantic` | `Vec<f32>` (1024D) | dense |
| `e2_temporal_recent` / `e3_temporal_periodic` / `e4_temporal_positional` | `Vec<f32>` (512D each) | dense |
| `e5_causal_as_cause` / `e5_causal_as_effect` | `Vec<f32>` (768D each) | asymmetric, current production path |
| `e5_causal` | `Vec<f32>` | legacy combined slot, written empty by current providers |
| `e6_sparse` | `SparseVector { indices: Vec<u16>, values: Vec<f32> }` | up to 30522 vocab |
| `e7_code` | `Vec<f32>` (1536D) | dense |
| `e8_graph_as_source` / `e8_graph_as_target` | `Vec<f32>` (1024D each) | asymmetric |
| `e8_graph` | `Vec<f32>` | legacy, empty |
| `e9_hdc` | `Vec<f32>` (1024D projected) | dense |
| `e10_multimodal_paraphrase` / `e10_multimodal_as_context` | `Vec<f32>` (768D each) | asymmetric |
| `e11_entity` | `Vec<f32>` (768D KEPLER) | dense |
| `e12_late_interaction` | `Vec<Vec<f32>>` (128D / token, variable token count) | multivector |
| `e13_splade` | `SparseVector` | up to 30522 vocab |
| `e14_bge_m3_dense` | `Vec<f32>` (1024D, may be empty) | dense, empty when E14 zero-adapter is active |

Validation errors raised by `SemanticFingerprint::validate()`: `DimensionMismatch`, `EmptyDenseEmbedding`, `TokenDimensionMismatch`, `SparseIndexOutOfBounds`, `SparseIndicesValuesMismatch`.

### Relevant constants (`semantic/constants.rs`)

| Constant | Value |
| --- | --- |
| `NUM_EMBEDDERS` | 14 |
| `E1_DIM` | 1024 |
| `E2_DIM` / `E3_DIM` / `E4_DIM` | 512 |
| `E5_DIM` | 768 |
| `E6_SPARSE_VOCAB` | 30522 |
| `E7_DIM` | 1536 |
| `E8_DIM` | 1024 |
| `E9_DIM` | 1024 |
| `E10_DIM` | 768 |
| `E11_DIM` | 768 |
| `E12_TOKEN_DIM` | 128 |
| `E13_SPLADE_VOCAB` | 30522 |
| `E14_DIM` | 1024 |
| `TELEOLOGICAL_VECTOR_DIM` | 13 (E14 excluded from the compact teleological vector) |
| `TOTAL_DENSE_DIMS` | sum of dense dims |

## 17. Tokenizer / instruction-prefix gotchas

- E1 (e5-large-v2) uses `query:` for queries and `passage:` for documents (`PASSAGE_PREFIX`/`QUERY_PREFIX` in `models/pretrained/semantic/constants.rs`). E1 normalizer assumes the prefix matches what training used.
- E5 (nomic-embed) uses `search_query:` style instructions. The training pipeline (`training/pipeline.rs:195`) deliberately switches to `search_document:` for both cause and effect during training because instruction prefixes were collapsing all vectors to the same direction.
- E2 instruction (production): `"timestamp:<unix_seconds>"` — but the sinusoidal path ignores the instruction and reads `created_at` directly (the instruction is informational only).
- E4 instruction: `"sequence:<n>"` or `"epoch:<n>"`.
- E5 instructions used at inference: `CAUSE_INSTRUCTION`, `EFFECT_INSTRUCTION`.

## 18. Cross-references

- HNSW search and fusion: [06_search_and_fusion.md](06_search_and_fusion.md).
- Storage of fingerprint + index + quantized payloads: [09_storage_layer.md](09_storage_layer.md).
- CUDA acceleration (kernels, allocator, candle device): [11_cuda_acceleration.md](11_cuda_acceleration.md).
- LoRA + projection training that produces the E5 trained weights: [16_training_subsystem.md](16_training_subsystem.md).
- Constellation aggregation over fingerprints: [17_constellation_subsystem.md](17_constellation_subsystem.md).
- Learner-state subsystem (E15-E21): [20_learning_subsystem.md](20_learning_subsystem.md).
