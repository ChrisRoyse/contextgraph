# Search And Fusion

**Source files covered:**
- `crates/context-graph-core/src/fusion/mod.rs` — `FusionStrategy`, `weighted_rrf`, `score_weighted_rrf`, `weighted_sum`, `normalize_minmax`
- `crates/context-graph-core/src/retrieval/mod.rs` — module re-exports
- `crates/context-graph-core/src/retrieval/aggregation.rs` — `AggregationStrategy::{RRF, WeightedAverage, MaxPooling}`, `aggregate_rrf`, `aggregate_rrf_weighted`, `aggregate_rrf_additive`, `CombinedResult`
- `crates/context-graph-core/src/retrieval/multi_space.rs` — `MultiSpaceSimilarity`, `compute_relevance_score`, `compute_weighted_similarity`
- `crates/context-graph-core/src/retrieval/distance.rs` — `compute_similarity_for_space`, `compute_similarity_for_space_with_direction`, `cosine_similarity`, `max_sim`, `jaccard_similarity`
- `crates/context-graph-core/src/retrieval/divergence.rs` — `DIVERGENCE_SPACES`, `DivergenceAlert`, `DivergenceSeverity`
- `crates/context-graph-core/src/retrieval/sparse_index.rs` — `SparseInvertedIndex` (E6/E13 stage-1 recall)
- `crates/context-graph-core/src/retrieval/insight_annotation.rs` — `compute_perspective_coverage`, `MissingPerspective`, `EmbedderCategory`
- `crates/context-graph-core/src/weights/mod.rs` — `WEIGHT_PROFILES`, `get_effective_weight_profile`, `apply_e11_disable`, `select_state_conditioned_weight_profile`, `validate_weights`, `E11_ENTITY_ENABLED`, `TEMPORAL_EMBEDDERS_ENABLED`
- `crates/context-graph-core/src/causal/asymmetric.rs` — `apply_causal_gate`, `causal_gate::*` thresholds, `direction_mod::*`, `infer_direction_from_fingerprint`
- `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs` — `search_e1_only_sync`, `search_multi_space_sync`, `search_pipeline_sync`, `search_single_embedder_sync`, `search_filtered_multi_space_sync`, `search_sparse_sync`, `compute_embedder_scores_sync`, `compute_semantic_fusion`, `suppress_degenerate_weights`, `resolve_weights_sync`, `DEFAULT_SEMANTIC_WEIGHTS`, `MULTI_SPACE_MAX_EMBEDDERS`, `STAGE1_RECALL_MULTIPLIER`, `STAGE2_CANDIDATE_MULTIPLIER`
- `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` — `weighted_rrf_fusion_with_scores`, `apply_synergy_boost`, `RRF_K` (i32)
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` — `search_graph` dispatch, embedder_scores assembly, perspective annotations, causal gate wiring
- `crates/context-graph-mcp/src/handlers/tools/temporal_tools.rs` — `search_recent`, `search_periodic`
- `crates/context-graph-mcp/src/handlers/tools/robustness_tools.rs` — `search_robust` (E9 blind-spot)
- `crates/context-graph-mcp/src/handlers/tools/keyword_tools.rs` — `search_by_keywords` (E6)
- `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` — `compare_embedder_views`, `search_cross_embedder_anomalies`, `search_by_tokens` (E12), `search_by_expansion` (E13), `get_embedder_clusters`
- `crates/context-graph-mcp/src/handlers/tools/code_tools.rs` — `search_code` (E7)
- `crates/context-graph-mcp/src/handlers/tools/causal_relationship_tools.rs` — `search_causes`, `search_effects`
- `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` — `search_by_entities` (E11)
- `crates/context-graph-mcp/src/handlers/tools/graph_tools.rs` — `search_connections`

Cross-links: [05_embedding_subsystem.md](05_embedding_subsystem.md), [07_graph_and_causal.md](07_graph_and_causal.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [12_clustering_and_topics.md](12_clustering_and_topics.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Scope

The retrieval subsystem fans a query out across the 14-embedder fingerprint, retrieves per-embedder ranked candidate lists, fuses them into a single ordering, applies a causal gate, performs optional post-retrieval temporal/periodic boosts, and returns scored memories. Four top-level search strategies (`SearchStrategy` in `context_graph_core::traits`) drive dispatch:

| Strategy | Variant | Used by | Purpose |
| --- | --- | --- | --- |
| `e1_only` | `SearchStrategy::E1Only` | `search_recent`, `search_periodic`, `search_robust` (Phase 1), `search_by_keywords` Phase 0 | Single-embedder E1 HNSW (smallest blast radius, baseline) |
| `multi_space` (default) | `SearchStrategy::MultiSpace` | `search_graph` (default), `compare_embedder_views`, `search_cross_embedder_anomalies` | Per-embedder HNSW recall + Weighted RRF fusion + weighted-cosine final score |
| `pipeline` | `SearchStrategy::Pipeline` | `search_graph` when `strategy="pipeline"`, `search_robust` when `strategy="pipeline"`, `search_by_expansion` (E13 prefilter) | Stage 1 sparse+dense recall → Stage 2 multi-space scoring → optional Stage 3 E12 MaxSim rerank |
| `embedder_first` (filter mode) | `SearchStrategy::MultiSpace` w/ `embedder_indices` | `search_by_embedder`, `search_by_tokens`, `search_code` candidate phase | Routes to single-embedder HNSW or filtered multi-space across the requested embedders |

`embedder_first` is implemented as a routing override inside `search_semantic_async` (`search.rs:1769`): when `options.embedder_indices` is non-empty, the strategy field is ignored and the call dispatches to either `search_single_embedder_sync` (one HNSW index) or `search_filtered_multi_space_sync` (multiple HNSW indexes).

---

## 2. Aggregation strategies

`AggregationStrategy` (`retrieval/aggregation.rs:39`) — algebraic enum used by storage-layer fusion and multi-space topic scoring.

| Variant | Formula | Notes |
| --- | --- | --- |
| `RRF { k }` | `RRF(d) = Σᵢ 1 / (k + rankᵢ(d) + 1)` | Default; `k` defaults to `similarity::RRF_K = 60`. Ranks are 0-indexed; `+1` produces the standard 1-indexed RRF denominator. Calling `aggregate(matches)` panics — use `aggregate_rrf(ranked_lists, k)`. |
| `WeightedAverage { weights, require_all }` | `Σ wᵢ·simᵢ / Σ wᵢ` | When `require_all=true` and matches < `NUM_EMBEDDERS`, returns `0.0`. |
| `MaxPooling` | `max(simᵢ)` | Empty input returns `0.0`. |

Additional helpers on `AggregationStrategy`:

| Helper | Purpose |
| --- | --- |
| `aggregate_rrf(ranked_lists, k)` → `HashMap<Uuid, f32>` | Plain weighted-equal RRF over `(space_index, Vec<Uuid>)` ranked lists. |
| `aggregate_rrf_weighted(ranked_lists, k, weights)` | Per-space-weighted RRF. |
| `aggregate_rrf_additive(ranked_lists, k, unique_boost)` | Tracks contributions, marks single-embedder finds as `unique_contribution=true`, multiplies their score by `1 + unique_boost` (default 0.10), produces an `insight_annotation` (e.g. `"Blind-spot discovery: Only E11_Entity found this result"` or `"Strong consensus: 10/14 embedders agree"`). Sorts by `boosted_score` descending. |
| `rrf_contribution(rank, k)` | Single-rank contribution `1 / (k + rank + 1)`. |

`CombinedResult` is the additive-RRF row: `{ memory_id, rrf_score, boosted_score, found_by: HashSet<usize>, primary_embedder, unique_contribution, best_rank, insight_annotation }`. `primary_embedder_name()` maps indices 0–13 to `E1_Semantic … E14_BgeM3Dense`.

---

## 3. Weighted RRF (core fusion crate)

`crates/context-graph-core/src/fusion/mod.rs` defines the public RRF surface used by the storage layer:

| Symbol | Purpose |
| --- | --- |
| `RRF_K: f32 = 60.0` | Core-side RRF constant. |
| `FusionStrategy::WeightedSum` | Legacy `Σ wᵢ·simᵢ`. |
| `FusionStrategy::WeightedRRF` (default) | `Σ wᵢ / (k + rank + 1)`. |
| `FusionStrategy::ScoreWeightedRRF` | For embedders in the score-weighted set: `wᵢ · simᵢ / (k + rank + 1)`; all others use plain Weighted RRF. `fuse_rankings(..., FusionStrategy::ScoreWeightedRRF, top_k)` hard-codes `["E5"]` as the score-weighted set so E5 magnitude is preserved. |
| `weighted_rrf(rankings, top_k)` | Wrapper around `score_weighted_rrf(rankings, &[], top_k)`. |
| `weighted_sum(rankings, top_k)` | Legacy weighted-sum fusion (skips zero-weight embedders). |
| `normalize_minmax(scores)` | In-place [0, 1] min-max rescaling; identical inputs collapse to `1.0` (not `0.0`) so downstream RRF still treats the slot as live. |
| `EmbedderRanking { embedder_name, weight, ranked_docs }` | One embedder's contribution to fusion. `EmbedderRanking::from_index(idx, w, docs)` formats the name as `E{idx+1}`. |
| `FusedResult { doc_id, fused_score, contributing_embedders }` | RRF output row. |

Sort order (`weighted_rrf` and `weighted_sum`): descending by `fused_score`; NaN goes last (`STOR-L3`) so deterministic ordering survives in degenerate inputs.

`crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` provides a parallel storage-side helper:

| Symbol | Purpose |
| --- | --- |
| `RRF_K: i32 = 60` | Storage-side RRF constant (i32 because the storage helpers were written before the core helpers consolidated on f32). |
| `weighted_rrf_fusion_with_scores(result_sets, k)` | Used by `causal_relationships.rs` causal-direction search; tracks per-embedder similarity in addition to RRF. |
| `apply_synergy_boost(results, &synergy_weights)` | Adds `synergy_weights[i] · synergy_weights[j] · 0.1` for every pair of contributing embedders; resorts. Currently `#[allow(dead_code)]`. |

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
    Pipeline   -> search_pipeline_sync (logs E12-MaxSim-not-implemented warning, then runs Stage 1+2; Stage 3 fires only when options.enable_rerank=true and query has E12 tokens)
