# TASK-P2-005: MultiArrayProvider Implementation

```xml
<task_spec id="TASK-P2-005" version="3.0">
<metadata>
  <title>MultiArrayProvider Implementation - Concrete Embedder Orchestration</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <phase>2</phase>
  <implements>
    <requirement_ref>REQ-P2-01</requirement_ref>
    <requirement_ref>REQ-P2-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-P2-001</task_ref>
    <task_ref status="COMPLETE">TASK-P2-002</task_ref>
    <task_ref status="COMPLETE">TASK-P2-003</task_ref>
    <task_ref status="COMPLETE">TASK-P2-003b</task_ref>
    <task_ref status="COMPLETE">TASK-P2-004</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <last_audit>2025-01-16</last_audit>
</metadata>

<critical_audit date="2025-01-16">
  ## CODEBASE STATE VERIFICATION

  ### WHAT ALREADY EXISTS (DO NOT RECREATE):

  **Traits (ALREADY IMPLEMENTED):**
  - `MultiArrayEmbeddingProvider` trait: `crates/context-graph-core/src/traits/multi_array_embedding.rs`
  - `SingleEmbedder` trait: same file
  - `SparseEmbedder` trait: same file
  - `TokenEmbedder` trait: same file
  - `MultiArrayEmbeddingOutput` struct: same file (with latency tracking)

  **Types (ALREADY IMPLEMENTED):**
  - `SemanticFingerprint` (alias `TeleologicalArray`): `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`
  - `ValidationError` enum: same file (lines 53-112)
  - `DenseVector`: `crates/context-graph-core/src/embeddings/vector.rs`
  - `BinaryVector`: same file
  - `SparseVector`: `crates/context-graph-core/src/types/fingerprint/sparse.rs`
  - `Embedder` enum: `crates/context-graph-core/src/teleological/embedder.rs`
  - `EmbedderCategory`: `crates/context-graph-core/src/embeddings/category.rs`
  - `EmbedderConfig` + `EMBEDDER_CONFIGS`: `crates/context-graph-core/src/embeddings/config.rs`
  - `TokenPruningEmbedding`: `crates/context-graph-core/src/embeddings/token_pruning.rs`

  **Errors (ALREADY IMPLEMENTED):**
  - `EmbeddingError` enum: `crates/context-graph-core/src/error/sub_errors.rs` (lines 18-88)
  - `ValidationError`: `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`
  - `CoreResult&lt;T&gt;`: `crates/context-graph-core/src/error/unified.rs`

  **Validation (ALREADY IMPLEMENTED):**
  - `SemanticFingerprint::validate()` - returns `Result&lt;(), ValidationError&gt;`
  - `SemanticFingerprint::validate_all()` - collects all errors
  - Per-embedder validation: validate_e1() through validate_e13()
  - Location: `crates/context-graph-core/src/types/fingerprint/semantic/validation.rs`

  ### WHAT THIS TASK MUST CREATE:

  1. **`StubMultiArrayProvider`** - A concrete implementation of `MultiArrayEmbeddingProvider`
     - Uses deterministic stub embedders (NOT mock - they produce consistent output)
     - Parallel execution via `tokio::join!`
     - Per-embedder and total timeouts
     - Fail-fast error propagation

  2. **Stub Embedder Implementations:**
     - `StubSingleEmbedder` implementing `SingleEmbedder` (E1-E5, E7-E11)
     - `StubSparseEmbedder` implementing `SparseEmbedder` (E6, E13)
     - `StubTokenEmbedder` implementing `TokenEmbedder` (E12)

  ### NAMING CORRECTIONS (CRITICAL):

  | TASK DOCUMENT (WRONG) | ACTUAL CODEBASE |
  |----------------------|-----------------|
  | `E1Semantic` | `Embedder::Semantic` |
  | `E2TempRecent` | `Embedder::TemporalRecent` |
  | `E8Emotional` | `Embedder::Emotional` (BUT field is `e8_graph`) |
  | `E8_Graph` | Enum is `Emotional`, field is `e8_graph` |
  | `EmbedderImpl` trait | `SingleEmbedder`, `SparseEmbedder`, `TokenEmbedder` |
  | `EmbeddingResult` enum | NOT NEEDED - use `MultiArrayEmbeddingOutput` |
  | `embedding/provider.rs` | Create NEW: `embeddings/provider.rs` |
  | `embedding/error.rs` | Already exists in `error/sub_errors.rs` |
  | `DenseVector&lt;N&gt;` generic | `DenseVector` (non-generic, uses Vec&lt;f32&gt;) |

  ### E8 FIELD NAMING DISCREPANCY:

  **CURRENT STATE (DO NOT CHANGE):**
  - Enum variant: `Embedder::Emotional`
  - Struct field: `SemanticFingerprint.e8_graph`
  - Config says: E8 is "Graph (MiniLM for structure)" with category RELATIONAL

  **WHY:** The field name `e8_graph` is legacy. The canonical name in the Embedder enum
  is `Emotional`. Both reference the same E8 embedder. The implementation MUST use
  `Embedder::Emotional` but access `fingerprint.e8_graph` field.

  ### DEPENDENCIES IN Cargo.toml (ALREADY PRESENT):
  - `tokio` (workspace)
  - `async-trait` (workspace)
  - `thiserror` (workspace)
  - All required for this task
</critical_audit>

<context>
Implements a concrete `StubMultiArrayProvider` that orchestrates 13 stub embedders
to produce a complete `SemanticFingerprint`. This provider is used for:
1. Integration testing without GPU/real models
2. Development and debugging
3. Benchmarking parallel execution
4. Verifying the embedding pipeline architecture

The provider uses deterministic stub implementations that produce content-hash-based
embeddings. This is NOT mock data - it's deterministic output useful for testing
that the same input always produces the same fingerprint.

Per constitution.yaml:
  - ARCH-01: "TeleologicalArray is atomic - store all 13 embeddings or nothing"
  - ARCH-05: "All 13 embedders required - missing embedder is fatal error"
  - AP-14: "No .unwrap() in library code"
  - Performance: single_embed &lt; 10ms, batch_embed_64 &lt; 50ms
</context>

<source_of_truth>
  ACTUAL File Paths (verified 2025-01-16):
    EXISTING (read-only):
      - crates/context-graph-core/src/traits/multi_array_embedding.rs (trait definitions)
      - crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs (SemanticFingerprint)
      - crates/context-graph-core/src/types/fingerprint/semantic/validation.rs (validation)
      - crates/context-graph-core/src/types/fingerprint/sparse.rs (SparseVector)
      - crates/context-graph-core/src/embeddings/vector.rs (DenseVector, BinaryVector)
      - crates/context-graph-core/src/embeddings/config.rs (EmbedderConfig, EMBEDDER_CONFIGS)
      - crates/context-graph-core/src/embeddings/category.rs (EmbedderCategory)
      - crates/context-graph-core/src/teleological/embedder.rs (Embedder enum)
      - crates/context-graph-core/src/error/sub_errors.rs (EmbeddingError)
      - crates/context-graph-core/src/error/unified.rs (CoreResult)

    TO CREATE:
      - crates/context-graph-core/src/embeddings/provider.rs (NEW)
      - crates/context-graph-core/src/embeddings/stubs.rs (NEW)

    TO MODIFY:
      - crates/context-graph-core/src/embeddings/mod.rs (add pub mod provider, stubs)

  ACTUAL Type Names:
    - SemanticFingerprint (alias: TeleologicalArray)
    - Embedder::Semantic (E1)
    - Embedder::TemporalRecent (E2)
    - Embedder::TemporalPeriodic (E3)
    - Embedder::TemporalPositional (E4)
    - Embedder::Causal (E5)
    - Embedder::Sparse (E6)
    - Embedder::Code (E7)
    - Embedder::Emotional (E8 - canonical enum name)
    - Embedder::Hdc (E9)
    - Embedder::Multimodal (E10)
    - Embedder::Entity (E11)
    - Embedder::LateInteraction (E12)
    - Embedder::KeywordSplade (E13)

  ACTUAL Field Names in SemanticFingerprint:
    - e1_semantic: Vec&lt;f32&gt;        (1024D)
    - e2_temporal_recent: Vec&lt;f32&gt; (512D)
    - e3_temporal_periodic: Vec&lt;f32&gt; (512D)
    - e4_temporal_positional: Vec&lt;f32&gt; (512D)
    - e5_causal: Vec&lt;f32&gt;          (768D)
    - e6_sparse: SparseVector       (~30522 vocab)
    - e7_code: Vec&lt;f32&gt;            (1536D)
    - e8_graph: Vec&lt;f32&gt;           (384D) - NOTE: field is "graph", enum is "Emotional"
    - e9_hdc: Vec&lt;f32&gt;             (1024D projected dense)
    - e10_multimodal: Vec&lt;f32&gt;     (768D)
    - e11_entity: Vec&lt;f32&gt;         (384D)
    - e12_late_interaction: Vec&lt;Vec&lt;f32&gt;&gt; (128D per token)
    - e13_splade: SparseVector      (~30522 vocab)

  ACTUAL Dimensions (from EMBEDDER_CONFIGS):
    E1:  1024 (dense, cosine)
    E2:  512  (dense, cosine)
    E3:  512  (dense, cosine)
    E4:  512  (dense, cosine)
    E5:  768  (dense, asymmetric cosine)
    E6:  30522 (sparse, jaccard)
    E7:  1536 (dense, cosine)
    E8:  384  (dense, cosine)
    E9:  1024 (dense, cosine) - projected from HDC
    E10: 768  (dense, cosine)
    E11: 384  (dense, cosine)
    E12: 128  (per-token, maxsim)
    E13: 30522 (sparse, jaccard)
</source_of_truth>

<input_context_files>
  <file purpose="trait_definitions" must_read="true">crates/context-graph-core/src/traits/multi_array_embedding.rs</file>
  <file purpose="fingerprint_struct" must_read="true">crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs</file>
  <file purpose="sparse_vector" must_read="true">crates/context-graph-core/src/types/fingerprint/sparse.rs</file>
  <file purpose="embedder_config" must_read="true">crates/context-graph-core/src/embeddings/config.rs</file>
  <file purpose="embedder_enum" must_read="true">crates/context-graph-core/src/teleological/embedder.rs</file>
  <file purpose="error_types" must_read="true">crates/context-graph-core/src/error/sub_errors.rs</file>
  <file purpose="dense_vector">crates/context-graph-core/src/embeddings/vector.rs</file>
  <file purpose="validation">crates/context-graph-core/src/types/fingerprint/semantic/validation.rs</file>
  <file purpose="embeddings_mod">crates/context-graph-core/src/embeddings/mod.rs</file>
</input_context_files>

<prerequisites>
  <check status="VERIFIED">MultiArrayEmbeddingProvider trait exists in traits/multi_array_embedding.rs</check>
  <check status="VERIFIED">SingleEmbedder, SparseEmbedder, TokenEmbedder traits exist</check>
  <check status="VERIFIED">MultiArrayEmbeddingOutput struct exists with latency tracking</check>
  <check status="VERIFIED">SemanticFingerprint struct exists with all 13 fields</check>
  <check status="VERIFIED">SemanticFingerprint::validate() returns Result&lt;(), ValidationError&gt;</check>
  <check status="VERIFIED">EmbeddingError enum exists with ModelNotLoaded, GenerationFailed variants</check>
  <check status="VERIFIED">Embedder enum has all 13 variants</check>
  <check status="VERIFIED">EMBEDDER_CONFIGS array has all 13 configurations</check>
  <check status="VERIFIED">tokio and async-trait dependencies exist in Cargo.toml</check>
</prerequisites>

<scope>
  <in_scope>
    - Create StubMultiArrayProvider implementing MultiArrayEmbeddingProvider
    - Create StubSingleEmbedder implementing SingleEmbedder (for E1-E5, E7-E11)
    - Create StubSparseEmbedder implementing SparseEmbedder (for E6, E13)
    - Create StubTokenEmbedder implementing TokenEmbedder (for E12)
    - Implement embed_all() with tokio::join! parallel execution
    - Implement embed_batch_all() for batch processing
    - Add per-embedder timeout (500ms default)
    - Add total timeout (1000ms default)
    - Validate dimensions before returning via fingerprint.validate()
    - Generate deterministic embeddings from content hash
    - Return proper EmbeddingError on failures
  </in_scope>
  <out_of_scope>
    - Actual GPU/CUDA model implementations
    - Real embedding model loading
    - Model caching/optimization
    - Quantization (TASK-P2-006)
    - Creating new error types (use existing EmbeddingError)
    - Modifying existing traits (they're already complete)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/embeddings/stubs.rs">
      //! Stub embedder implementations for testing.
      //!
      //! These produce deterministic embeddings based on content hash.
      //! NOT for production - use real model implementations.

      use async_trait::async_trait;
      use crate::error::CoreResult;
      use crate::traits::{SingleEmbedder, SparseEmbedder, TokenEmbedder};
      use crate::types::fingerprint::SparseVector;
      use crate::embeddings::get_dimension;
      use crate::teleological::Embedder;

      /// Stub dense embedder for testing.
      pub struct StubSingleEmbedder {
          embedder: Embedder,
          dimension: usize,
      }

      impl StubSingleEmbedder {
          pub fn new(embedder: Embedder) -> Self;
          pub fn for_e1() -> Self;
          pub fn for_e2() -> Self;
          // ... etc for E3-E5, E7-E11
      }

      #[async_trait]
      impl SingleEmbedder for StubSingleEmbedder {
          fn dimension(&amp;self) -> usize;
          fn model_id(&amp;self) -> &amp;str;
          async fn embed(&amp;self, content: &amp;str) -> CoreResult&lt;Vec&lt;f32&gt;&gt;;
          fn is_ready(&amp;self) -> bool;
      }

      /// Stub sparse embedder for E6 and E13.
      pub struct StubSparseEmbedder {
          embedder: Embedder,
          vocab_size: usize,
      }

      #[async_trait]
      impl SparseEmbedder for StubSparseEmbedder {
          fn vocab_size(&amp;self) -> usize;
          fn model_id(&amp;self) -> &amp;str;
          async fn embed_sparse(&amp;self, content: &amp;str) -> CoreResult&lt;SparseVector&gt;;
          fn is_ready(&amp;self) -> bool;
      }

      /// Stub token embedder for E12 ColBERT.
      pub struct StubTokenEmbedder {
          token_dim: usize,
          max_tokens: usize,
      }

      #[async_trait]
      impl TokenEmbedder for StubTokenEmbedder {
          fn token_dimension(&amp;self) -> usize;
          fn max_tokens(&amp;self) -> usize;
          fn model_id(&amp;self) -> &amp;str;
          async fn embed_tokens(&amp;self, content: &amp;str) -> CoreResult&lt;Vec&lt;Vec&lt;f32&gt;&gt;&gt;;
          fn is_ready(&amp;self) -> bool;
      }
    </signature>

    <signature file="crates/context-graph-core/src/embeddings/provider.rs">
      //! StubMultiArrayProvider - concrete implementation for testing.

      use std::sync::Arc;
      use std::time::Duration;
      use async_trait::async_trait;

      use crate::error::CoreResult;
      use crate::traits::{
          MultiArrayEmbeddingProvider, MultiArrayEmbeddingOutput,
          SingleEmbedder, SparseEmbedder, TokenEmbedder,
      };
      use crate::types::fingerprint::{SemanticFingerprint, NUM_EMBEDDERS};

      /// Stub provider using deterministic embedders for testing.
      pub struct StubMultiArrayProvider {
          // 10 dense embedders (E1-E5, E7-E11)
          dense_embedders: [Arc&lt;dyn SingleEmbedder&gt;; 10],
          // 2 sparse embedders (E6, E13)
          sparse_embedders: [Arc&lt;dyn SparseEmbedder&gt;; 2],
          // 1 token embedder (E12)
          token_embedder: Arc&lt;dyn TokenEmbedder&gt;,
          timeout_per_embedder: Duration,
          timeout_total: Duration,
      }

      impl StubMultiArrayProvider {
          /// Create provider with default stub embedders.
          pub fn new() -> Self;

          /// Create with custom timeouts.
          pub fn with_timeouts(
              timeout_per_embedder: Duration,
              timeout_total: Duration,
          ) -> Self;

          /// Default per-embedder timeout (500ms).
          pub const DEFAULT_EMBEDDER_TIMEOUT: Duration = Duration::from_millis(500);

          /// Default total timeout (1000ms).
          pub const DEFAULT_TOTAL_TIMEOUT: Duration = Duration::from_millis(1000);
      }

      #[async_trait]
      impl MultiArrayEmbeddingProvider for StubMultiArrayProvider {
          async fn embed_all(&amp;self, content: &amp;str) -> CoreResult&lt;MultiArrayEmbeddingOutput&gt;;
          async fn embed_batch_all(&amp;self, contents: &amp;[String]) -> CoreResult&lt;Vec&lt;MultiArrayEmbeddingOutput&gt;&gt;;
          fn model_ids(&amp;self) -> [&amp;str; NUM_EMBEDDERS];
          fn is_ready(&amp;self) -> bool;
          fn health_status(&amp;self) -> [bool; NUM_EMBEDDERS];
      }
    </signature>
  </signatures>

  <constraints>
    - All 13 embeddings computed in parallel via tokio::join!
    - Timeout per embedder: 500ms (configurable)
    - Total timeout: 1000ms (configurable)
    - ANY embedder failure = overall failure (fail fast, no partial results)
    - fingerprint.validate() called before returning
    - Empty content returns embeddings with zeros (not an error)
    - Deterministic: same content always produces same embeddings
    - Use existing error types from error/sub_errors.rs
    - Access e8 via fingerprint.e8_graph (field name) but report Embedder::Emotional (enum)
  </constraints>

  <verification>
    - StubMultiArrayProvider::new() creates working provider
    - embed_all("test content") returns valid SemanticFingerprint
    - embed_all("") returns zeroed fingerprint (validates successfully)
    - Same content produces identical fingerprints (determinism)
    - Different content produces different fingerprints
    - Timeout triggers EmbeddingError with appropriate context
    - All 13 embeddings have correct dimensions
    - Parallel execution faster than sequential
  </verification>
</definition_of_done>

<implementation_guide>
## Step 1: Read Required Files (MANDATORY)

Read these files to understand existing interfaces:
```bash
# Trait definitions - understand what you're implementing
cat crates/context-graph-core/src/traits/multi_array_embedding.rs

