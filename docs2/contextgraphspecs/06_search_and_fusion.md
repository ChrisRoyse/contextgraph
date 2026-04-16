# Search and Fusion

Source files covered:

- `crates/context-graph-core/src/retrieval/mod.rs` -- retrieval module root (re-exports, 5-stage pipeline doc)
- `crates/context-graph-core/src/retrieval/config.rs` -- thresholds, space weights, SPACE_WEIGHTS constant
- `crates/context-graph-core/src/retrieval/executor.rs` -- MultiEmbeddingQueryExecutor trait
- `crates/context-graph-core/src/retrieval/query.rs` -- MultiEmbeddingQuery, PipelineStageConfig, EmbeddingSpaceMask
- `crates/context-graph-core/src/retrieval/aggregation.rs` -- AggregationStrategy (RRF, WeightedAverage, MaxPooling)
- `crates/context-graph-core/src/retrieval/result.rs` -- result types, PipelineStageTiming
- `crates/context-graph-core/src/retrieval/multi_space.rs` -- MultiSpaceSimilarity service
- `crates/context-graph-core/src/retrieval/distance.rs` -- cosine, Jaccard, MaxSim, per-space dispatch
- `crates/context-graph-core/src/retrieval/similarity.rs` -- PerSpaceScores, SimilarityResult
- `crates/context-graph-core/src/retrieval/sparse_index.rs` -- SparseInvertedIndex for E6/E13
- `crates/context-graph-core/src/fusion/mod.rs` -- FusionStrategy, weighted_rrf, score_weighted_rrf, fuse_rankings
- `crates/context-graph-core/src/weights/mod.rs` -- WEIGHT_PROFILES, get_weight_profile, validate_weights
- `crates/context-graph-core/src/causal/asymmetric.rs` -- causal gate thresholds, direction modifiers
- `crates/context-graph-core/src/index/config.rs` -- HnswConfig, EmbedderIndex, DistanceMetric
- `crates/context-graph-core/src/index/hnsw_impl/mod.rs` -- HnswMultiSpaceIndex (12 HNSW + SPLADE)
- `crates/context-graph-core/src/index/hnsw_impl/multi_space.rs` -- multi-space index struct
- `crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -- MultiSpaceIndexManager impl
- `crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -- SearchStrategy enum, TeleologicalSearchOptions
- `crates/context-graph-core/src/config/constants.rs` -- RRF_K, DEFAULT_RRF_K
- `crates/context-graph-storage/src/teleological/search/mod.rs` -- search module root
- `crates/context-graph-storage/src/teleological/search/pipeline/mod.rs` -- RetrievalPipeline (5-stage)
- `crates/context-graph-storage/src/teleological/search/pipeline/types.rs` -- PipelineStage, PipelineConfig, PipelineCandidate
- `crates/context-graph-storage/src/teleological/search/pipeline/stages.rs` -- stage implementations
- `crates/context-graph-storage/src/teleological/search/multi/mod.rs` -- MultiEmbedderSearch
- `crates/context-graph-storage/src/teleological/search/multi/types.rs` -- NormalizationStrategy, AggregationStrategy (storage)
- `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` -- weighted_rrf_fusion (storage layer)
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` -- search_graph handler, strategy routing, causal gate
- `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -- search_by_embedder handler

---

## 1. Overview

Context Graph supports search across 13 embedding spaces using 4 search strategies. All searches produce similarity scores in the normalized range [0.0, 1.0]. Results from multiple embedders are combined using Reciprocal Rank Fusion (RRF) as the primary aggregation strategy per ARCH-18/ARCH-21.

### 1.1 The 13 Embedding Spaces

| Index | Name | Dimension | Type | Similarity Metric |
|-------|------|-----------|------|-------------------|
| 0 | E1_Semantic | 1024 | Dense HNSW | Cosine |
| 1 | E2_Temporal_Recent | 512 | Dense HNSW | Cosine |
| 2 | E3_Temporal_Periodic | 512 | Dense HNSW | Cosine |
| 3 | E4_Temporal_Positional | 512 | Dense HNSW | Cosine |
| 4 | E5_Causal | 768 | Dense HNSW | Asymmetric Cosine |
| 5 | E6_Sparse | 30,522 vocab | Sparse Inverted | Jaccard |
| 6 | E7_Code | 1536 | Dense HNSW | Cosine |
| 7 | E8_Graph | 1024 | Dense HNSW | Cosine |
| 8 | E9_HDC | 1024 | Dense HNSW | Cosine |
| 9 | E10_Multimodal | 768 | Dense HNSW | Cosine |
| 10 | E11_Entity | 768 | Dense HNSW | Cosine |
| 11 | E12_Late_Interaction | 128/token | ColBERT MaxSim | MaxSim |
| 12 | E13_SPLADE | 30,522 vocab | Sparse Inverted | Jaccard/BM25 |

