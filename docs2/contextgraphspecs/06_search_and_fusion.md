# 06 — Search and Fusion

Retrieval, ranking, and score fusion across the 13 embedding spaces. See [05_embedding_subsystem.md](05_embedding_subsystem.md) for embedder definitions, [12_clustering_and_topics.md](12_clustering_and_topics.md) for topic/portfolio details, and [08_mcp_tools_reference.md](08_mcp_tools_reference.md) for tool-level parameters.

Source files covered:
```
crates/context-graph-core/src/fusion/mod.rs
crates/context-graph-core/src/retrieval/mod.rs
crates/context-graph-core/src/retrieval/aggregation.rs
crates/context-graph-core/src/retrieval/config.rs
crates/context-graph-core/src/retrieval/distance.rs
crates/context-graph-core/src/retrieval/multi_space.rs
crates/context-graph-core/src/retrieval/retriever.rs
crates/context-graph-core/src/retrieval/sparse_index.rs
crates/context-graph-core/src/causal/asymmetric.rs
crates/context-graph-core/src/weights/mod.rs
crates/context-graph-core/src/index/hnsw_impl/{real_hnsw,multi_space,persistence}.rs
crates/context-graph-storage/src/teleological/rocksdb_store/search.rs
crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs
crates/context-graph-storage/src/teleological/rocksdb_store/provenance_storage.rs  (CF_CUSTOM_WEIGHT_PROFILES)
crates/context-graph-storage/src/teleological/search/pipeline/{execution,stages,types}.rs
crates/context-graph-mcp/src/weights.rs
crates/context-graph-mcp/src/handlers/tools/memory_tools.rs           (search_graph, causal gate)
crates/context-graph-mcp/src/handlers/tools/robustness_tools.rs       (search_robust)
crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs         (create_weight_profile)
```

---

## 1. Search Strategies

Strategy is selected by the `strategy` field on `TeleologicalSearchOptions` (MCP param `strategy`). When `embedder_indices` is set, strategy dispatch is bypassed and a direct single- or filtered-multi-embedder search runs instead.

| Strategy         | Use case                                        | Embedders engaged (HNSW recall)                | Score normalization                 | Fusion method                                                                 |
|------------------|-------------------------------------------------|------------------------------------------------|-------------------------------------|-------------------------------------------------------------------------------|
| `E1Only`         | Fast E1-only baseline, backward compat          | E1                                             | cosine → `(raw+1)/2`                | HNSW cosine only; no fusion                                                   |
| `MultiSpace` (default) | Broad semantic recall across spaces       | E1, E2*, E3*, E4*, E5 (direction-aware), E7, E8, E9, E10, E11†  | cosine → `[0,1]`                   | Per-slot HNSW rankings → Weighted RRF / ScoreWeightedRRF / WeightedSum → re-score with `compute_semantic_fusion` |
| `Pipeline`       | Two-stage recall + dense rerank                 | Stage 1: E13 SPLADE + E1, E5, E7, E8, E9, E11† (+ E2/E3/E4 when weight>0). Stage 2: all 13 scored | cosine → `[0,1]`, BM25-IDF for SPLADE | Stage 1 union → Stage 2 fusion (Weighted RRF or WeightedSum) → optional Stage 3 E12 MaxSim interpolation |
| Embedder-specific (`embedder_indices`) | Explicit per-space search              | Exactly the indices requested (HNSW-capable only; E6/E12/E13 rejected for single) | cosine → `[0,1]`                   | None for single; filtered multi-space for multiple                            |

\* E2/E3/E4 only participate when the profile assigns weight > 0 (e.g., `temporal_navigation`).
† E11 only when `E11_ENTITY_ENABLED = true` (compile-time constant; currently `false`).

E6 (sparse lexical) and E12 (ColBERT) are **never HNSW-recalled**; E6 scores are computed post-recall from stored sparse vectors, E12 is applied only as an interpolation rerank in pipeline Stage 3.

