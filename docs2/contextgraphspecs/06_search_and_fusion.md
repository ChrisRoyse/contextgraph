# Search and Fusion

**Source files covered:**
- `crates/context-graph-core/src/fusion/mod.rs` — `FusionStrategy`, `weighted_rrf`, `score_weighted_rrf`, `weighted_sum`, `fuse_rankings`, `normalize_minmax`, `RRF_K`, `EmbedderRanking`, `FusedResult`
- `crates/context-graph-core/src/retrieval/aggregation.rs` — `AggregationStrategy::{RRF, WeightedAverage, MaxPooling}`, `aggregate_rrf`, `aggregate_rrf_weighted`, `aggregate_rrf_additive`, `rrf_contribution`, `CombinedResult`
- `crates/context-graph-core/src/retrieval/multi_space.rs` — `MultiSpaceSimilarity`, `compute_relevance_score`, `compute_weighted_similarity`
- `crates/context-graph-core/src/retrieval/distance.rs` — `cosine_similarity`, `cosine_similarity_raw`, `max_sim`, `jaccard_similarity`, `compute_similarity_for_space`
- `crates/context-graph-core/src/retrieval/divergence.rs` — `DIVERGENCE_SPACES` (7 embedders), `DivergenceAlert`, `DivergenceSeverity`, `MAX_SUMMARY_LEN = 100`
- `crates/context-graph-core/src/retrieval/sparse_index.rs` — `SparseInvertedIndex` (E6/E13 stage-1 recall)
- `crates/context-graph-core/src/retrieval/insight_annotation.rs` — `EmbedderCategory`, `compute_perspective_coverage`, `MissingPerspective`
- `crates/context-graph-core/src/weights/mod.rs` — `WEIGHT_PROFILES` (20 profiles), `get_weight_profile`, `get_effective_weight_profile`, `apply_e11_disable`, `select_state_conditioned_weight_profile`, `validate_weights`, `space_name`, `E11_ENTITY_ENABLED`, `TEMPORAL_EMBEDDERS_ENABLED`
- `crates/context-graph-core/src/causal/asymmetric.rs` — `apply_causal_gate`, `causal_gate::*` thresholds, `direction_mod::*` constants, `infer_direction_from_fingerprint`, `CausalDirection`
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` — `EmbedderIndex` (18 variants), `all_hnsw()` (16 entries), `uses_hnsw()`, `dimension()`
- `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs` — `search_e1_only_sync`, `search_multi_space_sync`, `search_pipeline_sync`, `search_single_embedder_sync`, `search_filtered_multi_space_sync`, `search_sparse_sync`, `compute_embedder_scores_sync`, `compute_semantic_fusion`, `suppress_degenerate_weights`, `resolve_weights_sync`, `DEFAULT_SEMANTIC_WEIGHTS`, `MULTI_SPACE_MAX_EMBEDDERS = 10`, `STAGE1_RECALL_MULTIPLIER = 10`, `STAGE2_CANDIDATE_MULTIPLIER = 3`
- `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` — `RRF_K: i32 = 60`, `weighted_rrf_fusion_with_scores`, `apply_synergy_boost`, `SYNERGY_BASE_BOOST = 0.1`
- `crates/context-graph-storage/src/teleological/search/single/`, `multi/`, `pipeline/`, `matrix/` — search builders and the standalone 6-stage `RetrievalPipeline`
- `crates/context-graph-storage/src/teleological/search/maxsim.rs` — `MaxSimScorer`, `cosine_similarity_128d`, `compute_maxsim_direct`, `E12_TOKEN_DIM = 128`
- `crates/context-graph-mcp/src/handlers/tools/{memory_tools,embedder_tools,temporal_tools,robustness_tools,keyword_tools,code_tools,causal_relationship_tools,causal_tools,entity_tools,graph_tools}.rs`

Cross-links: [04_database_schema.md](04_database_schema.md), [05_embedding_subsystem.md](05_embedding_subsystem.md), [07_graph_and_causal.md](07_graph_and_causal.md), [09_clustering_topics_and_temporal.md](09_clustering_topics_and_temporal.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Scope

The retrieval subsystem fans a query out across the 14-embedder fingerprint, retrieves per-embedder ranked candidate lists, fuses them into a single ordering, applies the causal gate, performs optional post-retrieval temporal/periodic boosts, and returns scored memories. Four top-level search strategies (`SearchStrategy` in `context_graph_core::traits`) drive dispatch:

| Strategy | Variant | Default consumers | Purpose |
| --- | --- | --- | --- |
| `e1_only` | `SearchStrategy::E1Only` | `search_recent`, `search_periodic`, `search_robust` Phase 1 | Single E1 HNSW lookup; smallest blast radius |
| `multi_space` (default) | `SearchStrategy::MultiSpace` | `search_graph`, `compare_embedder_views`, `search_cross_embedder_anomalies` | Per-embedder HNSW recall → Weighted RRF candidate selection → weighted-cosine final score |
| `pipeline` | `SearchStrategy::Pipeline` | `search_graph` (`strategy="pipeline"`), `search_robust` (`pipeline`), `search_by_expansion` | E13 SPLADE Stage 1 recall → multi-space Stage 2 scoring → optional E12 MaxSim Stage 3 rerank |
| `embedder_first` (filter) | `SearchStrategy::MultiSpace` + `embedder_indices` | `search_by_embedder`, `search_by_tokens`, `search_code` | Routes to single-embedder HNSW or filtered multi-space across the requested embedders |

`embedder_first` is a routing override inside `search_semantic_async` (`search.rs:1741`): when `options.embedder_indices` is non-empty, the strategy field is ignored and the call dispatches to either `search_single_embedder_sync` (single HNSW index) or `search_filtered_multi_space_sync` (multiple HNSW indexes).

The storage crate also exposes a separate, more sophisticated 6-stage `RetrievalPipeline` (`teleological/search/pipeline/`) with stages `SpladeFilter → MatryoshkaAnn → RrfRerank → GraphExpansion → GnnEnhance → MaxSimRerank` (`pipeline/types.rs:215`). The MCP `pipeline` strategy currently dispatches to the simpler 3-stage `search_pipeline_sync` (rocksdb_store path). Both share the same RRF math.

---

## 2. Aggregation strategies

`AggregationStrategy` (`retrieval/aggregation.rs:39`) is the algebraic enum used by storage-layer fusion and multi-space topic scoring.

| Variant | Formula | Notes |
| --- | --- | --- |
| `RRF { k }` | `RRF(d) = Σᵢ 1 / (k + rankᵢ + 1)` | Default with `k = similarity::RRF_K = 60.0`. Ranks are 0-indexed; `+1` produces the standard 1-indexed RRF denominator. `aggregate(matches)` panics — use `aggregate_rrf(ranked_lists, k)`. |
| `WeightedAverage { weights, require_all }` | `Σ wᵢ·simᵢ / Σ wᵢ` | Returns `0.0` when `require_all=true` and matches < `NUM_EMBEDDERS = 14`. |
| `MaxPooling` | `max(simᵢ)` | Returns `0.0` for empty input. |

Helper functions on `AggregationStrategy`:

| Helper | Behavior |
| --- | --- |
| `aggregate_rrf(ranked_lists, k)` | Plain weighted-equal RRF over `(space_index, Vec<Uuid>)` ranked lists. |
| `aggregate_rrf_weighted(ranked_lists, k, weights)` | Per-space-weighted RRF. |
| `aggregate_rrf_additive(ranked_lists, k, unique_boost)` | Tracks contributions, marks single-embedder finds as `unique_contribution=true`, multiplies their score by `1 + unique_boost` (default 0.10), produces `insight_annotation` strings ("Blind-spot discovery: Only E11_Entity found this result", "Strong consensus: 10/14 embedders agree"). Sorts by `boosted_score`. |
| `rrf_contribution(rank, k)` | The single canonical formula: `1 / (k + rank + 1)`. CORE-M5 — all RRF code routes through this. |

`CombinedResult` is the additive-RRF row: `{ memory_id, rrf_score, boosted_score, found_by: HashSet<usize>, primary_embedder, unique_contribution, best_rank, insight_annotation }`. `primary_embedder_name()` maps slot indices 0–13 to `E1_Semantic … E14_BgeM3Dense`.

---

## 3. Weighted RRF (the core fusion crate)

`crates/context-graph-core/src/fusion/mod.rs` is the public RRF surface.

| Symbol | Purpose |
| --- | --- |
| `RRF_K: f32 = 60.0` | Core-side RRF constant. |
| `FusionStrategy::WeightedSum` | Legacy `Σ wᵢ·simᵢ`. |
| `FusionStrategy::WeightedRRF` (default) | `Σ wᵢ / (k + rank + 1)`. |
| `FusionStrategy::ScoreWeightedRRF` | For embedders in the score-weighted set: `wᵢ · simᵢ / (k + rank + 1)`; all others fall back to plain Weighted RRF. `fuse_rankings(.., FusionStrategy::ScoreWeightedRRF, top_k)` hard-codes `["E5"]` as the score-weighted set so E5 magnitude is preserved. |
| `weighted_rrf(rankings, top_k)` | Wrapper: `score_weighted_rrf(rankings, &[], top_k)`. |
| `weighted_sum(rankings, top_k)` | Legacy weighted sum (skips zero-weight embedders). |
| `normalize_minmax(scores)` | In-place [0, 1] min-max rescale; identical inputs collapse to `1.0` (not `0.0`) so downstream RRF still treats the slot as live. |
| `EmbedderRanking { embedder_name, weight, ranked_docs }` | One embedder's contribution to fusion. `EmbedderRanking::from_index(i, w, docs)` formats the name as `E{i+1}`. |
| `FusedResult { doc_id, fused_score, contributing_embedders }` | RRF output row. |

Sort order (`weighted_rrf` and `weighted_sum`): descending by `fused_score`; NaN sorts last (STOR-L3) to keep ordering deterministic on degenerate inputs. `score_weighted_rrf` skips any ranking with `weight <= 0.0` (`fusion/mod.rs:174`) and routes contributions through `AggregationStrategy::rrf_contribution` (CORE-M5 — single canonical formula).

`crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` provides parallel storage-side helpers:

| Symbol | Purpose |
| --- | --- |
| `RRF_K: i32 = 60` | Storage-side RRF constant (i32 because the storage helpers were written before the core helpers consolidated on f32). |
| `weighted_rrf_fusion(result_sets, k)` | Test-only (`#[cfg(test)]`) plain weighted RRF. |
| `weighted_rrf_fusion_with_scores(result_sets, k)` | Used by the causal-direction search; tracks per-embedder similarity in addition to RRF. |
| `apply_synergy_boost(results, &synergy_weights)` | Adds `synergy_weights[i] · synergy_weights[j] · SYNERGY_BASE_BOOST (0.1)` for every pair of contributing embedders, then re-sorts. Currently `#[allow(dead_code)]`. |
| `compute_consensus(id, sets, k)` | Test-only; `count(id ∈ top_k of any set) / total_sets`. |

