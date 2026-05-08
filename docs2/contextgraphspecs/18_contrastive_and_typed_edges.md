# Contrastive And Typed Edges

**Source files covered:**
- `crates/context-graph-core/src/contrastive/mod.rs` — module surface, `CONTRASTIVE_PAIR_VERSION = 1`
- `crates/context-graph-core/src/contrastive/types.rs` — `ContrastivePair`, `AnomalyKind`, `MiningConfig`, `MiningSummary`, `ContrastiveError`, `DEFAULT_*` constants, `NUM_ANOMALY_KINDS = 6`
- `crates/context-graph-core/src/contrastive/mining.rs` — `similarity_profile`, `classify_anomaly`, `mine_pair_from_candidate`, `GENERATOR_TAG = "cross_embedder_anomaly_v1"`, helpers (`src3_cosine`, `mean_pool_tokens`, `token_level_src3`)
- `crates/context-graph-core/src/typed_edge_export/mod.rs` — `TypedEdgeTrainingRecord`, `LLMValidationSummary`, `LLMVerdict`, `TYPED_EDGE_RECORD_VERSION = 1`
- `crates/context-graph-core/src/llm_edge_validation/mod.rs` — `LLMEdgeValidation`, `LLM_EDGE_VALIDATION_VERSION = 1`
- `crates/context-graph-core/src/graph_linking/edge_type.rs` — `GraphLinkEdgeType` (8 variants), `is_asymmetric`, `Display` snake-case
- `crates/context-graph-core/src/graph_linking/typed_edge.rs` — `TypedEdge` payload
- `crates/context-graph-storage/src/teleological/column_families.rs` — `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`, `CF_TYPED_EDGE_RECORDS`, `CF_TYPED_EDGE_VALIDATIONS`
- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_keys.rs` — `typed_edge_record_key`, `parse_typed_edge_record_key`, `TYPED_EDGE_KEY_LEN = 33`
- `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs` — atomic 3-CF write path
- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_export.rs` — F1 record persistence
- `crates/context-graph-storage/src/teleological/rocksdb_store/anomaly_derivation.rs` — F2 `derive_anomalies_from_edges`, `classify_edge_as_anomaly`
- `crates/context-graph-storage/src/teleological/rocksdb_store/llm_validation.rs` — F4 LLM verdict persistence
- `crates/context-graph-storage/src/graph_edges/repository.rs` — `EdgeRepository::store_typed_edge`, `get_typed_edges_by_type`, `get_typed_edges_to`, `iter_all_typed_edges`
- `crates/context-graph-mcp/src/tools/definitions/contrastive.rs` — 4 contrastive tool schemas
- `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs` — 4 typed-edge tool schemas
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` — handlers `call_mine_contrastive_pairs` / `call_list_contrastive_pairs` / `call_get_contrastive_pair` / `call_count_contrastive_pairs`
- `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs` — handlers `call_export_typed_edges_corpus` / `call_derive_anomalies_from_edges` / `call_validate_low_confidence_edges` / `call_list_typed_edge_records`
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs` — `get_typed_edges` (direction-aware)
- `crates/context-graph-storage/tests/contrastive_integration.rs` (8 async tests)
- `crates/context-graph-storage/tests/typed_edge_export_integration.rs` (4 async tests + 3 sync)
- `crates/context-graph-storage/tests/llm_validation_integration.rs` (4 async tests + 3 sync)

Cross-links: [04_storage_layer.md](04_storage_layer.md), [05_embedding_subsystem.md](05_embedding_subsystem.md), [07_graph_and_causal.md](07_graph_and_causal.md), [16_search_and_fusion.md](16_search_and_fusion.md).

---

## 1. Scope

Two related subsystems share this document:

1. **Contrastive pair mining** (`contrastive/`) — surfaces `(anchor, negative, similarity_profile)` triples where embedders disagree (e.g., high E1 semantic but low E5 causal). Pairs persist atomically across `CF_CONTRASTIVE_PAIRS` + two secondary indexes (`CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`). Used as training data for downstream contrastive / metric-learning loops and CAMD-DPO pair selection.
2. **Typed edges** (`typed_edge_export/`, `llm_edge_validation/`) — multi-relation edges between memories derived from per-embedder K-NN graph agreement. Each edge carries an 8-variant type tag (`GraphLinkEdgeType`), agreement metadata, and per-embedder similarity scores. The training-data factory exports each typed edge into `CF_TYPED_EDGE_RECORDS` (Phase 4 F1) and joins LLM verdicts from `CF_TYPED_EDGE_VALIDATIONS` (F4).

Both subsystems are RocksDB-only — every MCP handler downcasts `Handlers::teleological_store` to `RocksDbTeleologicalStore` and returns a tool error on other backends (`contrastive_tools.rs`, `typed_edges_tools.rs`).

---