---

## 2. MultiSpace Search Algorithm

Defined in `search_multi_space_sync` (`rocksdb_store/search.rs`). Pipeline:

1. **Resolve weights** — `resolve_weights_sync(options)`: custom_weights > named profile > `DEFAULT_SEMANTIC_WEIGHTS`. `apply_e11_disable` and `exclude_embedders` renormalization are applied after resolution.
2. **Compute per-slot recall limit** — `k = max(top_k * 3, 50)`.
3. **Per-slot HNSW recall** (only for slots with weight > 0, gated on `E11_ENTITY_ENABLED` for E11):
   - E1 (unconditional, entry point; failure = hard error)
   - E2, E3, E4 (weight-gated)
   - E5: direction-aware — `Cause` queries `E5CausalEffect` index using `get_e5_as_cause()`; `Effect` queries `E5CausalCause` using `get_e5_as_effect()`; `Unknown` queries `E5Causal` using `e5_active_vector()`
   - E7, E8 (source vector), E9, E10 (paraphrase vector), E11 (if enabled)
   - Failures on non-E1 slots are logged to `degraded_embedders` but do not abort
4. **Per-slot normalization** — HNSW distances converted via `hnsw_distance_to_similarity` (cosine distance → `(1 - d/2)` ∈ `[0,1]`).
5. **Weighted RRF fusion** — `fuse_rankings(rankings, options.fusion_strategy, top_k*2)` produces ranked candidate IDs.
6. **Two-pass re-scoring**:
   - Pass 1: hydrate fingerprints, call `compute_embedder_scores_sync` (all 13 scores)
   - `suppress_degenerate_weights`: any slot with across-candidate variance < 0.001 or all-sentinel scores has weight scaled by 0.25 (or zeroed for all-sentinel)
   - Pass 2: `compute_semantic_fusion(scores, adjusted_weights)` → weighted average in `[0,1]` (skips score < 0 sentinels)
7. **Min-similarity filter** → `sort_by similarity desc` → `truncate(top_k)`.
8. **Post-retrieval boosts** (handler layer): asymmetric E5 rerank, E5 causal gate, ColBERT interpolation, temporal boosts, time-window filter.

---

## 3. RRF Formula

```
fused_score(d) = Σ_slots  weight[slot]  /  (RRF_K + rank_slot(d) + 1)
```

| Constant  | Value | Location |
|-----------|-------|----------|
| `RRF_K`   | 60.0  | `fusion::RRF_K`, `config::constants::similarity::RRF_K`, `PipelineConfig.rrf_k` |

`rank_slot(d)` is the **0-based rank** of document `d` in slot's HNSW candidate list. Documents not retrieved by a slot contribute 0 from that slot. Zero-weight slots are skipped entirely (`if ranking.weight <= 0.0 { continue; }`).

**Canonical helper**: `AggregationStrategy::rrf_contribution(rank, k) = 1.0 / (k + rank + 1.0)`.

**Score-Weighted RRF** variant (`FusionStrategy::ScoreWeightedRRF`): for E5 specifically, contribution is `weight * similarity * base_rrf` instead of `weight * base_rrf`. Applied by `fuse_rankings` when the strategy enum is `ScoreWeightedRRF`; `score_weighted_rrf(rankings, &["E5"], top_k)` is the entry point. All other embedders retain standard RRF.

**Fused score ≠ user similarity**: RRF scores (~0.003 range) are only used to select Stage 2 candidates. Final `TeleologicalSearchResult.similarity` is always re-computed via `compute_semantic_fusion` in `[0,1]`.

---

## 4. Pipeline Strategy

Defined in `search_pipeline_sync`. Two-stage with optional Stage 3.

### Stage 1 — Fast Recall (union of candidates)