Defined in: `crates/context-graph-core/src/index/config.rs`

### 1.2 Index Architecture

Two index layers coexist:

- **Core layer** (`crates/context-graph-core/src/index/config.rs:108`): `EmbedderIndex::all_hnsw()` returns **11 HNSW**: E1, E1Matryoshka128, E2-E5, E7-E11.
- **Storage layer** (`crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs:37`): **15 HNSW** indexes adding the four asymmetric dual-vector indexes -- `E5CausalCause`, `E5CausalEffect`, `E10MultimodalParaphrase`, `E10MultimodalContext` (ARCH-15).
- **2 Inverted indexes**: E6 (sparse), E13 (SPLADE)
- **1 Token-level index**: E12 (ColBERT MaxSim, not HNSW)

E6, E12, and E13 do NOT use HNSW. E6 and E13 use inverted indexes; E12 uses MaxSim token-level matching. See `docs2/contextgraphspecs/05_embedding_subsystem.md` Section 16 for the full cross-reference.

---

## 2. Search Strategy Selection

Strategies are defined in `SearchStrategy` (file: `crates/context-graph-core/src/traits/teleological_memory_store/options.rs:61-77`).

```rust
#[derive(Default)]
pub enum SearchStrategy {
    #[default]
    E1Only,     // E1 HNSW only (fastest, backward compatible)
    MultiSpace, // Weighted fusion of multiple embedders
    Pipeline,   // 5-stage progressive filtering pipeline
}
```

`SearchStrategy::default()` returns `E1Only`. The MCP handler (`search_graph`) overrides this default to `MultiSpace` when no `strategy` parameter is supplied.

An additional mode, `embedder_first`, is not a formal `SearchStrategy` variant but is implemented as a separate MCP tool (`search_by_embedder`) that searches a single specified embedder.

### 2.1 Strategy Selection in the MCP Handler

In `search_graph` (file: `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`), the strategy is selected from the `strategy` parameter:

| Parameter Value | Mapped Strategy | Notes |
|----------------|-----------------|-------|
| `"e1_only"` | `SearchStrategy::E1Only` | Single-embedder semantic search |
| `"multi_space"` | `SearchStrategy::MultiSpace` | Default when omitted |
| `"pipeline"` | `SearchStrategy::Pipeline` | Full 5-stage pipeline |
| (none / missing) | `SearchStrategy::MultiSpace` | Default |

When a `weightProfile` or `customWeights` parameter is provided, the strategy is forced to `MultiSpace` regardless of the `strategy` parameter value, because custom weights only have meaning in multi-space fusion.

---

## 3. E1Only Strategy

The simplest and fastest strategy. Searches only the E1 Semantic HNSW index (1024D, e5-large-v2).

**Algorithm:**
1. Embed query text to produce E1 semantic vector (1024D)
2. Search E1 HNSW index for k nearest neighbors
3. Return results ranked by cosine similarity

**Participating embedders:** E1 only.

**Scoring:** Raw cosine similarity normalized to [0.0, 1.0] via SRC-3 conversion.

---

## 4. MultiSpace Strategy

Weighted fusion across all dense embedders. This is the default strategy.

### 4.1 Algorithm Steps

1. Embed query text to produce all 13 embeddings (SemanticFingerprint)
2. For each active embedder with weight > 0.0:
   - Search the embedder's HNSW index (or compute similarity for non-HNSW embedders)
   - Produce a ranked list of candidates
3. Combine results using Weighted RRF across all ranked lists
4. Apply causal gating if the query is detected as causal
5. Return top-k results sorted by fused score

### 4.2 Active Embedders in MultiSpace

From `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs:1545-1556`, MultiSpace seeds `strategy_indices = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]` and conditionally pushes `10` (E11) when `E11_ENTITY_ENABLED` is `true`. `E11_ENTITY_ENABLED` is `false` by default (`crates/context-graph-core/src/weights/mod.rs:45`), so the active set is 10 indices.

After seeding, active indices are filtered by `resolved_weights[idx] > 0.0` (`memory_tools.rs:1569-1571`). This makes all embedder slots weight-gated in practice:

- E2 (index 1), E3 (index 2), E4 (index 3) are first-class HNSW-indexed fusion participants when their weight is > 0.0. `TEMPORAL_EMBEDDERS_ENABLED = true` (`weights/mod.rs:40`) is a global kill-switch; when true, temporal profiles like `temporal_navigation` activate them.
- E6 (index 5) participates in scoring (not HNSW recall) when its weight > 0.

E12 (Late Interaction) and E13 (SPLADE) have weight 0.0 in all scoring profiles per ARCH-13 -- they are pipeline-stage-only embedders (E13 in Stage 1 recall, E12 in Stage 4 MaxSim rerank).

### 4.3 Scoring

Scores from each embedder are combined using Weighted RRF:

```
RRF_weighted(d) = SUM_i( weight_i / (K + rank_i(d) + 1) )
```

