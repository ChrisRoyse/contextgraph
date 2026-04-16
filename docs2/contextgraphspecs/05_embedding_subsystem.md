# Embedding Subsystem

Source files covered:

- `crates/context-graph-embeddings/src/lib.rs`
- `crates/context-graph-embeddings/src/types/model_id/core.rs`
- `crates/context-graph-embeddings/src/types/dimensions/constants.rs`
- `crates/context-graph-embeddings/src/types/dimensions/mod.rs`
- `crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs`
- `crates/context-graph-embeddings/src/traits/model_factory/mod.rs`
- `crates/context-graph-embeddings/src/traits/model_factory/memory.rs`
- `crates/context-graph-embeddings/src/provider/mod.rs`
- `crates/context-graph-embeddings/src/provider/multi_array.rs`
- `crates/context-graph-embeddings/src/models/mod.rs`
- `crates/context-graph-embeddings/src/models/custom/mod.rs`
- `crates/context-graph-embeddings/src/models/custom/temporal_recent/` (model, compute, constants)
- `crates/context-graph-embeddings/src/models/custom/temporal_periodic/` (model, encoding, constants)
- `crates/context-graph-embeddings/src/models/custom/temporal_positional/` (model, encoding, constants)
- `crates/context-graph-embeddings/src/models/custom/hdc/` (model, encoding, types, operations)
- `crates/context-graph-embeddings/src/models/pretrained/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/semantic/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/causal/` (mod, config)
- `crates/context-graph-embeddings/src/models/pretrained/code/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/graph/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/sparse/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/contextual/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/kepler/mod.rs`
- `crates/context-graph-embeddings/src/models/pretrained/late_interaction/mod.rs`
- `crates/context-graph-embeddings/src/cache/mod.rs`
- `crates/context-graph-embeddings/src/batch/processor/mod.rs`
- `crates/context-graph-embeddings/src/batch/processor/config.rs`
- `crates/context-graph-embeddings/src/config/batch.rs`
- `crates/context-graph-embeddings/src/quantization/mod.rs`
- `crates/context-graph-embeddings/src/quantization/pq8/types.rs`
- `crates/context-graph-embeddings/src/quantization/binary.rs`
- `crates/context-graph-embeddings/src/training/mod.rs`
- `crates/context-graph-embeddings/src/training/lora.rs`
- `crates/context-graph-embeddings/src/gpu/mod.rs`
- `crates/context-graph-embeddings/src/adapters/mod.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/mod.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`
- `crates/context-graph-core/src/types/fingerprint/sparse.rs`
- `crates/context-graph-core/src/embeddings/config.rs`
- `crates/context-graph-core/src/embeddings/provider.rs`

---

## 1. Architecture Overview

The embedding subsystem converts text, code, and metadata into a multi-space vector representation called a **SemanticFingerprint** (also aliased as **TeleologicalArray**). This fingerprint stores the output of 13 independent embedding models (E1--E13) without fusion, preserving 100% of information across all semantic spaces. Typical storage is approximately 46 KB per fingerprint.

### 1.1 High-Level Data Flow

```text
Input Text/Code
       |
       v
ProductionMultiArrayProvider
       |
       +---> E1  SemanticModel  (1024D dense)   -- GPU/Candle
       +---> E2  TemporalRecentModel (512D)     -- CPU (sinusoidal PE)
       +---> E3  TemporalPeriodicModel (512D)   -- CPU (Fourier basis)
       +---> E4  TemporalPositionalModel (512D) -- CPU (sinusoidal PE + session sig)
       +---> E5  CausalModel (768D x2)          -- GPU/Candle (dual: cause/effect)
       +---> E6  SparseModel (~30K sparse)      -- GPU/Candle (SPLADE)
       +---> E7  CodeModel (1536D dense)        -- GPU/Candle (Qodo-Embed)
       +---> E8  GraphModel (1024D x2)          -- GPU/Candle (dual: source/target)
       +---> E9  HdcModel (10K-bit -> 1024D)    -- CPU (hypervectors)
       +---> E10 ContextualModel (768D x2)      -- GPU/Candle (dual: paraphrase/context)
       +---> E11 KeplerModel (768D)             -- GPU/Candle (disabled in production)
       +---> E12 LateInteractionModel (128D/token) -- GPU/Candle (ColBERT)
       +---> E13 SpladeModel (~30K sparse)      -- GPU/Candle (SPLADE v3)
       |
       v
SemanticFingerprint  (all 13 stored separately, no fusion)
       |
       v
Per-Space HNSW Indexes  (storage layer: 15 HNSW, including dual asymmetric)
```

See Section 16 for a full breakdown of the core vs storage HNSW index sets.

### 1.2 Design Philosophy

- **No Fusion**: Each embedding space is preserved independently. Fusion occurs only at search/query time via Reciprocal Rank Fusion (RRF) or weighted combination across search strategies.
- **GPU-First**: The `candle` feature is mandatory. The crate targets NVIDIA RTX 5090 (32 GB VRAM, Blackwell sm_120). Dev environment builds against CUDA 13.2 toolkit with `sm_120` PTX target. A compile error is emitted if the `candle` feature is not enabled.
- **Fail Fast, No Fallbacks**: Invalid inputs, dimension mismatches, and initialization failures propagate as typed errors (`EmbeddingError`). No silent degradation.
- **Thread Safety**: All models implement `Send + Sync`. The `EmbeddingModel` trait requires `Send + Sync` bounds. Concurrent embedding is safe via `tokio::join!`.
- **E11 Disabled in Production**: `E11_ENTITY_ENABLED = false` in `crates/context-graph-core/src/weights/mod.rs:45`. `get_effective_weight_profile()` zeros E11 and redistributes weight across other active embedders. `ProductionMultiArrayProvider` also returns a zero vector for E11 without model inference when the flag is false (`crates/context-graph-embeddings/src/provider/multi_array.rs:1676-1678`).
- **Temporal Embedders Enabled**: `TEMPORAL_EMBEDDERS_ENABLED = true` in `crates/context-graph-core/src/weights/mod.rs:40`. E2/E3/E4 are first-class HNSW-indexed fusion participants whenever their weight > 0.0 (e.g., `temporal_navigation`, `sequence_navigation`, `conversation_history`).

### 1.3 Key Crate Layout

| Crate | Purpose |
|-------|---------|
| `context-graph-embeddings` | All model implementations, GPU ops, batch processing, cache, training, quantization |
| `context-graph-core` | Core types: `SemanticFingerprint`, `SparseVector`, `EmbedderConfig`, dimension constants |

---

## 2. The 13 Embedders -- Specification Table

| ID | Name | ModelId Variant | Dimension | Asymmetric | Sparse | Token-Level | Model | Type | Latency Budget | Max Tokens | VRAM Estimate |
|----|------|-----------------|-----------|------------|--------|-------------|-------|------|---------------|------------|---------------|
| E1 | Semantic | `Semantic` | 1024D | No | No | No | intfloat/e5-large-v2 | Pretrained | 5 ms | 512 | 1.3 GB |
| E2 | Temporal Recent | `TemporalRecent` | 512D | No | No | No | Sinusoidal PE / Exponential Decay | Custom | 2 ms | unlimited | 10 MB |
| E3 | Temporal Periodic | `TemporalPeriodic` | 512D | No | No | No | Fourier basis | Custom | 2 ms | unlimited | 10 MB |
| E4 | Temporal Positional | `TemporalPositional` | 512D | No | No | No | Sinusoidal PE + Session Signature | Custom | 2 ms | unlimited | 10 MB |
| E5 | Causal | `Causal` | 768D x2 | Yes | No | No | nomic-ai/nomic-embed-text-v1.5 | Pretrained | 8 ms | 512 | 547 MB |
| E6 | Sparse Lexical | `Sparse` | 30,522 sparse | No | Yes | No | naver/splade-cocondenser | Pretrained | 3 ms | 512 | 440 MB |
| E7 | Code | `Code` | 1536D | No | No | No | Qodo/Qodo-Embed-1-1.5B | Pretrained | 10 ms | 32,768 | 6 GB |
| E8 | Graph | `Graph` | 1024D x2 | Yes | No | No | intfloat/e5-large-v2 | Pretrained | 5 ms | 512 | 1.3 GB |
| E9 | HDC | `Hdc` | 10,000-bit -> 1024D | No | No | No | Hyperdimensional Computing | Custom | 1 ms | unlimited | 50 MB |
| E10 | Contextual Paraphrase | `Contextual` | 768D x2 | Yes | No | No | intfloat/e5-base-v2 | Pretrained | 15 ms | 512 | 440 MB |
| E11 | Entity (KEPLER) | `Kepler` | 768D | No | No | No | KEPLER (RoBERTa + TransE) | Pretrained | 5 ms | 512 | 300 MB |
| E12 | Late Interaction | `LateInteraction` | 128D/token | No | No | Yes | colbert-ir/colbertv2.0 | Pretrained | 8 ms | 512 | 440 MB |
| E13 | SPLADE v3 | `Splade` | 30,522 sparse | No | Yes | No | SPLADE v3 | Pretrained | 3 ms | 512 | 500 MB |

