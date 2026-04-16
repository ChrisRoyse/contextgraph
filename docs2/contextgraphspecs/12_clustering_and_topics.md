# Clustering and Topics

Source files covered:

- `crates/context-graph-core/src/clustering/mod.rs`
- `crates/context-graph-core/src/clustering/birch.rs`
- `crates/context-graph-core/src/clustering/hdbscan.rs`
- `crates/context-graph-core/src/clustering/cluster.rs`
- `crates/context-graph-core/src/clustering/membership.rs`
- `crates/context-graph-core/src/clustering/topic.rs`
- `crates/context-graph-core/src/clustering/stability.rs`
- `crates/context-graph-core/src/clustering/synthesizer.rs`
- `crates/context-graph-core/src/clustering/fingerprint_matrix.rs`
- `crates/context-graph-core/src/clustering/persistence.rs`
- `crates/context-graph-core/src/clustering/manager.rs`
- `crates/context-graph-core/src/clustering/error.rs`
- `crates/context-graph-core/src/embeddings/category.rs`
- `crates/context-graph-cuda/src/hdbscan/mod.rs`
- `crates/context-graph-cuda/src/hdbscan/clusterer.rs`
- `crates/context-graph-cuda/src/hdbscan/gpu_knn.rs`

---

## 1. Clustering Architecture Overview

The clustering subsystem discovers emergent topics from memories stored across 13 embedding spaces. It operates at two levels: **per-space clustering** (grouping similar embeddings within a single space) and **cross-space topic synthesis** (detecting when memories cluster together across multiple spaces).

### 1.1 Dual-Algorithm Design

Two clustering algorithms serve complementary roles:

| Algorithm | Mode | Complexity | Purpose |
|-----------|------|------------|---------|
| **BIRCH** | Incremental | O(log n) per insert | Real-time clustering on each new memory insertion |
| **HDBSCAN** | Batch | O(n^2) | Periodic reclustering for higher-quality cluster assignments |

BIRCH provides immediate cluster assignments as memories arrive. After a configurable number of incremental updates (default: 100), HDBSCAN runs a batch reclustering pass to refine cluster boundaries with density-based analysis.

### 1.2 Multi-Space Clustering Pipeline

```
Memory inserted with 13 embeddings
       |
       v
Per-space BIRCH insertion (13 parallel)
       |
       v
Incremental topic profile computed
       |
       v (every 100 inserts)
Per-space HDBSCAN batch reclustering (13 parallel)
       |
       v
Cross-space topic synthesis (weighted agreement)
       |
       v
Topic portfolio updated + stability snapshot
```

### 1.3 Two Clustering Approaches

The system supports two distinct clustering approaches:

1. **Independent Per-Space Clustering** -- Clusters each of the 13 embedding spaces independently, then synthesizes topics by finding memories that share cluster assignments across spaces (via weighted agreement).

2. **Fingerprint Distance Matrix Clustering (FDMC)** -- Builds a single pairwise similarity matrix using the `TeleologicalComparator` across all 13 spaces, then runs one HDBSCAN pass on the aggregated distances. Signal amplification: gaps of 0.03-0.08 across 7 semantic spaces become 0.21-0.56 in aggregate.

### 1.3a Relationship to Constellations (Training Subsystem)

The `crates/context-graph-core/src/constellation/` module (commits `7179bd7c`, `48f0420d`) is **related but distinct** from topics and clusters documented in this spec:

| Aspect | Topics / Clusters (this spec) | Constellations (training subsystem) |
|--------|-------------------------------|-------------------------------------|
| Role | Emergent membership — who belongs to which cluster | Compiled summary — what the centroid, spread, and percentiles look like for a caller-chosen group |
| Discovery | Automatic via BIRCH + HDBSCAN across all 13 spaces | Explicit: caller supplies a `ConstellationSelector` (topic id, session id, tag, time range, or explicit UUID list) |
| Persistence | `topic_portfolio` CF (JSON per session) + `topic_profiles` CF (13xf32 per memory) | `constellations` CF (bincode `Constellation`) + `constellation_by_selector` CF (O(1) selector lookup) |
| Statistics | Cluster centroid + silhouette + stability (churn, entropy) | Per-embedder `EmbedderStats` with Welford online mean/variance (`constellation/welford.rs`) and reservoir-sampled percentiles (`constellation/reservoir.rs`, cap = 1024 samples) |
| Update model | Incremental BIRCH, periodic HDBSCAN recluster (every ~100 inserts), stability snapshots | Immutable once compiled; call `compile_constellation` again to refresh |
| Consumers | Topic portfolio reports, divergence alerts, dream trigger | Training-time "does this new memory belong?" scoring via `score_against_constellation` |

A constellation does **not** create a topic: it snapshots the statistical shape of an already-selected group so downstream classifiers can answer membership questions without re-running retrieval. See `13_teleological_subsystem.md#15b-constellation-compiler` for the full API.

**New algorithm primitives** introduced with constellations (not used by the clustering subsystem itself):

- `WelfordStats` / `WelfordVector` (`constellation/welford.rs`) — numerically stable online mean and variance (single-pass, O(1) per update, no catastrophic cancellation).
- `ReservoirSample` (`constellation/reservoir.rs`) — bounded-memory uniform random sample for p50/p95 percentile estimation on unbounded streams.

