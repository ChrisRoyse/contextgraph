# Task: TASK-F005 - Implement Per-Embedder HNSW Index Configuration

## Metadata
- **ID**: TASK-F005
- **Layer**: Foundation
- **Priority**: P1 (High)
- **Estimated Effort**: M (Medium)
- **Dependencies**: TASK-F001
- **Traces To**: TS-202, FR-302

## Description

Implement the HNSW (Hierarchical Navigable Small World) index configuration for 13 per-embedder indexes plus one Purpose Vector index. Each embedding space requires a separate index with dimension-appropriate configuration.

Index types:
- **E1-E5, E7-E11**: Standard HNSW with cosine similarity
- **E1 Matryoshka 128D**: Secondary HNSW for fast Stage 2 filtering
- **E6 (Sparse)**: Inverted index (NOT HNSW) - legacy slot
- **E12 (Late-Interaction)**: ColBERT MaxSim (NOT HNSW)
- **E13 (SPLADE)**: Inverted index (NOT HNSW) - for Stage 1 recall
- **Purpose Vector**: 13D HNSW for teleological search

This task defines the configuration; actual index instantiation happens in Logic Layer.

**5-Stage Pipeline Index Support**:
- Stage 1: E13 SPLADE inverted index
- Stage 2: E1 Matryoshka 128D HNSW index
- Stage 3: Full E1-E12 HNSW indexes
- Stage 4: E12 ColBERT late interaction
- Stage 5: 13D Purpose Vector HNSW index

## Acceptance Criteria

- [ ] `EmbedderIndex` enum for all 13 embedders + PurposeVector + E1Matryoshka128
- [ ] `HnswConfig` struct with M, ef_construction, ef_search, dimension
- [ ] `DistanceMetric` enum (Cosine, DotProduct, Euclidean, AsymmetricCosine, Jaccard)
- [ ] `get_hnsw_config(index)` returns appropriate config or None
- [ ] `all_hnsw_configs()` returns map of all HNSW-able indexes (11 + 1 Matryoshka + 1 PurposeVector = 13)
- [ ] `recommended_metric(embedder_idx)` for query planning
- [ ] Documentation of which indexes use HNSW vs alternatives
- [ ] E1 Matryoshka 128D index configuration for fast Stage 2 filtering
- [ ] E13 SPLADE inverted index configuration for Stage 1 recall
- [ ] Unit tests for configuration correctness

## Implementation Steps

1. Create `crates/context-graph-storage/src/teleological/indexes/mod.rs`:
   - Define module structure
2. Create `crates/context-graph-storage/src/teleological/indexes/hnsw_config.rs`:
   - Implement `DistanceMetric` enum
   - Implement `EmbedderIndex` enum
   - Implement `HnswConfig` struct
   - Implement `get_hnsw_config()` function
   - Implement `all_hnsw_configs()` function
3. Create `crates/context-graph-storage/src/teleological/indexes/metrics.rs`:
   - Implement `recommended_metric()` function
   - Document metric selection rationale
4. Update `crates/context-graph-storage/src/teleological/mod.rs` to export indexes

## Files Affected

### Files to Create
- `crates/context-graph-storage/src/teleological/indexes/mod.rs` - Module definition
- `crates/context-graph-storage/src/teleological/indexes/hnsw_config.rs` - HNSW configuration
- `crates/context-graph-storage/src/teleological/indexes/metrics.rs` - Distance metrics

### Files to Modify
- `crates/context-graph-storage/src/teleological/mod.rs` - Export indexes module

## Code Signature (Definition of Done)

```rust
// hnsw_config.rs
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Number of connections per node (M parameter)
    pub m: usize,
    /// Size of dynamic candidate list during construction
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search
    pub ef_search: usize,
    /// Distance metric
    pub metric: DistanceMetric,
    /// Embedding dimension
    pub dimension: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    Cosine,
    DotProduct,
    Euclidean,
    AsymmetricCosine,
    Jaccard,  // For sparse vectors (E6, E13)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmbedderIndex {
    E1TextGeneral,      // 1024D HNSW (Matryoshka: supports 128/256/512/1024)
    E1Matryoshka128,    // 128D HNSW - truncated E1 for fast Stage 2 filtering
    E2TextSmall,        // 512D HNSW
    E3Multilingual,     // 512D HNSW
    E4Code,             // 512D HNSW
    E5QueryDoc,         // 768D x 2 HNSW (asymmetric)
    E6Sparse,           // Inverted index (NOT HNSW) - legacy slot
    E7OpenaiAda,        // 1536D HNSW
    E8Minilm,           // 384D HNSW
    E9Simhash,          // 1024D HNSW
    E10Instructor,      // 768D HNSW
    E11Fast,            // 384D HNSW
    E12TokenLevel,      // ColBERT MaxSim (NOT HNSW)
    E13Splade,          // SPLADE v3 Inverted index (NOT HNSW) - for Stage 1 recall
    PurposeVector,      // 13D HNSW
}

/// Get HNSW config for index type. Returns None for non-HNSW indexes (E6, E12, E13).
pub fn get_hnsw_config(index: EmbedderIndex) -> Option<HnswConfig>;

/// Get all indexes that use HNSW (excludes E6, E12, E13)
/// Returns 13 configs: 10 dense embedders + E1Matryoshka128 + 2 asymmetric E5 + PurposeVector
pub fn all_hnsw_configs() -> HashMap<EmbedderIndex, HnswConfig>;

/// Get inverted index config for sparse embedders (E6, E13)
pub fn get_inverted_index_config(index: EmbedderIndex) -> Option<InvertedIndexConfig>;

/// Configuration for inverted indexes (sparse vectors)
#[derive(Debug, Clone)]
pub struct InvertedIndexConfig {
    /// Vocabulary size for term IDs
    pub vocab_size: usize,
    /// Maximum non-zero entries per vector
    pub max_nnz: usize,
    /// Whether to use BM25 weighting
    pub use_bm25: bool,
}

// metrics.rs
/// Get recommended distance metric for embedder
pub fn recommended_metric(embedder_index: usize) -> DistanceMetric;

/// Compute distance using specified metric
pub fn compute_distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32;

/// Convert distance to similarity [0.0, 1.0]
pub fn distance_to_similarity(distance: f32, metric: DistanceMetric) -> f32;
```