**Total VRAM (all 14 variants, FP32)**: ~6.6 GB. Shared model loading (E1/E8 share e5-large-v2 weights) reduces effective VRAM.

**Note on ModelId**: There are 14 `ModelId` variants (not 13) because both `Entity` (legacy 384D MiniLM) and `Kepler` (production 768D RoBERTa+TransE) occupy the E11 weight slot. `ModelId::production()` returns the 13 production variants (excluding legacy `Entity`). `NUM_EMBEDDERS` = 13.

---

## 3. Detailed Embedder Descriptions

### 3.1 E1 -- Semantic (intfloat/e5-large-v2)

**Purpose**: Primary semantic understanding. Produces dense 1024D vectors that capture the meaning of text for similarity search.

**Model**: e5-large-v2 (335M parameters). BERT-family architecture with 24 encoder layers.

**Algorithm**:
1. Tokenize input using BERT WordPiece tokenizer (max 512 tokens)
2. Prepend instruction prefix: `"query: "` for queries, `"passage: "` for documents
3. Run GPU-accelerated BERT forward pass (embedding lookup, positional encoding, 24 transformer layers)
4. Apply mean pooling over the sequence dimension
5. L2-normalize the output vector

**Input/Output**:
- Input: `ModelInput::Text { content, instruction }`
- Output: `Vec<f32>` of length 1024, L2-normalized

**Distance Metric**: Cosine similarity

**Quantization**: PQ-8 (32 subvectors, 8 bits per code) for 32x compression

### 3.2 E2 -- Temporal Recent (Sinusoidal PE / Exponential Decay)

**Purpose**: Encodes temporal recency so that nearby timestamps produce similar vectors. Enables "find recent memories" queries.

**Model**: Custom (no pretrained weights). Pure mathematical encoding.

**Entry point**: `compute_decay_embedding(timestamp, reference_time, rates)` in `crates/context-graph-embeddings/src/models/custom/temporal_recent/compute.rs:42`. Dual-mode dispatch based on `reference_time`.

**Algorithm (Production -- sinusoidal mode, `reference_time=None`)**:
1. Extract Unix timestamp from input (via instruction field: `"timestamp:ISO8601"` or `"epoch:N"`)
2. Compute transformer-style positional encoding of the absolute timestamp (see `compute_sinusoidal_recency()` at `compute.rs:68`):
   - For each dimension pair (2k, 2k+1) where k in 0..255:
     - `vec[2k] = sin(timestamp / base^(2k/512))`
     - `vec[2k+1] = cos(timestamp / base^(2k/512))`
3. L2-normalize the 512D output

This produces:
- Low indices: fine-grained (second/minute differences)
- High indices: coarse-grained (day/week/month differences)
- Nearby timestamps: high cosine similarity
- Distinct timestamps: distinct vectors (non-degenerate)

**Algorithm (Test mode -- decay, `reference_time=Some(ref)`)**: Computes time delta from reference, applies exponential decay at 4 time scales (hour/day/week/month) with 128 phase-varied cosine features per scale, then L2-normalizes. Used only in legacy tests.

**Decay Rates (default, legacy only)**: `[1/3600, 1/86400, 1/604800, 1/2592000]` (hour, day, week, month reciprocal seconds).

**Rebuild behavior**: E2 is recomputed from `fp.created_at` during index rebuild; persisted E2 HNSW is skipped on startup to force rebuild against the current algorithm.

**Input/Output**:
- Input: `ModelInput::Text` with timestamp in instruction
- Output: `Vec<f32>` of length 512, L2-normalized

**Distance Metric**: Cosine similarity

### 3.3 E3 -- Temporal Periodic (Fourier Basis)

**Purpose**: Encodes cyclical time patterns -- hour-of-day, day-of-week, month-of-year, etc. Captures "morning vs evening" or "weekday vs weekend" patterns.

**Model**: Custom (no pretrained weights). Fourier-basis encoding.

**Algorithm**:
1. Extract Unix timestamp from input (same as E2)
2. For each period P in {hour(3600s), day(86400s), week(604800s), month(2592000s), year(31536000s)}:
   - Compute phase: `theta = 2*pi * (timestamp_secs mod P) / P`
   - For each harmonic n in 1..51:
     - `sin(n * theta)`
     - `cos(n * theta)`
   - This produces 102 features per period (51 harmonics x 2)
3. Concatenate all 5 periods: 5 x 102 = 510 features, pad to 512D with zeros
4. L2-normalize

**Constants**: 5 periods, 51 harmonics per period, 102 features per period, 512D output.

**Input/Output**:
- Input: `ModelInput::Text` with timestamp in instruction
- Output: `Vec<f32>` of length 512, L2-normalized

**Distance Metric**: Cosine similarity

### 3.4 E4 -- Temporal Positional (Sinusoidal PE + Session Signature)

**Purpose**: Encodes session sequence positions to enable "before/after" queries within a session. Combines session identity clustering with within-session ordering.

**Model**: Custom (no pretrained weights). Hybrid sinusoidal encoding.

**Algorithm (Hybrid mode, default)**:
1. Extract session ID and sequence position from instruction:
   - Format: `"session:abc123 sequence:42"` or `"session:abc123 timestamp:ISO8601"`
   - Falls back to sentinel session if no session provided
2. First 256D -- Session Signature:
   - Deterministic hash-based generation from session ID
   - Same session produces identical signatures for clustering
3. Last 256D -- Position Encoding:
   - Transformer-style sinusoidal PE (from "Attention Is All You Need"):
     - `PE(pos, 2i) = sin(pos / base^(2i/256))`
     - `PE(pos, 2i+1) = cos(pos / base^(2i/256))`
   - Base = 10,000 (default); sequence mode uses `base/100` for better consecutive-position differentiation
4. Concatenate session signature (256D) + position encoding (256D) = 512D
5. L2-normalize the combined vector

**Algorithm (Legacy mode, `hybrid_mode=false`)**: Full 512D sinusoidal positional encoding without session signature.

**Input/Output**:
- Input: `ModelInput::Text` with session/sequence in instruction
- Output: `Vec<f32>` of length 512, L2-normalized

**Distance Metric**: Cosine similarity

### 3.5 E5 -- Causal (nomic-ai/nomic-embed-text-v1.5)

**Purpose**: Encodes causal relationships with asymmetric dual vectors. A text is encoded both as a potential cause and as a potential effect. This enables directional causal search: "find effects of X" or "find causes of Y".

**Model**: nomic-embed-text-v1.5. NomicBERT architecture with Rotary Position Embeddings (RoPE), SwiGLU FFN, fused QKV projections, and no biases. 12 layers, 768D hidden, 30,528 vocab. ~547 MB FP32.

**Algorithm**:
1. Tokenize input with BERT tokenizer (max 512 tokens)
2. Prepend causal instruction prefix:
   - As cause: `"search_query: Identify the cause in: "`
   - As effect: `"search_query: Identify the effect of: "`
3. GPU-accelerated forward pass through 12 NomicBERT layers with RoPE attention
4. Mean pooling over sequence dimension
5. Learned projection via trainable `W_cause` and `W_effect` matrices (768x768)
6. L2-normalize both vectors

**Asymmetric Similarity**: For causal search, compare `query_as_cause` against `doc_as_effect` (cause->effect direction, 1.2x boost) or `query_as_effect` against `doc_as_cause` (effect->cause, 0.8x demotion). The causal gate applies thresholds: `CAUSAL_THRESHOLD=0.04`, `NON_CAUSAL_THRESHOLD=0.008`.

**Dual Vector Storage**: The `SemanticFingerprint` stores both `e5_causal_as_cause` (768D) and `e5_causal_as_effect` (768D). A legacy `e5_causal` field exists for backward compatibility.