Where `K = 60` (defined in `crates/context-graph-core/src/config/constants.rs` as `similarity::RRF_K`).

---

## 5. Pipeline Strategy

A 5-stage (with optional 3.5 and 3.75 substages) progressive filtering pipeline optimized for latency at scale.

### 5.1 Stage Definitions

| Stage | Name | Index Type | Input | Output | Latency Target |
|-------|------|-----------|-------|--------|---------------|
| 1 | SPLADE Sparse Pre-filter | Inverted index (E13 or E6) | Full corpus (1M+) | ~10K candidates | <5ms |
| 2 | Matryoshka 128D Fast ANN | HNSW (E1Matryoshka128) | ~10K | ~1K candidates | <10ms |
| 3 | Multi-space RRF Rerank | HNSW (E1, E5, E8 by default) | ~1K | ~100 candidates | <20ms |
| 3.5 | Graph Expansion (optional) | K-NN edge traversal | ~100 | ~150 candidates | <10ms |
| 3.75 | GNN Enhancement (optional) | R-GCN message passing | ~150 | ~150 candidates | <20ms |
| 4 | Late Interaction MaxSim | ColBERT (E12) | 100-150 | k results (e.g., 10) | <15ms |

Total latency target: <60ms at 1M memories.

Defined in:
- `crates/context-graph-storage/src/teleological/search/pipeline/types.rs` -- `PipelineStage` enum, `PipelineConfig`
- `crates/context-graph-storage/src/teleological/search/pipeline/stages.rs` -- stage implementations

### 5.2 Stage 1: SPLADE Sparse Pre-filter

Uses the sparse inverted index (E13 SPLADE or E6). NOT HNSW.

**Algorithm:**
1. Extract active terms from query sparse vector
2. Look up posting lists in the inverted index for all query terms
3. Score candidates by weighted overlap (BM25 weighting for E13)
4. Return top candidates (default: k * 10 * candidate_multiplier)

The sparse inverted index is implemented in `crates/context-graph-core/src/retrieval/sparse_index.rs`. Default recall limit: 10,000 candidates.

### 5.3 Stage 2: Matryoshka 128D Fast ANN

Uses the E1Matryoshka128 HNSW index (128D truncation of E1 semantic, M=32, ef_construction=256, ef_search=128).

**Algorithm:**
1. Take the first 128 dimensions of the E1 query vector
2. Search E1Matryoshka128 HNSW index
3. Filter results to Stage 1 candidate set (intersection)
4. Return filtered candidates

### 5.4 Stage 3: Multi-space RRF Rerank

Searches multiple HNSW-capable embedders and combines with RRF.

**Default RRF embedders** (from `PipelineConfig::default()`):
- E1Semantic
- E8Graph
- E5Causal

**RRF formula applied:**
```
RRF(d) = SUM_i( 1 / (K + rank_i(d) + 1) )
```
Where K = 60.0 (`PipelineConfig.rrf_k`).

### 5.5 Stage 3.5: Graph Expansion (Optional)

Expands the candidate set by traversing pre-computed K-NN graph edges.

**Configuration** (`GraphExpansionConfig`):
- `max_expansion_per_node`: 5 (default)
- `min_edge_weight`: 0.4
- `expansion_decay`: 0.8 (expanded node score = parent_score * edge_weight * 0.8)
- `max_total_expanded`: 150

### 5.6 Stage 3.75: GNN Enhancement (Optional)

R-GCN message passing for learned retrieval enhancement. Disabled by default (requires trained model).

**Configuration** (`GnnEnhanceConfig`):
- `blend_weight`: 0.3 (final_score = original * 0.7 + gnn_score * 0.3)
- `max_subgraph_nodes`: 200
- `min_similarity`: 0.3

### 5.7 Stage 4: Late Interaction MaxSim (E12)

ColBERT-style token-level matching. NOT HNSW.

**MaxSim formula** (from `crates/context-graph-core/src/retrieval/distance.rs`):

```
MaxSim(Q, M) = (1/|Q|) * SUM_{q in Q} max_{m in M} cosine(q, m)
```

Each query token (128D) finds its maximum cosine similarity against all memory tokens, then the scores are averaged. Returns a value in [0.0, 1.0].

### 5.8 Pipeline Stage Configuration

Default candidate counts per stage (from `PipelineStageConfig::default()`):

| Parameter | Default | Description |
|-----------|---------|-------------|
| `splade_candidates` | 1000 | Stage 1 output count |
| `matryoshka_128d_limit` | 200 | Stage 2 output count |
| `full_search_limit` | 100 | Stage 3 output count |
| `teleological_limit` | 50 | Stage 4 input count |
| `late_interaction_limit` | 20 | Stage 5 output count |
| `rrf_k` | 60.0 | RRF constant |

---

## 6. Embedder-First Strategy (search_by_embedder)