### 1.4 Constitution Rules

| Rule | Description |
|------|-------------|
| ARCH-01 | TeleologicalArray is atomic -- all 13 embeddings or nothing |
| ARCH-02 | Apples-to-apples comparison only -- E1 to E1, never cross-space |
| ARCH-04 | Temporal embedders (E2-E4) NEVER count toward topic detection |
| ARCH-09 | Topic threshold is weighted_agreement >= 2.5 |
| AP-10 | No NaN/Infinity in similarity scores (fail fast) |
| AP-60 | Temporal embedders (E2-E4) have topic weight 0.0 |

---

## 2. BIRCH Clustering Algorithm

BIRCH (Balanced Iterative Reducing and Clustering using Hierarchies) provides O(log n) incremental clustering for real-time memory insertion.

### 2.1 Clustering Feature (CF)

The CF is a triple `(n, LS, SS)` that summarizes a set of d-dimensional points:

| Component | Type | Definition |
|-----------|------|------------|
| `n` | `u32` | Number of data points |
| `LS` | `Vec<f32>` | Linear sum: d-dimensional vector = sum of Xi |
| `SS` | `f32` | Squared sum: scalar = sum of norm(Xi)^2 |

Key properties:

- **Additivity**: `CF(A union B) = CF(A) + CF(B)` -- enables merging without re-scanning members
- **Sufficient statistics**: Can compute centroid, radius, diameter from CF alone
- **Compact**: O(d) space regardless of n

Derived statistics:

| Metric | Formula |
|--------|---------|
| Centroid | `LS / n` |
| Radius | `sqrt(SS/n - norm(centroid)^2)` |
| Diameter | `2 * radius` (spherical approximation) |

### 2.2 BIRCH Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `branching_factor` | 50 | Maximum children per non-leaf node |
| `threshold` | 0.3 | Cluster radius threshold; points within this radius merge into same CF |
| `max_node_entries` | 50 | Maximum entries per leaf node; triggers split when exceeded |

Space-specific threshold overrides:

| Embedder | Threshold | Rationale |
|----------|-----------|-----------|
| Sparse (E6), KeywordSplade (E13) | 0.4 | Looser for high dimensionality |
| Code (E7) | 0.25 | Tighter for code specificity |
| All others | 0.3 | Constitution default |

Validation rules:

- `branching_factor` must be >= 2
- `threshold` must be > 0.0 and finite
- `max_node_entries` must be >= `branching_factor`

### 2.3 CF-Tree Structure

The BIRCH tree consists of three types:

**BIRCHEntry** -- An entry in a node containing:
- A `ClusteringFeature` summarizing points
- An optional child `BIRCHNode` (None for leaf entries)
- A `Vec<Uuid>` of memory IDs (populated for leaf entries only)

**BIRCHNode** -- A node that is either:
- A **leaf node**: entries contain CFs and memory IDs directly
- An **internal node**: entries contain CFs and child node pointers

**BIRCHTree** -- The root structure holding:
- `params: BIRCHParams`
- `root: BIRCHNode`
- `dimension: usize` (expected embedding dimension)
- `total_points: usize`

### 2.4 Insertion Algorithm

1. **Validate** the embedding dimension matches the tree's expected dimension. Reject NaN/Infinity values.
2. **Traverse** from root to find the closest leaf entry:
   - For leaf nodes: find the closest entry whose CF `would_fit` the point within threshold (i.e., the merged radius would remain within threshold). If no existing entry fits, create a new entry.
   - For internal nodes: find the closest child by centroid distance and descend recursively.
3. **Merge** the point into the matching entry: update CF by adding point to `n`, `LS`, and `SS`; append the memory UUID to the entry's `memory_ids`.
4. **Split** if root entries exceed `max_node_entries`:
   - Find the farthest pair of entries (by centroid distance) as split seeds
   - Create two new child nodes
   - Distribute entries to the closer seed node
   - Replace root with a new internal node containing two child entries

### 2.5 Node Splitting (Farthest-Pair Algorithm)

When a node exceeds `max_node_entries`:

1. Compute all pairwise centroid distances between entries
2. Select the two entries with maximum distance as seeds
3. Create two new nodes of the same type (leaf or internal)
4. Assign seed entries to their respective nodes
5. For each remaining entry, compute distance to the total CF of each new node and assign to the closer one
6. Update parent entries with new CFs and child pointers

### 2.6 Threshold Adaptation

The `adapt_threshold` method uses binary search to find a threshold that produces a target number of clusters. It adjusts `params.threshold` but does not rebuild the tree -- a full rebuild by reinserting all points is required for the change to take effect.

---

## 3. HDBSCAN Clustering Algorithm

HDBSCAN (Hierarchical Density-Based Spatial Clustering of Applications with Noise) performs batch density-based clustering for higher accuracy than incremental BIRCH.

### 3.1 HDBSCAN Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `min_cluster_size` | 3 | Minimum points to form a cluster |
| `min_samples` | 2 | Minimum samples for core point determination |
| `cluster_selection_method` | EOM | Excess of Mass (general) or Leaf (granular) |
| `metric` | Cosine | Distance metric; space-specific via `get_distance_metric(embedder)` |

