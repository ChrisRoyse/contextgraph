# TASK-L006: Purpose Pattern Index

```yaml
metadata:
  id: "TASK-L006"
  title: "Purpose Pattern Index"
  layer: "logic"
  priority: "P1"
  estimated_hours: 6
  created: "2026-01-04"
  status: "pending"
  dependencies:
    - "TASK-L002"  # Purpose Vector Computation
    - "TASK-L005"  # Per-Space HNSW Index Builder
  spec_refs:
    - "projectionplan1.md:purpose-pattern-index"
    - "projectionplan2.md:alignment-search"
```

## Problem Statement

Build a 13-dimensional HNSW index specifically for Purpose Vectors, enabling fast retrieval of memories with similar purpose alignment patterns across all embedding spaces including E13 SPLADE.

## Context

While per-space indexes enable content similarity search, the Purpose Pattern Index enables **purpose similarity search** - finding memories that serve similar goals regardless of their content. With E13 SPLADE integration, this now captures both semantic and lexical goal alignment. This supports:
- Goal-based memory organization
- Purpose clustering and visualization
- Alignment pattern discovery
- Strategic memory recommendation
- **Lexical goal alignment via E13 SPLADE**

## Technical Specification

### Data Structures

```rust
/// Entry in the purpose pattern index
#[derive(Clone, Debug)]
pub struct PurposeIndexEntry {
    /// Memory identifier
    pub memory_id: MemoryId,

    /// The 12D purpose vector
    pub purpose_vector: PurposeVector,

    /// Additional metadata for filtering
    pub metadata: PurposeMetadata,
}

/// Metadata for purpose-based filtering
#[derive(Clone, Debug)]
pub struct PurposeMetadata {
    /// Primary goal this memory aligns with
    pub primary_goal: GoalId,

    /// Confidence in purpose assignment
    pub confidence: f32,

    /// When purpose was last computed
    pub computed_at: Timestamp,

    /// Dominant quadrant from Johari
    pub dominant_quadrant: JohariQuadrant,
}

/// Configuration for purpose pattern index
#[derive(Clone, Debug)]
pub struct PurposeIndexConfig {
    /// HNSW M parameter (connections per node)
    pub m: usize,

    /// HNSW ef_construction
    pub ef_construction: usize,

    /// HNSW ef_search (default)
    pub ef_search: usize,

    /// Maximum elements
    pub max_elements: usize,

    /// Distance metric (typically Euclidean for purpose alignment)
    pub distance_metric: DistanceMetric,
}

impl Default for PurposeIndexConfig {
    fn default() -> Self {
        Self {
            m: 24,              // Higher M for low-dimension
            ef_construction: 200,
            ef_search: 100,
            max_elements: 1_000_000,
            distance_metric: DistanceMetric::Euclidean,
        }
    }
}

/// Query for purpose-based search
#[derive(Clone, Debug)]
pub struct PurposeQuery {
    /// Target purpose vector (or alignment pattern)
    pub target: PurposeQueryTarget,

    /// Maximum results
    pub limit: usize,

    /// Minimum similarity threshold
    pub min_similarity: f32,

    /// Optional goal filter
    pub goal_filter: Option<GoalId>,

    /// Optional quadrant filter
    pub quadrant_filter: Option<JohariQuadrant>,
}

#[derive(Clone, Debug)]
pub enum PurposeQueryTarget {
    /// Direct purpose vector
    Vector(PurposeVector),

    /// Alignment pattern (which spaces to prioritize)
    Pattern { alignment: [f32; 12] },

    /// Goal-derived (compute ideal purpose for goal)
    FromGoal(GoalId),

    /// Memory-derived (find similar purpose to existing memory)
    FromMemory(MemoryId),
}

/// Result from purpose search
#[derive(Clone, Debug)]
pub struct PurposeSearchResult {
    pub memory_id: MemoryId,
    pub purpose_similarity: f32,
    pub purpose_vector: PurposeVector,
    pub metadata: PurposeMetadata,
}
```

### Core Trait