Implemented as a separate MCP tool (`search_by_embedder`) in `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs`.

**Algorithm:**
1. Accept an explicit embedder name (e.g., "E5", "E7", "E9")
2. Embed query text into the full 13-embedder fingerprint
3. Search only the specified embedder's index
4. For E5 (Causal): uses explicit `CausalDirection::Cause` for asymmetric scoring
5. Return results from that single embedder

This mode is useful for exploring what specific embedders discover that E1 might miss (blind spot detection).

---

## 7. Weight Profiles

Weight profiles control how much each embedder contributes to multi-space fusion. Defined in `crates/context-graph-core/src/weights/mod.rs` as `WEIGHT_PROFILES`.

### 7.1 Built-in Profiles

All profiles have exactly 13 weights that sum to approximately 1.0.

| Profile | E1 | E2 | E3 | E4 | E5 | E6 | E7 | E8 | E9 | E10 | E11 | E12 | E13 | Primary Use |
|---------|-----|-----|-----|-----|-----|-----|-----|-----|-----|------|------|------|------|-------------|
| `semantic_search` | 0.33 | 0.0 | 0.0 | 0.0 | 0.15 | 0.05 | 0.20 | 0.05 | 0.02 | 0.15 | 0.05 | 0.0 | 0.0 | General queries |
| `causal_reasoning` | 0.40 | 0.0 | 0.0 | 0.0 | 0.10 | 0.05 | 0.15 | 0.10 | 0.0 | 0.10 | 0.10 | 0.0 | 0.0 | "Why" questions |
| `code_search` | 0.20 | 0.0 | 0.0 | 0.0 | 0.10 | 0.10 | 0.40 | 0.0 | 0.0 | 0.10 | 0.10 | 0.0 | 0.0 | Programming queries |
| `fact_checking` | 0.15 | 0.0 | 0.0 | 0.0 | 0.15 | 0.15 | 0.05 | 0.05 | 0.0 | 0.05 | 0.40 | 0.0 | 0.0 | Entity/fact queries |
| `graph_reasoning` | 0.15 | 0.0 | 0.0 | 0.0 | 0.10 | 0.10 | 0.0 | 0.40 | 0.0 | 0.05 | 0.20 | 0.0 | 0.0 | Structural queries |
| `temporal_navigation` | 0.13 | 0.23 | 0.23 | 0.23 | 0.03 | 0.02 | 0.03 | 0.02 | 0.03 | 0.03 | 0.02 | 0.0 | 0.0 | Time-based queries |
| `sequence_navigation` | 0.20 | 0.05 | 0.0 | 0.55 | 0.03 | 0.02 | 0.03 | 0.02 | 0.03 | 0.03 | 0.02 | 0.0 | 0.02 | Sequence traversal |
| `conversation_history` | 0.30 | 0.05 | 0.0 | 0.35 | 0.10 | 0.03 | 0.05 | 0.02 | 0.0 | 0.05 | 0.03 | 0.0 | 0.02 | Contextual recall |
| `category_weighted` | 1/6.5 (~0.154) | 0.0 | 0.0 | 0.0 | 1/6.5 | 1/6.5 | 1/6.5 | 0.5/6.5 (~0.077) | 0.5/6.5 | 1/6.5 | 0.5/6.5 | 0.0 | 0.0 | Constitution-compliant |
| `typo_tolerant` | 0.30 | 0.0 | 0.0 | 0.0 | 0.10 | 0.07 | 0.15 | 0.03 | 0.18 | 0.12 | 0.05 | 0.0 | 0.0 | Noisy queries |
| `pipeline_stage1_recall` | 0.20 | 0.0 | 0.0 | 0.0 | 0.05 | 0.25 | 0.10 | 0.0 | 0.05 | 0.05 | 0.05 | 0.0 | 0.25 | Stage 1 recall |
| `pipeline_stage2_scoring` | 0.50 | 0.0 | 0.0 | 0.0 | 0.12 | 0.05 | 0.15 | 0.03 | 0.02 | 0.08 | 0.05 | 0.0 | 0.0 | Stage 2 scoring |
| `pipeline_full` | 0.40 | 0.0 | 0.0 | 0.0 | 0.10 | 0.10 | 0.15 | 0.03 | 0.02 | 0.08 | 0.05 | 0.0 | 0.07 | Full pipeline |
| `balanced` | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.077 | 0.076 | Testing/comparison |

### 7.2 Profile Design Rules

1. **E12 and E13 have weight 0.0 in all semantic scoring profiles** (per ARCH-13). They are pipeline-stage-only: E13 for Stage 1 recall, E12 for Stage 4 MaxSim reranking.
2. **E2-E4 have weight 0.0 in semantic profiles** (per AP-60/AP-71). Temporal proximity is not topical similarity.
3. **E2-E4 have non-zero weights** only in temporal-aware profiles: `temporal_navigation`, `sequence_navigation`, `conversation_history`, `balanced`.