# SemanticFingerprint - understand the target struct
cat crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs

# SparseVector - understand sparse format
cat crates/context-graph-core/src/types/fingerprint/sparse.rs

# EmbedderConfig - get dimensions from config
cat crates/context-graph-core/src/embeddings/config.rs

# Embedder enum - canonical names
cat crates/context-graph-core/src/teleological/embedder.rs

# Error types - use existing errors
cat crates/context-graph-core/src/error/sub_errors.rs
```

## Step 2: Create stubs.rs

Create `crates/context-graph-core/src/embeddings/stubs.rs`:

```rust
//! Stub embedder implementations for testing.
//!
//! These produce deterministic embeddings based on content hash.
//! Same input always produces same output - useful for testing.

use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::embeddings::{get_dimension, EMBEDDER_CONFIGS};
use crate::error::{ContextGraphError, CoreResult, EmbeddingError};
use crate::teleological::Embedder;
use crate::traits::{SingleEmbedder, SparseEmbedder, TokenEmbedder};
use crate::types::fingerprint::SparseVector;

/// Generate deterministic hash from content.
fn content_hash(content: &amp;str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&amp;hasher);
    hasher.finish()
}

/// Generate deterministic f32 from hash and index.
fn hash_to_f32(hash: u64, index: usize) -> f32 {
    let combined = hash.wrapping_add(index as u64);
    // Map to [-0.5, 0.5] range
    ((combined as f64 / u64::MAX as f64) - 0.5) as f32
}