---

## 4. SearchStrategy dispatch (storage layer)

`RocksDbTeleologicalStore::search_semantic_async` (`search.rs:1741`) is the single entry point. Pseudocode:

```text
if options.embedder_indices is non-empty:
    if len == 1: search_single_embedder_sync(idx)
    else:        search_filtered_multi_space_sync(indices)
else match options.strategy:
    E1Only     -> search_e1_only_sync
    MultiSpace -> search_multi_space_sync
    Pipeline   -> search_pipeline_sync (logs E12-MaxSim warning, runs Stage 1+2;
                  Stage 3 fires only when options.enable_rerank=true and the query has E12 tokens)
post: time_window filter, full temporal boost (apply_full_temporal_boosts) if temporal_options.has_any_boost()
```

Concurrency: each branch is moved to a `tokio::task::spawn_blocking` closure; `query` is wrapped in `Arc` (P3) and `soft_deleted` / `db` / `index_registry` use `Arc<DashMap>` / `Arc<DB>` / `Arc<Registry>` clones (P5) to avoid copying ~63 KB fingerprints or 5–478 KB `HashMap`s.

### 4.1 E1Only (`search_e1_only_sync`, `search.rs:492`)

| Step | Action |
| --- | --- |
| 1 | Look up `EmbedderIndex::E1Semantic` HNSW. |
| 2 | Search with `query.e1_semantic`, `k = max(top_k * 2, 20)`. |
| 3 | Convert HNSW distance → similarity via `hnsw_distance_to_similarity(d) = (1 - d/2).clamp(0, 1)` (HNSW returns squared L2 on L2-normalized vectors). |
| 4 | Filter soft-deleted (`!options.include_deleted && DashMap::contains_key`). |
| 5 | For each survivor, deserialize the fingerprint and compute the full 14-element `embedder_scores` array via `compute_embedder_scores_sync`. |
| 6 | Filter by `min_similarity`, sort descending, truncate to `top_k`. |