| Sub-recall           | Source               | k                  | Notes                                           |
|----------------------|----------------------|--------------------|-------------------------------------------------|
| E13 SPLADE sparse    | `CF_E13_SPLADE_INVERTED` | `top_k * STAGE1_RECALL_MULTIPLIER` (=10×) | BM25-IDF scoring with total-doc-count from atomic counter; soft-deleted skipped |
| E1 Semantic HNSW     | registry             | `top_k * 10`       | Hard failure aborts pipeline                    |
| E5 Causal HNSW       | direction-aware idx  | `top_k * 5`        | Same cross-pair routing as MultiSpace           |
| E7 Code HNSW         | registry             | `top_k * 5`        | Soft failure = warn + continue                  |
| E8 Graph HNSW        | registry             | `top_k * 5`        | source vector                                   |
| E9 HDC HNSW          | registry             | `top_k * 5`        | Unconditional (cheap typo backup)               |
| E11 Entity HNSW      | registry             | `top_k * 5`        | Only if `E11_ENTITY_ENABLED`                    |
| E2/E3/E4 Temporal    | registry             | `top_k * 5`        | Weight-gated                                    |

Output: `HashSet<Uuid>` — deduplicated candidate ID union. Empty set → return `[]`.

### Stage 2 — Multi-Space Scoring

1. Hydrate all candidate fingerprints, skip soft-deleted (unless `include_deleted`).
2. `compute_embedder_scores_sync` → 13 scores per candidate.
3. `suppress_degenerate_weights` on cross-candidate variance.
4. Fusion per `options.fusion_strategy`:
   - `WeightedSum`: `compute_semantic_fusion` directly
   - `WeightedRRF` / `ScoreWeightedRRF`: build per-slot `EmbedderRanking` from the 13 scores, run `fuse_rankings` (E12 excluded; other slots with weight>0 included), then **re-score** surviving candidates via `compute_semantic_fusion` so output is in `[0,1]`.
5. Min-similarity filter, sort desc, `truncate(top_k * STAGE2_CANDIDATE_MULTIPLIER)` where `STAGE2_CANDIDATE_MULTIPLIER = 3`.

### Stage 3 — E12 MaxSim Interpolation (optional)

Triggered only when `options.enable_rerank` and query has `e12_late_interaction` tokens. For each surviving candidate:

```
maxsim  = max_sim(query.e12_tokens, stored.e12_tokens)
final   = (1 - rerank_weight) * stage2_score + rerank_weight * maxsim
```

Sort desc, `truncate(top_k)`. If candidate has no stored E12 tokens, maxsim is 0.0.

### Stage Cutoffs (constants)

| Constant                          | Value | Location                                   |
|-----------------------------------|-------|--------------------------------------------|
| `STAGE1_RECALL_MULTIPLIER`        | 10    | `rocksdb_store/search.rs`                  |
| `STAGE2_CANDIDATE_MULTIPLIER`     | 3     | `rocksdb_store/search.rs`                  |
| `DEFAULT_RECALL_LIMIT` (SPLADE)   | 10000 | `retrieval::sparse_index`                  |
| `MULTI_SPACE_MAX_EMBEDDERS`       | 10    | `rocksdb_store/search.rs` (Vec preallocation) |

---

## 5. Weight Profiles

All profiles are `[f32; 13]` arrays keyed as `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]` and MUST sum to ~1.0 (validated by `validate_weights`). E12 and E13 are always 0.0 in scoring profiles (they are pipeline-stage embedders).

Source: `crates/context-graph-core/src/weights/mod.rs::WEIGHT_PROFILES`. Lookups go through `get_effective_weight_profile` which applies `apply_e11_disable` when `E11_ENTITY_ENABLED = false` (current default): E11's weight is zeroed and redistributed proportionally to remaining non-zero slots.

