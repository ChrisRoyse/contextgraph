# Embedding Subsystem

Source files covered:

- `crates/context-graph-embeddings/src/lib.rs`
- `crates/context-graph-embeddings/src/global_provider.rs`
- `crates/context-graph-embeddings/src/provider/mod.rs`
- `crates/context-graph-embeddings/src/provider/multi_array.rs`
- `crates/context-graph-embeddings/src/provider/causal_hint.rs`
- `crates/context-graph-embeddings/src/types/model_id/core.rs`
- `crates/context-graph-embeddings/src/types/model_id/tokenizer.rs`
- `crates/context-graph-embeddings/src/types/dimensions/constants.rs`
- `crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs`
- `crates/context-graph-embeddings/src/traits/model_factory/memory.rs`
- `crates/context-graph-embeddings/src/models/pretrained/semantic/` (model, constants, loader)
- `crates/context-graph-embeddings/src/models/pretrained/causal/` (config, model, forward, weights, loader)
- `crates/context-graph-embeddings/src/models/pretrained/code/` (model, constants, forward, weights)
- `crates/context-graph-embeddings/src/models/pretrained/graph/` (model, constants, projections)
- `crates/context-graph-embeddings/src/models/pretrained/sparse/` (model, encoder, mlm_head)
- `crates/context-graph-embeddings/src/models/pretrained/contextual/` (model, constants)
- `crates/context-graph-embeddings/src/models/pretrained/kepler/` (model, encoding, transe, types)
- `crates/context-graph-embeddings/src/models/pretrained/late_interaction/` (model, gpu_forward, scoring, types)
- `crates/context-graph-embeddings/src/models/pretrained/weight_projection/constants.rs`
- `crates/context-graph-embeddings/src/models/pretrained/rgcn/constants.rs`
- `crates/context-graph-embeddings/src/models/custom/temporal_recent/` (model, compute, constants)
- `crates/context-graph-embeddings/src/models/custom/temporal_periodic/` (model, encoding, constants)
- `crates/context-graph-embeddings/src/models/custom/temporal_positional/` (model, encoding, constants)
- `crates/context-graph-embeddings/src/models/custom/hdc/` (model, encoding, operations, types)
- `crates/context-graph-embeddings/src/models/factory/default_factory.rs`
- `crates/context-graph-embeddings/src/models/registry/core.rs`
- `crates/context-graph-embeddings/src/batch/mod.rs`
- `crates/context-graph-embeddings/src/batch/processor/core.rs`
- `crates/context-graph-embeddings/src/batch/types/queue.rs`
- `crates/context-graph-embeddings/src/cache/mod.rs`
- `crates/context-graph-embeddings/src/gpu/mod.rs`
- `crates/context-graph-embeddings/src/warm/mod.rs`
- `crates/context-graph-embeddings/src/pruning/token_pruner.rs`
- `crates/context-graph-embeddings/src/quantization/mod.rs`
- `crates/context-graph-embeddings/src/quantization/types.rs`
- `crates/context-graph-embeddings/src/quantization/pq8/` (encoder, training, types)
- `crates/context-graph-embeddings/src/quantization/binary.rs`
- `crates/context-graph-embeddings/src/quantization/float8.rs`
- `crates/context-graph-embeddings/src/quantization/router/encoders.rs`
- `crates/context-graph-embeddings/src/training/mod.rs`
- `crates/context-graph-embeddings/src/training/pipeline.rs`
- `crates/context-graph-embeddings/src/training/lora.rs`
- `crates/context-graph-embeddings/src/training/loss.rs`
- `crates/context-graph-embeddings/src/training/optimizer.rs`
- `crates/context-graph-embeddings/src/training/trainer.rs`
- `crates/context-graph-embeddings/src/training/multitask.rs`
- `crates/context-graph-embeddings/src/training/distillation.rs`
- `crates/context-graph-embeddings/src/bin/train_causal.rs`
- `crates/context-graph-embeddings/src/adapters/code_provider.rs`
- `crates/context-graph-embeddings/src/error/types.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`
- `crates/context-graph-core/src/types/fingerprint/sparse.rs`
- `crates/context-graph-core/src/similarity/dense/primitives.rs`
- `crates/context-graph-core/src/similarity/sparse.rs`
- `crates/context-graph-core/src/similarity/token_level.rs`
- `crates/context-graph-core/src/weights/mod.rs`

Cross-references: [04_database_schema.md](04_database_schema.md) · [06_search_and_fusion.md](06_search_and_fusion.md) · [16_training_subsystem.md](16_training_subsystem.md) · [11_cuda_acceleration.md](11_cuda_acceleration.md)

---

## 2. Overview

The embedding subsystem converts text and code into a `SemanticFingerprint` composed of 13 independent embedding vectors (E1–E13). Each slot is stored separately (no fusion) and indexed by the storage layer (15 HNSW indexes total; see [04_database_schema.md](04_database_schema.md)). The crate is **CUDA-mandatory**: `lib.rs` emits a `compile_error!` if the `candle` feature is disabled. Target hardware is NVIDIA RTX 5090 (Blackwell, CC 12.0) with CUDA 13.1. There are no CPU fallbacks.

| Property | Value |
|----------|-------|
| Number of embedder slots | 13 (E1–E13) |
| Number of `ModelId` variants | 14 (13 + legacy `Entity`) |
| Dense HNSW indexes in storage | 15 (includes asymmetric duals for E5, E8, E10) |
| Global provider | `ProductionMultiArrayProvider` behind `OnceLock<RwLock<ProviderState>>` singleton |
| All models loaded | Eagerly at startup, in parallel via `tokio::join!` |
| GPU backend | Candle with CUDA; FP16 by default (`SingleModelConfig::cuda_fp16()`) |
| Total VRAM (FP32 estimate) | ~6.6 GB (see `MEMORY_ESTIMATES`) |
| Models directory | `CONTEXT_GRAPH_MODELS_PATH` env var, fallback `./models` |