### 4.2 MultiSpace (`search_multi_space_sync`, `search.rs:554`)

Default for `search_graph`. Always queries E1; queries E5/E7/E8/E9/E10 unconditionally; gates E2/E3/E4 on `weights[idx] > 0.0`. E11 is gated by `E11_ENTITY_ENABLED` (compile-time constant in `weights/mod.rs:48` — currently `true`). E14 is wired into `compute_embedder_scores_sync` but does not currently participate in MultiSpace HNSW recall — its score is computed at fusion time only.

| Step | Action |
| --- | --- |
| 1 | `resolve_weights_sync(options)` — see §5.3. |
| 2 | Per-embedder HNSW search, `k = max(top_k * 3, 50)`. **Direction-aware E5**: `Cause` queries `E5CausalEffect` with `query.get_e5_as_cause()`; `Effect` queries `E5CausalCause` with `query.get_e5_as_effect()`; `Unknown` falls back to `E5Causal` with `query.e5_active_vector()` (cross-pair retrieval per STOR-L5). Failed HNSW searches are logged into `degraded_embedders` (SEARCH-4). |
| 3 | Per ranking added only when `weights[idx] > 0.0` AND non-empty (zero-weight embedders are skipped before fusion). |
| 4 | `fuse_rankings(rankings, options.fusion_strategy, top_k * 2)` — selects the candidate set. |
| 5 | Pass 1: deserialize each candidate's fingerprint, compute `embedder_scores` via `compute_embedder_scores_sync`. |
| 6 | `suppress_degenerate_weights(all_scores, &weights)` — see §7. |
| 7 | Pass 2: final user-facing score = `compute_semantic_fusion(scores, adjusted_weights)` = weighted average over embedders with `weight > 0.0` AND `score >= 0.0` (the `>= 0.0` guard preserves anti-correlated `0.0` after SRC-3 normalization while filtering out the `-1.0` no-signal sentinel). |
| 8 | Sort by `similarity` descending, truncate to `top_k`. |

The fusion score is **weighted cosine in [0, 1]**, NOT an RRF score. RRF chooses the candidate set; the final number is comparable across queries.

`MULTI_SPACE_MAX_EMBEDDERS = 10` (`search.rs:98`) is a `Vec::with_capacity` preallocation hint, not a limit — storage iterates all 14 embedder slots via weight gating.

### 4.3 Pipeline (`search_pipeline_sync`, `search.rs:951`)

| Stage | Action | Source |
| --- | --- | --- |
| Stage 1 — sparse + dense recall | E13 SPLADE inverted index (`search_sparse_sync`) → E1 HNSW → E5 (direction-aware, `recall_k/2`) → E7 (`recall_k/2`) → E8 (`recall_k/2`) → E11 (gated, `recall_k/2`) → E9 (`recall_k/2`) → weight-gated E2/E3/E4 (`recall_k/2`). All extend `candidate_ids: HashSet<Uuid>`. `recall_k = top_k * STAGE1_RECALL_MULTIPLIER (10)`. | `search.rs:951–1194` |
| Stage 2 — multi-space scoring | Compute `embedder_scores` per surviving candidate; `suppress_degenerate_weights`; route by `options.fusion_strategy`. For `WeightedSum`: per-candidate `compute_semantic_fusion`. For `WeightedRRF` / `ScoreWeightedRRF`: build per-embedder rankings dynamically from `adjusted_weights > 0.0` excluding E12 (rerank-only), `fuse_rankings(...)`, then **re-score** each fused doc with `compute_semantic_fusion` so the user-facing similarity is a weighted cosine in [0, 1] (PIPE-1: RRF scores ~0.003 are not directly comparable). `stage2_k = top_k * STAGE2_CANDIDATE_MULTIPLIER (3)`. | `search.rs:1196–1311` |
| Stage 3 — E12 ColBERT MaxSim rerank | When `options.enable_rerank=true` AND `query.e12_late_interaction` is non-empty: `final = (1 - rerank_weight) · stage2 + rerank_weight · maxsim`, `maxsim = mean over query tokens of max cosine to any candidate token`. Re-sort. | `search.rs:1318–1347` |

E13 SPLADE Stage 1 (`search_sparse_sync`, `search.rs:1474`) uses BM25-IDF: `idf = ln((N - df + 0.5)/(df + 0.5) + 1)`, `score = Σ query_weight · idf`. `total_doc_count` is read once via an atomic counter (P1 fix). Posting lists are batch-fetched via `multi_get_cf` (P8 fix — for 100-term queries, 100 sequential reads collapse to 1 batch).

### 4.4 Embedder-first (single / filtered)

