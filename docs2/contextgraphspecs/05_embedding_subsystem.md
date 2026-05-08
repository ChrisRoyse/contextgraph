# Embedding Subsystem

**Source files covered:**

- `crates/context-graph-embeddings/src/lib.rs` (mandatory `candle` feature gate, public re-exports)
- `crates/context-graph-embeddings/src/types/model_id/{core,conversions,display,repository,tokenizer}.rs` (`ModelId` enum and methods)
- `crates/context-graph-embeddings/src/types/dimensions/{constants,aggregates}.rs`
- `crates/context-graph-embeddings/src/traits/{embedding_model,model_factory,mod}.rs`
- `crates/context-graph-embeddings/src/provider/{mod,multi_array,causal_hint,diagnostic_test}.rs`
- `crates/context-graph-embeddings/src/global_provider.rs` (warm singleton)
- `crates/context-graph-embeddings/src/models/pretrained/` (12 model directories: `bge_m3_dense`, `causal`, `code`, `contextual`, `graph`, `kepler`, `late_interaction`, `rgcn`, `semantic`, `sparse`, `weight_projection`, `shared.rs`)
- `crates/context-graph-embeddings/src/models/custom/` (4 model directories: `hdc`, `temporal_periodic`, `temporal_positional`, `temporal_recent`)
- `crates/context-graph-embeddings/src/models/factory/{default_factory,trait_impl}.rs`
- `crates/context-graph-embeddings/src/models/registry/{mod,core,config,loader,operations,stats}.rs`
- `crates/context-graph-embeddings/src/quantization/{mod,types,binary,float8,pq8/*,router/*}.rs`
- `crates/context-graph-embeddings/src/pruning/{config,token_pruner,mod}.rs`
- `crates/context-graph-embeddings/src/cache/mod.rs`
- `crates/context-graph-embeddings/src/batch/{processor/*,types/*}.rs`
- `crates/context-graph-embeddings/src/learner/mod.rs` (E15-E21 namespace)
- `crates/context-graph-embeddings/src/warm/{config,state,handle,error,registry,memory_pool/*,validation,cuda_alloc/*,loader/*,diagnostics,inference/*,integration,health/*}.rs`
- `crates/context-graph-embeddings/src/training/{lora,pipeline,trainer,loss,optimizer,multitask,evaluation,distillation,data,mod}.rs`
- `crates/context-graph-embeddings/src/bin/train_causal.rs` (training entry binary)
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs` (consumer-side dimensions)
- `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs` (E2 rebuild)

## 1. Purpose and feature gate

`context-graph-embeddings` is the GPU-resident inference layer that produces a per-content `MultiArrayEmbeddingOutput` for every memory ingested by the storage layer. The `candle` Cargo feature is **mandatory**: `lib.rs:45-51` issues a `compile_error!` whenever it is missing, because every embedder runs through Candle on CUDA. The `cuda` feature is an alias for `candle`. Default features are `["candle"]`.

The crate's public surface (`lib.rs:53-134`) exports `EmbeddingProvider`, `ProductionMultiArrayProvider`, `EmbeddingModel`, `ModelId`, `ModelRegistry`, the warm-provider helpers, the quantization router, the token-pruning structs, the cache types, and the learner-namespace primitives.

The persisted shape that consumes the provider's output is `SemanticFingerprint` from `context-graph-core` (see [04_database_schema.md](04_database_schema.md) and [09_storage_layer.md](09_storage_layer.md) for storage layout). `LearnerFingerprint` is a separate type that holds learner-state E15-E21 signal vectors and never enters content fusion.

## 2. Embedder roster (E1-E14, NUM_EMBEDDERS = 14)

`ModelId` is the canonical 15-variant enum (`types/model_id/core.rs:38-77`), but only 14 variants are production: `ModelId::production()` returns 14 (`core.rs:234-251`) and `NUM_EMBEDDERS` (in `context-graph-core::types::fingerprint::semantic::constants.rs:81`) equals 14. `ModelId::all()` returns 15 (`core.rs:200-218`) because the legacy 384D MiniLM `Entity` slot is preserved alongside the production 768D `Kepler` slot. **Always use `NUM_EMBEDDERS` (14) for array sizing — never `ModelId::all().len()` (15).** `MODEL_COUNT` in `types/dimensions/aggregates.rs` is 15 because the dimensions table covers both Entity and Kepler.

| Slot | `ModelId` variant | Backing model | Native dim | Stored / projected dim | Max tokens | Type | Asymmetric? | Tokenizer family | Latency budget (ms) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| E1 | `Semantic` | intfloat/e5-large-v2 | 1024 | 1024 | 512 | dense | no | BertWordpiece | 5 |
| E2 | `TemporalRecent` | sinusoidal PE of `created_at` (no weights) | 512 | 512 | unlimited | dense | no | None | 2 |
| E3 | `TemporalPeriodic` | Fourier basis (no weights) | 512 | 512 | unlimited | dense | no | None | 2 |
| E4 | `TemporalPositional` | sinusoidal PE on session sequence (no weights) | 512 | 512 | unlimited | dense | no | None | 2 |
| E5 | `Causal` | nomic-ai/nomic-embed-text-v1.5 | 768 | 768 | 512 | dense | yes (cause / effect) | BertWordpiece | 8 |
| E6 | `Sparse` | naver/splade-cocondenser-ensembledistil | 30522 vocab | 30522 sparse (1536 if ever projected) | 512 | sparse | no | BertWordpiece | 3 |
| E7 | `Code` | Qodo/Qodo-Embed-1-1.5B | 1536 | 1536 | 32768 | dense | no | BertWordpiece (loads its own Qwen2 BPE) | 10 |
| E8 | `Graph` | intfloat/e5-large-v2 (shares E1 weights) | 1024 | 1024 | 512 | dense | yes (source / target) | BertWordpiece | 5 |
| E9 | `Hdc` | custom hyperdimensional (no weights) | 10000 binary | 1024 projected | unlimited | dense | no | None | 1 |
| E10 | `Contextual` (serde alias `Multimodal`) | intfloat/e5-base-v2 | 768 | 768 | 512 | dense | yes (paraphrase=doc / context=query) | BertWordpiece | 15 |
| E11 (legacy) | `Entity` | sentence-transformers/all-MiniLM-L6-v2 | 384 | 384 | 512 | dense | no | BertWordpiece | 2 |
| E11 (production) | `Kepler` | THU-KEG/KEPLER-Wiki5M-KE (RoBERTa-base + TransE) | 768 | 768 | 512 | dense | no | RobertaBpe | 5 |
| E12 | `LateInteraction` | colbert-ir/colbertv2.0 | 128 / token | 128 / token | 512 | multivector | no | BertWordpiece | 8 |
| E13 | `Splade` | prithivida/Splade_PP_en_v1 (SPLADE v3) | 30522 vocab | 30522 sparse (1536 if ever projected) | 512 | sparse | no | BertWordpiece | 3 |
| E14 | `BgeM3Dense` | BAAI/bge-m3 (XLM-RoBERTa-Large dense head) | 1024 | 1024 | 8192 | dense | no | XlmRobertaSentencePiece | 50 |

Source authority: `ModelId::dimension()` (`core.rs:85-103`), `ModelId::projected_dimension()` (`core.rs:114-123`), `ModelId::max_tokens()` (`core.rs:149-163`), `ModelId::tokenizer_family()` (`core.rs:170-188`), `ModelId::latency_budget_ms()` (`core.rs:267-283`). The ColBERT entry returns `dimension() = 128` because that is the per-token dimension; total output is `[num_tokens × 128]`.

### Native dimensions cross-checked against `semantic/constants.rs`

| Constant | Value |
| --- | --- |
| `E1_DIM` | 1024 |
| `E2_DIM`, `E3_DIM`, `E4_DIM` | 512 each |
| `E5_DIM` | 768 |
| `E6_SPARSE_VOCAB` | 30522 |
| `E7_DIM` | 1536 |
| `E8_DIM` | 1024 |
| `E9_DIM` | 1024 (projected from 10000-bit) |
| `E10_DIM` | 768 |
| `E11_DIM` | 768 (KEPLER; legacy `Entity` is 384D in `ModelId::dimension()`) |
| `E12_TOKEN_DIM` | 128 |
| `E13_SPLADE_VOCAB` | 30522 |
| `E14_DIM` | 1024 |
| `NUM_EMBEDDERS` | 14 |

`TOTAL_DENSE_DIMS` (`semantic/constants.rs:95-105`) = 1024 + 512 + 512 + 512 + 768 + 1536 + 1024 + 1024 + 768 + 768 + 1024 = 9472 — this is the *single-vector* sum (E5/E8/E10 contribute one vector each). Adding the second asymmetric vectors gives 9472 + 768 + 1024 + 768 = 12032 dense dims; HNSW indexes those second vectors separately.

## 3. Asymmetric embedder mechanics (E5, E8, E10)

Three embedders return a pair of vectors per input. The provider stores both in the fingerprint and indexes them in separate HNSW indexes (see §6).

### E5 Causal (`models/pretrained/causal/`)

- 768D from nomic-embed-text-v1.5. `NomicConfig` defaults (`config.rs:36-87`): `vocab_size=30528`, `hidden_size=768`, `num_hidden_layers=12`, `num_attention_heads=12`, `intermediate_size=3072`, `max_position_embeddings=8192`, `layer_norm_eps=1e-12`, `rotary_emb_base=1000.0`, `rotary_emb_fraction=1.0`, `rotary_emb_interleaved=false`, no biases on QKV/MLP.
- Inference instruction prefixes (`config.rs:21-27`): `CAUSE_INSTRUCTION = "search_query: Identify the cause in: "`, `EFFECT_INSTRUCTION = "search_query: Identify the effect of: "`.
- Inference flow: encoder → `TrainableProjection::project_cause` and `project_effect` → optional LoRA → L2 normalize. The provider FAILS FAST if `models/causal/trained/{lora,projection}.safetensors` is missing (`provider/multi_array.rs:813-816`); there is no silent fallback to base weights.
- `embed_dual_with_hint(text, CausalHint)` accepts an LLM-derived hint. When `hint.is_useful()` returns true, the per-vector bias multipliers (default 1.3 / 0.8 for `Cause`; swapped for `Effect`; 1.0 / 1.0 for `Neutral`) scale the cause and effect vectors before normalization. LLM-supplied marker spans use `embed_dual_guided`.
- The cause vector lives in `e5_causal_as_cause`; the effect vector lives in `e5_causal_as_effect`. The legacy combined `e5_causal` slot is written empty by current providers (`multi_array.rs:1199`).

### E8 Graph (`models/pretrained/graph/`)

- 1024D shared with E1 — the production constructor in `provider/multi_array.rs:791` reads from `models_dir.join("semantic")` rather than `graph` to avoid duplicating the 600 MB e5-large-v2 weight file. `MAX_CONTEXT_NEIGHBORS = 5`, `GRAPH_DIMENSION = 1024`, `GRAPH_MAX_TOKENS = 512`, `GRAPH_LATENCY_BUDGET_MS = 5`.
- Asymmetry comes from the learned `W_source` / `W_target` projections in `graph/projections.rs` applied after the shared encoder pass.
- Source-vs-target direction multipliers are applied at fusion time (see [06_search_and_fusion.md](06_search_and_fusion.md)).
- Source goes into `e8_graph_as_source`; target into `e8_graph_as_target`. The legacy combined `e8_graph` slot is written empty (`multi_array.rs:1204`).

### E10 Contextual (`models/pretrained/contextual/`)

- 768D from intfloat/e5-base-v2 (`contextual/constants.rs`): `CONTEXTUAL_HIDDEN_SIZE=768`, `CONTEXTUAL_INTERMEDIATE_SIZE=3072`, `CONTEXTUAL_NUM_LAYERS=12`, `CONTEXTUAL_NUM_HEADS=12`, `CONTEXTUAL_LAYER_NORM_EPS=1e-12`, `CONTEXTUAL_VOCAB_SIZE=30522`, `CONTEXTUAL_MAX_TOKENS=512`, `CONTEXTUAL_LATENCY_BUDGET_MS=20`.
- Asymmetry is achieved entirely through prefixing — no separate projection matrices: `INTENT_PREFIX = "query: "` (used for the query/context-of-interest vector), `CONTEXT_PREFIX = "passage: "` (used for the document/paraphrase vector).
- Doc / passage encoding lands in `e10_multimodal_paraphrase`; query / context encoding lands in `e10_multimodal_as_context`. The mapping is "paraphrase = doc, context = query" — opposite of the variable name's surface reading.

### Cosine normalization invariant

Every fusion path normalizes raw cosine `[-1, 1]` to `[0, 1]` via `(raw + 1) / 2` so that orthogonal vectors score 0.5 and antipodes score 0.0. This is load-bearing: any new code path that emits a cosine score for fusion MUST apply that transform. `code/store.rs::cosine_similarity_with_norm` historically returned raw `[-1, 1]`, which corrupted fusion weights for E7. See [06_search_and_fusion.md](06_search_and_fusion.md) for the fusion math.

## 4. Custom (no-weights) embedders

| Slot | Module | Output |
| --- | --- | --- |
| E2 | `models/custom/temporal_recent/compute.rs` | 512D sinusoidal PE of `created_at.timestamp()` (production: `reference_time = None`). Test path uses original exponential decay (`reference_time = Some(...)`). |
| E3 | `models/custom/temporal_periodic/` | 512D Fourier basis (`DEFAULT_PERIODS`, `FEATURES_PER_PERIOD`, `HARMONICS_PER_PERIOD`); captures hour-of-day, day-of-week, month-of-year, etc. |
| E4 | `models/custom/temporal_positional/` | 512D mixed: first 256D session-signature, last 256D sinusoidal PE on `sequence:N` or `epoch:N` from the `EmbeddingMetadata` instruction. `DEFAULT_BASE` controls the geometric frequency progression. |
| E9 | `models/custom/hdc/` | 10000-bit hypervector via character n-gram binding (`DEFAULT_NGRAM_SIZE = 3`, `DEFAULT_SEED = 42`) projected to 1024D float (`+1.0` for set bits, `-1.0` for unset, then L2-normalized). Operations: bind (XOR), bundle (majority), permute (rotate), random hypervector with ~50% bits set. |

E2/E3/E4 honor the `TEMPORAL_EMBEDDERS_ENABLED` flag in `context-graph-core::weights::mod.rs`. When disabled, the provider emits `vec![0.0; 512]` and skips inference.

### E2 sinusoidal PE detail

`compute_decay_embedding(timestamp, None, decay_rates)` calls `compute_sinusoidal_recency`:

```text
half_dim = 256
base     = 86400.0   (one day in seconds)
for k in 0..256:
    exponent = 2k / 512
    divisor  = base ** exponent
    angle    = timestamp.timestamp() / divisor
    vector[2k]   = sin(angle) as f32
    vector[2k+1] = cos(angle) as f32