**Input/Output**:
- Input: `ModelInput::Text` with content
- Output: Two `Vec<f32>` of length 768 each, L2-normalized

**Distance Metric**: Asymmetric Cosine similarity

**Quantization**: PQ-8 (24 subvectors, 8 bits per code). Both cause and effect vectors are quantized independently.

### 3.6 E6 -- Sparse Lexical (SPLADE)

**Purpose**: Produces sparse lexical representations where each dimension corresponds to a vocabulary term. Captures keyword-level matching with learned term expansion.

**Model**: naver/splade-cocondenser-ensembledistil. BERT backbone with Masked Language Model (MLM) head. ~440 MB FP32.

**Algorithm (SPLADE -- Sparse Lexical and Expansion)**:
1. Tokenize input with BERT tokenizer (max 512 tokens)
2. BERT encoder produces contextualized token representations (hidden_size x seq_len)
3. MLM head projects each token to vocabulary dimension (30,522)
4. Log-saturating activation: `log(1 + ReLU(x))` -- this encourages sparsity
5. Max-pooling over all sequence positions (one score per vocabulary term)
6. Output: sparse vector where non-zero entries indicate important terms

**Sparsity**: Typically ~5% of vocabulary positions are active (~1,526 non-zero entries out of 30,522).

**Input/Output**:
- Input: `ModelInput::Text`
- Output: `SparseVector { indices: Vec<u16>, values: Vec<f32> }` -- sorted ascending indices, corresponding activation weights

**Distance Metric**: Jaccard similarity (for index-based retrieval); sparse dot product for scoring

**Quantization**: Inverted index (native sparse storage, no compression needed)

### 3.7 E7 -- Code (Qodo-Embed-1-1.5B)

**Purpose**: Encodes source code and code-related text for code search, function matching, and code similarity.

**Model**: Qodo/Qodo-Embed-1-1.5B. Based on Qwen2 architecture. 1.5 billion parameters, 28 decoder layers, 1536D hidden, 12 attention heads with Grouped-Query Attention (2 KV heads per layer), SwiGLU activation (8960 intermediate size), RoPE (theta=1,000,000). ~6 GB FP32 (~3 GB FP16).

**Algorithm**:
1. Tokenize input with Qwen2 BPE tokenizer (max 32,768 tokens -- 32K context)
2. GPU-accelerated forward pass through 28 decoder layers with GQA and RoPE
3. Last-token pooling for embedding output
4. L2-normalization

**Input/Output**:
- Input: `ModelInput::Code { content, language }` or `ModelInput::Text`
- Output: `Vec<f32>` of length 1536, L2-normalized

**Distance Metric**: Cosine similarity

**Quantization**: PQ-8 (48 subvectors, 8 bits per code)

### 3.8 E8 -- Graph (intfloat/e5-large-v2)

**Purpose**: Encodes graph structure relationships with asymmetric dual vectors. A text is encoded as both a source and a target of graph edges. Enables directional graph queries: "What imports module X?" or "What does module X import?"

**Model**: intfloat/e5-large-v2 (same as E1, shares VRAM). 335M parameters, 1024D output. Asymmetric source/target embeddings via learned projection weights.

**Algorithm**:
1. Tokenize input with BERT tokenizer (max 512 tokens)
2. GPU-accelerated BERT forward pass (same pipeline as E1)
3. Mean pooling over sequence dimension
4. Learned source/target projection matrices produce dual vectors
5. L2-normalize both vectors

**Asymmetric Similarity**: Query-as-source is compared against doc-as-target for outgoing-edge search; query-as-target against doc-as-source for incoming-edge search.

**Dual Vector Storage**: `e8_graph_as_source` (1024D) and `e8_graph_as_target` (1024D). Legacy `e8_graph` field for backward compatibility. `migrate_legacy_e8()` copies legacy to both dual vectors.

**Input/Output**:
- Input: `ModelInput::Text` with optional graph context
- Output: Two `Vec<f32>` of length 1024 each, L2-normalized

**Distance Metric**: Cosine similarity

**Quantization**: Float8 (4x compression)

### 3.9 E9 -- HDC (Hyperdimensional Computing)

**Purpose**: Typo-robust and noise-tolerant text encoding. "authentication" and "authetication" (typo) produce highly similar vectors because encoding uses a bag-of-character-n-grams approach without absolute position binding.

**Model**: Custom (no pretrained weights). 10,000-bit binary hypervectors projected to 1024D dense.

**Algorithm**:
1. Convert input text to a sequence of characters
2. Extract sliding-window character trigrams (default n-gram size = 3)
3. For each n-gram:
   - Generate a character hypervector for each character using deterministic PRNG (LCG seeded with `0xDEAD_BEEF_CAFE_BABE`)
   - Bind characters within the n-gram using XOR with relative position shifts (permutation)
   - This preserves local ordering ("abc" != "cba") but not absolute position
4. Bundle all n-gram hypervectors using majority vote (bitwise threshold voting)
5. Project 10,000-bit hypervector to 1024D float:
   - For each output dimension i, map to a range of bits
   - Count 1-bits in that range: `value = (2 * ones / chunk_size) - 1` (maps to [-1, +1])
6. L2-normalize the 1024D output

**Noise Tolerance**: Because n-grams are bundled position-independently, a single character insertion/deletion only affects 2-3 local n-grams, not the entire vector. ~5% character difference produces ~5% bit difference, yielding high similarity.

**Data Type**: `Hypervector = BitVec<u64, Lsb0>` (from the `bitvec` crate), 10,000 bits.

**Constants**: `HDC_DIMENSION = 10,000` bits, `HDC_PROJECTED_DIMENSION = 1,024`, `DEFAULT_NGRAM_SIZE = 3`, `DEFAULT_SEED = 0xDEAD_BEEF_CAFE_BABE`.

**Input/Output**:
- Input: `ModelInput::Text` or `ModelInput::Code`
- Output: `Vec<f32>` of length 1024, L2-normalized

**Distance Metric**: Cosine similarity (on projected dense vector; Hamming distance used internally on binary hypervectors)

**Quantization**: PQ-8 (32 subvectors, 8 bits per code). Binary quantization also available for the raw hypervectors (32x compression).

### 3.10 E10 -- Contextual Paraphrase (intfloat/e5-base-v2)

**Purpose**: Asymmetric retrieval for paraphrase detection and contextual matching. Encodes text both as expressed intent (paraphrase) and as background context.

**Model**: intfloat/e5-base-v2. 110M parameters, 768D output. 12 BERT encoder layers.

**Algorithm**:
1. Tokenize input with BERT tokenizer (max 512 tokens)
2. Prepend asymmetric prefix:
   - Paraphrase/query side: `"query: "` prefix (captures expressed meaning/intent)
   - Context/document side: `"passage: "` prefix (captures background context)
3. GPU-accelerated BERT forward pass (12 layers)
4. Mean pooling over sequence dimension
5. L2-normalize

**Asymmetry**: E5-base-v2 achieves asymmetry through learned prefix-based encoding, not separate projection matrices. The model was trained to distinguish "query:" from "passage:" at the representation level.

**Dual Vector Storage**: `e10_multimodal_paraphrase` (768D) and `e10_multimodal_as_context` (768D). Unlike E5/E8, E10 has no legacy single-vector field -- it was introduced with dual vectors from the start.

**Input/Output**:
- Input: `ModelInput::Text`
- Output: Two `Vec<f32>` of length 768 each, L2-normalized

**Distance Metric**: Cosine similarity

**Quantization**: PQ-8 (24 subvectors, 8 bits per code)

### 3.11 E11 -- Entity/KEPLER (RoBERTa-base + TransE)

**Purpose**: Knowledge graph entity embeddings where TransE operations (`h + r - t`) are semantically meaningful. Enables relationship inference, entity linking, and knowledge validation.

**Model**: KEPLER (Knowledge Embedding and Pre-training for Language Representation). RoBERTa-base fine-tuned with TransE objective on Wikidata5M (4.8M entities, 20M triples). 125M parameters, 768D output. ~500 MB FP32.

**Note**: **DISABLED by default in production.** `E11_ENTITY_ENABLED = false` in `crates/context-graph-core/src/weights/mod.rs:45`. When disabled:

- `get_effective_weight_profile()` zeros E11's weight and redistributes to remaining active embedders.
- `ProductionMultiArrayProvider` skips model inference for E11 and emits a zero vector (`crates/context-graph-embeddings/src/provider/multi_array.rs:1676-1678`).
- MCP search handlers omit index 10 from `SearchStrategy::MultiSpace` active indices (`crates/context-graph-mcp/src/handlers/tools/memory_tools.rs:1548-1555`).

The legacy `Entity` variant (all-MiniLM-L6-v2, 384D) exists in `ModelId` for backward-compatible serialization (`ModelId::Entity = 10`) but is deprecated.

**Algorithm**:
1. Tokenize input with RoBERTa tokenizer (GPT-2 BPE, max 512 tokens)
2. GPU-accelerated RoBERTa forward pass (12 layers)
3. Mean pooling over sequence dimension
4. L2-normalize

**TransE Operations** (semantically meaningful because model was trained with `||h + r - t||_2` objective):
- `transe_score(h, r, t)` -- Compute score: `-||h + r - t||_2`
- `predict_tail(h, r)` -- Predict tail embedding: `t_hat = h + r`
- `predict_relation(h, t)` -- Predict relation: `r_hat = t - h`

**Score Thresholds**: Valid triples score > -5.0; uncertain in [-10.0, -5.0]; invalid < -10.0.

**Input/Output**:
- Input: `ModelInput::Text`
- Output: `Vec<f32>` of length 768, L2-normalized

**Distance Metric**: Cosine similarity

**Quantization**: Float8

### 3.12 E12 -- Late Interaction (ColBERT v2)

**Purpose**: Token-level fine-grained matching via MaxSim scoring. Preserves per-token information for more expressive retrieval than single-vector models.

**Model**: colbert-ir/colbertv2.0. BERT backbone (12 layers) with linear projection from 768D to 128D per token. ~440 MB FP32.

**Algorithm**:
1. Tokenize input with BERT tokenizer (max 512 tokens)
2. GPU-accelerated BERT forward pass (12 layers, 768D hidden)
3. Linear projection: 768D -> 128D per token
4. L2-normalize each token vector independently

**MaxSim Scoring**:
```
score(Q, D) = SUM_i MAX_j cosine(q_i, d_j)
```
For each query token q_i, find the maximum cosine similarity to any document token d_j, then sum over all query tokens. This allows partial matching -- a query token matches its best document token regardless of position.

**Memory per text** (100 tokens): 100 x 128 x 4 bytes = 51.2 KB.

**Pooled Output**: For fusion in the multi-array pipeline, per-token embeddings can be pooled to a single 128D vector (average pooling or max pooling).

**Input/Output**:
- Input: `ModelInput::Text`
- Output: `Vec<Vec<f32>>` -- variable number of 128D token vectors. Stored in `SemanticFingerprint.e12_late_interaction`.

**Distance Metric**: MaxSim

**Quantization**: Float8

### 3.13 E13 -- SPLADE v3 (Sparse)

**Purpose**: Second sparse lexical embedder for Stage 1 recall in the 5-stage retrieval pipeline. Produces vocabulary-sized sparse vectors similar to E6 but using SPLADE v3.

**Model**: SPLADE v3. BERT-based with vocabulary projection. ~500 MB FP32.

**Algorithm**: Same mechanism as E6 (Section 3.6): BERT encoder -> MLM head -> log-saturating activation -> max-pooling -> sparse output.

**Input/Output**:
- Input: `ModelInput::Text`
- Output: `SparseVector { indices: Vec<u16>, values: Vec<f32> }` (30,522 vocabulary)

**Distance Metric**: Jaccard similarity

**Quantization**: Inverted index (native sparse storage)

---

## 4. SemanticFingerprint Struct

Defined in `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`.

The `SemanticFingerprint` (also type-aliased as `TeleologicalArray`) is the atomic storage unit. It stores all 13 embeddings independently, without fusion.

### 4.1 Fields

```rust
pub struct SemanticFingerprint {
    // Dense embeddings
    pub e1_semantic: Vec<f32>,                // E1: 1024D
    pub e2_temporal_recent: Vec<f32>,         // E2: 512D
    pub e3_temporal_periodic: Vec<f32>,       // E3: 512D
    pub e4_temporal_positional: Vec<f32>,     // E4: 512D

    // E5 Causal -- dual vectors (asymmetric)
    pub e5_causal_as_cause: Vec<f32>,         // E5: 768D (cause side)
    pub e5_causal_as_effect: Vec<f32>,        // E5: 768D (effect side)
    pub e5_causal: Vec<f32>,                  // E5: legacy single vector (deprecated)

    // E6 Sparse
    pub e6_sparse: SparseVector,              // E6: sorted (u16 index, f32 value) pairs

    // Dense embeddings (continued)
    pub e7_code: Vec<f32>,                    // E7: 1536D

    // E8 Graph -- dual vectors (asymmetric)
    pub e8_graph_as_source: Vec<f32>,         // E8: 1024D (source side)
    pub e8_graph_as_target: Vec<f32>,         // E8: 1024D (target side)
    pub e8_graph: Vec<f32>,                   // E8: legacy single vector (deprecated)

    pub e9_hdc: Vec<f32>,                     // E9: 1024D (projected from 10K-bit)

    // E10 Contextual -- dual vectors (asymmetric)
    pub e10_multimodal_paraphrase: Vec<f32>,  // E10: 768D (paraphrase/intent side)
    pub e10_multimodal_as_context: Vec<f32>,  // E10: 768D (context/document side)

    pub e11_entity: Vec<f32>,                 // E11: 768D

    // E12 Late Interaction -- token-level
    pub e12_late_interaction: Vec<Vec<f32>>,   // E12: variable count of 128D token vectors

    // E13 SPLADE v3
    pub e13_splade: SparseVector,             // E13: sorted (u16 index, f32 value) pairs
}
```

### 4.2 Dimension Constants

Defined in `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`:

| Constant | Value | Description |
|----------|-------|-------------|
| `E1_DIM` | 1024 | Semantic |
| `E2_DIM` | 512 | Temporal Recent |
| `E3_DIM` | 512 | Temporal Periodic |
| `E4_DIM` | 512 | Temporal Positional |
| `E5_DIM` | 768 | Causal (per vector) |
| `E6_SPARSE_VOCAB` | 30,522 | Sparse vocabulary size |
| `E7_DIM` | 1536 | Code |
| `E8_DIM` | 1024 | Graph (per vector) |
| `E9_DIM` | 1024 | HDC projected |
| `E10_DIM` | 768 | Contextual (per vector) |
| `E11_DIM` | 768 | Entity/KEPLER |
| `E12_TOKEN_DIM` | 128 | ColBERT per-token |
| `E13_SPLADE_VOCAB` | 30,522 | SPLADE v3 vocabulary |
| `NUM_EMBEDDERS` | 13 | Total embedder count |
| `TOTAL_DENSE_DIMS` | 8,448 | Sum of single-vector dense dims (E1+E2+E3+E4+E5+E7+E8+E9+E10+E11) |

With dual vectors the total is: 8,448 + 768 (E5 effect) + 1024 (E8 target) + 768 (E10 context) = **11,008 dense dimensions**.

### 4.3 Accessing Embeddings

The `get_embedding(idx)` method returns an `EmbeddingSlice` enum:

```rust
pub enum EmbeddingSlice<'a> {
    Dense(&'a [f32]),
    Sparse(&'a SparseVector),
    TokenLevel(&'a [Vec<f32>]),
}
```

Index mapping: 0=E1, 1=E2, 2=E3, 3=E4, 4=E5, 5=E6, 6=E7, 7=E8, 8=E9, 9=E10, 10=E11, 11=E12, 12=E13.

For asymmetric embedders, `get_embedding()` returns the "active" vector (the primary/cause/source/paraphrase side). Dedicated accessors exist for the other side:
- `get_e5_as_cause()`, `get_e5_as_effect()`
- `get_e8_as_source()`, `get_e8_as_target()`
- `get_e10_as_paraphrase()`, `get_e10_as_context()`

### 4.4 Test Utilities

- `SemanticFingerprint::zeroed()` -- All-zero vectors (test-only, compile-gated behind `#[cfg(test)]`). Zero vectors have undefined cosine similarity and should not be used in production.
- `SemanticFingerprint::stub()` -- All components set to 0.1 (test-only). Safe for HNSW insertion since norm is non-zero.
- No `Default` implementation -- deliberately omitted to prevent accidental creation of all-zero fingerprints.