| Profile                    | Dominant slots (weight)                          | Full vector (or description)                                                                                     | Use case                                       |
|----------------------------|--------------------------------------------------|------------------------------------------------------------------------------------------------------------------|------------------------------------------------|
| `semantic_search`          | E1=0.33, E7=0.20, E5=0.15, E10=0.15              | `[0.33, 0, 0, 0, 0.15, 0.05, 0.20, 0.05, 0.02, 0.15, 0.05, 0, 0]`                                                | General-purpose semantic queries               |
| `causal_reasoning`         | E1=0.40, E7=0.15, E5=0.10, E8=0.10, E10=0.10, E11=0.10 | `[0.40, 0, 0, 0, 0.10, 0.05, 0.15, 0.10, 0, 0.10, 0.10, 0, 0]`                                             | "Why"/cause-effect questions (E5 is gate signal) |
| `code_search`              | E7=0.40, E1=0.20, E6=0.10, E10=0.10, E11=0.10    | `[0.20, 0, 0, 0, 0.10, 0.10, 0.40, 0, 0, 0.10, 0.10, 0, 0]`                                                      | Source code retrieval                          |
| `fact_checking`            | E11=0.40, E5=0.15, E6=0.15, E1=0.15              | `[0.15, 0, 0, 0, 0.15, 0.15, 0.05, 0.05, 0, 0.05, 0.40, 0, 0]`                                                   | Entity/fact verification                       |
| `graph_reasoning`          | E8=0.40, E11=0.20, E1=0.15                       | `[0.15, 0, 0, 0, 0.10, 0.10, 0, 0.40, 0, 0.05, 0.20, 0, 0]`                                                      | "What connects to X?" structural queries       |
| `temporal_navigation`      | E2=E3=E4=0.23, E1=0.13                           | `[0.13, 0.23, 0.23, 0.23, 0.03, 0.02, 0.03, 0.02, 0.03, 0.03, 0.02, 0, 0]`                                       | Explicit time-based queries                    |
| `sequence_navigation`      | E4=0.55, E1=0.20                                 | `[0.20, 0.05, 0, 0.55, 0.03, 0.02, 0.03, 0.02, 0.03, 0.03, 0.02, 0, 0.02]`                                       | Session/conversation turn ordering             |
| `conversation_history`     | E4=0.35, E1=0.30, E5=0.10                        | `[0.30, 0.05, 0, 0.35, 0.10, 0.03, 0.05, 0.02, 0, 0.05, 0.03, 0, 0.02]`                                          | Mixed temporal + topic recall                  |
| `category_weighted`        | E1=E5=E6=E7=E10=1/6.5, E8=E9=E11=0.5/6.5         | Normalizer 6.5 = 5×SEMANTIC(1.0) + 2×RELATIONAL(0.5) + 1×STRUCTURAL(0.5)                                         | Constitution-compliant balanced fusion         |
| `typo_tolerant`            | E1=0.30, E9=0.18, E7=0.15, E10=0.12              | `[0.30, 0, 0, 0, 0.10, 0.07, 0.15, 0.03, 0.18, 0.12, 0.05, 0, 0]`                                                | Queries with spelling errors (E9 primary)      |
| `pipeline_stage1_recall`   | E13=0.25, E6=0.25, E1=0.20                       | `[0.20, 0, 0, 0, 0.05, 0.25, 0.10, 0, 0.05, 0.05, 0.05, 0, 0.25]`                                                | Recall-stage weight hint (sparse-heavy)        |
| `pipeline_stage2_scoring`  | E1=0.50, E7=0.15, E5=0.12                        | `[0.50, 0, 0, 0, 0.12, 0.05, 0.15, 0.03, 0.02, 0.08, 0.05, 0, 0]`                                                | Dense scoring stage                            |
| `pipeline_full`            | E1=0.40, E7=0.15, E5=0.10, E6=0.10               | `[0.40, 0, 0, 0, 0.10, 0.10, 0.15, 0.03, 0.02, 0.08, 0.05, 0, 0.07]`                                             | Complete pipeline end-to-end                   |
| `balanced`                 | all ≈ 0.077                                      | `[0.077 × 12, 0.076]` (E13 trimmed to sum to 1.0)                                                                | Testing / comparison baseline                  |

