# TASK-CORE-007: Per-Embedder Index Structure

```xml
<task_spec id="TASK-CORE-007" version="1.0">
<metadata>
  <title>Define Per-Embedder Index Structure</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>7</sequence>
  <implements>
    <requirement_ref>REQ-INDEX-PEREMBED-01</requirement_ref>
    <requirement_ref>REQ-HNSW-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
Each of the 13 embedders requires its own index for efficient search. Different
embedding types (dense, sparse, token-level) need different index structures.
This task defines the configuration and trait for per-embedder indices.
</context>

<objective>
Create the EmbedderIndex trait and index configuration types that support HNSW
for dense vectors, inverted indices for sparse vectors, and specialized structures
for token-level embeddings.
</objective>

<rationale>
Per-embedder indexing enables:
1. Entry-point discovery - search any of 13 spaces independently
2. Optimal index types per embedding format
3. Parallel index updates
4. Selective index loading based on query patterns

HNSW provides logarithmic search for dense vectors, while inverted indices
excel for sparse SPLADE embeddings.
</rationale>

<input_context_files>
  <file purpose="embedder_enum">crates/context-graph-core/src/teleology/embedder.rs</file>
  <file purpose="store_trait">crates/context-graph-storage/src/teleological/store.rs</file>
  <file purpose="search_spec">docs2/refactor/03-SEARCH.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-006 complete (storage trait exists)</check>
  <check>Embedder enum with dimension metadata exists</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Define EmbedderIndex trait</item>
    <item>Define IndexType enum (HNSW, Inverted, TokenLevel)</item>
    <item>Define HnswConfig with m, ef_construction, ef_search</item>
    <item>Define InvertedIndexConfig for sparse embeddings</item>
    <item>Define EmbedderIndexConfig per-embedder settings</item>
    <item>Define default configurations per embedder type</item>
  </in_scope>
  <out_of_scope>
    <item>RocksDB implementation (TASK-CORE-008)</item>
    <item>Actual HNSW library integration</item>
    <item>Search engine (TASK-LOGIC-005 through 008)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/index.rs">
      use crate::teleological::store::StorageResult;
      use context_graph_core::teleology::embedder::Embedder;
      use context_graph_core::teleology::array::EmbedderOutput;
      use async_trait::async_trait;
      use uuid::Uuid;

      /// Type of index used for an embedder.
      #[derive(Debug, Clone, Copy, PartialEq, Eq)]
      pub enum IndexType {
          /// Hierarchical Navigable Small World graph for dense vectors
          Hnsw,
          /// Inverted index for sparse vectors
          Inverted,
          /// Specialized index for token-level embeddings
          TokenLevel,
          /// Binary-specific index for HDC
          Binary,
      }

      /// Configuration for HNSW index.
      #[derive(Debug, Clone)]
      pub struct HnswConfig {
          /// Number of bidirectional links per node
          pub m: usize,
          /// Size of dynamic candidate list during construction
          pub ef_construction: usize,
          /// Size of dynamic candidate list during search
          pub ef_search: usize,
          /// Maximum level in the graph
          pub max_level: usize,
      }

      impl HnswConfig {
          pub fn default_for_dims(dims: usize) -> Self;
          pub fn high_recall() -> Self;
          pub fn balanced() -> Self;
          pub fn fast_search() -> Self;
      }

      /// Configuration for inverted index (sparse embeddings).
      #[derive(Debug, Clone)]
      pub struct InvertedIndexConfig {
          /// Maximum number of terms to index per vector
          pub max_terms: usize,
          /// Minimum term weight to include
          pub min_weight: f32,
          /// Use BM25 scoring
          pub use_bm25: bool,
      }

      /// Configuration for token-level index.
      #[derive(Debug, Clone)]
      pub struct TokenLevelConfig {
          /// Maximum tokens to index per document
          pub max_tokens: usize,
          /// Dimensions per token
          pub dims_per_token: usize,
          /// Use approximate MaxSim
          pub approximate: bool,
      }

      /// Per-embedder index configuration.
      #[derive(Debug, Clone)]
      pub struct EmbedderIndexConfig {
          pub embedder: Embedder,
          pub index_type: IndexType,
          pub hnsw_config: Option<HnswConfig>,
          pub inverted_config: Option<InvertedIndexConfig>,
          pub token_level_config: Option<TokenLevelConfig>,
          pub enabled: bool,
      }

      impl EmbedderIndexConfig {
          pub fn default_for(embedder: Embedder) -> Self;
      }

      /// Result from an index search.
      #[derive(Debug, Clone)]
      pub struct IndexSearchResult {
          pub id: Uuid,
          pub score: f32,
      }

      /// Trait for per-embedder index operations.
      #[async_trait]
      pub trait EmbedderIndex: Send + Sync {
          /// Get the embedder this index serves.
          fn embedder(&self) -> Embedder;

          /// Get the index type.
          fn index_type(&self) -> IndexType;

          /// Add a vector to the index.
          async fn add(&self, id: Uuid, embedding: &EmbedderOutput) -> StorageResult<()>;

          /// Add multiple vectors (batch).
          async fn add_batch(&self, items: &[(Uuid, EmbedderOutput)]) -> StorageResult<()>;

          /// Remove a vector from the index.
          async fn remove(&self, id: Uuid) -> StorageResult<()>;

          /// Search for similar vectors.
          async fn search(
              &self,
              query: &EmbedderOutput,
              limit: usize,
              threshold: Option<f32>,
          ) -> StorageResult<Vec<IndexSearchResult>>;

          /// Get index size.
          async fn size(&self) -> StorageResult<usize>;

          /// Optimize the index (compact, rebalance).
          async fn optimize(&self) -> StorageResult<()>;
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>EmbedderIndex trait is object-safe</constraint>
    <constraint>Default config for each of 13 embedders</constraint>
    <constraint>HNSW for dense, Inverted for sparse</constraint>
    <constraint>TokenLevel for ColBERT (E12)</constraint>
    <constraint>Binary for HDC (E9)</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-storage</command>
    <command>cargo test -p context-graph-storage index_config</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/index.rs

use async_trait::async_trait;
use uuid::Uuid;
use context_graph_core::teleology::embedder::{Embedder, EmbedderDims};
use context_graph_core::teleology::array::EmbedderOutput;
use crate::teleological::store::StorageResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    Hnsw,
    Inverted,
    TokenLevel,
    Binary,
}

#[derive(Debug, Clone)]
pub struct HnswConfig {
    pub m: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub max_level: usize,
}

impl HnswConfig {
    pub fn balanced() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 100,
            max_level: 16,
        }
    }

    pub fn high_recall() -> Self {
        Self {
            m: 32,
            ef_construction: 400,
            ef_search: 200,
            max_level: 20,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InvertedIndexConfig {
    pub max_terms: usize,
    pub min_weight: f32,
    pub use_bm25: bool,
}

impl Default for InvertedIndexConfig {
    fn default() -> Self {
        Self {
            max_terms: 1000,
            min_weight: 0.01,
            use_bm25: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbedderIndexConfig {
    pub embedder: Embedder,
    pub index_type: IndexType,
    pub hnsw_config: Option<HnswConfig>,
    pub inverted_config: Option<InvertedIndexConfig>,
    pub token_level_config: Option<TokenLevelConfig>,
    pub enabled: bool,
}

impl EmbedderIndexConfig {
    pub fn default_for(embedder: Embedder) -> Self {
        let dims = embedder.expected_dims();
        let (index_type, hnsw, inverted, token_level) = match dims {
            EmbedderDims::Dense(_) => (
                IndexType::Hnsw,
                Some(HnswConfig::balanced()),
                None,
                None,
            ),
            EmbedderDims::Sparse { .. } => (
                IndexType::Inverted,
                None,
                Some(InvertedIndexConfig::default()),
                None,
            ),
            EmbedderDims::TokenLevel { per_token } => (
                IndexType::TokenLevel,
                None,
                None,
                Some(TokenLevelConfig { max_tokens: 512, dims_per_token: per_token, approximate: true }),
            ),
            EmbedderDims::Binary { .. } => (
                IndexType::Binary,
                None,
                None,
                None,
            ),
        };

        Self {
            embedder,
            index_type,
            hnsw_config: hnsw,
            inverted_config: inverted,
            token_level_config: token_level,
            enabled: true,
        }
    }
}

#[async_trait]
pub trait EmbedderIndex: Send + Sync {
    fn embedder(&self) -> Embedder;
    fn index_type(&self) -> IndexType;
    async fn add(&self, id: Uuid, embedding: &EmbedderOutput) -> StorageResult<()>;
    async fn add_batch(&self, items: &[(Uuid, EmbedderOutput)]) -> StorageResult<()>;
    async fn remove(&self, id: Uuid) -> StorageResult<()>;
    async fn search(
        &self,
        query: &EmbedderOutput,
        limit: usize,
        threshold: Option<f32>,
    ) -> StorageResult<Vec<IndexSearchResult>>;
    async fn size(&self) -> StorageResult<usize>;
    async fn optimize(&self) -> StorageResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_per_embedder() {
        for e in Embedder::all() {
            let config = EmbedderIndexConfig::default_for(e);
            assert!(config.enabled);
            // Verify appropriate index type for embedding format
            match e.expected_dims() {
                EmbedderDims::Dense(_) => assert_eq!(config.index_type, IndexType::Hnsw),
                EmbedderDims::Sparse { .. } => assert_eq!(config.index_type, IndexType::Inverted),
                _ => {}
            }
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/index.rs">
    Per-embedder index trait and configuration types
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/mod.rs">
    Add: pub mod index;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>EmbedderIndex trait is object-safe</criterion>
  <criterion>default_for() returns HNSW for dense embedders</criterion>
  <criterion>default_for() returns Inverted for SPLADE embedders</criterion>
  <criterion>default_for() returns TokenLevel for E12</criterion>
  <criterion>HnswConfig has reasonable defaults</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage index -- --nocapture</command>
</test_commands>
</task_spec>
```