| Mode | Function | Behavior |
| --- | --- | --- |
| Single | `search_single_embedder_sync(embedder_idx)` (`search.rs:158`) | Looks up the requested HNSW index via `EmbedderIndex::from_index`. Errors with `ValidationError` if `!embedder.uses_hnsw()` (E6=5, E12=11, E13=12 are not HNSW-backed). Returns up to `top_k` results with full `embedder_scores`. |
| Multi-filter | `search_filtered_multi_space_sync(embedder_indices)` (`search.rs:255`) | MultiSpace algorithm but only iterates the listed indices. Skips non-HNSW embedders with a debug log. Same suppression + weighted-cosine scoring as 4.2. |

### 4.5 Query → query-vector mapping

`get_query_vector_for_embedder` (`search.rs:131`) is the single source of truth.

| idx | Embedder | Source | HNSW? |
| --- | --- | --- | --- |
| 0 | E1 Semantic | `query.e1_semantic` | yes |
| 1 | E2 Temporal Recent | `query.e2_temporal_recent` | yes |
| 2 | E3 Temporal Periodic | `query.e3_temporal_periodic` | yes |
| 3 | E4 Temporal Positional | `query.e4_temporal_positional` | yes |
| 4 | E5 Causal | `query.e5_active_vector()` (or directional override; see §4.2) | yes (E5Causal + E5CausalCause + E5CausalEffect) |
| 5 | E6 Sparse | — | **no** (inverted index) |
| 6 | E7 Code | `query.e7_code` | yes |
| 7 | E8 Graph | `query.e8_active_vector()` (source-side default) | yes |
| 8 | E9 HDC | `query.e9_hdc` | yes |
| 9 | E10 Multimodal | `query.e10_active_vector()` (paraphrase-side default) | yes (E10Multimodal legacy + E10MultimodalParaphrase + E10MultimodalContext) |
| 10 | E11 Entity | `query.e11_entity` | yes (gated by `E11_ENTITY_ENABLED`) |
| 11 | E12 LateInteraction | — | **no** (MaxSim only) |
| 12 | E13 SPLADE | — | **no** (sparse inverted) |
| 13 | E14 BgeM3Dense | `query.e14_bge_m3_dense` | yes |

### 4.6 Storage HNSW indexes — `EmbedderIndex::all_hnsw()` returns 16

`indexes/hnsw_config/embedder.rs:186`:

| # | Variant | Dim | Metric |
| --- | --- | --- | --- |
| 1 | `E1Semantic` | 1024 | Cosine |
| 2 | `E1Matryoshka128` | 128 | Cosine — Stage 2 fast filter; PipelineBuilder skips when no matryoshka query is supplied |
| 3 | `E2TemporalRecent` | 512 | Cosine |
| 4 | `E3TemporalPeriodic` | 512 | Cosine |
| 5 | `E4TemporalPositional` | 512 | Cosine |
| 6 | `E5Causal` | 768 | AsymmetricCosine — legacy active-vector index |
| 7 | `E5CausalCause` | 768 | AsymmetricCosine — Cause-vector index (ARCH-15) |
| 8 | `E5CausalEffect` | 768 | AsymmetricCosine — Effect-vector index (ARCH-15) |
| 9 | `E7Code` | 1536 | Cosine |
| 10 | `E8Graph` | 1024 | Cosine |
| 11 | `E9HDC` | 1024 | Cosine |
| 12 | `E10Multimodal` | 768 | AsymmetricCosine — legacy active-vector index |
| 13 | `E10MultimodalParaphrase` | 768 | AsymmetricCosine — paraphrase index (ARCH-15) |
| 14 | `E10MultimodalContext` | 768 | AsymmetricCosine — context index (ARCH-15) |
| 15 | `E11Entity` | 768 | Cosine |
| 16 | `E14BgeM3Dense` | 1024 | Cosine |

E6 (`E6Sparse`), E12 (`E12LateInteraction`), and E13 (`E13Splade`) are excluded — they use inverted indexes / token-level MaxSim, not HNSW.

---

## 5. Weight profiles

`WEIGHT_PROFILES: &[(&str, [f32; NUM_EMBEDDERS])]` lives at `crates/context-graph-core/src/weights/mod.rs:171`. **20 named profiles**, each a 14-slot weight array. The slots are `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14]`. All profiles sum to 1.0 ± 0.01 (validated by `validate_weights`); E12 and E13 are 0.0 in every semantic profile per ARCH-13 (pipeline-stage-only).

