# TASK-L005: Per-Space HNSW Index Builder

```yaml
metadata:
  id: "TASK-L005"
  title: "Per-Space HNSW Index Builder"
  layer: "logic"
  priority: "P1"
  estimated_hours: 10
  created: "2026-01-04"
  status: "pending"
  dependencies:
    - "TASK-F001"  # SemanticFingerprint struct
    - "TASK-F005"  # HNSW index configuration
    - "TASK-F004"  # Storage schema
  spec_refs:
    - "projectionplan1.md:per-embedder-indexes"
    - "projectionplan2.md:hnsw-configuration"
```

## Problem Statement

Build and maintain 13 separate HNSW indexes plus specialized indexes for the 5-stage retrieval pipeline, including Matryoshka 128D for Stage 2 fast filtering and SPLADE inverted index for Stage 1 sparse retrieval.

## Context

The Multi-Array architecture stores 13 separate embedding spaces (E1-E13) that require independent HNSW indexes. The 5-stage pipeline also requires:
- **Stage 1: SPLADE Inverted Index** - Sparse lexical retrieval for initial candidates
- **Stage 2: Matryoshka 128D Index** - Dimensionality-reduced dense index for fast filtering
- **Stages 3-5: Full HNSW Indexes** - Per-space indexes for final retrieval and reranking