### 7.3 E11 Entity Disable

When `E11_ENTITY_ENABLED = false` (current default), `get_effective_weight_profile()` zeros out E11's weight and proportionally redistributes it to remaining active embedders so the profile still sums to ~1.0.

### 7.4 Custom Profiles

Users can provide custom weights via the `customWeights` parameter in `search_graph`. Requirements:
- Array of 13 floats: `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]`
- Each weight must be in [0.0, 1.0]
- All weights must sum to approximately 1.0 (tolerance: 0.01)

Validation is performed by `validate_weights()` in `crates/context-graph-core/src/weights/mod.rs`.

### 7.5 Weight Normalization

`SpaceWeights::normalize()` (in `crates/context-graph-core/src/retrieval/config.rs`) normalizes weights to sum to 13.0:

```
factor = 13.0 / sum(weights)
weight_i = weight_i * factor
```

If sum is 0.0, weights remain unchanged to avoid division by zero.

---

## 8. Result Fusion

### 8.1 Reciprocal Rank Fusion (RRF)

RRF is the primary fusion strategy. It is rank-based and score-distribution-invariant.

**Standard RRF** (from `crates/context-graph-core/src/retrieval/aggregation.rs`):

```
RRF(d) = SUM_i( 1 / (K + rank_i(d) + 1) )
```

Where:
- `K = 60` (constant, from Constitution `embeddings.similarity.rrf_constant`)
- `rank_i(d)` is the 0-indexed rank of document `d` in embedder `i`'s results
- The `+1` converts 0-based rank to 1-based denominator

**Weighted RRF** (from `crates/context-graph-core/src/fusion/mod.rs`):

```
RRF_weighted(d) = SUM_i( weight_i / (K + rank_i(d) + 1) )
```

Where `weight_i` is the embedder's weight from the active weight profile.

**Score-Weighted RRF** (`ScoreWeightedRRF` variant):

For score-weighted embedders (e.g., E5 Causal):
```
contribution = weight * similarity_score * (1 / (K + rank + 1))
```

For standard embedders:
```
contribution = weight * (1 / (K + rank + 1))
```

This preserves E5 score magnitude -- a strong causal signal (0.58) contributes more than a weak one (0.12) at the same rank.

### 8.2 RRF Score Examples

For K=60:
- Rank 0: 1/(60+0+1) = 1/61 = 0.01639
- Rank 1: 1/(60+1+1) = 1/62 = 0.01613
- Rank 9: 1/(60+9+1) = 1/70 = 0.01429
- Rank 99: 1/(60+99+1) = 1/160 = 0.00625

### 8.3 Additive RRF with Blind-Spot Detection

`aggregate_rrf_additive()` (in `aggregation.rs`) extends standard RRF with contribution tracking:

- Tracks which embedders found each result
- Results found by only one embedder are "blind spot discoveries"
- Blind spot discoveries receive a 10% boost: `boosted_score = rrf_score * 1.1`
- Results found by 10+ embedders receive a "strong consensus" annotation

### 8.4 Alternative Aggregation Strategies

Besides RRF, the system supports:

| Strategy | Formula | Use Case |
|----------|---------|----------|
| `WeightedAverage` | `SUM(weight_i * sim_i) / SUM(weight_i)` | When raw scores are calibrated |
| `MaxPooling` | `max(sim_i)` | When any strong signal suffices |

These are defined in `AggregationStrategy` (file: `crates/context-graph-core/src/retrieval/aggregation.rs`) but RRF is the default and recommended strategy.

### 8.5 Fusion Strategy Enum

The `FusionStrategy` enum (in `crates/context-graph-core/src/fusion/mod.rs`) selects between fusion modes:

| Variant | Description |
|---------|-------------|
| `WeightedRRF` (default) | Standard weighted RRF |
| `WeightedSum` | Legacy `SUM(weight * similarity)` |
| `ScoreWeightedRRF` | E5 uses score-weighted contribution; others use standard RRF |

### 8.6 Min-Max Score Normalization

`normalize_minmax()` (in `crates/context-graph-core/src/fusion/mod.rs`) normalizes scores within a result set:

```
normalized_score = (score - min) / (max - min)
```

When all scores are equal, they normalize to 1.0. When the input set is empty, returns (0.0, 1.0).

---

## 9. Similarity Computation

### 9.1 Cosine Similarity

**Normalized cosine** (`cosine_similarity()` in `distance.rs`):

```
raw = dot(a, b) / (||a|| * ||b||)        -- clamped to [-1, 1]
normalized = (raw + 1) / 2               -- SRC-3: maps to [0, 1]
```

- Zero-magnitude vectors return 0.0 (AP-10: no NaN)
- Empty or mismatched-dimension vectors return 0.0
- Zero-allocation single-pass implementation

**Raw cosine** (`cosine_similarity_raw()`):