L2-normalize(vector)
```

Wavelengths span seconds (k=0 ≈ 6.28 s) through months (k=255). The previous decay-only encoding produced delta ≈ 0 for every memory because embedding happens immediately after `created_at`, which made every E2 vector nearly identical and broke the E2 HNSW index. `rebuild_indexes_from_store()` in `context-graph-storage/src/teleological/rocksdb_store/store.rs:686-770` recomputes E2 from `fp.created_at` on store open, replacing any persisted-but-degenerate E2 vectors and re-adding fingerprints to all HNSW indexes under `compaction_lock`.

## 5. E1 Matryoshka-128 head

E1 ships a second HNSW index that holds the first 128 dimensions of the L2-normalized E1 vector after re-normalization. There is no extra inference call — it is a slice of the existing 1024D E1 output. The 128D head is used for fast candidate generation; downstream stages re-rank with the full 1024D E1 vector. See [06_search_and_fusion.md](06_search_and_fusion.md).

## 6. HNSW index roster (15 indexes)

`NUM_EMBEDDERS = 14`, but the HNSW layer maintains 15 indexes because asymmetric embedders store dual vectors and E1 has the Matryoshka head:

| # | Index | Vector source | Dim |
| --- | --- | --- | --- |
| 1 | E1 | `e1_semantic` | 1024 |
| 2 | E1Matryoshka128 | `e1_semantic[0..128]` (truncate, then re-normalize) | 128 |
| 3 | E2 | `e2_temporal_recent` | 512 |
| 4 | E3 | `e3_temporal_periodic` | 512 |
| 5 | E4 | `e4_temporal_positional` | 512 |
| 6 | E5 (combined) | concat / averaged cause + effect | 768 |
| 7 | E5CausalCause | `e5_causal_as_cause` | 768 |
| 8 | E5CausalEffect | `e5_causal_as_effect` | 768 |
| 9 | E7 | `e7_code` | 1536 |
| 10 | E8 (combined) | concat / averaged source + target | 1024 |
| 11 | E9 | `e9_hdc` | 1024 |
| 12 | E10MultimodalParaphrase | `e10_multimodal_paraphrase` | 768 |
| 13 | E10MultimodalContext | `e10_multimodal_as_context` | 768 |
| 14 | E11 | `e11_entity` (KEPLER, 768D) | 768 |
| 15 | E14 | `e14_bge_m3_dense` | 1024 |

E6, E12, and E13 do **not** participate in HNSW. E6 and E13 use inverted indexes (`CF_E6_SPARSE_INVERTED`, `CF_E13_SPLADE_INVERTED`); E12 uses MaxSim against `CF_E12_LATE_INTERACTION`. See [04_database_schema.md](04_database_schema.md) for column-family layout.

## 7. Pretrained model directories

Each pretrained model lives under `crates/context-graph-embeddings/src/models/pretrained/<dir>/`. `pretrained/mod.rs` re-exports the model type, the dimension constants, and the prefix/instruction strings used at inference.

| Directory | Type | Key constants |
| --- | --- | --- |
| `bge_m3_dense/` | `BgeM3DenseModel` | `BGE_M3_DENSE_DIMENSION = 1024`, `BGE_M3_DENSE_MAX_TOKENS = 8192`, `BGE_M3_DENSE_LATENCY_BUDGET_MS = 50`, `XLM_R_BOS_TOKEN_ID`, `XLM_R_PAD_TOKEN_ID`, `XLM_R_POSITION_OFFSET`, `XLM_R_WEIGHT_PREFIX` |
| `causal/` | `CausalModel`, `TrainableProjection`, `NomicWeights`, `load_nomic_weights` | `CAUSAL_DIMENSION = 768`, `CAUSAL_MAX_TOKENS = 512`, `CAUSAL_LATENCY_BUDGET_MS = 8`, `CAUSE_INSTRUCTION`, `EFFECT_INSTRUCTION` |
| `code/` | `CodeModel` | `CODE_NATIVE_DIMENSION = 1536`, `CODE_PROJECTED_DIMENSION = 1536`, `CODE_MAX_TOKENS = 32768`, `CODE_LATENCY_BUDGET_MS = 10`, `CODE_MODEL_NAME = "Qodo/Qodo-Embed-1-1.5B"` |
| `contextual/` | `ContextualModel` | `CONTEXTUAL_DIMENSION = 768`, `CONTEXTUAL_MAX_TOKENS = 512`, `CONTEXTUAL_LATENCY_BUDGET_MS = 20`, `INTENT_PREFIX = "query: "`, `CONTEXT_PREFIX = "passage: "`, `CONTEXTUAL_HIDDEN_SIZE = 768`, `CONTEXTUAL_INTERMEDIATE_SIZE = 3072`, `CONTEXTUAL_NUM_HEADS = 12`, `CONTEXTUAL_NUM_LAYERS = 12`, `CONTEXTUAL_LAYER_NORM_EPS = 1e-12`, `CONTEXTUAL_VOCAB_SIZE = 30522`, `CONTEXTUAL_MODEL_NAME = "intfloat/e5-base-v2"` |
| `graph/` | `GraphModel` | `GRAPH_DIMENSION = 1024`, `GRAPH_MAX_TOKENS = 512`, `GRAPH_LATENCY_BUDGET_MS = 5`, `MAX_CONTEXT_NEIGHBORS = 5`, `GRAPH_MODEL_NAME` |
| `kepler/` | `KeplerModel` (with `transe.rs` for entity-relation arithmetic) | `KEPLER_DIMENSION = 768`, `KEPLER_MAX_TOKENS = 512`, `KEPLER_LATENCY_BUDGET_MS = 5`, `KEPLER_MODEL_NAME = "KEPLER (RoBERTa-base + TransE)"` |
| `late_interaction/` | `LateInteractionModel`, `TokenEmbeddings` | `LATE_INTERACTION_DIMENSION = 128`, `LATE_INTERACTION_MAX_TOKENS = 512`, `LATE_INTERACTION_LATENCY_BUDGET_MS = 8`, `LATE_INTERACTION_MODEL_NAME = "colbert-ir/colbertv2.0"` |
| `rgcn/` | `RelationalGCN` | `INPUT_DIM`, `HIDDEN_DIM`, `RGCN_OUTPUT_DIM`, `NUM_BASES`, `NUM_RELATIONS`, `RELATION_NAMES`, `RGCN_WEIGHTS_PATH`, `DEFAULT_CONFIG_PATH` |
| `semantic/` | `SemanticModel` | `SEMANTIC_DIMENSION = 1024`, `SEMANTIC_MAX_TOKENS = 512`, `SEMANTIC_LATENCY_BUDGET_MS = 5`, `QUERY_PREFIX = "query: "`, `PASSAGE_PREFIX = "passage: "` |
| `sparse/` | `SparseModel`, `SparseVector` | `SPARSE_NATIVE_DIMENSION = 30522`, `SPARSE_PROJECTED_DIMENSION = 1536`, `SPARSE_EXPECTED_SPARSITY`, `SPARSE_MAX_TOKENS = 512`, `SPARSE_LATENCY_BUDGET_MS = 10`, `SPARSE_MODEL_NAME = "naver/splade-cocondenser-ensembledistil"` |
| `weight_projection/` | `LearnedWeightProjection` | `NUM_EMBEDDERS = 14`, `HIDDEN_DIM_1`, `HIDDEN_DIM_2`, `OUTPUT_DIM`, `MAX_WEIGHTED_AGREEMENT`, `WEIGHTED_AGREEMENT_THRESHOLD`, `DEFAULT_CATEGORY_WEIGHTS`, `DEFAULT_WEIGHTS_PATH` |
| `shared.rs` | shared utilities used by multiple BERT-family models | — |

### Per-model on-disk subdirectories

`DefaultModelFactory::get_model_path` (`models/factory/default_factory.rs:111-132`) maps `ModelId` to the subdirectory under `models_dir`:

| `ModelId` | Subdirectory |
| --- | --- |
| `Semantic` | `semantic` |
| `Causal` | `causal` |
| `Sparse` | `sparse` |
| `Code` | `code-1536` |
| `Graph` | `graph` (production constructor reads `semantic` instead, sharing E1 weights) |
| `Contextual` | `contextual` |
| `Entity` | `entity` |
| `Kepler` | `kepler` |
| `LateInteraction` | `late-interaction` |
| `Splade` | `splade-v3` |
| `BgeM3Dense` | `bge-m3-dense` |
| `TemporalRecent`, `TemporalPeriodic`, `TemporalPositional`, `Hdc` | none (custom) |

### SafeTensors loading and SHA256 verification

`warm/loader/operations.rs::load_weights` reads the bytes, computes a real SHA256 (`sha2 = "0.10"`) over the file, then parses the SafeTensors structure (`safetensors = "0.4"`). The checksum is logged at `debug!` level (`operations.rs:451-463`); `verify_checksum` raises `WarmError::WeightChecksumMismatch` when an expected hash is supplied and does not match (`operations.rs:521`). Models are then handed to a `VarBuilder` via `unsafe { VarBuilder::from_mmaped_safetensors(...) }` so weights are memory-mapped from disk into device memory.

### Tokenizer crate version

All BERT-family / Qwen2 / RoBERTa / XLM-R tokenizers go through `tokenizers = "0.22"` (Cargo.toml:64-66). Version 0.22+ is required for Qwen2 `tokenizer.json` format compatibility used by Qodo-Embed (E7).

## 8. Multi-array provider

### Trait surface

`MultiArrayEmbeddingProvider` (in `context-graph-core::traits`) is what the storage layer calls. `ProductionMultiArrayProvider` (`provider/multi_array.rs`) implements it.

| Method | Purpose |
| --- | --- |
| `embed_all(content)` | All 14 production embedders run in parallel via `tokio::join!`; returns `MultiArrayEmbeddingOutput` containing the fingerprint plus per-embedder latency. |
| `embed_all_with_metadata(content, EmbeddingMetadata)` | Same as `embed_all` plus E2/E3 timestamp instruction, E4 sequence/epoch instruction, E5 causal hint. |
| `embed_batch_all(contents, &[EmbeddingMetadata])` | Spawns one tokio task per content, reuses metadata-aware code path. |
| `model_ids() -> [String; 14]` | Telemetry IDs in the production order. |
| `is_ready() / health_status()` | Per-embedder readiness probes. |

### Adapter wiring

`provider/multi_array.rs` defines six concrete adapters that feed each model into the unified provider:

| Adapter | Trait | Notes |
| --- | --- | --- |
| `DenseEmbedderAdapter` | `SingleEmbedder` | Holds `model_id` + `dimension`; offers `embed_with_instruction` so the provider can pass E2/E3 timestamps and E4 sequence numbers. |
| `SparseEmbedderAdapter` | `SparseEmbedder` | Calls `model.embed_sparse(...)` directly so SPLADE / SPLADE-v3 return the 30522-vocab `(indices, values)` tuple, not a 1536D dense projection. |
| `TokenEmbedderAdapter` | `TokenEmbedder` | Reshapes ColBERT's flat `[num_tokens × 128]` output into `Vec<Vec<f32>>`. |
| `CausalDualEmbedderAdapter` | concrete | Owns `Arc<CausalModel>`; exposes `embed_dual()` and `embed_dual_with_hint()`. |
| `GraphDualEmbedderAdapter` | concrete | Owns `Arc<GraphModel>`; exposes `embed_dual()`. |
| `ContextualDualEmbedderAdapter` | concrete | Owns `Arc<ContextualModel>`; exposes `embed_dual()` for paraphrase / context. |
| `E14ZeroAdapter` | `SingleEmbedder` | Installed when `./models/bge-m3-dense/` is missing `model.safetensors`, `tokenizer.json`, or `config.json`. Returns `Ok(Vec::new())` so the slot stays present, fusion treats E14 as inactive, and a `tracing::warn!` is emitted at startup naming the missing directory. |

**Load-bearing rule:** any wrapper around `MultiArrayEmbeddingProvider` MUST override **both** `embed_all` and `embed_all_with_metadata`. Overriding only the first causes the metadata-aware path to fall through to the default trait impl, which silently drops timestamps, sequence numbers, and causal hints.

### Construction order in `ProductionMultiArrayProvider::new`

`provider/multi_array.rs:771-952`:

1. Build `DefaultModelFactory` with `models_dir` + `gpu_config`.
2. Build `SingleModelConfig::cuda_fp16()` for every pretrained model.
3. Construct E1, E2, E3, E4 via factory.
4. Construct E5 directly (`CausalModel::new(&models_dir.join("causal"), ...)`) — needed for `embed_dual_with_hint`.
5. Construct E6 (Sparse) and E7 (Code) via factory.
6. Construct E8 directly (`GraphModel::new(&models_dir.join("semantic"), ...)`) — shares e5-large-v2 weights with E1.
7. Construct E9 (HDC) via factory.
8. Construct E10 directly (`ContextualModel::new(&models_dir.join("contextual"), ...)`).
9. Construct E11 = `factory.create_model(ModelId::Kepler, ...)`.
10. Construct E12 (LateInteraction) and E13 (Splade) via factory.
11. **Eagerly call `model.load().await?`** on every model in order (E1 → E13).
12. After loading the base causal model, FAIL FAST if `models/causal/trained/{lora,projection}.safetensors` is missing (`multi_array.rs:813-816`).
13. E14 is conditional: probe `models/bge-m3-dense/{model.safetensors,tokenizer.json,config.json}`. If all three exist, load BGE-M3 dense head; otherwise install `E14ZeroAdapter`.
14. Wrap loaded models in adapters and store in struct fields.

## 9. Warm singleton lifecycle

### `global_provider.rs`

| Item | Detail |
| --- | --- |
| `static GLOBAL_WARM_PROVIDER: OnceLock<Arc<RwLock<ProviderState>>>` | line 52 |
| `ProviderState { provider: Option<Arc<dyn MultiArrayEmbeddingProvider>>, concrete_provider: Option<Arc<ProductionMultiArrayProvider>>, init_error: Option<String> }` | lines 60-69 |
| `initialize_global_warm_provider()` (cuda impl) | lines 107-186; resolves `CONTEXT_GRAPH_MODELS_PATH` else `./models`, builds `ProductionMultiArrayProvider`, asserts `is_ready()`. Subsequent calls are no-ops on success; on failure the error message is cached and never retried. |
| `get_warm_provider()` | non-blocking `try_read`; returns the `Arc<dyn MultiArrayEmbeddingProvider>`. |
| `get_warm_causal_model()` / `get_warm_graph_model()` | lines 331-393; return concrete `Arc<CausalModel>` / `Arc<GraphModel>` for the discovery services (CausalDiscoveryService, GraphDiscoveryService). |
| `is_warm_initialized()`, `warm_status_message()` | non-blocking probes. Status messages: `"Not initialized"`, `"Initialization in progress"`, `"Initialization failed: <err>"`, `"Ready (14 models warm)"`. |

The lower-level `src/warm/` subsystem (loader, registry, memory_pool, validation, diagnostics, health, inference, integration, cuda_alloc) provides VRAM allocation, golden-vector validation, health checks, and diagnostic reporting under the public singleton. See [11_cuda_acceleration.md](11_cuda_acceleration.md) for the VRAM allocation lifecycle.

### Eager-load model size budget (FP16, approximate)

`warm/loader/constants.rs:13-29` (`MODEL_SIZES`):

| Slot | FP16 size |
| --- | --- |
| E1 Semantic | 600 MB |
| E2 TemporalRecent | 400 MB (no weights but reserved) |
| E3 TemporalPeriodic | 400 MB |
| E4 TemporalPositional | 350 MB |
| E5 Causal | 500 MB |
| E6 Sparse | 450 MB |
| E7 Code | 700 MB |
| E8 Graph | 550 MB |
| E9 HDC | 300 MB |
| E10 Contextual | 800 MB |
| E11 Entity (legacy) | 450 MB |
| E12 LateInteraction | 600 MB |
| E13 Splade | 450 MB |
| E11 Kepler (production) | 350 MB |
| E14 BgeM3Dense | 1200 MB |

`DEFAULT_EMBEDDING_DIMENSION = 768` is the loader's fallback. Total ≈ 7.6 GB, well under the 24 GB warm budget.

### Lazy / on-demand loading: `ModelRegistry`

`models/registry/`:
- Models loaded on first `get_model(ModelId)` call (`registry/operations.rs`).
- Per-model load locks serialize concurrent requests for the same model.
- Memory budget tracked via `MemoryTracker`; refuses to load when the total VRAM would exceed `ModelRegistryConfig::memory_budget_bytes`. Defaults: `RegistryConfig::rtx_5090()` = 32 GB, `rtx_4090()` = 24 GB, `testing(n)` overrides for tests (`registry/config.rs:50-86`).
- `max_concurrent_loads = 4` by default (`config.rs:34-58`).
- `RegistryStats { loaded_count, total_memory_bytes, ... }` (`registry/stats.rs`).
- Registry is used primarily by tests and the `BatchProcessor`; production runtime always goes through the warm singleton.

## 10. Warm-loading subsystem and VRAM budget

### `warm/config.rs` — `WarmConfig`

| Field | Default | Env override |
| --- | --- | --- |
| `vram_budget_bytes` | 24 GiB | `WARM_VRAM_BUDGET_BYTES` |
| `vram_headroom_bytes` | 8 GiB | `WARM_VRAM_HEADROOM_BYTES` |
| `model_weights_path` | `./models` | `WARM_MODEL_WEIGHTS_PATH` |
| `diagnostic_dump_path` | `/var/log/context-graph` | `WARM_DIAGNOSTIC_DUMP_PATH` |
| `cuda_device_id` | 0 (first of `CUDA_VISIBLE_DEVICES`) | `CUDA_VISIBLE_DEVICES` |
| `enable_test_inference` | true | `WARM_ENABLE_TEST_INFERENCE` |
| `max_load_time_per_model_ms` | 30000 | `WARM_MAX_LOAD_TIME_MS` |
| `quantization` | `Fp16` | (none) |

`QuantizationMode { Fp32 | Fp16 | Fp8 }` with memory multipliers 1.0 / 0.5 / 0.25 (`config.rs:48-67`). `total_vram_required = budget + headroom = 32 GiB` for the RTX 5090 default. `validate()` errors when `vram_budget_bytes == 0` or `model_weights_path` does not exist (`config.rs:217-233`).

### `warm/cuda_alloc/` — non-evictable VRAM

| Constant | Value |
| --- | --- |
| `REQUIRED_COMPUTE_MAJOR` | 12 (Blackwell / RTX 5090) |
| `REQUIRED_COMPUTE_MINOR` | 0 |
| `MINIMUM_VRAM_BYTES` | `31 * 1024^3` (≈31 GB) |
| `GB` | `1024^3` |
| `MAX_ALLOCATION_HISTORY` | 100 |
| `FAKE_ALLOCATION_BASE_PATTERN` | `0x7f80_0000_0000` (mock detector → exit 109) |
| `FAKE_ALLOCATION_BASE_MASK` | `0xFFFF_F000_0000_0000` |
| `SIN_WAVE_ENERGY_THRESHOLD` | 0.80 (FFT-based fake-output detector) |
| `GOLDEN_SIMILARITY_THRESHOLD` | 0.99 (golden-output cosine match for inference validation) |

The warm allocator uses `cudaMalloc` (not `cudaMallocManaged` / UVM) because UVM allocations can be silently migrated to system RAM under pressure, causing PCIe-bound latency spikes. `WarmCudaAllocator::allocate_protected(bytes)` issues `cudaMalloc`, stores the resulting tensor in a `live_tensors: Vec<(u64, Box<dyn Any + Send>)>` map keyed by allocation ID, and returns a `VramAllocation { ptr, size_bytes, device_id, is_protected: true }`. `free_protected(&allocation)` removes the tensor (drop → `cudaFree`). Allocator drop cleans all remaining tensors automatically. `VramAllocation` is `Send + Sync` data only — `Clone` and `Drop` were deliberately removed to prevent double-free when the allocator calls `cudaFree` while another handle still references the same pointer.

### Other warm modules

| Module | Purpose |
| --- | --- |
| `warm/state.rs` | `WarmModelState { Unloaded, Loading, Loaded, Failed(reason) }` |
| `warm/handle.rs` | `ModelHandle` token returned to callers; tracks owning allocation. |
| `warm/registry/` | `WarmModelRegistry`, `EMBEDDING_MODEL_IDS`, `TOTAL_MODEL_COUNT`. |
| `warm/memory_pool/` | `WarmMemoryPools { ModelMemoryPool (non-evictable), WorkingMemoryPool (evictable), ModelAllocation }`. |
| `warm/validation/` | `WarmValidator`, `TestInferenceConfig`, `TestInput`, `ValidationResult` — golden-vector regression on each load. |
| `warm/loader/{engine,operations,preflight,summary,types/*}` | `WarmLoader::load_all()` orchestrator, `load_weights`, `verify_checksum`, `LoadingSummary`, `WarmLoadResult`. |
| `warm/diagnostics/` | `WarmDiagnostics`, `WarmDiagnosticReport`, `GpuDiagnostics`, `MemoryDiagnostics`, `ModelDiagnostic`, `SystemInfo`, `ErrorDiagnostic`. JSON written to `diagnostic_dump_path` on fatal startup error. |
| `warm/inference/` | `InferenceEngine`, `cosine_similarity`, `detect_sin_wave_pattern`, `validate_inference_output_ap007`. |
| `warm/health/` | `WarmHealthChecker`, `WarmHealthCheck`, `WarmHealthStatus`. |
| `warm/integration/` | `WarmEmbeddingPipeline` (test-inference + warm-loaded model handles unified). |
| `warm/error.rs` | `WarmError` with `exit_code()` returning 101–110 for fatal startup errors. |

### Warm load phases (high level)

| Phase | Action |
| --- | --- |
| Preflight | Probe device compute capability vs `REQUIRED_COMPUTE_{MAJOR,MINOR}`, free VRAM vs `MINIMUM_VRAM_BYTES`, models directory vs `WARM_MODEL_WEIGHTS_PATH`. |
| SafeTensors + SHA256 | `load_weights` reads file bytes, computes SHA256, parses SafeTensors, returns `(bytes, checksum, TensorMetadata)`. |
| Allocate VRAM | `WarmCudaAllocator::allocate_protected(weight_bytes)` returns a `VramAllocation`. |
| Load tensors | `unsafe VarBuilder::from_mmaped_safetensors(...)` materializes weights on the device. |
| Validate | If `enable_test_inference`, run a known input through the model and compare against the golden-vector cosine threshold (`GOLDEN_SIMILARITY_THRESHOLD = 0.99`). FFT-based sin-wave check (`SIN_WAVE_ENERGY_THRESHOLD = 0.80`) flags fake outputs. |
| Register | `WarmModelRegistry` records `WarmModelState::Loaded` with the `ModelHandle`. |

## 11. BatchProcessor

`src/batch/processor/{core,worker,submit,config,stats,mod}.rs`:

- One `BatchQueue` per `ModelId`, initialized for **all 15 variants** (including legacy `Entity`).
- `mpsc::channel(request_buffer_size)` feeds a single worker task that polls per-model queues every `poll_interval_ms` (default 10 ms), flushes when `batch.len() >= max_batch_size` or when `oldest_age >= max_wait`. Bounded concurrency via `Semaphore::new(max_concurrent_batches)` (default 4).
- `BatchProcessorConfig::default()`: `poll_interval_ms = 10`, `max_concurrent_batches = 4`, `request_buffer_size = 1000` (`config.rs:29-37`).
- `BatchProcessorStats` tracks requests submitted / completed / failed / timed-out, batches flushed by trigger, queue depths.
- Shutdown drains in-flight batches before joining the worker.

Batch types (`src/batch/types/`): `BatchRequest`, `BatchQueue`, `Batch`, `BatchStats`.

## 12. Cache layer

`src/cache/mod.rs`:

- Backed by `moka::sync::Cache<CacheKey, Arc<MultiArrayEmbedding>>` (line 257).
- `CacheKey(u64) = xxhash_rust::xxh64(content.as_bytes(), 0)` (line 52).
- `CacheStats { hits, misses, insertions, estimated_evictions }` tracked atomically.
- Eviction: weight-based via `moka` weigher; `max_capacity = config.max_bytes` and weigher = `estimate_embedding_memory()` (lines 199-220). Each entry weighs ≈ `13 dims × 4 bytes + per-embedder overhead`.
- TTL configurable via `config.ttl_seconds` (lines 305-310); errors propagate via `EmbeddingResult` — there is no fallback on cache failure.
- `estimate_complete_embedding_memory()` returns `TOTAL_DIMENSION × 4 + MODEL_COUNT × 64 + 128` (lines 225-235), ≈ 52 KB per fully-filled entry. Tests assert the 30–100 KB range.
- `EmbeddingCache::disabled()` returns a no-op cache (`get` → `None`, `insert` → noop).

## 13. Quantization

`QuantizationMethod::for_model_id(ModelId)` is the routing table (`quantization/types.rs:58-87`):

| Method | Compression | Max recall loss | Embedders | CFs holding output |
| --- | --- | --- | --- | --- |
| `PQ8` | 32x | < 5 % (0.05) | E1, E5, E7, E10, E11 (Kepler), E14 | `CF_QUANTIZED_E1`, `CF_QUANTIZED_E5_*`, `CF_QUANTIZED_E7`, `CF_QUANTIZED_E10_*`, `CF_QUANTIZED_E11`, `CF_QUANTIZED_E14` |
| `Float8E4M3` | 4x | < 0.3 % (0.003) | E2, E3, E4, E8, legacy E11 (Entity) | `CF_QUANTIZED_E2`, `CF_QUANTIZED_E3`, `CF_QUANTIZED_E4`, `CF_QUANTIZED_E8_*` |
| `Binary` | 32x | 5–10 % (0.10) | E9 | `CF_QUANTIZED_E9` |
| `SparseNative` | native (depends on sparsity) | 0 % | E6, E13 | `CF_E6_SPARSE_INVERTED`, `CF_E13_SPLADE_INVERTED` |
| `TokenPruning` | ≈ 2x | < 2 % (0.02) | E12 | `CF_E12_LATE_INTERACTION` |

Implementations:

- `binary.rs` — `BinaryEncoder` sign-bit packs E9; threshold encoded in `QuantizationMetadata::Binary { threshold }`.
- `float8.rs` — `Float8E4M3Encoder` scalar quantize/dequantize; `QuantizationMetadata::Float8 { scale, bias }`. Range ≈ 1.5e-5 to 448 (positive).
- `pq8/` — Product Quantization with `NUM_SUBVECTORS = 8 × NUM_CENTROIDS = 256`. The `PQ8Codebook { embedding_dim, num_subvectors, num_centroids, centroids: Vec<Vec<Vec<f32>>>, codebook_id }` stores actual per-subvector centroids learned from training data — hardcoding `[-1, 1]` corrupts dequantization for any model whose normalized output is asymmetric. Submodules: `encoder.rs`, `training.rs` (k-means via `KMeansConfig`), `persistence.rs` (binary `CODEBOOK_MAGIC` + `CODEBOOK_VERSION`), `types.rs`.
- `router/` — `QuantizationRouter` dispatches `(ModelId, &[f32]) -> QuantizedEmbedding` and reverses on read. Pre-creates PQ-8 encoders for `{1024 → E1, 768 → E5/E10/Kepler, 1536 → E7}` (`router/mod.rs:67-72`). E14 (1024D) shares the E1 PQ-8 encoder.

`QuantizedEmbedding { method, original_dim, data: Vec<u8>, metadata: QuantizationMetadata }`.

`QuantizationMetadata` variants:

- `PQ8 { codebook_id: u32, num_subvectors: u8 }`
- `Float8 { scale: f32, bias: f32 }`
- `Binary { threshold: f32 }`
- `Sparse { vocab_size: usize, nnz: usize }`
- `TokenPruning { original_tokens: usize, kept_tokens: usize, threshold: f32 }`

**Asymmetric quantization (E5 / E8 / E10):** both vectors are quantized independently — cloning cause → effect destroys the dual-vector signal entirely.

## 14. Token pruning (E12)

`src/pruning/token_pruner.rs` + `src/pruning/config.rs`:

- `TokenPruningQuantizer::prune(&[Vec<f32>], optional_attention_weights)` keeps the top fraction of tokens by importance.
- `ImportanceScoringMethod`: `EmbeddingMagnitude`, `AttentionWeights`, `Combined`.
- Defaults: `target_compression = 0.5`, `min_tokens = 10`, `scoring_method = EmbeddingMagnitude`.
- Output: `PrunedEmbeddings { embeddings, kept_indices, original_count }`.

ColBERT MaxSim mechanics: the per-token query embedding `q_i ∈ R^128` is matched against every per-token document embedding `d_j ∈ R^128`; the score is `sum_i max_j cos(q_i, d_j)`. Token pruning keeps the most informative document tokens to bound memory while preserving recall. See [06_search_and_fusion.md](06_search_and_fusion.md).

## 15. Adapters (cross-crate trait bridges)

`src/adapters/code_provider.rs`: `E7CodeEmbeddingProvider` wraps `CodeModel` so it implements the `CodeEmbeddingProvider` trait defined in `context-graph-core`. Used by code-search ingestion paths.

## 16. Storage types

`src/storage/types/` re-exports:

- `StoredQuantizedFingerprint` — complete fingerprint with quantized embeddings (≈ 17 KB on disk vs ≈ 63 KB unquantized in-memory `TeleologicalFingerprint`).
- `IndexEntry` — entry in per-embedder HNSW index (dequantized for search).
- `EmbedderQueryResult` — single-embedder result.
- `MultiSpaceQueryResult` — RRF-fused multi-space result.
- Constants: `NUM_EMBEDDERS = 14`, `RRF_K`, `STORAGE_VERSION`, `EXPECTED_QUANTIZED_SIZE_BYTES`, `MAX_QUANTIZED_SIZE_BYTES`, `MIN_QUANTIZED_SIZE_BYTES`.

`src/storage/multi_space.rs::MultiSpaceSearchEngine` drives Stage-3 search; see [06_search_and_fusion.md](06_search_and_fusion.md) for the fusion semantics.

## 17. Training subsystem (`src/training/`)

E5 fine-tuning lives entirely under `src/training/`. Full details in [16_training_subsystem.md](16_training_subsystem.md).

| File | Contents |
| --- | --- |
| `lora.rs` | `LoraConfig { rank=16, alpha=16.0, dropout=0.1, hidden_size=768, num_layers=12, apply_query=true, apply_value=true, apply_key=false }`; `LoraAdapter { a: Var, b: Var, scale, dropout }` — A initialized with quasi-random Kaiming, B initialized to zero (so the adapter is identity at start). 589,824 trainable params at default config. |
| `pipeline.rs` | 3-stage progressive training pipeline: Stage 1 (warm-up, projection-only, 25 ep, `stage1_max_difficulty = 0.2`); Stage 2 (LoRA activation, 20 ep); Stage 3 (`stage3_direction_weight = 0.6`, 20 ep). Defaults: `batch_size = 16`, `eval_every = 5`, `early_stopping_patience = 10`, `output_dir = models/causal/trained`, `seed = 42`, optional `multitask_config = MultiTaskConfig::default()`. |
| `data.rs` | `CausalDataLoader`, `CausalTrainingPair`, `TrainingBatch`, `seed_training_pairs`, `expand_seed_pairs`. |
| `loss.rs` | `LossConfig`, `LossComponents` (contrastive InfoNCE + directional + multitask). |
| `optimizer.rs` | `AdamWConfig`, `ParamGroup`. |
| `multitask.rs` | `MultiTaskConfig`, `MultiTaskHeads` (direction 3-class + mechanism 7-class), `DirectionLabel`, `MechanismLabel`. |
| `evaluation.rs` | `Evaluator`, `EvaluationMetrics`. |
| `distillation.rs` | Optional teacher-student distillation. |
| `trainer.rs` | `CausalTrainer`, `TrainingConfig`, `EpochResult`. |

**Training-time prefix override:** `pipeline.rs:195,212` deliberately switches both cause and effect to `"search_document: "` instead of the inference-time `CAUSE_INSTRUCTION` / `EFFECT_INSTRUCTION` because identical instruction prefixes were collapsing all training vectors to the same direction. The projection heads learn the cause/effect separation. Production inference still uses the per-direction prefixes via `embed_dual` / `embed_dual_with_hint`.

### Training entry binary

`src/bin/train_causal.rs` provides the standalone `train-causal` binary. CLI flags: `--model-path` (default `models/causal`), `--data` (optional JSONL), `--output` (default `models/causal/trained`), `--epochs`, `--batch-size` (default 16), `--lora-rank` (default 16), `--eval-every` (default 5), `--seed` (default 42), `--no-multitask`. Builds `CausalTrainingPipeline` with `PipelineConfig` and runs all three stages.

## 18. Learner-state E15-E21 (`src/learner/mod.rs`)

The content embedding pipeline owns production E1-E14. The seven learner-state embedders are numbered after it, in a separate namespace. They never alter the `ModelId` enum or the persisted `TeleologicalFingerprint` layout.

| Constant | Value | Source |
| --- | --- | --- |
| `LEARNER_EMBEDDER_COUNT` | 7 | `learner/mod.rs:25` |
| `UTL_PLANNED_TOTAL_EMBEDDERS` | 21 | line 28 |
| `UTL_CONTENT_EMBEDDERS_BEFORE_LEARNER_STATE` | 14 | line 31 |

| Slot | `LearnerEmbedderSlot` | Modality | Native dim | Model name | `model_path` (under `models/`) | Scalar heads |
| --- | --- | --- | --- | --- | --- | --- |
| E15 | `E15AffectSpeech` | AffectSpeech | 1024 | Wav2Vec2-large-MSP-DIM | `affect-speech` | arousal, dominance, valence |
| E16 | `E16AffectFace` | AffectFace | 512 | OpenFace 3.0 + MTL emotion/AU/gaze head | `affect-face` | valence, arousal, au_intensity |
| E17 | `E17AffectText` | AffectText | 384 | MiniLM-L6-v2 + EmoBank regressor | `affect-text` | valence, arousal |
| E18 | `E18Ppg` | Ppg | 512 | PaPaGei PPG foundation model | `ppg` | hrv_coherence |
| E19 | `E19Eda` | Eda | 64 | WESAD chest physiology stress head | `eda` | stress_floor |
| E20 | `E20Eeg` | Eeg | 768 | LaBraM EEG foundation model | `eeg` | plasticity_window |
| E21 | `E21EegArtifactRobust` | EegArtifactRobust | 768 | EEGPT artifact-robust EEG model | `eeg-robust` | plasticity_window |

Public surface (re-exported from `lib.rs:121-131`):

- `learner_embedder_specs() -> Vec<LearnerEmbedderSpec>`
- `learner_model_asset_requirements() -> Vec<LearnerModelAssetRequirement>`
- `preflight_learner_assets(models_root, calibration_root) -> LearnerAssetPreflightReport`
- `embed_learner_signal(input) -> EmbeddingResult<LearnerEmbeddingOutput>`
- `embed_calibration_sample(...) -> EmbeddingResult<...>`
- `state_vector_from_outputs(&[LearnerEmbeddingOutput]) -> LearnerStateVector`
- `synthetic_calibration_fixture(slot) -> LearnerCalibrationFixture`
- `CALIBRATION_DATASET_MANIFEST: &[CalibrationDatasetSpec]` — WESAD, MSP-Podcast, EmoBank, GoEmotions, DAiSEE, AffWild2, DEAP, MAHNOB-HCI, SEED / SEED-IV, Sleep-EDF, etc.

These slots back `LearnerStateVector` and goal-distance computations only; they never enter content fusion. See [20_learning_subsystem.md](20_learning_subsystem.md).

## 19. SemanticFingerprint field layout

`crates/context-graph-core/src/types/fingerprint/semantic/`:

| Field | Representation | Notes |
| --- | --- | --- |
| `e1_semantic` | `Vec<f32>` (1024D) | dense |
| `e2_temporal_recent` / `e3_temporal_periodic` / `e4_temporal_positional` | `Vec<f32>` (512D each) | dense |
| `e5_causal_as_cause` / `e5_causal_as_effect` | `Vec<f32>` (768D each) | asymmetric |
| `e5_causal` | `Vec<f32>` | legacy combined slot, written empty by current providers |
| `e6_sparse` | `SparseVector { indices: Vec<u16>, values: Vec<f32> }` | up to 30522 vocab |
| `e7_code` | `Vec<f32>` (1536D) | dense |
| `e8_graph_as_source` / `e8_graph_as_target` | `Vec<f32>` (1024D each) | asymmetric |
| `e8_graph` | `Vec<f32>` | legacy, empty |
| `e9_hdc` | `Vec<f32>` (1024D projected) | dense |
| `e10_multimodal_paraphrase` / `e10_multimodal_as_context` | `Vec<f32>` (768D each) | asymmetric (paraphrase = doc, context = query) |
| `e11_entity` | `Vec<f32>` (768D KEPLER) | dense |
| `e12_late_interaction` | `Vec<Vec<f32>>` (128D / token, variable token count) | multivector |
| `e13_splade` | `SparseVector` | up to 30522 vocab |
| `e14_bge_m3_dense` | `Vec<f32>` (1024D, may be empty when E14 zero-adapter is active) | dense |

`SemanticFingerprint::validate()` errors: `DimensionMismatch`, `EmptyDenseEmbedding`, `TokenDimensionMismatch`, `SparseIndexOutOfBounds`, `SparseIndicesValuesMismatch`.

## 20. Tokenizer / instruction-prefix matrix

| Slot | Tokenizer crate config | Inference prefix | Training prefix | Notes |
| --- | --- | --- | --- | --- |
| E1 | `tokenizers = "0.22"`, BertWordpiece | `query: ` for queries, `passage: ` for documents | `passage: ` | train-time prefix MUST match inference. |
| E2 | none | informational `"timestamp:<unix_seconds>"` (ignored — sinusoidal path reads `created_at` directly) | n/a | — |
| E3 | none | informational `"timestamp:<unix_seconds>"` | n/a | — |
| E4 | none | `"sequence:<n>"` or `"epoch:<n>"` | n/a | first 256D session-signature, last 256D sinusoidal PE on instruction. |
| E5 | BertWordpiece (nomic-embed) | `search_query: Identify the cause in: ` and `search_query: Identify the effect of: ` | `search_document: ` for both | identical training prefixes collapse vectors; projection heads learn the separation. |
| E6 | BertWordpiece (SPLADE) | none | n/a | sparse indices / values output. |
| E7 | "BertWordpiece" enum value but loads its own Qwen2 BPE `tokenizer.json`. `tokenizers = "0.22"` is required for Qwen2 compatibility. | none | n/a | 32K context. |
| E8 | BertWordpiece (shared with E1) | none (asymmetric via projections) | n/a | source / target via learned projections. |
| E9 | none | none | n/a | character-trigram bind / bundle / permute. |
| E10 | BertWordpiece (e5-base-v2) | `query: ` for context (query-of-interest), `passage: ` for paraphrase (doc) | match inference | asymmetric via prefixing. |
| E11 (Kepler) | RobertaBpe | none | n/a | RoBERTa BPE; uses TransE entity-relation arithmetic. |
| E11 (Entity, legacy) | BertWordpiece | none | n/a | 384D MiniLM. |
| E12 | BertWordpiece (ColBERT) | none | n/a | per-token output for MaxSim. |
| E13 | BertWordpiece (SPLADE v3) | none | n/a | sparse indices / values. |
| E14 | XlmRobertaSentencePiece | none | n/a | up to 8192 tokens. |

## 21. Lesson highlights (load-bearing)

- **`(raw + 1) / 2` cosine normalization** on every fusion path. `code/store.rs::cosine_similarity_with_norm` historically returned raw `[-1, 1]`; verify any new cosine path applies the transform.
- **15 HNSW indexes, NOT 11/12/14** — see §6.
- **bincode + skip_serializing_if = BROKEN** — use JSON for all provenance / SourceMetadata.
- **RwLockReadGuard (parking_lot) is !Send** — drop before `.await` in `tokio::spawn`.
- **with_custom_weights() does NOT auto-set MultiSpace strategy** — caller must opt in.
- **RocksDB requires ALL on-disk CFs to be opened** — delete old DBs when removing CFs.
- **Cell\<bool\> for interior mutability** for `&self` methods that need to toggle flags (LoRA training mode in `training/lora.rs`).
- **LoRA dropout increases VRAM** — drop `batch_size` from 16 to 8 when dropout tensors are tracked in the grad graph.
- **Train / inference prefix MUST match** for nomic-embed (`search_document:` for training, `search_query: Identify the cause/effect:` for inference).
- **hard_negative_scale ≥ 2.0 with temperature = 0.05 causes catastrophic collapse** — use 1.0.
- **Best-stage selection should use `score_spread`, not `directional_accuracy`.**
- **Per-stage LoRA checkpoints needed** so the best stage's LoRA pairs with its projection.
- **Quantize BOTH asymmetric vectors** — cloning cause → effect destroys the dual-vector signal entirely.
- **PQ-8 must store actual centroids** — hardcoding `[-1, 1]` corrupts dequantization when data has different range.
- **`ModelId::all().len() = 15`, `ModelId::production().len() = 14`, `NUM_EMBEDDERS = 14`** — never confuse them when sizing arrays.
- **`ModelId::Entity = 384D legacy MiniLM`, `ModelId::Kepler = 768D production E11`** — don't change `Entity` to 768D, it breaks `EntityModel` tests.
- **E2 uses sinusoidal PE (FIXED)** — `compute_decay_embedding(timestamp, None, rates)` reads `created_at` directly. Old decay encoding produced delta ≈ 0 for every memory.
- **E2/E3/E4 HNSW indexes are populated** when `TEMPORAL_EMBEDDERS_ENABLED = true` and weight > 0.0.
- **E7 raw cosine enters fusion unnormalized** — `code/mod.rs` returns `[-1, 1]`. Apply `(raw + 1) / 2` before fusion.
- **Adapter / wrapper classes must override ALL trait methods they intercept** — overriding `embed_all` but not `embed_all_with_metadata` silently drops timestamps, sequence numbers, and causal hints.
- **VramAllocation: no Clone, no Drop** — only `WarmCudaAllocator::free_protected` knows the back-reference; `live_tensors` storage in the allocator is the single source of truth.
- **`tokenizers = "0.22"` is mandatory** for Qwen2 `tokenizer.json` format compatibility used by Qodo-Embed (E7).

## 22. Cross-references

- HNSW search and fusion: [06_search_and_fusion.md](06_search_and_fusion.md).
- Storage of fingerprint + index + quantized payloads: [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md).
- CUDA acceleration (kernels, allocator, candle device): [11_cuda_acceleration.md](11_cuda_acceleration.md).
- LoRA + projection training that produces the E5 trained weights: [16_training_subsystem.md](16_training_subsystem.md).
- Constellation aggregation over fingerprints: [17_constellation_subsystem.md](17_constellation_subsystem.md).
- Learner-state subsystem (E15-E21): [20_learning_subsystem.md](20_learning_subsystem.md).