Space-specific overrides:

| Embedder | min_cluster_size | min_samples | metric |
|----------|------------------|-------------|--------|
| Sparse (E6), KeywordSplade (E13) | 5 | 3 | Jaccard |
| All others | 3 | 2 | per-space config |

Validation rules:

- `min_cluster_size` must be >= 2
- `min_samples` must be >= 1
- `min_samples` must be <= `min_cluster_size`

### 3.2 Algorithm Steps

**Step 1: Compute Core Distances**

For each point, compute the distance to all other points, sort, and take the distance to the k-th nearest neighbor (where k = `min_samples`). Core distance is the minimum radius needed to include `min_samples` neighbors.

**Step 2: Build Mutual Reachability Graph**

For each pair (a, b):

```
MR(a, b) = max(core_dist(a), core_dist(b), dist(a, b))
```

This transforms the distance space so that sparse regions have inflated distances, naturally separating clusters.

**Step 3: Construct Minimum Spanning Tree**

Uses Prim's algorithm on the mutual reachability graph:

1. Start from node 0, mark it as in-tree
2. Initialize minimum distances from node 0 to all others
3. Repeatedly: select the minimum-distance node not in tree, add it, update distances
4. Sort resulting edges by weight (ascending)

**Step 4: Extract Clusters with Union-Find**

1. **Gap detection**: Find the largest absolute gap in sorted MST edge weights to identify natural cluster boundaries. Metric-specific minimum significant gap thresholds:

   | Metric | Min Significant Gap | Floor |
   |--------|--------------------:|------:|
   | Cosine / AsymmetricCosine | 0.02 | 0.03 |
   | Jaccard | 0.05 | 0.10 |
   | Euclidean | 0.10 | 0.05 |

   If the largest gap exceeds the minimum significant gap, use the edge weight just before the gap as the threshold. Otherwise, fall back to the 75th percentile edge weight.

2. **Union-Find**: Process MST edges in weight order up to the gap threshold. Merge components using Union-Find with rank-based union and path compression.

3. **Label assignment**: Components with size >= `min_cluster_size` get sequential cluster IDs (0, 1, 2, ...). Smaller components are labeled as noise (cluster_id = -1).

4. **Membership probability**: `1.0 - (1.0 / cluster_size).min(0.5)` -- larger clusters yield higher confidence.

**Step 5: Core Point Identification**

A point is core if it has >= `min_samples` neighbors in the same cluster (determined from labels only, not distances).

### 3.3 Precomputed Distance Matrix Support

`fit_precomputed` accepts a symmetric n x n distance matrix directly, bypassing core distance and mutual reachability computation. This is used by the FDMC approach where the fingerprint similarity matrix has already aggregated all 13 spaces into a single distance matrix.

### 3.4 Distance Metrics

| Metric | Computation |
|--------|-------------|
| Cosine | `1.0 - cosine_similarity(a, b)` |
| Euclidean | `sqrt(sum((a_i - b_i)^2))` |
| AsymmetricCosine | Same as Cosine (asymmetry applied at embedding time) |
| Jaccard | `1.0 - (intersection / union)` where presence is defined as value > 0.0 |

### 3.5 Silhouette Score Computation

The clusterer computes silhouette scores to measure cluster quality:

- For each non-noise point `i`:
  - `a(i)` = mean distance to points in the same cluster
  - `b(i)` = minimum mean distance to points in any other cluster
  - `s(i) = (b(i) - a(i)) / max(a(i), b(i))`
- Overall silhouette = mean of all `s(i)` values
- Range: -1.0 (poor) to 1.0 (excellent); requires at least 2 clusters

### 3.6 Mega-Cluster Detection

After clustering, if any single component contains more than 50% of all points (when n > 10), a warning is logged with provenance metadata: component size, total points, gap threshold, and number of components. This indicates degenerate clustering.

---

## 4. GPU-Accelerated HDBSCAN

The `crates/context-graph-cuda/src/hdbscan/` module provides a GPU-accelerated HDBSCAN implementation targeting sub-20ms topic detection.

### 4.1 Architecture

The GPU acceleration targets the core distance computation bottleneck (O(n^2 k) on CPU):