---

## 5. Asymmetric Embedders (E5, E8, E10)

Three embedders produce dual vectors to support directional/asymmetric similarity:

| Embedder | Side A | Side B | Use Case |
|----------|--------|--------|----------|
| E5 Causal | `e5_causal_as_cause` (768D) | `e5_causal_as_effect` (768D) | "Find effects of X" vs "Find causes of Y" |
| E8 Graph | `e8_graph_as_source` (1024D) | `e8_graph_as_target` (1024D) | "What does X import?" vs "What imports X?" |
| E10 Contextual | `e10_multimodal_paraphrase` (768D) | `e10_multimodal_as_context` (768D) | Intent-based retrieval vs context matching |

### 5.1 Storage Layout

Each asymmetric embedder stores two vectors per memory node. The SemanticFingerprint has separate fields for each side. HNSW indexes are created separately for each side (e.g., `E5CausalCause` and `E5CausalEffect` are separate HNSW indexes).

### 5.2 Similarity Computation

For E5 causal search with direction:
- **Cause->Effect** (query is a cause, find effects): Compare `query.e5_causal_as_cause` against `doc.e5_causal_as_effect`. Apply 1.2x boost.
- **Effect->Cause** (query is an effect, find causes): Compare `query.e5_causal_as_effect` against `doc.e5_causal_as_cause`. Apply 0.8x demotion.

### 5.3 Legacy Compatibility

E5 and E8 have legacy single-vector fields (`e5_causal`, `e8_graph`) populated during deserialization of old fingerprints. The `e5_active_vector()` and `e8_active_vector()` methods return the dual-vector side if populated, falling back to the legacy field. `migrate_legacy_e8()` copies the legacy vector to both source and target.

E10 has no legacy field -- it was introduced with dual vectors.

---

## 6. Sparse Vector Representation (E6, E13)

Defined in `crates/context-graph-core/src/types/fingerprint/sparse.rs`.

### 6.1 SparseVector Format

```rust
pub struct SparseVector {
    pub indices: Vec<u16>,   // Sorted ascending, unique, in [0, 30521]
    pub values: Vec<f32>,    // Corresponding activation weights
}
```

**Invariants enforced by `SparseVector::new()`**:
1. `indices.len() == values.len()`
2. All indices < 30,522 (`SPARSE_VOCAB_SIZE`)
3. Indices sorted in ascending order
4. No duplicate indices

**Typical Size**: ~1,500 active entries (~5% sparsity). Memory: `nnz * 6` bytes (2 bytes per u16 index + 4 bytes per f32 value). A typical vector uses ~9 KB.

### 6.2 Operations

| Operation | Complexity | Description |
|-----------|-----------|-------------|
| `dot(other)` | O(n + m) | Sparse dot product via two-pointer merge-join on sorted indices |
| `cosine_similarity(other)` | O(n + m) | Dot product divided by product of L2 norms |
| `jaccard_similarity(other)` | O(n + m) | Index-overlap based: \|A intersect B\| / \|A union B\| |
| `get(vocab_index)` | O(log n) | Binary search for value at specific vocabulary index |
| `l2_norm()` | O(n) | Square root of sum of squared values |
| `to_dense(dim)` | O(dim) | Convert to dense vector (allocates ~120 KB for full vocabulary) |
| `sparsity()` | O(1) | `1.0 - (nnz / 30522)` |
| `memory_size()` | O(1) | `nnz * 6` bytes |

---

## 7. ColBERT Late Interaction (E12)

### 7.1 Token-Level Embeddings

E12 produces a variable number of 128D vectors, one per input token. Stored as `Vec<Vec<f32>>` in `SemanticFingerprint.e12_late_interaction`.

### 7.2 MaxSim Scoring

```
score(Q, D) = SUM_{i in Q_tokens} MAX_{j in D_tokens} cosine(q_i, d_j)
```

Steps:
1. For each query token vector q_i (128D):
   - Compute cosine similarity with every document token vector d_j
   - Take the maximum similarity across all document tokens
2. Sum all per-query-token maximums
3. Result: a scalar relevance score

This enables precise partial matching -- each query token independently finds its best match in the document.

### 7.3 Pooled Representation

For multi-array storage and HNSW indexing, E12 token vectors can be pooled to a single 128D vector. The pooled vector participates in the standard fusion pipeline. Full token-level MaxSim scoring is used for re-ranking.

---

## 8. Model Loading and GPU Acceleration

### 8.1 Candle Framework

All pretrained models use the Candle deep learning framework (HuggingFace) for GPU-accelerated inference. The `candle` feature is mandatory -- the crate emits a compile error without it.

Target hardware: NVIDIA RTX 5090 (32 GB VRAM, CUDA 13.1, Blackwell GB202).

### 8.2 GPU Module (`crates/context-graph-embeddings/src/gpu/`)

| Component | Description |
|-----------|-------------|
| `GpuDevice` | Singleton device manager with automatic CUDA detection |
| `GpuTensor` | Type-safe tensor wrapper with automatic device placement |
| `ops` | GPU-accelerated operations: L2 normalization, cosine similarity, matmul |
| `memory/VramTracker` | VRAM usage tracking to prevent OOM |
| `memory/GpuMemoryPool` | Memory pool for tensor reuse |
| `memory/ModelSlotManager` | Per-model VRAM slot allocation |
| `model_loader/UnifiedModelLoader` | Loads safetensors weights from disk to GPU |

### 8.3 Model Loading Pipeline

1. `ModelFactory::create_model(model_id, config)` creates a model instance
2. `model.load()` loads pretrained weights from safetensors files:
   - Custom models (E2-E4, E9): No-op (no weights to load)
   - Pretrained models: Load via `UnifiedModelLoader` -> GPU memory
3. `GpuDevice::init_gpu()` initializes CUDA context
4. Weights are placed on GPU via `DevicePlacement` (CPU, CUDA, or Auto)

### 8.4 Memory Estimates

| Model | VRAM (FP32) |
|-------|-------------|
| E1 Semantic (e5-large-v2) | 1.3 GB |
| E2 Temporal Recent | 10 MB |
| E3 Temporal Periodic | 10 MB |
| E4 Temporal Positional | 10 MB |
| E5 Causal (nomic-embed) | 600 MB |
| E6 Sparse (SPLADE) | 500 MB |
| E7 Code (Qodo-Embed) | 500 MB |
| E8 Graph (e5-large-v2) | 1.3 GB |
| E9 HDC | 50 MB |
| E10 Contextual (e5-base-v2) | 440 MB |
| E11 Entity (legacy MiniLM) | 100 MB |
| E11 Kepler (RoBERTa+TransE) | 300 MB |
| E12 Late Interaction (ColBERT) | 400 MB |
| E13 SPLADE v3 | 500 MB |
| **Total (all 14 variants)** | **~6.6 GB** |

E1 and E8 share e5-large-v2 weights, reducing effective VRAM. With FP16 quantization, total is roughly halved.

### 8.5 Model Registry

`ModelRegistry` manages the lifecycle of all models:

```text
ModelRegistry
  +-- models: RwLock<HashMap<ModelId, Arc<dyn EmbeddingModel>>>
  +-- memory_tracker: RwLock<MemoryTracker>
  +-- loading_locks: HashMap<ModelId, Arc<Semaphore>>  (per-model)
  +-- factory: Arc<dyn ModelFactory>
  +-- stats: RwLock<RegistryStatsInternal>
```

Models are lazy-loaded on first access. Per-model semaphores prevent concurrent loading of the same model.

---

## 9. Batch Processing Pipeline

### 9.1 Architecture

```text
BatchProcessor
  +-- Worker Task
  |     +-- request_rx  --> Per-Model Queues
  |     +-- should_flush() check
  |     +-- Semaphore (max_concurrent_batches)
  |     +-- process_batch(batch, registry)
  |     +-- batch.complete(results)
  +-- request_tx (MPSC channel)
```

### 9.2 Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_batch_size` | 32 | Maximum items per GPU batch |
| `min_batch_size` | 1 | Minimum items before flushing |
| `max_wait_ms` | 50 | Maximum wait before forced flush |
| `max_concurrent_batches` | 4 | Concurrent batches across all models |
| `request_buffer_size` | 1000 | MPSC channel buffer |
| `poll_interval_ms` | 10 | Queue check interval |
| `dynamic_batching` | true | Adaptive batch sizing |
| `sort_by_length` | true | Sort sequences for padding efficiency |
| `padding_strategy` | `DynamicMax` | Pad to longest in batch |