## 2. AnomalyKind taxonomy

`AnomalyKind` (`contrastive/types.rs:48`) — six variants encoded as `u8` 0–5. The byte is the first key prefix of `CF_CONTRASTIVE_BY_KIND` so reordering is a breaking on-disk change requiring a `CONTRASTIVE_PAIR_VERSION` bump.

| Discriminant | Variant | Pattern (high / low) | Snake-case (MCP `kinds`) |
| --- | --- | --- | --- |
| 0 | `SemanticButNotCausal` | High E1, low E5 | `semantic_but_not_causal` |
| 1 | `KeywordButNotParaphrase` | High E6 OR E13, low E10 | `keyword_but_not_paraphrase` |
| 2 | `CodeShapeButDifferentIntent` | High E7, low E1 | `code_shape_but_different_intent` |
| 3 | `EntitySharedButDifferentStructure` | High E11, low E8 | `entity_shared_but_different_structure` |
| 4 | `HdcRobustButSemanticDifferent` | High E9, low E1 | `hdc_robust_but_semantic_different` |
| 5 | `Other` | Sentinel for unmatched high/low | `other` |

`NUM_ANOMALY_KINDS = 6`. `AnomalyKind::all()` returns the variants in `as_u8` order. `from_u8(v)` returns `None` for any byte ≥ 6. `AnomalyKind::parse(s)` accepts the snake-case form (case-sensitive).

### 2.1 Two classifiers, two contracts

`classify_anomaly` (`contrastive/mining.rs:109`) — **type-agnostic**, used by `mine_contrastive_pairs`. First matching rule wins:

1. `high(6) && low(0)` → `CodeShapeButDifferentIntent` (E7 first to avoid accidental `SemanticButNotCausal` for code).
2. `high(10) && low(7)` → `EntitySharedButDifferentStructure`.
3. `high(8) && low(0)` → `HdcRobustButSemanticDifferent`.
4. `(high(5) || high(12)) && low(9)` → `KeywordButNotParaphrase`.
5. `high(0) && low(4)` → `SemanticButNotCausal`.
6. otherwise → `Other`.

`classify_edge_as_anomaly` (`anomaly_derivation.rs:203`) — **edge-type-aware**, used by `derive_anomalies_from_edges`. Pattern is gated on `edge.edge_type()`:

| Edge type | Pattern | Resulting kind |
| --- | --- | --- |
| `SemanticSimilar` | `is_high(0) && is_low(4)` | `SemanticButNotCausal` |
| `KeywordOverlap` | `(is_high(5) || is_high(12)) && is_low(9)` | `KeywordButNotParaphrase` |
| `CodeRelated` | `is_high(6) && is_low(0)` | `CodeShapeButDifferentIntent` |
| `EntityShared` | `is_high(10) && is_low(7)` | `EntitySharedButDifferentStructure` |
| any other | `is_high(8) && is_low(0)` | `HdcRobustButSemanticDifferent` (cross-type fallback) |
| no match | — | `None` (skipped — `Other` is **never** produced from edges) |

Both classifiers operate on SRC-3 `[0, 1]` scores. The edge classifier uses `is_high = score >= cfg.high_threshold` and `is_low = score <= cfg.low_threshold` (inclusive); the offline miner uses strict `>` and `<` (exclusive).

---

## 3. Mining configuration

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
| Handler default `maxPairs` (when arg absent) | `1_000` | `contrastive_tools.rs:83` |
| `CONTRASTIVE_PAIR_VERSION` | `1` | `contrastive/mod.rs:49` |
| `GENERATOR_TAG` | `"cross_embedder_anomaly_v1"` | `contrastive/mining.rs:31` |

`MiningConfig::validate` enforces:
- `min_disagreement.is_finite() && 0.0 <= min_disagreement <= 1.0` — else `ContrastiveError::InvalidDisagreement`.
- `low_threshold.is_finite() && high_threshold.is_finite() && low_threshold < high_threshold` — else `ContrastiveError::InvalidThresholds`.

`mine_pair_from_candidate` returns `None` (and counts the result) when:
- `anchor_id == negative_id`, or
- no embedder is above `high_threshold` AND no embedder is below `low_threshold` (no disagreement signal), or
- `disagreement = max(high_sims) - min(low_sims) < min_disagreement` or non-finite, or
- `cfg.kinds = Some(list)` and the classified kind is not in the list.

Surviving pairs receive `Utc::now()` as `mined_at` and `cross_embedder_anomaly_v1` as `generator`.

### 3.1 `AnomalyDerivationConfig` (online derivation defaults)

`anomaly_derivation.rs:32`:

| Field | Default | Notes |
| --- | --- | --- |
| `high_threshold` | `0.60` | Mirrors `DEFAULT_HIGH_THRESHOLD` |
| `low_threshold` | `0.30` | Mirrors `DEFAULT_LOW_THRESHOLD` |
| `min_disagreement` | `0.30` | Mirrors `DEFAULT_MIN_DISAGREEMENT` |
| `max_pairs` | `10_000` | Mirrors `DEFAULT_MAX_PAIRS` |

A unit test asserts these stay in lockstep with the offline miner.

---

## 4. Similarity profile

`similarity_profile(anchor, negative)` (`contrastive/mining.rs:42`) returns `[f32; NUM_EMBEDDERS]` — length **14** (`teleological/types.rs:217`) — in the SRC-3 `[0, 1]` convention. Per-slot rules:

| Slot | Embedder | Method | Notes |
| --- | --- | --- | --- |
| 0 | E1 Semantic | SRC-3 cosine of `e1_semantic` | `(raw + 1) / 2` |
| 1 | E2 Temporal Recent | SRC-3 cosine of `e2_temporal_recent` | symmetric |
| 2 | E3 Temporal Periodic | SRC-3 cosine of `e3_temporal_periodic` | symmetric |
| 3 | E4 Temporal Positional | SRC-3 cosine of `e4_temporal_positional` | symmetric |
| 4 | E5 Causal | SRC-3 cosine of `e5_causal_as_cause` (both sides) | matches constellation/topic profile |
| 5 | E6 Sparse | `jaccard_similarity(e6_sparse)` | already `[0, 1]` |
| 6 | E7 Code | SRC-3 cosine of `e7_code` | symmetric |
| 7 | E8 Graph | SRC-3 cosine of `e8_graph_as_source` (both sides) | source-side asymmetric |
| 8 | E9 HDC | SRC-3 cosine of `e9_hdc` | symmetric |
| 9 | E10 Multimodal | SRC-3 cosine of `e10_multimodal_paraphrase` (both sides) | paraphrase-side asymmetric |
| 10 | E11 Entity | SRC-3 cosine of `e11_entity` | symmetric |
| 11 | E12 LateInteraction | mean-pool both sides' tokens, then SRC-3 cosine | `0.0` when either side has no tokens |
| 12 | E13 SPLADE | `jaccard_similarity(e13_splade)` | already `[0, 1]` |
| 13 | E14 BGE-M3 Dense | SRC-3 cosine of `e14_bge_m3_dense` | `0.0` when either side empty (legacy fingerprints) |

Final clamp: each entry is `clamp(0.0, 1.0)`; non-finite becomes `0.0`.

`src3_cosine` returns `0.0` on empty / dimension-mismatched / zero-norm inputs (no panics). `mean_pool_tokens` skips rows whose dim doesn't match the canonical first-row dim. `token_level_src3` mean-pools both sides then applies `src3_cosine`.

> **Note**: A few in-source comments still say "13-slot profile" (`mining.rs:10`, `mining.rs:37`), but the actual array is `[f32; NUM_EMBEDDERS]` with `NUM_EMBEDDERS = 14`. E14 is real and contributes when populated.

---

## 5. Contrastive storage layout

| CF | Key (length) | Value | Purpose |
| --- | --- | --- | --- |
| `contrastive_pairs` | `[anchor:16][negative:16]` (32 B) | `[CONTRASTIVE_PAIR_VERSION:u8][bincode ContrastivePair]` | Primary store. Idempotent on composite key. |
| `contrastive_by_kind` | `[kind:u8][anchor:16][negative:16]` (33 B) | `[]` (empty — presence = membership) | Prefix-scan by `AnomalyKind`. 1-byte prefix extractor. |
| `contrastive_by_anchor` | `anchor_uuid` (16 B) | `bincode(Vec<Uuid>)` of every mined negative for this anchor | O(1) "list every negative for this anchor" lookup. Deduped lazily on read. |

`ContrastivePair` (`contrastive/types.rs:144`):

| Field | Type |
| --- | --- |
| `anchor_id`, `negative_id` | `Uuid` |
| `anchor_text`, `negative_text` | `String` |
| `similarity_profile` | `[f32; NUM_EMBEDDERS]` (14 slots, all in `[0, 1]`) |
| `high_embedders`, `low_embedders` | `Vec<u8>` (slot indices that cleared / fell below threshold at mining time) |
| `disagreement_magnitude` | `f32` (`max(high_sim) - min(low_sim)`, ≥ 0 for stored pairs) |
| `anomaly_kind` | `AnomalyKind` |
| `mined_at` | `DateTime<Utc>` |
| `generator` | `String` (`cross_embedder_anomaly_v1` or `typed_edge_anomaly_derivation_v1`) |

Versioning: on-disk layout is `[version:u8 = CONTRASTIVE_PAIR_VERSION][bincode]`. Any other version byte is rejected with `CoreError::SerializationError` — no automatic migration. Bincode is positional, so new fields must append (per `tests/contrastive_integration.rs:108`-style round-trip tests + PRD v6 §2.2).