Entry points: `initialize_global_warm_provider()` → loads all 13 models into VRAM and locks the `OnceLock`. `get_warm_provider()` returns `Arc<dyn MultiArrayEmbeddingProvider>`. Per-model direct access exposed by `get_warm_causal_model()` and `get_warm_graph_model()` for discovery services.

---

## 3. Embedder Table

All dimensions are the **projected dimension** used by the fingerprint (see `types::dimensions::constants`). Asymmetric embedders store two vectors per memory.

| ID | `ModelId` | Model | Native Dim | Projected Dim | Role | Asymmetric | Sparse/Dense | HNSW | Weight-Gated | File |
|----|-----------|-------|-----------|---------------|------|------------|--------------|------|--------------|------|
| E1 | `Semantic` | intfloat/e5-large-v2 | 1024 | 1024 | Semantic retrieval | No | Dense | Yes | No | `models/pretrained/semantic/` |
| E2 | `TemporalRecent` | Sinusoidal PE of Unix timestamp | 512 | 512 | Recency / temporal proximity | No | Dense | Yes (weight-gated) | Yes (`TEMPORAL_EMBEDDERS_ENABLED`) | `models/custom/temporal_recent/` |
| E3 | `TemporalPeriodic` | Fourier basis over multiple periods | 512 | 512 | Cyclical time (daily/weekly/yearly) | No | Dense | Yes (weight-gated) | Yes (`TEMPORAL_EMBEDDERS_ENABLED`) | `models/custom/temporal_periodic/` |
| E4 | `TemporalPositional` | Sinusoidal PE of session seq or timestamp | 512 | 512 | Session ordering | No | Dense | Yes (weight-gated) | Yes (`TEMPORAL_EMBEDDERS_ENABLED`) | `models/custom/temporal_positional/` |
| E5 | `Causal` | nomic-ai/nomic-embed-text-v1.5 + LoRA + projection | 768 | 768 (×2 cause/effect) | Causal reasoning | Yes (cause, effect) | Dense | Yes (2 HNSW: cause, effect) | No | `models/pretrained/causal/` |
| E6 | `Sparse` | naver/splade-cocondenser-ensembledistil | 30522 | 1536 projected | Lexical recall | No | Sparse (inverted) | No (inverted index) | No | `models/pretrained/sparse/` |
| E7 | `Code` | Qodo/Qodo-Embed-1-1.5B | 1536 | 1536 | Code patterns (32K ctx) | No | Dense | Yes | No | `models/pretrained/code/` |
| E8 | `Graph` | intfloat/e5-large-v2 + W_source, W_target | 1024 | 1024 (×2 source/target) | Directed graph | Yes (source, target) | Dense | Yes (2 HNSW: source, target) | No | `models/pretrained/graph/` |
| E9 | `Hdc` | Hyperdimensional (char n-grams, 10K-bit) | 10000 | 1024 projected | Typo-robust / structural | No | Dense | Yes | No | `models/custom/hdc/` |
| E10 | `Contextual` | intfloat/e5-base-v2 + prefix asymmetry | 768 | 768 (×2 paraphrase/context) | Paraphrase ↔ context | Yes (paraphrase, context) | Dense | Yes (2 HNSW: paraphrase, context) | No | `models/pretrained/contextual/` |
| E11 | `Kepler` | KEPLER (RoBERTa-base + TransE on Wikidata5M) | 768 | 768 | Entity / TransE | No | Dense | Yes | Yes (`E11_ENTITY_ENABLED = false`) | `models/pretrained/kepler/` |
| E11 legacy | `Entity` | all-MiniLM-L6-v2 (deprecated) | 384 | 384 | Entity (legacy) | No | Dense | No (not loaded) | N/A (replaced by Kepler) | n/a |
| E12 | `LateInteraction` | colbert-ir/colbertv2.0 | 128/token | 128/token | Token-level MaxSim | No | Dense (token-level) | No (per-token storage) | No | `models/pretrained/late_interaction/` |
| E13 | `Splade` | SPLADE v3 (BERT + MLM head) | 30522 | 1536 projected | Lexical recall (Stage 1) | No | Sparse (inverted) | No (inverted index) | No | `models/pretrained/sparse/` (shared impl) |

`ModelId::all()` returns 14 variants (includes legacy `Entity`). `ModelId::production()` returns the 13 active slots. `NUM_EMBEDDERS = 13` in `context_graph_core::types::fingerprint::semantic::constants`.

### 3.1 `ModelId` enum (repr u8)

| Variant | u8 | Slot |
|---------|-----|------|
| `Semantic` | 0 | E1 |
| `TemporalRecent` | 1 | E2 |
| `TemporalPeriodic` | 2 | E3 |
| `TemporalPositional` | 3 | E4 |
| `Causal` | 4 | E5 |
| `Sparse` | 5 | E6 |
| `Code` | 6 | E7 |
| `Graph` | 7 | E8 |
| `Hdc` | 8 | E9 |
| `Contextual` | 9 | E10 (serde alias `"Multimodal"`) |
| `Entity` | 10 | E11 legacy (384D MiniLM) |
| `LateInteraction` | 11 | E12 |
| `Splade` | 12 | E13 |
| `Kepler` | 13 | E11 production (768D KEPLER) |

### 3.2 Tokenizer families (`TokenizerFamily`)

