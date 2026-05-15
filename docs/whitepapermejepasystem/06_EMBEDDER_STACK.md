# 06 — The Embedder Stack

The predictor side of the JEPA architecture sees code through 12 different lenses. Each lens (embedder) captures a different facet of the input. Cross-attention between them is where the predictive signal lives. This doc describes the stack, why each embedder is in it, and how they compose.

## Why twelve

Reasonable people would design this with two or three embedders. The reason we have twelve:

1. **No single embedder captures all of code.** A semantic embedder (e5-large-v2) learns natural-language paraphrase but flattens syntax. A code-specific embedder (Qodo-Embed) learns API surface but misses temporal context. A sparse embedder (SPLADE) captures keyword identity but loses structure. The 12 together cover the space.
2. **Embedder disagreement is itself a reward signal.** When semantic says "close" but graph says "distant" and sparse sees token drift, that disagreement is informative. With one embedder you have no disagreement to mine. With twelve you have ~66 unordered pairs to mine.
3. **Per-cell variance.** Some (mutation_category × language) cells respond to one embedder and not another. `subtle_flip` cells respond to E7 (Qodo code) very specifically; `wrong_file` cells respond to E8 (graph). The system can route per cell to the embedders that best discriminate.
4. **Fail-soft.** When one embedder model fails to load (corrupted weights, GPU OOM), the system can run with 11 and emit a degraded prediction. Single-embedder systems can't degrade gracefully.

Twelve isn't sacred. The number was picked by experiment + cost-of-VRAM-on-a-single-RTX-5090. If a 13th embedder demonstrably adds signal, it goes in.

## The list

| Slot | Model | Dim | Active in forward-cache | Type | Strength |
|---|---|---|---|---|---|
| E1 | intfloat/e5-large-v2 | 1024 | ✅ | dense | natural-language semantic |
| E2 | algorithmic temporal_recent (sinusoidal PE) | 512 | ✅ | algorithmic | recency |
| E3 | algorithmic temporal_periodic (recurrence) | 512 | ✅ | algorithmic | cyclic patterns |
| E4 | algorithmic temporal_positional (session) | 512 | ✅ | algorithmic | session-relative position |
| E5 | (causal asymmetric — disabled in current corpus) | 768 | ⛔ | dense | causal direction |
| E6 | naver/splade-cocondenser-ensembledistil | 1536 | ✅ | sparse | keyword |
| E7 | Qodo/Qodo-Embed-1-1.5B | 1536 | ✅ | dense | code-specific |
| E8 | intfloat/e5-large-v2 (graph mode) | 1024 | ✅ | dense | graph-context |
| E9 | algorithmic HDC | 1024 | ✅ | hyperdimensional | binary patterns |
| E10 | intfloat/e5-base-v2 (multimodal) | 768 | ✅ | dense | contextual paraphrase |
| E11 | (entity — disabled per CLAUDE.md) | 768 | ⛔ | dense | named entities |
| E12 | colbert-ir/colbertv2.0 | 128/token | ✅ | late-interaction | token-level match |
| E13 | prithivida/Splade_PP_en_v1 | 1536 | ✅ | sparse | SPLADE-style keyword |
| E14 | BAAI/bge-m3 | 1024 | ✅ | dense | multilingual code |

**12 active content embedders** in the current Python corpus run. E5 and E11 are disabled by current policy. E15-E21 (learner-state: affect, ppg, eda, eeg) are out of scope for the code-prediction product.

## Each embedder's job

### E1 — Semantic (e5-large-v2, 1024-d)
Captures natural-language paraphrase. Two chunks that *mean* roughly the same thing will be close in E1, even if they share no tokens. Useful for catching when the agent's claim *describes* something the patch *implements* under different wording.

### E2/E3/E4 — Algorithmic temporal (512-d each)
No neural model. E2 is sinusoidal positional encoding over time-since-event. E3 is a recurrence-based encoding capturing cyclic patterns (daily, weekly). E4 is session-relative positional encoding. These run on CPU, no GPU. They're cheap and capture "when did this happen relative to other events" without learning anything.