`store_contrastive_pair` writes the primary record + both secondary indexes in one `WriteBatch`. The `BY_ANCHOR` row is a `Vec<Uuid>` merged read-modify-write outside the batch; concurrent writers may race but the composite primary key is the correctness guarantor (duplicates dedupe on read).

---

## 6. Typed edge storage layout

| CF | Key (length) | Value | Purpose |
| --- | --- | --- | --- |
| `EMBEDDER_EDGES` (graph_edges crate) | `[embedder_id:u8][source:16]` (17 B, `EdgeStorageKey`) | bincode `Vec<EmbedderEdge>` | Per-embedder K-NN edges. |
| `TYPED_EDGES` (graph_edges crate) | `[source:16][target:16]` (32 B, `TypedEdgeStorageKey`) | bincode `TypedEdge` | Primary multi-relation edge. |
| `TYPED_EDGES_BY_TYPE` (graph_edges crate) | `[edge_type:u8][source:16][target:16]` (33 B) | `target_uuid` (16 B) | Secondary index for type-prefix scans. |
| `typed_edge_records` | `[source:16][target:16][edge_type:u8]` (33 B, `typed_edge_record_key`) | `[TYPED_EDGE_RECORD_VERSION:u8][bincode TypedEdgeTrainingRecord]` | F1 export corpus row. |
| `typed_edge_validations` | `[source:16][target:16][edge_type:u8]` (33 B, same shape) | `[LLM_EDGE_VALIDATION_VERSION:u8][bincode LLMEdgeValidation]` | F4 LLM verdict. |

`TYPED_EDGE_KEY_LEN = 33` (`typed_edge_keys.rs`). The two F-suffix CFs share an **identical** key layout so a future migration touches one place. Note the byte ordering: F-suffix CFs use `[source][target][type]` (point lookup by full triple) while `TYPED_EDGES_BY_TYPE` uses `[type][source][target]` (prefix-scan by type).

### 6.1 Secondary-index key fix (BUG-3, 2026-02-26)

`graph_edges/repository.rs:235`: secondary key is now `secondary_key[0] = edge_type as u8; secondary_key[1..17] = source; secondary_key[17..33] = target`. The original 17-byte form `[type(1)][source(16)]` **dropped edges when the same source had multiple same-type edges to different targets**. The current 33-byte form preserves all edges; legacy 17-byte keys still parse for backward read compat in `repository.rs`.

### 6.2 `TypedEdge` payload

`graph_linking/typed_edge.rs`:

| Field | Type | Purpose |
| --- | --- | --- |
| `source`, `target` | `Uuid` | Endpoints |
| `edge_type` | `GraphLinkEdgeType` (u8 0–7) | One of 8 variants |
| `weight` | `f32 ∈ [0, 1]` | Computed via `WeightProjector` from agreeing embedder scores |
| `direction` | `DirectedRelation` ∈ {`Symmetric`, `Forward`, `Backward`} | Asymmetric only for `CausalChain` and `GraphConnected` |
| `agreement_count` | `u8` | Number of agreeing embedders |
| `agreeing_embedders` | `u16` | Bitset (bit `i` = embedder `i`); `MultiAgreement` requires popcount ≥ 3 |
| `embedder_scores` | `[f32; 14]` | Full per-embedder similarity at edge-build time |

### 6.3 `GraphLinkEdgeType`

`graph_linking/edge_type.rs`:

| u8 | Variant | Primary embedder | Asymmetric? | Default threshold |
| --- | --- | --- | --- | --- |
| 0 | `SemanticSimilar` | E1 (idx 0) | no | 0.75 |
| 1 | `CodeRelated` | E7 (idx 6) | no | 0.70 |
| 2 | `EntityShared` | E11 (idx 10) | no | 0.65 |
| 3 | `CausalChain` | E5 (idx 4) | **yes** (cause→effect) | 0.60 |
| 4 | `GraphConnected` | E8 (idx 7) | **yes** (source→target) | 0.60 |
| 5 | `ParaphraseAligned` | E10 (idx 9) | no | 0.70 |
| 6 | `KeywordOverlap` | E6 (idx 5) (or E13 idx 12) | no | 0.50 |
| 7 | `MultiAgreement` | none (3+ embedders agree) | no | 0.60 |

`is_asymmetric()` returns true only for `CausalChain` and `GraphConnected`. `from_u8(8..=255)` returns `None`. `Display` returns the snake-case form (`semantic_similar`, `causal_chain`, `multi_agreement`, etc.).

---

## 7. K-NN graph construction (upstream of typed edges)