Returns raw cosine in [-1.0, 1.0] without SRC-3 normalization. Used internally where raw cosine is needed (e.g., causal chain scoring).

### 9.2 Jaccard Similarity

Used for sparse vectors (E6, E13):

```
Jaccard(A, B) = |A intersect B| / |A union B|
```

Computed over non-zero index sets. Returns [0.0, 1.0] where 1.0 = identical index sets.

### 9.3 MaxSim (ColBERT Late Interaction)

Used for E12 token-level matching:

```
MaxSim(Q, M) = (1/|Q|) * SUM_{q in Q}( max_{m in M}( cosine(q, m) ) )
```

For each query token embedding, find its maximum cosine similarity against all memory token embeddings, then average. Returns [0.0, 1.0].

### 9.4 Per-Space Dispatch

`compute_similarity_for_space()` in `distance.rs` dispatches to the correct metric:

| Embedder | Metric | Notes |
|----------|--------|-------|
| E1, E2-E4, E7, E9, E11 | Cosine (symmetric) | Standard normalized cosine |
| E5 (Causal) | Returns -1.0 sentinel | Must use `compute_similarity_for_space_with_direction()` |
| E6 (Sparse) | Jaccard | Sparse inverted index |
| E8 (Graph) | Cosine cross-pair max | `max(cosine(q.source, m.target), cosine(q.target, m.source))` |
| E10 (Multimodal) | Cosine cross-pair max | `max(cosine(q.paraphrase, m.context), cosine(q.context, m.paraphrase))` |
| E12 (Late Interaction) | MaxSim | Token-level |
| E13 (SPLADE) | Jaccard | Sparse inverted index |

### 9.5 E5 Asymmetric Similarity

E5 uses dual vectors (cause/effect). Without a direction, `compute_similarity_for_space()` returns -1.0 as a sentinel indicating "no signal" (distinct from 0.0 which indicates anti-correlation after normalization).

With direction (`compute_similarity_for_space_with_direction()`):

```
sim = asymmetric_cosine(query, memory, query_is_cause) * direction_modifier
```

Clamped to [0.0, 1.0].

### 9.6 E8/E10 Cross-Pair Comparison

Both E8 (Graph) and E10 (Multimodal) store asymmetric dual vectors:
- E8: source/target pairs
- E10: paraphrase/context pairs

The similarity is computed as the maximum of both cross-pair directions, capturing the strongest signal regardless of direction.

---

## 10. Causal Gating

Causal gating applies a binary boost/demotion to search results based on E5 causal scores. Defined in `crates/context-graph-core/src/causal/asymmetric.rs`.

### 10.1 Threshold Values

| Constant | Value | Meaning |
|----------|-------|---------|
| `CAUSAL_THRESHOLD` | 0.04 | E5 score >= this = "definitely causal" |
| `NON_CAUSAL_THRESHOLD` | 0.008 | E5 score <= this = "definitely non-causal" |
| `CAUSAL_BOOST` | 1.10 | 10% score boost for causal content |
| `NON_CAUSAL_DEMOTION` | 0.85 | 15% score demotion for non-causal content |

Calibrated against post-training E5 score distribution (causal mean=0.138, non-causal mean=0.016).

### 10.2 Gate Logic

From `apply_causal_gate()`:

```
if NOT is_causal_query:
    return original_score  -- no change for non-causal queries

if e5_score >= 0.04:
    return original_score * 1.10   -- boost

if e5_score <= 0.008:
    return original_score * 0.85   -- demotion

return original_score  -- ambiguous zone, no change
```

The ambiguous dead zone (0.008 < e5_score < 0.04) receives no modification.

### 10.3 Direction Modifiers

Applied during E5 asymmetric similarity computation:

| Direction Pair | Modifier | Meaning |
|---------------|----------|---------|
| cause to effect | 1.2 | Forward causal inference amplified |
| effect to cause | 0.8 | Backward inference dampened |
| same direction | 1.0 | No modification |
| unknown | 1.0 | No modification |

---

## 11. HNSW Recall Phase

### 11.1 HNSW Configuration

Default per-embedder configuration (from `HnswConfig::default_for_dimension()`):

| Parameter | Value | Notes |
|-----------|-------|-------|
| M | 16 | Bi-directional links per node |
| ef_construction | 200 | Candidate list size during build |
| ef_search | 100 | Candidate list size during search |
| Metric | Cosine | Default for most embedders |

Matryoshka 128D (Stage 2) uses tuned values:

| Parameter | Value |
|-----------|-------|
| M | 32 |
| ef_construction | 256 |
| ef_search | 128 |
| Dimension | 128 |

### 11.2 Index Initialization