| # | Profile | Primary | E14 | Use case |
| --- | --- | --- | --- | --- |
| 1 | `semantic_search` | E1=0.33, E7=0.20, E5=0.15, E10=0.15, E6=0.05, E8=0.05, E11=0.05, E9=0.02 | 0.0 | General queries (default named profile). The storage-layer `DEFAULT_SEMANTIC_WEIGHTS` is a manually-redistributed mirror used when no named profile is set. |
| 2 | `causal_reasoning` | E1=0.40, E7=0.15, E5=0.10, E8=0.10, E10=0.10, E11=0.10, E6=0.05 | 0.0 | "Why" questions; E5 demoted to binary-gate signal only (see §6). |
| 3 | `code_search` | E7=0.40, E1=0.20, E5=0.10, E6=0.10, E10=0.10, E11=0.10 | 0.0 | Programming queries; E7 primary. |
| 4 | `fact_checking` | E11=0.40, E1=0.15, E5=0.15, E6=0.15, E7=0.05, E8=0.05, E10=0.05 | 0.0 | Named-entity-anchored verification. |
| 5 | `graph_reasoning` | E8=0.40, E11=0.20, E1=0.15, E5=0.10, E6=0.10, E10=0.05 | 0.0 | Structural connectivity ("what imports X?", "what connects to X?"). |
| 6 | `temporal_navigation` | E2=E3=E4=0.23, E1=0.13, E5/E7/E9/E10=0.03 each, E6/E8/E11=0.02 each | 0.0 | Explicit time-based queries; E2/E3/E4 take fusion plurality. |
| 7 | `sequence_navigation` | E4=0.55, E1=0.20, E2=0.05, E13=0.02, E5/E7/E9/E10=0.03 each, E6/E8/E11=0.02 each | 0.0 | Conversation/document ordering ("what came before X?"). |
| 8 | `conversation_history` | E4=0.35, E1=0.30, E5=0.10, E2=0.05, E7=0.05, E10=0.05, E11=0.03, E6=0.03, E13=0.02 | 0.0 | Session continuity (E4 + E1 backbone). |
| 9 | `category_weighted` | (1/6.5)≈0.154 for E1/E5/E6/E7/E10; (0.5/6.5)≈0.077 for E8/E9/E11; 0.0 for E2-E4/E12/E13 | 0.0 | Constitution-compliant (5 SEMANTIC × 1.0 + 2 RELATIONAL × 0.5 + 1 STRUCTURAL × 0.5). |
| 10 | `typo_tolerant` | E1=0.30, E9=0.18, E7=0.15, E10=0.12, E5=0.10, E6=0.07, E11=0.05, E8=0.03 | 0.0 | Noisy queries; E9 elevated for HDC robustness. |
| 11 | `pipeline_stage1_recall` | E13=0.25, E6=0.25, E1=0.20, E7=0.10, E5=0.05, E9=0.05, E10=0.05, E11=0.05 | 0.0 | E13/E6 sparse-recall biased. |
| 12 | `pipeline_stage2_scoring` | E1=0.50, E7=0.15, E5=0.12, E10=0.08, E6=0.05, E11=0.05, E8=0.03, E9=0.02 | 0.0 | E1 dense-scoring biased. |
| 13 | `pipeline_full` | E1=0.40, E7=0.15, E5=0.10, E6=0.10, E10=0.08, E13=0.07, E11=0.05, E8=0.03, E9=0.02 | 0.0 | Stage-balanced. |
| 14 | `balanced` | 0.0715 each for E1–E13, E14=0.071 | 0.071 | Uniform baseline (sum ≈ 0.9995 — within `validate_weights` tolerance). |
| 15 | `multilingual_search` | E14=0.40, E1=0.20, E10=0.15, E5/E6/E7/E11=0.05 each, E8=0.03, E9=0.02 | **0.40** | Cross-lingual retrieval. |
| 16 | `long_context` | E14=0.45, E1=0.15, E7=0.10, E10=0.10, E5/E6/E8/E11=0.05 each | **0.45** | 8192-token BGE-M3 context. |
| 17 | `translation_finder` | E14=0.70, E10=0.20, E1=0.05, E11=0.05 | **0.70** | Cross-lingual paraphrase / translation pairs. |
| 18 | `affect_repair` | E1=0.30, E10=0.20, E14=0.20, E9=0.10, E5/E6/E7/E8=0.05 each | 0.20 | Stress-floor routing target (broad/analogy-friendly). |
| 19 | `affect_priming` | E1=0.25, E10=0.25, E14=0.25, E5/E6/E7/E8/E9=0.05 each | 0.25 | Low-valence routing target (paraphrase-biased). |
| 20 | `affect_neutral` | E1=0.25, E6=0.20, E5=0.15, E14=0.15, E10=0.10, E7/E8/E9=0.05 each | 0.15 | High-stress + arousal routing target (literal/factual). |

Profiles 18–20 are reachable via state-conditioned routing only (§5.2).

### 5.1 Resolution helpers

| Function | Behavior |
| --- | --- |
| `get_weight_profile(name)` | Raw lookup; returns `WeightProfileError::UnknownProfile { name, available }` on miss. |
| `get_effective_weight_profile(name)` | Same as above but applies `apply_e11_disable` when `E11_ENTITY_ENABLED=false`. |
| `apply_e11_disable(&mut weights)` | Zeroes `weights[10]`, scales remaining nonzero weights by `(remaining_sum + e11_weight) / remaining_sum` so the sum stays ≈ 1.0. No-op when E11 weight is already 0. |
| `validate_weights(weights)` | Each weight in [0, 1]; sum ≈ 1.0 (`±0.01`); fail-fast `OutOfRange` / `InvalidSum`. |
| `space_name(idx)` | `0→E1_Semantic` … `13→E14_BgeM3Dense`, `_→Unknown`. |

### 5.2 State-conditioned routing — `select_state_conditioned_weight_profile`

Driven by `LearnerStateComponents` (validated for finiteness and bounds). Highest-priority condition wins.

| Condition (in order) | Selected profile | `reason` |
| --- | --- | --- |
| `stress_floor >= 0.80` OR (`stress_floor >= 0.60` AND `arousal >= 0.65`) | `affect_neutral` | `stress_floor/arousal above neutral-routing threshold` |
| `stress_floor >= 0.60` | `affect_repair` | `stress_floor above repair-routing threshold` |
| `valence <= -0.30` | `affect_priming` | `valence below priming-routing threshold` |
| else | base profile (default `semantic_search`) | `learner state within base-profile band` |

Both base and selected profiles are looked up at routing time, so `InvalidBaseProfile` and `InvalidLearnerState` errors fail fast. Exposed via MCP `resolve_learner_retrieval_policy`.

### 5.3 Custom weights and exclusions — `resolve_weights_sync`

Priority (`search.rs:1388`):

1. `options.custom_weights` if set (validated by `validate_weights`).
2. Else `get_effective_weight_profile(options.weight_profile)` if set (returns `ValidationError` on unknown profile).
3. Else `DEFAULT_SEMANTIC_WEIGHTS` (`search.rs:1569`).
4. After step 1–3: if `!E11_ENTITY_ENABLED`, run `apply_e11_disable`.
5. If `options.exclude_embedders` is non-empty: zero each excluded slot, then renormalize. Out-of-range index returns `ValidationError`. Excluding all embedders (sum = 0.0) returns `ValidationError`.

Custom weight profiles persisted via `create_weight_profile` are capped at `MAX_CUSTOM_PROFILES = 1_000` per database (`embedder_tools.rs:34`); the cap rejects new names but allows in-place updates.

