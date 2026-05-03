# Contrastive And Typed Edges

**Source files covered:**
- `crates/context-graph-core/src/contrastive/mod.rs` — module surface, `CONTRASTIVE_PAIR_VERSION`
- `crates/context-graph-core/src/contrastive/types.rs` — `ContrastivePair`, `AnomalyKind`, `MiningConfig`, `MiningSummary`, `ContrastiveError`, `DEFAULT_*` constants, `NUM_ANOMALY_KINDS`
- `crates/context-graph-core/src/contrastive/mining.rs` — `similarity_profile`, `classify_anomaly`, `mine_pair_from_candidate`, `GENERATOR_TAG`, helpers (`src3_cosine`, `mean_pool_tokens`, `token_level_src3`)
- `crates/context-graph-core/src/graph_linking/mod.rs` — `KNN_K`, `NN_DESCENT_ITERATIONS`, `NN_DESCENT_SAMPLE_RATE`, `MIN_KNN_SIMILARITY`
- `crates/context-graph-core/src/graph_linking/edge_type.rs` — `GraphLinkEdgeType` (8-variant enum), default thresholds, asymmetric / symmetric variant lists
- `crates/context-graph-core/src/graph_linking/typed_edge.rs` — `TypedEdge` payload (source, target, edge_type, weight, agreement_count, agreeing_embedders, direction, embedder_scores)
- `crates/context-graph-core/src/graph_linking/storage_keys.rs` — `EdgeStorageKey` (17B), `TypedEdgeStorageKey` (32B)
- `crates/context-graph-core/src/graph_linking/direction.rs` — `DirectedRelation::{Symmetric, Forward, Backward}`
- `crates/context-graph-core/src/graph_linking/edge_builder.rs` — `EdgeBuilder`, `EdgeBuilderConfig`, `EdgeBuilderStats`, `DEFAULT_EMBEDDER_THRESHOLDS`
- `crates/context-graph-core/src/graph_linking/knn_graph.rs` — `KnnGraph`, `KnnGraphStats`
- `crates/context-graph-core/src/graph_linking/nn_descent.rs` — `NnDescent`, `NnDescentConfig`, `NnDescentStats`, `build_asymmetric_knn`
- `crates/context-graph-core/src/graph_linking/thresholds.rs` — `EdgeThresholds`, `DEFAULT_THRESHOLDS`
- `crates/context-graph-core/src/graph_linking/weight_projector.rs` — `WeightProjector`, `OptionalProjector`
- `crates/context-graph-core/src/typed_edge_export/mod.rs` — `TypedEdgeTrainingRecord`, `LLMValidationSummary`, `LLMVerdict`, `TYPED_EDGE_RECORD_VERSION`
- `crates/context-graph-storage/src/teleological/column_families.rs` — `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`, `CF_TYPED_EDGE_RECORDS`, `CF_TYPED_EDGE_VALIDATIONS`
- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_keys.rs` — `typed_edge_record_key`, `parse_typed_edge_record_key`, `TYPED_EDGE_KEY_LEN = 33`
- `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs` — contrastive CF write/read paths
- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_export.rs` — F1 export
- `crates/context-graph-storage/src/teleological/rocksdb_store/anomaly_derivation.rs` — F2 derive_anomalies_from_edges
- `crates/context-graph-storage/src/teleological/rocksdb_store/llm_validation.rs` — F4 LLM verdicts
- `crates/context-graph-storage/src/graph_edges/repository.rs` — `EdgeRepository::store_typed_edge`, `get_typed_edges_by_type`, `get_typed_edges_to`, secondary-index key construction
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` — `mine_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs`
- `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs` — `export_typed_edges_corpus`, `derive_anomalies_from_edges`, `validate_low_confidence_edges`, `list_typed_edge_records`
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs` — `get_typed_edges` (direction-aware filtering)

Cross-links: [05_embedding_subsystem.md](05_embedding_subsystem.md), [06_search_and_fusion.md](06_search_and_fusion.md), [07_graph_and_causal.md](07_graph_and_causal.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).