post: time_window filter, full temporal boost (apply_full_temporal_boosts) if temporal_options.has_any_boost()
```

### 4.1 E1Only (`search_e1_only_sync`)

1. Look up `EmbedderIndex::E1Semantic` HNSW.
2. Search with `query.e1_semantic`, `k = max(top_k * 2, 20)`.
3. Convert HNSW distance → similarity via `hnsw_distance_to_similarity` (`(1 - distance/2).clamp(0,1)`).
4. Filter soft-deleted (`!options.include_deleted && DashMap::contains_key`).
5. For each survivor, deserialize fingerprint and compute the full 14-element `embedder_scores` array via `compute_embedder_scores_sync`.
6. Filter by `min_similarity`, sort descending, truncate to `top_k`.

### 4.2 MultiSpace (`search_multi_space_sync`)

Default for `search_graph`. Always queries E1; queries E5/E7/E8/E9/E10/E11 unconditionally; gates E2/E3/E4 on `weights[idx] > 0.0`. E11 is gated by `E11_ENTITY_ENABLED` (compile-time constant — currently `true` for the EXP-2 audit; flip to `false` in `weights/mod.rs:48` to disable retrieval and re-enable redistribution).

| Step | Action |
| --- | --- |
| 1 | `resolve_weights_sync(options)` — see §6. |
| 2 | Per-embedder HNSW search. `k = max(top_k * 3, 50)`. Direction-aware E5: `Cause` direction queries `E5CausalEffect` with `query.get_e5_as_cause()`; `Effect` queries `E5CausalCause` with `query.get_e5_as_effect()`; `Unknown` falls back to `E5Causal` with `query.e5_active_vector()` (cross-pair retrieval per `STOR-L5`). |
| 3 | Per ranking added only when `weights[idx] > 0.0` AND non-empty (zero-weight embedders are skipped before fusion). |
| 4 | `fuse_rankings(rankings, options.fusion_strategy, top_k * 2)` — selects candidate set (RRF-by-rank). |
| 5 | Pass 1: deserialize each candidate's fingerprint, compute `embedder_scores` via `compute_embedder_scores_sync`. |
| 6 | `suppress_degenerate_weights(all_scores, &weights)` — see §7. |
| 7 | Pass 2: final user-facing score = `compute_semantic_fusion(scores, adjusted_weights)` = weighted average over embedders with `weight > 0.0` AND `score >= 0.0` (the `>= 0.0` guard preserves anti-correlated `0.0` after SRC-3 normalization, while filtering out the `-1.0` no-signal sentinel). |
| 8 | Sort by `similarity` descending, truncate to `top_k`. |

The fusion score is **weighted cosine in [0, 1]**, not an RRF score. RRF chooses the candidate set; the final number is comparable across queries.

### 4.3 Pipeline (`search_pipeline_sync`)

| Stage | Action | Source |
| --- | --- | --- |
| Stage 1 — sparse + dense recall | E13 SPLADE inverted index (`search_sparse_sync`) → E1 HNSW → E5 (direction-aware) → E7 → E8 → E9 → E11 (gated) → E2/E3/E4 (weight-gated). All extend `candidate_ids: HashSet<Uuid>`. `recall_k = top_k * STAGE1_RECALL_MULTIPLIER (10)`. | `search.rs:951` |
| Stage 2 — multi-space scoring | Compute `embedder_scores` per surviving candidate; `suppress_degenerate_weights`; route by `options.fusion_strategy`. For `WeightedSum`: per-candidate `compute_semantic_fusion`. For `WeightedRRF` / `ScoreWeightedRRF`: build per-embedder rankings dynamically from `adjusted_weights > 0.0` excluding E12 (rerank-only), `fuse_rankings(...)`, then **re-score** each fused doc with `compute_semantic_fusion` so the user-facing similarity is a weighted cosine in [0, 1] (per `PIPE-1` fix — RRF scores ~0.003 are not directly comparable). `stage2_k = top_k * STAGE2_CANDIDATE_MULTIPLIER (3)`. | `search.rs:1196` |
| Stage 3 — E12 ColBERT MaxSim rerank | When `options.enable_rerank=true` AND `query.e12_late_interaction` is non-empty: `final = (1 - rerank_weight) * stage2 + rerank_weight * maxsim`, where `maxsim = max_sim(query_tokens, candidate_tokens)`. Resort. | `search.rs:1318` |

`E13_SPLADE` Stage 1 uses BM25-IDF: `idf = ln((N - df + 0.5)/(df + 0.5) + 1)`, `score = Σ query_weight · idf`. `total_doc_count` is read once via an atomic counter (P1 fix). Posting lists are batch-fetched via `multi_get_cf` (P8 fix).

### 4.4 Embedder-first (single / filtered)

| Mode | Function | Behavior |
| --- | --- | --- |
| Single | `search_single_embedder_sync(embedder_idx)` | Looks up the requested HNSW index via `EmbedderIndex::from_index`. Errors if `!embedder.uses_hnsw()` (E6=5, E12=11, E13=12 are not HNSW-backed). Returns up to `top_k` results with full `embedder_scores`. |
| Multi-filter | `search_filtered_multi_space_sync(embedder_indices)` | MultiSpace algorithm but only iterates the listed indices. Skips non-HNSW embedders with a debug log. Same suppression + weighted-cosine scoring as 4.2. |

### 4.5 Query → query-vector mapping

`get_query_vector_for_embedder` (`search.rs:131`) is the single source of truth.

| idx | Embedder | Source | HNSW |
| --- | --- | --- | --- |
| 0 | E1 Semantic | `query.e1_semantic` | yes |
| 1 | E2 Temporal Recent | `query.e2_temporal_recent` | yes |
| 2 | E3 Temporal Periodic | `query.e3_temporal_periodic` | yes |
| 3 | E4 Temporal Positional | `query.e4_temporal_positional` | yes |
| 4 | E5 Causal | `query.e5_active_vector()` (cause OR effect — see direction-aware override below) | yes (E5Causal + E5CausalCause + E5CausalEffect dual indexes) |
| 5 | E6 Sparse | — | **no** (inverted index, scoring-only) |
| 6 | E7 Code | `query.e7_code` | yes |
| 7 | E8 Graph | `query.e8_active_vector()` (source-side default) | yes |
| 8 | E9 HDC | `query.e9_hdc` | yes |
| 9 | E10 Multimodal | `query.e10_active_vector()` (paraphrase-side default) | yes (legacy `E10Multimodal`; asymmetric `E10MultimodalParaphrase`/`E10MultimodalContext` exist but not yet wired — Audit-14 STOR-L6) |
| 10 | E11 Entity | `query.e11_entity` | yes (gated by `E11_ENTITY_ENABLED`) |
| 11 | E12 LateInteraction | — | **no** (MaxSim only) |
| 12 | E13 SPLADE | — | **no** (sparse inverted) |
| 13 | E14 BgeM3Dense | `query.e14_bge_m3_dense` | yes |

---

## 5. Weight profiles

`WEIGHT_PROFILES: &[(&str, [f32; 14])]` — `crates/context-graph-core/src/weights/mod.rs:171`.

| Profile | Primary embedders | E14 | Notes |
| --- | --- | --- | --- |
| `semantic_search` | E1=0.33, E7=0.20, E5=0.15, E10=0.15, E6=0.05, E8=0.05, E11=0.05, E9=0.02 | 0.0 | Default for general queries. Storage layer also has `DEFAULT_SEMANTIC_WEIGHTS` (E1=0.35, E5=0.16, E7=0.21, E10=0.16, E6=0.05, E8=0.05, E9=0.02, E11=0.0) — kept manually in sync with the named profile under E11_ENTITY_ENABLED=false redistribution; switching E11 back on requires reverting both. |
| `causal_reasoning` | E1=0.40, E7=0.15, E5=0.10, E8=0.10, E10=0.10, E11=0.10, E6=0.05 | 0.0 | E5 demoted to binary-gate signal only. |
| `code_search` | E7=0.40, E1=0.20, E5=0.10, E6=0.10, E10=0.10, E11=0.10 | 0.0 | E7 primary. |
| `fact_checking` | E11=0.40, E1=0.15, E5=0.15, E6=0.15, E7=0.05, E8=0.05, E10=0.05 | 0.0 | Named-entity-anchored. |
| `graph_reasoning` | E8=0.40, E11=0.20, E1=0.15, E5=0.10, E6=0.10, E10=0.05 | 0.0 | Structural connectivity. |
| `temporal_navigation` | E2=E3=E4=0.23, E1=0.13, E5/E7=0.03 each, others 0.02–0.03 | 0.0 | E2/E3/E4 take fusion plurality. |
| `sequence_navigation` | E4=0.55, E1=0.20, E2=0.05, E13=0.02, others 0.02–0.03 | 0.0 | Conversation ordering. |
| `conversation_history` | E4=0.35, E1=0.30, E5=0.10, E2=0.05, E7=0.05, E10=0.05, E11=0.03, E6=0.03, E13=0.02 | 0.0 | Session continuity. |
| `category_weighted` | (1/6.5)≈0.154 for E1/E5/E6/E7/E10; (0.5/6.5)≈0.077 for E8/E9/E11; 0.0 for E2-E4/E12/E13 | 0.0 | Constitution-weights. |
| `typo_tolerant` | E1=0.30, E9=0.18, E7=0.15, E10=0.12, E5=0.10, E6=0.07, E11=0.05, E8=0.03 | 0.0 | E9 elevated. |
| `pipeline_stage1_recall` | E13=0.25, E6=0.25, E1=0.20, E7=0.10, E5=0.05, E9=0.05, E10=0.05, E11=0.05 | 0.0 | Sparse-recall biased. |
| `pipeline_stage2_scoring` | E1=0.50, E7=0.15, E5=0.12, E10=0.08, E6=0.05, E11=0.05, E8=0.03, E9=0.02 | 0.0 | Dense-scoring biased. |
| `pipeline_full` | E1=0.40, E7=0.15, E5=0.10, E6=0.10, E10=0.08, E13=0.07, E11=0.05, E8=0.03, E9=0.02 | 0.0 | Stage-balanced. |
| `balanced` | 0.0715 each for E1–E13, E14=0.071 | 0.071 | Uniform baseline (sum ≈ 0.9995). |
| `multilingual_search` | E14=0.40, E1=0.20, E10=0.15, E5/E6/E7/E11=0.05 each, E8=0.03, E9=0.02 | **0.40** | Cross-lingual retrieval. |
| `long_context` | E14=0.45, E1=0.15, E7=0.10, E10=0.10, E5/E6/E8/E11=0.05 each | **0.45** | 8192-token BGE-M3 context. |
| `translation_finder` | E14=0.70, E10=0.20, E1=0.05, E11=0.05 | **0.70** | Cross-lingual paraphrase. |
| `affect_repair` | E1=0.30, E10=0.20, E14=0.20, E9=0.10, E5/E6/E7/E8=0.05 each | 0.20 | Stress-floor routing target. |
| `affect_priming` | E1=0.25, E10=0.25, E14=0.25, E5/E6/E7/E8/E9=0.05 each | 0.25 | Low-valence routing target. |
| `affect_neutral` | E1=0.25, E6=0.20, E5=0.15, E14=0.15, E10=0.10, E7/E8/E9=0.05 each | 0.15 | High-stress + arousal routing target. |

All 14-slot profiles sum to 1.0 ± 0.01 (validated by `validate_weights`). `MAX_WEIGHTED_AGREEMENT = 9.5`.

### 5.1 Resolution helpers

| Function | Behavior |
| --- | --- |
| `get_weight_profile(name)` | Raw lookup; returns `WeightProfileError::UnknownProfile { name, available }` on miss. |
| `get_effective_weight_profile(name)` | Same as above but applies `apply_e11_disable` when `E11_ENTITY_ENABLED=false`. |
| `apply_e11_disable(&mut weights)` | Zeroes `weights[10]`, scales remaining nonzero weights by `(remaining_sum + e11_weight) / remaining_sum` so the sum still ≈ 1.0. No-op when E11 weight is already 0. |
| `validate_weights(weights)` | Each weight in [0, 1]; sum ≈ 1.0 (`±0.01`); fail-fast `OutOfRange` / `InvalidSum`. |
| `space_name(idx)` | `0→E1_Semantic` … `13→E14_BgeM3Dense`, `_→Unknown`. |
| `select_state_conditioned_weight_profile(base, components)` | Routes to learner-state profile, see §5.2. |

### 5.2 State-conditioned routing

`LearnerStateComponents` (validated for finiteness and bounds) drives a deterministic override. Priority is highest stress band first.

| Condition (in order) | Selected profile | `reason` |
| --- | --- | --- |
| `stress_floor >= 0.80` OR (`stress_floor >= 0.60` AND `arousal >= 0.65`) | `affect_neutral` | `stress_floor/arousal above neutral-routing threshold` |
| `stress_floor >= 0.60` | `affect_repair` | `stress_floor above repair-routing threshold` |
| `valence <= -0.30` | `affect_priming` | `valence below priming-routing threshold` |
| else | `base_profile` (default `semantic_search`) | `learner state within base-profile band` |

Both base and selected profiles are looked up at routing time, so `InvalidBaseProfile` and `InvalidLearnerState` errors fail fast. Exposed via MCP `resolve_learner_retrieval_policy`.

### 5.3 Custom weights and exclusions

`resolve_weights_sync` priority (`search.rs:1388`):

1. `options.custom_weights` if set (validated by `validate_weights`).
2. Else `get_effective_weight_profile(options.weight_profile)` if set.
3. Else `DEFAULT_SEMANTIC_WEIGHTS`.
4. After step 1–3: if `!E11_ENTITY_ENABLED`, run `apply_e11_disable`.
5. If `options.exclude_embedders` is non-empty: zero each excluded slot, then renormalize. Out-of-range index returns `ValidationError`. Excluding all embedders returns `ValidationError`.

Custom weight profiles persisted via `create_weight_profile` are capped at **1000 per database** (see `create_weight_profile` MCP tool — namespace cap to prevent unbounded growth).

---

## 6. Causal gate

`crates/context-graph-core/src/causal/asymmetric.rs:51` — gate constants and `apply_causal_gate(original, e5_score, is_causal_query)`.

| Constant | Value | Meaning |
| --- | --- | --- |
| `causal_gate::CAUSAL_THRESHOLD` | `0.04` | E5 ≥ this → "definitely causal" → boost |
| `causal_gate::NON_CAUSAL_THRESHOLD` | `0.008` | E5 ≤ this → "definitely non-causal" → demote |
| `causal_gate::CAUSAL_BOOST` | `1.10` | Multiplier on `original_score` when boosted |
| `causal_gate::NON_CAUSAL_DEMOTION` | `0.85` | Multiplier on `original_score` when demoted |
| `direction_mod::CAUSE_TO_EFFECT` | `1.2` | Forward-inference amplification |
| `direction_mod::EFFECT_TO_CAUSE` | `0.8` | Backward-inference dampening |
| `direction_mod::SAME_DIRECTION` | `1.0` | No modification |
| `direction_mod::UNKNOWN` | `1.0` | No modification when direction unset |

`apply_causal_gate` short-circuits when `is_causal_query=false` (returns original). The ambiguous zone `(NON_CAUSAL_THRESHOLD, CAUSAL_THRESHOLD)` is a no-op.

Calibration: post-training E5 score distribution (GPU benchmark 2026-02-14, prefix-aligned retrain) — causal mean=0.138, non-causal mean=0.016, gap=0.121. Thresholds are calibrated for the prefix-aligned LoRA + projection model on `nomic-embed-text-v1.5` with `"search_document:"` prefix used during BOTH training and inference. Untrained models score 0.93–0.98 on every input → gate becomes a no-op (boosts everything equally).

`memory_tools.rs:2575` documents the user-facing semantics. `causal_tools.rs` (search_causes/search_effects) and `memory_tools.rs` apply the gate after fusion when the query is classified causal.

`infer_direction_from_fingerprint(fp)` chooses between Cause / Effect / Unknown by comparing the **component variance** of `e5_causal_as_cause` vs `e5_causal_as_effect` (since both are L2-normalized, magnitudes are useless). Higher variance → more peaked projection → that role is the embedded direction. 5% variance gap is required.

---

## 7. Degenerate-weight suppression

`suppress_degenerate_weights(all_scores: &[[f32; 14]], weights: &[f32; 14]) -> [f32; 14]` — `search.rs:1625`.

| Constant | Value | Purpose |
| --- | --- | --- |
| `MIN_VARIANCE` | `0.001` | Below this → embedder is "noise, not signal" |
| `SUPPRESSION_FACTOR` | `0.25` | Multiplier applied to that embedder's weight |

Algorithm:

1. Skip if fewer than 3 candidates (variance is unreliable).
2. For each embedder slot with `weights[idx] > 0.0`:
   a. If **every** candidate has score `< 0.0` (the `-1.0` no-signal sentinel — currently emitted by E5 with `CausalDirection::Unknown` per AP-77 / CORE-H1), zero the weight.
   b. Otherwise compute mean+variance via Welford's online algorithm (P2), skipping sentinel `< 0.0` entries. With Bessel's correction (SRC-8 — denominator `count - 1`) for an unbiased sample variance.
   c. If `count < 3` after sentinel filtering, skip (no data).
   d. If `variance < MIN_VARIANCE`, multiply that embedder's weight by `0.25`.

**Important**: `compute_semantic_fusion` accepts `score >= 0.0` (not `> 0.0`). After SRC-3 normalization `(raw + 1) / 2`, anti-correlated cosine becomes `0.0` and is **valid signal**. Filtering zeros would drop legitimate anti-correlated matches. Only the explicit `-1.0` sentinel is excluded.

Constants: `MULTI_SPACE_MAX_EMBEDDERS = 10` (preallocation hint for `embedder_rankings: Vec<EmbedderRanking>`), `STAGE1_RECALL_MULTIPLIER = 10`, `STAGE2_CANDIDATE_MULTIPLIER = 3`.

---

## 8. SRC-3 cosine normalization

Per the codebase convention `(raw_cos + 1) / 2`, raw cosine `[-1, 1]` maps to `[0, 1]`:
- `1.0` → `1.0` (identical)
- `0.0` → `0.5` (orthogonal)
- `-1.0` → `0.0` (maximally anti-correlated)

`compute_embedder_scores_sync` applies this for E6 (`(raw + 1) / 2`) and E13 (`(raw + 1) / 2`). Dense embedders use `compute_cosine_similarity` (`crates/context-graph-storage/src/teleological/rocksdb_store/helpers.rs`) which is already `[0, 1]`. E5 returns the asymmetric similarity (already in `[0, 1]` after `direction_mod` × normalized cosine). E12 MaxSim is already `[0, 1]` (max over per-token normalized cosines). E7's query-type-aware path returns `[0, 1]` (`compute_e7_similarity_with_query_type`).

`hnsw_distance_to_similarity(d) = (1 - d/2).clamp(0, 1)` (HNSW returns squared L2 on L2-normalized vectors, i.e. `2 - 2·cos`).

---

## 9. Search MCP tool catalog

| Tool | Strategy | Embedder focus | Rerank | Key sources |
| --- | --- | --- | --- | --- |
| `search_graph` | configurable (default `multi_space`) | all | optional E12 (`enableRerank`) | `memory_tools.rs` |
| `search_recent` | `e1_only` then post-retrieval E2 boost | E1 + E2 decay (`linear`/`exponential`/`step`) | none | `temporal_tools.rs:34` |
| `search_periodic` | `e1_only` then post-retrieval E3 boost | E1 + E3 hour-of-day / day-of-week | none | `temporal_tools.rs:203` |
| `search_robust` | configurable (default `e1_only`); `pipeline` enables E12 rerank | E1 ∪ E9 blind-spot detection | E12 if `pipeline` | `robustness_tools.rs:73` |
| `search_by_keywords` | `e1_only` (E6 keyword boost post-fusion) | E1 + E6 sparse Jaccard | none | `keyword_tools.rs:59` |
| `search_by_tokens` | `embedder_first` to E12 candidate set (top-K via E1, then MaxSim rerank) | E12 ColBERT MaxSim | implicit | `embedder_tools.rs:1576` |
| `search_by_expansion` | `pipeline` (E13 SPLADE Stage 1) | E13 + E1 stage 2 | optional | `embedder_tools.rs:1642` |
| `search_code` | `multi_space` candidate, then E7 query-type-aware scoring (Code2Code / Text2Code / NonCode auto-detected from `query_text`) | E7 primary, E1/E11 secondary | none | `code_tools.rs:62` |
| `search_causes` / `search_effects` | direction-aware E5 HNSW + multi-space; gate applied post-fusion | E5 (asymmetric) + E1/E8 | optional rerank | `causal_relationship_tools.rs`, `causal_tools.rs` |
| `search_by_entities` | `embedder_first` E11 | E11 KEPLER | none | `entity_tools.rs:760` |
| `search_connections` | graph traversal over typed edges + multi-space scoring | E8 graph + per-edge weights | none | `graph_tools.rs:81` |
| `search_by_embedder` | `embedder_first` (single index) | requested embedder | none | `embedder_tools.rs` (direct dispatch) |
| `compare_embedder_views` | per-embedder MultiSpace search; aligns 14 ranked lists | all 14 | none | `embedder_tools.rs:543` |
| `search_cross_embedder_anomalies` | multi-space search + post-filter on `(highEmbedder ≥ highThreshold) AND (lowEmbedder ≤ lowThreshold)` | configurable pair | none | `embedder_tools.rs:1362` |
| `get_embedder_clusters` | per-embedder cluster lookup (no fusion) | one embedder | none | `embedder_tools.rs:322` |

`search_robust` algorithm (`robustness_tools.rs`): (1) embed query 14×, (2) E1 `top_k * 3` candidates, (3) E9 `top_k * 3` candidates, (4) compute blind-spots = `{ id : E9_score >= e9_discovery_threshold AND E1_score < e1_weakness_threshold }` (defaults 0.08 / 0.5), (5) merge dedup, label each result `Source::E1Match | E9Discovery | Both`.

`compare_embedder_views`: runs `search_graph`-style retrieval with `embedder_indices=[i]` for each `i ∈ 0..14`, then aligns the 14 ranked lists per memory and reports `consensus_score = found_by.len() / N_active_embedders`. Useful for identifying blind spots and unique-embedder finds.

`search_cross_embedder_anomalies`: post-filter on the same `embedder_scores` array used by `search_graph`. Implementation iterates results and keeps those satisfying the threshold pair.

---

## 10. Insight annotations and perspective coverage

`crates/context-graph-core/src/retrieval/insight_annotation.rs`:

| Symbol | Purpose |
| --- | --- |
| `EmbedderCategory::{Semantic, Temporal, Relational, Structural, PipelineStage}` | Categorization used to spot missing perspectives. |
| `compute_perspective_coverage(found_by: &HashSet<usize>)` → `PerspectiveCoverage` | Returns `{ semantic_count, temporal_count, relational_count, structural_count, missing: Vec<MissingPerspective> }`. |
| `generate_insight_annotation(combined: &CombinedResult)` | Human-readable string ("Blind-spot discovery", "Strong consensus", category-specific notes). |
| `embedder_names_for_indices(&[usize])` → `Vec<&'static str>` | Maps slot indices to canonical names. |
| `annotate_results(results: &mut [CombinedResult])` | Bulk-annotation pass. |