This approach enables:
- Per-space tuning (different M, efConstruction per dimension)
- Selective space activation (skip unused indexes)
- Independent scaling (high-traffic spaces can be larger)
- Graceful degradation (one index failure doesn't affect others)
- **Progressive refinement** through pipeline stages

## Technical Specification

### Data Structures

```rust
/// Configuration for a single HNSW index
#[derive(Clone, Debug)]
pub struct HnswIndexConfig {
    /// Space index (0-11)
    pub space_index: usize,

    /// Space name for logging
    pub space_name: &'static str,

    /// Vector dimension for this space
    pub dimension: usize,

    /// Maximum number of connections per layer
    pub m: usize,

    /// Size of dynamic candidate list during construction
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search
    pub ef_search: usize,

    /// Distance metric
    pub distance_metric: DistanceMetric,

    /// Whether to use SIMD optimizations
    pub use_simd: bool,

    /// Maximum index size (for pre-allocation)
    pub max_elements: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

/// Configuration for Matryoshka 128D index (Stage 2 fast filtering)
pub const MATRYOSHKA_128D_CONFIG: HnswIndexConfig = HnswIndexConfig {
    space_index: 100,  // Special index ID for Matryoshka
    space_name: "matryoshka_128d",
    dimension: 128,
    m: 32,              // Higher M for low dimension
    ef_construction: 300,
    ef_search: 150,
    distance_metric: DistanceMetric::Cosine,
    use_simd: true,
    max_elements: 1_000_000,
};

/// Configuration for SPLADE inverted index (Stage 1 sparse retrieval)
#[derive(Clone, Debug)]
pub struct SpladeIndexConfig {
    /// Space index for E13 SPLADE
    pub space_index: usize,

    /// Vocabulary size (BERT-base)
    pub vocab_size: usize,

    /// Maximum posting list length
    pub max_posting_length: usize,

    /// Quantization bits for term weights (0 = no quantization)
    pub weight_quantization_bits: u8,

    /// Maximum elements in index
    pub max_elements: usize,
}

impl Default for SpladeIndexConfig {
    fn default() -> Self {
        Self {
            space_index: 12,  // E13
            vocab_size: 30522,
            max_posting_length: 100_000,
            weight_quantization_bits: 8,
            max_elements: 1_000_000,
        }
    }
}

/// Default configurations for each embedding space
pub const SPACE_CONFIGS: [HnswIndexConfig; 13] = [
    // E1: Semantic Core (dense, high-dimensional)
    HnswIndexConfig {
        space_index: 0,
        space_name: "semantic_core",
        dimension: 1536,
        m: 16,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 1_000_000,
    },
    // E2: Temporal Context (medium)
    HnswIndexConfig {
        space_index: 1,
        space_name: "temporal",
        dimension: 768,
        m: 12,
        ef_construction: 150,
        ef_search: 80,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 1_000_000,
    },
    // E3: Causal Relations (medium)
    HnswIndexConfig {
        space_index: 2,
        space_name: "causal",
        dimension: 768,
        m: 12,
        ef_construction: 150,
        ef_search: 80,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 500_000,
    },
    // E4: Sparse BM25 (sparse representation)
    HnswIndexConfig {
        space_index: 3,
        space_name: "sparse_bm25",
        dimension: 30522, // Vocabulary size
        m: 8,
        ef_construction: 100,
        ef_search: 50,
        distance_metric: DistanceMetric::DotProduct,
        use_simd: true,
        max_elements: 1_000_000,
    },
    // E5: Code Embeddings
    HnswIndexConfig {
        space_index: 4,
        space_name: "code",
        dimension: 768,
        m: 16,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 500_000,
    },
    // E6: Graph Embeddings
    HnswIndexConfig {
        space_index: 5,
        space_name: "graph",
        dimension: 256,
        m: 24,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Euclidean,
        use_simd: true,
        max_elements: 500_000,
    },
    // E7: Hyperdimensional Computing
    HnswIndexConfig {
        space_index: 6,
        space_name: "hdc",
        dimension: 10000,
        m: 8,
        ef_construction: 100,
        ef_search: 50,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 200_000,
    },
    // E8: Multimodal (CLIP-like)
    HnswIndexConfig {
        space_index: 7,
        space_name: "multimodal",
        dimension: 512,
        m: 16,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 500_000,
    },
    // E9: Entity Linking
    HnswIndexConfig {
        space_index: 8,
        space_name: "entity",
        dimension: 256,
        m: 24,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 1_000_000,
    },
    // E10: Late Interaction (ColBERT-style)
    HnswIndexConfig {
        space_index: 9,
        space_name: "late_interaction",
        dimension: 128,
        m: 32,
        ef_construction: 300,
        ef_search: 150,
        distance_metric: DistanceMetric::DotProduct,
        use_simd: true,
        max_elements: 1_000_000,
    },
    // E11: Contextual Embeddings
    HnswIndexConfig {
        space_index: 10,
        space_name: "contextual",
        dimension: 1024,
        m: 16,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 1_000_000,
    },
    // E12: Meta Embeddings
    HnswIndexConfig {
        space_index: 11,
        space_name: "meta",
        dimension: 256,
        m: 24,
        ef_construction: 200,
        ef_search: 100,
        distance_metric: DistanceMetric::Cosine,
        use_simd: true,
        max_elements: 500_000,
    },
    // E13: SPLADE Sparse Embeddings (stored as dense for HNSW fallback)
    HnswIndexConfig {
        space_index: 12,
        space_name: "splade",
        dimension: 30522,  // Vocabulary size
        m: 8,
        ef_construction: 100,
        ef_search: 50,
        distance_metric: DistanceMetric::DotProduct,
        use_simd: true,
        max_elements: 1_000_000,
    },
];

/// Status of a single index
#[derive(Clone, Debug)]
pub struct IndexStatus {
    pub space_index: usize,
    pub space_name: &'static str,
    pub is_loaded: bool,
    pub element_count: usize,
    pub memory_usage_bytes: usize,
    pub last_updated: Timestamp,
    pub health: IndexHealth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexHealth {
    Healthy,
    Degraded,
    Failed,
    Rebuilding,
}
```

### Core Trait

```rust
/// Manages multiple HNSW indexes for the 13 embedding spaces plus pipeline indexes
#[async_trait]
pub trait MultiSpaceIndexManager: Send + Sync {
    /// Initialize all indexes (13 per-space + Matryoshka 128D + SPLADE inverted)
    async fn initialize(&mut self, configs: &[HnswIndexConfig]) -> Result<(), IndexError>;

    /// Initialize Matryoshka 128D index for Stage 2 fast filtering
    async fn initialize_matryoshka(&mut self, config: &HnswIndexConfig) -> Result<(), IndexError>;

    /// Initialize SPLADE inverted index for Stage 1 sparse retrieval
    async fn initialize_splade_inverted(&mut self, config: &SpladeIndexConfig) -> Result<(), IndexError>;

    /// Build/rebuild a specific index
    async fn build_index(
        &mut self,
        space_index: usize,
        vectors: impl Iterator<Item = (MemoryId, Vec<f32>)>,
    ) -> Result<IndexStatus, IndexError>;

    /// Add a vector to a specific index
    async fn add_vector(
        &mut self,
        space_index: usize,
        memory_id: MemoryId,
        vector: Vec<f32>,
    ) -> Result<(), IndexError>;

    /// Add vectors to multiple indexes (from SemanticFingerprint)
    async fn add_fingerprint(
        &mut self,
        memory_id: MemoryId,
        fingerprint: &SemanticFingerprint,
    ) -> Result<(), IndexError>;

    /// Search a specific index
    async fn search(
        &self,
        space_index: usize,
        query: &[f32],
        k: usize,
        ef_search: Option<usize>,
    ) -> Result<Vec<(MemoryId, f32)>, IndexError>;

    /// Parallel search across multiple indexes
    async fn search_multi(
        &self,
        spaces: &[usize],
        queries: &[Vec<f32>],
        k: usize,
    ) -> Result<Vec<Vec<(MemoryId, f32)>>, IndexError>;

    /// Stage 1: SPLADE sparse retrieval
    async fn search_splade(
        &self,
        sparse_query: &[(usize, f32)],  // (term_id, weight) pairs
        k: usize,
    ) -> Result<Vec<(MemoryId, f32)>, IndexError>;

    /// Stage 2: Matryoshka 128D fast filtering
    async fn search_matryoshka_128d(
        &self,
        query_128d: &[f32],
        k: usize,
    ) -> Result<Vec<(MemoryId, f32)>, IndexError>;

    /// Add to Matryoshka 128D index (truncated from full embedding)
    async fn add_matryoshka(
        &mut self,
        memory_id: MemoryId,
        full_embedding: &[f32],  // Will be truncated to 128D
    ) -> Result<(), IndexError>;

    /// Add to SPLADE inverted index
    async fn add_splade(
        &mut self,
        memory_id: MemoryId,
        sparse_embedding: &[(usize, f32)],  // (term_id, weight) pairs
    ) -> Result<(), IndexError>;

    /// Remove a vector from all indexes
    async fn remove(&mut self, memory_id: MemoryId) -> Result<(), IndexError>;

    /// Get status of all indexes
    fn status(&self) -> Vec<IndexStatus>;

    /// Get status of a specific index
    fn index_status(&self, space_index: usize) -> Option<&IndexStatus>;

    /// Warm up specific indexes (pre-load into memory)
    async fn warm_up(&mut self, spaces: &[usize]) -> Result<(), IndexError>;

    /// Persist indexes to storage
    async fn persist(&self) -> Result<(), IndexError>;

    /// Load indexes from storage
    async fn load(&mut self) -> Result<(), IndexError>;
}
```

### Implementation Details

```rust
/// Implementation using hnswlib-rs or similar
pub struct HnswMultiSpaceIndex {
    /// 13 per-space HNSW indexes (E1-E13)
    indexes: [Option<HnswIndex>; 13],
    configs: [HnswIndexConfig; 13],
    status: [IndexStatus; 13],

    /// Matryoshka 128D index for Stage 2 fast filtering
    matryoshka_index: Option<HnswIndex>,
    matryoshka_config: HnswIndexConfig,

    /// SPLADE inverted index for Stage 1 sparse retrieval
    splade_inverted_index: Option<SpladeInvertedIndex>,
    splade_config: SpladeIndexConfig,

    storage_path: PathBuf,
}

/// SPLADE inverted index structure
pub struct SpladeInvertedIndex {
    /// Posting lists: term_id -> [(memory_id, weight)]
    posting_lists: HashMap<usize, Vec<(MemoryId, f32)>>,

    /// Document norms for score normalization
    doc_norms: HashMap<MemoryId, f32>,

    /// Total documents indexed
    num_docs: usize,
}

impl HnswMultiSpaceIndex {
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            indexes: Default::default(),
            configs: SPACE_CONFIGS,
            status: [IndexStatus::default(); 12],
            storage_path,
        }
    }

    fn create_index(&self, config: &HnswIndexConfig) -> Result<HnswIndex, IndexError> {
        let mut index = HnswIndex::new(
            config.dimension,
            config.max_elements,
            config.m,
            config.ef_construction,
            config.distance_metric.into(),
        )?;

        index.set_ef(config.ef_search);

        if config.use_simd {
            index.enable_simd()?;
        }

        Ok(index)
    }
}

#[async_trait]
impl MultiSpaceIndexManager for HnswMultiSpaceIndex {
    async fn add_fingerprint(
        &mut self,
        memory_id: MemoryId,
        fingerprint: &SemanticFingerprint,
    ) -> Result<(), IndexError> {
        // Add to each space that has an embedding
        for (space_idx, embedding) in fingerprint.embeddings.iter().enumerate() {
            if let Some(emb) = embedding {
                self.add_vector(space_idx, memory_id.clone(), emb.clone()).await?;
            }
        }
        Ok(())
    }

    async fn search_multi(
        &self,
        spaces: &[usize],
        queries: &[Vec<f32>],
        k: usize,
    ) -> Result<Vec<Vec<(MemoryId, f32)>>, IndexError> {
        // Parallel search across all requested spaces
        let futures: Vec<_> = spaces.iter()
            .zip(queries.iter())
            .filter_map(|(&space_idx, query)| {
                if let Some(index) = &self.indexes[space_idx] {
                    Some(async move {
                        index.search(query, k)
                    })
                } else {
                    None
                }
            })
            .collect();

        // Execute in parallel
        let results = futures::future::try_join_all(futures).await?;

        Ok(results)
    }
}
```

## Implementation Requirements

### Prerequisites

- [ ] TASK-F001 complete (SemanticFingerprint available)
- [ ] TASK-F005 complete (HNSW configuration defined)
- [ ] TASK-F004 complete (Storage schema for persistence)

### Scope

#### In Scope

- **13 HNSW index instances** (E1-E13)
- Per-space configuration
- Parallel multi-index search
- Index persistence/loading
- Health monitoring
- Incremental updates
- **Matryoshka 128D index** for Stage 2 fast filtering
- **SPLADE inverted index** for Stage 1 sparse retrieval
- 5-stage pipeline index coordination

#### Out of Scope

- Query execution logic (TASK-L001)
- Purpose pattern index (TASK-L006)
- GPU acceleration (future enhancement)

### Constraints

- Memory < 4GB per index (configurable)
- Search latency < 10ms for single space
- Thread-safe for concurrent operations
- Atomic index updates

## Pseudo Code

```
FUNCTION initialize_indexes(configs):
    FOR config IN configs:
        index = create_hnsw_index(
            dimension: config.dimension,
            max_elements: config.max_elements,
            m: config.m,
            ef_construction: config.ef_construction,
            metric: config.distance_metric
        )

        IF config.use_simd:
            index.enable_simd()

        indexes[config.space_index] = index
        status[config.space_index] = IndexStatus {
            space_index: config.space_index,
            is_loaded: true,
            element_count: 0,
            health: Healthy
        }

    RETURN Ok(())

FUNCTION add_fingerprint(memory_id, fingerprint):
    FOR space_idx IN 0..12:
        embedding = fingerprint.embeddings[space_idx]
        IF embedding IS NOT NULL:
            index = indexes[space_idx]
            IF index IS NULL:
                CONTINUE  // Skip uninitialized indexes

            TRY:
                index.add(memory_id, embedding)
                status[space_idx].element_count += 1
            CATCH error:
                status[space_idx].health = Degraded
                LOG_ERROR("Failed to add to index {}: {}", space_idx, error)

    RETURN Ok(())

FUNCTION search_multi(spaces, queries, k):
    // Launch parallel searches
    futures = []
    FOR (space_idx, query) IN zip(spaces, queries):
        index = indexes[space_idx]
        IF index IS NULL:
            CONTINUE

        future = spawn_async {
            result = index.search(query, k)
            (space_idx, result)
        }
        futures.push(future)

    // Collect results
    results = await_all(futures)

    // Return in order
    ordered = []
    FOR space_idx IN spaces:
        FOR (idx, result) IN results:
            IF idx == space_idx:
                ordered.push(result)
                BREAK

    RETURN ordered

FUNCTION persist():
    FOR space_idx IN 0..12:
        IF indexes[space_idx] IS NOT NULL:
            path = storage_path / format!("index_{}.hnsw", space_idx)
            indexes[space_idx].save(path)?

    RETURN Ok(())

FUNCTION load():
    FOR space_idx IN 0..12:
        path = storage_path / format!("index_{}.hnsw", space_idx)
        IF path.exists():
            TRY:
                indexes[space_idx] = HnswIndex::load(path)?
                status[space_idx].is_loaded = true
                status[space_idx].element_count = indexes[space_idx].len()
            CATCH:
                status[space_idx].health = Failed

    RETURN Ok(())
```

## Definition of Done

### Implementation Checklist

- [ ] `HnswIndexConfig` struct with per-space defaults
- [ ] `SPACE_CONFIGS` constant with **13 configurations** (E1-E13)
- [ ] `IndexStatus` and `IndexHealth` types
- [ ] `MultiSpaceIndexManager` trait
- [ ] HNSW-based implementation (13x per-space)
- [ ] Parallel multi-index search
- [ ] Index persistence to disk
- [ ] Index loading from disk
- [ ] Health monitoring
- [ ] **`MATRYOSHKA_128D_CONFIG`** for Stage 2 index
- [ ] **`SpladeIndexConfig`** for Stage 1 inverted index
- [ ] **`SpladeInvertedIndex`** with posting lists
- [ ] **`search_splade()`** for Stage 1 sparse retrieval
- [ ] **`search_matryoshka_128d()`** for Stage 2 fast filtering
- [ ] **`add_matryoshka()`** with 128D truncation
- [ ] **`add_splade()`** for inverted index updates

### Testing Requirements

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize_all_indexes() {
        let mut manager = HnswMultiSpaceIndex::new(temp_dir());
        manager.initialize(&SPACE_CONFIGS).await.unwrap();
        manager.initialize_matryoshka(&MATRYOSHKA_128D_CONFIG).await.unwrap();
        manager.initialize_splade_inverted(&SpladeIndexConfig::default()).await.unwrap();

        let status = manager.status();
        assert_eq!(status.len(), 13);  // 13 per-space indexes
        for s in status {
            assert!(s.is_loaded);
            assert_eq!(s.health, IndexHealth::Healthy);
        }
    }

    #[tokio::test]
    async fn test_matryoshka_128d_search() {
        let mut manager = create_test_manager().await;
        add_test_data(&mut manager, 100).await;

        // Matryoshka uses truncated 128D vectors
        let query_128d = vec![0.1; 128];
        let results = manager.search_matryoshka_128d(&query_128d, 10).await.unwrap();

        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_splade_inverted_search() {
        let mut manager = create_test_manager().await;
        add_test_data(&mut manager, 100).await;

        // SPLADE uses sparse (term_id, weight) pairs
        let sparse_query = vec![(100, 0.8), (500, 0.6), (1000, 0.4)];
        let results = manager.search_splade(&sparse_query, 10).await.unwrap();

        assert!(results.len() <= 10);
    }

    #[tokio::test]
    async fn test_add_fingerprint() {
        let mut manager = create_test_manager().await;
        let fingerprint = create_test_fingerprint(); // All 12 embeddings
        let memory_id = MemoryId::new();

        manager.add_fingerprint(memory_id.clone(), &fingerprint).await.unwrap();

        // All indexes should have 1 element
        for status in manager.status() {
            assert_eq!(status.element_count, 1);
        }
    }

    #[tokio::test]
    async fn test_search_single_space() {
        let mut manager = create_test_manager().await;
        add_test_data(&mut manager, 100).await;

        let query = vec![0.1; SPACE_CONFIGS[0].dimension];
        let results = manager.search(0, &query, 10, None).await.unwrap();

        assert_eq!(results.len(), 10);
        // Results should be sorted by similarity
        for i in 1..results.len() {
            assert!(results[i-1].1 >= results[i].1);
        }
    }

    #[tokio::test]
    async fn test_search_multi_parallel() {
        let mut manager = create_test_manager().await;
        add_test_data(&mut manager, 100).await;

        let spaces = vec![0, 1, 2];
        let queries = vec![
            vec![0.1; SPACE_CONFIGS[0].dimension],
            vec![0.1; SPACE_CONFIGS[1].dimension],
            vec![0.1; SPACE_CONFIGS[2].dimension],
        ];

        let results = manager.search_multi(&spaces, &queries, 10).await.unwrap();

        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_persist_and_load() {
        let dir = temp_dir();
        let mut manager = HnswMultiSpaceIndex::new(dir.clone());
        manager.initialize(&SPACE_CONFIGS).await.unwrap();
        add_test_data(&mut manager, 100).await;

        manager.persist().await.unwrap();

        let mut manager2 = HnswMultiSpaceIndex::new(dir);
        manager2.load().await.unwrap();

        for (s1, s2) in manager.status().iter().zip(manager2.status().iter()) {
            assert_eq!(s1.element_count, s2.element_count);
        }
    }
}
```

### Verification Commands

```bash
# Run unit tests
cargo test -p context-graph-core hnsw_index

# Run with larger dataset
cargo test -p context-graph-core hnsw_index --features large-tests

# Benchmark search performance
cargo bench -p context-graph-core -- hnsw_search

# Memory usage test
cargo test -p context-graph-core hnsw_memory -- --nocapture
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/index/mod.rs` | Index module |
| `crates/context-graph-core/src/index/config.rs` | HnswIndexConfig and SPACE_CONFIGS |
| `crates/context-graph-core/src/index/manager.rs` | MultiSpaceIndexManager trait |
| `crates/context-graph-core/src/index/hnsw_impl.rs` | HNSW implementation |
| `crates/context-graph-core/src/index/status.rs` | IndexStatus and health |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/lib.rs` | Add `pub mod index` |
| `crates/context-graph-core/Cargo.toml` | Add hnswlib dependency |

## Traceability

| Requirement | Source | Coverage |
|-------------|--------|----------|
| Per-embedder indexes | projectionplan1.md:per-embedder | Complete |
| HNSW configuration | projectionplan2.md:hnsw | Complete |
| Parallel search | projectionplan1.md:performance | Complete |
| Index persistence | projectionplan2.md:storage | Complete |
| **13x per-space HNSW** | projectionplan2.md:13-spaces | Complete |
| **Matryoshka 128D Stage 2** | projectionplan2.md:matryoshka | Complete |
| **SPLADE inverted Stage 1** | projectionplan2.md:splade | Complete |

---

*Task created: 2026-01-04*
*Layer: Logic*
*Priority: P1 - Core indexing infrastructure*