Storage-layer default (when no profile is specified): `DEFAULT_SEMANTIC_WEIGHTS` = `[0.35, 0, 0, 0, 0.16, 0.05, 0.21, 0.05, 0.02, 0.16, 0, 0, 0]` — equivalent to `semantic_search` with E11's 0.05 pre-redistributed to E1/E5/E7/E10.

---

## 6. Custom Weight Profiles

Created via `create_weight_profile` MCP tool, persisted to RocksDB column family `CF_CUSTOM_WEIGHT_PROFILES` ("custom_weight_profiles").

**Storage layout**:
- Key: profile name as UTF-8 bytes.
- Value: JSON-serialized `[f32; 13]`.
- Access: `RocksDbTeleologicalStore::{store,get,list,delete}_custom_weight_profile` (sync) with async trait wrappers.

**Creation constraints** (`call_create_weight_profile`):
- Name must not collide with any built-in profile (`get_profile_names`).
- `validate_weights` must pass: every weight ∈ `[0,1]` and sum ≈ 1.0.
- `MAX_CUSTOM_PROFILES = 1000` enforced on the in-memory cache before persistence.
- Persisted to RocksDB first, then inserted into `custom_profiles` `RwLock<HashMap>`.

**Lookup order** (in `search_graph` / `resolve_weights_sync`):
1. Explicit `customWeights` (array) overrides everything.
2. Named `weightProfile`:
   - First check `self.custom_profiles` (handler-layer cache).
   - If absent, `get_effective_weight_profile(name)` resolves built-ins.
   - Unknown name → `ToolErrorKind::Validation` with profile list.
3. Default (`semantic_search` at handler layer; `DEFAULT_SEMANTIC_WEIGHTS` at storage layer).

Custom profiles are restored from RocksDB into the in-memory cache at handler init. Custom weights force `SearchStrategy::MultiSpace` regardless of the caller's `strategy` to guarantee the weights are actually consulted.

---

## 7. Causal Gate

Applied post-retrieval at handler layer when a query is classified as causal (`CausalDirection != Unknown`). Source: `core/causal/asymmetric.rs::{causal_gate, apply_causal_gate}`.

| Constant                               | Value  | Meaning                                                                 |
|----------------------------------------|--------|-------------------------------------------------------------------------|
| `causal_gate::CAUSAL_THRESHOLD`        | 0.04   | E5 asymmetric sim ≥ this → content deemed causal                        |
| `causal_gate::NON_CAUSAL_THRESHOLD`    | 0.008  | E5 asymmetric sim ≤ this → content deemed non-causal                    |
| `causal_gate::CAUSAL_BOOST`            | 1.10   | Multiplier for passing results                                          |
| `causal_gate::NON_CAUSAL_DEMOTION`     | 0.85   | Multiplier for failing results                                          |

Ambiguous zone (between the two thresholds) leaves the score unchanged. Applied in:

- `memory_tools.rs::apply_direction_aware_reranking` (per-result, after asymmetric E5 sim computed between query and stored fingerprint)
- `causal_tools.rs` (for `search_causes`, `search_effects`, etc.)

`apply_causal_gate(original_score, e5_score, is_causal_query)` is a short-circuit no-op when `is_causal_query == false`.

---

## 8. Topic Boost

No score-level topic boost is applied during search fusion. Topic portfolios (`CF_TOPIC_PORTFOLIO`) are persisted and restored across sessions but are a classification/analysis artifact: they are surfaced through `get_topic_portfolio`, `detect_topics`, and `get_topic_stability` tools, not folded back into search scores.