---

## 1. Scope

Two related subsystems share this document:

1. **Contrastive pair mining** — surfaces `(anchor, negative, similarity_profile)` triples where embedders disagree (e.g., high E1 semantic but low E5 causal). Mined pairs persist to `CF_CONTRASTIVE_PAIRS` with two secondary indexes (`by_kind`, `by_anchor`) for prefix scans. Used as training data for downstream contrastive / metric-learning loops and CAMD-DPO pair selection.
2. **Typed edges** — multi-relation edges between memories derived from per-embedder K-NN graph agreement. Each edge carries an 8-variant type tag (`GraphLinkEdgeType`), agreement metadata, and per-embedder similarity scores. Stored in `CF_TYPED_EDGES` with a secondary `[type:u8][source:16][target:16]` (33-byte) index for fast type-prefix scans. The training-data factory exports each typed edge into `CF_TYPED_EDGE_RECORDS` (Phase 4 F1) and joins LLM verdicts from `CF_TYPED_EDGE_VALIDATIONS` (F4).

Both subsystems live behind RocksDB-only paths; in-memory backends return tool errors.

---

## 2. AnomalyKind taxonomy (contrastive miner)

`AnomalyKind` (`contrastive/types.rs:48`) — six variants, encoded as `u8` 0–5 (the byte is also the first key of `CF_CONTRASTIVE_BY_KIND` so reordering is a breaking change):

| Discriminant | Variant | Pattern (high / low) | Snake-case (MCP `kinds`) |
| --- | --- | --- | --- |
| 0 | `SemanticButNotCausal` | High E1, low E5 | `semantic_but_not_causal` |
| 1 | `KeywordButNotParaphrase` | High E6 OR E13, low E10 | `keyword_but_not_paraphrase` |
| 2 | `CodeShapeButDifferentIntent` | High E7, low E1 | `code_shape_but_different_intent` |
| 3 | `EntitySharedButDifferentStructure` | High E11, low E8 | `entity_shared_but_different_structure` |
| 4 | `HdcRobustButSemanticDifferent` | High E9, low E1 | `hdc_robust_but_semantic_different` |
| 5 | `Other` | Sentinel for unmatched high/low | `other` |

`NUM_ANOMALY_KINDS = 6`. `AnomalyKind::all()` returns the variants in `as_u8` order. `from_u8` returns `None` for any other byte. `AnomalyKind::parse(s)` accepts the snake-case form (case-sensitive).

`classify_anomaly` (`contrastive/mining.rs:109`) priority is **stable** — the first matching rule wins, in this order:

1. `high(6) && low(0)` → `CodeShapeButDifferentIntent` (E7 first to avoid swallowing into `SemanticButNotCausal`)
2. `high(10) && low(7)` → `EntitySharedButDifferentStructure`
3. `high(8) && low(0)` → `HdcRobustButSemanticDifferent`
4. `(high(5) || high(12)) && low(9)` → `KeywordButNotParaphrase`
5. `high(0) && low(4)` → `SemanticButNotCausal`
6. otherwise → `Other`

---

## 3. Mining defaults

| Constant | Value | Source |
| --- | --- | --- |
| `DEFAULT_MAX_PAIRS` | `10_000` | `contrastive/types.rs:24` |
| `DEFAULT_MIN_DISAGREEMENT` | `0.3` | `contrastive/types.rs:27` |
| `DEFAULT_HIGH_THRESHOLD` | `0.6` | `contrastive/types.rs:30` |
| `DEFAULT_LOW_THRESHOLD` | `0.3` | `contrastive/types.rs:33` |
| `DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR` | `10` | `contrastive/types.rs:36` |
| `DEFAULT_CANDIDATE_POOL_SIZE` (handler) | `500` | `contrastive_tools.rs:57` |
| `HARD_MAX_CANDIDATE_POOL` | `50_000` | `contrastive_tools.rs:60` |
| `HARD_MAX_PAIRS` (handler cap) | `100_000` | `contrastive_tools.rs:63` |
| `HARD_MAX_TOP_K` | `500` | `contrastive_tools.rs:66` |
| `CONTRASTIVE_PAIR_VERSION` | `1` | `contrastive/mod.rs:49` |
| `GENERATOR_TAG` | `"cross_embedder_anomaly_v1"` | `contrastive/mining.rs:31` |