These power the `insight_annotation` field surfaced by `aggregate_rrf_additive`.

---

## 11. Divergence detection

`crates/context-graph-core/src/retrieval/divergence.rs`:

| Constant | Value | Purpose |
| --- | --- | --- |
| `DIVERGENCE_SPACES` | 7 embedders: E1, E6, E7, E10, E12, E13, E14 | Semantic-only set; explicitly excludes E5 (AP-77 — needs `CausalDirection`), E2/E3/E4 (AP-63 — temporal), E8/E11 (relational). |
| `MAX_SUMMARY_LEN` | `100` | Memory-summary truncation in alerts. |
| `DivergenceSeverity::from_score(score)` | `score < 0.10 → High; 0.10..0.20 → Medium; 0.20..0.30 → Low` | Lower similarity = higher severity. |

`DivergenceDetector` (`detector.rs`) computes per-space similarity between current and recent activity (`RecentMemory`); if any `DIVERGENCE_SPACES` slot drops below the configured low threshold, it emits a `DivergenceAlert`. Surfaced via `get_divergence_alerts` MCP tool.

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
| `unique_boost` (additive RRF default) | `0.10` (10%) | `aggregation::aggregate_rrf_additive` |
| `MAX_WEIGHTED_AGREEMENT` | `9.5` | `weights/mod.rs`, `clustering/manager.rs` |
| `TOTAL_WEIGHT` | `9.5` | `retrieval/config.rs` |
| `WEIGHT_PROFILE` array length | `14` | `weights/mod.rs` |
| `NUM_EMBEDDERS` | `14` | `types/fingerprint.rs` |
| `TEMPORAL_EMBEDDERS_ENABLED` | `true` | `weights/mod.rs:42` |
| `E11_ENTITY_ENABLED` | `true` (EXP-2 audit; flip to disable retrieval) | `weights/mod.rs:48` |
| `causal_gate::CAUSAL_THRESHOLD` | `0.04` | `causal/asymmetric.rs:61` |
| `causal_gate::NON_CAUSAL_THRESHOLD` | `0.008` | `causal/asymmetric.rs:64` |
| `causal_gate::CAUSAL_BOOST` | `1.10` | `causal/asymmetric.rs:68` |
| `causal_gate::NON_CAUSAL_DEMOTION` | `0.85` | `causal/asymmetric.rs:72` |
| `direction_mod::CAUSE_TO_EFFECT` | `1.2` | `causal/asymmetric.rs:42` |
| `direction_mod::EFFECT_TO_CAUSE` | `0.8` | `causal/asymmetric.rs:44` |
| Custom weight profiles cap | `1000` | per-DB cap on user-created profiles |
| `DEFAULT_RECALL_LIMIT` (sparse) | see `sparse_index.rs` | E13 stage-1 default top-K |