`HnswMultiSpaceIndex` creates 11 real HNSW indexes using `hnsw_rs`:
- E1Semantic (1024D), E1Matryoshka128 (128D)
- E2TemporalRecent (512D), E3TemporalPeriodic (512D), E4TemporalPositional (512D)
- E5Causal (768D), E7Code (1536D), E8Graph (1024D)
- E9HDC (1024D), E10Multimodal (768D), E11Entity (768D)

### 11.3 Fingerprint Indexing

When a memory is stored, `add_fingerprint()` extracts vectors from the `SemanticFingerprint` and adds them to each HNSW index. The Matryoshka 128D vector is created by truncating E1 to its first 128 dimensions.

### 11.4 Performance Targets

| Operation | Target |
|-----------|--------|
| `add_vector()` | <1ms per index |
| `search()` | <10ms per index |
| `persist()` | <1s for 100K vectors |
| Total search | <60ms at 1M memories |

---

## 12. Relevance and Threshold Detection

### 12.1 Multi-Space Relevance

`MultiSpaceSimilarity` (in `multi_space.rs`) computes relevance using ANY-match logic: a result is relevant if ANY embedding space has a score above its high threshold.

### 12.2 High Thresholds (Relevance Detection)

From `high_thresholds()` in `config.rs`:

| Embedder | Threshold |
|----------|-----------|
| E1 Semantic | 0.75 |
| E2 Temporal Recent | 0.70 |
| E3 Temporal Periodic | 0.70 |
| E4 Temporal Positional | 0.70 |
| E5 Causal | 0.70 |
| E6 Sparse | 0.60 |
| E7 Code | 0.80 |
| E8 Graph | 0.70 |
| E9 HDC | 0.70 |
| E10 Multimodal | 0.70 |
| E11 Entity | 0.70 |
| E12 Late Interaction | 0.70 |
| E13 SPLADE | 0.60 |

### 12.3 Low Thresholds (Divergence Detection)

From `low_thresholds()` in `config.rs`:

| Embedder | Threshold |
|----------|-----------|
| E1 Semantic | 0.30 |
| E2 Temporal Recent | 0.30 |
| E3 Temporal Periodic | 0.30 |
| E4 Temporal Positional | 0.30 |
| E5 Causal | 0.25 |
| E6 Sparse | 0.20 |
| E7 Code | 0.35 |
| E8 Graph | 0.30 |
| E9 HDC | 0.30 |
| E10 Multimodal | 0.30 |
| E11 Entity | 0.30 |
| E12 Late Interaction | 0.30 |
| E13 SPLADE | 0.20 |

Scores below the low threshold indicate topic divergence.

### 12.4 Weighted Relevance Score

From `compute_relevance_score()` in `multi_space.rs`:

```
weighted_sum = SUM( category_weight * max(0, score - high_threshold) )
max_possible = SUM( category_weight * max(0, 1.0 - high_threshold) )
relevance = weighted_sum / max_possible       -- clamped to [0.0, 1.0]
```

Temporal spaces (E2-E4) have `category_weight = 0.0` and are excluded (AP-60).

### 12.5 Category Weights for Topic Detection

From `SPACE_WEIGHTS` constant in `config.rs`:

| Category | Embedders | Weight | Count |
|----------|-----------|--------|-------|
| Semantic | E1, E5, E6, E7, E10, E12, E13 | 1.0 | 7 |
| Temporal | E2, E3, E4 | 0.0 | 3 |
| Relational | E8, E11 | 0.5 | 2 |
| Structural | E9 | 0.5 | 1 |

**TOTAL_WEIGHT** = 7 * 1.0 + 2 * 0.5 + 1 * 0.5 = **8.5**

Topic threshold (ARCH-09): `weighted_agreement >= 2.5`.

---

## 13. Search Parameters

### 13.1 Query Parameters

From `MultiEmbeddingQuery` (in `query.rs`):

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `query_text` | String | (required) | non-empty | Text to embed and search |
| `active_spaces` | EmbeddingSpaceMask | ALL (0x1FFF) | bitmask | Which spaces to search |
| `space_weights` | Option<[f32; 13]> | None | [0.0, 1.0] each | Per-space weight overrides |
| `per_space_limit` | usize | 100 | [1, 1000] | Max results per space |
| `final_limit` | usize | 10 | [1, 1000] | Final result count |
| `min_similarity` | f32 | 0.0 | [0.0, 1.0] | Minimum similarity threshold |
| `aggregation` | AggregationStrategy | RRF { k: 60.0 } | -- | Aggregation method |

### 13.2 TeleologicalSearchOptions

From `TeleologicalSearchOptions` (in `options.rs`):

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `top_k` | usize | 10 | Maximum results (max: 1000) |
| `min_similarity` | f32 | 0.0 | Minimum similarity [0.0, 1.0] |
| `strategy` | SearchStrategy | E1Only | Search strategy selection |
| `weight_profile` | Option<String> | None | Named weight profile |
| `custom_weights` | Option<[f32; 13]> | None | Custom per-embedder weights |
| `synergy_weights` | Option<[f32; 13]> | None | Per-embedder synergy multipliers for pairwise boost (`options.rs:1138`) |
| `use_quantized_prefilter` | bool | false | Enable quantized prefiltering for faster candidate selection (`options.rs:1148`) |
| `enable_teleological_prefilter` | bool | false | Enable 13D teleological vector prefilter (`options.rs:1158`) |