`MiningConfig::validate` enforces:
- `min_disagreement.is_finite() && 0.0 <= min_disagreement <= 1.0` — else `InvalidDisagreement`.
- `low_threshold.is_finite() && high_threshold.is_finite() && low_threshold < high_threshold` — else `InvalidThresholds`.

`mine_pair_from_candidate` short-circuits and returns `None` when:
- `anchor_id == negative_id`, or
- no embedder is above `high_threshold` or below `low_threshold` (no disagreement signal), or
- `disagreement = max(high_sims) - min(low_sims) < min_disagreement` or non-finite, or
- `cfg.kinds` is `Some(list)` and the classified kind is not in the list.

Surviving pairs get a fresh `Utc::now()` timestamp and the `cross_embedder_anomaly_v1` generator tag.

---

## 4. Similarity profile contract

`similarity_profile(anchor, negative)` (`contrastive/mining.rs:42`) returns a `[f32; NUM_EMBEDDERS]` (length 14) in the SRC-3 `[0, 1]` convention, with explicit per-embedder rules:

| Slot | Embedder | Method | Notes |
| --- | --- | --- | --- |
| 0 | E1 Semantic | SRC-3 cosine of `e1_semantic` | `(raw + 1) / 2` |
| 1–3 | E2/E3/E4 Temporal | SRC-3 cosine | symmetric |
| 4 | E5 Causal | SRC-3 cosine of `e5_causal_as_cause` (both sides) | matches constellation/topic profile |
| 5 | E6 Sparse | `jaccard_similarity(e6_sparse)` | already `[0, 1]` |
| 6 | E7 Code | SRC-3 cosine of `e7_code` | |
| 7 | E8 Graph | SRC-3 cosine of `e8_graph_as_source` (both sides) | source-side asymmetric |
| 8 | E9 HDC | SRC-3 cosine of `e9_hdc` | |
| 9 | E10 Multimodal | SRC-3 cosine of `e10_multimodal_paraphrase` (both sides) | paraphrase-side asymmetric |
| 10 | E11 Entity | SRC-3 cosine of `e11_entity` | |
| 11 | E12 LateInteraction | SRC-3 cosine of mean-pooled token vectors | `0.0` when either side has no tokens |
| 12 | E13 SPLADE | `jaccard_similarity(e13_splade)` | already `[0, 1]` |
| 13 | E14 BGE-M3 Dense | SRC-3 cosine of `e14_bge_m3_dense` | `0.0` when either side empty (legacy fingerprints) |

Final clamp: each entry is `clamp(0.0, 1.0)` and replaced with `0.0` if non-finite.

`src3_cosine` returns `0.0` on empty vectors, dimension mismatch, or zero-norm inputs (treated as no signal). `mean_pool_tokens` skips rows with mismatched dimensions; returns empty when no rows match the canonical first-row dim. `token_level_src3` mean-pools both sides then runs `src3_cosine`.

---

## 5. Contrastive storage layout

| CF | Key (length) | Value | Purpose |
| --- | --- | --- | --- |
| `CF_CONTRASTIVE_PAIRS` | `[anchor:16][negative:16]` (32 B) | `[CONTRASTIVE_PAIR_VERSION:u8][bincode ContrastivePair]` | Primary store; idempotent on composite key. |
| `CF_CONTRASTIVE_BY_KIND` | `[kind:u8][anchor:16][negative:16]` (33 B) | `[]` (empty pointer; presence = membership) | Prefix-scan by `AnomalyKind`. |
| `CF_CONTRASTIVE_BY_ANCHOR` | `[anchor:16][negative:16]` (32 B, indexed by anchor) | `[]` | Prefix-scan by anchor UUID. |