| Step | Execution | Complexity |
|------|-----------|------------|
| Core distances (k-NN) | **GPU** via custom CUDA kernel | O(n) GPU operations |
| Mutual reachability | CPU | O(n^2) but simple operations |
| MST (Prim's) | CPU | O(n^2) |
| Cluster extraction | CPU | O(n) |

### 4.2 GPU k-NN Index

`GpuKnnIndex` stores vectors in flattened row-major layout and computes core distances using a custom CUDA kernel via the Driver API (avoiding WSL2 cudart static initialization issues):

1. **add**: Validates dimension and finite values, then flattens and appends vectors
2. **compute_core_distances**: Launches a GPU k-NN kernel to find the k-th nearest neighbor distance for all vectors in a single batch operation. Returns L2 distances.

### 4.3 Parameters

`HdbscanParams` mirrors the core HDBSCAN parameters:

| Parameter | Default | Constraint |
|-----------|---------|------------|
| `min_cluster_size` | 3 | >= 2 |
| `min_samples` | 2 | >= 1, <= min_cluster_size |
| `cluster_selection_method` | EOM | EOM or Leaf |

No CPU fallback is provided. If GPU is unavailable, the operation fails fast with `GpuHdbscanError::GpuNotAvailable`.

---

## 5. Cluster Membership Management

### 5.1 ClusterMembership

Each memory has a `ClusterMembership` per embedding space, defined in `membership.rs`:

| Field | Type | Description |
|-------|------|-------------|
| `memory_id` | `Uuid` | The memory this membership belongs to |
| `space` | `Embedder` | The embedding space (one of 13) |
| `cluster_id` | `i32` | Cluster ID; -1 = noise (not in any cluster) |
| `membership_probability` | `f32` | Probability of belonging to this cluster [0.0, 1.0] |
| `is_core_point` | `bool` | Whether this point is central to cluster density |

### 5.2 Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `CONFIDENT_THRESHOLD` | 0.8 | Minimum probability for high-confidence assignment |
| `NOISE_CLUSTER_ID` | -1 | Sentinel value for noise/outlier points |

### 5.3 Noise Membership

`ClusterMembership::noise(memory_id, space)` creates a membership with cluster_id = -1, probability = 0.0, and is_core_point = false.

---

## 6. Cluster Type

### 6.1 Cluster

Defined in `cluster.rs`, the `Cluster` struct represents a discovered cluster in a single embedding space:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `i32` | Cluster identifier (unique per space) |
| `space` | `Embedder` | The embedding space this cluster belongs to |
| `centroid` | `Vec<f32>` | Mean embedding vector of all members |
| `member_count` | `u32` | Number of members |
| `silhouette_score` | `f32` | Quality metric [-1.0, 1.0]; default 0.0 until computed |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

### 6.2 High Quality Threshold

`HIGH_QUALITY_THRESHOLD = 0.3` -- a cluster is considered high quality when its silhouette score >= 0.3 (per constitution `clustering.parameters.silhouette_threshold`).

---

## 7. Topic Extraction and Management

### 7.1 Weighted Agreement Formula

Topics are discovered when memories cluster together across multiple embedding spaces. The core metric is **weighted agreement**:

```
weighted_agreement = Sum(topic_weight_i * strength_i)  for all 13 embedders
```

Where `strength_i` is the fraction of topic members in the dominant cluster for embedder `i`, and `topic_weight_i` comes from the embedder's category.

### 7.2 Embedder Category Weights

Defined in `crates/context-graph-core/src/embeddings/category.rs`:

| Category | Embedders | Weight | Count |
|----------|-----------|-------:|------:|
| **Semantic** | E1, E5, E6, E7, E10, E12, E13 | 1.0 | 7 |
| **Temporal** | E2, E3, E4 | 0.0 | 3 |
| **Relational** | E8, E11 | 0.5 | 2 |
| **Structural** | E9 | 0.5 | 1 |

Maximum possible weighted agreement:

```
7 * 1.0 + 2 * 0.5 + 1 * 0.5 = 8.5
```

### 7.3 Topic Detection Thresholds

| Constant | Value | Location | Description |
|----------|-------|----------|-------------|
| `TOPIC_THRESHOLD` | 2.5 | `manager.rs` | Minimum weighted agreement for topic formation |
| `MAX_WEIGHTED_AGREEMENT` | 8.5 | `manager.rs` | Maximum possible weighted agreement |
| `TOPIC_SILHOUETTE_THRESHOLD` | 0.3 | `topic.rs` | Minimum silhouette score for valid topics |
| `MAX_TOPICS` | 10,000 | `manager.rs` | Hard cap on tracked topics |
| `MAX_TOPIC_DEPTH` | 3 | `topic.rs` | Maximum depth in topic hierarchy |

Topic confidence is computed as:

```
confidence = weighted_agreement / 8.5
```

### 7.4 Examples of Threshold Behavior

| Scenario | Weighted Agreement | Meets Threshold? |
|----------|--------------------|:----------------:|
| 3 semantic spaces at strength 1.0 | 3.0 | Yes |
| 2 semantic + 1 relational at strength 1.0 | 2.5 | Yes (exactly) |
| 2 semantic spaces only | 2.0 | No |
| 5 temporal spaces at strength 1.0 | 0.0 | No (AP-60) |

### 7.5 Topic Struct

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Deterministic UUID computed from sorted member UUIDs (via `Uuid::new_v5`) |
| `name` | `Option<String>` | Auto-generated label, e.g. "Semantic+Code+Entity (5 memories)" |
| `profile` | `TopicProfile` | Per-space strength array [f32; 13] with all values clamped to [0.0, 1.0] |
| `contributing_spaces` | `Vec<Embedder>` | Spaces where strength > 0.5 AND topic_weight > 0.0 |
| `cluster_ids` | `HashMap<Embedder, i32>` | Dominant cluster ID per space |
| `member_memories` | `Vec<Uuid>` | Memory UUIDs belonging to this topic |
| `confidence` | `f32` | weighted_agreement / 8.5 |
| `silhouette_score` | `f32` | Average silhouette from contributing clusters; default 1.0 |
| `stability` | `TopicStability` | Lifecycle and health metrics |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `parent_id` | `Option<Uuid>` | Parent topic for hierarchical topics |
| `depth` | `u8` | Depth in hierarchy (0 = root, max = 3) |

Topic IDs are deterministic: the same set of member UUIDs always produces the same topic ID regardless of input order, which is essential for accurate churn tracking across reclusters.

The `parent_id` / `depth` fields and the `build_topic_hierarchy()` helper landed in commit 80558edc (2026-02-26) alongside the auto-naming logic introduced in d70a9631. Topic cap enforcement checks the `MAX_TOPICS` limit *before* allocating a new `Topic` (commit 5b0231e7 — refinement R1 to the memory review).

### 7.6 Topic Validity

A topic is valid when both conditions hold:

1. `profile.is_topic()` -- weighted_agreement >= 2.5
2. `silhouette_score >= 0.3`

### 7.7 TopicPhase Lifecycle

| Phase | Conditions | Description |
|-------|------------|-------------|
| **Emerging** | age < 1 hour AND churn > 0.3 | New, membership changing rapidly |
| **Stable** | age >= 24 hours AND churn < 0.1 | Consistent membership |
| **Declining** | churn >= 0.5 | Members leaving |
| **Merging** | Set externally | Being absorbed into another topic |

### 7.8 TopicStability (Per-Topic Metrics)

| Field | Type | Description |
|-------|------|-------------|
| `phase` | `TopicPhase` | Current lifecycle phase |
| `age_hours` | `f32` | Age since creation |
| `membership_churn` | `f32` | Rate of member changes [0.0, 1.0] |
| `centroid_drift` | `f32` | Profile drift since last snapshot [0.0, 1.0] |
| `access_count` | `u32` | Total access count (saturating) |
| `last_accessed` | `Option<DateTime<Utc>>` | Last access time |

Health check: a topic is "healthy" when `membership_churn < 0.3`.

### 7.9 Degenerate Embedder Detection (TOPIC-2)

When computing topic profiles, if ALL members of a topic group land in exactly one cluster for a given embedder (i.e., only 1 distinct non-noise cluster and more than 2 members), that embedder's strength is set to 0.0. This prevents non-functional embedders (e.g., E5 returning all-zero when a LoRA model is not loaded) from inflating weighted agreement. Genuine agreement would show multiple clusters with one dominant.

---

## 8. Topic Synthesizer

The `TopicSynthesizer` in `synthesizer.rs` is a standalone component for cross-space topic discovery, usable independently of `MultiSpaceClusterManager`.

### 8.1 Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `merge_similarity_threshold` | 0.9 | Cosine similarity threshold for merging similar topics |
| `min_silhouette` | 0.3 | Minimum silhouette score for valid clusters |

### 8.2 Synthesis Algorithm

1. **Build per-memory cluster map**: Reorganize memberships from per-space format (`HashMap<Embedder, Vec<ClusterMembership>>`) to per-memory format (`HashMap<Uuid, HashMap<Embedder, i32>>`).

2. **Compute pairwise weighted agreement**: For each pair of memories, sum `category_for(embedder).topic_weight()` for embedders where both memories share the same non-noise cluster. Result range: [0.0, 8.5].

3. **Find connected components**: Build edges between memory pairs with weighted_agreement >= 2.5 (topic_threshold). Use Union-Find with path compression to find connected components.

4. **Create topic candidates**: For each component with >= 2 members:
   - Compute `TopicProfile` where each strength is the fraction of members in the dominant cluster for that embedder
   - Compute dominant `cluster_ids` per space (most common cluster among members)
   - Create `Topic` with computed profile, cluster IDs, and members
   - Filter out invalid topics (weighted_agreement < 2.5)

5. **Merge similar topics**: Sort topics by member count descending. For each pair with profile cosine similarity >= `merge_similarity_threshold` (0.9), absorb the smaller topic into the larger one. Members and cluster IDs are merged; contributing spaces are recomputed.

---

## 9. MultiSpaceClusterManager

The `MultiSpaceClusterManager` in `manager.rs` orchestrates clustering across all 13 embedding spaces.

### 9.1 Structure

```rust
struct MultiSpaceClusterManager {
    params: ManagerParams,
    spaces: [PerSpaceState; 13],       // One per embedder
    topics: HashMap<Uuid, Topic>,       // Discovered topics
    total_memories: usize,
    stability_tracker: TopicStabilityTracker,
}
```

Each `PerSpaceState` contains:

- `tree: BIRCHTree` -- incremental clustering tree
- `embeddings: Vec<Vec<f32>>` -- accumulated embeddings for HDBSCAN
- `memory_ids: Vec<Uuid>` -- corresponding memory IDs
- `memberships: HashMap<Uuid, ClusterMembership>` -- current assignments
- `clusters: HashMap<i32, Cluster>` -- discovered clusters
- `updates_since_recluster: usize` -- counter for triggering HDBSCAN

### 9.2 Memory Insertion

1. For each of the 13 spaces:
   - Validate embedding dimension and finite values
   - Insert into BIRCH tree (returns cluster index)
   - Accumulate embedding and memory ID for later HDBSCAN
   - **Eviction**: when accumulated embeddings exceed `MAX_EMBEDDINGS_PER_SPACE` (50,000), evict the oldest half and prune their memberships
   - Increment `updates_since_recluster`

2. Compute a quick topic profile from the BIRCH cluster indices

3. Return `InsertResult` containing cluster indices, topic profile, and whether reclustering is needed

### 9.3 Batch Reclustering (Independent Per-Space)

Triggered when any space reaches `recluster_threshold` (default: 100) updates:

1. For each space with enough data (>= `min_cluster_size`):
   - Create space-specific `HDBSCANClusterer`
   - Run `fit()` on accumulated embeddings
   - Rebuild cluster objects with computed centroids
   - Reset update counter

2. Re-synthesize topics via `synthesize_topics()`

3. Take a stability snapshot

### 9.4 Topic Synthesis (Manager Internal)

The manager's internal `synthesize_topics()` method differs from the standalone `TopicSynthesizer` in one key way: it applies a **discriminating space filter**. Spaces with only 1 non-noise cluster are excluded from weighted agreement computation because they provide zero discriminative information (every pair would trivially "agree").

Additionally, it implements **mega-component splitting** to handle the transitive closure problem in Union-Find (where A~B and B~C implies A~C even if A and C are unrelated):

1. If a connected component exceeds 40% of all memories and has >= 5 members:
2. Try HDBSCAN re-clustering in each discriminating space (ordered by number of clusters, most first)
3. If any space produces >= 2 sub-clusters, split the component
4. Repeat recursively up to `MAX_SPLIT_ROUNDS` (5)
5. Noise from sub-clustering becomes singleton components (effectively unassigned)

### 9.5 FDMC Reclustering

`recluster_fingerprint_matrix()` provides the alternative FDMC approach:

1. Build fingerprint similarity matrix using `TeleologicalComparator` with `FingerprintMatrixConfig::for_topic_detection()`
2. Convert to distance matrix (`1.0 - similarity`)
3. Run `HDBSCANClusterer::fit_precomputed()` on the distance matrix
4. Compute silhouette score for quality assessment
5. Build topics from cluster assignments where profile meets topic threshold
6. Analyze embedder contributions (variance of per-space similarities)

Returns `FdmcResult` with cluster count, topic count, memberships, silhouette score, similarity stats, per-embedder contributions, and dominant embedder.

### 9.6 Manager Caps

| Cap | Value | Effect |
|-----|-------|--------|
| `MAX_EMBEDDINGS_PER_SPACE` | 50,000 | Oldest half evicted when exceeded (~2.6 GB worst case at 13 spaces) |
| `MAX_TOPICS` | 10,000 | New topics silently skipped when cap reached |
| `DEFAULT_RECLUSTER_THRESHOLD` | 100 | Number of insertions before triggering HDBSCAN |

---

## 10. Fingerprint Matrix Construction

The `fingerprint_matrix.rs` module builds co-association matrices using weighted similarity scores from all 13 embedding spaces.

### 10.1 FingerprintMatrixConfig

| Field | Default | Description |
|-------|---------|-------------|
| `aggregation` | `WeightedSum` | Strategy for combining per-embedder scores |
| `min_similarity_threshold` | 0.0 | Pairs below this are set to 0.0 |
| `store_per_space` | true | Whether to store per-space similarities |

### 10.2 Aggregation Strategies

| Strategy | Behavior |
|----------|----------|
| **WeightedSum** | Uses `TeleologicalComparator`'s built-in category weighting (recommended) |
| **MaxPooling** | Maximum similarity across spaces |
| **MeanPooling** | Mean of all non-zero similarities (unweighted) |
| **ProductRule** | Geometric mean of positive similarities |
| **MinPooling** | Minimum similarity across spaces (most conservative) |

### 10.3 Matrix Construction

`build_fingerprint_matrix()`:

1. Create `TeleologicalComparator`
2. Initialize n x n similarity matrix (diagonal = 1.0)
3. For each pair (i, j):
   - Compare fingerprints using the comparator (apples-to-apples per ARCH-02)
   - Validate result is finite (fail fast per AP-10)
   - Aggregate per-embedder scores using the configured strategy
   - Apply minimum similarity threshold
   - Store in symmetric matrix
4. Optionally store per-space similarity tensor [n x n x 13]

### 10.4 Incremental Update

`update_matrix_incremental()` adds a new memory in O(n) instead of O(n^2):

1. Extend all matrix dimensions by 1
2. Set self-similarity to 1.0
3. Compute similarities only between the new memory and all existing ones

### 10.5 Analysis Methods

| Method | Description |
|--------|-------------|
| `to_distance_matrix()` | Converts similarity to distance (1.0 - sim) for HDBSCAN |
| `analyze_embedder_contributions()` | Returns per-embedder variance of similarity values; high variance = good discriminator |
| `dominant_embedder()` | Returns the embedder with highest contribution to cluster separation |
| `similarity_stats()` | Returns min, max, mean, std_dev, max_gap, gap_position, pair_count |

---

## 11. Cluster Stability Metrics and Divergence Detection

### 11.1 Portfolio-Level Stability (stability.rs)

`TopicStabilityTracker` monitors the entire topic portfolio over time, distinct from the per-topic `TopicStability` metrics.

### 11.2 Snapshot Management

| Constant | Value | Description |
|----------|-------|-------------|
| `SNAPSHOT_RETENTION_HOURS` | 24 | Snapshots older than this are cleaned |
| `MAX_SNAPSHOTS` | 1,440 | Hard cap (1 snapshot/minute for 24 hours) |

Each `TopicSnapshot` stores:

- `timestamp: DateTime<Utc>`
- `topic_ids: Vec<Uuid>` -- only IDs, not full profiles
- `total_members: usize` -- sum of all topic member counts

### 11.3 Churn Calculation

Churn uses the Jaccard distance between two snapshots:

```
churn = |symmetric_difference| / |union|
```

where `symmetric_difference = topics_added + topics_removed`.

- Prefers comparing to a snapshot from ~1 hour ago for stable long-term tracking
- Falls back to the oldest available snapshot in short sessions
- Result clamped to [0.0, 1.0]; NaN/Infinity guarded per AP-10

### 11.4 Dream Trigger Conditions (AP-70)

Dream consolidation triggers when EITHER:

1. `entropy > 0.7 AND churn > 0.5` (both simultaneously)
2. `entropy > 0.7` for 5+ continuous minutes (300 seconds)

| Constant | Value | Description |
|----------|-------|-------------|
| `DEFAULT_CHURN_THRESHOLD` | 0.5 | Churn threshold for dream trigger |
| `DEFAULT_ENTROPY_THRESHOLD` | 0.7 | Entropy threshold for dream trigger |
| `DEFAULT_ENTROPY_DURATION_SECS` | 300 | Required sustained high-entropy duration |

The tracker maintains a `high_entropy_start` timestamp. When entropy drops below threshold, the timer resets. After a dream completes, `reset_entropy_tracking()` clears the timer.

The consumer-side `DreamPhase` enum and `DreamConsolidationStatus` struct (introduced in commit 80558edc, 2026-02-26) live in `crates/context-graph-mcp/src/handlers/tools/consolidation.rs` (lines 49 and 59). The clustering subsystem only provides the trigger conditions via `TopicStabilityTracker`; the MCP layer owns the lifecycle/phase DTOs exposed through `trigger_consolidation` / `get_memetic_status`.

### 11.5 Portfolio Stability Assessment

| State | Condition | Description |
|-------|-----------|-------------|
| Healthy | avg churn < 0.2 over 6 hours | System is stable |
| Warning | churn [0.3, 0.5) | Moderate instability |
| Unstable | churn >= 0.5 | High instability, dream candidate |

The `is_stable()` method uses a conservative threshold of `avg_churn(6 hours) < 0.2`.

---

## 12. Cluster Persistence

### 12.1 PersistedTopicPortfolio

Defined in `persistence.rs`, this is the serializable snapshot of the topic portfolio for session continuity:

| Field | Type | Description |
|-------|------|-------------|
| `topics` | `Vec<Topic>` | All topics with profiles and stability metrics |
| `churn_rate` | `f32` | Portfolio-level churn at persistence time [0.0, 1.0] |
| `entropy` | `f32` | Portfolio-level entropy at persistence time [0.0, 1.0] |
| `session_id` | `String` | Session that created this snapshot |
| `persisted_at_ms` | `u64` | Unix timestamp in milliseconds |

Serialization uses JSON format (via serde_json) for human readability and debuggability. All `f32` metrics are clamped to [0.0, 1.0] with NaN/Infinity guarded to 0.0.

### 12.2 TopicPortfolio (Live Runtime)

`TopicPortfolio` is the runtime counterpart with integrated stability tracking:

- Stores topics in `HashMap<Uuid, Topic>` for O(1) lookup
- Embeds a `TopicStabilityTracker` for churn computation and dream trigger support
- Provides `export()` to create a `PersistedTopicPortfolio`
- Provides `import()` to restore topics from a persisted portfolio (does NOT restore stability tracker state; snapshots are not persisted)

### 12.3 Manager Portfolio Export/Import

The `MultiSpaceClusterManager` provides:

- `export_portfolio(session_id, churn_rate, entropy)` -- Creates a `PersistedTopicPortfolio` from current topics
- `export_portfolio_with_internal_churn(session_id, entropy)` -- Uses the stability tracker's computed churn instead of an external value
- `import_portfolio(portfolio)` -- Restores topics from a persisted portfolio, replacing current topics

---

## 13. Topic Hierarchy

### 13.1 build_topic_hierarchy

The `build_topic_hierarchy()` function in `topic.rs` groups flat topics into a hierarchy using single-linkage clustering on topic profile cosine similarity:

1. Compute pairwise cosine similarity of `TopicProfile::strengths` vectors
2. Union topics where similarity exceeds `threshold` (typical: 0.7) using Union-Find
3. For each group with 2+ members:
   - Compute averaged profile strengths across group members
   - Create a parent topic with the averaged profile and all members from children
   - Set parent name to `"Category: {first_child_name}"`
   - Set `parent_id` on children, `depth = 1` on children, `depth = 0` on parent

The hierarchy supports maximum `MAX_TOPIC_DEPTH = 3` levels.

### 13.2 Topic Naming

Topics are auto-named from their dominant contributing spaces:

```
{Space1}+{Space2}+{Space3} ({N} memories)
```

Embedder labels: Semantic, Causal, Keyword, Code, Graph, Robust, Paraphrase, Entity, Precision, SPLADE. Temporal embedders never appear in contributing spaces per AP-60.

---

## 14. Error Handling

### 14.1 ClusterError

| Variant | Description |
|---------|-------------|
| `InsufficientData { required, actual }` | Not enough data points for clustering |
| `DimensionMismatch { expected, actual }` | Embedding dimension does not match expected |
| `NoValidClusters` | All points classified as noise |
| `StorageError(StorageError)` | Underlying storage operation failed |
| `InvalidParameter { message }` | Invalid configuration parameter |
| `SpaceNotInitialized(Embedder)` | Embedding space not initialized |

### 14.2 PersistenceError

| Variant | Description |
|---------|-------------|
| `Serialization(serde_json::Error)` | JSON serialization/deserialization failed |
| `InvalidData { message }` | Invalid data format |

---

## 15. Integration with the Search Subsystem

The clustering and topic subsystem integrates with search in several ways:

1. **Topic-guided search**: Discovered topics provide cluster IDs and member lists that can scope search operations. A search can be restricted to memories within a specific topic for focused retrieval.

2. **Fingerprint comparison**: The FDMC approach reuses the `TeleologicalComparator` from the search/comparison subsystem (see the teleological embedder documentation) to compute apples-to-apples similarity across all 13 spaces.

3. **Embedder category weights**: The same `EmbedderCategory` and `category_for()` function used by the search pipeline's fusion and weight profiles is used by the clustering system for weighted agreement. This ensures topic detection aligns with the search system's understanding of embedder importance.

4. **Dream consolidation trigger**: The stability tracker's dream trigger (entropy > 0.7 AND churn > 0.5) interfaces with the memory consolidation system. When triggered, the dream phases (NREM replay, REM exploration) use topic boundaries to guide consolidation.

5. **Cluster membership in MCP**: The MCP layer exposes topic and cluster data through tools like `get_memetic_status`, which reports the current topic portfolio, churn rate, and stability metrics to clients.

6. **Per-space distance metrics**: HDBSCAN uses the same per-space distance metrics (`get_distance_metric(embedder)`) configured for HNSW index search, ensuring clustering boundaries align with search-time distance computations.

---

## 16. Constellation Module (Related Capability)

A separate, parallel module — `crates/context-graph-core/src/constellation/` (added in commits `7179bd7c` and wired through `48f0420d`) — provides a distinct but related compile-time grouping capability. Unlike the clustering subsystem, which discovers emergent groupings from the full corpus on every recluster pass, a **constellation** is an explicitly compiled snapshot of a caller-selected set of memories (by filter/selector) collapsed into a single aggregate record.

### 16.1 Module Layout

| File | Purpose |
|------|---------|
| `constellation/mod.rs` | Public types: `Constellation`, `ConstellationSelector`, `EmbedderStats`, `VectorKind`, `ConstellationScoringResult`; version/dim constants |
| `constellation/compiler.rs` | Collapses a memory group into a single `Constellation` record (per-embedder centroid + moments) |
| `constellation/reservoir.rs` | `ReservoirSample` — uniform bounded-memory sampler for percentile estimation, cap `RESERVOIR_SAMPLE_SIZE = 1024` |
| `constellation/welford.rs` | `WelfordMoments` — online mean / variance (numerically stable single-pass) |
| `constellation/types.rs` | `EmbedderStats` / `VectorKind` / dimension constants / scoring structs |
| `constellation/tests.rs` | 15 tests covering reservoir uniformity, Welford correctness, compile/score roundtrips |

### 16.2 Key Constants

| Constant | Value | Location |
|----------|------:|----------|
| `CONSTELLATION_VERSION` | 1 | `constellation/mod.rs:52` |
| `RESERVOIR_SAMPLE_SIZE` | 1,024 | `constellation/mod.rs:56` |
| `MIN_CONSTELLATION_MEMBERS` | 3 | `constellation/mod.rs:61` |

### 16.3 Relationship to Clustering

| Property | Clustering (`manager.rs`) | Constellation (`constellation/`) |
|----------|----------------------------|------------------------------------|
| Trigger | Automatic (BIRCH per insert, HDBSCAN every `DEFAULT_RECLUSTER_THRESHOLD`) | Explicit — caller invokes `compile_constellation` MCP tool with selector |
| Scope | Full workspace corpus | Caller-specified memory subset |
| Output | `Topic` records with hierarchy + stability snapshots | Single `Constellation` record persisted via `CF_CONSTELLATIONS` + secondary index `CF_CONSTELLATION_BY_SELECTOR` |
| Algorithm | HDBSCAN + BIRCH + weighted agreement | Per-embedder centroid + Welford moments + reservoir percentiles |
| Dynamics | Updates continuously; tracks churn | Static snapshot; fully replaced on recompile |
| MCP surface | `detect_topics`, `get_topic_portfolio`, `get_topic_stability`, `get_divergence_alerts` | `compile_constellation`, `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation` |

### 16.4 Storage

Constellations persist to `CF_CONSTELLATIONS` (UUID-keyed) with a secondary index `CF_CONSTELLATION_BY_SELECTOR` for selector-based lookup. Wire format is `[version: u8][bincode payload]`. See [04_database_schema.md](04_database_schema.md) Section 3.2 and [09_storage_layer.md](09_storage_layer.md) Section 11 for full schema.