// ============================================================================
// StubSingleEmbedder
// ============================================================================

pub struct StubSingleEmbedder {
    embedder: Embedder,
    dimension: usize,
    model_id: String,
}

impl StubSingleEmbedder {
    pub fn new(embedder: Embedder) -> Self {
        let config = &amp;EMBEDDER_CONFIGS[embedder as usize];
        Self {
            embedder,
            dimension: config.dimension,
            model_id: format!("stub-{:?}", embedder).to_lowercase(),
        }
    }

    // Factory methods for each dense embedder
    pub fn for_e1() -> Self { Self::new(Embedder::Semantic) }
    pub fn for_e2() -> Self { Self::new(Embedder::TemporalRecent) }
    pub fn for_e3() -> Self { Self::new(Embedder::TemporalPeriodic) }
    pub fn for_e4() -> Self { Self::new(Embedder::TemporalPositional) }
    pub fn for_e5() -> Self { Self::new(Embedder::Causal) }
    pub fn for_e7() -> Self { Self::new(Embedder::Code) }
    pub fn for_e8() -> Self { Self::new(Embedder::Emotional) }
    pub fn for_e9() -> Self { Self::new(Embedder::Hdc) }
    pub fn for_e10() -> Self { Self::new(Embedder::Multimodal) }
    pub fn for_e11() -> Self { Self::new(Embedder::Entity) }
}