"Topic similarity" computed by `MultiSpaceSimilarity::compute_relevance_score` / `compute_weighted_similarity` uses category weights (`SPACE_WEIGHTS`: Semantic=1.0, Relational=0.5, Structural=0.5, Temporal=0.0; `TOTAL_WEIGHT = 8.5`) and is used by the similarity retriever and divergence detector (`SimilarityRetriever`, `DivergenceDetector`) — not by the production `search_graph` path, which uses weight profiles instead.

See [12_clustering_and_topics.md](12_clustering_and_topics.md) for the full topic subsystem.

---

## 9. E5 Causal Direction

Asymmetric similarity for E5 per `ARCH-15` / `AP-77`. Constants in `causal::asymmetric::direction_mod`:

| Modifier                      | Value | Applied when                                  |
|-------------------------------|-------|-----------------------------------------------|
| `CAUSE_TO_EFFECT`             | 1.2   | query = Cause, result = Effect (forward inference) |
| `EFFECT_TO_CAUSE`             | 0.8   | query = Effect, result = Cause (backward inference) |
| `SAME_DIRECTION`              | 1.0   | both Cause or both Effect                     |
| `UNKNOWN`                     | 1.0   | either side Unknown                           |

**Direction detection**:
- Query direction: from user's `causalDirection` param (`auto`/`cause`/`effect`/`none`) or `detect_causal_query_intent(query_text)`.
- Result direction: `infer_direction_from_fingerprint` — compares component variance of E5 cause vs E5 effect projections; requires >5% variance gap to commit to `Cause` or `Effect`, else `Unknown`.

**HNSW routing** (in `search.rs`):

| Query direction | HNSW index queried      | Query vector used          |
|-----------------|-------------------------|----------------------------|
| `Cause`         | `E5CausalEffect`        | `query.get_e5_as_cause()`  |
| `Effect`        | `E5CausalCause`         | `query.get_e5_as_effect()` |
| `Unknown`       | `E5Causal`              | `query.e5_active_vector()` |

The cross-pair routing is intentional: a cause query looks for documents whose **effect** vector matches the query's **cause** vector.

**Score computation**: `compute_similarity_for_space_with_direction(Causal, …)` calls `compute_e5_asymmetric_fingerprint_similarity`, then multiplies by the direction modifier and clamps to `[0,1]`. For `Unknown` direction, the function returns `-1.0` as a **sentinel** meaning "no signal" — `suppress_degenerate_weights` and `compute_semantic_fusion` skip negative scores so 0.0 remains valid anti-correlated signal.

Additional reranking passes at the handler layer (`apply_asymmetric_e5_reranking`, `apply_direction_aware_reranking` in `memory_tools.rs`) recompute asymmetric E5 between the query and each result's stored fingerprint, apply the direction modifier, then run `apply_causal_gate`.

---

## 10. Normalization

All similarity paths output values in `[0.0, 1.0]`.

| Source                                            | Raw range   | Normalization                     |
|---------------------------------------------------|-------------|-----------------------------------|
| Dense cosine (`cosine_similarity`)                | `[-1, 1]`   | `(raw + 1.0) / 2.0` (SRC-3)       |
| Dense cosine raw (`cosine_similarity_raw`)        | `[-1, 1]`   | none (caller must normalize)      |
| Sparse cosine on E6/E13 (`SparseVector::cosine_similarity`) | `[-1, 1]` | `(raw + 1.0) / 2.0` applied in `compute_embedder_scores_sync` |
| Sparse Jaccard (`jaccard_similarity`)             | `[0, 1]`    | none                              |
| ColBERT MaxSim (`max_sim`)                        | `[0, 1]`    | normalized internally (mean of per-token max cosines) |
| HNSW distance → similarity (`hnsw_distance_to_similarity`) | `[0, 2]` cosine distance | `1 - d/2` → `[0, 1]`   |
| Direction-modified E5 (`compute_similarity_for_space_with_direction`) | up to `1.2` | `.clamp(0.0, 1.0)`  |