`ContrastivePair` fields:

| Field | Type |
| --- | --- |
| `anchor_id`, `negative_id` | `Uuid` |
| `anchor_text`, `negative_text` | `String` |
| `similarity_profile` | `[f32; NUM_EMBEDDERS]` (14 slots, all in `[0, 1]`) |
| `high_embedders`, `low_embedders` | `Vec<u8>` (slot indices that cleared / fell below threshold at mining time) |
| `disagreement_magnitude` | `f32` (`max(high_sim) - min(low_sim)`, ≥ 0 for stored pairs) |
| `anomaly_kind` | `AnomalyKind` |
| `mined_at` | `DateTime<Utc>` |
| `generator` | `String` (today: `cross_embedder_anomaly_v1`) |

Versioning: `[version:u8 = CONTRASTIVE_PAIR_VERSION][bincode]` — readers reject any other byte with `CoreError::SerializationError`. No automatic migration. Bincode is positional, so new fields can only be appended (PRD v6 §2.2).

---

## 6. Typed edge storage layout

| CF | Key (length) | Value | Purpose |
| --- | --- | --- | --- |
| `EMBEDDER_EDGES` (graph_edges crate) | `[embedder_id:u8][source:16]` (17 B, `EdgeStorageKey`) | bincode `Vec<EmbedderEdge>` | Per-embedder K-NN edges (K=20). |
| `TYPED_EDGES` | `[source:16][target:16]` (32 B, `TypedEdgeStorageKey`) | bincode `TypedEdge` | Primary multi-relation edge. |
| `TYPED_EDGES_BY_TYPE` | `[edge_type:u8][source:16][target:16]` (33 B) | `target_uuid` (16 B) | Secondary index — type-prefix scan. |
| `CF_TYPED_EDGE_RECORDS` | `[source:16][target:16][edge_type:u8]` (33 B, `typed_edge_record_key`) | `[TYPED_EDGE_RECORD_VERSION:u8][bincode TypedEdgeTrainingRecord]` | F1 export corpus row. |
| `CF_TYPED_EDGE_VALIDATIONS` | `[source:16][target:16][edge_type:u8]` (33 B, same key as records) | bincode `LLMEdgeValidation` | F4 LLM verdict on a typed edge. |

`TYPED_EDGE_KEY_LEN = 33` (`typed_edge_keys.rs:14`). The two F-suffix CFs share the **identical** key layout so a future migration only touches one place. Note the byte order differs between the F-suffix CFs (`[source][target][type]`) and the `TYPED_EDGES_BY_TYPE` secondary index (`[type][source][target]`) — the `TYPED_EDGES_BY_TYPE` ordering puts type first because it's prefix-scanned by type, while F-suffix CFs are point-looked-up by `(source, target, type)` triple.

Secondary-index key construction (`graph_edges/repository.rs:235`): `secondary_key[0] = edge_type as u8; secondary_key[1..17] = source; secondary_key[17..33] = target`. **Important fix** (BUG-3 from K-NN audit, 2026-02-26): the secondary key was originally `[type(1)][source(16)] = 17 bytes`, which **dropped edges when the same source had multiple same-type edges to different targets**. The current 33-byte form (extending with target UUID) preserves all edges; legacy 17-byte keys still parse for backward read compat in `repository.rs`.

`TypedEdge` payload (`graph_linking/typed_edge.rs`):

