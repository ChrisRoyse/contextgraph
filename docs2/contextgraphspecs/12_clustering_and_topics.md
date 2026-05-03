# Clustering And Topics

**Source files covered:**
- `crates/context-graph-core/src/clustering/mod.rs` — public surface and re-exports
- `crates/context-graph-core/src/clustering/manager.rs` — `MultiSpaceClusterManager`, `ManagerParams`, `MAX_WEIGHTED_AGREEMENT`, `TOPIC_THRESHOLD`, `MAX_TOPICS`, `MAX_EMBEDDINGS_PER_SPACE`, `DEFAULT_RECLUSTER_THRESHOLD`, `InsertResult`, `ReclusterResult`, `FdmcResult`
- `crates/context-graph-core/src/clustering/topic.rs` — `Topic`, `TopicProfile`, `TopicPhase`, `TopicStability`, `MAX_TOPIC_DEPTH`, `TOPIC_SILHOUETTE_THRESHOLD`, `build_topic_hierarchy`
- `crates/context-graph-core/src/clustering/synthesizer.rs` — `TopicSynthesizer`, `DEFAULT_MERGE_THRESHOLD`, `DEFAULT_MIN_SILHOUETTE`
- `crates/context-graph-core/src/clustering/cluster.rs` — `Cluster` (per-embedder cluster + centroid)
- `crates/context-graph-core/src/clustering/membership.rs` — `ClusterMembership`
- `crates/context-graph-core/src/clustering/birch.rs` — `BIRCHTree`, `BIRCHParams`, `ClusteringFeature`, `BIRCHEntry`, `BIRCHNode`, `birch_defaults`
- `crates/context-graph-core/src/clustering/hdbscan.rs` — `HDBSCANClusterer`, `HDBSCANParams`, `ClusterSelectionMethod::{EOM, Leaf}`, `hdbscan_defaults`
- `crates/context-graph-core/src/clustering/fingerprint_matrix.rs` — `FingerprintMatrix`, `FingerprintMatrixConfig`, `AggregationStrategy::{Weighted, Max, Mean, Product, Min}`, `SimilarityStats`, `build_fingerprint_matrix`, `update_matrix_incremental`
- `crates/context-graph-core/src/clustering/stability.rs` — `TopicStabilityTracker`, `TopicSnapshot`, `DEFAULT_CHURN_THRESHOLD`, `DEFAULT_ENTROPY_THRESHOLD`, `DEFAULT_ENTROPY_DURATION_SECS`, `SNAPSHOT_RETENTION_HOURS`, `MAX_SNAPSHOTS`
- `crates/context-graph-core/src/clustering/persistence.rs` — `PersistedTopicPortfolio`, `TopicPortfolio`, `PersistenceError`
- `crates/context-graph-core/src/retrieval/divergence.rs` — `DIVERGENCE_SPACES`, `DivergenceAlert`, `DivergenceSeverity`, `MAX_SUMMARY_LEN`
- `crates/context-graph-core/src/retrieval/detector.rs` — `DivergenceDetector`, `RecentMemory`, `is_divergence_space`
- `crates/context-graph-core/src/retrieval/multi_space.rs` — `MultiSpaceSimilarity::compute_relevance_score`, `compute_weighted_similarity`
- `crates/context-graph-mcp/src/handlers/tools/topic_tools.rs` — `call_get_topic_portfolio`, `call_get_topic_stability`, `call_detect_topics`, `call_get_divergence_alerts`
- `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` — `call_get_embedder_clusters`