### 9.3 Padding Strategies

| Strategy | Description |
|----------|-------------|
| `MaxLength` | Pad all to model's max_tokens (most memory, safest) |
| `DynamicMax` | Pad to longest in current batch (default, most efficient) |
| `PowerOfTwo` | Pad to next power of 2 (GPU cache-friendly) |
| `Bucket` | Predefined buckets: 64, 128, 256, 512 |

### 9.4 Performance Targets

- Single content: <30 ms for all 13 embeddings
- Batch (64 items): <100 ms per item average

---

## 10. LoRA Training Support

### 10.1 Architecture

The training subsystem focuses on fine-tuning the E5 causal embedder using LLM-supervised contrastive learning:

```text
LLM (Hermes 2 Pro Mistral-7B) --> Training Pairs --> DataLoader
                                                          |
                                                    +-----------+
                                                    |  Trainer  |
                                                    |  - InfoNCE Loss
                                                    |  - Directional Loss
                                                    |  - Separation Loss
                                                    |  - Soft Label Loss
                                                    |  - AdamW Optimizer
                                                    +-----------+
                                                          |
                                              W_cause, W_effect (trained)
```

### 10.2 LoRA Configuration

LoRA (Low-Rank Adaptation) adapters are applied to NomicBERT attention projections:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `rank` | 16 | Low-rank decomposition rank |
| `alpha` | 16.0 | Scaling factor (scale = alpha/rank = 1.0) |
| `dropout` | 0.1 | Dropout rate during training |
| `hidden_size` | 768 | NomicBERT hidden dimension |
| `num_layers` | 12 | Number of encoder layers |
| `apply_query` | true | LoRA on Q projections |
| `apply_value` | true | LoRA on V projections |
| `apply_key` | false | LoRA on K projections (optional) |

**Total trainable parameters**: 589,824 (2 adapters x 12 layers x 2 matrices x 768 x 16).

### 10.3 LoRA Adapter Structure

For each attention layer:
```
x -> [W_q + B_q * A_q] -> query   (original + low-rank update)
x -> [W_v + B_v * A_v] -> value   (original + low-rank update)
```

- A (down-projection): [hidden_size, rank] -- initialized with Kaiming (golden-ratio quasi-random)
- B (up-projection): [rank, hidden_size] -- initialized to zero (identity at start)

### 10.4 Training Modules

| Module | Purpose |
|--------|---------|
| `data` | Training pair structures and data loading |
| `loss` | InfoNCE + directional + separation + soft label losses |
| `optimizer` | AdamW with warmup + cosine decay schedule |
| `trainer` | Training loop with momentum encoder |
| `evaluation` | Directional accuracy, MRR, AUC metrics |
| `distillation` | Online LLM-to-embedder teaching loop |
| `lora` | LoRA adapters for NomicBERT Q/V attention |
| `multitask` | Direction classification + mechanism prediction heads |

---

## 11. Quantization

### 11.1 Quantization Methods

| Method | Embedders | Compression | Max Recall Loss | Description |
|--------|-----------|-------------|-----------------|-------------|
| PQ-8 | E1, E5, E7, E9, E10 | 32x | <5% | Product Quantization with 8-bit codes |
| Float8 | E2, E3, E4, E8, E11, E12 | 4x | <0.3% | 8-bit floating point |
| Binary | E9 (raw) | 32x | 5-10% | Sign-based binarization for hypervectors |
| Inverted | E6, E13 | native | 0% | Sparse inverted index (no compression) |

### 11.2 PQ-8 (Product Quantization)

Algorithm:
1. Split embedding into 8 subvectors (`NUM_SUBVECTORS = 8`)
2. For each subvector, train a codebook of 256 centroids (`NUM_CENTROIDS = 256`) using k-means++
3. Encode: assign each subvector to its nearest centroid (1 byte per subvector)
4. Compressed output: 8 bytes per embedding regardless of original dimension
5. Decode: look up centroid vectors from codebook and concatenate

K-Means training config: max 100 iterations, convergence threshold 1e-6, deterministic via LCG seed.

Important: PQ-8 stores actual min/max per codebook (not hardcoded [-1,1]) and quantizes both asymmetric vectors independently.

### 11.3 Binary Quantization (E9 raw hypervectors)

Algorithm:
1. Binarize: `bit = 1 if value >= threshold else 0` (default threshold: 0.0, sign-based)
2. Pack 8 bits per byte (MSB first)
3. Result: 32x compression (8 f32 values -> 1 byte)
4. Similarity: Hamming distance on packed bytes

### 11.4 Float8 (E4M3 format)

4x compression by storing each f32 as an 8-bit float with 4 exponent bits and 3 mantissa bits.

### 11.5 Quantization Router

`QuantizationRouter` selects the appropriate quantization method based on the embedder's `EmbedderConfig.quantization` setting (see Section 2 table). The router dispatches to `BinaryEncoder`, `Float8Encoder`, or `PQ8Encoder` accordingly.

---

## 12. Embedding Cache

Defined in `crates/context-graph-embeddings/src/cache/mod.rs`.

### 12.1 Architecture

```rust
pub struct EmbeddingCache {
    inner: Cache<CacheKey, Arc<MultiArrayEmbedding>>,  // moka concurrent cache
    config: CacheConfig,
    stats: Arc<CacheStats>,
    enabled: bool,
}
```

### 12.2 Cache Key

```rust
pub struct CacheKey(pub u64);  // xxhash64 of input content
```

Keys are computed via `CacheKey::from_content(text)` using the xxhash-rust crate.

### 12.3 Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enabled` | true | Enable/disable caching |
| `max_entries` | configurable | Maximum cache entries |
| `max_bytes` | configurable | Maximum cache memory |
| `ttl_seconds` | optional | Time-to-live per entry |
| `eviction_policy` | configurable | LRU-based via moka |

### 12.4 Operations

| Operation | Target Latency | Description |
|-----------|---------------|-------------|
| `get(key)` | <100 us | Cache lookup, returns `Option<Arc<MultiArrayEmbedding>>` |
| `insert(key, embedding)` | <500 us | Store embedding (wrapped in Arc) |
| `get_or_insert_with(key, factory)` | -- | Atomic get-or-compute pattern |
| `remove(key)` | -- | Invalidate single entry |
| `clear()` | -- | Invalidate all entries |

### 12.5 Statistics

Thread-safe atomic counters track hits, misses, and insertions. `hit_ratio()` returns `hits / (hits + misses)`.

### 12.6 Memory Estimation

Each cached `MultiArrayEmbedding` is approximately 30-80 KB. The weigher function computes actual memory per entry for moka's weight-based eviction.

---

## 13. Provider Trait and Implementations

### 13.1 EmbeddingProvider Trait

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> EmbeddingResult<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
    fn max_tokens(&self) -> usize;
}
```

This trait is for single-model embedding providers.

### 13.2 EmbeddingModel Trait

The core trait all 13 models implement:

```rust
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    fn model_id(&self) -> ModelId;
    fn supported_input_types(&self) -> &[InputType];
    fn is_initialized(&self) -> bool;
    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding>;
    async fn load(&self) -> EmbeddingResult<()>;  // default no-op for custom
    async fn embed_sparse(&self, input: &ModelInput) -> EmbeddingResult<(Vec<u16>, Vec<f32>)>;
    // ... convenience methods: dimension(), projected_dimension(), max_tokens(), etc.
}
```

### 13.3 ProductionMultiArrayProvider

Orchestrates all 13 embedders to produce a `SemanticFingerprint`:

```text
ProductionMultiArrayProvider
  +-- 10 DenseEmbedderAdapter instances (E1-E5, E7-E11)
  +-- 2  SparseEmbedderAdapter instances (E6, E13)
  +-- 1  TokenEmbedderAdapter (E12 ColBERT)

  Returns: SemanticFingerprint with all 13 embeddings
