# Embedding Subsystem

**Source files covered:**
- `crates/context-graph-embeddings/src/lib.rs`
- `crates/context-graph-embeddings/src/types/model_id/{core,conversions,display,repository,tokenizer,tests}.rs`
- `crates/context-graph-embeddings/src/types/dimensions/{constants,aggregates,arrays,helpers}.rs`
- `crates/context-graph-embeddings/src/traits/{embedding_model,model_factory,mod}.rs`
- `crates/context-graph-embeddings/src/provider/{mod,multi_array,causal_hint,diagnostic_test}.rs`
- `crates/context-graph-embeddings/src/global_provider.rs`
- `crates/context-graph-embeddings/src/models/pretrained/mod.rs` and 12 model subdirs (`bge_m3_dense`, `causal`, `code`, `contextual`, `graph`, `kepler`, `late_interaction`, `rgcn`, `semantic`, `sparse`, `weight_projection`, `shared.rs`)
- `crates/context-graph-embeddings/src/models/custom/{temporal_recent,temporal_periodic,temporal_positional,hdc}/`
- `crates/context-graph-embeddings/src/models/factory/{default_factory,trait_impl}.rs`
- `crates/context-graph-embeddings/src/models/registry/{mod,core,config,loader,operations,stats}.rs`
- `crates/context-graph-embeddings/src/models/{attention,memory_tracker}.rs`
- `crates/context-graph-embeddings/src/quantization/{mod,types,binary,float8,pq8/*,router/*}`
- `crates/context-graph-embeddings/src/pruning/{config,token_pruner,mod}.rs`
- `crates/context-graph-embeddings/src/cache/mod.rs`
- `crates/context-graph-embeddings/src/batch/{processor/*,types/*}`
- `crates/context-graph-embeddings/src/adapters/{mod,code_provider}.rs`
- `crates/context-graph-embeddings/src/learner/mod.rs` (1,723 LOC, all E15-E21 specs)
- `crates/context-graph-embeddings/src/warm/{mod,config,state,handle,error,registry,memory_pool/*,validation,cuda_alloc/*,loader/*,diagnostics,inference/*,integration,health/*}.rs`
- `crates/context-graph-embeddings/src/gpu/{device,memory,model_loader,tensor,ops}/`
- `crates/context-graph-embeddings/src/storage/{mod,multi_space,types/*}.rs`
- `crates/context-graph-embeddings/src/training/{lora,pipeline,trainer,loss,optimizer,multitask,evaluation,distillation,data,mod}.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/*.rs` (consumer-side fingerprint shape)
- `crates/context-graph-core/src/weights/mod.rs` (E11/temporal feature flags)
- `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:686-770` (E2 rebuild)

## 1. Purpose

`SemanticFingerprint` (in `context-graph-core`) is the persisted shape that holds every per-embedder vector for one memory: 11 dense slots (E1, E2-E4, E5 cause/effect, E7, E8 source/target, E9, E10 paraphrase/context, E11, E14), 2 sparse slots (E6, E13), and 1 token-level slot (E12). `ProductionMultiArrayProvider` is the singleton that produces a `MultiArrayEmbeddingOutput` (fingerprint + per-embedder latency) for one piece of text. `LearnerFingerprint` (separate type) holds learner-state E15-E21 signal vectors and never enters content fusion.

The `candle` Cargo feature is mandatory for `context-graph-embeddings`; the crate refuses to compile without it (`crates/context-graph-embeddings/src/lib.rs:45-51`). Default features: `["candle"]`. The `cuda` alias is identical to `candle`. Library re-exports cover provider, error, traits, model registry, quantization, pruning, storage, learner, adapters, and the global warm provider (`lib.rs:53-134`).

## 2. Embedder Inventory (E1-E14, NUM_EMBEDDERS=14)

| Slot | ModelId variant | Repository | Native Dim | Stored / Projected | Max tokens | Embedder type | Index type | Asymmetric | Quantization | Latency budget (ms) | Tokenizer family |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| E1 | `Semantic` | intfloat/e5-large-v2 | 1024 | 1024 | 512 | dense | HNSW (+ Matryoshka128 head) | no | PQ-8 (32x) | 5 | BertWordpiece |
| E2 | `TemporalRecent` | sinusoidal PE of timestamp (custom) | 512 | 512 | unlimited | dense | HNSW (rebuilt from `created_at`) | no | Float8E4M3 (4x) | 2 | None |
| E3 | `TemporalPeriodic` | Fourier basis (custom) | 512 | 512 | unlimited | dense | HNSW | no | Float8E4M3 (4x) | 2 | None |
| E4 | `TemporalPositional` | Sinusoidal PE on session sequence (custom) | 512 | 512 | unlimited | dense | HNSW | no | Float8E4M3 (4x) | 2 | None |
| E5 | `Causal` | nomic-ai/nomic-embed-text-v1.5 | 768 | 768 | 512 | dense | HNSW × 3 (combined + cause + effect) | yes (cause/effect) | PQ-8 (32x) | 8 | BertWordpiece |
| E6 | `Sparse` | naver/splade-cocondenser-ensembledistil | 30522 sparse | sparse (1536 if ever projected) | 512 | sparse | Inverted (`CF_E6_SPARSE_INVERTED`) | no | SparseNative | 3 | BertWordpiece |
| E7 | `Code` | Qodo/Qodo-Embed-1-1.5B | 1536 | 1536 | 32768 | dense | HNSW | no | PQ-8 (32x) | 10 | BertWordpiece (loads its own Qwen2 BPE) |
| E8 | `Graph` | intfloat/e5-large-v2 (shares weights with E1) | 1024 | 1024 | 512 | dense | HNSW × 3 (combined + source + target) | yes (source/target) | Float8E4M3 (4x) | 5 | BertWordpiece |
| E9 | `Hdc` | custom hyperdimensional | 10000 | 1024 projected | unlimited | dense | HNSW | no | Binary (32x) | 1 | None |
| E10 | `Contextual` (serde alias `Multimodal`) | intfloat/e5-base-v2 | 768 | 768 | 512 | dense | HNSW × 2 (paraphrase + context) | yes (paraphrase/context) | PQ-8 (32x) | 15 | BertWordpiece |
| E11 (legacy) | `Entity` | sentence-transformers/all-MiniLM-L6-v2 | 384 | 384 | 512 | dense | HNSW (legacy) | no | Float8E4M3 (4x) | 2 | BertWordpiece |
| E11 (production) | `Kepler` | THU-KEG/KEPLER-Wiki5M-KE (RoBERTa-base + TransE) | 768 | 768 | 512 | dense | HNSW (E11) | no | PQ-8 (32x) | 5 | RobertaBpe |
| E12 | `LateInteraction` | colbert-ir/colbertv2.0 | 128 / token | 128 / token | 512 | multivector | MaxSim (`CF_E12_LATE_INTERACTION`) | no | TokenPruning (~50%) | 8 | BertWordpiece |
| E13 | `Splade` | prithivida/Splade_PP_en_v1 (SPLADE v3) | 30522 sparse | sparse (1536 if ever projected) | 512 | sparse | Inverted (`CF_E13_SPLADE_INVERTED`) | no | SparseNative | 3 | BertWordpiece |
| E14 | `BgeM3Dense` | BAAI/bge-m3 (XLM-RoBERTa-Large dense head) | 1024 | 1024 | 8192 | dense | HNSW | no | PQ-8 (32x) | 50 | XlmRobertaSentencePiece |