#[async_trait]
impl SingleEmbedder for StubSingleEmbedder {
    fn dimension(&amp;self) -> usize {
        self.dimension
    }

    fn model_id(&amp;self) -> &amp;str {
        &amp;self.model_id
    }

    async fn embed(&amp;self, content: &amp;str) -> CoreResult&lt;Vec&lt;f32&gt;&gt; {
        let hash = content_hash(content);
        let embedding: Vec&lt;f32&gt; = (0..self.dimension)
            .map(|i| hash_to_f32(hash, i))
            .collect();
        Ok(embedding)
    }

    fn is_ready(&amp;self) -> bool {
        true
    }
}

// ============================================================================
// StubSparseEmbedder
// ============================================================================

pub struct StubSparseEmbedder {
    embedder: Embedder,
    vocab_size: usize,
    model_id: String,
}

impl StubSparseEmbedder {
    pub fn new(embedder: Embedder) -> Self {
        let vocab_size = match embedder {
            Embedder::Sparse | Embedder::KeywordSplade => 30_522,
            _ => 30_522, // Default to BERT vocab
        };
        Self {
            embedder,
            vocab_size,
            model_id: format!("stub-{:?}", embedder).to_lowercase(),
        }
    }

    pub fn for_e6() -> Self { Self::new(Embedder::Sparse) }
    pub fn for_e13() -> Self { Self::new(Embedder::KeywordSplade) }
}

#[async_trait]
impl SparseEmbedder for StubSparseEmbedder {
    fn vocab_size(&amp;self) -> usize {
        self.vocab_size
    }

    fn model_id(&amp;self) -> &amp;str {
        &amp;self.model_id
    }

    async fn embed_sparse(&amp;self, content: &amp;str) -> CoreResult&lt;SparseVector&gt; {
        if content.is_empty() {
            return Ok(SparseVector::empty());
        }

        let hash = content_hash(content);
        // Generate ~5% active indices (typical SPLADE sparsity)
        let num_active = (content.len() % 100).max(5).min(1500);

        let mut indices: Vec&lt;u16&gt; = Vec::with_capacity(num_active);
        let mut values: Vec&lt;f32&gt; = Vec::with_capacity(num_active);

        for i in 0..num_active {
            let idx = ((hash.wrapping_add(i as u64 * 7919) as usize) % self.vocab_size) as u16;
            let val = (hash_to_f32(hash, i) + 0.5).abs(); // Positive activation
            indices.push(idx);
            values.push(val);
        }

        // Sort indices (required by SparseVector)
        let mut pairs: Vec&lt;_&gt; = indices.into_iter().zip(values).collect();
        pairs.sort_by_key(|(idx, _)| *idx);
        pairs.dedup_by_key(|(idx, _)| *idx);

        let (indices, values): (Vec&lt;_&gt;, Vec&lt;_&gt;) = pairs.into_iter().unzip();

        SparseVector::new(indices, values)
            .map_err(|e| ContextGraphError::Embedding(
                EmbeddingError::GenerationFailed {
                    embedder: self.embedder,
                    reason: format!("Sparse vector creation failed: {}", e),
                }
            ))
    }

    fn is_ready(&amp;self) -> bool {
        true
    }
}

// ============================================================================
// StubTokenEmbedder
// ============================================================================

pub struct StubTokenEmbedder {
    token_dim: usize,
    max_tokens: usize,
    model_id: String,
}

impl StubTokenEmbedder {
    pub fn new() -> Self {
        Self {
            token_dim: 128,
            max_tokens: 512,
            model_id: "stub-colbert".to_string(),
        }
    }
}

impl Default for StubTokenEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenEmbedder for StubTokenEmbedder {
    fn token_dimension(&amp;self) -> usize {
        self.token_dim
    }

    fn max_tokens(&amp;self) -> usize {
        self.max_tokens
    }

    fn model_id(&amp;self) -> &amp;str {
        &amp;self.model_id
    }

    async fn embed_tokens(&amp;self, content: &amp;str) -> CoreResult&lt;Vec&lt;Vec&lt;f32&gt;&gt;&gt; {
        if content.is_empty() {
            return Ok(Vec::new());
        }

        let hash = content_hash(content);
        // Simulate tokenization: ~1.3 tokens per word
        let num_tokens = content.split_whitespace().count().max(1).min(self.max_tokens);

        let token_embeddings: Vec&lt;Vec&lt;f32&gt;&gt; = (0..num_tokens)
            .map(|t| {
                (0..self.token_dim)
                    .map(|d| hash_to_f32(hash.wrapping_add(t as u64 * 1000), d))
                    .collect()
            })
            .collect();

        Ok(token_embeddings)
    }