```

Key methods:
- `embed_all(text) -> MultiArrayEmbeddingOutput` -- runs all 13 embedders concurrently via `tokio::join!`
- `embed_batch_all(texts) -> Vec<Output>` -- batch embedding for multiple texts
- `health_status() -> [bool; 13]` -- per-embedder health check
- `is_ready() -> bool` -- all models initialized

### 13.4 ModelFactory

```rust
pub trait ModelFactory: Send + Sync {
    fn create_model(&self, model_id: ModelId, config: &SingleModelConfig)
        -> EmbeddingResult<Box<dyn EmbeddingModel>>;
}
```

`DefaultModelFactory` creates the appropriate model implementation for each `ModelId`. `SingleModelConfig` specifies `DevicePlacement` (CPU/CUDA/Auto) and `QuantizationMode` (None/Int8/FP16/BF16).

### 13.5 ModelId Enum

14 variants (13 production + 1 legacy), as defined in `crates/context-graph-embeddings/src/types/model_id/core.rs`:

| Variant | repr(u8) | Production | Notes |
|---------|----------|------------|-------|
| `Semantic` | 0 | Yes | E1 (1024D) |
| `TemporalRecent` | 1 | Yes | E2 (512D) |
| `TemporalPeriodic` | 2 | Yes | E3 (512D) |
| `TemporalPositional` | 3 | Yes | E4 (512D) |
| `Causal` | 4 | Yes | E5 (768D, dual cause/effect) |
| `Sparse` | 5 | Yes | E6 (30,522 sparse vocab) |
| `Code` | 6 | Yes | E7 (1536D native, Qodo-Embed-1-1.5B) |
| `Graph` | 7 | Yes | E8 (1024D, dual source/target) |
| `Hdc` | 8 | Yes | E9 (10,000-bit -> 1024D projected) |
| `Contextual` | 9 | Yes | E10 (768D, dual paraphrase/context), `#[serde(alias = "Multimodal")]` |
| `Entity` | 10 | No | Legacy E11 (384D MiniLM, deprecated) |
| `LateInteraction` | 11 | Yes | E12 (128D/token) |
| `Splade` | 12 | Yes | E13 (30,522 sparse vocab) |
| `Kepler` | 13 | Yes | Production E11 (768D RoBERTa+TransE, **DISABLED** by default) |

`ModelId::all()` returns 14 variants (`core.rs:194`); `ModelId::production()` returns the 13 production variants excluding `Entity` (`core.rs:225`). Both `Entity` and `Kepler` share the E11 weight slot -- only one is live at a time.

---

## 14. Embedder Categories

Each embedder belongs to one of four categories, defined in `crates/context-graph-core/src/embeddings/category.rs`:

| Category | Embedders | Topic Weight | Description |
|----------|-----------|-------------|-------------|
| Semantic | E1, E5, E6, E7, E10, E12, E13 | 1.0 | Content meaning |
| Temporal | E2, E3, E4 | 0.0 | Time-based (excluded from topic detection) |
| Relational | E8, E11 | 0.5 | Graph/entity relationships |
| Structural | E9 | 0.5 | Noise-robust structure |

---

## 15. Embedder Configuration Registry

Static configuration for all 13 embedders is defined in `EMBEDDER_CONFIGS` (in `crates/context-graph-core/src/embeddings/config.rs`). Each entry specifies:

| Field | Description |
|-------|-------------|
| `embedder` | Embedder enum variant |
| `dimension` | Output dimension (vocab size for sparse, per-token dim for E12) |
| `distance_metric` | Cosine, AsymmetricCosine, Jaccard, or MaxSim |
| `quantization` | PQ8, Float8, Binary, Inverted, or None |
| `is_asymmetric` | True for E5, E8, E10 |
| `is_sparse` | True for E6, E13 |
| `is_token_level` | True for E12 |

Lookup is O(1) via embedder index: `get_config(embedder) -> &'static EmbedderConfig`.

---

## 16. HNSW Index Topology

Two `EmbedderIndex` enums coexist in the workspace:

- `crates/context-graph-core/src/index/config.rs:108` -- **core** enum with 14 variants; exactly **11 HNSW** returned by `all_hnsw()` (E6, E12, E13 excluded).
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs:37` -- **storage** enum with **18 variants** (extends the core set with `E5CausalCause`, `E5CausalEffect`, `E10MultimodalParaphrase`, `E10MultimodalContext`). Of these, **15 use HNSW** (all except E6, E12, E13).

### 16.1 Storage Layer (15 HNSW indexes)

| Index | Embedder | Dimension | Notes |
|-------|----------|-----------|-------|
| E1Semantic | Semantic | 1024D | Primary semantic search |
| E1Matryoshka128 | Semantic | 128D | Truncated matryoshka variant (Stage 2) |
| E2TemporalRecent | Temporal Recent | 512D | Recency search (first-class when weight > 0) |
| E3TemporalPeriodic | Temporal Periodic | 512D | Cyclical pattern search (first-class when weight > 0) |
| E4TemporalPositional | Temporal Positional | 512D | Session/position search (first-class when weight > 0) |
| E5Causal | Causal (legacy) | 768D | Active-vector fallback index |
| E5CausalCause | Causal | 768D | Cause-side index (ARCH-15) |
| E5CausalEffect | Causal | 768D | Effect-side index (ARCH-15) |
| E7Code | Code | 1536D | Code search |
| E8Graph | Graph (symmetric) | 1024D | Active-vector graph search |
| E9HDC | HDC | 1024D | Typo-robust search |
| E10Multimodal | Contextual (legacy) | 768D | Active-vector fallback index |
| E10MultimodalParaphrase | Contextual | 768D | Paraphrase-side index (ARCH-15) |
| E10MultimodalContext | Contextual | 768D | Context-side index (ARCH-15) |
| E11Entity | Entity/KEPLER | 768D | Entity search (disabled by default) |

E6, E12, and E13 do not use HNSW indexes: E6/E13 use inverted indexes (sparse); E12 uses MaxSim scoring.

### 16.2 Core Layer (`HnswMultiSpaceIndex`, 11 HNSW + 1 SPLADE)

The `context-graph-core` `HnswMultiSpaceIndex` is documented as "12 HNSW indexes + SPLADE" (see `crates/context-graph-core/src/index/hnsw_impl/multi_space.rs:15`) but `EmbedderIndex::all_hnsw()` returns exactly **11** (E1, E1Matryoshka128, E2-E5, E7-E11). The "12 HNSW" comment counts SPLADE/E13 as an index slot even though it is sparse. Dual asymmetric indexes (E5CausalCause, E5CausalEffect, E10MultimodalParaphrase, E10MultimodalContext, E8 dual) live in the storage layer, not the core layer.

---

## 17. Training Export Bundle

Context Graph can export a per-memory training record that packages all 13 embedding vectors plus teleological fusion features into a single binary payload suitable for offline ML training. The export is triggered via the `export_training_corpus` MCP tool (handler in `crates/context-graph-mcp/src/handlers/tools/training_tools.rs:62`).

### 17.1 Module Layout (`crates/context-graph-core/src/training/`)

| File | Purpose |
|------|---------|
| `mod.rs` | `TrainingRecord` v2 struct, `TrainingEdge`, `KnnNeighbor`, `CausalLabel`, `TopicMembership`; constants `TRAINING_RECORD_VERSION: u8 = 1`, `NUM_CROSS_CORRELATIONS = 78`, `NUM_GROUP_ALIGNMENTS = 6` |
| `computation.rs` | `topic_profile_or_fallback`, `compute_cross_correlations` (C(13,2)=78 synergy-weighted pairs), `compute_group_alignments` |
| `temporal.rs` | `extract_temporal_labels`, `TemporalLabels`, `PeriodicBucket` (8-variant enum) |
| `tests.rs` | Group-alignment math identities, fallback profile coverage |

### 17.2 `TrainingRecord` Structure (`training/mod.rs:69-146`)

Each record is a single bincode-serializable struct containing:

| Section | Fields |
|---------|--------|
| Identity | `memory_id` (Uuid), `content` (String), `importance` (f32), `created_at`, `session_id` |
| Provenance | `source_type`, `source_path`, `content_hash: Option<[u8; 32]>` (SHA-256) |
| Dense embeddings | `e1_semantic` (1024D), `e2_temporal_recent` (512D), `e3_temporal_periodic` (512D), `e4_temporal_positional` (512D), `e5_causal_cause` + `e5_causal_effect` (768D each), `e7_code` (1536D), `e8_graph_source` + `e8_graph_target` (1024D each), `e9_hdc` (1024D), `e10_paraphrase` + `e10_context` (768D each), `e11_entity` (768D) |
| Sparse embeddings | `e6_sparse_indices`/`e6_sparse_values`, `e13_splade_indices`/`e13_splade_values` |
| Token-level | `e12_token_embeddings: Vec<Vec<f32>>` (variable token count x 128D per token) |
| Teleological fusion | `topic_profile: [f32; 13]`, `cross_correlations: Vec<f32>` (len=78), `group_alignments: [f32; 6]` |
| Graph structure | `outgoing_edges`, `incoming_edges` (both `Vec<TrainingEdge>`), `knn_neighbors: Vec<Vec<KnnNeighbor>>` (13 outer vecs) |
| Causal labels | `causal_effects`, `causal_causes` (both `Vec<CausalLabel>`) |
| Cluster | `topic_memberships: Vec<TopicMembership>` |
| Temporal labels | `temporal_labels: Option<TemporalLabels>` |
| Tucker-core | `tucker_core: Option<TuckerCore>` (Phase-4 hook, always `None` in Phase-1 export) |

Asymmetric embedders (E5, E8, E10) ship **both** vectors; the legacy single-vector fields from `SemanticFingerprint` (Section 4) are not replicated. E11 ships its vector when populated; under the default `E11_ENTITY_ENABLED = false` the slot is empty.

Empty `Vec` values signal "not exported" when the caller passes `includeEmbeddings=false`, `includeSparseVectors=false`, or `includeTokenEmbeddings=false` to the MCP tool (`training_tools.rs:85-96`).

### 17.3 On-Disk Format

Records are persisted to RocksDB column family `CF_TRAINING_RECORDS` (defined in `crates/context-graph-storage/src/teleological/column_families.rs:399`). Encoding is `[TRAINING_RECORD_VERSION: u8][bincode]` per `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs` (see `encode_training_record` at line 284). The version byte is checked on deserialization; mismatches produce `CoreError::SerializationError` with no automatic migration.

Constitution rule: positional bincode format, `skip_serializing_if` is prohibited, and new fields append to the end only (per `training/mod.rs:67-68`).

### 17.4 Cross-Correlations and Group Alignments

`compute_cross_correlations` (`training/computation.rs:118`) produces 78 synergy-weighted pairwise products in canonical pair order (0,1), (0,2), ..., (11,12):

```text
cross_corr[(i,j)] = clamp(topic_profile[i], 0, 1)
                  * clamp(topic_profile[j], 0, 1)
                  * clamp(synergy_matrix.get_synergy(i, j), 0, 1)