Native dimensions and projected dimensions are computed in `ModelId::dimension()` / `projected_dimension()` (`types/model_id/core.rs:85-123`). Maximum tokens come from `max_tokens()` (`core.rs:148-163`). Latency budgets come from `latency_budget_ms()` (`core.rs:267-283`).

### Variant-count caveats

- `ModelId::all()` returns **15** entries (`types/model_id/core.rs:200-218`) because `Entity` (legacy 384D MiniLM) and `Kepler` (production 768D KEPLER) both carry the E11 weight slot.
- `ModelId::production()` returns **14** (`core.rs:234-251`) — the production set (no `Entity`).
- `NUM_EMBEDDERS` (in `context_graph_core::types::fingerprint`) is **14**. Always use `NUM_EMBEDDERS` for array sizing — never `ModelId::all().len()`.
- `MODEL_COUNT` in `context_graph_embeddings::types::dimensions::aggregates` is **15** (counts both Entity and Kepler).
- `TOTAL_DIMENSION` = **13056** (compile-time-asserted in `aggregates.rs:47-48`).

### Feature flags that gate live inference

`context_graph_core::weights::mod.rs`:
- `TEMPORAL_EMBEDDERS_ENABLED: bool = true` (line 42) — when `false`, E2/E3/E4 emit `vec![0.0; 512]` and skip GPU inference (`provider/multi_array.rs:1093-1109`).
- `E11_ENTITY_ENABLED: bool = true` (line 48) — when `false`, E11 emits `vec![0.0; 768]` and KEPLER GPU inference is skipped (`provider/multi_array.rs:1136-1144`); also strips E11 from active weight profiles (`weights/mod.rs:705,1054`).

### E14 zero-fallback

`E14ZeroAdapter` (`provider/multi_array.rs:719-742`) is installed when `./models/bge-m3-dense/` is missing `model.safetensors`, `tokenizer.json`, or `config.json`. It returns `Ok(Vec::new())` so the slot stays present in the fingerprint while fusion treats E14 as inactive. A `tracing::warn!` is emitted at startup naming the missing directory (`provider/multi_array.rs:905-913`).

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

E6, E12, E13 do **not** participate in HNSW — see [06_search_and_fusion.md](06_search_and_fusion.md) for inverted/MaxSim handling.

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

`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs`:

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

`provider/multi_array.rs` defines four adapters:

| Adapter | Trait implemented | Notes |
| --- | --- | --- |
| `DenseEmbedderAdapter` | `SingleEmbedder` | Stores `model_id` and `dimension`; `embed_with_instruction` for E4 sequence/E2/E3 timestamps |
| `SparseEmbedderAdapter` | `SparseEmbedder` | Calls `model.embed_sparse(...)` (NOT `embed`) so the SPLADE 30522-vocab indices/values are returned, not a 1536D dense projection |
| `TokenEmbedderAdapter` | `TokenEmbedder` | Reshapes ColBERT's flat output `[num_tokens × 128]` into `Vec<Vec<f32>>` |
| `CausalDualEmbedderAdapter` | (concrete, not behind trait) | Holds `Arc<CausalModel>`; exposes `embed_dual()` and `embed_dual_with_hint()` |
| `GraphDualEmbedderAdapter` | (concrete) | Holds `Arc<GraphModel>`; exposes `embed_dual()` |
| `ContextualDualEmbedderAdapter` | (concrete) | Holds `Arc<ContextualModel>`; exposes `embed_dual()` for paraphrase/context |
| `E14ZeroAdapter` | `SingleEmbedder` | Emits `Vec::new()` when BGE-M3 weights are absent; logs WARN at startup |

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

- 768D from nomic-embed-text-v1.5; max 512 tokens (capped from upstream 8192). `NomicConfig` defaults: vocab=30528, layers=12, heads=12, intermediate=3072, rotary_base=1000.0, no biases (`config.rs:36-87`).
- Instruction prefixes (`config.rs:21-27`):
  - `CAUSE_INSTRUCTION = "search_query: Identify the cause in: "`
  - `EFFECT_INSTRUCTION = "search_query: Identify the effect of: "`
- A trained projection head (`TrainableProjection`) plus optional LoRA layers post-process the encoder output. Loaded eagerly via `e5_causal_model.load_trained_weights(&models/causal/trained)` — the provider FAILS FAST if these are missing (`provider/multi_array.rs:815-816`); no silent fallback to base weights.
- `embed_dual_with_hint()` accepts a `CausalHint` (LLM-derived). When `hint.is_useful()`:
  - direction-bias multiply: `cause_vec *= cause_bias`, `effect_vec *= effect_bias` from `hint.bias_factors()` (default 1.3 / 0.8 for `Cause`, swapped for `Effect`, `1.0/1.0` for `Neutral`).
  - LLM-supplied marker spans get injected via `embed_dual_guided`.