**Synergy boost** (commit 80558edc): When `synergy_weights` is set, `apply_synergy_boost()` in `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs:192` adds a pairwise bonus proportional to `synergy_weights[i] * synergy_weights[j] * SYNERGY_BASE_BOOST` for every pair of active embedders that both contribute to a result.

**Teleological prefilter**: Uses a 13-dimensional teleological vector (`TELEOLOGICAL_VECTOR_DIM = 13`, defined in `crates/context-graph-core/src/types/fingerprint/teleological/types.rs:12`) for fast filtering before full scoring.

### 13.3 EmbeddingSpaceMask Presets

| Preset | Value | Active Count | Description |
|--------|-------|-------------|-------------|
| `ALL` | 0x1FFF | 13 | All spaces |
| `ALL_DENSE` | 0x0FFF | 12 | All except E13 SPLADE |
| `SEMANTIC_ONLY` | 0x0001 | 1 | E1 only |
| `TEXT_CORE` | 0x0007 | 3 | E1, E2, E3 |
| `SPLADE_ONLY` | 0x1000 | 1 | E13 only |
| `HYBRID` | 0x1001 | 2 | E1 + E13 |
| `CODE_FOCUSED` | 0x1041 | 3 | E1 + E7 + E13 |

### 13.4 Decay Functions

For temporal post-retrieval boosting (from `DecayFunction` in `options.rs`):

| Function | Formula | Description |
|----------|---------|-------------|
| `Linear` (default) | `1.0 - (age / max_age)` | Predictable, good for short horizons |
| `Exponential` | `exp(-age * 0.693 / half_life)` | Natural forgetting curve |
| `Step` | Bucket-based | <5min: 1.0, <1h: 0.8, <1d: 0.5, else: 0.1 |
| `NoDecay` | Always 1.0 | No temporal factor |

---

## 14. Score Normalization

### 14.1 SRC-3: Cosine to [0, 1]

All cosine similarity values are normalized from the raw range [-1.0, 1.0] to [0.0, 1.0]:

```
normalized = (raw_cosine + 1.0) / 2.0
```

Under this normalization:
- 1.0 = identical direction
- 0.5 = orthogonal (was 0.0 in raw cosine)
- 0.0 = opposite direction

Applied in `cosine_similarity()` (file: `distance.rs`). The raw variant `cosine_similarity_raw()` returns [-1.0, 1.0] without this conversion.

### 14.2 NaN Safety (AP-10)

All similarity functions return 0.0 for degenerate inputs (zero-magnitude vectors, empty vectors, dimension mismatches). No NaN or Infinity values are ever returned.

### 14.3 Score Clamping

`PerSpaceScores::set_score()` clamps all scores to [0.0, 1.0]. `SimilarityResult::with_relevance()` clamps the relevance score to [0.0, 1.0].

---

## 15. Normalization Strategies (Storage Layer)

The storage-layer multi-embedder search (file: `crates/context-graph-storage/src/teleological/search/multi/types.rs`) supports four normalization strategies:

| Strategy | Formula | Description |
|----------|---------|-------------|
| `None` (default) | Raw scores | When all embedders use same metric |
| `MinMax` | `(score - min) / (max - min)` | Scales to [0, 1] within each result set |
| `ZScore` | `(score - mean) / stddev` | Centers around 0, unit variance |
| `RankNorm` | `1 / rank` | Rank 1 = 1.0, rank 2 = 0.5, rank 3 = 0.33, etc. |

And four aggregation strategies for combining scores across embedders:

| Strategy | Formula | Description |
|----------|---------|-------------|
| `Max` (default) | `max(score_i)` | Any strong signal suffices |
| `Sum` | `SUM(score_i)` | Rewards multi-embedder consensus |
| `Mean` | `mean(score_i)` | Balanced signal/occurrence |
| `WeightedSum` | `SUM(weight_i * score_i)` | Per-embedder weight control |

---

## 16. Cross-References

- **Training export bundle** (all 13 embeddings + 78 cross-correlations + 6 group alignments + temporal labels): See `docs2/contextgraphspecs/05_embedding_subsystem.md` Section 17. The `export_training_corpus` MCP tool (`crates/context-graph-mcp/src/handlers/tools/training_tools.rs:62`) packages the outputs of every embedder described in Sections 1-9 above into a bincode `TrainingRecord` keyed by memory UUID, persisted to `CF_TRAINING_RECORDS`. Cross-correlations use the same synergy matrix consumed by the `synergy_weights` search option (Section 13.2).