```

`compute_group_alignments` (`training/computation.rs:141`) delegates to `GroupAlignments::from_alignments` in `crates/context-graph-core/src/teleological/groups.rs:74`. Group indices (from `groups.rs:53-63`):

| Group | Indices | Embedders | Average identity (tests verify) |
|-------|---------|-----------|---------------------------------|
| Factual (0) | [0, 11, 12] | E1, E12, E13 | `Factual = avg(e1, e12, e13)` (tests.rs:137) |
| Temporal (1) | [1, 2] | E2, E3 | `avg(e2, e3)` |
| Causal (2) | [3, 6] | E4, E7 | `avg(e4, e7)` |
| Relational (3) | [4, 7, 8] | E5, E8, E9 | `avg(e5, e8, e9)` |
| Qualitative (4) | [9, 10] | E10, E11 | With E11 disabled (index 10 = 0), `Qualitative = E10/2` (tests.rs:121-130) |
| Implementation (5) | [5] | E6 | `Implementation = topic_profile[5]` (tests.rs:106-115) |

The docstring in `training/computation.rs:137-140` captures this grouping.

### 17.5 Topic Profile Resolution

`topic_profile_or_fallback` (`training/computation.rs:41`) prefers the stored 13D profile from `CF_TOPIC_PROFILES` when available. Fallback derivation (when no row exists) is a per-embedder presence indicator clamped to [0, 1]:

- Dense vectors (E1/E2/E3/E4/E5_cause/E7/E8_source/E9/E10_paraphrase/E11): `l2_norm.clamp(0, 1)`.
- Sparse vectors (E6/E13): `sqrt(sum_sq).clamp(0, 1)`.
- Token-level (E12): mean of non-zero per-token dense alignments.

For asymmetric embedders the cause/source/paraphrase side is used.

### 17.6 Temporal Labels (`training/temporal.rs:60-138`)

`extract_temporal_labels` derives per-memory time features from `fingerprint.created_at`, an optional `SourceMetadata`, the session sequence/total (when tracked), and a caller-provided `export_now` snapshot used uniformly across a single run.

Fields on `TemporalLabels`:

| Field | Type | Source |
|-------|------|--------|
| `stored_at` | `DateTime<Utc>` | `fingerprint.created_at` |
| `stored_hour_utc` | u8 (0-23) | UTC hour |
| `stored_day_of_week` | u8 (1=Mon..7=Sun) | ISO weekday |
| `stored_month` | u8 (1-12) | UTC month |
| `age_seconds_at_export` | i64 | `export_now - stored_at` |
| `session_sequence` | `Option<u32>` | Caller |
| `session_total` | `Option<u32>` | Caller |
| `relative_position` | `Option<f32>` | `session_sequence / session_total` when both known |
| `periodic_bucket` | `PeriodicBucket` | 8 variants: Weekday/Weekend x Morning(5-11)/Afternoon(12-16)/Evening(17-20)/Night |
| `e2_recency_norm` | f32 | L2 norm of `e2_temporal_recent` (0.0 when empty) |
| `e3_periodic_norm` | f32 | L2 norm of `e3_temporal_periodic` |
| `e4_positional_norm` | f32 | L2 norm of `e4_temporal_positional` |

`PeriodicBucket` boundaries are applied directly to the UTC hour (no local-TZ conversion); see `temporal.rs:16-50`.

### 17.7 Export Flags

The MCP tool (`training_tools.rs:85-120`) accepts per-section flags (defaults shown):

| Flag | Default | Effect when `false` |
|------|---------|---------------------|
| `includeEmbeddings` | true | E1/E2/E3/E4/E5(both)/E7/E8(both)/E9/E10(both)/E11 ship as empty `Vec` |
| `includeSparseVectors` | true | E6/E13 indices+values ship empty |
| `includeTokenEmbeddings` | false | `e12_token_embeddings` ships empty |
| `includeEdges` | true | Outgoing edges suppressed |
| `includeIncomingEdges` | false | Incoming edges suppressed |
| `includeCausalLabels` | true | `causal_effects`/`causal_causes` ship empty |
| `includeTemporalLabels` | true | `temporal_labels: None` |
| `includeTuckerCore` | false | `tucker_core: None` |
| `clearExisting` | false | Pre-existing records in CF are truncated before re-export |

Output is binary-only: the MCP response is a count+bytes+timing summary; no JSON/JSONL/Parquet is emitted.

### 17.8 Constellation Compiler (Phase 2)

The parallel `crates/context-graph-core/src/constellation/` module aggregates multiple memories into a single compiled summary for downstream clustering/classification. It shares the training-records dependency graph (synergy matrix, topic profiles) but is separate from the per-memory export bundle described above.

| File | Purpose |
|------|---------|
| `mod.rs` | `CONSTELLATION_VERSION: u8 = 1`, `RESERVOIR_SAMPLE_SIZE = 1024`, `MIN_CONSTELLATION_MEMBERS = 3` |
| `types.rs` | `Constellation`, `ConstellationSelector`, `EmbedderStats`, `VectorKind`, `ConstellationScoringResult` |
| `welford.rs` | Welford online mean+variance (`WelfordStats`, `WelfordVector`) |
| `reservoir.rs` | `ReservoirSample` bounded-memory percentile estimator |
| `compiler.rs` | `ConstellationAccumulator`, `compile_constellation`, `score_memory_against_constellation` |

Constellations persist to `CF_CONSTELLATIONS` (column family definition `column_families.rs:418`) with secondary index `CF_CONSTELLATION_BY_SELECTOR` (line 436) for O(1) selector lookup.

---

## 18. Cross-References

- **Search strategies** (multi_space, pipeline, embedder_first, e1_only): See `docs2/contextgraphspecs/06_search_and_fusion.md`
- **Causal discovery** (LLM-based): Uses Hermes-2-Pro-Mistral-7B for causal relationship extraction. See causal discovery documentation.
- **Graph discovery**: Uses E8 graph embeddings for knowledge graph link prediction and edge typing.
- **Weight profiles**: Customizable per-embedder weights control fusion behavior. See weight profiles documentation.
- **RocksDB storage**: SemanticFingerprints are serialized to RocksDB column families. See storage subsystem documentation.