### E8 — Graph (`models/pretrained/graph/`)

- 1024D, shares e5-large-v2 weights with E1. Constructor at `provider/multi_array.rs:791` reads from `models_dir.join("semantic")` (path is intentionally `semantic`, not `graph`, to avoid duplicating the 600 MB weight file).
- Asymmetry comes from learned source/target projections (`projections.rs` defines `W_source`, `W_target`).
- Direction multipliers applied at fusion time: source→target preferred when query is a "what does X use?" probe.

### E10 — Contextual (`models/pretrained/contextual/`)

- 768D from intfloat/e5-base-v2. `CONTEXTUAL_HIDDEN_SIZE=768`, `CONTEXTUAL_INTERMEDIATE_SIZE=3072`, 12 layers, 12 heads, `CONTEXTUAL_LAYER_NORM_EPS=1e-12`, `CONTEXTUAL_VOCAB_SIZE=30522` (`constants.rs:11-40`).
- Asymmetric prefixes (`constants.rs:55-59`): `INTENT_PREFIX = "query: "`, `CONTEXT_PREFIX = "passage: "`. These produce learned asymmetric representations without separate projection matrices.
- Direction multipliers per plan: paraphrase→context = 1.2x, context→paraphrase = 0.8x.

## 7. E1 Matryoshka-128 Head

E1 ships a second HNSW index that holds the first 128 dimensions of the L2-normalized E1 vector (after re-normalization). Used for cheap candidate generation; downstream stages re-rank using full 1024D. No additional inference call — the head is a slice of the existing E1 vector.

## 8. Custom (no-weights) embedders

| Slot | Implementation | Output |
| --- | --- | --- |
| E2 | `models/custom/temporal_recent/` | 512D sinusoidal PE of `created_at` (production) or exponential decay (tests via `with_reference_time`) |
| E3 | `models/custom/temporal_periodic/` | 512D Fourier basis of timestamp (hour-of-day, day-of-week, month-of-year, etc.) |
| E4 | `models/custom/temporal_positional/` | 512D mixed signature — first 256D session-signature (`session_signature.rs`), last 256D sinusoidal PE of `sequence:N` or `epoch:N` instruction (`encoding.rs`/`timestamp.rs`) |
| E9 | `models/custom/hdc/` | 10000-bit hypervector (`encoding.rs`) projected to 1024D via fixed projection (`operations.rs`); trigram features (`ngram_size=3`, `seed=42`) |

E2/E3/E4 honor `TEMPORAL_EMBEDDERS_ENABLED`. The provider passes `metadata.e{2,3,4}_instruction()` (e.g., `"timestamp:1714358400"`, `"sequence:42"`) when invoked through `embed_all_with_metadata`.

## 9. Pretrained model loading

`models/pretrained/mod.rs` re-exports concrete model types and constants:

| Model file | Type | Constants |
| --- | --- | --- |
| `bge_m3_dense/` | `BgeM3DenseModel` | `BGE_M3_DENSE_DIMENSION=1024`, `BGE_M3_DENSE_MAX_TOKENS=8192`, `BGE_M3_DENSE_LATENCY_BUDGET_MS=50`, `XLM_R_BOS_TOKEN_ID`, `XLM_R_PAD_TOKEN_ID`, `XLM_R_POSITION_OFFSET`, `XLM_R_WEIGHT_PREFIX` |
| `causal/` | `CausalModel`, `TrainableProjection`, `NomicWeights` | `CAUSAL_DIMENSION=768`, `CAUSAL_MAX_TOKENS=512`, `CAUSAL_LATENCY_BUDGET_MS=8`, `CAUSE_INSTRUCTION`, `EFFECT_INSTRUCTION` |
| `code/` | `CodeModel` | `CODE_NATIVE_DIMENSION=1536`, `CODE_PROJECTED_DIMENSION=1536`, `CODE_MAX_TOKENS=32768`, `CODE_LATENCY_BUDGET_MS=10`, `CODE_MODEL_NAME="Qodo/Qodo-Embed-1-1.5B"` |
| `contextual/` | `ContextualModel` | `CONTEXTUAL_DIMENSION=768`, `CONTEXTUAL_MAX_TOKENS=512`, `CONTEXTUAL_LATENCY_BUDGET_MS=20`, `INTENT_PREFIX="query: "`, `CONTEXT_PREFIX="passage: "`, `CONTEXTUAL_HIDDEN_SIZE=768`, `CONTEXTUAL_INTERMEDIATE_SIZE=3072`, `CONTEXTUAL_NUM_HEADS=12`, `CONTEXTUAL_NUM_LAYERS=12`, `CONTEXTUAL_LAYER_NORM_EPS=1e-12`, `CONTEXTUAL_VOCAB_SIZE=30522` |
| `graph/` | `GraphModel` | `GRAPH_DIMENSION=1024`, `MAX_CONTEXT_NEIGHBORS`, `GRAPH_MAX_TOKENS=512`, `GRAPH_LATENCY_BUDGET_MS=5` |
| `kepler/` | `KeplerModel` (with `transe.rs` for entity-relation arithmetic) | `KEPLER_DIMENSION=768`, `KEPLER_MAX_TOKENS=512`, `KEPLER_LATENCY_BUDGET_MS=5`, `KEPLER_MODEL_NAME` |
| `late_interaction/` | `LateInteractionModel`, `TokenEmbeddings` | `LATE_INTERACTION_DIMENSION=128`, `LATE_INTERACTION_MAX_TOKENS=512`, `LATE_INTERACTION_LATENCY_BUDGET_MS=8` |
| `rgcn/` | `RelationalGCN` | `INPUT_DIM`, `HIDDEN_DIM`, `RGCN_OUTPUT_DIM`, `NUM_BASES`, `NUM_RELATIONS`, `RELATION_NAMES`, `RGCN_WEIGHTS_PATH`, `DEFAULT_CONFIG_PATH` |
| `semantic/` | `SemanticModel` | `SEMANTIC_DIMENSION=1024`, `QUERY_PREFIX="query: "`, `PASSAGE_PREFIX="passage: "`, `SEMANTIC_MAX_TOKENS=512` |
| `sparse/` | `SparseModel`, `SparseVector` | `SPARSE_NATIVE_DIMENSION=30522`, `SPARSE_PROJECTED_DIMENSION=1536`, `SPARSE_EXPECTED_SPARSITY`, `SPARSE_MAX_TOKENS=512`, `SPARSE_LATENCY_BUDGET_MS=3` |
| `weight_projection/` | `LearnedWeightProjection` | `NUM_EMBEDDERS=14`, `HIDDEN_DIM_1`, `HIDDEN_DIM_2`, `OUTPUT_DIM`, `MAX_WEIGHTED_AGREEMENT`, `WEIGHTED_AGREEMENT_THRESHOLD`, `DEFAULT_CATEGORY_WEIGHTS`, `DEFAULT_WEIGHTS_PATH` |
| `shared.rs` | shared utilities used by multiple BERT-family models |  |