| Constant | Value | Source |
| --- | --- | --- |
| `KNN_K` | `20` | `graph_linking/mod.rs` |
| `NN_DESCENT_ITERATIONS` | `8` | `graph_linking/mod.rs` |
| `NN_DESCENT_SAMPLE_RATE` | `0.5` | `graph_linking/mod.rs` |
| `MIN_KNN_SIMILARITY` | `0.3` | `graph_linking/mod.rs` |

`NnDescent` builds per-embedder K-NN graphs with `O(N · K · iterations)` complexity. `build_asymmetric_knn` handles E5/E8 dual-vector embedders (two K-NN passes — one per direction — combined with `DirectedRelation::Forward` / `Backward` annotations).

`EdgeBuilder` consumes the per-embedder K-NN graphs, applies `EdgeBuilderConfig::thresholds` (per-embedder thresholds from `DEFAULT_EMBEDDER_THRESHOLDS`), and emits `TypedEdge`s. Multi-agreement edges trigger when 3+ embedders independently link the same `(source, target)` pair — the resulting `TypedEdge` carries `edge_type = MultiAgreement` and a populated `agreeing_embedders` bitset.

---

## 8. MCP tools

### 8.1 Contrastive miner (Phase 3, 4 tools)

| Tool | Behavior | Source |
| --- | --- | --- |
| `mine_contrastive_pairs` | Sample bounded candidate pool (`candidatePoolSize`, default 500, hard max 50K) once per run. For each anchor: compute full 14-slot similarity profile against every pool member; keep `topKCandidatesPerAnchor` (default 10, hard max 500) with largest disagreement; classify each via `classify_anomaly`; persist atomically across all three contrastive CFs. Returns `MiningSummary { pairs_stored, pairs_skipped_below_threshold, pairs_skipped_kind_filter, anchors_scanned, anchors_no_candidate, duration_ms }`. Optional `sessionFilter` restricts anchors by `SourceMetadata.session_id`. Schema default `maxPairs = 1000` (handler), JSON schema default also `1000`. | `contrastive_tools.rs:70`, def `definitions/contrastive.rs:20` |
| `list_contrastive_pairs` | Paginated `(anchorId, negativeId)` listing. Optional `kind` (uses `BY_KIND` prefix scan) or `anchorId` (uses `BY_ANCHOR` lookup) filters. `includeFull=true` hydrates the full bincode payload. `limit` default 100, max 10 000. | `contrastive_tools.rs:381` |
| `get_contrastive_pair` | Composite-key point lookup `(anchorId, negativeId)` against `CF_CONTRASTIVE_PAIRS`. | `contrastive_tools.rs:524` |
| `count_contrastive_pairs` | Row count over the primary CF, or per-kind via `BY_KIND` prefix scan when `kind` is set. | `contrastive_tools.rs:569` |

The handler reproduces the `search_cross_embedder_anomalies` algorithm in-process rather than re-entering the search pipeline (avoids `O(anchors × search_pipeline)` cost and keeps the miner deterministic over the candidate pool). All four handlers downcast `Handlers::teleological_store` to `RocksDbTeleologicalStore`; non-RocksDB backends return a tool error.

### 8.2 Typed-edge training-data factory (Phase 4, 4 tools)

