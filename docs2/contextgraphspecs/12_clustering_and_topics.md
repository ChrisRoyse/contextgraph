# Clustering And Topics

**Source files covered:**
- `crates/context-graph-core/src/clustering/mod.rs` — public surface and re-exports
- `crates/context-graph-core/src/clustering/manager.rs` — `MultiSpaceClusterManager`, `ManagerParams`, `MAX_WEIGHTED_AGREEMENT (9.5)`, `TOPIC_THRESHOLD (2.5)`, `MAX_TOPICS (10_000)`, `MAX_EMBEDDINGS_PER_SPACE (50_000)`, `DEFAULT_RECLUSTER_THRESHOLD (100)`, `InsertResult`, `ReclusterResult`, `FdmcResult`, `UpdateStatus`
- `crates/context-graph-core/src/clustering/topic.rs` — `Topic`, `TopicProfile`, `TopicPhase`, `TopicStability`, `MAX_TOPIC_DEPTH (3)`, `TOPIC_SILHOUETTE_THRESHOLD (0.3)`, `build_topic_hierarchy`, `Topic::deterministic_id`
- `crates/context-graph-core/src/clustering/synthesizer.rs` — `TopicSynthesizer`, `DEFAULT_MERGE_THRESHOLD (0.9)`, `DEFAULT_MIN_SILHOUETTE (0.3)`
- `crates/context-graph-core/src/clustering/cluster.rs` — `Cluster` (per-embedder cluster + centroid)
- `crates/context-graph-core/src/clustering/membership.rs` — `ClusterMembership`
- `crates/context-graph-core/src/clustering/birch.rs` — `BIRCHTree`, `BIRCHParams`, `ClusteringFeature`, `BIRCHEntry`, `BIRCHNode`, `birch_defaults`
- `crates/context-graph-core/src/clustering/hdbscan.rs` — `HDBSCANClusterer`, `HDBSCANParams`, `ClusterSelectionMethod::{EOM, Leaf}`, `hdbscan_defaults`, `HDBSCANParams::default_for_space` (sparse override), `HDBSCANClusterer::for_space`, `fit_precomputed`, `compute_silhouette_precomputed`, `detect_gap_threshold`
- `crates/context-graph-core/src/clustering/fingerprint_matrix.rs` — `FingerprintMatrix`, `FingerprintMatrixConfig`, `AggregationStrategy::{WeightedSum, MaxPooling, MeanPooling, ProductRule, MinPooling}`, `SimilarityStats`, `build_fingerprint_matrix`, `update_matrix_incremental`
- `crates/context-graph-core/src/clustering/stability.rs` — `TopicStabilityTracker`, `TopicSnapshot`, `DEFAULT_CHURN_THRESHOLD (0.5)`, `DEFAULT_ENTROPY_THRESHOLD (0.7)`, `DEFAULT_ENTROPY_DURATION_SECS (300)`, `SNAPSHOT_RETENTION_HOURS (24)`, `MAX_SNAPSHOTS (1440)`
- `crates/context-graph-core/src/clustering/persistence.rs` — `PersistedTopicPortfolio`, `TopicPortfolio`, `PersistenceError`
- `crates/context-graph-core/src/retrieval/divergence.rs` — `DIVERGENCE_SPACES`, `DivergenceAlert`, `DivergenceSeverity`, `MAX_SUMMARY_LEN`
- `crates/context-graph-core/src/retrieval/detector.rs` — `DivergenceDetector`, `RecentMemory`, `is_divergence_space`
- `crates/context-graph-core/src/retrieval/multi_space.rs` — `MultiSpaceSimilarity::compute_relevance_score`, `compute_weighted_similarity`
- `crates/context-graph-cuda/src/hdbscan/` — GPU-accelerated clusterer (`GpuHdbscanClusterer`, `HdbscanParams`, `ClusterSelectionMethod`, `GpuKnnIndex` via FAISS for batch k-NN)
- `crates/context-graph-mcp/src/handlers/tools/topic_tools.rs` — `call_get_topic_portfolio`, `call_get_topic_stability`, `call_detect_topics`, `call_get_divergence_alerts`, `MIN_MEMORIES_FOR_CLUSTERING (3)`, `MIN_MEMORIES_FOR_DIVERGENCE (2)`, `MAX_SUMMARY_WORDS (50)`
- `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` — `call_get_embedder_clusters`, `MAX_CUSTOM_PROFILES (1_000)`