---

## 13. Source notes

- **`compute_semantic_fusion` returns weighted cosine, not RRF.** RRF is the candidate selector; the user-facing similarity is the weighted average over per-embedder cosines (after suppression). This applies to MultiSpace AND the WeightedRRF/ScoreWeightedRRF branches of Pipeline (`PIPE-1` fix).
- **HNSW returns squared L2.** `hnsw_distance_to_similarity(d) = (1 - d/2).clamp(0, 1)` undoes this for L2-normalized vectors so it's directly comparable to cosine similarity.
- **Direction-aware E5 cross-pair retrieval.** A "cause-seeking" query embeds the cause vector but searches the **effect** index — we want to find memories whose effect matches our cause ("what did this cause?"). Effect-seeking is symmetric. This is intentional (`STOR-L5`); without it, asymmetric direction signal is lost.
- **`E11_ENTITY_ENABLED` is currently `true`.** Set to `false` in `weights/mod.rs:48` to disable E11 retrieval — `apply_e11_disable` will then redistribute its weight slot. The MCP `compare_embedder_views` short-circuits with empty results when E11 is disabled.
- **E12 / E13 are pipeline-stage-only in named profiles.** All semantic profiles assign E12=E13=0.0; they enter scoring via Stage 1 recall (E13) and Stage 3 rerank (E12), not fusion (per ARCH-13).
- **`-1.0` is the "no signal" sentinel.** Pre-CORE-H1 the codebase used `0.0`, but `0.0` is now valid (anti-correlated after SRC-3). E5 with `CausalDirection::Unknown` is the canonical sentinel emitter (per AP-77).
- **`MULTI_SPACE_MAX_EMBEDDERS = 10` is a preallocation hint, not a limit.** Storage iterates all 14 embedder slots via weight gating; the constant sizes `Vec::with_capacity` for `embedder_rankings` and `degraded_embedders`.
- **`balanced` profile sums to 0.9995, not 1.0.** Tolerated by `validate_weights` (±0.01).
- **Divergence excludes E5 and E14 inconsistency.** `DIVERGENCE_SPACES` includes E14 (semantic per ARCH-10) but excludes E5 (needs direction). Both are explicit.
- **Custom weights bypass profile-name resolution.** `custom_weights` in `TeleologicalSearchOptions` overrides `weight_profile`, but `apply_e11_disable` and `exclude_embedders` still apply afterwards.