| Field | Type | Purpose |
| --- | --- | --- |
| `source`, `target` | `Uuid` | Endpoints |
| `edge_type` | `GraphLinkEdgeType` (u8 0–7) | One of 8 variants |
| `weight` | `f32 ∈ [0, 1]` | Computed via `WeightProjector` from agreeing embedder scores |
| `direction` | `DirectedRelation::{Symmetric, Forward, Backward}` | Asymmetric for `CausalChain` and `GraphConnected`, else `Symmetric` |
| `agreement_count` | `u8` | Number of agreeing embedders |
| `agreeing_embedders` | `u16` | Bitset (bit `i` = embedder `i`); `MultiAgreement` requires popcount ≥ 3 |
| `embedder_scores` | `[f32; 14]` | Full per-embedder similarity at edge-build time |

`GraphLinkEdgeType` (`graph_linking/edge_type.rs`):

| u8 | Variant | Primary embedder | Asymmetric? | Default threshold |
| --- | --- | --- | --- | --- |
| 0 | `SemanticSimilar` | E1 (idx 0) | no | 0.75 |
| 1 | `CodeRelated` | E7 (idx 6) | no | 0.70 |
| 2 | `EntityShared` | E11 (idx 10) | no | 0.65 |
| 3 | `CausalChain` | E5 (idx 4) | **yes** (cause→effect) | 0.60 |
| 4 | `GraphConnected` | E8 (idx 7) | **yes** (source→target) | 0.60 |
| 5 | `ParaphraseAligned` | E10 (idx 9) | no | 0.70 |
| 6 | `KeywordOverlap` | E6 (idx 5) (or E13 idx 12) | no | 0.50 |
| 7 | `MultiAgreement` | none (3+ embedders) | no | 0.60 |

`is_asymmetric()` returns true only for `CausalChain` and `GraphConnected`. `from_u8(8..=255)` returns `None`. `Display` returns the snake-case form (`semantic_similar`, `causal_chain`, `multi_agreement`, etc.).

---

## 7. K-NN graph construction

| Constant | Value | Source |
| --- | --- | --- |
| `KNN_K` | `20` | `graph_linking/mod.rs:53` |
| `NN_DESCENT_ITERATIONS` | `8` | `graph_linking/mod.rs:56` |
| `NN_DESCENT_SAMPLE_RATE` | `0.5` | `graph_linking/mod.rs:59` |
| `MIN_KNN_SIMILARITY` | `0.3` | `graph_linking/mod.rs:62` |

`NnDescent` builds per-embedder K-NN graphs with `O(N · K · iterations)` complexity. `build_asymmetric_knn` handles E5/E8 dual-vector embedders (two K-NN passes — one per direction — combined with `DirectedRelation::Forward` / `Backward` annotations).

`EdgeBuilder` consumes the per-embedder K-NN graphs, applies `EdgeBuilderConfig::thresholds` (per-embedder thresholds from `DEFAULT_EMBEDDER_THRESHOLDS`), and emits `TypedEdge`s. Multi-agreement edges trigger when 3+ embedders independently link the same `(source, target)` pair — the resulting `TypedEdge` carries `edge_type = MultiAgreement` and a populated `agreeing_embedders` bitset.

---

## 8. MCP tools

### 8.1 Contrastive miner (Phase 3)

| Tool | Behavior | Source |
| --- | --- | --- |
| `mine_contrastive_pairs` | Sample bounded candidate pool (`candidatePoolSize`, default 500, hard max 50K) once per run. For each anchor: compute full 14-slot similarity profile against every pool member; keep `topKCandidatesPerAnchor` (default 10, hard max 500) with largest disagreement; classify each via `classify_anomaly`; persist atomically across `CF_CONTRASTIVE_PAIRS` + `BY_KIND` + `BY_ANCHOR`. Returns `MiningSummary { pairs_stored, pairs_skipped_below_threshold, pairs_skipped_kind_filter, anchors_scanned, anchors_no_candidate, duration_ms }`. Optional `sessionFilter` restricts anchors by `SourceMetadata.session_id`. | `contrastive_tools.rs:70` |
| `list_contrastive_pairs` | Paginated `(anchorId, negativeId)` listing. Optional `kind` (uses `BY_KIND` prefix) or `anchorId` (uses `BY_ANCHOR` prefix) filters. `includeFullRecord=true` hydrates the bincode payload. | `contrastive_tools.rs:381` |
| `get_contrastive_pair` | Composite-key point lookup `(anchorId, negativeId)`. | `contrastive_tools.rs:524` |
| `count_contrastive_pairs` | Row count over the primary CF, or per-kind via `BY_KIND` prefix scan when `kind` is set. | `contrastive_tools.rs:569` |