`DefaultModelFactory` (`models/factory/default_factory.rs`) maps `ModelId` → on-disk subdirectory (`get_model_path`, lines 111-132):

| `ModelId` | Subdirectory under `models_dir` |
| --- | --- |
| Semantic | `semantic` |
| Causal | `causal` |
| Sparse | `sparse` |
| Code | `code-1536` |
| Graph | `graph` (but production constructor reads `semantic` to share E1 weights) |
| Contextual | `contextual` |
| Entity (legacy) | `entity` |
| Kepler (E11 production) | `kepler` |
| LateInteraction | `late-interaction` |
| Splade | `splade-v3` |
| BgeM3Dense | `bge-m3-dense` |
| TemporalRecent / TemporalPeriodic / TemporalPositional / Hdc | none (custom) |

Construction order in `ProductionMultiArrayProvider::new` (`provider/multi_array.rs:771-952`):
1. Build `DefaultModelFactory` with `models_dir` + `gpu_config`.
2. Use `SingleModelConfig::cuda_fp16()` for all pretrained models.
3. Construct E1, E2, E3, E4 via factory; E5 directly (`CausalModel::new(&models_dir.join("causal"), ...)` — needed for `embed_dual_with_hint`); E6, E7 via factory; E8 directly (`GraphModel::new(&models_dir.join("semantic"), ...)` — shares e5-large-v2 with E1); E9 via factory; E10 directly (`ContextualModel::new(&models_dir.join("contextual"), ...)`); E11 = `factory.create_model(ModelId::Kepler, ...)`; E12, E13 via factory.
4. **Eagerly call `model.load().await?`** on every model in order (E1→E13).
5. After loading the base causal model, FAIL FAST if `models/causal/trained/{lora,projection}.safetensors` is missing — the search gate depends on those weights (`multi_array.rs:813-816`).
6. E14 is conditional: probe `models/bge-m3-dense/{model.safetensors,tokenizer.json,config.json}`. If all three exist, load the full BGE-M3 dense head; otherwise install `E14ZeroAdapter` and emit `tracing::warn!`.
7. Wrap loaded models in adapters and store in struct fields.

### Lazy / on-demand path: `ModelRegistry`

`crates/context-graph-embeddings/src/models/registry/`:
- Lazy loading: models loaded on first `get_model(ModelId)` call (`registry/operations.rs`).
- Per-model load locks (serialize concurrent requests for the same model).
- Memory budget tracking via `MemoryTracker` — refuses to load when total VRAM would exceed `ModelRegistryConfig::memory_budget_bytes` (default 32 GB; `RegistryConfig::rtx_5090()` = 32 GB, `rtx_4090()` = 24 GB, `testing(n)` lets tests override; `config.rs:50-86`).
- `max_concurrent_loads` defaults to 4 (`config.rs:34-58`).
- Registry stats (`registry/stats.rs`): `RegistryStats { loaded_count, total_memory_bytes, ... }`.
- Used primarily by tests and `BatchProcessor`; production runtime path uses the warm singleton.

### Warm singleton

`crates/context-graph-embeddings/src/global_provider.rs`:
- `static GLOBAL_WARM_PROVIDER: OnceLock<Arc<RwLock<ProviderState>>>` (line 52).
- `ProviderState { provider: Option<Arc<dyn MultiArrayEmbeddingProvider>>, concrete_provider: Option<Arc<ProductionMultiArrayProvider>>, init_error: Option<String> }` (lines 60-69).
- `initialize_global_warm_provider()` (`cuda_impl::initialize_global_warm_provider_impl`, lines 107-186) is called once at MCP/CLI startup. Subsequent calls are no-ops on success; on failure the error is cached and never retried.
- Resolves models directory via `CONTEXT_GRAPH_MODELS_PATH` env var, else `./models` (lines 87-98).
- `get_warm_provider()` (lines 242-269) returns the trait object; `get_warm_causal_model()` / `get_warm_graph_model()` (lines 331-393) return concrete `Arc<CausalModel>` / `Arc<GraphModel>` for the discovery services (CausalDiscoveryService, GraphDiscoveryService).
- `is_warm_initialized()` and `warm_status_message()` provide non-blocking probes; status messages are `"Not initialized" | "Initialization in progress" | "Initialization failed: <err>" | "Ready (14 models warm)"`.

The dedicated warm subsystem under `src/warm/` (loader, memory_pool, validation, diagnostics, health, inference, integration, cuda_alloc, registry) provides the lower-level allocator/health-check machinery — see [11_cuda_acceleration.md](11_cuda_acceleration.md) for the VRAM allocation lifecycle.

## 10. Warm-Loading Subsystem and VRAM Budgeting

### `warm/config.rs` — `WarmConfig`