```rust
/// Index for purpose pattern similarity search
#[async_trait]
pub trait PurposePatternIndex: Send + Sync {
    /// Add a purpose vector to the index
    async fn add(
        &mut self,
        entry: PurposeIndexEntry,
    ) -> Result<(), PurposeIndexError>;

    /// Add multiple entries (batch)
    async fn add_batch(
        &mut self,
        entries: Vec<PurposeIndexEntry>,
    ) -> Result<(), PurposeIndexError>;

    /// Update purpose for existing memory
    async fn update(
        &mut self,
        memory_id: MemoryId,
        new_purpose: PurposeVector,
    ) -> Result<(), PurposeIndexError>;

    /// Search by purpose similarity
    async fn search(
        &self,
        query: PurposeQuery,
    ) -> Result<Vec<PurposeSearchResult>, PurposeIndexError>;

    /// Find memories with similar purpose patterns
    async fn find_similar(
        &self,
        memory_id: MemoryId,
        limit: usize,
    ) -> Result<Vec<PurposeSearchResult>, PurposeIndexError>;

    /// Cluster memories by purpose
    async fn cluster_by_purpose(
        &self,
        num_clusters: usize,
    ) -> Result<Vec<PurposeCluster>, PurposeIndexError>;

    /// Remove from index
    async fn remove(&mut self, memory_id: MemoryId) -> Result<(), PurposeIndexError>;

    /// Get index statistics
    fn stats(&self) -> PurposeIndexStats;

    /// Persist to storage
    async fn persist(&self) -> Result<(), PurposeIndexError>;

    /// Load from storage
    async fn load(&mut self) -> Result<(), PurposeIndexError>;
}

/// Cluster of memories with similar purpose
#[derive(Clone, Debug)]
pub struct PurposeCluster {
    pub cluster_id: usize,
    pub centroid: PurposeVector,
    pub members: Vec<MemoryId>,
    pub dominant_goal: GoalId,
    pub alignment_variance: f32,
}

/// Index statistics
#[derive(Clone, Debug)]
pub struct PurposeIndexStats {
    pub total_entries: usize,
    pub unique_goals: usize,
    pub avg_confidence: f32,
    pub memory_usage_bytes: usize,
}
```

### Implementation

```rust
/// HNSW-based purpose pattern index
pub struct HnswPurposeIndex {
    index: HnswIndex,
    metadata: HashMap<MemoryId, PurposeMetadata>,
    id_to_offset: HashMap<MemoryId, usize>,
    offset_to_id: Vec<MemoryId>,
    config: PurposeIndexConfig,
    storage_path: PathBuf,
}

impl HnswPurposeIndex {
    pub fn new(config: PurposeIndexConfig, storage_path: PathBuf) -> Result<Self, PurposeIndexError> {
        let index = HnswIndex::new(
            13,  // Purpose vector is 13D (E1-E12 dense + E13 SPLADE)
            config.max_elements,
            config.m,
            config.ef_construction,
            config.distance_metric.into(),
        )?;

        Ok(Self {
            index,
            metadata: HashMap::new(),
            id_to_offset: HashMap::new(),
            offset_to_id: Vec::new(),
            config,
            storage_path,
        })
    }
}

#[async_trait]
impl PurposePatternIndex for HnswPurposeIndex {
    async fn search(
        &self,
        query: PurposeQuery,
    ) -> Result<Vec<PurposeSearchResult>, PurposeIndexError> {
        // Resolve query target to vector
        let query_vector = match query.target {
            PurposeQueryTarget::Vector(pv) => pv.alignment.to_vec(),
            PurposeQueryTarget::Pattern { alignment } => alignment.to_vec(),
            PurposeQueryTarget::FromGoal(goal_id) => {
                self.compute_goal_purpose(&goal_id)?
            }
            PurposeQueryTarget::FromMemory(memory_id) => {
                self.get_purpose(&memory_id)?.alignment.to_vec()
            }
        };

        // HNSW search
        let raw_results = self.index.search(&query_vector, query.limit * 2)?;

        // Filter and transform
        let mut results = Vec::with_capacity(query.limit);
        for (offset, distance) in raw_results {
            let memory_id = &self.offset_to_id[offset];
            let metadata = &self.metadata[memory_id];

            // Apply filters
            if let Some(ref goal) = query.goal_filter {
                if &metadata.primary_goal != goal {
                    continue;
                }
            }

            if let Some(quadrant) = query.quadrant_filter {
                if metadata.dominant_quadrant != quadrant {
                    continue;
                }
            }

            // Convert distance to similarity
            let similarity = 1.0 / (1.0 + distance);
            if similarity < query.min_similarity {
                continue;
            }

            results.push(PurposeSearchResult {
                memory_id: memory_id.clone(),
                purpose_similarity: similarity,
                purpose_vector: self.get_purpose(memory_id)?,
                metadata: metadata.clone(),
            });

            if results.len() >= query.limit {
                break;
            }
        }

        Ok(results)
    }
}
```