The handler reproduces the `search_cross_embedder_anomalies` algorithm in-process rather than re-entering the search pipeline (avoids `O(anchors × search_pipeline)` cost and keeps the miner deterministic over the candidate pool). All four handlers downcast `Handlers::teleological_store` to `RocksDbTeleologicalStore`; non-RocksDB backends return a tool error.

### 8.2 Typed-edge training-data factory (Phase 4)

| Tool | Behavior | Source |
| --- | --- | --- |
| `export_typed_edges_corpus` | F1 — walk every row in `TYPED_EDGES`, optionally join content (`includeContent`), source metadata (`includeSourceMetadata`), causal mechanism type (`includeMechanismType`), and LLM validation (`joinLLMValidation`); persist one `TypedEdgeTrainingRecord` per edge into `CF_TYPED_EDGE_RECORDS`. Idempotent on the 33-byte composite key. `clearExisting=true` truncates first. Default `maxEdges=10_000`, hard cap `1_000_000`. | `typed_edges_tools.rs:62` |
| `derive_anomalies_from_edges` | F2 — walk `TYPED_EDGES`, classify each edge against the five expressible `AnomalyKind` patterns (`Other` is excluded — only named axes are emitted), persist matching pairs into `CF_CONTRASTIVE_PAIRS` via the standard contrastive write path with `generator="edge_derived"`. Hard cap `1_000_000`. | `typed_edges_tools.rs:338` |
| `validate_low_confidence_edges` | F4 — LLM-gated. Find typed edges with `weight < ceiling`, call the graph agent (Hermes-2-Pro-Mistral-7B), persist verdicts to `CF_TYPED_EDGE_VALIDATIONS`. Returns `{status:"skipped"}` when the LLM is unavailable (`#[cfg(not(feature = "llm"))]` or no model loaded). Default `maxEdges=100`, hard cap `10_000`. | `typed_edges_tools.rs` |
| `list_typed_edge_records` | Paginated read view of `CF_TYPED_EDGE_RECORDS`. Default limit 100, hard cap 1000. | `typed_edges_tools.rs:747` |

`EXPORTER_VERSION = "typed_edge_export_v1"` is stamped into each record's `exporter_version` field.

### 8.3 Typed-edge query (graph linking)

| Tool | Behavior | Source |
| --- | --- | --- |
| `get_typed_edges` | Returns typed edges incident to a memory. Direction-aware: | `graph_link_tools.rs:343` |

`get_typed_edges` direction handling (per BUG-1/BUG-2 fixes from K-NN audit, 2026-02-26):

| `direction` parameter | `edgeType` filter | Implementation |
| --- | --- | --- |
| `outgoing` (default) | unset | `EdgeRepository::get_typed_edges_from(memory_uuid)` — primary CF prefix scan by source. |
| `outgoing` | set | `EdgeRepository::get_typed_edges_by_type(memory_uuid, edge_type)` — `TYPED_EDGES_BY_TYPE` prefix scan with `[type:u8][source:16]` prefix. |
| `incoming` | unset | `EdgeRepository::get_typed_edges_to(memory_uuid)` — full scan over `TYPED_EDGES` filtered by target. |
| `incoming` | set | `get_typed_edges_to(memory_uuid)` then post-filter by `edge.edge_type() == filter` (BUG-1 fix — `TYPED_EDGES_BY_TYPE` is keyed by source, not target, so it cannot serve incoming queries). |
| `both` | unset | Outgoing primary + incoming full scan, deduplicated. |
| `both` | set | Outgoing via secondary index + incoming via full-scan-with-post-filter. |