    fn is_ready(&amp;self) -> bool {
        true
    }
}
```

## Step 3: Create provider.rs

Create `crates/context-graph-core/src/embeddings/provider.rs`:

```rust
//! StubMultiArrayProvider - concrete implementation for testing.
//!
//! Orchestrates 13 stub embedders to produce complete SemanticFingerprint.

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use tokio::time::timeout;

use crate::error::{ContextGraphError, CoreResult, EmbeddingError};
use crate::teleological::Embedder;
use crate::traits::{
    MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider,
    SingleEmbedder, SparseEmbedder, TokenEmbedder,
};
use crate::types::fingerprint::{SemanticFingerprint, NUM_EMBEDDERS};

use super::stubs::{StubSingleEmbedder, StubSparseEmbedder, StubTokenEmbedder};

/// Stub provider using deterministic embedders for testing.
pub struct StubMultiArrayProvider {
    // Dense embedders: E1, E2, E3, E4, E5, E7, E8, E9, E10, E11 (indices 0-9)
    dense_embedders: [Arc&lt;dyn SingleEmbedder&gt;; 10],
    // Sparse embedders: E6, E13 (indices 0-1)
    sparse_embedders: [Arc&lt;dyn SparseEmbedder&gt;; 2],
    // Token embedder: E12
    token_embedder: Arc&lt;dyn TokenEmbedder&gt;,
    timeout_per_embedder: Duration,
    timeout_total: Duration,
}

impl StubMultiArrayProvider {
    /// Default per-embedder timeout (500ms).
    pub const DEFAULT_EMBEDDER_TIMEOUT: Duration = Duration::from_millis(500);

    /// Default total timeout (1000ms).
    pub const DEFAULT_TOTAL_TIMEOUT: Duration = Duration::from_millis(1000);

    /// Create provider with default stub embedders.
    pub fn new() -> Self {
        Self::with_timeouts(Self::DEFAULT_EMBEDDER_TIMEOUT, Self::DEFAULT_TOTAL_TIMEOUT)
    }

    /// Create with custom timeouts.
    pub fn with_timeouts(timeout_per_embedder: Duration, timeout_total: Duration) -> Self {
        // Dense embedders in order: E1, E2, E3, E4, E5, E7, E8, E9, E10, E11
        let dense_embedders: [Arc&lt;dyn SingleEmbedder&gt;; 10] = [
            Arc::new(StubSingleEmbedder::for_e1()),   // 0: E1 Semantic
            Arc::new(StubSingleEmbedder::for_e2()),   // 1: E2 TemporalRecent
            Arc::new(StubSingleEmbedder::for_e3()),   // 2: E3 TemporalPeriodic
            Arc::new(StubSingleEmbedder::for_e4()),   // 3: E4 TemporalPositional
            Arc::new(StubSingleEmbedder::for_e5()),   // 4: E5 Causal
            Arc::new(StubSingleEmbedder::for_e7()),   // 5: E7 Code
            Arc::new(StubSingleEmbedder::for_e8()),   // 6: E8 Emotional (field: e8_graph)
            Arc::new(StubSingleEmbedder::for_e9()),   // 7: E9 HDC
            Arc::new(StubSingleEmbedder::for_e10()),  // 8: E10 Multimodal
            Arc::new(StubSingleEmbedder::for_e11()),  // 9: E11 Entity
        ];

        let sparse_embedders: [Arc&lt;dyn SparseEmbedder&gt;; 2] = [
            Arc::new(StubSparseEmbedder::for_e6()),   // 0: E6 Sparse
            Arc::new(StubSparseEmbedder::for_e13()),  // 1: E13 KeywordSplade
        ];

        let token_embedder: Arc&lt;dyn TokenEmbedder&gt; = Arc::new(StubTokenEmbedder::new());

        Self {
            dense_embedders,
            sparse_embedders,
            token_embedder,
            timeout_per_embedder,
            timeout_total,
        }
    }

    /// Execute all embedders in parallel and assemble SemanticFingerprint.
    async fn embed_all_inner(&amp;self, content: &amp;str) -> CoreResult&lt;(SemanticFingerprint, [Duration; NUM_EMBEDDERS])&gt; {
        let per_timeout = self.timeout_per_embedder;

        // Run all 13 embedders in parallel
        let (
            e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13,
            t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13,
        ) = tokio::join!(
            // Dense embedders
            Self::timed_embed(&amp;*self.dense_embedders[0], content, per_timeout, Embedder::Semantic),
            Self::timed_embed(&amp;*self.dense_embedders[1], content, per_timeout, Embedder::TemporalRecent),
            Self::timed_embed(&amp;*self.dense_embedders[2], content, per_timeout, Embedder::TemporalPeriodic),
            Self::timed_embed(&amp;*self.dense_embedders[3], content, per_timeout, Embedder::TemporalPositional),
            Self::timed_embed(&amp;*self.dense_embedders[4], content, per_timeout, Embedder::Causal),
            Self::timed_sparse(&amp;*self.sparse_embedders[0], content, per_timeout, Embedder::Sparse),
            Self::timed_embed(&amp;*self.dense_embedders[5], content, per_timeout, Embedder::Code),
            Self::timed_embed(&amp;*self.dense_embedders[6], content, per_timeout, Embedder::Emotional),
            Self::timed_embed(&amp;*self.dense_embedders[7], content, per_timeout, Embedder::Hdc),
            Self::timed_embed(&amp;*self.dense_embedders[8], content, per_timeout, Embedder::Multimodal),
            Self::timed_embed(&amp;*self.dense_embedders[9], content, per_timeout, Embedder::Entity),
            Self::timed_tokens(&amp;*self.token_embedder, content, per_timeout),
            Self::timed_sparse(&amp;*self.sparse_embedders[1], content, per_timeout, Embedder::KeywordSplade),
            // Timing futures (just return duration, no actual work)
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
            async { Instant::now() },
        );

        // Fail fast on any error
        let fingerprint = SemanticFingerprint {
            e1_semantic: e1?,
            e2_temporal_recent: e2?,
            e3_temporal_periodic: e3?,
            e4_temporal_positional: e4?,
            e5_causal: e5?,
            e6_sparse: e6?,
            e7_code: e7?,
            e8_graph: e8?,  // NOTE: field is e8_graph, embedder is Emotional
            e9_hdc: e9?,
            e10_multimodal: e10?,
            e11_entity: e11?,
            e12_late_interaction: e12?,
            e13_splade: e13?,
        };

        // Validate before returning
        fingerprint.validate().map_err(|e| {
            ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                embedder: Embedder::Semantic, // First embedder for context
                reason: format!("Validation failed: {}", e),
            })
        })?;

        // Compute latencies (placeholder - actual timing would be more sophisticated)
        let latencies = [Duration::ZERO; NUM_EMBEDDERS];

        Ok((fingerprint, latencies))
    }

    async fn timed_embed(
        embedder: &amp;dyn SingleEmbedder,
        content: &amp;str,
        per_timeout: Duration,
        which: Embedder,
    ) -> CoreResult&lt;Vec&lt;f32&gt;&gt; {
        timeout(per_timeout, embedder.embed(content))
            .await
            .map_err(|_| ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                embedder: which,
                reason: format!("Timeout after {:?}", per_timeout),
            }))?
    }

    async fn timed_sparse(
        embedder: &amp;dyn SparseEmbedder,
        content: &amp;str,
        per_timeout: Duration,
        which: Embedder,
    ) -> CoreResult&lt;crate::types::fingerprint::SparseVector&gt; {
        timeout(per_timeout, embedder.embed_sparse(content))
            .await
            .map_err(|_| ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                embedder: which,
                reason: format!("Timeout after {:?}", per_timeout),
            }))?
    }

    async fn timed_tokens(
        embedder: &amp;dyn TokenEmbedder,
        content: &amp;str,
        per_timeout: Duration,
    ) -> CoreResult&lt;Vec&lt;Vec&lt;f32&gt;&gt;&gt; {
        timeout(per_timeout, embedder.embed_tokens(content))
            .await
            .map_err(|_| ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                embedder: Embedder::LateInteraction,
                reason: format!("Timeout after {:?}", per_timeout),
            }))?
    }
}