| Family | Embedders |
|--------|-----------|
| `BertWordpiece` | E1, E5, E6, E7, E8, E10, E12, E13, legacy Entity |
| `RobertaBpe` | E11 Kepler |
| `None` | E2, E3, E4, E9 (no tokenization) |

E7 declares `BertWordpiece` for enum simplicity but loads its own Qwen2 BPE tokenizer internally.

### 3.3 Output normalization

All dense embedders produce **L2-normalized** vectors. Cosine similarity via `context_graph_core::similarity::cosine_similarity` returns raw `[-1, 1]`; fusion layers apply `(raw + 1) / 2` to map to `[0, 1]` (see `similarity::default_engine::compute_similarity`, line 268):

```rust
let normalized = (sim + 1.0) / 2.0;   // orthogonal -> 0.5, identical -> 1.0
```

---

## 4. Encoder Pipelines

### 4.1 Dense transformer path (E1, E5, E7, E8, E10, E11)

```text
text -> tokenizer (BERT/BPE) -> input_ids + attention_mask
     -> embed lookup (GPU) -> transformer forward (candle)
     -> pooling (mean / CLS) -> optional projection (LoRA, W_src/W_tgt)
     -> L2 normalize -> Vec<f32>
```

Prefix strings are prepended to text before tokenization:

| Embedder | Prefix | Constant |
|----------|--------|----------|
| E1 passage | `passage: ` | `semantic::PASSAGE_PREFIX` |
| E1 query | `query: ` | `semantic::QUERY_PREFIX` |
| E5 cause | `search_query: Identify the cause in: ` | `causal::CAUSE_INSTRUCTION` |
| E5 effect | `search_query: Identify the effect of: ` | `causal::EFFECT_INSTRUCTION` |
| E5 content (trained) | `search_document: ` | inline in `causal/model.rs` |
| E8 (both roles) | `passage: ` (from shared e5-large-v2) | — |
| E10 intent / paraphrase | `query: ` | `contextual::INTENT_PREFIX` |
| E10 context / passage | `passage: ` | `contextual::CONTEXT_PREFIX` |
| E11 Kepler | `[TYPE] name` formatting via `encode_entity()` | — |

Model-family configs:

| Model | Layers | Hidden | Heads | FFN | LN eps | Position |
|-------|--------|--------|-------|-----|--------|----------|
| E1 semantic (e5-large-v2) | 24 | 1024 | 16 | 4096 | 1e-12 | absolute |
| E5 causal (nomic-embed v1.5) | 12 | 768 | 12 | 3072 | 1e-12 | rotary (base 1000, SwiGLU FFN, no biases, fused QKV) |
| E7 code (Qodo/Qwen2) | Qwen2 config from `config.json` | 1536 | — | — | — | RoPE (32K ctx) |
| E8 graph (e5-large-v2) | 24 | 1024 | 16 | 4096 | 1e-12 | absolute (same weights as E1, separate W_source/W_target projections) |
| E10 contextual (e5-base-v2) | 12 | 768 | 12 | 3072 | 1e-12 | absolute |
| E11 Kepler (RoBERTa-base) | 12 | 768 | 12 | 3072 | 1e-5 | absolute (BPE) |
| E12 ColBERT (BERT-base) | 12 | 768 | 12 | 3072 | 1e-12 | absolute, then 768→128 projection per token |

### 4.2 Sparse path (E6, E13)

```text
text -> BERT tokenizer (max 512) -> transformer forward
     -> MLM head (30522 vocab projection)
     -> log(1 + ReLU(logits)) per token
     -> max-pool across tokens -> sparse 30522D vector
     -> top-k (~5% active) -> (indices: Vec<u16>, values: Vec<f32>)
     -> stored via SparseVector (indices sorted, values aligned)
```

Both E6 and E13 use the same `SparseModel` backbone; the `ModelId` field distinguishes them. Sparse vectors skip HNSW and are queried through the inverted index (see [04_database_schema.md](04_database_schema.md) CF_SPARSE). Similarity uses Jaccard on active dimensions via `similarity::sparse::jaccard_similarity` (merge-join, O(n+m)).

### 4.3 Temporal path (E2, E3, E4)

Pure CPU, no tokenizer, no model weights. Timestamps come from `EmbeddingMetadata` via `e2_instruction()` / `e3_instruction()` / `e4_instruction()`. See §6.

### 4.4 Code path (E7)

```text
source code -> Qwen2 BPE tokenizer (max 32768)
            -> Qwen2 transformer forward (candle GPU)
            -> mean pooling -> L2 normalize -> 1536D dense
```

E7 raw cosine is mapped to `[0, 1]` via `(raw + 1) / 2` before fusion.

### 4.5 Entity path (E11 Kepler)

```text
entity name (optionally "[TYPE] name") -> RoBERTa BPE tokenizer
    -> RoBERTa forward (12 layers) -> mean pool -> L2 normalize -> 768D
```

KEPLER supports TransE operations in the same 768D space:

- `transe_score(h, r, t) = -||h + r - t||_2`
- `predict_tail(h, r) = h + r`
- `predict_relation(h, t) = t - h`

### 4.6 HDC path (E9)

```text
text -> char n-grams (default trigrams, seed 42)
     -> each char -> 10000-bit random hypervector (seeded RNG)
     -> bind (XOR) with position permutation
     -> bundle (majority vote) across n-grams -> 10000-bit hypervector
     -> projection to 1024D f32 (L2 normalized)
```

Operations defined in `custom/hdc/operations.rs`: `bind` (XOR, commutative self-inverse), `bundle` (majority vote over vectors), `permute` (circular shift), `hamming_distance`, `similarity` in `[0, 1]` (0.5 = orthogonal).

### 4.7 ColBERT path (E12)