| Field | Default | Env override |
| --- | --- | --- |
| `vram_budget_bytes` | 24 GiB (`24 * GB`) | `WARM_VRAM_BUDGET_BYTES` |
| `vram_headroom_bytes` | 8 GiB | `WARM_VRAM_HEADROOM_BYTES` |
| `model_weights_path` | `./models` | `WARM_MODEL_WEIGHTS_PATH` |
| `diagnostic_dump_path` | `/var/log/context-graph` | `WARM_DIAGNOSTIC_DUMP_PATH` |
| `cuda_device_id` | 0 (first of `CUDA_VISIBLE_DEVICES`) | `CUDA_VISIBLE_DEVICES` |
| `enable_test_inference` | true | `WARM_ENABLE_TEST_INFERENCE` |
| `max_load_time_per_model_ms` | 30,000 | `WARM_MAX_LOAD_TIME_MS` |
| `quantization` | `Fp16` | (none) |

`QuantizationMode { Fp32 | Fp16 | Fp8 }` with memory multipliers 1.0 / 0.5 / 0.25 (`config.rs:48-67`). `total_vram_required = budget + headroom = 32 GiB` for the RTX 5090 default.

### `warm/cuda_alloc/` — non-evictable VRAM allocations

| Constant | Value | Source |
| --- | --- | --- |
| `REQUIRED_COMPUTE_MAJOR` | 12 | `cuda_alloc/constants.rs:9` |
| `REQUIRED_COMPUTE_MINOR` | 0 | `constants.rs:12` |
| `MINIMUM_VRAM_BYTES` | `31 * 1024^3` (≈31 GB) | `constants.rs:26` |
| `GB` | `1024^3` | `constants.rs:29` |
| `MAX_ALLOCATION_HISTORY` | 100 | `constants.rs:32` |
| `FAKE_ALLOCATION_BASE_PATTERN` | `0x7f80_0000_0000` | `constants.rs:43` (detects mock allocations → exit 109) |
| `SIN_WAVE_ENERGY_THRESHOLD` | 0.80 | `constants.rs:54` (FFT-based fake-output detector) |
| `GOLDEN_SIMILARITY_THRESHOLD` | 0.99 | `constants.rs:59` (golden-output cosine match) |

Why `cudaMalloc` (not `cudaMallocManaged`/UVM): UVM allocations can be silently migrated to system RAM under memory pressure, causing PCIe-bound latency spikes. Model weights MUST stay resident (`cuda_alloc/mod.rs:9-23`).

`WarmCudaAllocator` (`allocator.rs`):
- Owns `live_tensors: Vec<(u64, Box<dyn Any + Send>)>` — live CUDA tensors keyed by allocation ID; dropping a tensor triggers candle's destructor which calls `cudaFree`.
- `allocate_protected(bytes)` issues `cudaMalloc`, stores the resulting tensor in `live_tensors`, increments `next_alloc_id`, and returns a `VramAllocation { ptr, size_bytes, device_id, is_protected: true }`.
- `free_protected(&allocation)` removes the tensor from `live_tensors` (drop → `cudaFree`), updates `total_allocated_bytes` and `allocation_history`.
- Allocator drop cleans all remaining tensors automatically.

`VramAllocation` (`allocation.rs`):
- Plain `Send + Sync` data struct (no Drop, no Clone) — only `WarmCudaAllocator` knows how to free.
- `is_valid()` returns `ptr != 0`.
- `size_mb() / size_gb()` helpers.

**Lesson (load-bearing):** `Clone` was deliberately removed from `VramAllocation` to prevent double-free when the allocator calls `cudaFree` while another handle still references the same pointer.

### `warm/loader/constants.rs` — model size budget

```text
MODEL_SIZES (FP16, approximate):
  E1_Semantic            600 MB     E2_TemporalRecent      400 MB
  E3_TemporalPeriodic    400 MB     E4_TemporalPositional  350 MB
  E5_Causal              500 MB     E6_Sparse              450 MB
  E7_Code                700 MB     E8_Graph               550 MB
  E9_HDC                 300 MB    E10_Contextual          800 MB
  E11_Entity             450 MB    E12_LateInteraction     600 MB
  E13_Splade             450 MB    E11_Kepler              350 MB
  E14_BgeM3Dense       1,200 MB
```

Total ≈ 7.6 GB (well under the 24 GB warm budget; ~6.4 GB without legacy E11_Entity). `DEFAULT_EMBEDDING_DIMENSION = 768`.

### Other warm modules

| Module | Purpose |
| --- | --- |
| `warm/state.rs` | `WarmModelState { Unloaded, Loading, Loaded, Failed(reason) }` |
| `warm/handle.rs` | `ModelHandle` (token returned to callers; tracks owning allocation) |
| `warm/registry/` | `WarmModelRegistry`, `EMBEDDING_MODEL_IDS`, `TOTAL_MODEL_COUNT` |
| `warm/memory_pool/` | `WarmMemoryPools { ModelMemoryPool, WorkingMemoryPool, ModelAllocation }` — model weight pool (non-evictable) vs working/activation pool (evictable) |
| `warm/validation/` | `WarmValidator`, `TestInferenceConfig`, `TestInput`, `ValidationResult` (golden-vector regression on each load) |
| `warm/loader/{engine,operations,preflight,summary,types/*}` | Main `WarmLoader::load_all()` orchestrator; `load_weights`, `verify_checksum`, `LoadingSummary`, `WarmLoadResult` |
| `warm/diagnostics/` | `WarmDiagnostics`, `WarmDiagnosticReport`, `GpuDiagnostics`, `MemoryDiagnostics`, `ModelDiagnostic`, `SystemInfo`, `ErrorDiagnostic` (JSON written to `diagnostic_dump_path` on fatal startup error) |
| `warm/inference/` | `InferenceEngine`, `cosine_similarity`, `detect_sin_wave_pattern`, `validate_inference_output_ap007` |
| `warm/health/` | `WarmHealthChecker`, `WarmHealthCheck`, `WarmHealthStatus` |
| `warm/integration/` | `WarmEmbeddingPipeline` (test-inference + warm-loaded model handles unified) |
| `warm/error.rs` | `WarmError` with `exit_code()` returning 101-110 for fatal startup errors |