## HNSW Configuration Table

| Index | Dimension | M | ef_construction | ef_search | Metric | Stage |
|-------|-----------|---|-----------------|-----------|--------|-------|
| E1TextGeneral | 1024 | 16 | 200 | 100 | Cosine | 3 |
| E1Matryoshka128 | 128 | 16 | 200 | 100 | Cosine | 2 (fast) |
| E2TextSmall | 512 | 16 | 200 | 100 | Cosine | 3 |
| E3Multilingual | 512 | 16 | 200 | 100 | Cosine | 3 |
| E4Code | 512 | 16 | 200 | 100 | Cosine | 3 |
| E5QueryDoc | 768 | 16 | 200 | 100 | AsymmetricCosine | 3 |
| E6Sparse | N/A | - | - | - | Jaccard (inverted) | - |
| E7OpenaiAda | 1536 | 16 | 200 | 100 | Cosine | 3 |
| E8Minilm | 384 | 16 | 200 | 100 | Cosine | 3 |
| E9Simhash | 1024 | 16 | 200 | 100 | Cosine | 3 |
| E10Instructor | 768 | 16 | 200 | 100 | Cosine | 3 |
| E11Fast | 384 | 16 | 200 | 100 | Cosine | 3 |
| E12TokenLevel | 128/token | - | - | - | MaxSim | 4 (rerank) |
| E13Splade | N/A | - | - | - | Jaccard (inverted) | 1 (recall) |
| PurposeVector | 13 | 16 | 200 | 100 | Cosine | 5 (teleological) |

## Inverted Index Configuration Table

| Index | Vocab Size | Max NNZ | Use BM25 | Stage |
|-------|------------|---------|----------|-------|
| E6Sparse | 30,522 | 1,500 | false | - |
| E13Splade | 30,522 | 1,500 | true | 1 (recall) |

## Testing Requirements

### Unit Tests
- `test_get_hnsw_config_e1` - Returns 1024D config
- `test_get_hnsw_config_e1_matryoshka_128` - Returns 128D config for fast Stage 2
- `test_get_hnsw_config_e6_none` - Returns None (sparse)
- `test_get_hnsw_config_e12_none` - Returns None (late-interaction)
- `test_get_hnsw_config_e13_none` - Returns None (SPLADE inverted)
- `test_get_hnsw_config_purpose` - Returns 13D config
- `test_all_hnsw_configs_count` - Returns 13 configs (excludes E6, E12, E13)
- `test_get_inverted_index_config_e6` - Returns E6 sparse config
- `test_get_inverted_index_config_e13` - Returns E13 SPLADE config with BM25
- `test_recommended_metric_dense` - Cosine for E1-E5, E7-E11
- `test_recommended_metric_sparse` - Jaccard for E6, E13
- `test_recommended_metric_maxsim` - MaxSim for E12
- `test_compute_distance_cosine` - Correct cosine distance
- `test_distance_to_similarity` - Correct conversion

## Verification

```bash
# Compile check
cargo check -p context-graph-storage

# Run unit tests
cargo test -p context-graph-storage indexes
```

## Constraints

- HNSW parameters from constitution.yaml:
  - M = 16 (connections per node)
  - ef_construction = 200 (build quality)
  - ef_search = 100 (search quality)
- E6 uses inverted index for sparse vectors (legacy slot)
- E12 uses ColBERT-style MaxSim aggregation
- E13 uses inverted index with BM25 weighting for Stage 1 recall
- E1 Matryoshka 128D provides fast Stage 2 filtering (128D vs 1024D)
- Purpose vector is 13D (very fast search, updated from 12D)

## Notes

This task defines index configuration only. The actual HNSW index instantiation and management happens in Logic Layer (TASK-L004 or similar).

For E5 (asymmetric), we need TWO indexes:
1. Query index - for indexing query embeddings
2. Document index - for indexing document embeddings

**5-Stage Pipeline Index Mapping**:
| Stage | Index Type | Embedder | Purpose |
|-------|------------|----------|---------|
| 1 (Recall) | Inverted | E13 SPLADE | BM25 + lexical-semantic initial selection |
| 2 (Semantic) | HNSW 128D | E1 Matryoshka 128D | Fast dense filtering (8x faster than 1024D) |
| 3 (Precision) | HNSW full | E1-E12 (dense) | Full-resolution dense ranking |
| 4 (Rerank) | MaxSim | E12 TokenLevel | ColBERT late interaction reranking |
| 5 (Teleological) | HNSW 13D | PurposeVector | Goal alignment filtering |

**E1 Matryoshka Truncation**:
E1 uses Matryoshka representation learning, allowing truncation to 128D, 256D, 512D, or full 1024D.
For Stage 2, we use 128D truncated vectors stored in `idx_e1_matryoshka_128` for 8x faster search
while preserving ~95% of the semantic quality.

Reference implementation in TECH-SPEC-001 Section 2.2 (TS-202).