```text
text -> BERT tokenizer -> BERT forward (GPU)
     -> ColBERT linear projection 768D -> 128D per token
     -> L2 normalize per token
     -> Vec<Vec<f32>>  (one 128D vector per kept token)
```

All per-token vectors are returned; scoring at query time uses MaxSim:

```rust
// context_graph_core::similarity::token_level::max_sim
// score = (1/|Q|) * sum_i max_j cosine(q_i, d_j)
```

Token pruning (top 50% by importance) is handled by `pruning::TokenPruner` before storage (see §9).

---

## 5. Asymmetric Embedders

Three embedders produce **two vectors per memory** for direction-sensitive similarity.

### 5.1 E5 Causal — (cause, effect)

```rust
// provider/multi_array.rs :: CausalDualEmbedderAdapter::embed_dual
// Returns (Vec<f32>, Vec<f32>) each 768D
let cause  = forward(CAUSE_INSTRUCTION  + content);   // "search_query: Identify the cause in: ..."
let effect = forward(EFFECT_INSTRUCTION + content);   // "search_query: Identify the effect of: ..."
```

Stored in `SemanticFingerprint.e5_causal_as_cause` and `e5_causal_as_effect` (legacy `e5_causal` field is empty). Trained LoRA adapters (rank 16, Q+V on 12 layers = 589,824 params) and a learned 768×768 projection are applied to both passes; weights loaded from `models/causal/trained/` (required — no base-model fallback). An optional `CausalHint` from the LLM can bias the vectors post-hoc (`embed_dual_with_hint`): cause bias 1.3×/effect 0.8× for `Cause` direction, symmetric for `Effect`, identity for `Neutral`.

Asymmetric scoring at query time applies 1.2× modifier for cause→effect and 0.8× for effect→cause (see [06_search_and_fusion.md](06_search_and_fusion.md)).

### 5.2 E8 Graph — (source, target)

```rust
// provider/multi_array.rs :: GraphDualEmbedderAdapter::embed_dual
// Returns (Vec<f32>, Vec<f32>) each 1024D
let base   = e5_large_v2_forward(content);                     // shared with E1 (VRAM reuse)
let source = L2_normalize(W_source * base + b_source);         // learned projection
let target = L2_normalize(W_target * base + b_target);
```

Stored in `e8_graph_as_source` / `e8_graph_as_target`. Model path: `models/semantic/` (same e5-large-v2 checkpoint as E1) plus graph-specific projection weights in `models/graph/`.

### 5.3 E10 Contextual — (paraphrase, context)

```rust
// provider/multi_array.rs :: ContextualDualEmbedderAdapter::embed_dual
// Returns (Vec<f32>, Vec<f32>) each 768D
let paraphrase = forward(INTENT_PREFIX  + content);   // "query: ..."
let context    = forward(CONTEXT_PREFIX + content);   // "passage: ..."
```

Stored in `e10_multimodal_paraphrase` / `e10_multimodal_as_context`. No extra projection — asymmetry comes from the e5-base-v2 prefix-conditioning alone.

---

## 6. Temporal Embedders (E2, E3, E4)

All three are pure functions of a timestamp (or session sequence). CPU-only, deterministic, L2-normalized.

### 6.1 E2 TemporalRecent — sinusoidal PE of absolute Unix timestamp

`compute_decay_embedding(timestamp, reference_time=None, decay_rates)`:

```text
half_dim = 256           // 512D = 256 sin/cos pairs
base     = 86400.0       // 1 day in seconds

for k in 0..256:
    divisor = base^(2k / 512)
    angle   = timestamp_secs / divisor
    vec[2k]   = sin(angle)
    vec[2k+1] = cos(angle)

L2-normalize(vec)
```

Frequency progression: low `k` captures seconds-level proximity, high `k` captures month-level. When `reference_time = Some(ref)` (test path only), falls back to legacy exponential-decay encoding over `(ref - timestamp)` with phase-varied cosine features — not used in production.

### 6.2 E3 TemporalPeriodic — Fourier basis over configured periods

```text
periods        = DEFAULT_PERIODS   (e.g., 3600, 86400, 604800, 2592000, 31536000)
num_harmonics  = FEATURES_PER_PERIOD / (2 * periods.len())

for period in periods:
    position_in_cycle = timestamp_secs mod period
    phase             = 2π * position_in_cycle / period
    for n in 1..=num_harmonics:
        vec.push(sin(n * phase))
        vec.push(cos(n * phase))

pad with 0.0 to 512
L2-normalize(vec)
```

Extracts timestamp from `ModelInput.instruction` via `"timestamp:<RFC3339>"` or `"epoch:<unix_secs>"`. Falls back to `Utc::now()` if not parseable.

### 6.3 E4 TemporalPositional — sinusoidal PE of session sequence OR timestamp

`compute_positional_encoding_from_position(position, base=10000, d_model=512, is_sequence)`:

```text
if is_sequence:
    effective_base = max(base / 100, 10.0)   // tighter spread for small seq numbers
else:
    effective_base = base                    // 10000 for Unix timestamps

for i in 0..256:
    divisor = effective_base^(2i / 512)
    angle   = position / divisor
    vec[2i]   = sin(angle)
    vec[2i+1] = cos(angle)

L2-normalize(vec)
```

Instruction parsing: `"sequence:<N>"` → `is_sequence=true`; `"epoch:<N>"` → `is_sequence=false`. `EmbeddingMetadata::e4_instruction()` prefers session sequence when available. A hybrid 256D mode (`compute_positional_encoding_hybrid`) exists for concatenation with a 256D session signature — used for V_ordering fingerprints.

Zero vectors (512D) are emitted for E2/E3/E4 when `TEMPORAL_EMBEDDERS_ENABLED = false` (see `core::weights::mod.rs`).