## 11. BatchProcessor and dynamic batching

`src/batch/processor/{core,worker,submit,config,stats,mod}.rs`:
- One `BatchQueue` per `ModelId` (initialised for **all 15 variants** including legacy `Entity`).
- `mpsc::channel(request_buffer_size)` feeds a single worker task that:
  - polls per-model queues every `poll_interval_ms` (default 10 ms)
  - flushes when `batch.len() >= max_batch_size` or `oldest_age >= max_wait`
  - bounded concurrency via `Semaphore::new(max_concurrent_batches)` (default 4)
- `BatchProcessorConfig::default()`: `poll_interval_ms=10`, `max_concurrent_batches=4`, `request_buffer_size=1000` (`config.rs:29-37`).
- Stats (`BatchProcessorStats`): requests submitted/completed/failed/timed-out, batches flushed by trigger, queue depths.
- Shutdown drains in-flight batches before joining the worker.

Batch types (`src/batch/types/`): `BatchRequest`, `BatchQueue`, `Batch`, `BatchStats`.

## 12. Cache layer

`src/cache/mod.rs`:
- Backed by `moka::sync::Cache<CacheKey, Arc<MultiArrayEmbedding>>` (line 257).
- `CacheKey(u64)` = `xxhash_rust::xxh64(content.as_bytes(), 0)` (line 52).
- Tracked via atomic `CacheStats { hits, misses, insertions, estimated_evictions }`.
- Eviction: weight-based via `moka` weigher — `max_capacity = config.max_bytes` and weigher = `estimate_embedding_memory()` (lines 199-220). Each entry weighs ~`13 dims * 4 bytes + per-embedder overhead`.
- TTL configurable via `config.ttl_seconds` (lines 305-310); no fallback on cache failure (errors propagate via `EmbeddingResult`).
- `estimate_complete_embedding_memory()` returns `TOTAL_DIMENSION * 4 + MODEL_COUNT * 64 + 128` (lines 225-235), i.e. ~52 KB per fully-filled entry. Tests assert 30-100 KB range.
- `EmbeddingCache::disabled()` returns a no-op cache for tests (`get`→None, `insert`→noop).

## 13. Quantization (`src/quantization/`)

`QuantizationMethod::for_model_id(ModelId)` is the routing table (no fallbacks; `quantization/types.rs:58-87`):

| Method | Compression | Max recall loss | Embedders |
| --- | --- | --- | --- |
| `PQ8` | 32x | <5% (0.05) | E1, E5, E7, E10, E11 (Kepler), E14 |
| `Float8E4M3` | 4x | <0.3% (0.003) | E2, E3, E4, E8, legacy E11 (Entity) |
| `Binary` | 32x | 5–10% (0.10) | E9 |
| `SparseNative` | native (depends on sparsity) | 0% | E6, E13 |
| `TokenPruning` | ~2x | <2% (0.02) | E12 |

Implementations:
- `binary.rs` — sign-bit pack for E9 (`BinaryEncoder`). Threshold encoded in `QuantizationMetadata::Binary { threshold }`.
- `float8.rs` — E4M3 scalar quantize/dequantize for temporal/graph slots (`Float8E4M3Encoder`). Stores `{ scale, bias }` in `QuantizationMetadata::Float8`. Range ~1.5e-5 to 448 (positive).
- `pq8/` — Product Quantization with `NUM_SUBVECTORS=8` × `NUM_CENTROIDS=256` per subvector. The codebook (`PQ8Codebook { embedding_dim, num_subvectors, num_centroids, centroids: Vec<Vec<Vec<f32>>>, codebook_id }`) stores actual per-subvector centroids from training data — hardcoding `[-1, 1]` corrupts dequantization for any model whose normalized output is asymmetric (load-bearing lesson). Submodules: `encoder.rs`, `training.rs` (k-means via `KMeansConfig`), `persistence.rs` (binary `CODEBOOK_MAGIC` + `CODEBOOK_VERSION`), `types.rs`.
- `router/` — `QuantizationRouter` dispatches `(ModelId, &[f32]) -> QuantizedEmbedding` and reverses on read. Pre-creates PQ-8 encoders for `{1024 → E1, 768 → E5/E10/Kepler, 1536 → E7}` (`router/mod.rs:67-72`). E14 BGE-M3 (1024D) shares the E1 PQ-8 encoder.

`QuantizedEmbedding` shape: `{ method, original_dim, data: Vec<u8>, metadata: QuantizationMetadata }`.

`QuantizationMetadata` variants:
- `PQ8 { codebook_id: u32, num_subvectors: u8 }`
- `Float8 { scale: f32, bias: f32 }`
- `Binary { threshold: f32 }`
- `Sparse { vocab_size: usize, nnz: usize }`
- `TokenPruning { original_tokens: usize, kept_tokens: usize, threshold: f32 }`

**Asymmetric quantization (E5/E8/E10):** both vectors must be quantized independently — cloning cause→effect destroys the dual-vector signal entirely.

## 14. Token pruning (E12)

`src/pruning/token_pruner.rs` + `src/pruning/config.rs`:
- `TokenPruningQuantizer::prune(&[Vec<f32>], optional_attention_weights)` keeps the top fraction of tokens by importance.
- Importance scoring methods (`ImportanceScoringMethod`): `EmbeddingMagnitude`, `AttentionWeights`, `Combined`.
- Default config: `target_compression=0.5`, `min_tokens=10`, `scoring_method=EmbeddingMagnitude`.
- Output: `PrunedEmbeddings { embeddings, kept_indices, original_count }`.

## 15. Adapters (cross-crate trait bridges)

`src/adapters/code_provider.rs`: `E7CodeEmbeddingProvider` wraps `CodeModel` so it implements the `CodeEmbeddingProvider` trait defined in `context-graph-core`. Used by code-search ingestion paths.

## 16. Storage types (`src/storage/`)

