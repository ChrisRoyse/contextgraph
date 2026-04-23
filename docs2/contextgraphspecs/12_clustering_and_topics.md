# Clustering and Topics

Source files covered:

| File | Role |
|------|------|
| `crates/context-graph-core/src/clustering/mod.rs` | Module re-exports |
| `crates/context-graph-core/src/clustering/cluster.rs` | `Cluster` struct (centroid + silhouette + touch time) |
| `crates/context-graph-core/src/clustering/membership.rs` | `ClusterMembership { memory_id, embedder, cluster_id, probability, is_core }` |
| `crates/context-graph-core/src/clustering/birch.rs` | `BIRCHParams`, `BIRCHTree`, `ClusteringFeature`, `BIRCHEntry`, `BIRCHNode` — incremental CF tree |
| `crates/context-graph-core/src/clustering/hdbscan.rs` | `HDBSCANParams`, `HDBSCANClusterer`, `ClusterSelectionMethod` — batch CPU clusterer |
| `crates/context-graph-core/src/clustering/topic.rs` | `Topic`, `TopicProfile`, `TopicPhase`, `TopicStability`, `build_topic_hierarchy`, `MAX_TOPIC_DEPTH`, `TOPIC_SILHOUETTE_THRESHOLD` |
| `crates/context-graph-core/src/clustering/stability.rs` | `TopicSnapshot`, `TopicStabilityTracker` — portfolio-level churn + dream triggers |
| `crates/context-graph-core/src/clustering/synthesizer.rs` | `TopicSynthesizer` — standalone weighted-agreement synthesizer |
| `crates/context-graph-core/src/clustering/fingerprint_matrix.rs` | FDMC primitives: `build_fingerprint_matrix`, `FingerprintMatrix`, `AggregationStrategy`, `FingerprintMatrixConfig`, `SimilarityStats` |
| `crates/context-graph-core/src/clustering/manager.rs` | `MultiSpaceClusterManager` — orchestrates BIRCH+HDBSCAN across all 13 spaces |
| `crates/context-graph-core/src/clustering/persistence.rs` | `PersistedTopicPortfolio`, `TopicPortfolio`, JSON session continuity |
| `crates/context-graph-core/src/clustering/error.rs` | `ClusterError` |
| `crates/context-graph-core/src/embeddings/category.rs` | `EmbedderCategory`, `category_for()`, `topic_weight()`, `topic_threshold() = 2.5`, `max_weighted_agreement() = 8.5` |
| `crates/context-graph-cuda/src/hdbscan/{mod,clusterer,gpu_knn}.rs` | GPU HDBSCAN (FAISS k-NN, CPU MST) — see [11_cuda_acceleration.md](11_cuda_acceleration.md) |