Cross-links: [05_embedding_subsystem.md](05_embedding_subsystem.md), [06_search_and_fusion.md](06_search_and_fusion.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [11_cuda_acceleration.md](11_cuda_acceleration.md), [13_teleological_subsystem.md](13_teleological_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Scope

The clustering subsystem maintains per-embedder clusters across all 14 embedding spaces, synthesizes cross-space "topics" via weighted agreement, tracks topic stability and lifecycle phases, and feeds dream-consolidation triggers. It exposes four MCP tools (`detect_topics`, `get_topic_portfolio`, `get_topic_stability`, `get_divergence_alerts`) plus per-embedder cluster inspection (`get_embedder_clusters`).

The pipeline is two-tier:

1. **Per-embedder clustering** (BIRCH for incremental, HDBSCAN for batch reclustering).
2. **Cross-space topic synthesis** (weighted-agreement formula across the 14-slot fingerprint).

A third path — **Fingerprint Distance Matrix Clustering (FDMC)** via `MultiSpaceClusterManager::recluster_fingerprint_matrix` — runs HDBSCAN once on a single aggregated similarity matrix instead of 14 independent per-space runs. Topics emerge from memories with similar overall fingerprints.

Per-memory `TopicProfile` (14 × f32 strength array) is stored in `CF_TOPIC_PROFILES`; per-session `TopicPortfolio` snapshots are persisted in `CF_TOPIC_PORTFOLIO`.

---

## 2. Constants

| Constant | Value | Source |
| --- | --- | --- |
| `TOPIC_THRESHOLD` | `2.5` | `clustering/manager.rs:80` |
| `MAX_WEIGHTED_AGREEMENT` | `9.5` | `clustering/manager.rs:77` (`8 SEMANTIC × 1.0 + 2 RELATIONAL × 0.5 + 1 STRUCTURAL × 0.5 = 9.5`; updated post-E14 from 8.5) |
| `MAX_TOPICS` | `10_000` | `clustering/manager.rs:93` (private — silently skips new topics once exceeded; emits `tracing::warn!` with cap) |
| `MAX_EMBEDDINGS_PER_SPACE` | `50_000` | `clustering/manager.rs:87` (oldest evicted; ~2.6 GB worst-case across 13 spaces × ~4 KB/vector) |
| `DEFAULT_RECLUSTER_THRESHOLD` | `100` | `clustering/manager.rs:71` (incremental updates before HDBSCAN runs per space) |
| `MAX_TOPIC_DEPTH` | `3` | `clustering/topic.rs:574` (Hierarchy depth cap for `build_topic_hierarchy`) |
| `TOPIC_SILHOUETTE_THRESHOLD` | `0.3` | `clustering/topic.rs:28` (constitution `clustering.parameters.silhouette_threshold`) |
| `DEFAULT_MERGE_THRESHOLD` | `0.9` | `clustering/synthesizer.rs:35` (cosine sim of `TopicProfile::strengths` for merging) |
| `DEFAULT_MIN_SILHOUETTE` | `0.3` | `clustering/synthesizer.rs:38` |
| HDBSCAN default `min_cluster_size` | `3` | `clustering/hdbscan.rs:106` (sparse override = 5) |
| HDBSCAN default `min_samples` | `2` | `clustering/hdbscan.rs:107` (must be `<= min_cluster_size`, validated by `validate()`) |
| HDBSCAN default `cluster_selection_method` | `ClusterSelectionMethod::EOM` | `clustering/hdbscan.rs:108` |
| HDBSCAN default `metric` | `DistanceMetric::Cosine` | `clustering/hdbscan.rs:109` |
| `DEFAULT_CHURN_THRESHOLD` | `0.5` | `clustering/stability.rs:25` |
| `DEFAULT_ENTROPY_THRESHOLD` | `0.7` | `clustering/stability.rs:28` |
| `DEFAULT_ENTROPY_DURATION_SECS` | `300` (5 min) | `clustering/stability.rs:31` |
| `SNAPSHOT_RETENTION_HOURS` | `24` | `clustering/stability.rs:34` |
| `MAX_SNAPSHOTS` | `1440` | `clustering/stability.rs:38` (1 snap/min × 24 h hard cap; pruned by both retention and count) |
| `MIN_MEMORIES_FOR_CLUSTERING` | `3` | `topic_tools.rs:37` (matches HDBSCAN `min_cluster_size`) |
| `MIN_MEMORIES_FOR_DIVERGENCE` | `2` | `topic_tools.rs:40` |
| `MAX_SUMMARY_WORDS` | `50` | `topic_tools.rs:43` (divergence-alert content truncation, words) |
| `DivergenceAlert::MAX_SUMMARY_LEN` | `100` | `divergence.rs:37` (chars; UTF-8 safe via `floor_char_boundary`) |
| `DIVERGENCE_SPACES` count | `7` | `retrieval/divergence.rs:26` (E1, E6, E7, E10, E12, E13, E14) |
| `MAX_CUSTOM_PROFILES` | `1_000` | `embedder_tools.rs:34` (per-DB cap on user-created weight profiles via `create_weight_profile`) |
| `detect_topics` `max_memories` range | `1..=50_000` | `topic_tools.rs:458` (Audit-11 SB-9 — rejects out-of-range instead of silent clamping) |

---

## 3. Category weights

Topic weights derive from the embedder category (`crate::embeddings::category::category_for`). Per AP-60, temporal embedders (E2/E3/E4) have `topic_weight = 0.0` and **never** count toward topic detection.

| Category | Weight | Embedders |
| --- | --- | --- |
| `Semantic` | `1.0` | E1, E5, E6, E7, E10, E12, E13, E14 (8 spaces) |
| `Temporal` | `0.0` | E2, E3, E4 (3 spaces — excluded per AP-60) |
| `Relational` | `0.5` | E8, E11 (2 spaces) |
| `Structural` | `0.5` | E9 (1 space) |
| `PipelineStage` | n/a | E12, E13 also classified as pipeline-stage in `EmbedderCategory`; see `insight_annotation.rs` |

Maximum weighted agreement = `8 × 1.0 + 2 × 0.5 + 1 × 0.5 = 9.5`. Topic confidence = `weighted_agreement / MAX_WEIGHTED_AGREEMENT`.

A topic emerges when `TopicProfile::weighted_agreement() >= TOPIC_THRESHOLD (2.5)` — the minimum is hit by 3 semantic spaces in agreement, 2 semantic + 1 relational, or 1 semantic + 2 relational + 1 structural.

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

### 4.2 HDBSCAN (batch reclustering, CPU)

`crates/context-graph-core/src/clustering/hdbscan.rs`. Parameters in `HDBSCANParams`:

| Field | Default | Source |
| --- | --- | --- |
| `min_cluster_size` | `3` | constitution `clustering.parameters.min_cluster_size` |
| `min_samples` | `2` | must be `<= min_cluster_size`, validated by `validate()` (rejects `min_cluster_size < 2`) |
| `cluster_selection_method` | `EOM` | `Excess of Mass` (general purpose) vs `Leaf` (granular) |
| `metric` | `Cosine` | per-space override via `get_distance_metric(embedder)` |

Per-embedder defaults via `HDBSCANParams::default_for_space(embedder)` — sparse spaces (E6 `Sparse`, E13 `KeywordSplade`) bump `min_cluster_size` to 5 (and `min_samples` to 3) because high-dimensional sparse data needs larger clusters; all other spaces use the constitution default (`3`, `2`).

Algorithm steps (`HDBSCANClusterer::fit`):

1. **Core distances**: For each point, find distance to its k-th nearest neighbor (`min_samples` parameter).
2. **Mutual reachability graph**: For every pair, compute `MR(a,b) = max(core(a), core(b), dist(a,b))`.
3. **MST (Prim's)**: Build minimum spanning tree from MR distances.
4. **Cluster extraction**: Sort MST edges by weight, detect the largest absolute "gap" using `detect_gap_threshold` (data-driven, no hardcoded thresholds) with metric-specific minimum-significance floors (Cosine ≥ 0.02, Jaccard ≥ 0.05, Euclidean ≥ 0.1). Apply Union-Find to merge components below the gap threshold; assign cluster labels respecting `min_cluster_size`. Probabilities scale with cluster size: `1.0 - (1.0 / cluster_size).min(0.5)`.
5. **Mega-cluster detection**: If `n > 10` and a single component contains `> 50%` of points, emits `tracing::warn!` with provenance (component size, gap threshold, num components, metric) — degenerate-clustering early warning.

`HDBSCANClusterer::fit_precomputed(distances, memory_ids)` accepts a precomputed pairwise distance matrix (used by FDMC). `compute_silhouette_precomputed(distances, labels)` returns the average silhouette score for quality scoring.

`MultiSpaceClusterManager::recluster()` (`manager.rs:456`) runs HDBSCAN per embedder when `updates_since_recluster >= recluster_threshold` (default 100). For each space:
- Skips if `state.embeddings.len() < space_params.min_cluster_size` (resets counter anyway).
- Calls `HDBSCANClusterer::for_space(embedder).fit(...)` to produce memberships.
- Builds `Cluster` objects per non-noise label with centroid + size.
- Re-synthesizes topics across all spaces (`synthesize_topics`).
- Logs mega-topic warnings when any topic absorbs > 50% of the largest space's memories.

`needs_recluster()` (`manager.rs:1314`) exposes the per-space predicate; `recluster_fingerprint_matrix` runs FDMC instead.

### 4.3 HDBSCAN (GPU)

`crates/context-graph-cuda/src/hdbscan/`:

- **Goal** (per ARCH-GPU-05 / AP-GPU-04): topic_detection < 20 ms.
- **Strategy**: Use FAISS GPU Flat index for batch k-NN (O(n) GPU operations vs O(n²k) CPU); mutual reachability + MST + cluster extraction stay on CPU (already O(n log n)).
- **Components**: `GpuHdbscanClusterer` (with `HdbscanParams`, `ClusterSelectionMethod`, `ClusterMembership`), `GpuKnnIndex` (FAISS wrapper), `GpuHdbscanError`.
- **No fallbacks**: fails fast with detailed errors if GPU unavailable.

The CPU `HDBSCANClusterer` and GPU `GpuHdbscanClusterer` are independent code paths with parallel APIs; the manager currently uses the CPU clusterer.

### 4.4 Fingerprint matrix (FDMC)

`crates/context-graph-core/src/clustering/fingerprint_matrix.rs`:

| Aggregation strategy | Formula | Notes |
| --- | --- | --- |
| `WeightedSum` (default) | `Σ category_weight × score` (normalized) | Matches topic weighted-agreement |
| `MaxPooling` | `max(scoreᵢ)` | Strongest agreement wins |
| `MeanPooling` | `Σ score / N` | Unweighted average over non-zero |
| `ProductRule` | `Π score` | Geometric mean (harsh but precise) |
| `MinPooling` | `min(scoreᵢ)` | Conservative — requires all spaces to agree |

`FingerprintMatrixConfig` fields: `aggregation` (default `WeightedSum`), `min_similarity_threshold` (default 0.0; pairs below set to 0.0; validated 0–1), `store_per_space` (default `true` — keeps per-embedder tensor for diagnostics).

`FingerprintMatrix` stores pairwise per-embedder similarity in a triangular layout. Helpers:
- `to_distance_matrix()` → `1.0 - similarity` for HDBSCAN's `fit_precomputed`.
- `analyze_embedder_contributions()` → per-embedder variance over upper-triangle entries (high variance = good discriminator).
- `dominant_embedder()` → embedder with highest contribution variance.
- `similarity_stats()` → `SimilarityStats { min, max, mean, std_dev, max_gap, gap_position }` for thresholding diagnostics.

`build_fingerprint_matrix(fp_refs, &config)` builds the matrix from `(Uuid, &SemanticFingerprint)` pairs; `update_matrix_incremental` updates an existing matrix.

`MultiSpaceClusterManager::recluster_fingerprint_matrix(&fingerprints)` → `FdmcResult` (manager.rs:593): builds matrix → converts to distance → runs `HDBSCANClusterer::with_defaults().fit_precomputed` → computes silhouette → builds topics from cluster assignments. Returns clustering quality metrics + `dominant_embedder` (best discriminator).

### 4.5 Topic synthesis

`TopicSynthesizer::synthesize_topics(memberships)` — `clustering/synthesizer.rs:368`:

1. Group memberships by memory_id → `mem_clusters: HashMap<Uuid, HashMap<Embedder, i32>>`.
2. **Find topic mates** (`find_topic_mates`): pairwise `weighted_agreement` between memories where both are in the same non-noise cluster per embedder; build edges where `weighted_agreement >= TOPIC_THRESHOLD (2.5)`; Union-Find with path compression to find connected components.
3. **Compute topic profiles** (`compute_topic_profile`): for each space, `strength = (count of members in dominant cluster) / total_members`. **TOPIC-2 degenerate-embedder fix**: if `counts.len() == 1 && members.len() > 2` (all members in same cluster — no discrimination), set `strength = 0.0`.
4. **Build `Topic`** for each group with ≥ 2 members where `is_valid()` holds (`weighted_agreement >= 2.5` AND `silhouette_score >= 0.3`).
5. **Merge similar topics** (`merge_similar_topics`): sort topics by member count descending (larger absorb smaller); merge any pair where `TopicProfile::similarity` (cosine of strength arrays) `>= merge_similarity_threshold (0.9)`. Larger absorbs smaller's members and cluster_ids.

Key methods on `TopicProfile`:

| Method | Purpose |
| --- | --- |
| `new(strengths)` | Constructor (clamps each entry to `[0, 1]`) |
| `strength(embedder) / set_strength` | Per-slot accessors |
| `dominant_spaces()` | Embedders with `strength > 0.5` AND `topic_weight > 0.0` (excludes temporal per AP-60) |
| `weighted_agreement()` | `Σ strength × topic_weight`; NaN/Inf → 0.0 (AP-10); clamped to `MAX_WEIGHTED_AGREEMENT` |
| `is_topic()` | `weighted_agreement() >= TOPIC_THRESHOLD` (note: `>=`, not `>`) |
| `similarity(other)` | Cosine of two 14-slot strength arrays; NaN-safe (returns 0.0); clamped `[0, 1]` |
| `active_space_count()` | Count slots with `strength > 0.1` |

`Topic::deterministic_id(members)` — sort UUIDs, hash via `Uuid::new_v5(NAMESPACE_OID, sorted_bytes)`. Same membership set always yields the same topic ID, regardless of insertion order — essential for accurate churn tracking (without this, every recluster produces "new" topics and churn always reads 1.0).

`Topic::generate_name(contributing_spaces, member_count)` produces a label like `"Semantic+Code+Entity (5 memories)"` (TOPIC-1).

---

## 5. Topic lifecycle

`TopicPhase` (`clustering/topic.rs:47`): enum `Emerging`, `Stable`, `Declining`, `Merging`. Default = `Emerging`.

`TopicStability::update_phase()` (`topic.rs:316`):

| Phase | Trigger |
| --- | --- |
| `Emerging` | `age_hours < 1.0` AND `membership_churn > 0.3` |
| `Stable` | `age_hours >= 24.0` AND `membership_churn < 0.1` |
| `Declining` | `membership_churn >= 0.5` |
| `Merging` | Set externally when absorbed into another topic |

`TopicStability` fields: `phase`, `age_hours`, `membership_churn ∈ [0,1]`, `centroid_drift ∈ [0,1]`, `access_count`, `last_accessed`. `is_stable()` and `is_healthy()` (`churn < 0.3`) are convenience predicates.

Memberships are pruned on memory eviction (`MultiSpaceClusterManager::insert` evicts oldest at `MAX_EMBEDDINGS_PER_SPACE`); the manager's `topic_count()` and `MAX_TOPICS` cap prevents unbounded growth (silent skip past the cap, `tracing::warn!` emitted with the cap value at three call sites — `manager.rs:682`, `1068`, `1445`).

`build_topic_hierarchy(topics, threshold)` (`topic.rs:612`): groups topics by `TopicProfile::strengths` cosine similarity using single-linkage Union-Find. Topics in groups of ≥ 2 get a synthetic parent topic with averaged profile, name `"Category: <first child name>"`, `depth = 0`; children get `depth = 1` and `parent_id = parent.id`. Maximum depth `MAX_TOPIC_DEPTH = 3`.

---

## 6. Topic stability tracking (portfolio level)

`TopicStabilityTracker` (`clustering/stability.rs`) is **distinct** from per-topic `TopicStability`. It tracks the entire topic portfolio for dream consolidation triggers per AP-70.

| Operation | Effect |
| --- | --- |
| `take_snapshot(topics)` | Push `TopicSnapshot { timestamp, topic_ids, total_members }` to `snapshots: VecDeque`. Caps at `MAX_SNAPSHOTS (1440)` and prunes by `SNAPSHOT_RETENTION_HOURS (24)` (both bounds applied). |
| `track_churn()` | `\|symmetric_difference\| / \|union\|` between current and a snapshot from ≥ 1 hour ago (or oldest if no hour-old snapshot exists, so churn is meaningful in short sessions). NaN/Inf → 0.0; clamped `[0, 1]`. Stores in `current_churn`; appends `(timestamp, churn)` to `churn_history`, also capped at `MAX_SNAPSHOTS`. |
| `compute_churn(old, current)` | Internal helper using Jaccard distance (`1 - Jaccard similarity`). |
| `check_dream_trigger(entropy)` | Returns `true` when EITHER `entropy > 0.7 AND churn > 0.5` (simultaneous, condition 1) OR `entropy > 0.7` for ≥ 5 minutes continuously (condition 2; `high_entropy_start` tracked across calls and reset when entropy drops). |
| `average_churn(hours)` | Mean churn over the last `hours` hours from `churn_history`. |
| `set_current_churn(churn)` | For testing/simulation; clamps to `[0, 1]`. |
| `reset_entropy_tracking()` | Clears `high_entropy_start`. |

The thresholds (`churn_threshold`, `entropy_threshold`, `entropy_duration_secs`) are configurable per tracker via `TopicStabilityTracker::with_thresholds(...)` but default to the constitution AP-70 values.

Snapshot store is bounded twice: `MAX_SNAPSHOTS = 1440` is a hard cap; `SNAPSHOT_RETENTION_HOURS = 24` prunes older snapshots even if under the count cap.

---

## 7. Storage integration

| Concern | Column family | Key | Value |
| --- | --- | --- | --- |
| Per-memory topic profile | `CF_TOPIC_PROFILES` | UUID (16 bytes) | `[f32; 14]` strengths (56 bytes) — was 13 × f32 = 52B pre-E14 |
| Per-session topic portfolio | `CF_TOPIC_PORTFOLIO` | session_id string OR `__latest__` | bincode `PersistedTopicPortfolio` |
| Training records carry topic profile | `CF_TRAINING_RECORDS` | training key | record includes `topic_profile: [f32; 14]` |
| Constellation centroids include topic profile aggregation | `CF_CONSTELLATIONS` | constellation_id | bincode `Constellation` (centroid + topic profile aggregate) |

`PersistedTopicPortfolio` (`persistence.rs:107`) wraps:
- `topics: Vec<Topic>` (full Topic structs with profile + stability)
- `churn_rate: f32`
- `entropy: f32`
- `session_id: String`
- `persisted_at_ms: u64`

Serialized via JSON (per docstring) for human readability. `PersistenceError::{Serialization, InvalidData}` for fail-fast on malformed input.

---

## 8. MCP tools

`crates/context-graph-mcp/src/handlers/tools/topic_tools.rs` — four handlers backed by `MultiSpaceClusterManager` + `TopicStabilityTracker`. Constitutional compliance:

- AP-60: temporal embedders zero-weighted in `weighted_agreement`.
- AP-62: divergence alerts use only `DIVERGENCE_SPACES` (semantic-only, 7 embedders).
- ARCH-09: topic threshold `weighted_agreement >= 2.5`.

| Tool | Behavior | Returns |
| --- | --- | --- |
| `detect_topics` | Validates `max_memories ∈ [1, 50_000]` (hard reject — Audit-11 SB-9). Requires ≥ `MIN_MEMORIES_FOR_CLUSTERING (3)` memories. **Loads ALL fingerprints from storage** via `scan_fingerprints_for_clustering(Some(max_memories))` (FIX-BUG-001 — previously only used in-session fingerprints). Calls `cluster_manager.clear_all_spaces()`, re-inserts each, then `recluster()`. Calls `track_churn()` after recluster (was previously missing). Diffs `topics_before` vs `topics_after` to identify new topics. Emits `AuditOperation::TopicDetected` per topic (non-fatal on failure). | `DetectTopicsResponse { new_topics, merged_topics, total_after, message }` |
| `get_topic_portfolio` | Reads current topic table; converts each `Topic` to `TopicSummary { id, name, confidence, weighted_agreement, member_count, contributing_spaces, phase, parent_topic_id, depth }`. Optionally filters by phase. | `TopicPortfolioResponse { topics, total_topics, phase_breakdown }` |
| `get_topic_stability` | Pulls metrics from `TopicStabilityTracker`: current churn, average churn over a window, snapshot count, entropy tracking state, dream-trigger predicate. Computes Shannon entropy from member counts (`compute_topic_entropy` — normalized to `[0,1]` via `log2(N)`). | `TopicStabilityResponse { stability: StabilityMetricsSummary, recent_snapshots, dream_trigger_active }` |
| `get_divergence_alerts` | Validates `MIN_MEMORIES_FOR_DIVERGENCE (2)`. **Uses `list_fingerprints_unbiased(200)`** (MCP-09 fix — previously biased by hardcoded "context memory" query). For each recent memory, compares against current activity in `DIVERGENCE_SPACES`. If any space drops below the configured low threshold (`retrieval::config::low_thresholds`), emits a `DivergenceAlert { memory_id, space, similarity_score, memory_summary, detected_at, severity }`. | `DivergenceAlertsResponse { alerts, alert_count }` |
| `get_embedder_clusters` | Per-embedder cluster inspection via `MultiSpaceClusterManager::get_clusters(embedder)`. Returns cluster IDs, centroids, member counts. | `{ embedder, clusters: [{ cluster_id, centroid, member_count, members }] }` |

`embedder_to_dto_space(embedder)` (`topic_tools.rs:49`) maps `Embedder` → user-facing string. E5 is mapped explicitly with a comment that it should be filtered out before reaching the divergence path (AP-77); a `tracing::warn!("AP-62 violation")` fires if E5 ever reaches the helper.

`truncate_summary(content, max_words)` truncates content to `MAX_SUMMARY_WORDS (50)` words for divergence alerts; the underlying `DivergenceAlert::memory_summary` field uses `MAX_SUMMARY_LEN (100)` chars (UTF-8 safe via `floor_char_boundary`).

**Concurrency note**: handlers acquire `cluster_manager.write()` (parking_lot `RwLock` — guard is `!Send`). The guard MUST be dropped before any `.await` calls; see `call_detect_topics` (`topic_tools.rs:537` — explicit scope-block pattern).

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

`DivergenceAlert::new(...)` asserts (in both debug AND release builds per CORE-M3 — `debug_assert!` was previously silently disabled in release) that the supplied space is in `DIVERGENCE_SPACES`. Score is clamped to `[0, 1]`.

`DivergenceDetector::detect(query, recent_memories)` iterates `RecentMemory` items, computes per-space similarity via `compute_embedder_scores_sync`, and emits a `DivergenceAlert` per offending space when below the configured low threshold (from `retrieval::config::low_thresholds`).

`DivergenceReport` (`divergence.rs:163`) collects alerts with helpers: `most_severe()`, `sort_by_severity()`, `count_by_severity()` returning `(high, medium, low)`, `format_all()`.

---

## 10. Memberships and eviction

`ClusterMembership` (`clustering/membership.rs`) — `{ embedder, cluster_id: i32, cluster_size, distance_to_centroid }`. `MultiSpaceClusterManager::get_memberships(memory_id)` returns `[Option<ClusterMembership>; 14]`.

When `MultiSpaceClusterManager::insert(id, embeddings)` evicts the oldest entry past `MAX_EMBEDDINGS_PER_SPACE`, the corresponding `ClusterMembership` is dropped from the per-space state and the topic table is recomputed lazily on the next reclustering trigger. `clear_topics()` (`manager.rs:1464`) and `clear_all_spaces()` provide explicit reset (the latter is what `detect_topics` uses to reload from storage).

---

## 11. Source notes

- **`MAX_WEIGHTED_AGREEMENT = 9.5` (post-E14).** Earlier docs reference `8.5`; the constant was bumped when E14 BGE-M3 Dense joined the SEMANTIC tier.
- **`balanced` profile sums to 0.9995, not 1.0**, but topic detection uses `topic_weight()` from `EmbedderCategory`, not the search weight profile — they're orthogonal.
- **Topic threshold `2.5` is `>=` not `>`.** `is_topic()` uses `>=` (`topic.rs:221`).
- **Temporal embedders are doubly excluded.** Both `topic_weight() = 0.0` (no contribution to `weighted_agreement`) AND `dominant_spaces()` filter them out even if `strength > 0.5`. AP-60.
- **HDBSCAN per-embedder defaults differ for sparse spaces.** E6 and E13 use `min_cluster_size = 5`, `min_samples = 3`; all others use `3, 2`. See `HDBSCANParams::default_for_space`.
- **NaN-safe weighted agreement.** `weighted_agreement()` returns 0.0 on NaN/Inf per AP-10; `similarity(other)` clamps to `[0, 1]` on NaN.
- **Snapshot store is bounded twice.** `MAX_SNAPSHOTS = 1440` is a hard cap; `SNAPSHOT_RETENTION_HOURS = 24` prunes older snapshots even if under the count cap. `churn_history` is capped at `MAX_SNAPSHOTS` separately.
- **`MAX_TOPICS = 10000` is a silent skip cap.** When exceeded, `tracing::warn!(max=MAX_TOPICS, ...)` fires and new topics are not created. There is no failure — recluster still runs and existing topics still update.
- **`MIN_MEMORIES_FOR_CLUSTERING = 3` matches HDBSCAN `min_cluster_size`.** Both come from `clustering.parameters.min_cluster_size: 3`.
- **`E11_ENTITY_ENABLED` does not affect clustering.** Topic synthesis uses `category_for(embedder).topic_weight()`, which is independent of search-time E11 disable. E11's strength still contributes to `weighted_agreement` even when retrieval skips it.
- **Divergence excludes E5.** Despite E5 being SEMANTIC, it requires `CausalDirection` to score; `compute_embedder_scores_sync` returns `-1.0` sentinel for `Unknown`, which would be a phantom divergence. AP-77.
- **Deterministic topic IDs.** `Topic::deterministic_id(members)` sorts UUIDs and hashes via `new_v5(NAMESPACE_OID, sorted)`; same membership produces the same ID across reclusters, which is the only way churn tracking is meaningful.
- **TOPIC-2 degenerate-embedder fix.** `compute_topic_profile` zeros the strength for any embedder whose memberships all land in a single cluster (`counts.len() == 1 && members.len() > 2`) — that embedder is not discriminating among the candidates, so the strength signal is noise.
- **TOPIC-1 auto-naming.** `Topic::generate_name(contributing_spaces, count)` produces labels like `"Semantic+Code+Entity (5 memories)"` so topic browsers don't show raw UUIDs.
- **Mega-cluster + mega-topic provenance logging.** `HDBSCANClusterer::extract_clusters` warns when one component absorbs > 50% of points; `MultiSpaceClusterManager::recluster` warns when one topic absorbs > 50% of any space's memories — both with full diagnostic context.
- **Custom weight profiles cap.** `MAX_CUSTOM_PROFILES = 1_000` per database (`embedder_tools.rs:34`). The cap rejects new names but allows in-place updates (existing profile name passes the `!profiles.contains_key(&request.name)` guard).
- **Two distinct AggregationStrategy enums.** `crates/context-graph-core/src/retrieval/aggregation.rs` defines `{RRF, WeightedAverage, MaxPooling}` (used by retrieval fusion); `crates/context-graph-core/src/clustering/fingerprint_matrix.rs` defines `{WeightedSum, MaxPooling, MeanPooling, ProductRule, MinPooling}` (used by FDMC). They share the `MaxPooling` name only by coincidence.
- **detect_topics reloads from storage every call.** Even with `force=false`, the handler calls `clear_all_spaces()` and re-inserts every fingerprint via `scan_fingerprints_for_clustering`. There is no incremental fast path at the MCP layer; per-space `BIRCH` incremental updates only happen on `MultiSpaceClusterManager::insert` calls inside store/inject paths.
- **CPU + GPU HDBSCAN are parallel implementations.** `crates/context-graph-core/src/clustering/hdbscan.rs` (CPU, used by manager) and `crates/context-graph-cuda/src/hdbscan/` (GPU, FAISS-backed for k-NN) expose similar APIs (`HDBSCANClusterer` vs `GpuHdbscanClusterer`, both with `ClusterSelectionMethod::{EOM, Leaf}` and silhouette computation). The manager currently routes to the CPU path.