### E6 — SPLADE-style sparse (1536-d)
Captures keyword identity. If the patch removes the word `assert` and adds the word `raise`, E6 sees that exactly. SPLADE is sparse — most dimensions are zero — so it's efficient for set-similarity queries.

### E7 — Qodo code embedder (1536-d, 1.5B params)
The single biggest embedder by VRAM (~3 GB). Specifically trained on code. Captures API surface, control flow, identifier patterns. The most influential embedder for Q2 (does it work) predictions on code-specific failure modes.

### E8 — Graph-mode semantic (e5-large-v2 with graph context, 1024-d)
Same model as E1 but called with the chunk's static-graph neighborhood as context (parent function, calling functions, called functions). Captures structural context that E1 alone misses.

### E9 — Hyperdimensional Computing (HDC, 1024-d, algorithmic)
No neural model. Bipolar (+1/-1) vectors built by binding token n-grams. Robust to noise. Useful for set-membership style queries: "does this chunk share a subset of tokens with that chunk."

### E10 — Multimodal-e5 (768-d)
Smaller paraphrase model. Captures contextual similarity at a different granularity than E1. Helps cover the gap between long-form e5-large-v2 and token-level ColBERT.

### E12 — ColBERT v2 late-interaction (128-d per token)
Token-level matching. Instead of one vector per chunk, ColBERT produces N vectors (one per token). At query time, MaxSim across the token vectors gives a similarity score. Catches token-level matches that pooled embeddings miss.

### E13 — SPLADE v3 (1536-d sparse)
Newer SPLADE model than E6. Complementary keyword surface. Per the `06_RISKS_AND_NON_TRANSFERABLE` doc, both SPLADE variants are kept because they disagree on enough chunks to be worth carrying.

### E14 — BGE-M3 multilingual dense (1024-d)
Multilingual code embedder. Captures non-English identifiers and comments better than e5. Future-generalization value as the corpus moves beyond Python.

## The asymmetric variants

Some embedders are **asymmetric**: they produce *different* vectors depending on whether the input is a "cause" vs an "effect", or a "source" vs a "target", or a "document" vs a "query". The asymmetric embedders are:

- **E5 (causal):** stores `cause_vec` and `effect_vec` separately. Cosine(cause_vec_A, effect_vec_B) ≠ Cosine(effect_vec_A, cause_vec_B). Useful for directional reasoning. Currently disabled in the Python corpus pipeline.
- **E8 (graph):** stores `source_vec` (the chunk) and `target_vec` (the chunk's neighborhood context). 
- **E10 (paraphrase):** stores `doc_vec` and `query_vec`.

For asymmetric embedders, the system stores both halves in the cache and forwards both at training time. The quantization compresses both. The Rust runner doubles its packed-records work for these embedders.

## The forward cache layout

For each (chunk, embedder) pair the system stores:

```rust
struct PackedRecord {
    chunk_id: ChunkId,         // sha256 of the canonical AST chunk
    embedder_slot: EmbedderId, // E1, E6, E7, ...
    vector: Vec<f32>,          // [dim] or [2 * dim] for asymmetric
    asymmetric_half: Option<AsymmetricHalf>,  // None for symmetric, Some(Cause) / Some(Effect) etc.
}
```

Packed as JSONL with per-shard files:

```
cache_root/shards/shard-NNNNNN-offset-MMMMMM/packed_records/<embedder_slug>.jsonl
cache_root/shards/shard-NNNNNN-offset-MMMMMM/packed_records/<embedder_slug>.index.jsonl
```

The `.index.jsonl` is the byte-offset index for random access in the `.jsonl`. Used by quantization tier compression downstream.

## Quantization tiers (downstream)

Per `docs/futurebuild/09_*` and `context-graph-tier-compression` crate, embeddings live at four quantization tiers:

| Tier | Bits | Compression ratio | Cosine retention (768-d Gaussian) |
|---|---|---|---|
| 8-bit | 8 | 3.89× | 0.99999 |
| 7-bit | 7 | 4.43× | 0.99997 |
| 5-bit | 5 | 6.12× | 0.99960 |
| 3-bit | 3 | 9.91× | 0.99192 |

Hot tier (in-memory + query-path): 8-bit. Warm tier (recent durable): 7-bit. Cold tier (long-term durable): 5-bit. Archive tier: 3-bit.

The tier-compression crate ships, FSV-verified at `/mnt/d/contextgraph/fsv/contextgraph-tier-compression-fsv/`.

## How the predictor joins them

Per chunk, the predictor sees:
1. Concatenate or attention-pool the 12 per-embedder vectors → joint embedding (~10-12K-d depending on choice).
2. For asymmetric embedders, take a learned weighting of `cause`/`effect` halves.
3. Apply per-cell weight profiles when in routing mode (some cells weight E7 heavily, others weight E1).
4. Forward into the predictor head MLP.

The joint embedding is what the predictor head consumes. It's the dense representation across all twelve lenses. The output of the predictor head is the per-Q1-Q5 prediction.

## VRAM budget

On the RTX 5090 (32 GB VRAM), with leapable-tei reserving ~3 GB:

| Embedder | Approx VRAM under fp16 | Notes |
|---|---|---|
| E1 | 700 MB | e5-large-v2 |
| E6 | 700 MB | sparse model |
| E7 | 3.0 GB | Qodo-Embed 1.5B — heaviest |
| E8 | 700 MB | e5-large-v2 graph |
| E10 | 500 MB | e5-base-v2 |
| E12 | 1.0 GB | ColBERT v2 |
| E13 | 700 MB | SPLADE v3 |
| E14 | 2.3 GB | BGE-M3 |
| E2/E3/E4/E9 | 0 (CPU) | algorithmic |

Total when all 8 GPU embedders concurrent + CUDA context overhead: ~12-15 GB peak, well under 28 GB safety guard. par=6 is stable; par=12 OOM'd (per session learnings).

## Per-embedder failure modes

**E7 (Qodo) is the most expensive and most likely to OOM** under aggressive parallelism. Batch sizes are tuned per embedder:

```
--embedder-batch-sizes e1=32,e6=32,e7=32,e8=32,e9=128,e10=32,e12=32,e13=32,e14=8
```

E14 (BGE-M3) gets batch=8 because of its 2.3 GB VRAM footprint; E9 (HDC) gets batch=128 because it's CPU-only.

The `--gpu-vram-guard-bytes 30064771072` (28 GB) is the safety threshold; the cargo runner emits `CUDA_ERROR_OUT_OF_MEMORY → true_batch_gpu_oom_split` and splits the batch if it gets close.

## Frozen vs trainable

**Embedders in the predictor path are frozen by default** (per CLAUDE.md §6.10: E1, E6, E7, E8, E10, E12, E13, E14 are frozen by design). The reason: re-finetuning a 1.5B-param embedder per cell is computationally infeasible, and the public pretrained checkpoints are strong enough that the JEPA-style cross-attention head on top can do the cell-specific adaptation.

**The trainable head sits *above* the embedders.** The head learns:
1. How to weight each embedder per cell.
2. How to combine the joint embedding into the per-question heads.
3. How to calibrate against the frozen target panel.

This is where the trainable knobs live. The embedders are stable substrate; the head is the learning surface.

## What's running right now

Per the SITREP, the forward-cache run on aiwonder is processing all 399,777 chunks through all 12 active embedders. Each chunk × 12 = 4.8 M (chunk, embedder) pairs. With quantization at 8-bit, that's ~30 GB on disk. Cache root: `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/`.

When this completes, the substrate is ready for joint-embedding training.