## Implementation Requirements

### Prerequisites

- [ ] TASK-L002 complete (PurposeVector available)
- [ ] TASK-L005 complete (HNSW infrastructure)

### Scope

#### In Scope

- 12D HNSW index for purpose vectors
- Purpose similarity search
- Metadata filtering (goal, quadrant)
- Purpose-based clustering
- Index persistence

#### Out of Scope

- Purpose vector computation (TASK-L002)
- Goal hierarchy management (TASK-L003)
- Multi-space content search (TASK-L001)

### Constraints

- Index dimension fixed at **13** (E1-E12 dense + E13 SPLADE)
- Memory < 500MB for 1M entries
- Search latency < 5ms
- Thread-safe operations

## Pseudo Code

```
FUNCTION add_to_purpose_index(entry):
    // Convert purpose vector to 13D array
    vector = entry.purpose_vector.alignment  // [f32; 13] (E1-E12 + E13 SPLADE)

    // Get next offset
    offset = offset_to_id.len()

    // Add to HNSW index
    index.add(offset, vector)

    // Store mappings
    id_to_offset[entry.memory_id] = offset
    offset_to_id.push(entry.memory_id)
    metadata[entry.memory_id] = entry.metadata

    RETURN Ok(())

FUNCTION search_by_purpose(query):
    // Resolve query to 13D vector
    query_vector = resolve_query_target(query.target)

    // Search HNSW
    raw_results = index.search(query_vector, query.limit * 2)

    // Filter and transform
    results = []
    FOR (offset, distance) IN raw_results:
        memory_id = offset_to_id[offset]
        meta = metadata[memory_id]

        // Apply goal filter
        IF query.goal_filter AND meta.primary_goal != query.goal_filter:
            CONTINUE

        // Apply quadrant filter
        IF query.quadrant_filter AND meta.dominant_quadrant != query.quadrant_filter:
            CONTINUE

        // Convert distance to similarity
        similarity = 1.0 / (1.0 + distance)

        IF similarity >= query.min_similarity:
            results.push(PurposeSearchResult {
                memory_id,
                purpose_similarity: similarity,
                purpose_vector: get_purpose_vector(memory_id),
                metadata: meta
            })

        IF results.len() >= query.limit:
            BREAK

    RETURN results

FUNCTION cluster_by_purpose(num_clusters):
    // Extract all purpose vectors
    vectors = []
    FOR offset IN 0..offset_to_id.len():
        memory_id = offset_to_id[offset]
        pv = get_purpose_vector(memory_id)
        vectors.push((memory_id, pv.alignment))

    // K-means clustering on 12D space
    centroids = kmeans_init(vectors, num_clusters)

    FOR iteration IN 0..MAX_ITERATIONS:
        // Assign each vector to nearest centroid
        assignments = []
        FOR (memory_id, vector) IN vectors:
            nearest = find_nearest_centroid(vector, centroids)
            assignments.push((memory_id, nearest))

        // Recompute centroids
        new_centroids = compute_centroids(vectors, assignments)

        IF converged(centroids, new_centroids):
            BREAK

        centroids = new_centroids

    // Build cluster results
    clusters = []
    FOR cluster_id IN 0..num_clusters:
        members = get_members(assignments, cluster_id)
        centroid = centroids[cluster_id]
        dominant_goal = find_dominant_goal(members)
        variance = compute_variance(members)

        clusters.push(PurposeCluster {
            cluster_id,
            centroid: PurposeVector::from_alignment(centroid),
            members,
            dominant_goal,
            alignment_variance: variance
        })

    RETURN clusters
```