---

## 7. Late Interaction and Sparse

### 7.1 E12 ColBERT MaxSim

Storage: `Vec<Vec<f32>>` of shape `[num_tokens, 128]` in `SemanticFingerprint.e12_late_interaction`. No HNSW index — retrieval operates directly on token lists. Scoring:

```rust
// context_graph_core::similarity::token_level
pub fn max_sim(query: &[Vec<f32>], document: &[Vec<f32>]) -> f32 {
    // score = (1/|Q|) * sum_i max_j cosine(q_i, d_j)
    // both query and doc tokens must be 128D
}
```

Also exposed: `symmetric_max_sim`, `approximate_max_sim` (sampling), `token_alignments` (best match per query token with index and score).

### 7.2 E6 & E13 SPLADE (inverted)

Storage: `SparseVector { indices: Vec<u16>, values: Vec<f32> }` with sorted unique indices, vocab size 30522 (`SPARSE_VOCAB_SIZE`). Maximum active dimensions: `MAX_SPARSE_ACTIVE = 10_000` (typical ~1500). The storage layer maintains an inverted index CF mapping token → memory IDs; Jaccard similarity is computed merge-join over sorted indices.

---

## 8. Quantization

Every vector written to disk is quantized per `QuantizationMethod::for_model_id(model_id)`.

| Method | Embedders | Compression | Max Recall Loss | Metadata |
|--------|-----------|-------------|-----------------|----------|
| `PQ8` | E1, E5, E7, E10, E11 Kepler | 32× | <5% | `{ codebook_id: u32, num_subvectors: u8 }` |
| `Float8E4M3` | E2, E3, E4, E8, legacy Entity | 4× | <0.3% | `{ scale: f32, bias: f32 }` |
| `Binary` | E9 | 32× | 5–10% | `{ threshold: f32 }` |
| `SparseNative` | E6, E13 | native | 0% | `{ vocab_size, nnz }` |
| `TokenPruning` | E12 | ~50% | <2% | `{ original_tokens, kept_tokens, threshold }` |

Dispatch: `quantization::router::QuantizationRouter` routes each `ModelId` to its encoder. Encoded output: `QuantizedEmbedding { method, original_dim, data: Vec<u8>, metadata: QuantizationMetadata }`.

### 8.1 PQ-8 details

```text
NUM_SUBVECTORS = 8
NUM_CENTROIDS  = 256         // 1 byte per subvector index
codebook_magic = b"PQ8C"
codebook_version = 1

dim must be divisible by 8
quantize:   split vec into 8 subvectors of D/8 -> nearest centroid index (1 byte)
            -> data = 8 bytes total
dequantize: lookup centroid[subvector_idx][centroid_id] -> concatenate -> D f32

codebook:   PQ8Codebook { embedding_dim, num_subvectors, num_centroids,
                          centroids: [[[f32; D/8]; 256]; 8],
                          codebook_id: u32 }
```

Codebooks are trained with k-means per embedder (see `quantization/pq8/training.rs`) and persisted with `CODEBOOK_MAGIC` + version header. `PQ8Encoder` training errors fail fast on insufficient samples or non-convergence (`KMeansDidNotConverge`).

### 8.2 Float8 E4M3

1 sign bit + 4-bit exponent (bias 7) + 3-bit mantissa. Range ~1.5e-5 to 448. Each vector stores per-vector `scale` and `bias` for denormalization: `original = quantized × scale + bias`.

### 8.3 Binary (E9)

Per-element threshold → single sign bit. Stored as packed bytes.

---

## 9. Provider Abstraction

### 9.1 Traits

`context_graph_embeddings::traits::EmbeddingModel` (async_trait):

```rust
pub trait EmbeddingModel: Send + Sync {
    fn model_id(&self) -> ModelId;
    fn supported_input_types(&self) -> &[InputType];
    fn is_initialized(&self) -> bool;
    async fn load(&self) -> EmbeddingResult<()>;
    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding>;
    async fn embed_sparse(&self, input: &ModelInput) -> EmbeddingResult<(Vec<u16>, Vec<f32>)>;
    // defaults delegate to ModelId
    fn dimension(&self) -> usize;
    fn projected_dimension(&self) -> usize;
    fn latency_budget_ms(&self) -> u32;
    fn max_tokens(&self) -> usize;
    fn is_pretrained(&self) -> bool;
    fn validate_input(&self, input: &ModelInput) -> EmbeddingResult<()>;
}
```

`context_graph_core::traits`:

- `SingleEmbedder` — one text → dense `Vec<f32>` (E1, E2, E3, E4, E7, E9, E11).
- `SparseEmbedder` — one text → `SparseVector` (E6, E13).
- `TokenEmbedder` — one text → `Vec<Vec<f32>>` (E12).
- `MultiArrayEmbeddingProvider` — `embed_all(text)` / `embed_all_with_metadata(text, meta)` → `MultiArrayEmbeddingOutput { fingerprint, total_latency, per_embedder_latency: [Duration; 13], model_ids: [String; 13], e5_hint_provenance: Option<...> }`.

### 9.2 Adapters (`provider/multi_array.rs`)

| Adapter | Wraps | Implements | Notes |
|---------|-------|------------|-------|
| `DenseEmbedderAdapter` | `Box<dyn EmbeddingModel>` in `Arc<RwLock<_>>` | `SingleEmbedder` | E1, E2, E3, E4, E7, E9, E11 |
| `SparseEmbedderAdapter` | `Box<dyn EmbeddingModel>` | `SparseEmbedder` | E6, E13 |
| `TokenEmbedderAdapter` | `Box<dyn EmbeddingModel>` | `TokenEmbedder` | E12 (reshapes flat output to `[num_tokens][128]`) |
| `CausalDualEmbedderAdapter` | `Arc<CausalModel>` | custom dual API | E5; exposes `embed_dual(text) -> (cause, effect)` and `embed_dual_with_hint` |
| `GraphDualEmbedderAdapter` | `Arc<GraphModel>` | custom dual API | E8 |
| `ContextualDualEmbedderAdapter` | `Arc<ContextualModel>` | custom dual API | E10 |