| Tool | Behavior | Source |
| --- | --- | --- |
| `export_typed_edges_corpus` | F1 — walk every row in `TYPED_EDGES`, optionally join content (`includeContent`, default true), source metadata (`includeSourceMetadata`, default true), causal mechanism type (`includeMechanismType`, default true, joined from `CF_CAUSAL_RELATIONSHIPS`), and LLM validation (`joinLLMValidation`, default true); persist one `TypedEdgeTrainingRecord` per edge into `CF_TYPED_EDGE_RECORDS`. Idempotent on the 33-byte composite key. `clearExisting=true` truncates first. Default `maxEdges=10_000`, hard cap `1_000_000`. Output is binary on-disk (version byte + bincode); no external files written. | `typed_edges_tools.rs:62`, def `definitions/typed_edges.rs:25` |
| `derive_anomalies_from_edges` | F2 — walk `TYPED_EDGES` via `EdgeRepository::iter_all_typed_edges()`, classify each edge against five expressible `AnomalyKind` patterns (`Other` is excluded — only named axes are emitted), persist matching pairs into `CF_CONTRASTIVE_PAIRS` via the standard contrastive write path with `generator="typed_edge_anomaly_derivation_v1"`. Edges whose source/target content is missing are counted under `skipped_missing_content`. Hard cap `1_000_000`. **The `kinds` filter is declared in the schema but rejected with `INVALID_PARAMS` at the handler** (storage layer doesn't yet support per-kind filtering — `typed_edges_tools.rs:357`). Returns `AnomalyDerivationSummary { edges_scanned, pairs_written, skipped_below_threshold, skipped_missing_content, per_kind_counts: HashMap<AnomalyKind, usize>, duration_ms }`. | `typed_edges_tools.rs:340`, `anomaly_derivation.rs:79` |
| `validate_low_confidence_edges` | F4 — LLM-gated. Find typed edges with `weight < weightCeiling` (default 0.60), call the graph agent (Hermes-2-Pro-Mistral-7B via `service.llm().analyze_relationship`), persist `LLMEdgeValidation` rows to `CF_TYPED_EDGE_VALIDATIONS`. Skips edges already validated (idempotent). Returns `{status:"skipped"}` when `feature="llm"` is disabled at compile time, when `graph_discovery_service` is not wired, or when the LLM is not loaded. Default `maxEdges=100`, hard cap `10_000`. Optional `sessionFilter` restricts to edges whose source's `SourceMetadata.session_id` matches. | `typed_edges_tools.rs:448`, def `definitions/typed_edges.rs:130` |
| `list_typed_edge_records` | Paginated read view of `CF_TYPED_EDGE_RECORDS`. Returns composite keys `(source_memory_id, target_memory_id, edge_type, edge_type_name)` in RocksDB iteration order. Set `includeFull=true` to hydrate each row's `TypedEdgeTrainingRecord`. Default `limit=100`, hard cap `1000`. | `typed_edges_tools.rs:747`, def `definitions/typed_edges.rs:166` |

`EXPORTER_VERSION = "typed_edge_export_v1"` (`typed_edges_tools.rs:53`) is stamped into each record's `exporter_version` field.

### 8.3 Typed-edge query (graph linking)

| Tool | Behavior | Source |
| --- | --- | --- |
| `get_typed_edges` | Returns typed edges incident to a memory. Direction-aware (see table below). | `graph_link_tools.rs:343` |

Direction handling (per BUG-1/BUG-2 fixes from K-NN audit, 2026-02-26):

| `direction` | `edgeType` filter | Implementation |
| --- | --- | --- |
| `outgoing` (default) | unset | `EdgeRepository::get_typed_edges_from(memory_uuid)` — primary CF prefix scan by source. |
| `outgoing` | set | `EdgeRepository::get_typed_edges_by_type(memory_uuid, edge_type)` — `TYPED_EDGES_BY_TYPE` prefix scan with `[type:u8][source:16]` prefix. |
| `incoming` | unset | `EdgeRepository::get_typed_edges_to(memory_uuid)` — full scan over `TYPED_EDGES` filtered by target. |
| `incoming` | set | `get_typed_edges_to(memory_uuid)` then post-filter by `edge.edge_type() == filter` (BUG-1 fix — `TYPED_EDGES_BY_TYPE` is keyed by source, not target, so it cannot serve incoming queries). |
| `both` | unset | Outgoing primary + incoming full scan, deduplicated. |
| `both` | set | Outgoing via secondary index + incoming via full-scan-with-post-filter. |

For incoming edges, the response's `target_id` is set to `edge.source()` (the peer node), not the memory_uuid itself (BUG-2 fix — previously the response showed self for incoming edges). `min_weight` filters apply post-fetch.

---

## 9. `TypedEdgeTrainingRecord` (F1 export schema)

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

`TYPED_EDGE_RECORD_VERSION = 1`. On-disk: `[version:u8][bincode]`. Decoders reject any other byte with `CoreError::SerializationError`.

---

## 10. LLM validation schemas

Two related types share the same on-disk key shape but differ in field set.

### 10.1 `LLMEdgeValidation` (full row in `CF_TYPED_EDGE_VALIDATIONS`)

`crates/context-graph-core/src/llm_edge_validation/mod.rs:35`:

| Field | Type | Notes |
| --- | --- | --- |
| `validated_at` | `DateTime<Utc>` | LLM call wall-clock |
| `verdict` | `LLMVerdict` | Tagged enum (see below) |
| `confidence` | `f32 ∈ [0, 1]` | LLM confidence |
| `rationale` | `String` | 1-3 sentence justification from grammar-constrained output |
| `auto_derived_weight` | `f32` | Edge weight at validation time (drift-audit hook) |
| `validator_version` | `String` | Currently `"Hermes-2-Pro-Mistral-7B@2026-04"` (set in `typed_edges_tools.rs:694`) |
| `prompt_hash` | `[u8; 32]` | SHA-256 of the prompt actually sent (`compute_prompt_hash` in `typed_edges_tools.rs`); detects template drift |

`LLM_EDGE_VALIDATION_VERSION = 1`. On-disk: `[version:u8][bincode]`. The hash is computed from `b"graph_agent::analyze_relationship::v1\n"` plus the source/target content.

### 10.2 `LLMValidationSummary` (compact projection embedded in `TypedEdgeTrainingRecord`)

`typed_edge_export/mod.rs:94`:

| Field | Type |
| --- | --- |
| `validated_at` | `DateTime<Utc>` |
| `verdict` | `LLMVerdict` |
| `confidence` | `f32 ∈ [0, 1]` |
| `rationale` | `String` |
| `validator_version` | `String` |

The summary drops `auto_derived_weight` and `prompt_hash` because consumers of the F1 corpus don't need the audit hooks; the full row stays available via direct lookup on `CF_TYPED_EDGE_VALIDATIONS`.

### 10.3 `LLMVerdict`

`typed_edge_export/mod.rs:114` — serde-tagged enum, bincode-encoded as variant index + payload:

| Variant | Encoding | Meaning |
| --- | --- | --- |
| `Valid` | unit | LLM confirmed the auto-derived edge |
| `Invalid` | unit | LLM rejected the auto-derived edge |
| `Reclassify { new_edge_type: u8 }` | tagged | Two memories are linked but classified under a different `GraphLinkEdgeType` (0..=7) |

`LLMVerdict::as_str()` returns `"valid" | "invalid" | "reclassify"`. Adding or reordering variants requires bumping both `LLM_EDGE_VALIDATION_VERSION` and `TYPED_EDGE_RECORD_VERSION`.

### 10.4 LLM call path (Hermes-2-Pro-Mistral-7B)

`call_validate_low_confidence_edges` (`typed_edges_tools.rs:448`) is gated three ways:

1. `#[cfg(not(feature = "llm"))]` returns `{status:"skipped", reason:"llm feature not enabled"}` at compile time.
2. `self.graph_discovery_service` is `None` → `{status:"skipped", reason:"graph-agent API not exposed for single-edge validation"}`.
3. `service.llm().is_loaded()` is false → `{status:"skipped", reason:"graph-agent LLM not loaded"}`.

When all gates pass, the handler:

1. Walks `iter_all_typed_edges()` filtering by `weight < weightCeiling`, `sessionFilter`, and "no existing validation row".
2. Fetches both endpoints' content via `teleological_store.get_content()`.
3. Computes `prompt_hash` from the source/target content + a stable salt.
4. Calls `llm.analyze_relationship(source, target).await`, mapping `analysis.has_connection` to `LLMVerdict::Valid | Invalid` (the current handler does not emit `Reclassify`).
5. Persists `LLMEdgeValidation` to `CF_TYPED_EDGE_VALIDATIONS` keyed by `(source, target, edge_type)`.

---

## 11. Generators (provenance on contrastive records)

| `generator` value | Producer | Notes |
| --- | --- | --- |
| `cross_embedder_anomaly_v1` | `mine_contrastive_pairs` (`GENERATOR_TAG` in `mining.rs:31`) | Offline miner — anchors × candidate-pool sweep with full per-embedder profile |
| `typed_edge_anomaly_derivation_v1` | `derive_anomalies_from_edges` (`anomaly_derivation.rs:163`) | Online derivation — walks `CF_TYPED_EDGES` and reuses the edge's `embedder_scores()` array as the similarity profile |

Storage-layer write paths set `generator` before persisting, so consumers can filter by origin (e.g., CAMD-DPO miners select `cross_embedder_anomaly_v1` only).

---

## 12. Export pipeline (records → downstream training)

The F1 export is **RocksDB-internal**; there is no JSONL or Parquet emitter in the current source. `export_typed_edges_corpus` writes `[TYPED_EDGE_RECORD_VERSION:u8][bincode TypedEdgeTrainingRecord]` rows into `CF_TYPED_EDGE_RECORDS` keyed by `[source][target][edge_type]`. Consumers (training scripts, CAMD-DPO miners) use:

- `RocksDbTeleologicalStore::list_typed_edge_record_keys()` for key enumeration.
- `RocksDbTeleologicalStore::get_typed_edge_record(source, target, edge_type)` for point reads.
- The MCP tool `list_typed_edge_records` for paginated remote access (set `includeFull=true` to hydrate).

Records are bincode-positional; new fields must append at the end of the struct. `clear_all_typed_edge_records()` exists for clean re-exports (single `WriteBatch` deleting every row).

---

## 13. Integration tests

Three integration test files exercise the persistence layer against a real RocksDB instance:

| File | Async tests | Coverage |
| --- | --- | --- |
| `tests/contrastive_integration.rs` (416 lines) | `roundtrip_pair_preserves_all_fields`, `store_populates_all_three_cfs_atomically`, `delete_removes_from_all_three_cfs`, `clear_all_contrastive_pairs_empties_all_three`, `list_pairs_by_kind_filters_correctly`, `pairs_for_anchor_returns_all_negatives`, `store_is_idempotent_for_same_key` (+ 4 sync key-shape tests) | Atomicity of 3-CF write path, secondary-index correctness, idempotency |
| `tests/typed_edge_export_integration.rs` (219 lines) | `stores_all_eight_edge_type_records`, `roundtrip_is_byte_identical`, `get_returns_none_for_missing_key`, `clear_all_empties_the_cf` (+ 3 sync) | All 8 `GraphLinkEdgeType` values round-trip; key-layout determinism |
| `tests/llm_validation_integration.rs` (235 lines) | `stores_all_three_verdict_variants`, `get_returns_none_for_missing_key`, `overwrite_is_idempotent`, `clear_all_empties_the_cf` (+ 3 sync) | All `LLMVerdict` variants (`Valid`, `Invalid`, `Reclassify { new_edge_type }`) round-trip; same-key overwrite |

---

## 14. Cross-subsystem integration

- **TrainingRecord** carries an `edge_type_distribution: [f32; 8]` over outgoing typed edges per memory — used by the learner subsystem to condition retrieval on local graph topology. (See [07_graph_and_causal.md](07_graph_and_causal.md).)
- **CAMD-DPO pair miners** consume contrastive pairs by `generator` filter and by `AnomalyKind`, joined with constellation centroid scores.
- **LLM validation** feeds typed-edge **weight gating** used by graph traversal scoring (`search_connections`, `get_causal_chain`). Validations with `verdict = Invalid` reduce or zero the traversal weight contribution; `Reclassify` retags the edge for downstream consumers.
- **K-NN graph linking** feeds `EdgeBuilder` which feeds `TYPED_EDGES`, which feeds `export_typed_edges_corpus` (F1) which feeds training. `derive_anomalies_from_edges` is a parallel path that reuses the same edge corpus to produce contrastive pairs.

---

## 15. Source notes

- **Secondary index key extension (BUG-3 fix, 2026-02-26).** `TYPED_EDGES_BY_TYPE` was `[type(1)][source(16)] = 17 bytes`, which dropped edges when the same source had multiple same-type edges. The current `[type(1)][source(16)][target(16)] = 33 bytes` form preserves all edges. `repository.rs` reads both legacy 17B and current 33B keys for compat.
- **`get_typed_edges` direction-aware filtering (BUG-1, 2026-02-26).** The `edge_type` filter only fast-pathed via the source-keyed secondary index; for `direction=incoming`, the secondary index cannot serve the query (it's keyed by source, not target). Fix: post-filter incoming results by `edge_type`.
- **`get_typed_edges` peer-id fix (BUG-2, 2026-02-26).** For incoming edges, the response's peer-end was previously the memory_uuid itself; now it's `edge.source()`. Outgoing edges already used `edge.target()` correctly.
- **AnomalyKind discriminants are part of the on-disk contract.** Reordering `as_u8` requires bumping `CONTRASTIVE_PAIR_VERSION`. The byte appears as the first key of `CF_CONTRASTIVE_BY_KIND`.
- **Bincode positional layout — append-only.** New fields on `ContrastivePair`, `TypedEdgeTrainingRecord`, `LLMEdgeValidation`, `LLMValidationSummary` go at the end of the struct. No `skip_serializing_if` (bincode silently corrupts on optional fields per PRD v6 §2.2).
- **`Other` kind is never produced from edges.** `derive_anomalies_from_edges` excludes `Other` — only the 5 named patterns produce contrastive pairs from typed edges. `Other` is reserved for the in-process miner's catch-all bucket.
- **`MultiAgreement` requires `agreement_count >= 3`** AND no single primary embedder. The bitset distinguishes which embedders agreed.
- **Hard caps mirror MCP schema maxima.** Handler-side constants (`HARD_MAX_PAIRS`, `HARD_MAX_LIST_LIMIT`, `HARD_MAX_VALIDATE_EDGES`, etc.) are defense-in-depth — the JSON schema also enforces them but `additionalProperties:false` plus `maximum:` on numeric params catches malformed inputs first.
- **`derive_anomalies_from_edges` `kinds` filter is unimplemented.** The MCP schema declares the parameter, but the handler returns `INVALID_PARAMS` if it's present (`typed_edges_tools.rs:357`). Filter at the read side via `list_pairs_by_kind` instead.
- **Validator version is hardcoded.** `typed_edges_tools.rs:694` writes `"Hermes-2-Pro-Mistral-7B@2026-04"`; the doc comment in `mod.rs` shows the longer canonical form `"Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"`. The hash check protects against undetected drift.
- **The `LLMValidationSummary` is a strict subset of `LLMEdgeValidation`.** Joining F1 records does not expose `auto_derived_weight` or `prompt_hash` to consumers; those stay in `CF_TYPED_EDGE_VALIDATIONS` for audit-only access.
- **`search_cross_embedder_anomalies` vs `mine_contrastive_pairs`.** The MCP tool `search_cross_embedder_anomalies` is a query against existing fingerprints (returns scored memories). `mine_contrastive_pairs` writes to `CF_CONTRASTIVE_PAIRS` for downstream training. Both share the `(high, low) → AnomalyKind` classification logic but differ in storage and rate-limit semantics.
