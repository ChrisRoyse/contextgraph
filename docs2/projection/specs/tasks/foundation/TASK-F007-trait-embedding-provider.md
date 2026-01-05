# Task: TASK-F007 - Modify EmbeddingProvider Trait for Multi-Array Output

## Metadata
- **ID**: TASK-F007
- **Layer**: Foundation
- **Priority**: P0 (Critical Path)
- **Estimated Effort**: M (Medium)
- **Dependencies**: TASK-F001, TASK-F006
- **Traces To**: TS-301, FR-101, FR-102, FR-104

## Description

Modify the `EmbeddingProvider` trait to return `SemanticFingerprint` with all 13 embeddings instead of a single `Vec<f32>`. This is a fundamental change that enables the multi-array architecture.

Current trait returns `EmbeddingOutput` with single vector. New trait returns `MultiArrayEmbeddingOutput` with complete `SemanticFingerprint`.

**Key Change**: `embed()` returns 13 embeddings, not 1.

**13 Embedders (E1-E13)**:
- E1-E12: Original dense embeddings from various models
- E13: SPLADE v3 sparse embeddings for Stage 1 recall in 5-stage pipeline

## Acceptance Criteria

- [ ] `MultiArrayEmbeddingOutput` struct with SemanticFingerprint (13 embedders)
- [ ] Per-embedder latency tracking (for optimization, 13 embedders)
- [ ] `MultiArrayEmbeddingProvider` trait replacing legacy `EmbeddingProvider`
- [ ] `embed_all()` method returns complete fingerprint (13 embeddings)
- [ ] `embed_batch_all()` for batch processing
- [ ] `dimensions()` returns array of 13 dimensions
- [ ] `SingleEmbedder` trait for composing multi-array provider
- [ ] `SparseEmbedder` trait for E6 and E13 SPLADE sparse embeddings
- [ ] Performance target check method (30ms for all 13)
- [ ] Legacy `EmbeddingProvider` kept but marked deprecated (for gradual migration)
- [ ] Unit tests for new trait structure

## Implementation Steps

1. Read existing `crates/context-graph-core/src/traits/embedding_provider.rs`
2. Create `crates/context-graph-core/src/traits/multi_array_embedding.rs`:
   - Import SemanticFingerprint from fingerprint module
   - Implement MultiArrayEmbeddingOutput struct
   - Implement MultiArrayEmbeddingProvider trait
   - Implement SingleEmbedder trait
3. Update `crates/context-graph-core/src/traits/mod.rs`:
   - Add `pub mod multi_array_embedding;`
   - Export new types
4. Add `#[deprecated]` attribute to old EmbeddingProvider (optional, for gradual migration)
5. Update stub implementation in `crates/context-graph-core/src/stubs/embedding_stub.rs`

## Files Affected

### Files to Create
- `crates/context-graph-core/src/traits/multi_array_embedding.rs` - New trait definition

### Files to Modify
- `crates/context-graph-core/src/traits/mod.rs` - Export new module
- `crates/context-graph-core/src/traits/embedding_provider.rs` - Add deprecation note
- `crates/context-graph-core/src/stubs/embedding_stub.rs` - Implement new trait

## Code Signature (Definition of Done)