All adapters are `Send + Sync` (manual `unsafe impl` justified by internal `RwLock` mediation of non-Send inner state).

### 9.3 Global singleton (`global_provider.rs`)

```rust
static GLOBAL_WARM_PROVIDER: OnceLock<Arc<RwLock<ProviderState>>>;

struct ProviderState {
    provider:          Option<Arc<dyn MultiArrayEmbeddingProvider>>,
    concrete_provider: Option<Arc<ProductionMultiArrayProvider>>,
    init_error:        Option<String>,
}
```

Public API:

| Function | Purpose |
|----------|---------|
| `initialize_global_warm_provider()` | Eagerly load all 13 models. Idempotent. Fails fast if any model missing or CUDA unavailable. |
| `get_warm_provider() -> Arc<dyn MultiArrayEmbeddingProvider>` | Non-blocking `try_read`; fails if not initialized. |
| `get_warm_causal_model() -> Arc<CausalModel>` | Direct handle for `CausalDiscoveryService`. |
| `get_warm_graph_model() -> Arc<GraphModel>` | Direct handle for `GraphDiscoveryService`. |
| `is_warm_initialized() -> bool` | Non-blocking status check. |
| `warm_status_message() -> String` | `"Ready (13 models warm)"` / `"Not initialized"` / `"Initialization failed: ..."` / `"Initialization in progress"`. |

### 9.4 `ProductionMultiArrayProvider`

Owns one `Arc` per embedder slot and runs all 13 in parallel via `tokio::join!`. Pseudocode of `embed_all`:

```rust
tokio::join!(
    e1.embed(text),
    if TEMPORAL_EMBEDDERS_ENABLED { e2.embed(text) } else { zeros(512) },
    if TEMPORAL_EMBEDDERS_ENABLED { e3.embed(text) } else { zeros(512) },
    if TEMPORAL_EMBEDDERS_ENABLED { e4.embed(text) } else { zeros(512) },
    e5.embed_dual(text),          // (cause, effect)
    e6.embed_sparse(text),
    e7.embed(text),
    e8.embed_dual(text),          // (source, target)
    e9.embed(text),
    e10.embed_dual(text),         // (paraphrase, context)
    if E11_ENTITY_ENABLED { e11.embed(text) } else { zeros(768) },
    e12.embed_tokens(text),
    e13.embed_sparse(text),
)
-> SemanticFingerprint { e1_semantic, e2_temporal_recent, ...
                         e5_causal_as_cause, e5_causal_as_effect, e5_causal: vec![],
                         e8_graph_as_source, e8_graph_as_target, e8_graph: vec![],
                         e10_multimodal_paraphrase, e10_multimodal_as_context,
                         ... }
```

Target latency (constitution): <30 ms for all 13 on a single text.

### 9.5 Causal hint provider

`CausalHintProvider` trait (`provider::causal_hint`) allows an LLM to supply a `CausalHint { direction_hint: Cause | Effect | Neutral, confidence: f32, cause_spans, effect_spans, asymmetry_strength }` which `embed_dual_with_hint` uses to (a) inject guided markers during E5 forward and (b) apply post-hoc direction bias multipliers. `NoOpCausalHintProvider` is the default when no LLM is wired in.

---

## 10. Model Loading

### 10.1 Directory layout

Resolved from `CONTEXT_GRAPH_MODELS_PATH` env var, default `./models`:

```text
models/
├── semantic/                 # e5-large-v2 (shared by E1 and E8)
│   ├── model.safetensors
│   ├── tokenizer.json
│   └── config.json
├── causal/                   # nomic-embed-text-v1.5
│   ├── model.safetensors
│   ├── tokenizer.json
│   ├── config.json
│   └── trained/              # REQUIRED — LoRA + projection from train-causal
│       ├── lora_q.safetensors
│       ├── lora_v.safetensors
│       └── projection.safetensors
├── code-1536/                # Qodo-Embed-1-1.5B
├── graph/                    # W_source / W_target projection weights for E8
├── contextual/               # e5-base-v2
├── kepler/                   # KEPLER RoBERTa + TransE weights
├── sparse/                   # SPLADE (E6)
├── splade-v3/                # SPLADE v3 (E13)
├── late-interaction/         # ColBERT v2 + 768->128 projection
├── graph_weights/            # LearnedWeightProjection
├── rgcn/                     # R-GCN relational graph conv weights
└── hermes-2-pro/             # LLM for causal discovery / hints
```

### 10.2 Weight loading

All pretrained embedders load `.safetensors` via `gpu::GpuModelLoader` (wraps `candle_core::safetensors`). BERT-family shared weights handled by `gpu::BertWeights`. Each model:

1. `init_gpu()` → CUDA device handle.
2. `Tokenizer::from_file("tokenizer.json")` (HuggingFace `tokenizers` crate).
3. Parse `config.json` → `NomicConfig` / BERT config / Qwen2 config.
4. Load weights into GPU tensors (FP16 per `SingleModelConfig::cuda_fp16()` default).
5. Mark `AtomicBool loaded = true`.

E5 additionally loads `trained/` artifacts via `load_trained_weights(&path)` — absence is a hard error (fail-fast, no silent base-model fallback).