`src/storage/types/` re-exports:
- `StoredQuantizedFingerprint` — complete fingerprint with quantized embeddings (~17 KB on disk vs ~63 KB unquantized in-memory `TeleologicalFingerprint`).
- `IndexEntry` — entry in per-embedder HNSW index (dequantized for search).
- `EmbedderQueryResult` — single-embedder result.
- `MultiSpaceQueryResult` — RRF-fused multi-space result.
- Constants: `NUM_EMBEDDERS=14`, `RRF_K`, `STORAGE_VERSION`, `EXPECTED_QUANTIZED_SIZE_BYTES`, `MAX_QUANTIZED_SIZE_BYTES`, `MIN_QUANTIZED_SIZE_BYTES`.

`src/storage/multi_space.rs`: `MultiSpaceSearchEngine` driving Stage-3 search; see [06_search_and_fusion.md](06_search_and_fusion.md) for fusion semantics.

## 17. Training subsystem (`src/training/`)

E5 fine-tuning lives here; full details in [16_training_subsystem.md](16_training_subsystem.md).

| File | Contents |
| --- | --- |
| `lora.rs` | `LoraConfig { rank=16, alpha=16.0, dropout=0.1, hidden_size=768, num_layers=12, apply_query=true, apply_value=true, apply_key=false }`; `LoraAdapter { a: Var, b: Var, scale, dropout }` — A initialized with quasi-random Kaiming, B initialized to zero (so adapter is identity at start). 589,824 trainable params total at default config |
| `pipeline.rs` | 3-stage progressive training pipeline: Stage 1 (warm-up, projection-only, 25 ep, max_difficulty=0.2); Stage 2 (LoRA activation, 20 ep); Stage 3 (directional weight ×0.6, 20 ep). Defaults: `batch_size=16`, `eval_every=5`, `early_stopping_patience=10`, `output_dir=models/causal/trained`, `seed=42` |
| `data.rs` | `CausalDataLoader`, `CausalTrainingPair`, `TrainingBatch` |
| `loss.rs` | `LossConfig`, `LossComponents` (contrastive + directional + multitask) |
| `optimizer.rs` | `AdamWConfig`, `ParamGroup` |
| `multitask.rs` | `MultiTaskConfig`, `MultiTaskHeads`, `DirectionLabel`, `MechanismLabel` |
| `evaluation.rs` | `Evaluator`, `EvaluationMetrics` |
| `distillation.rs` | Optional teacher-student distillation |
| `trainer.rs` | `CausalTrainer`, `TrainingConfig`, `EpochResult` |

**Training-time prefix override:** `pipeline.rs:195,212` deliberately switches both cause and effect to `"search_document: "` instead of the inference-time `CAUSE_INSTRUCTION` / `EFFECT_INSTRUCTION` because identical instruction prefixes were collapsing all training vectors to the same direction. Production inference still uses the per-direction prefixes via `embed_dual` / `embed_dual_with_hint`.

## 18. Learner-state E15-E21 (`src/learner/mod.rs`)

| Slot | `LearnerEmbedderSlot` | Modality | Native dim | Model name | Path | Scalar heads |
| --- | --- | --- | --- | --- | --- | --- |
| E15 | `E15AffectSpeech` | AffectSpeech | 1024 | Wav2Vec2-large-MSP-DIM | `affect-speech` | arousal, dominance, valence |
| E16 | `E16AffectFace` | AffectFace | 512 | OpenFace 3.0 + MTL emotion/AU/gaze | `affect-face` | valence, arousal, au_intensity |
| E17 | `E17AffectText` | AffectText | 384 | MiniLM-L6-v2 + EmoBank regressor | `affect-text` | valence, arousal |
| E18 | `E18Ppg` | Ppg | 512 | PaPaGei PPG foundation model | `ppg` | hrv_coherence |
| E19 | `E19Eda` | Eda | 64 | WESAD chest physiology stress head | `eda` | stress_floor |
| E20 | `E20Eeg` | Eeg | 768 | LaBraM EEG foundation model | `eeg` | plasticity_window |
| E21 | `E21EegArtifactRobust` | EegArtifactRobust | 768 | EEGPT artifact-robust EEG model | `eeg-robust` | plasticity_window |

Constants:
- `LEARNER_EMBEDDER_COUNT = 7` (`learner/mod.rs:25`)
- `UTL_PLANNED_TOTAL_EMBEDDERS = 21` (line 28)
- `UTL_CONTENT_EMBEDDERS_BEFORE_LEARNER_STATE = 14` (line 31)

Public surface (re-exported from `lib.rs:121-131`):
- `learner_embedder_specs() -> Vec<LearnerEmbedderSpec>`
- `learner_model_asset_requirements() -> Vec<LearnerModelAssetRequirement>`
- `preflight_learner_assets(models_root, calibration_root) -> LearnerAssetPreflightReport`
- `embed_learner_signal(input) -> EmbeddingResult<LearnerEmbeddingOutput>`
- `embed_calibration_sample(...) -> EmbeddingResult<...>`
- `state_vector_from_outputs(&[LearnerEmbeddingOutput]) -> LearnerStateVector`
- `synthetic_calibration_fixture(slot) -> LearnerCalibrationFixture`
- `CALIBRATION_DATASET_MANIFEST: &[CalibrationDatasetSpec]` — WESAD, MSP-Podcast, EmoBank, GoEmotions, DAiSEE, AffWild2, DEAP, MAHNOB-HCI, SEED/SEED-IV, Sleep-EDF (and more)

These slots back `LearnerStateVector` and goal-distance computations only; they never enter content fusion. See [20_learning_subsystem.md](20_learning_subsystem.md).

## 19. SemanticFingerprint Field Layout

`crates/context-graph-core/src/types/fingerprint/semantic/`:

| Field | Representation | Notes |
| --- | --- | --- |
| `e1_semantic` | `Vec<f32>` (1024D) | dense |
| `e2_temporal_recent` / `e3_temporal_periodic` / `e4_temporal_positional` | `Vec<f32>` (512D each) | dense |
| `e5_causal_as_cause` / `e5_causal_as_effect` | `Vec<f32>` (768D each) | asymmetric, current production path |
| `e5_causal` | `Vec<f32>` | legacy combined slot, written empty by current providers (`multi_array.rs:1199`) |
| `e6_sparse` | `SparseVector { indices: Vec<u16>, values: Vec<f32> }` | up to 30522 vocab |
| `e7_code` | `Vec<f32>` (1536D) | dense |
| `e8_graph_as_source` / `e8_graph_as_target` | `Vec<f32>` (1024D each) | asymmetric |
| `e8_graph` | `Vec<f32>` | legacy, empty (`multi_array.rs:1204`) |
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

## 20. Tokenizer / instruction-prefix gotchas

- E1 (e5-large-v2) uses `query: ` for queries and `passage: ` for documents (`PASSAGE_PREFIX`/`QUERY_PREFIX` in `models/pretrained/semantic/constants.rs`). Train-time prefix MUST match inference.
- E5 (nomic-embed) uses `search_query: Identify the cause/effect of: ` at inference. The training pipeline (`training/pipeline.rs:195,212`) deliberately switches to `search_document: ` for both cause and effect during training because identical instruction prefixes were collapsing all training vectors to the same direction.
- E2 instruction (production): `"timestamp:<unix_seconds>"` — but the sinusoidal path ignores the instruction and reads `created_at` directly (the instruction is informational only).
- E4 instruction: `"sequence:<n>"` or `"epoch:<n>"`.
- E10 prefixes: `"query: "` for intents, `"passage: "` for context (`contextual/constants.rs:55-59`).
- E14 BGE-M3 uses XLM-RoBERTa SentencePiece tokenizer (`tokenizer_family() = XlmRobertaSentencePiece`).
- E11 KEPLER uses RoBERTa BPE (`tokenizer_family() = RobertaBpe`).
- E7 Qodo-Embed uses Qwen2 BPE internally but is reported as `BertWordpiece` in `TokenizerFamily` for enum simplicity (`core.rs:175`); the model loads its own tokenizer file.

## 21. Lesson Highlights (load-bearing)

- **CodeStore cosine returns raw [-1,1]** — `code/store.rs cosine_similarity_with_norm` is NOT normalized to [0,1] like all other paths. Always verify new cosine paths apply `(raw+1)/2`.
- **15 HNSW indexes, NOT 11/12/14** — see Section 3.
- **bincode + skip_serializing_if = BROKEN** — use JSON for all provenance/SourceMetadata.
- **RwLockReadGuard (parking_lot) is !Send** — must drop before `.await` in tokio::spawn.
- **with_custom_weights() does NOT auto-set MultiSpace strategy** — caller must opt in.
- **RocksDB requires ALL on-disk CFs to be opened** — delete old DBs when removing CFs.
- **Cell\<bool\> for interior mutability** for `&self` methods that need to toggle flags (LoRA training mode in `training/lora.rs`).
- **LoRA dropout increases VRAM** — batch_size 16→8 when dropout tensors tracked in grad graph.
- **Dropout regularization compresses absolute scores** — always recalibrate thresholds after.
- **floor_char_boundary/ceil_char_boundary** for safe string slicing on multi-byte UTF-8.
- **Train/inference prefix MUST match** for nomic-embed (`search_document:` for training, `search_query: Identify the cause/effect:` for inference). See Section 20.
- **hard_negative_scale ≥ 2.0 with temperature=0.05 causes catastrophic collapse** — use 1.0.
- **Best-stage selection should use `score_spread`, not `directional_accuracy`.**
- **Per-stage LoRA checkpoints needed** so best stage's LoRA pairs with its projection.
- **Quantize BOTH asymmetric vectors** — cloning cause→effect destroys the dual-vector signal entirely.
- **PQ-8 must store actual min/max** — hardcoding `[-1, 1]` corrupts dequantization when data has different range.
- **ModelId has 14 variants in production, 15 with legacy `Entity`, NUM_EMBEDDERS=14** — never confuse `ModelId::all().len()` (15) with `NUM_EMBEDDERS` (14) for array sizing.
- **ModelId::Entity = 384D legacy MiniLM, ModelId::Kepler = 768D production E11** — don't change `Entity` to 768D, it breaks `EntityModel` tests.
- **E2 uses sinusoidal PE (FIXED)** — old decay encoding produced identical vectors. Now uses `compute_decay_embedding(timestamp, None, rates)`. Rebuild recomputes E2 from `fp.created_at`.
- **E2/E3/E4 HNSW indexes NOW populated** when `TEMPORAL_EMBEDDERS_ENABLED=true` and weight > 0.0.
- **E7 raw cosine enters fusion unnormalized** — `code/mod.rs` returns [-1, 1] while all other embedders return [0, 1]. Apply `(raw+1)/2` before fusion.
- **suppress_degenerate_weights >0.0 filter drops valid zeros** — E5=0.0 for non-causal is legitimate; filtering it out corrupts weight fusion.
- **Adapter/wrapper classes must override ALL trait methods they intercept** — `LazyMultiArrayProvider` overrode `embed_all` but not `embed_all_with_metadata`, causing silent metadata loss through the default impl fallback.
- **VramAllocation: no Clone, no Drop** — only `WarmCudaAllocator::free_protected` knows the back-reference; `live_tensors` storage in the allocator is the single source of truth (`warm/cuda_alloc/allocator.rs:73`, `allocation.rs:24-37`).

## 22. Cross-references

- HNSW search and fusion: [06_search_and_fusion.md](06_search_and_fusion.md).
- Storage of fingerprint + index + quantized payloads: [09_storage_layer.md](09_storage_layer.md).
- CUDA acceleration (kernels, allocator, candle device): [11_cuda_acceleration.md](11_cuda_acceleration.md).
- LoRA + projection training that produces the E5 trained weights: [16_training_subsystem.md](16_training_subsystem.md).
- Constellation aggregation over fingerprints: [17_constellation_subsystem.md](17_constellation_subsystem.md).
- Learner-state subsystem (E15-E21): [20_learning_subsystem.md](20_learning_subsystem.md).