Cross references: [05_embedding_subsystem.md](05_embedding_subsystem.md), [06_search_and_fusion.md](06_search_and_fusion.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [11_cuda_acceleration.md](11_cuda_acceleration.md).

---

## 1. Overview

The subsystem discovers **emergent topics** by clustering memories in each of the 13 embedding spaces and then synthesizing cross-space agreement. Two algorithms run in concert:

| Algorithm | Mode | Complexity | Purpose | Source |
|-----------|------|------------|---------|--------|
| **BIRCH** | Incremental | O(log n) per insert | Immediate cluster assignment on new memory | `clustering/birch.rs` |
| **HDBSCAN** | Batch | O(n²) cpu / O(n·k) knn on gpu | Periodic recluster for density-based quality | `clustering/hdbscan.rs`, `context-graph-cuda/src/hdbscan/*` |

Batch HDBSCAN is triggered when **any** space has accumulated ≥ `recluster_threshold` (default `100`) inserts since the previous recluster (`MultiSpaceClusterManager::needs_recluster`).

### 1.1 Pipeline

```
insert(memory_id, [13 embeddings])
  └─ for each Embedder e (i=0..12):
       validate dim == get_dimension(e), all values finite
       PerSpaceState[i].tree.insert(emb, memory_id)       -> BIRCH cluster_idx
       accumulate embedding + memory_id into PerSpaceState[i] (LRU-evicted, cap 50_000)
       bump updates_since_recluster
  └─ compute_topic_profile from cluster indices
  └─ return InsertResult { cluster_indices[13], topic_profile, needs_recluster }

recluster()                   // triggered at threshold or explicit call
  └─ for each Embedder e:
       HDBSCANParams::default_for_space(e) → min_cluster_size = 5 (Sparse/KeywordSplade) else 3
       HDBSCANClusterer::for_space(e).fit(embeddings, ids, e)
       refresh memberships + clusters (centroid via compute_centroid_refs)
  └─ synthesize_topics()      // see §6
  └─ take_stability_snapshot()
  └─ emit mega-topic warn if member_count > 50% of max_space_size
```

### 1.2 Two Synthesis Approaches

| Approach | Entry | Idea |
|----------|-------|------|
| **Independent per-space + weighted agreement** | `MultiSpaceClusterManager::recluster` → `synthesize_topics` | Cluster each space, then pair memories where weighted_agreement ≥ 2.5 |
| **Fingerprint Distance Matrix Clustering (FDMC)** | `MultiSpaceClusterManager::recluster_fingerprint_matrix` | Build one NxN similarity matrix via `TeleologicalComparator`, single HDBSCAN pass |

FDMC amplifies weak signals: small per-space gaps (0.03-0.08) combine into a 0.21-0.56 aggregate gap across 7 semantic spaces.

### 1.3 Constitution Rules

| Rule | Effect |
|------|--------|
| ARCH-01 | `TeleologicalArray` atomic — all 13 embeddings or reject insert |
| ARCH-02 | Apples-to-apples — only compare E_i to E_i, never cross-space |
| ARCH-04 / AP-60 | Temporal embedders (E2-E4) `topic_weight = 0.0`; never contribute to topics |
| ARCH-09 / AP-61 | Topic threshold `weighted_agreement >= 2.5` |
| AP-10 | No NaN/Infinity — fail fast in validation |
| AP-62 / AP-63 | Divergence detection uses SEMANTIC embedders only |
| AP-70 | Dream trigger: `(entropy > 0.7 AND churn > 0.5)` OR `entropy > 0.7 for ≥ 300s` |

---

## 2. Embedder Categories and Weights

`EmbedderCategory` (`embeddings/category.rs`):

| Category | Members | `topic_weight()` | Role |
|----------|---------|------------------|------|
| Semantic | E1, E5, E6, E7, E10, E12, E13 | **1.0** | Primary topic signal |
| Temporal | E2, E3, E4 | **0.0** | Excluded |
| Relational | E8, E11 | **0.5** | Secondary |
| Structural | E9 | **0.5** | Secondary |

Constants (`category.rs`):
- `max_weighted_agreement() = 7·1.0 + 2·0.5 + 1·0.5 = 8.5`
- `topic_threshold() = 2.5`
- `confidence = weighted_agreement / 8.5 ∈ [0,1]`

---

## 3. Cluster Types

### 3.1 `ClusterMembership` (`clustering/membership.rs`)

```rust
pub struct ClusterMembership {
    pub memory_id: Uuid,
    pub embedder:  Embedder,        // space this membership belongs to
    pub cluster_id: i32,            // -1 = noise
    pub probability: f32,            // 0.0..=1.0
    pub is_core: bool,
}
```

Constructors: `new(memory_id, embedder, cluster_id, probability, is_core)`, `noise(memory_id, embedder)`. Predicates: `is_noise()` (cluster_id < 0), `is_confident()` (prob ≥ 0.5).

### 3.2 `Cluster` (`clustering/cluster.rs`)

```rust
pub struct Cluster {
    pub id: i32,
    pub space: Embedder,
    pub centroid: Vec<f32>,
    pub member_count: u32,
    pub silhouette: f32,
    pub last_accessed: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
```

Key methods: `update_silhouette(score)`, `touch()`, `is_high_quality()` (silhouette ≥ 0.3 per constitution), `update_centroid(centroid, count)`.

---

## 4. BIRCH (Incremental) — `clustering/birch.rs`

### 4.1 Params

| Field | Default | Notes |
|-------|---------|-------|
| `branching_factor` | 50 | Max children per internal node |
| `threshold` | 0.3 | Adaptive; 0.4 for Sparse/KeywordSplade, 0.25 for Code |
| `max_node_entries` | 50 | Leaf capacity before split |

`BIRCHParams::default_for_space(e)` returns space-tuned thresholds.

### 4.2 Clustering Feature (CF)

`ClusteringFeature { n, LS, SS }` — `(count, linear_sum_vector, squared_sum_scalar)`. CFs are **additive** (`CF(A ∪ B) = CF(A) + CF(B)`). Incremental centroid: `LS/n`; radius: `√(SS/n − (LS/n)²)`.

`BIRCHTree::insert(embedding, memory_id)` walks the tree (`O(log n)`), merges into the nearest CF if radius ≤ threshold, splits nodes that exceed `max_node_entries`. Memory-id tracking enables per-memory cluster lookup.

---

## 5. HDBSCAN (Batch) — `clustering/hdbscan.rs`

### 5.1 Params

| Field | Default | Notes |
|-------|---------|-------|
| `min_cluster_size` | 3 (5 for Sparse/KeywordSplade) | `HDBSCANParams::default_for_space` |
| `min_samples` | 2 (3 for sparse) | Must be ≤ `min_cluster_size` |
| `cluster_selection_method` | `EOM` | `EOM` (Excess of Mass, persistence) or `Leaf` (granular) |
| `metric` | `get_distance_metric(e)` | Cosine for dense, Jaccard for sparse |

Validation: `min_cluster_size ≥ 2`, `min_samples ≥ 1`, `min_samples ≤ min_cluster_size`.

### 5.2 Algorithm Steps (CPU: `HDBSCANClusterer::fit`)

1. **Core distances** — for each point compute distance to k-th (`min_samples`) nearest neighbor.
2. **Mutual reachability** — `MR(a,b) = max(core_dist(a), core_dist(b), dist(a,b))`.
3. **Minimum Spanning Tree** — Prim's over the MR matrix; return edges sorted by weight.
4. **Extract clusters (`extract_clusters`)** — Union-Find walks MST edges in ascending weight, stopping at a **data-driven gap threshold** (`detect_gap_threshold`: largest absolute gap in sorted MST weights; falls back to 75th percentile if no significant gap; metric-specific floor prevents over-splitting). Components with size ≥ `min_cluster_size` become clusters; smaller ones become noise (`cluster_id = -1`).
5. **Probability** — `probability = 1.0 - min(1/cluster_size, 0.5)`; noise = 0.0.
6. **Core points** — identified from final labels (heuristic: non-noise cluster members).
7. **Provenance warnings** — if a single component covers >50% of points, warns with gap_threshold + component count for diagnostic trace.

### 5.3 GPU HDBSCAN (`context-graph-cuda/src/hdbscan/`)

Split execution (`GpuHdbscanClusterer`):

1. `GpuKnnIndex` (FAISS Flat-GPU) — batch k-NN for core distances; O(n) GPU ops vs O(n²·k) CPU.
2. Mutual reachability computed on CPU using precomputed core distances.
3. MST + cluster extraction on CPU (already O(n log n), cheap).

Target latency: `topic_detection < 20ms` per ARCH-GPU-05. See [11_cuda_acceleration.md](11_cuda_acceleration.md).

---

## 6. Cross-Space Topic Synthesis

### 6.1 Weighted Agreement (Pairwise)

For memories `a` and `b`:

```
weighted_agreement(a, b) =
    Σ_{e ∈ discriminating_spaces} topic_weight(e) · 𝟙[cluster_e(a) == cluster_e(b) ∧ cluster_e(a) ≠ -1]
```

**`discriminating_spaces`** = spaces with ≥ 2 non-noise clusters AND `topic_weight > 0`. This excludes E2-E4 and any space that put everything in a single cluster (trivial "agreement").

Pair is a **topic edge** iff `weighted_agreement(a, b) >= 2.5`.

### 6.2 Synthesis Algorithm (`MultiSpaceClusterManager::synthesize_topics`)

1. Build `mem_clusters: memory_id → (embedder → cluster_id)` from all 13 `memberships` maps.
2. Compute `space_cluster_counts` once.
3. Derive `discriminating_spaces`.
4. Iterate all O(n²) pairs; emit edges meeting the 2.5 threshold.
5. **Union-Find** with path compression — one component per topic candidate.
6. **Mega-component splitter**: any component > 40% of total memories (and ≥ 5 members) is re-clustered with HDBSCAN in the most-discriminating space; sub-components replace the parent. Up to `MAX_SPLIT_ROUNDS = 5` recursive passes. Noise from sub-clustering becomes singleton components (dropped).
7. For each surviving component with ≥ 2 members:
   - `compute_topic_profile_from_clusters` — per-space strength = fraction of members in dominant cluster (TOPIC-2: strength forced to 0 if only one cluster exists and members > 2 — degenerate embedder filter).
   - `compute_dominant_cluster_ids` — most common cluster id per embedder.
   - Build `Topic::new(profile, cluster_ids, members)` if `profile.is_topic()` (weighted_agreement ≥ 2.5).
8. Cap: `MAX_TOPICS = 10_000`; additional topics skipped with warning.
9. `take_stability_snapshot()` at the end.

### 6.3 FDMC Synthesis (`recluster_fingerprint_matrix`)

Single-shot alternative:

1. `build_fingerprint_matrix(&[(Uuid, &SemanticFingerprint)], &FingerprintMatrixConfig)` — NxN aggregate similarity via `TeleologicalComparator::compare`.
2. `FingerprintMatrix::to_distance_matrix()` → precomputed distance matrix.
3. `HDBSCANClusterer::with_defaults().fit_precomputed(&distances, &memory_ids)` — single HDBSCAN pass.
4. `compute_silhouette_precomputed` — quality metric.
5. `matrix.analyze_embedder_contributions()` + `matrix.dominant_embedder()` — which embedder drove the separation.
6. Build topics from valid clusters (`profile.is_topic()`).

Returns `FdmcResult { total_clusters, topics_discovered, memberships, silhouette_score, similarity_stats, embedder_contributions, dominant_embedder }`.

### 6.4 `AggregationStrategy` (FDMC)

| Strategy | Default | Effect |
|----------|---------|--------|
| `WeightedSum` | ✓ | Σ topic_weight·sim (temporal=0) — recommended |
| `MaxPooling` | | Strongest agreement in any space |
| `MeanPooling` | | Unweighted average of non-zero sims |
| `ProductRule` | | Geometric mean (harsh) |
| `MinPooling` | | Requires all spaces to agree |

---

## 7. Topic Data Structures

### 7.1 `TopicProfile` (`clustering/topic.rs`)

```rust
pub struct TopicProfile { pub strengths: [f32; 13] }
```

Values clamped to `[0,1]` on construction. Key methods:

| Method | Returns |
|--------|---------|
| `weighted_agreement()` | `Σ strength_i · topic_weight(i)`, clamped to `[0, 8.5]`, NaN-safe → 0 |
| `is_topic()` | `weighted_agreement() >= 2.5` |
| `dominant_spaces()` | `Vec<Embedder>` where `strength > 0.5 AND topic_weight > 0` — excludes temporal |
| `strength(e)`, `set_strength(e, v)` | clamped accessor |
| `similarity(other)` | cosine similarity (zero-safe, NaN-safe) |
| `active_space_count()` | count of strengths > 0.1 |

### 7.2 `Topic`

```rust
pub struct Topic {
    pub id: Uuid,                              // deterministic from sorted members
    pub name: Option<String>,                  // e.g. "Semantic+Code+Entity (5 memories)"
    pub profile: TopicProfile,
    pub contributing_spaces: Vec<Embedder>,    // profile.dominant_spaces()
    pub cluster_ids: HashMap<Embedder, i32>,
    pub member_memories: Vec<Uuid>,
    pub confidence: f32,                       // weighted_agreement / 8.5
    pub silhouette_score: f32,                 // 1.0 default; updated from cluster silhouettes
    pub stability: TopicStability,
    pub created_at: DateTime<Utc>,
    pub parent_id: Option<Uuid>,               // hierarchy, MAX_TOPIC_DEPTH = 3
    pub depth: u8,
}
```

Deterministic topic id: same member set → same `Topic.id` — required for accurate churn tracking across reclusters.

Validity: `is_valid()` requires `weighted_agreement >= 2.5 AND silhouette >= TOPIC_SILHOUETTE_THRESHOLD (0.3)`.

Hierarchy: `build_topic_hierarchy(topics)` computes parent relations up to `MAX_TOPIC_DEPTH = 3`.

### 7.3 `TopicPhase` + Per-topic `TopicStability`

```rust
pub enum TopicPhase { Emerging, Stable, Declining, Merging }

pub struct TopicStability {
    pub phase: TopicPhase,
    pub age_hours: f32,
    pub membership_churn: f32,
    pub centroid_drift: f32,
    pub access_count: u32,
    pub last_accessed: Option<DateTime<Utc>>,
}
```

`update_phase()` rules:
- `Emerging` — `age < 1h AND churn > 0.3`
- `Stable` — `age ≥ 24h AND churn < 0.1`
- `Declining` — `churn ≥ 0.5`
- `Merging` — set externally

`is_stable()` ⇔ phase is Stable. `is_healthy()` ⇔ `churn < 0.3`.

---

## 8. Portfolio-Level Stability — `clustering/stability.rs`

`TopicStabilityTracker` is **distinct** from per-topic `TopicStability`: it tracks the entire portfolio over time and gates dream consolidation.

### 8.1 Fields and Thresholds

| Constant | Value | Role |
|----------|-------|------|
| `DEFAULT_CHURN_THRESHOLD` | 0.5 | Dream trigger churn threshold |
| `DEFAULT_ENTROPY_THRESHOLD` | 0.7 | Dream trigger entropy threshold |
| `DEFAULT_ENTROPY_DURATION_SECS` | 300 | Sustained-entropy window for trigger |
| `SNAPSHOT_RETENTION_HOURS` | 24 | Rolling retention window |
| `MAX_SNAPSHOTS` | 1440 | Hard cap (1/min × 24h) |

### 8.2 `TopicSnapshot`

```rust
pub struct TopicSnapshot {
    pub timestamp: DateTime<Utc>,
    pub topic_ids: Vec<Uuid>,          // only ids, not full profiles
    pub total_members: usize,
}
```

### 8.3 Churn Formula

```
old_ids = set(snapshot_old.topic_ids)
cur_ids = set(snapshot_current.topic_ids)
churn   = |symmetric_difference(old_ids, cur_ids)| / |union(old_ids, cur_ids)|    # Jaccard distance
```

`track_churn()` prefers comparing against a snapshot ~1 hour old; falls back to the oldest available snapshot for short sessions. Result clamped to `[0,1]`, NaN-safe. Pushed into `churn_history` with timestamp.

### 8.4 Dream Trigger (`check_dream_trigger(entropy)`)

Triggers (returns `true`) if **either**:

1. `entropy > 0.7 AND current_churn > 0.5`, **or**
2. `entropy > 0.7` has been continuously true for ≥ `entropy_duration_secs` (300s).

`high_entropy_start` is set the first moment entropy breaches 0.7 and reset the moment it drops. `reset_entropy_tracking()` after dream completes.

### 8.5 Portfolio Queries

| Method | Returns |
|--------|---------|
| `current_churn()` | Most recently computed churn |
| `average_churn(hours)` | Mean over the window (NaN-safe) |
| `is_stable()` | `average_churn(6) < 0.2` |
| `topic_count_change()` | `(added, removed)` since oldest snapshot |
| `snapshot_count()`, `latest_snapshot()` | Introspection |

---

## 9. Persistence — `clustering/persistence.rs`

### 9.1 `PersistedTopicPortfolio`

Session-continuity JSON blob containing full topics plus portfolio metrics.

```rust
pub struct PersistedTopicPortfolio {
    pub topics: Vec<Topic>,
    pub churn_rate: f32,          // [0,1]
    pub entropy: f32,              // [0,1]
    pub session_id: String,
    pub saved_at: u64,             // seconds since UNIX_EPOCH
}
```

| Method | Purpose |
|--------|---------|
| `new(topics, churn, entropy, session_id)` | Construct with `saved_at = now` |
| `to_bytes()` / `from_bytes(&[u8])` | JSON serde (readable in storage) |
| `valid_topics()` / `valid_topic_count()` | Filter by `Topic::is_valid()` |
| `is_unstable()` | Snapshot-level check |
| `age_seconds()` | Staleness |

Errors: `PersistenceError::{Serialization(serde_json::Error), InvalidData{message}}`.

### 9.2 In-Memory Portfolio

`TopicPortfolio` — thin wrapper around `HashMap<Uuid, Topic>` with thresholds (`new()`, `with_thresholds()`, `insert`, `get`, `get_mut`).

---

## 10. RocksDB Column Families

| CF | Constant | Key | Value | Writer |
|----|----------|-----|-------|--------|
| `topic_profiles` | `CF_TOPIC_PROFILES` | `memory_uuid` (16 bytes) | 13 × f32 little-endian (52 bytes) — the per-memory `TopicProfile` | Clustering pipeline after `compute_topic_profile` |
| `topic_portfolio` | `CF_TOPIC_PORTFOLIO` | session-scoped key | JSON-encoded `PersistedTopicPortfolio` | Session persistence (see `storage/teleological/rocksdb_store/persistence.rs:563`) |

Both CFs are registered in `TELEOLOGICAL_CFS` (`storage/teleological/column_families.rs:519`). Topic profiles are hard-deleted defensively in the `crud.rs` delete path even though the clustering pipeline owns writes.

Storage detail: `topic_profile_cf_options` and `topic_portfolio_cf_options` apply bounded bloom filters + block cache. Profile row size is fixed at 52 bytes — malformed rows fail fast during export.

See [09_storage_layer.md](09_storage_layer.md) for column-family option details.

---

## 11. `MultiSpaceClusterManager` API

File: `clustering/manager.rs`. Constants:

| Constant | Value |
|----------|-------|
| `DEFAULT_RECLUSTER_THRESHOLD` | 100 |
| `MAX_WEIGHTED_AGREEMENT` | 8.5 |
| `TOPIC_THRESHOLD` | 2.5 |
| `MAX_EMBEDDINGS_PER_SPACE` (private) | 50_000 (LRU-evict oldest half on overflow) |
| `MAX_TOPICS` (private) | 10_000 |

Key methods:

| Method | Purpose |
|--------|---------|
| `new(ManagerParams)` / `with_defaults()` | Construct; validates params |
| `insert(memory_id, &[Vec<f32>; 13]) -> InsertResult` | O(log n) per space; returns `{ cluster_indices[13], topic_profile, needs_recluster }` |
| `recluster() -> ReclusterResult` | Run HDBSCAN across all 13 spaces + synthesize; `{ total_clusters, per_space_clusters[13], topics_discovered }` |
| `recluster_fingerprint_matrix(&[(Uuid, SemanticFingerprint)]) -> FdmcResult` | FDMC one-shot reclustering |
| `get_memberships(memory_id) -> [Option<ClusterMembership>; 13]` | Per-space membership for one memory |
| `get_topics() -> &HashMap<Uuid, Topic>` | All current topics |
| `get_topic(&Uuid) -> Option<&Topic>` | By id |
| `get_clusters(Embedder) -> &HashMap<i32, Cluster>` | Per-space cluster index |
| `get_cluster_members(Embedder, i32) -> Vec<Uuid>` | Members of a specific cluster |
| `needs_recluster() -> bool` | Any space at threshold |
| `updates_status() -> [UpdateStatus; 13]` | Per-space update counts |
| `cluster_count(e)`, `total_clusters()`, `topic_count()`, `total_memories()` | Counters |

`ManagerParams` builder methods: `with_recluster_threshold`, `with_birch_params`, `with_hdbscan_params`.

---

## 12. MCP Tool Surface (Clustering-related)

Handlers live under `crates/context-graph-mcp/src/handlers/tools/`. Each clustering-adjacent tool is summarized below — see [08_mcp_tools_reference.md](08_mcp_tools_reference.md) for full JSON schemas.

| Tool | Handler | Input | Output |
|------|---------|-------|--------|
| `get_topic_portfolio` | `topic_tools::call_get_topic_portfolio` | `{ format: brief\|standard\|verbose }` | Topics with profiles (verbose adds all 13 embedder strengths) |
| `get_topic_stability` | `topic_tools::call_get_topic_stability` | `{ hours: 1..=168 }` | `{ churn_rate, entropy, dream_recommended, phases }` |
| `detect_topics` | `topic_tools::call_detect_topics` | `{ force: bool }` | `{ memories_processed, topics_found, topics }` — runs FDMC if ≥ `min_cluster_size` memories |
| `get_divergence_alerts` | `topic_tools::call_get_divergence_alerts` | `{ lookback_hours: 1..=48 }` | `{ is_divergent, divergence_score, alerts, embedder_divergences }` |
| `get_embedder_clusters` | `embedder_tools::call_get_embedder_clusters` | `{ embedder, minClusterSize?, includeSample? }` | Per-embedder cluster list; auto-loads fingerprints and reclusters on first call |
| `compare_embedder_views` | `embedder_tools::call_compare_embedder_views` | `{ query, embedders[], topK }` | Per-embedder top-K with divergence metrics |
| `search_cross_embedder_anomalies` | `embedder_tools::call_search_cross_embedder_anomalies` | `{ minScoreDelta, embedders[]? }` | Memories where embedders disagree strongly |
| `find_related_entities` | `entity_tools` | `{ entity, topK }` | E11-KEPLER related entities (see [05_embedding_subsystem.md](05_embedding_subsystem.md)) |

### 12.1 Divergence Computation (`call_get_divergence_alerts`)

1. Load **unbiased** fingerprints (`list_fingerprints_unbiased(200)`) — MCP-09 fix: no semantic query bias.
2. Filter to `created_at >= now - lookback_hours`.
3. Abort if `len < MIN_MEMORIES_FOR_DIVERGENCE`.
4. Sort by `created_at` descending; take most-recent memory as the "current" query.
5. Run `search_semantic(current_semantic, ...)` against the remainder.
6. Compute per-semantic-embedder divergence scores (E1, E5-E7, E10, E12, E13 only).
7. Emit `{ is_divergent, divergence_score, alerts, embedder_divergences }`.

Temporal embedders E2-E4 are never queried here, per AP-62/AP-63.

### 12.2 CLI Wrappers

Commands `topic portfolio`, `topic stability`, `topic detect`, `divergence check` wrap the MCP tools above — see [10_cli_reference.md](10_cli_reference.md) §§6-7 for flags and exit codes.

---

## 13. `SemanticFingerprint` Topic-Membership Field

The fingerprint carries per-memory clustering state:

```rust
pub struct SemanticFingerprint {
    // ...
    pub topic_profile: TopicProfile,          // 13xf32 strengths (persisted in CF_TOPIC_PROFILES)
    pub cluster_membership: [Option<ClusterMembership>; 13], // per-space assignments
    pub topic_id: Option<Uuid>,               // current dominant topic id
    // ...
}
```

On `insert`, the manager writes `cluster_indices[13]` and `topic_profile` back to the fingerprint before persistence. On recluster, memberships are updated and the profile is recomputed via `compute_topic_profile_from_clusters`.

---

## 14. Error Model — `clustering/error.rs`

```rust
pub enum ClusterError {
    InvalidParameter(String),           // config validation failure
    DimensionMismatch { expected, actual },
    InsufficientData { required, provided },
    Internal(String),
    // ...
}
```

Convenience constructors: `invalid_parameter`, `dimension_mismatch`, `insufficient_data`. All fingerprint insertion paths fail fast on non-finite values (AP-10).

---

## 15. Quick-Reference Cheat Sheet

```
Topic detection                    : weighted_agreement ≥ 2.5
Category weights                   : Semantic=1.0  Temporal=0.0  Relational=0.5  Structural=0.5
Max weighted agreement             : 8.5 (= 7·1.0 + 2·0.5 + 1·0.5)
Topic confidence                   : weighted_agreement / 8.5
Per-topic silhouette threshold     : 0.3
Default min_cluster_size            : 3 (5 for Sparse / KeywordSplade)
BIRCH defaults                      : branching=50 threshold=0.3 max_entries=50
Recluster trigger                   : 100 inserts in any space
Per-space embedding cap             : 50 000 (LRU half-evict)
Topic cap                           : 10 000
Dream trigger                       : (entropy > 0.7 AND churn > 0.5) OR entropy > 0.7 for 300s
Healthy churn                       : < 0.3 (per topic); portfolio `is_stable` : 6h avg < 0.2
Snapshot retention                  : 24h, hard cap 1440
Topic hierarchy max depth           : 3
Topic id derivation                 : deterministic from sorted member uuids
CFs                                 : topic_profiles (52 bytes per memory), topic_portfolio (JSON)
```