### 10.3 Registry (`models::ModelRegistry`)

Central registry used outside the global singleton for selective loading (tests, tools):

```rust
struct ModelRegistry {
    models:          RwLock<HashMap<ModelId, Arc<dyn EmbeddingModel>>>,
    memory_tracker:  RwLock<MemoryTracker>,
    loading_locks:   HashMap<ModelId, Arc<Semaphore>>,   // per-model, concurrency=1
    factory:         Arc<dyn ModelFactory>,
    stats:           RwLock<RegistryStatsInternal>,
}
```

`MemoryTracker` enforces the `MemoryBudgetExceeded` error using `MEMORY_ESTIMATES` (see table below). Production uses `ProductionMultiArrayProvider` (eager), not `ModelRegistry` (lazy).

### 10.4 Memory estimates (FP32 overestimates)

| Model | Bytes | Notes |
|-------|-------|-------|
| `Semantic` | 1,400,000,000 | e5-large-v2, 335M params |
| `Graph` | 1,400,000,000 | same weights as E1 (VRAM-shared in practice) |
| `Causal` | 650,000,000 | nomic-embed v1.5 + LoRA |
| `Code` | 550,000,000 | Qodo-Embed 1.5B |
| `Sparse` | 550,000,000 | SPLADE BERT |
| `Splade` | 550,000,000 | SPLADE v3 BERT |
| `Contextual` | 500,000,000 | e5-base-v2, 110M params |
| `LateInteraction` | 450,000,000 | ColBERT v2 BERT-base |
| `Kepler` | 350,000,000 | RoBERTa-base + TransE |
| `Entity` (legacy) | 120,000,000 | MiniLM-L6-v2 |
| `Hdc` | 60,000,000 | hypervector RNG tables |
| `TemporalRecent` | 15,000,000 | no weights, buffer only |
| `TemporalPeriodic` | 15,000,000 | no weights, buffer only |
| `TemporalPositional` | 15,000,000 | no weights, buffer only |
| **Total** | **6,625,000,000** | `TOTAL_MEMORY_ESTIMATE` (~6.6 GB FP32) |

`get_memory_estimate(ModelId)` returns 500 MB default and logs `E_EMB_MEM_001` if unlisted.

---

## 11. Batch Processing and GPU Throughput

### 11.1 Batch queue

`batch::BatchQueue<ModelId>` collects `BatchRequest { input, model_id, tx: oneshot::Sender<EmbeddingResult<ModelEmbedding>> }`. Queue flushes when full or deadline hit, producing `Batch` which the adapter runs through `EmbeddingModel::embed_batch` (or single `embed` if not overridden).

`BatchProcessor` (`batch/processor/`) spawns per-model async workers. Config knobs (`BatchProcessorConfig`): `max_batch_size`, `max_wait_ms`, `max_queue_depth`, `worker_timeout_ms`. `BatchProcessorStats` tracks throughput and latency.

### 11.2 GPU module

- `gpu::device::GpuDevice` — wraps `candle_core::Device::cuda_if_available()`; errors if CUDA missing.
- `gpu::memory` — `MemoryPool`, `MemoryBudget`, `MemoryPressure` signals, VRAM slot allocation.
- `gpu::tensor::ops` — shared tensor ops (matmul, softmax, layer norm) used by all transformer forward passes.
- `gpu::model_loader` — unified safetensors loading with per-layer tensor deserialization and GPU placement.

### 11.3 Warm initialization (`warm/`)

The `warm` module is an alternative orchestration path that pre-allocates VRAM with non-evictable `WarmCudaAllocator` slots, validates each loaded model with a `ValidationResult` (dimension, weight checksum, test-inference golden similarity), and exposes a `WarmEmbeddingPipeline`. Used primarily in benchmarks and diagnostics; `ProductionMultiArrayProvider` is the production path.

Validation constants:

```text
GOLDEN_SIMILARITY_THRESHOLD = minimum cosine(test_input, expected_output) for valid model
SIN_WAVE_ENERGY_THRESHOLD   = rejects random-weight outputs (sin-wave signature detect)
MINIMUM_VRAM_BYTES          = abort if GPU reports < this
REQUIRED_COMPUTE_MAJOR = 12, REQUIRED_COMPUTE_MINOR = 0    // Blackwell (RTX 5090)
```

### 11.4 Cache

`cache::EmbeddingCache` wraps `moka::sync::Cache<CacheKey, MultiArrayEmbedding>` with:

- `CacheKey = xxhash64(content_bytes)` via `xxhash-rust`.
- TTL + size eviction from `CacheConfig` (`capacity`, `ttl_seconds`, `eviction_policy`).
- `CacheStats { hits, misses, hit_rate }` via `AtomicU64`.
- Miss returns `None`; population is external (cache is a lookup sidecar, not a wrapper).

---

## 12. Training Subsystem

Binary: `train-causal` (`src/bin/train_causal.rs`). Trains E5 only; other embedders use stock HF weights. See [16_training_subsystem.md](16_training_subsystem.md) for end-to-end training plan.

### 12.1 Pipeline overview

```text
Hermes-2-Pro LLM ──┐
                   ├─> seed + expand training pairs (CausalTrainingPair)
Curated CSV       ─┘       │
                           ▼
                    CausalDataLoader (batches + curriculum filter)
                           │
                           ▼
                ┌─────────────────────────────────┐
                │ CausalTrainingPipeline (3 stages)│
                │  ┌───────────────────────────┐   │
                │  │ Stage 1: projection only   │   │
                │  │   easy pairs, 25 epochs    │   │
                │  ├───────────────────────────┤   │
                │  │ Stage 2: + LoRA activated  │   │
                │  │   all pairs, 20 epochs     │   │
                │  ├───────────────────────────┤   │
                │  │ Stage 3: direction emphasis│   │
                │  │   triplet weight 0.6, 20   │   │
                │  └───────────────────────────┘   │
                │    AdamW + warmup + cosine decay │
                │    InfoNCE + directional + sep + │
                │    soft-label losses             │
                │    Multi-task heads (Stages 2,3) │
                └─────────────────────────────────┘
                           │
                           ▼
                 models/causal/trained/
                 {lora_q, lora_v, projection}.safetensors
```