impl Default for StubMultiArrayProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MultiArrayEmbeddingProvider for StubMultiArrayProvider {
    async fn embed_all(&amp;self, content: &amp;str) -> CoreResult&lt;MultiArrayEmbeddingOutput&gt; {
        let start = Instant::now();

        // Overall timeout
        let result = timeout(self.timeout_total, self.embed_all_inner(content))
            .await
            .map_err(|_| ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                embedder: Embedder::Semantic,
                reason: format!("Total timeout exceeded: {:?}", self.timeout_total),
            }))??;

        let (fingerprint, per_embedder_latency) = result;

        Ok(MultiArrayEmbeddingOutput {
            fingerprint,
            total_latency: start.elapsed(),
            per_embedder_latency,
            model_ids: self.model_ids().map(|s| s.to_string()),
        })
    }

    async fn embed_batch_all(
        &amp;self,
        contents: &amp;[String],
    ) -> CoreResult&lt;Vec&lt;MultiArrayEmbeddingOutput&gt;&gt; {
        let mut results = Vec::with_capacity(contents.len());
        for content in contents {
            results.push(self.embed_all(content).await?);
        }
        Ok(results)
    }

    fn model_ids(&amp;self) -> [&amp;str; NUM_EMBEDDERS] {
        [
            self.dense_embedders[0].model_id(),   // E1
            self.dense_embedders[1].model_id(),   // E2
            self.dense_embedders[2].model_id(),   // E3
            self.dense_embedders[3].model_id(),   // E4
            self.dense_embedders[4].model_id(),   // E5
            self.sparse_embedders[0].model_id(),  // E6
            self.dense_embedders[5].model_id(),   // E7
            self.dense_embedders[6].model_id(),   // E8
            self.dense_embedders[7].model_id(),   // E9
            self.dense_embedders[8].model_id(),   // E10
            self.dense_embedders[9].model_id(),   // E11
            self.token_embedder.model_id(),       // E12
            self.sparse_embedders[1].model_id(),  // E13
        ]
    }

    fn is_ready(&amp;self) -> bool {
        self.dense_embedders.iter().all(|e| e.is_ready())
            &amp;&amp; self.sparse_embedders.iter().all(|e| e.is_ready())
            &amp;&amp; self.token_embedder.is_ready()
    }

    fn health_status(&amp;self) -> [bool; NUM_EMBEDDERS] {
        [
            self.dense_embedders[0].is_ready(),
            self.dense_embedders[1].is_ready(),
            self.dense_embedders[2].is_ready(),
            self.dense_embedders[3].is_ready(),
            self.dense_embedders[4].is_ready(),
            self.sparse_embedders[0].is_ready(),
            self.dense_embedders[5].is_ready(),
            self.dense_embedders[6].is_ready(),
            self.dense_embedders[7].is_ready(),
            self.dense_embedders[8].is_ready(),
            self.dense_embedders[9].is_ready(),
            self.token_embedder.is_ready(),
            self.sparse_embedders[1].is_ready(),
        ]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embed_all_produces_valid_fingerprint() {
        let provider = StubMultiArrayProvider::new();
        let result = provider.embed_all("Test content for embedding").await;

        assert!(result.is_ok(), "embed_all should succeed");
        let output = result.unwrap();

        // Verify dimensions
        assert_eq!(output.fingerprint.e1_semantic.len(), 1024, "E1 should be 1024D");
        assert_eq!(output.fingerprint.e7_code.len(), 1536, "E7 should be 1536D");
        assert_eq!(output.fingerprint.e8_graph.len(), 384, "E8 should be 384D");

        // Validation should pass
        assert!(output.fingerprint.validate().is_ok());
    }

    #[tokio::test]
    async fn test_embed_all_deterministic() {
        let provider = StubMultiArrayProvider::new();
        let content = "Deterministic test content";

        let result1 = provider.embed_all(content).await.unwrap();
        let result2 = provider.embed_all(content).await.unwrap();

        // Same content should produce identical fingerprints
        assert_eq!(result1.fingerprint.e1_semantic, result2.fingerprint.e1_semantic);
        assert_eq!(result1.fingerprint.e5_causal, result2.fingerprint.e5_causal);
    }

    #[tokio::test]
    async fn test_embed_all_empty_content() {
        let provider = StubMultiArrayProvider::new();
        let result = provider.embed_all("").await;

        assert!(result.is_ok(), "Empty content should not error");
        let output = result.unwrap();

        // Sparse vectors should be empty
        assert_eq!(output.fingerprint.e6_sparse.nnz(), 0);
        assert_eq!(output.fingerprint.e13_splade.nnz(), 0);

        // Token embeddings should be empty
        assert!(output.fingerprint.e12_late_interaction.is_empty());
    }

    #[tokio::test]
    async fn test_embed_all_different_content_different_output() {
        let provider = StubMultiArrayProvider::new();

        let result1 = provider.embed_all("Content A").await.unwrap();
        let result2 = provider.embed_all("Content B").await.unwrap();

        // Different content should produce different fingerprints
        assert_ne!(result1.fingerprint.e1_semantic, result2.fingerprint.e1_semantic);
    }

    #[tokio::test]
    async fn test_is_ready() {
        let provider = StubMultiArrayProvider::new();
        assert!(provider.is_ready(), "Stub provider should always be ready");
    }

    #[tokio::test]
    async fn test_health_status() {
        let provider = StubMultiArrayProvider::new();
        let health = provider.health_status();

        assert_eq!(health.len(), NUM_EMBEDDERS);
        assert!(health.iter().all(|&amp;h| h), "All embedders should be healthy");
    }

    #[tokio::test]
    async fn test_model_ids() {
        let provider = StubMultiArrayProvider::new();
        let ids = provider.model_ids();

        assert_eq!(ids.len(), NUM_EMBEDDERS);
        assert!(ids[0].contains("semantic"), "E1 model ID should reference semantic");
    }

    #[tokio::test]
    async fn test_latency_tracking() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("Test").await.unwrap();

        // Total latency should be recorded
        assert!(output.total_latency.as_nanos() > 0);

        // Should be within target (stubs are fast)
        assert!(output.is_within_latency_target());
    }

    #[tokio::test]
    async fn test_embed_batch_all() {
        let provider = StubMultiArrayProvider::new();
        let contents = vec![
            "Content 1".to_string(),
            "Content 2".to_string(),
            "Content 3".to_string(),
        ];

        let results = provider.embed_batch_all(&amp;contents).await.unwrap();

        assert_eq!(results.len(), 3);
        for (i, result) in results.iter().enumerate() {
            assert!(result.fingerprint.validate().is_ok(), "Batch item {} should validate", i);
        }
    }
}
```

## Step 4: Update embeddings/mod.rs

Add to `crates/context-graph-core/src/embeddings/mod.rs`:

```rust
pub mod provider;
pub mod stubs;