---

## 6. Causal gate

`crates/context-graph-core/src/causal/asymmetric.rs` — gate constants and `apply_causal_gate(original, e5_score, is_causal_query)`.

| Constant | Value | Meaning | Source |
| --- | --- | --- | --- |
| `causal_gate::CAUSAL_THRESHOLD` | `0.04` | E5 ≥ this → "definitely causal" → boost | `asymmetric.rs:61` |
| `causal_gate::NON_CAUSAL_THRESHOLD` | `0.008` | E5 ≤ this → "definitely non-causal" → demote | `asymmetric.rs:64` |
| `causal_gate::CAUSAL_BOOST` | `1.10` | Multiplier applied to `original_score` when boosted | `asymmetric.rs:68` |
| `causal_gate::NON_CAUSAL_DEMOTION` | `0.85` | Multiplier applied to `original_score` when demoted | `asymmetric.rs:72` |
| `direction_mod::CAUSE_TO_EFFECT` | `1.2` | Forward inference amplification | `asymmetric.rs:42` |
| `direction_mod::EFFECT_TO_CAUSE` | `0.8` | Backward inference dampening | `asymmetric.rs:44` |
| `direction_mod::SAME_DIRECTION` | `1.0` | No modification | `asymmetric.rs:46` |
| `direction_mod::UNKNOWN` | `1.0` | No modification when direction is unset | `asymmetric.rs:48` |

`apply_causal_gate` short-circuits when `is_causal_query=false` (returns the original score). The ambiguous zone `(NON_CAUSAL_THRESHOLD, CAUSAL_THRESHOLD)` is a no-op.

Calibration: post-training E5 score distribution (GPU benchmark 2026-02-14, prefix-aligned retrain) — causal mean = 0.138, non-causal mean = 0.016, gap = 0.121. Thresholds are calibrated for the prefix-aligned LoRA + projection model on `nomic-embed-text-v1.5` with `"search_document:"` prefix used during BOTH training and inference. Untrained models score 0.93–0.98 on every input, so the gate becomes a no-op (boosts everything equally).

The handler MUST set `CausalDirection::Cause` / `Effect` on `TeleologicalSearchOptions` before calling search when E5 is selected — `compute_embedder_scores_sync` returns `-1.0` (no-signal sentinel) for `CausalDirection::Unknown` per AP-77 + CORE-H1, and `suppress_degenerate_weights` then zero-weights the slot.

`infer_direction_from_fingerprint(fp)` (`asymmetric.rs:120`) chooses Cause / Effect / Unknown by comparing the **component variance** of `e5_causal_as_cause` vs `e5_causal_as_effect`. Since both are L2-normalized, magnitudes are useless; higher variance = more peaked projection = the embedded direction. A 5% variance gap is required.

---

## 7. Degenerate-weight suppression

`suppress_degenerate_weights(all_scores: &[[f32; 14]], weights: &[f32; 14]) -> [f32; 14]` — `search.rs:1625`.

| Constant | Value | Purpose |
| --- | --- | --- |
| `MIN_VARIANCE` | `0.001` | Below this, the embedder is "noise, not signal" |
| `SUPPRESSION_FACTOR` | `0.25` | Multiplier applied to that embedder's weight |

Algorithm (per slot with `weights[idx] > 0.0`):

1. Skip entirely if fewer than 3 candidates (variance is unreliable).
2. **Zero-out** the slot if **every** candidate has score `< 0.0` (the `-1.0` no-signal sentinel — currently emitted by E5 with `CausalDirection::Unknown` per AP-77 / CORE-H1).
3. Otherwise compute mean+variance via Welford's online algorithm (P2), skipping sentinel `< 0.0` entries.
4. Use Bessel's correction (SRC-8 — denominator `count - 1`) for an unbiased sample variance.
5. If `count < 3` after sentinel filtering, skip (no data).
6. If `variance < MIN_VARIANCE`, multiply that embedder's weight by `0.25`.

`compute_semantic_fusion` (`search.rs:1691`) accepts `score >= 0.0` (not `> 0.0`). After SRC-3 normalization `(raw + 1) / 2`, anti-correlated cosine becomes `0.0` and is **valid signal**. Filtering zeros would drop legitimate anti-correlated matches. Only the explicit `-1.0` sentinel is excluded. Fallback when all weights collapse: `scores[0].max(0.0)` (E1 score, clamped to 0).

`MULTI_SPACE_MAX_EMBEDDERS = 10`, `STAGE1_RECALL_MULTIPLIER = 10`, `STAGE2_CANDIDATE_MULTIPLIER = 3` (storage capacity hints / fan-out multipliers).

---

## 8. SRC-3 cosine normalization

The codebase convention is `(raw_cos + 1) / 2`, mapping raw cosine `[-1, 1]` to `[0, 1]`:

- `1.0 → 1.0` (identical)
- `0.0 → 0.5` (orthogonal)
- `-1.0 → 0.0` (maximally anti-correlated)

`retrieval/distance.rs::cosine_similarity` (`distance.rs:32`) is the canonical implementation; `cosine_similarity_raw` (`:66`) returns the unnormalized `[-1, 1]` form for callers that need raw cosine (CORE-M3).

`compute_embedder_scores_sync` (`search.rs:422`) applies SRC-3 explicitly for E6 (`(raw + 1) / 2`, `:467`) and E13 (`(raw + 1) / 2`, `:480`). Dense embedders use `compute_cosine_similarity` (`helpers.rs`) which is already `[0, 1]`. E5 returns the asymmetric similarity (already in `[0, 1]` after `direction_mod` × normalized cosine), or `-1.0` sentinel if `Unknown`. E12 MaxSim is already `[0, 1]` (mean over per-token max normalized cosines). E7's query-type-aware path (`compute_e7_similarity_with_query_type`) returns `[0, 1]`. E8 and E10 take the max over both directional pairs (source-vs-target, paraphrase-vs-context) — STOR-H1.