## Definition of Done

### Implementation Checklist

- [ ] `PurposeIndexEntry` struct
- [ ] `PurposeQuery` with multiple target types
- [ ] `PurposePatternIndex` trait
- [ ] HNSW-based implementation (**13D**: E1-E12 + E13 SPLADE)
- [ ] Metadata filtering support
- [ ] Purpose-based clustering
- [ ] Index persistence/loading
- [ ] Statistics tracking
- [ ] **E13 SPLADE alignment in purpose queries**

### Testing Requirements

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_search() {
        let mut index = HnswPurposeIndex::new(
            PurposeIndexConfig::default(),
            temp_dir(),
        ).unwrap();

        // Add entries with similar purpose
        for i in 0..100 {
            let entry = PurposeIndexEntry {
                memory_id: MemoryId::from(i),
                purpose_vector: create_purpose_vector(i as f32 / 100.0),
                metadata: PurposeMetadata::default(),
            };
            index.add(entry).await.unwrap();
        }

        // Search for similar purpose
        let query = PurposeQuery {
            target: PurposeQueryTarget::Pattern {
                alignment: [0.5; 13],  // 13D: E1-E12 + E13 SPLADE
            },
            limit: 10,
            min_similarity: 0.0,
            goal_filter: None,
            quadrant_filter: None,
        };

        let results = index.search(query).await.unwrap();
        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_goal_filter() {
        let mut index = create_test_index().await;

        let query = PurposeQuery {
            target: PurposeQueryTarget::Pattern { alignment: [0.5; 13] },  // 13D
            limit: 10,
            min_similarity: 0.0,
            goal_filter: Some(GoalId("target_goal".into())),
            quadrant_filter: None,
        };

        let results = index.search(query).await.unwrap();
        for r in results {
            assert_eq!(r.metadata.primary_goal, GoalId("target_goal".into()));
        }
    }

    #[tokio::test]
    async fn test_clustering() {
        let mut index = create_test_index_with_clusters().await;

        let clusters = index.cluster_by_purpose(3).await.unwrap();

        assert_eq!(clusters.len(), 3);
        // Each cluster should have members
        for c in clusters {
            assert!(!c.members.is_empty());
        }
    }

    #[tokio::test]
    async fn test_find_similar() {
        let mut index = create_test_index().await;
        let memory_id = MemoryId::from(50);

        let similar = index.find_similar(memory_id, 5).await.unwrap();

        assert_eq!(similar.len(), 5);
    }
}
```

### Verification Commands

```bash
# Run unit tests
cargo test -p context-graph-core purpose_pattern_index

# Benchmark search
cargo bench -p context-graph-core -- purpose_search

# Memory usage test
cargo test -p context-graph-core purpose_index_memory -- --nocapture
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/index/purpose_index.rs` | Purpose pattern index implementation |
| `crates/context-graph-core/src/index/purpose_query.rs` | Query types |
| `crates/context-graph-core/src/index/clustering.rs` | Purpose clustering |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/index/mod.rs` | Add purpose index exports |

## Traceability

| Requirement | Source | Coverage |
|-------------|--------|----------|
| **13D purpose index** | projectionplan1.md:purpose-pattern | Complete |
| Purpose similarity | projectionplan2.md:alignment | Complete |
| Clustering | projectionplan2.md:organization | Complete |
| **E13 SPLADE alignment** | projectionplan2.md:splade | Complete |

---

*Task created: 2026-01-04*
*Layer: Logic*
*Priority: P1 - Purpose-aware retrieval*