### 12.2 LoRA (`training/lora.rs`)

```text
LoraConfig default:
    rank:         16
    alpha:        16.0           // scale = alpha / rank = 1.0
    dropout:      0.1
    hidden_size:  768            // NomicBERT
    num_layers:   12
    apply_query:  true
    apply_value:  true
    apply_key:    false

per-adapter params = 2 * hidden_size * rank   = 24,576
adapters_per_layer = apply_q + apply_v + apply_k = 2
total_params       = 24,576 * 2 * 12 = 589,824
```

Per layer, Q and V projections are augmented: `x -> (W + B @ A) x`, where `A: [hidden, rank]` is Kaiming-initialized (quasi-random golden-ratio sequence) and `B: [rank, hidden]` is zero-initialized so LoRA contribution starts at identity. Dropout applied via a `Cell<bool>` training flag (enabled only during training, disabled at inference).

### 12.3 `PipelineConfig`

| Field | Default | Meaning |
|-------|---------|---------|
| `stage1_epochs` | 25 | projection-only warm-up |
| `stage2_epochs` | 20 | + LoRA |
| `stage3_epochs` | 20 | + direction emphasis |
| `stage1_max_difficulty` | 0.2 | curriculum filter for easy pairs |
| `stage3_direction_weight` | 0.6 | multiplier on directional triplet loss |
| `batch_size` | 16 | |
| `eval_every` | 5 | epochs between evaluations |
| `early_stopping_patience` | 10 | |
| `output_dir` | `models/causal/trained` | |
| `seed` | 42 | |
| `multitask_config` | `Some(default)` | direction (3-class) + mechanism (7-class) heads, active Stages 2–3 |

### 12.4 Loss components (`training/loss.rs`)

- **InfoNCE contrastive**: temperature 0.05 default, hard-negative scale 1.0.
- **Directional triplet**: margin-based, enforces cause-closer-to-effect-forward than reverse.
- **Separation loss**: pushes unrelated pairs apart.
- **Soft-label KL**: when LLM provides confidence scores.
- **Multi-task cross-entropy**: direction head (3 classes: Cause, Effect, None) + mechanism head (7 classes: causation, correlation, temporal, spatial, compositional, statistical, unknown).

### 12.5 Optimizer (`training/optimizer.rs`)

AdamW with `ParamGroup` per tensor group (LoRA, projection, multitask heads) — different learning rates per group. Warmup (linear) + cosine decay. Default schedule: warmup 10% of total steps, cosine to 10% of peak LR.

### 12.6 Training data (`training/data.rs`)

`CausalTrainingPair { cause_text, effect_text, direction: DirectionLabel, mechanism: MechanismLabel, difficulty: f32, confidence: f32 }`. Seed pairs via `seed_training_pairs()`; LLM-augmented via `expand_seed_pairs()` (Hermes 2 Pro). Also supports online distillation via `training::distillation` which streams LLM-generated pairs during training.

### 12.7 Evaluation metrics (`training/evaluation.rs`)

`EvaluationMetrics { directional_accuracy, mrr, auc, score_spread, mechanism_accuracy }`. Best-stage selection uses `score_spread` (not `directional_accuracy`) — spread between positive and negative pair scores. Per-stage checkpoints kept so best stage's LoRA pairs with its projection.

---

## 13. Errors and Retries

`EmbeddingError` (`error/types.rs`) is a broad enum with both legacy variants and the SPEC-EMB-001 taxonomy (EMB-E001..EMB-E012). `ErrorSeverity { Critical, High, Medium }`.

| Code | Variant | Severity | Recoverable |
|------|---------|----------|-------------|
| EMB-E001 | `CudaUnavailable` | Critical | No |
| EMB-E002 | `InsufficientVram` | Critical | No |
| EMB-E003 | `WeightsCorrupted` / `InvalidChecksum` | Critical | No |
| EMB-E009 | `RecallLossExceeded` | Medium | Yes |
| — | `ModelNotFound`, `ModelLoadError`, `NotInitialized` | High | No |
| — | `InvalidDimension`, `InvalidValue`, `EmptyInput`, `InputTooLong` | Medium | input fix |
| — | `GpuError`, `BatchError`, `Timeout` | High | retry |
| — | `TokenizationError`, `UnsupportedModality`, `ConfigError` | Medium | config fix |
| — | `MemoryBudgetExceeded` | High | lower concurrency |
| — | `IoError`, `SerializationError`, `CacheError` | varies | retry |

Design principles (enforced by `compile_error!` on missing `candle`, and by fail-fast construction paths):

- **NO FALLBACKS** — `ProductionMultiArrayProvider::new` returns `Err` if any of the 13 models fails to load. `initialize_global_warm_provider()` stores the error in `ProviderState.init_error` and refuses to retry.
- **FAIL FAST** — invalid input shapes, missing tokenizer files, non-finite embedding values all return errors immediately.
- **CONTEXTUAL** — every variant carries identifying fields (`model_id`, `index`, `expected`, `actual`, source chain via `#[source]`).

Retry policy is the caller's responsibility. Only `RecallLossExceeded` (EMB-E009) is classified recoverable; all others require operator intervention or input correction.