`hnsw_distance_to_similarity(d) = (1 - d/2).clamp(0, 1)` undoes squared-L2 on L2-normalized vectors so it's directly comparable to cosine.

Sparse pieces (`SparseInvertedIndex`, `inverted_index.rs`) require sorted posting lists on **both** read and write paths (Audit #14): the remove path used to deserialize raw without sorting while the update path sorted, so concurrent ops corrupted the lists.

---

## 9. MaxPooling and MaxSim

**MaxPooling** is a fusion variant in `AggregationStrategy::MaxPooling` — `max(simᵢ)` across embedder scores. It returns `0.0` on empty input. Used for topic scoring when a single strong embedder should win.

**MaxSim** is the ColBERT late-interaction primitive used for E12 reranking:

```text
MaxSim(Q, D) = (1 / |Q|) × Σᵢ max_j cos(qᵢ, dⱼ)
```

Implementation: `crates/context-graph-storage/src/teleological/search/maxsim.rs`. `cosine_similarity_128d` uses AVX2 SIMD when available (8 f32 per instruction, fused multiply-add) and scalar fallback otherwise. `MaxSimScorer::score_batch` runs over candidate doc IDs in parallel via Rayon. `E12_TOKEN_DIM = 128`.

`max_sim` (`distance.rs:115`) returns the **mean** of per-query-token max cosines (not sum), keeping E12 in `[0, 1]`. Stage 3 interpolation `final = (1 - rerank_weight)·stage2 + rerank_weight·maxsim` preserves the range.

---

## 10. Search MCP tool catalog

| Tool | Strategy | Embedder focus | Rerank | Source |
| --- | --- | --- | --- | --- |
| `search_graph` | configurable (default `multi_space`) | all | optional E12 (`enableRerank`) | `memory_tools.rs` |
| `search_recent` | `e1_only` then post-retrieval E2 boost | E1 + E2 decay (`linear`/`exponential`/`step`) | none | `temporal_tools.rs` |
| `search_periodic` | `e1_only` then post-retrieval E3 boost | E1 + E3 hour-of-day / day-of-week | none | `temporal_tools.rs` |
| `search_robust` | configurable (default `e1_only`); `pipeline` enables E12 rerank | E1 ∪ E9 blind-spot detection | E12 if `pipeline` | `robustness_tools.rs` |
| `search_by_keywords` | `e1_only` (E6 keyword boost post-fusion) | E1 + E6 sparse Jaccard | none | `keyword_tools.rs` |
| `search_by_tokens` | `embedder_first` to E12 (top-K via E1, then MaxSim rerank) | E12 ColBERT MaxSim | implicit | `embedder_tools.rs` |
| `search_by_expansion` | `pipeline` (E13 SPLADE Stage 1) | E13 + E1 stage 2 | optional | `embedder_tools.rs` |
| `search_code` | `multi_space` candidate, then E7 query-type-aware scoring (Code2Code / Text2Code / NonCode auto-detected) | E7 primary, E1/E11 secondary | none | `code_tools.rs` |
| `search_causes` / `search_effects` | direction-aware E5 HNSW + multi-space; gate applied post-fusion | E5 (asymmetric) + E1/E8 | optional rerank | `causal_relationship_tools.rs`, `causal_tools.rs` |
| `search_by_entities` | `embedder_first` E11 | E11 KEPLER | none | `entity_tools.rs` |
| `search_connections` | graph traversal over typed edges + multi-space scoring | E8 graph + per-edge weights | none | `graph_tools.rs` |
| `search_by_embedder` | `embedder_first` (single index) | requested embedder | none | `embedder_tools.rs` |
| `compare_embedder_views` | per-embedder MultiSpace search; aligns 14 ranked lists | all 14 | none | `embedder_tools.rs` |
| `search_cross_embedder_anomalies` | multi-space search + post-filter on `(highEmbedder ≥ highThreshold) AND (lowEmbedder ≤ lowThreshold)` | configurable pair | none | `embedder_tools.rs` |
| `get_embedder_clusters` | per-embedder cluster lookup (no fusion) | one embedder | none | `embedder_tools.rs` |

`search_robust` algorithm (`robustness_tools.rs`): (1) embed query 14×, (2) E1 `top_k * 3` candidates, (3) E9 `top_k * 3` candidates, (4) compute blind-spots = `{ id : E9_score >= e9_discovery_threshold AND E1_score < e1_weakness_threshold }` (defaults 0.08 / 0.5), (5) merge dedup, label each result `Source::E1Match | E9Discovery | Both`.

---

## 11. Insight annotations and divergence detection

`crates/context-graph-core/src/retrieval/insight_annotation.rs` provides:

| Symbol | Purpose |
| --- | --- |
| `EmbedderCategory::{Semantic, Temporal, Relational, Structural, PipelineStage}` | Categorization used to spot missing perspectives. |
| `compute_perspective_coverage(found_by: &HashSet<usize>)` → `PerspectiveCoverage` | `{ semantic_count, temporal_count, relational_count, structural_count, missing: Vec<MissingPerspective> }`. |
| `generate_insight_annotation(combined: &CombinedResult)` | Human-readable string ("Blind-spot discovery", "Strong consensus", category notes). |
| `embedder_names_for_indices(&[usize])` → `Vec<&'static str>` | Maps slot indices to canonical names. |
| `annotate_results(results: &mut [CombinedResult])` | Bulk-annotation pass. |

`crates/context-graph-core/src/retrieval/divergence.rs`:

| Constant | Value | Purpose |
| --- | --- | --- |
| `DIVERGENCE_SPACES` | 7 embedders: E1, E6, E7, E10, E12, E13, E14 | Semantic-only set; explicitly excludes E5 (AP-77 — needs `CausalDirection`), E2/E3/E4 (temporal), E8/E11 (relational), E9 (structural). |
| `MAX_SUMMARY_LEN` | 100 chars | Memory-summary truncation in alerts (UTF-8 safe via `floor_char_boundary`). |
| `DivergenceSeverity::from_score(score)` | `<0.10 → High; 0.10..0.20 → Medium; 0.20..0.30 → Low; >=0.30 not-an-alert` | Lower similarity = higher severity. |

`DivergenceAlert::new(...)` asserts (in both debug AND release per CORE-M3) that the supplied `space` is in `DIVERGENCE_SPACES`. Score is clamped to `[0, 1]`. `DivergenceDetector` (`detector.rs`) computes per-space similarity between current and recent activity (`RecentMemory`) and emits alerts when any space drops below the configured threshold.

---

## 12. Constants summary

| Constant | Value | Source |
| --- | --- | --- |
| `RRF_K` (core, f32) | `60.0` | `fusion/mod.rs:41` |
| `RRF_K` (storage helper, i32) | `60` | `rocksdb_store/fusion.rs:49` |
| `similarity::RRF_K` (default for `AggregationStrategy::default`) | `60.0` | `config/constants` |
| `MULTI_SPACE_MAX_EMBEDDERS` | `10` | `rocksdb_store/search.rs:98` |
| `STAGE1_RECALL_MULTIPLIER` | `10` | `rocksdb_store/search.rs:1598` |
| `STAGE2_CANDIDATE_MULTIPLIER` | `3` | `rocksdb_store/search.rs:1601` |
| `MIN_VARIANCE` (suppression) | `0.001` | `rocksdb_store/search.rs:1626` |
| `SUPPRESSION_FACTOR` | `0.25` | `rocksdb_store/search.rs:1627` |
| `SYNERGY_BASE_BOOST` | `0.10` | `rocksdb_store/fusion.rs:212` |
| `unique_boost` (additive RRF default) | `0.10` | `aggregation::aggregate_rrf_additive` |
| `WEIGHT_PROFILES.len()` | `20` | `weights/mod.rs:171` |
| `NUM_EMBEDDERS` | `14` | `types/fingerprint.rs` |
| `TEMPORAL_EMBEDDERS_ENABLED` | `true` | `weights/mod.rs:42` |
| `E11_ENTITY_ENABLED` | `true` (EXP-2 audit) | `weights/mod.rs:48` |
| `causal_gate::CAUSAL_THRESHOLD` | `0.04` | `causal/asymmetric.rs:61` |
| `causal_gate::NON_CAUSAL_THRESHOLD` | `0.008` | `causal/asymmetric.rs:64` |
| `causal_gate::CAUSAL_BOOST` | `1.10` | `causal/asymmetric.rs:68` |
| `causal_gate::NON_CAUSAL_DEMOTION` | `0.85` | `causal/asymmetric.rs:72` |
| `direction_mod::CAUSE_TO_EFFECT` | `1.2` | `causal/asymmetric.rs:42` |
| `direction_mod::EFFECT_TO_CAUSE` | `0.8` | `causal/asymmetric.rs:44` |
| `MAX_CUSTOM_PROFILES` (per DB) | `1_000` | `embedder_tools.rs:34` |
| `E12_TOKEN_DIM` | `128` | `search/maxsim.rs:49` |
| `MAX_SUMMARY_LEN` (divergence) | `100` chars | `retrieval/divergence.rs:37` |
| `EmbedderIndex::all_hnsw().len()` | `16` | `indexes/hnsw_config/embedder.rs:186` |

---

## 13. Reality notes (what this means in practice)

- **`compute_semantic_fusion` returns weighted cosine, not RRF.** RRF is the candidate selector; the user-facing similarity is the weighted average over per-embedder cosines (after suppression). This applies to MultiSpace AND the WeightedRRF/ScoreWeightedRRF branches of Pipeline (PIPE-1: RRF scores ~0.003 are not directly comparable for `min_similarity` filtering).
- **Direction-aware E5 cross-pair retrieval.** A "cause-seeking" query embeds the cause vector but searches the **effect** index — the goal is "find memories whose effect matches our cause" ("what did this cause?"). Effect-seeking is symmetric. Without it, asymmetric direction signal is lost (STOR-L5).
- **`-1.0` is the "no signal" sentinel.** `0.0` is now valid (anti-correlated after SRC-3). E5 with `CausalDirection::Unknown` is the canonical sentinel emitter (per AP-77 / CORE-H1).
- **`MULTI_SPACE_MAX_EMBEDDERS = 10` is a preallocation hint, not a limit.** Storage iterates all 14 embedder slots via weight gating.
- **Pipeline RRF exclusion is just `[11]` (E12).** E2/E3/E4 participate when weight > 0; E13 is never excluded from Stage 2 RRF.
- **E12 / E13 are pipeline-stage-only in named profiles.** All semantic profiles assign E12=E13=0.0; they enter scoring via Stage 1 recall (E13) and Stage 3 rerank (E12), not fusion (per ARCH-13).
- **Custom weights bypass profile-name resolution.** `custom_weights` overrides `weight_profile`, but `apply_e11_disable` and `exclude_embedders` still apply afterwards.
- **Storage 6-stage pipeline coexists with the simpler 3-stage `search_pipeline_sync`.** `teleological/search/pipeline/types.rs` defines `PipelineStage::{SpladeFilter, MatryoshkaAnn, RrfRerank, GraphExpansion, GnnEnhance, MaxSimRerank}` for the standalone `RetrievalPipeline` builder. The MCP `pipeline` strategy currently routes through `search_pipeline_sync`, the 3-stage E13→E1→E12 implementation.
- **`balanced` profile sums to 0.9995, not 1.0.** Tolerated by `validate_weights` (±0.01).
- **HNSW returns squared L2 on L2-normalized vectors.** `hnsw_distance_to_similarity(d) = (1 - d/2).clamp(0, 1)` recovers cosine-comparable similarity.
- **Hard-delete order matters.** HNSW removal happens AFTER RocksDB commit; if the RocksDB write fails, the index keeps the entry (consistent on retry).