pub use provider::StubMultiArrayProvider;
pub use stubs::{StubSingleEmbedder, StubSparseEmbedder, StubTokenEmbedder};
```

## Step 5: Run Full State Verification
</implementation_guide>

<full_state_verification>
  <source_of_truth_definition>
    The source of truth for this task's success is:
    1. The provider.rs and stubs.rs files exist and compile
    2. StubMultiArrayProvider implements MultiArrayEmbeddingProvider trait
    3. All tests pass with real assertions (not mocks)
    4. embed_all() returns a valid SemanticFingerprint that passes validate()
  </source_of_truth_definition>

  <execute_and_inspect>
    After implementation, run these commands and verify output:

    ```bash
    # 1. Build the crate
    cd /home/cabdru/contextgraph
    cargo build --package context-graph-core 2>&amp;1 | tee /tmp/build_output.txt

    # Verify: No errors, warnings acceptable

    # 2. Run all provider tests
    cargo test --package context-graph-core provider:: -- --nocapture 2>&amp;1 | tee /tmp/test_output.txt

    # Verify: All tests pass with actual assertions

    # 3. Run stubs tests
    cargo test --package context-graph-core stubs:: -- --nocapture

    # 4. Run clippy
    cargo clippy --package context-graph-core -- -D warnings 2>&amp;1 | tee /tmp/clippy_output.txt

    # Verify: No errors or warnings

    # 5. Verify types exist in documentation
    cargo doc --package context-graph-core --no-deps
    grep -r "StubMultiArrayProvider" target/doc/context_graph_core/

    # Verify: StubMultiArrayProvider appears in docs
    ```
  </execute_and_inspect>

  <boundary_edge_cases>
    <case name="empty_content">
      Input: provider.embed_all("").await
      Expected Output: Ok(MultiArrayEmbeddingOutput) with:
        - e1_semantic.len() == 1024 (zeros)
        - e6_sparse.nnz() == 0
        - e12_late_interaction.is_empty() == true
        - fingerprint.validate() == Ok(())
      Verification: Check output.fingerprint.e6_sparse.nnz() == 0
    </case>

    <case name="determinism">
      Input: provider.embed_all("same content") called twice
      Expected: Both outputs have identical e1_semantic vectors
      Verification: assert_eq!(output1.fingerprint.e1_semantic, output2.fingerprint.e1_semantic)
    </case>

    <case name="different_content_different_output">
      Input: provider.embed_all("A") vs provider.embed_all("B")
      Expected: Different e1_semantic vectors
      Verification: assert_ne!(output1.fingerprint.e1_semantic, output2.fingerprint.e1_semantic)
    </case>

    <case name="all_dimensions_correct">
      Input: provider.embed_all("test").await
      Expected dimensions:
        - e1_semantic: 1024
        - e2_temporal_recent: 512
        - e3_temporal_periodic: 512
        - e4_temporal_positional: 512
        - e5_causal: 768
        - e6_sparse: indices &lt; 30522
        - e7_code: 1536
        - e8_graph: 384
        - e9_hdc: 1024
        - e10_multimodal: 768
        - e11_entity: 384
        - e12_late_interaction: each token 128D
        - e13_splade: indices &lt; 30522
      Verification: fingerprint.validate() == Ok(())
    </case>

    <case name="batch_processing">
      Input: provider.embed_batch_all(&amp;["a", "b", "c"].map(String::from))
      Expected: Vec with 3 valid outputs
      Verification: results.len() == 3 &amp;&amp; all validate
    </case>

    <case name="latency_tracking">
      Input: provider.embed_all("test")
      Expected: output.total_latency > Duration::ZERO
      Verification: assert!(output.total_latency.as_nanos() > 0)
    </case>

    <case name="health_status_all_healthy">
      Input: provider.health_status()
      Expected: [true; 13]
      Verification: health.iter().all(|&amp;h| h)
    </case>

    <case name="validation_catches_bad_dimensions">
      Input: Manually construct SemanticFingerprint with e1_semantic.len() != 1024
      Expected: fingerprint.validate() returns Err(ValidationError::DimensionMismatch)
      Verification: This is tested in validation_tests.rs, provider should never produce invalid
    </case>
  </boundary_edge_cases>

  <evidence_log>
    Required Evidence Before Marking Complete:

    1. [ ] cargo build --package context-graph-core succeeds without errors
    2. [ ] cargo test --package context-graph-core provider:: shows all tests passing
    3. [ ] test_embed_all_produces_valid_fingerprint passes with dimension assertions
    4. [ ] test_embed_all_deterministic passes showing same content = same output
    5. [ ] test_embed_all_empty_content passes showing empty handling works
    6. [ ] test_embed_all_different_content_different_output passes
    7. [ ] test_embed_batch_all passes with 3 items
    8. [ ] cargo clippy produces no errors
    9. [ ] StubMultiArrayProvider appears in cargo doc output
    10. [ ] fingerprint.validate() called in embed_all (verify in code)
  </evidence_log>

  <manual_verification>
    After implementation, manually verify:

    ```bash
    # Run specific test with output
    cd /home/cabdru/contextgraph
    cargo test --package context-graph-core test_embed_all_produces_valid_fingerprint -- --nocapture

    # Expected output includes assertions like:
    # E1 should be 1024D
    # E7 should be 1536D

    # Verify the provider file exists
    ls -la crates/context-graph-core/src/embeddings/provider.rs
    ls -la crates/context-graph-core/src/embeddings/stubs.rs

    # Check for StubMultiArrayProvider in exports
    grep "StubMultiArrayProvider" crates/context-graph-core/src/embeddings/mod.rs

    # Check trait implementation
    grep "impl MultiArrayEmbeddingProvider for StubMultiArrayProvider" crates/context-graph-core/src/embeddings/provider.rs
    ```
  </manual_verification>

  <physical_proof_verification>
    The output MUST be verified by checking:

    1. **Code Compilation**: cargo build must succeed
       - Check: No "error[E" messages in output
       - Source: stdout of cargo build

    2. **Test Execution**: All tests pass
       - Check: "test result: ok" at end of test run
       - Source: stdout of cargo test

    3. **Dimension Validation**: fingerprint.validate() called
       - Check: grep "validate()" in provider.rs shows it's called
       - Source: provider.rs source code

    4. **Type Implementation**: StubMultiArrayProvider implements trait
       - Check: grep "impl MultiArrayEmbeddingProvider" returns match
       - Source: provider.rs source code

    5. **Determinism**: Same input = same output
       - Check: test_embed_all_deterministic passes
       - Source: test output showing assertion passed
  </physical_proof_verification>
</full_state_verification>

<files_to_create>
  <file path="crates/context-graph-core/src/embeddings/provider.rs">
    StubMultiArrayProvider implementing MultiArrayEmbeddingProvider
  </file>
  <file path="crates/context-graph-core/src/embeddings/stubs.rs">
    StubSingleEmbedder, StubSparseEmbedder, StubTokenEmbedder implementations
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/embeddings/mod.rs">
    Add: pub mod provider; pub mod stubs; and re-exports
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>StubMultiArrayProvider::new() compiles and creates working instance</criterion>
  <criterion>embed_all() returns CoreResult&lt;MultiArrayEmbeddingOutput&gt;</criterion>
  <criterion>All 13 embeddings have correct dimensions (verified by validate())</criterion>
  <criterion>Parallel execution via tokio::join! (visible in code)</criterion>
  <criterion>Timeout handling returns EmbeddingError with embedder context</criterion>
  <criterion>Deterministic: same content always produces same fingerprint</criterion>
  <criterion>Empty content produces valid (zero/empty) fingerprint</criterion>
  <criterion>No .unwrap() in library code (per AP-14)</criterion>
  <criterion>All tests pass with actual assertions, no mocks</criterion>
</validation_criteria>

<test_commands>
  <command description="Build crate">cargo build --package context-graph-core</command>
  <command description="Run provider tests">cargo test --package context-graph-core provider:: -- --nocapture</command>
  <command description="Run stubs tests">cargo test --package context-graph-core stubs:: -- --nocapture</command>
  <command description="Run all embedding tests">cargo test --package context-graph-core embeddings:: -- --nocapture</command>
  <command description="Check for warnings">cargo clippy --package context-graph-core -- -D warnings</command>
  <command description="Generate docs">cargo doc --package context-graph-core --no-deps</command>
</test_commands>
</task_spec>
```