```rust
// multi_array_embedding.rs
use crate::error::CoreResult;
use crate::types::fingerprint::SemanticFingerprint;
use async_trait::async_trait;
use std::time::Duration;

/// Number of embedders in the multi-array architecture (E1-E13).
pub const NUM_EMBEDDERS: usize = 13;

/// Output from multi-array embedding generation
#[derive(Debug, Clone)]
pub struct MultiArrayEmbeddingOutput {
    /// The complete 13-embedding fingerprint (E1-E13)
    pub fingerprint: SemanticFingerprint,

    /// Total latency for all 13 embeddings
    pub total_latency: Duration,

    /// Per-embedder latencies (for optimization)
    pub per_embedder_latency: [Duration; NUM_EMBEDDERS],

    /// Per-embedder model IDs
    pub model_ids: [String; NUM_EMBEDDERS],
}

impl MultiArrayEmbeddingOutput {
    /// Expected total latency target: <30ms for all 13 embedders
    pub const TARGET_LATENCY_MS: u64 = 30;

    /// Check if latency is within target
    pub fn is_within_latency_target(&self) -> bool;

    /// Get E1 Matryoshka 128D truncated embedding for fast Stage 2 filtering
    pub fn e1_matryoshka_128(&self) -> &[f32];
}

/// Multi-Array Embedding Provider trait
///
/// REPLACES the legacy EmbeddingProvider that returned Vec<f32>.
/// Returns complete SemanticFingerprint with all 13 embeddings (E1-E13).
///
/// NO FUSION - each embedder output stored independently.
#[async_trait]
pub trait MultiArrayEmbeddingProvider: Send + Sync {
    /// Generate complete 13-embedding fingerprint for content
    ///
    /// # Performance Target
    /// - Single content: <30ms for all 13 embeddings
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput>;

    /// Generate fingerprints for multiple contents in batch
    ///
    /// # Performance Target
    /// - 64 contents: <100ms for all 13 embeddings per content
    async fn embed_batch_all(&self, contents: &[String]) -> CoreResult<Vec<MultiArrayEmbeddingOutput>>;

    /// Get expected dimensions for each embedder
    /// E6 and E13 return 0 (sparse is variable)
    fn dimensions(&self) -> [usize; NUM_EMBEDDERS] {
        [1024, 512, 512, 512, 768, 0, 1536, 384, 1024, 768, 384, 128, 0]
    }

    /// Get model IDs for each embedder
    fn model_ids(&self) -> [&str; NUM_EMBEDDERS];

    /// Check if all embedders are ready
    fn is_ready(&self) -> bool;

    /// Get health status per embedder
    fn health_status(&self) -> [bool; NUM_EMBEDDERS];
}

/// Individual embedder trait for composing MultiArrayEmbeddingProvider
/// Used for dense embedders (E1-E5, E7-E12)
#[async_trait]
pub trait SingleEmbedder: Send + Sync {
    /// Embedding dimension for this embedder
    fn dimension(&self) -> usize;

    /// Model identifier
    fn model_id(&self) -> &str;

    /// Generate single embedding
    async fn embed(&self, content: &str) -> CoreResult<Vec<f32>>;

    /// Check if ready
    fn is_ready(&self) -> bool;
}

/// Sparse embedder trait for SPLADE-style embeddings
/// Used for sparse embedders (E6, E13)
#[async_trait]
pub trait SparseEmbedder: Send + Sync {
    /// Vocabulary size for sparse vectors
    fn vocab_size(&self) -> usize;

    /// Model identifier
    fn model_id(&self) -> &str;

    /// Generate sparse embedding (indices and values)
    async fn embed_sparse(&self, content: &str) -> CoreResult<SparseVector>;

    /// Check if ready
    fn is_ready(&self) -> bool;
}
```

## Testing Requirements

### Unit Tests
- `test_multi_array_output_latency_check` - Returns true when under 30ms
- `test_multi_array_output_latency_exceeded` - Returns false when over 30ms
- `test_dimensions_default` - Returns correct 13 dimensions (E6=0, E13=0 for sparse)
- `test_trait_object_safety` - Can create `dyn MultiArrayEmbeddingProvider`
- `test_e1_matryoshka_128_truncation` - Returns first 128 elements of E1
- `test_sparse_embedder_trait` - Can create `dyn SparseEmbedder`

### Integration Tests
- Test with stub implementation returning valid SemanticFingerprint (13 embedders)
- Test batch processing returns correct count
- Test E13 SPLADE sparse embedding generation

## Verification

```bash
# Compile check
cargo check -p context-graph-core

# Run unit tests
cargo test -p context-graph-core multi_array_embedding

# Verify trait exports
cargo doc -p context-graph-core --open
```

## Constraints

- Performance targets from constitution.yaml:
  - Single embed (all 13): <30ms
  - Batch embed (64 x 13): <100ms
- All methods must be `async` for parallel embedding generation
- `Send + Sync` required for multi-threaded use
- Dimensions array: E6 and E13 return 0 (sparse is variable), E12 returns 128 (per token)
- E1 Matryoshka supports truncation to 128D for fast Stage 2 filtering
- E13 SPLADE must return SparseVector compatible with Stage 1 recall

## Migration Notes

The legacy `EmbeddingProvider` trait is kept for backwards compatibility during migration:
1. New code should use `MultiArrayEmbeddingProvider`
2. Existing implementations can implement both traits
3. Gradual migration path: old trait calls can wrap new trait

However, per FR-602 (No Backwards Compatibility), migration shims should NOT be created. Old code using `EmbeddingProvider` should be updated directly.

## Notes

This trait change is fundamental to the multi-array architecture. All embedding generation now returns 12 vectors, not 1.

The `SingleEmbedder` and `SparseEmbedder` traits allow composition:
```rust
struct CompositeEmbeddingProvider {
    dense_embedders: [Box<dyn SingleEmbedder>; 11], // E1-E5, E7-E12 (skip E6)
    sparse_embedders: [Box<dyn SparseEmbedder>; 2], // E6, E13
}

impl MultiArrayEmbeddingProvider for CompositeEmbeddingProvider {
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        // Call all 13 embedders in parallel
        // Combine results into SemanticFingerprint
        // Store E13 SPLADE in e13_splade field
    }
}
```

**5-Stage Pipeline Integration**:
- Stage 1: E13 SPLADE sparse embedding used for initial recall
- Stage 2: E1 Matryoshka 128D (truncated) for fast dense filtering
- Stage 3: Full E1-E12 dense embeddings for precision ranking
- Stage 4: E12 ColBERT for late interaction reranking
- Stage 5: Purpose vector computed from all 13 embeddings

Reference implementation in TECH-SPEC-001 Section 3.1 (TS-301).