**Zero-magnitude vectors** return `0.0` (AP-10: no NaN). **Dimension mismatch or empty vectors** return `0.0`. **Unknown-direction E5** returns `-1.0` sentinel (distinct from 0.0 anti-correlated).

**NaN ordering**: `fusion::score_weighted_rrf`, `weighted_sum`, and result sorters use `partial_cmp` with NaN-last tiebreak for deterministic ordering.

---

## 11. Score Computation

### `compute_embedder_scores_sync(query, stored, code_query_type, causal_direction) → [f32; 13]`

Location: `rocksdb_store/search.rs`. Called once per candidate fingerprint post-recall. Per-slot logic:

| Slot | Computation                                                                                                              |
|------|--------------------------------------------------------------------------------------------------------------------------|
| E1   | `compute_cosine_similarity(query.e1_semantic, stored.e1_semantic)`                                                       |
| E2-E4| `compute_cosine_similarity` on the respective temporal dense vectors                                                     |
| E5   | `compute_similarity_for_space_with_direction(Causal, query, stored, causal_direction)` → `-1.0` sentinel when Unknown    |
| E6   | `(query.e6_sparse.cosine_similarity(stored.e6_sparse) + 1.0) / 2.0`                                                      |
| E7   | `compute_e7_similarity_with_query_type(query.e7_code, stored.e7_code, query_type)` (Code2Code / Text2Code / NonCode) when provided; else plain cosine |
| E8   | `max(cos(q.get_e8_as_source(), s.get_e8_as_target()), cos(q.get_e8_as_target(), s.get_e8_as_source()))`                  |
| E9   | `compute_cosine_similarity(query.e9_hdc, stored.e9_hdc)`                                                                 |
| E10  | `max(cos(q.get_e10_as_paraphrase(), s.get_e10_as_context()), cos(q.get_e10_as_context(), s.get_e10_as_paraphrase()))`    |
| E11  | `compute_cosine_similarity(query.e11_entity, stored.e11_entity)`                                                         |
| E12  | `compute_maxsim_direct(query.e12_tokens, stored.e12_tokens)` — skipped (0.0) when either is empty (MED-21 O(N·M) guard)  |
| E13  | `(query.e13_splade.cosine_similarity(stored.e13_splade) + 1.0) / 2.0`                                                    |

### `compute_semantic_fusion(scores, weights) → f32`

Weighted average over slots with `weight > 0` AND `score >= 0` (negative E5 sentinel excluded):
```
final = Σ(score_i * weight_i) / Σ(weight_i)
```
If all surviving weight is zero (all sentinels, all-zero profile, etc.), falls back to `scores[0].max(0.0)`.

### `suppress_degenerate_weights(all_scores, weights) → [f32; 13]`

Defense against degenerate/anisotropic embedders. Across the retrieved candidate set:

- If every candidate has `score[i] < 0` (all-sentinel) → `adjusted[i] = 0.0`.
- Otherwise compute Welford variance over non-sentinel scores with Bessel correction. If `count >= 3` and `variance < MIN_VARIANCE = 0.001` → `adjusted[i] *= SUPPRESSION_FACTOR (= 0.25)`.
- Requires `all_scores.len() >= 3` to act; below threshold it's a no-op.

### `apply_synergy_boost(results, synergy_weights)` (auxiliary)

When a candidate is found by multiple embedders, applies a post-fusion additive boost `Σ_{i<j} w_i · w_j · SYNERGY_BASE_BOOST (= 0.1)` over embedder index pairs that contributed. Synergy-weight vectors are selected per weight-profile name in `memory_tools.rs::call_search_graph` and passed via `TeleologicalSearchOptions::with_synergy_weights` for transparency; the final-score wiring is not enabled in the default fusion path.

### Group alignments (category bucketing)

`embeddings::category::category_for(embedder).topic_weight()` returns `SPACE_WEIGHTS`: Semantic=1.0 (E1, E5, E6, E7, E10, E12, E13), Relational=0.5 (E8, E11), Structural=0.5 (E9), Temporal=0.0 (E2, E3, E4). `TOTAL_WEIGHT = 8.5`. Used by `MultiSpaceSimilarity::compute_weighted_similarity` and `compute_relevance_score`; not applied inside `search_graph` fusion.