## Execution Checklist

### Pre-Implementation (MANDATORY)
- [ ] Read `crates/context-graph-core/src/traits/multi_array_embedding.rs` (understand trait)
- [ ] Read `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` (understand struct)
- [ ] Read `crates/context-graph-core/src/types/fingerprint/sparse.rs` (understand SparseVector)
- [ ] Read `crates/context-graph-core/src/embeddings/config.rs` (understand dimensions)
- [ ] Read `crates/context-graph-core/src/teleological/embedder.rs` (understand Embedder enum)
- [ ] Read `crates/context-graph-core/src/error/sub_errors.rs` (understand EmbeddingError)

### Implementation
- [ ] Create `stubs.rs` with StubSingleEmbedder
- [ ] Add StubSparseEmbedder to `stubs.rs`
- [ ] Add StubTokenEmbedder to `stubs.rs`
- [ ] Create `provider.rs` with StubMultiArrayProvider
- [ ] Implement embed_all() with tokio::join!
- [ ] Implement embed_batch_all()
- [ ] Add timeout handling
- [ ] Call fingerprint.validate() before returning
- [ ] Update `embeddings/mod.rs` with new modules

### Testing
- [ ] Add test_embed_all_produces_valid_fingerprint
- [ ] Add test_embed_all_deterministic
- [ ] Add test_embed_all_empty_content
- [ ] Add test_embed_all_different_content_different_output
- [ ] Add test_embed_batch_all
- [ ] Add test_is_ready
- [ ] Add test_health_status
- [ ] Add test_latency_tracking

### Verification
- [ ] Run `cargo build --package context-graph-core`
- [ ] Run `cargo test --package context-graph-core provider::`
- [ ] Run `cargo clippy --package context-graph-core -- -D warnings`
- [ ] Verify all edge cases from boundary_edge_cases section
- [ ] Complete evidence_log checklist
- [ ] Mark task COMPLETE

## Key Corrections from Original Task

| Original (WRONG) | Corrected |
|------------------|-----------|
| `embedding/provider.rs` | `embeddings/provider.rs` (note: embedding**s**) |
| `EmbedderImpl` trait | `SingleEmbedder`, `SparseEmbedder`, `TokenEmbedder` traits |
| `EmbeddingResult` enum | Not needed - trait returns specific types |
| `MockEmbedder` | `StubSingleEmbedder` (deterministic, not mock) |
| `Embedder::E1Semantic` | `Embedder::Semantic` |
| `Embedder::E8Emotional` | `Embedder::Emotional` (but field is `e8_graph`) |
| `BinaryVector` for E9 | E9 is 1024D dense Vec<f32> (projected) |
| Create new error types | Use existing `EmbeddingError` from `error/sub_errors.rs` |
| `embedding/error.rs` | Not needed - errors already exist |
| `validate_teleological_array()` | `fingerprint.validate()` (method on struct) |
| `TeleologicalArray::new()` | `SemanticFingerprint::zeroed()` (test-only) |