For incoming edges, the response's `target_id` is set to `edge.source()` (the peer node), not the memory_uuid itself (BUG-2 fix — previously the response showed self for incoming edges). `min_weight` filters apply post-fetch.

---

## 9. TypedEdgeTrainingRecord (F1 export schema)

`crates/context-graph-core/src/typed_edge_export/mod.rs:38`. One row per typed edge.

| Field | Type | Notes |
| --- | --- | --- |
| `source_memory_id`, `target_memory_id` | `Uuid` | Edge endpoints |
| `edge_type` | `u8` | 0..=7 from `GraphLinkEdgeType::as_u8` |
| `edge_type_name` | `String` | Snake-case (e.g. `causal_chain`) |
| `weight` | `f32 ∈ [0, 1]` | Edge weight |
| `direction` | `u8` | `0=Symmetric, 1=Forward, 2=Backward` (matches `DirectedRelation`) |
| `embedder_scores` | `[f32; 14]` | SRC-3 `[0, 1]` profile |
| `agreement_count` | `u8` | Embedders agreeing above threshold |
| `agreeing_embedders` | `u16` | Bitset (bit 0 = E1, bit 13 = E14) |
| `source_content`, `target_content` | `String` | Empty when `includeContent=false` |
| `source_session_id`, `target_session_id` | `Option<String>` | From `SourceMetadata` |
| `source_type`, `target_type` | `Option<String>` | From `SourceMetadata::source_type` |
| `mechanism_type` | `Option<String>` | Joined from `CF_CAUSAL_RELATIONSHIPS` for `CausalChain` edges |
| `llm_validation` | `Option<LLMValidationSummary>` | Joined from `CF_TYPED_EDGE_VALIDATIONS` when `joinLLMValidation=true` |
| `exported_at` | `DateTime<Utc>` | Wall-clock at export |
| `exporter_version` | `String` | `"typed_edge_export_v1"` |

`LLMValidationSummary { validated_at, verdict: LLMVerdict, confidence ∈ [0, 1] }` is a compact projection of the full `LLMEdgeValidation` row. `LLMVerdict` is a serde-tagged enum; verdict values guide downstream typed-edge weight gating used in graph traversal scoring.

`TYPED_EDGE_RECORD_VERSION = 1`. Each record is on-disk as `[version:u8][bincode]`. Decoders reject any other byte with `CoreError::SerializationError`.

---

## 10. Generators (provenance field on contrastive records)

| `generator` value | Source tool |
| --- | --- |
| `cross_embedder_anomaly_v1` | `mine_contrastive_pairs` (the `GENERATOR_TAG` constant in `contrastive/mining.rs:31`) |
| `edge_derived` | `derive_anomalies_from_edges` (typed-edge → contrastive transform) |

Storage-layer write paths set `generator` before persisting, so consumers can filter by origin (e.g., CAMD-DPO miners select `cross_embedder_anomaly_v1` only).

---

## 11. Hard-negative scaling caveats

When mined contrastive pairs feed downstream training (CAMD-DPO, projection learning):

| Issue | Rule |
| --- | --- |
| `hard_negative_scale >= 2.0` with `temperature=0.05` causes catastrophic collapse | Use `hard_negative_scale = 1.0`. |
| Dropout regularization compresses absolute scores | Recalibrate causal-gate thresholds AFTER any change to dropout. |
| LoRA dropout increases VRAM usage | Drop `batch_size` from 16 → 8 when dropout tensors are tracked in the gradient graph. |
| Train/inference prefix mismatch | nomic-embed-text-v1.5 requires `"search_document:"` prefix in BOTH train and inference. |
| Model-dependent metrics never changing across runs | Indicates a hardcoded benchmark path; flag immediately. |

---

## 12. Per-stage best-stage selection (training)

Best-stage selection during multi-stage contrastive training **must** use `score_spread`, not `directional_accuracy`:

- `score_spread` = `mean(causal_scores) - mean(non_causal_scores)` — measures the practical discriminability the calibration thresholds (CAUSAL_THRESHOLD=0.04, NON_CAUSAL_THRESHOLD=0.008) can exploit.
- `directional_accuracy` only measures a binary classification surface that doesn't reflect score-magnitude separation.

When per-stage LoRA + projection checkpoints are saved, the projection head and LoRA must be paired by stage — picking the best LoRA from stage A with the projection from stage B yields incoherent thresholds (the selected stage's projection determines the score distribution). See lessons in `tasks/lessons.md`.

---

## 13. Integration with other subsystems

- **TrainingRecord** carries an `edge_type_distribution: [f32; 8]` over outgoing typed edges per memory — used by the learner subsystem to condition retrieval on local graph topology.
- **CAMD-DPO pair miners** ([19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md)) consume contrastive pairs by `generator` filter and by `AnomalyKind`, joined with constellation centroid scores.
- **LLM validation** feeds typed-edge **weight gating** used by graph traversal scoring (`search_connections`, `get_causal_chain`). Validations with `verdict = Reject` reduce or zero the traversal weight contribution.
- **K-NN graph linking** feeds `EdgeBuilder` which feeds `TYPED_EDGES`, which feeds `export_typed_edges_corpus` (F1) which feeds training. `derive_anomalies_from_edges` is a parallel path that reuses the same edge corpus to produce contrastive pairs.

---

## 14. Source notes

- **Secondary index key extension (BUG-3 fix, 2026-02-26).** `TYPED_EDGES_BY_TYPE` was `[type(1)][source(16)] = 17 bytes`, which dropped edges when the same source had multiple same-type edges. The current `[type(1)][source(16)][target(16)] = 33 bytes` form preserves all edges. The repository code (`graph_edges/repository.rs`) reads both legacy 17B and current 33B keys.
- **`get_typed_edges` direction-aware filtering (BUG-1, 2026-02-26).** The `edge_type` filter only fast-pathed via the source-keyed secondary index; for `direction=incoming`, the secondary index cannot serve the query (it's keyed by source, not target). Fix: post-filter incoming results by `edge_type`.
- **`get_typed_edges` peer-id fix (BUG-2, 2026-02-26).** For incoming edges, the response's peer-end was previously the memory_uuid itself; now it's `edge.source()` — the actual peer. Outgoing edges already used `edge.target()` correctly.
- **AnomalyKind discriminants are part of the on-disk contract.** Reordering `as_u8` requires bumping `CONTRASTIVE_PAIR_VERSION`. The byte appears as the first key of `CF_CONTRASTIVE_BY_KIND`.
- **Bincode positional layout — append-only.** New fields on `ContrastivePair`, `TypedEdgeTrainingRecord`, `LLMValidationSummary` must go at the end of the struct. No `skip_serializing_if` (bincode silently corrupts on optional fields per PRD v6 §2.2).
- **`Other` kind is excluded from `derive_anomalies_from_edges`.** Only the 5 named patterns produce contrastive pairs from typed edges; `Other` is a sentinel reserved for the in-process miner's catch-all bucket.
- **`MultiAgreement` requires `agreement_count >= 3`** AND no single primary embedder. The bitset distinguishes which 3+ embedders agreed.
- **Hard caps mirror MCP schema maxima.** Handler-side constants (`HARD_MAX_PAIRS`, `HARD_MAX_LIST_LIMIT`, etc.) are defense-in-depth — the JSON schema also enforces them but `additionalProperties:false` plus `maximum:` on numeric params catches malformed inputs first.
- **`search_cross_embedder_anomalies` vs `mine_contrastive_pairs`.** The MCP tool `search_cross_embedder_anomalies` is a query against existing fingerprints (returns scored memories). `mine_contrastive_pairs` writes to `CF_CONTRASTIVE_PAIRS` for downstream training. Both share the `(high, low) → AnomalyKind` classification logic but have different storage and rate-limit semantics.