---

## 12. Session and Soft-Delete Filtering

### Soft-delete

- State: `Arc<DashMap<Uuid, i64>> soft_deleted` on the store (lock-free concurrent reads).
- Every stage (E1Only, MultiSpace per-slot, Pipeline Stage 1 per-slot, Pipeline Stage 2 hydration, sparse search) checks `is_soft_deleted_sync(soft_deleted, &id)` and skips unless `options.include_deleted == true`.
- The tombstone set is persisted in `CF_SYSTEM` and restored at store open.

### Session scope

Resolved at the handler layer in `call_search_graph`:

| `sessionScope` | Action                                                                                  |
|----------------|-----------------------------------------------------------------------------------------|
| `"current"`    | `options.with_session_filter(current_session_id)` — filters to active session           |
| `"recent"`     | `options.with_last_hours(24)` across all sessions                                       |
| `"all"`        | No session filter, but explicit `sessionId` arg still applied if provided               |

Session filtering and time-window filters are enforced in `search_semantic_async` after the synchronous strategy dispatch: `temporal_boost::filter_by_time_window` rejects out-of-window results, and session filter is applied via `source_metadata.session_id` lookup (post-retrieval).

### Temporal boosts

When `options.temporal_options.has_any_boost()`, `apply_full_temporal_boosts` runs post-retrieval (ARCH-14): E2 recency decay, E3 periodic match, E4 sequence anchor scoring. Decay function (`Linear`/`Exponential`/`Step`/`NoDecay`), half-life seconds, periodic targets (hour / day-of-week / auto-detect), and sequence options are all carried on `TeleologicalSearchOptions.temporal_options`.

---

## 13. Error Handling and Fallbacks

| Failure                                              | Behavior                                                                             |
|------------------------------------------------------|--------------------------------------------------------------------------------------|
| E1 HNSW recall failure (MultiSpace or Pipeline)      | `CoreError::IndexError`, abort search (fatal)                                        |
| E2-E11 HNSW recall failure (MultiSpace)              | Log + push to `degraded_embedders`, continue; result slot omitted from fusion        |
| E2-E11 HNSW recall failure (Pipeline Stage 1)        | Warn + continue; candidates from other slots still flow                              |
| E13 SPLADE recall failure (Pipeline)                 | Warn + continue with dense-only recall                                               |
| Direction-aware E5 index missing                     | Warn; no E5 candidates injected (rest of search proceeds)                            |
| Unknown `weightProfile`                              | `ToolErrorKind::Validation` with full list of built-in + custom profiles             |
| Invalid `customWeights` (out of range, wrong sum)    | `validate_weights` returns `WeightProfileError`; handler returns `Validation` error  |
| `excludeEmbedders` removes all weight                | `CoreError::ValidationError` (cannot renormalize zero vector)                        |
| Empty candidate set after any stage                  | Return `Ok(vec![])` — not an error                                                   |
| Zero-magnitude / dim-mismatch vector                 | `cosine_similarity` returns 0.0 (no NaN)                                             |
| NaN/Infinite in pipeline query vector                | `PipelineError::Search(SearchError::InvalidVector)` — FAIL FAST                      |
| Pipeline stage exceeds `max_latency_ms`              | `PipelineError::Timeout { stage, elapsed_ms, max_ms }`                               |
| All embedders produce sentinel / zero-variance       | `compute_semantic_fusion` falls back to `scores[0].max(0.0)` (E1 score)              |
| Content / source-metadata batch retrieval failure    | `ToolErrorKind::Storage` — no silent degradation                                     |

Degraded embedders are logged via `warn!(degraded = ?list, "…")` for operational visibility but do not surface in the tool response.