Cross-links: [05_embedding_subsystem.md](05_embedding_subsystem.md), [06_search_and_fusion.md](06_search_and_fusion.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [13_teleological_subsystem.md](13_teleological_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Scope

The clustering subsystem maintains per-embedder clusters across all 14 embedding spaces, synthesizes cross-space "topics" via weighted agreement, tracks topic stability and lifecycle phases, and feeds dream-consolidation triggers. It exposes four MCP tools (`detect_topics`, `get_topic_portfolio`, `get_topic_stability`, `get_divergence_alerts`) plus per-embedder cluster inspection (`get_embedder_clusters`).

The pipeline is two-tier:

1. **Per-embedder clustering** (BIRCH for incremental, HDBSCAN for batch reclustering).
2. **Cross-space topic synthesis** (weighted-agreement formula across the 14-slot fingerprint).

Per-memory `TopicProfile` (14 × f32 strength array) is stored in `CF_TOPIC_PROFILES`; per-session `TopicPortfolio` snapshots are persisted in `CF_TOPIC_PORTFOLIO`.

---

## 2. Constants

| Constant | Value | Source |
| --- | --- | --- |
| `TOPIC_THRESHOLD` | `2.5` | `clustering/manager.rs:80` |
| `MAX_WEIGHTED_AGREEMENT` | `9.5` | `clustering/manager.rs:77` (`8 SEMANTIC × 1.0 + 2 RELATIONAL × 0.5 + 1 STRUCTURAL × 0.5 = 9.5`; updated post-E14 from 8.5) |
| `MAX_TOPICS` | `10_000` | `clustering/manager.rs:93` (private — silently skips new topics once exceeded) |
| `MAX_EMBEDDINGS_PER_SPACE` | `50_000` | `clustering/manager.rs:87` (oldest evicted; ~2.6 GB worst-case across 13 spaces) |
| `DEFAULT_RECLUSTER_THRESHOLD` | `100` | `clustering/manager.rs:71` (incremental updates before HDBSCAN runs) |
| `MAX_TOPIC_DEPTH` | see `topic.rs` | Hierarchy depth cap for `build_topic_hierarchy` |
| `TOPIC_SILHOUETTE_THRESHOLD` | `0.3` | `clustering/topic.rs:28` (constitution `clustering.parameters.silhouette_threshold`) |
| `DEFAULT_MERGE_THRESHOLD` | `0.9` | `clustering/synthesizer.rs:35` (cosine sim of `TopicProfile::strengths`) |
| `DEFAULT_MIN_SILHOUETTE` | `0.3` | `clustering/synthesizer.rs:38` |
| HDBSCAN default `min_cluster_size` | `3` | `clustering/hdbscan.rs:106` |
| HDBSCAN default `min_samples` | `2` | `clustering/hdbscan.rs:107` |
| HDBSCAN default `cluster_selection_method` | `ClusterSelectionMethod::EOM` | `clustering/hdbscan.rs:108` |
| `DEFAULT_CHURN_THRESHOLD` | `0.5` | `clustering/stability.rs:25` |
| `DEFAULT_ENTROPY_THRESHOLD` | `0.7` | `clustering/stability.rs:28` |
| `DEFAULT_ENTROPY_DURATION_SECS` | `300` (5 min) | `clustering/stability.rs:31` |
| `SNAPSHOT_RETENTION_HOURS` | `24` | `clustering/stability.rs:34` |
| `MAX_SNAPSHOTS` | `1440` | `clustering/stability.rs:38` (1 snap/min × 24 h hard cap) |
| `MIN_MEMORIES_FOR_CLUSTERING` | `3` | `topic_tools.rs:37` (matches HDBSCAN `min_cluster_size`) |
| `MIN_MEMORIES_FOR_DIVERGENCE` | `2` | `topic_tools.rs:40` |
| `MAX_SUMMARY_WORDS` | `50` | `topic_tools.rs:43` (divergence-alert content truncation) |
| `DIVERGENCE_SPACES` count | `7` | `retrieval/divergence.rs:26` (E1, E6, E7, E10, E12, E13, E14) |

---

## 3. Category weights

Topic weights derive from the embedder category (`crate::embeddings::category::category_for`). Per AP-60, temporal embedders (E2/E3/E4) have `topic_weight=0.0` and **never** count toward topic detection.

| Category | Weight | Embedders |
| --- | --- | --- |
| `Semantic` | `1.0` | E1, E5, E6, E7, E10, E12, E13, E14 (8 spaces) |
| `Temporal` | `0.0` | E2, E3, E4 (3 spaces — excluded per AP-60) |
| `Relational` | `0.5` | E8, E11 (2 spaces) |
| `Structural` | `0.5` | E9 (1 space) |
| `PipelineStage` | n/a | E12, E13 also classified as pipeline-stage in `EmbedderCategory`; see `insight_annotation.rs` |

Maximum weighted agreement = `8 × 1.0 + 2 × 0.5 + 1 × 0.5 = 9.5`. Topic confidence = `weighted_agreement / MAX_WEIGHTED_AGREEMENT`.

A topic emerges when `TopicProfile::weighted_agreement() >= TOPIC_THRESHOLD (2.5)` — the minimum is hit by 3 semantic spaces in agreement, or 2 semantic + 1 relational, or 5 relational/structural.

---

## 4. Algorithms

### 4.1 BIRCH (incremental)

`crates/context-graph-core/src/clustering/birch.rs` — used per embedder for streaming inserts.

| Parameter | Default | Purpose |
| --- | --- | --- |
| Branching factor | per `birch_defaults` | Max children per internal node |
| Threshold (T) | per `birch_defaults` | Max radius for a CF leaf entry |
| Memory limit | per `birch_defaults` | Tree size cap |

Each node holds a `ClusteringFeature` `(N, LS, SS)` — count, linear-sum vector, squared-sum scalar — that supports incremental centroid + radius computation in O(1).

### 4.2 HDBSCAN (batch reclustering)

`crates/context-graph-core/src/clustering/hdbscan.rs`. Parameters in `HDBSCANParams`:

| Field | Default | Source |
| --- | --- | --- |
| `min_cluster_size` | `3` | constitution `clustering.parameters.min_cluster_size` |
| `min_samples` | `2` | must be `<= min_cluster_size`, validated by `validate()` |
| `cluster_selection_method` | `EOM` | `Excess of Mass` (general purpose) vs `Leaf` (granular) |

Per-embedder defaults via `HDBSCANParams::for_embedder(embedder)` — sparse spaces (E6, E13) bump `min_cluster_size` to 5 because high-dimensional sparse data needs larger clusters; all other spaces use the constitution default (`3`).

Algorithm:
1. Compute core distances (k-th nearest neighbor in `min_samples` space).
2. Build mutual reachability graph: `MR(a,b) = max(core(a), core(b), dist(a,b))`.
3. Run Prim's MST.
4. Extract clusters via Union-Find respecting `min_cluster_size`.
5. Validate each cluster's average silhouette ≥ `TOPIC_SILHOUETTE_THRESHOLD (0.3)`; below-threshold clusters are dropped (returned as noise).

`MultiSpaceClusterManager::recluster()` runs HDBSCAN per embedder when `updates_since_recluster >= recluster_threshold` (default 100). `needs_recluster()` exposes the predicate; `recluster_fingerprint_matrix` runs over a precomputed per-pair matrix.

### 4.3 Fingerprint matrix

`crates/context-graph-core/src/clustering/fingerprint_matrix.rs`:

| Aggregation | Formula |
| --- | --- |
| `Weighted` | `Σ category_weight × score` (default; matches topic weighted-agreement) |
| `Max` | `max(scoreᵢ)` |
| `Mean` | `Σ score / N` |
| `Product` | `Π score` |
| `Min` | `min(scoreᵢ)` |

`FingerprintMatrix` stores pairwise per-embedder similarity in a triangular layout. `SimilarityStats` provides distribution diagnostics for thresholding.

### 4.4 Topic synthesis

`TopicSynthesizer::synthesize_topics(memberships)` — `clustering/synthesizer.rs`:

1. Group memories by their per-embedder `ClusterMembership` array `[Option<ClusterMembership>; 14]`.
2. For each cluster-tuple, build a `TopicProfile` (14-slot strength array).
3. Compute `weighted_agreement()` — `Σ strength_i × topic_weight_i`, clamped to `[0, MAX_WEIGHTED_AGREEMENT]`, NaN-safe (returns 0.0 per AP-10).
4. Reject if `< TOPIC_THRESHOLD (2.5)` OR average silhouette `< min_silhouette (0.3)`.
5. Merge candidates whose `TopicProfile::similarity` (cosine of strength arrays, NaN-clamped to `[0, 1]`) `>= merge_similarity_threshold (0.9)`.

Key methods on `TopicProfile`:

| Method | Purpose |
| --- | --- |
| `new(strengths)` | Constructor (clamps each entry to `[0, 1]`) |
| `strength(embedder) / set_strength` | Per-slot accessors |
| `dominant_spaces()` | Embedders with `strength > 0.5` AND `topic_weight > 0.0` (excludes temporal) |
| `weighted_agreement()` | `Σ strength × topic_weight`; NaN/Inf → 0.0; clamped to `MAX_WEIGHTED_AGREEMENT` |
| `is_topic()` | `weighted_agreement() >= TOPIC_THRESHOLD` |
| `similarity(other)` | Cosine of two 14-slot strength arrays; NaN-safe; clamped `[0, 1]` |
| `active_space_count()` | Count slots with `strength > 0.1` |

---

## 5. Topic lifecycle

`TopicPhase` (`clustering/topic.rs:47`):

| Phase | Trigger |
| --- | --- |
| `Emerging` | `age_hours < 1.0` AND `membership_churn > 0.3` |
| `Stable` | `age_hours >= 24.0` AND `membership_churn < 0.1` |
| `Declining` | `membership_churn >= 0.5` |
| `Merging` | Set externally when absorbed into another topic |

`TopicStability` fields: `phase`, `age_hours`, `membership_churn ∈ [0,1]`, `centroid_drift ∈ [0,1]`, `access_count`, `last_accessed`. `update_phase()` recomputes `phase` from the current metrics.

Memberships are pruned on memory eviction (`MultiSpaceClusterManager::insert` evicts oldest at `MAX_EMBEDDINGS_PER_SPACE`); the manager's `topic_count()` and `MAX_TOPICS` cap prevents unbounded growth (silent skip past the cap, `tracing::warn!` emitted with the cap value).

---

## 6. Topic stability tracking (portfolio level)

`TopicStabilityTracker` (`clustering/stability.rs`) is **distinct** from per-topic `TopicStability`. It tracks the entire topic portfolio for dream consolidation triggers per AP-70.

| Operation | Effect |
| --- | --- |
| `take_snapshot(topics)` | Push `TopicSnapshot { timestamp, topic_ids, total_members }` to `snapshots: VecDeque`. Caps at `MAX_SNAPSHOTS (1440)` and prunes by `SNAPSHOT_RETENTION_HOURS (24)`. |
| `compute_churn()` | `\|symmetric_difference\| / \|union\|` between current and previous snapshot. Stores in `current_churn`; appends `(timestamp, churn)` to `churn_history`. |
| `check_dream_trigger(entropy)` | Returns `true` when EITHER `entropy > 0.7 AND churn > 0.5` OR `entropy > 0.7` for ≥ 5 minutes (`high_entropy_start` tracked per call). |
| `average_churn(hours)` | Mean churn over the last `hours` hours from `churn_history`. |
| `reset_entropy_tracking()` | Clears `high_entropy_start`. |

The thresholds (`churn_threshold`, `entropy_threshold`, `entropy_duration_secs`) are configurable per tracker but default to the constitution AP-70 values.

---

## 7. Storage integration

| Concern | Column family | Key | Value |
| --- | --- | --- | --- |
| Per-memory topic profile | `CF_TOPIC_PROFILES` | UUID (16 bytes) | `[f32; 14]` strengths (56 bytes) — was 13 × f32 = 52B pre-E14 |
| Per-session topic portfolio | `CF_TOPIC_PORTFOLIO` | session_id string OR `__latest__` | bincode `PersistedTopicPortfolio` |
| Training records carry topic profile | `CF_TRAINING_RECORDS` | training key | record includes `topic_profile: [f32; 14]` |
| Constellation centroids include topic profile aggregation | `CF_CONSTELLATIONS` | constellation_id | bincode `Constellation` (centroid + topic profile aggregate) |

`PersistedTopicPortfolio` includes a version byte and roundtrips via bincode (no `skip_serializing_if` per PRD v6 §2.2).

---

## 8. MCP tools

`crates/context-graph-mcp/src/handlers/tools/topic_tools.rs` — four handlers backed by `MultiSpaceClusterManager` + `TopicStabilityTracker`. Constitutional compliance:

- AP-60: temporal embedders zero-weighted in `weighted_agreement`.
- AP-62: divergence alerts use only `DIVERGENCE_SPACES` (semantic-only, 7 embedders).
- ARCH-09: topic threshold `weighted_agreement >= 2.5`.

| Tool | Behavior | Returns |
| --- | --- | --- |
| `detect_topics` | Triggers HDBSCAN reclustering on demand (or via `force_recluster=true`). Requires ≥ `MIN_MEMORIES_FOR_CLUSTERING (3)` memories. Updates topic phases and pushes a stability snapshot. | `DetectTopicsResponse { topics, churn, total_memories, reclustered: bool }` |
| `get_topic_portfolio` | Reads current topic table; converts each `Topic` to `TopicSummary { id, name, confidence, weighted_agreement, member_count, contributing_spaces, phase, parent_topic_id }`. Optionally filters by phase. | `TopicPortfolioResponse { topics, total_topics, phase_breakdown }` |
| `get_topic_stability` | Pulls metrics from `TopicStabilityTracker`: current churn, average churn over a window, snapshot count, entropy tracking state, dream-trigger predicate. | `TopicStabilityResponse { stability: StabilityMetricsSummary, recent_snapshots, dream_trigger_active }` |
| `get_divergence_alerts` | For each recent memory (last `N` per `RecentMemory`), compares against current activity in `DIVERGENCE_SPACES`. If any space drops below the configured low threshold, emits a `DivergenceAlert { memory_id, space, similarity_score, memory_summary, detected_at, severity }`. | `DivergenceAlertsResponse { alerts, alert_count }` |
| `get_embedder_clusters` | Per-embedder cluster inspection via `MultiSpaceClusterManager::get_clusters(embedder)`. Returns cluster IDs, centroids, member counts. | `{ embedder, clusters: [{ cluster_id, centroid, member_count, members }] }` |

`embedder_to_dto_space` (`topic_tools.rs:49`) maps `Embedder` → user-facing string. E5 is mapped explicitly with a comment that it should be filtered out before reaching the divergence path (AP-77).

`truncate_summary` truncates content to `MAX_SUMMARY_WORDS (50)` words for divergence alerts; `MAX_SUMMARY_LEN (100)` chars is the underlying `DivergenceAlert::memory_summary` cap.

---

## 9. Divergence detection

`DIVERGENCE_SPACES` (semantic-only set, AP-62):

| Slot | Embedder | Semantic |
| --- | --- | --- |
| 0 | `Embedder::Semantic` (E1) | yes |
| 5 | `Embedder::Sparse` (E6) | yes |
| 6 | `Embedder::Code` (E7) | yes |
| 9 | `Embedder::Contextual` (E10) | yes |
| 11 | `Embedder::LateInteraction` (E12) | yes |
| 12 | `Embedder::KeywordSplade` (E13) | yes |
| 13 | `Embedder::BgeM3Dense` (E14) | yes |

Excluded:
- E5 (causal) — needs `CausalDirection` per AP-77; would emit false positives with `compute_embedder_scores_sync` returning `-1.0` sentinel.
- E2/E3/E4 (temporal) — AP-63: temporal proximity is not topical.
- E8/E11 (relational) — different signal class.
- E9 (structural) — different signal class.

`DivergenceSeverity::from_score`:

| Range | Severity |
| --- | --- |
| `score < 0.10` | `High` (major topic divergence) |
| `0.10 ≤ score < 0.20` | `Medium` (significant topic change) |
| `0.20 ≤ score < 0.30` | `Low` (minor drift) |
| `score >= 0.30` | not an alert |

`DivergenceDetector::detect(query, recent_memories)` iterates `RecentMemory` items, computes per-space similarity via `compute_embedder_scores_sync`, and emits a `DivergenceAlert` per offending space when below the configured low threshold (from `retrieval::config::low_thresholds`).

---

## 10. Memberships and eviction

`ClusterMembership` (`clustering/membership.rs`) — `{ embedder, cluster_id: i32, cluster_size, distance_to_centroid }`. `MultiSpaceClusterManager::get_memberships(memory_id)` returns `[Option<ClusterMembership>; 14]`.

When `MultiSpaceClusterManager::insert(id, embeddings)` evicts the oldest entry past `MAX_EMBEDDINGS_PER_SPACE`, the corresponding `ClusterMembership` is dropped from the per-space state and the topic table is recomputed lazily on the next reclustering trigger. `clear_topics()` and `clear_all_spaces()` provide explicit reset.

---

## 11. Source notes

- **`MAX_WEIGHTED_AGREEMENT = 9.5` (post-E14).** Earlier docs reference `8.5`; the constant was bumped when E14 BGE-M3 Dense joined the SEMANTIC tier.
- **`balanced` profile sums to 0.9995, not 1.0**, but topic detection uses `topic_weight()` from `EmbedderCategory`, not the search weight profile — they're orthogonal.
- **Topic threshold `2.5` is `>= ` not `>`.** `is_topic()` uses `>=` (`topic.rs:221`).
- **Temporal embedders are doubly excluded.** Both `topic_weight()=0.0` (no contribution to `weighted_agreement`) AND `dominant_spaces()` filter them out even if `strength > 0.5`. AP-60.
- **HDBSCAN per-embedder defaults differ for sparse spaces.** E6 and E13 use `min_cluster_size=5`; all others use `3`. See `HDBSCANParams::for_embedder`.
- **NaN-safe weighted agreement.** `weighted_agreement()` returns 0.0 on NaN/Inf per AP-10; `similarity(other)` clamps to `[0, 1]` on NaN.
- **Snapshot store is bounded twice.** `MAX_SNAPSHOTS=1440` is a hard cap; `SNAPSHOT_RETENTION_HOURS=24` prunes older snapshots even if under the count cap.
- **`MAX_TOPICS=10000` is a silent skip cap.** When exceeded, `tracing::warn!(max=MAX_TOPICS, ...)` fires and new topics are not created. There is no failure — recluster still runs and existing topics still update.
- **`MIN_MEMORIES_FOR_CLUSTERING=3` matches HDBSCAN `min_cluster_size`.** Both come from `clustering.parameters.min_cluster_size: 3`.
- **`E11_ENTITY_ENABLED` does not affect clustering.** Topic synthesis uses `category_for(embedder).topic_weight()`, which is independent of search-time E11 disable. E11's strength still contributes to `weighted_agreement` even when retrieval skips it.
- **Divergence excludes E5.** Despite E5 being SEMANTIC, it requires `CausalDirection` to score; `compute_embedder_scores_sync` returns `-1.0` sentinel for `Unknown`, which would be a phantom divergence. AP-77.
