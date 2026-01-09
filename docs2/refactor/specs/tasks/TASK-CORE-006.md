# TASK-CORE-006: Storage Trait Definition

```xml
<task_spec id="TASK-CORE-006" version="1.0">
<metadata>
  <title>Define TeleologicalArrayStore Trait</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>6</sequence>
  <implements>
    <requirement_ref>REQ-STORAGE-TRAIT-01</requirement_ref>
    <requirement_ref>REQ-STORAGE-ASYNC-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>1.5</estimated_days>
</metadata>

<context>
Defines the async trait for storing and retrieving teleological arrays. This trait
is implemented by RocksDB (TASK-CORE-008) and potentially other backends. Must be
object-safe for dynamic dispatch.
</context>

<objective>
Create the TeleologicalArrayStore async trait with methods for CRUD operations,
batch processing, and search capabilities.
</objective>

<rationale>
The storage trait abstracts backend details, enabling:
1. Multiple backend implementations (RocksDB, SQLite, cloud)
2. Testing with mock implementations
3. Future-proofing for new storage engines
4. Async operations for non-blocking I/O

The trait includes both basic operations and indexed search operations via
the IndexedTeleologicalStore extension trait.
</rationale>

<input_context_files>
  <file purpose="array_type">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="comparison_types">crates/context-graph-core/src/teleology/comparison.rs</file>
  <file purpose="storage_spec">docs2/refactor/02-STORAGE.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-004 complete (ComparisonType exists)</check>
  <check>TeleologicalArray type exists</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Define TeleologicalArrayStore async trait</item>
    <item>Define IndexedTeleologicalStore extended trait</item>
    <item>Define SearchResult, SearchFilter types</item>
    <item>Define StorageStats, IndexStats types</item>
    <item>Define StorageError enum</item>
    <item>Ensure trait is object-safe</item>
  </in_scope>
  <out_of_scope>
    <item>Per-embedder index structure (TASK-CORE-007)</item>
    <item>RocksDB implementation (TASK-CORE-008)</item>
    <item>Search engine (TASK-LOGIC-005 through 008)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/store.rs">
      use crate::teleology::array::TeleologicalArray;
      use crate::teleology::comparison::ComparisonType;
      use async_trait::async_trait;
      use uuid::Uuid;

      /// Result type for storage operations.
      pub type StorageResult<T> = Result<T, StorageError>;

      /// Error type for storage operations.
      #[derive(Debug, thiserror::Error)]
      pub enum StorageError {
          #[error("Array not found: {0}")]
          NotFound(Uuid),
          #[error("Storage backend error: {0}")]
          Backend(String),
          #[error("Serialization error: {0}")]
          Serialization(String),
          #[error("Index error: {0}")]
          Index(String),
          #[error("Transaction error: {0}")]
          Transaction(String),
      }

      /// Base trait for teleological array storage.
      #[async_trait]
      pub trait TeleologicalArrayStore: Send + Sync {
          /// Store a single array.
          async fn store(&self, array: &TeleologicalArray) -> StorageResult<()>;

          /// Store multiple arrays in a batch (atomic).
          async fn store_batch(&self, arrays: &[TeleologicalArray]) -> StorageResult<()>;

          /// Retrieve a single array by ID.
          async fn retrieve(&self, id: Uuid) -> StorageResult<TeleologicalArray>;

          /// Retrieve multiple arrays by IDs.
          async fn retrieve_batch(&self, ids: &[Uuid]) -> StorageResult<Vec<TeleologicalArray>>;

          /// Delete an array by ID.
          async fn delete(&self, id: Uuid) -> StorageResult<()>;

          /// Check if an array exists.
          async fn exists(&self, id: Uuid) -> StorageResult<bool>;

          /// Get total count of arrays.
          async fn count(&self) -> StorageResult<usize>;

          /// Get storage statistics.
          async fn stats(&self) -> StorageResult<StorageStats>;
      }

      /// Extended trait with indexed search capabilities.
      #[async_trait]
      pub trait IndexedTeleologicalStore: TeleologicalArrayStore {
          /// Search for similar arrays.
          async fn search(
              &self,
              query: &TeleologicalArray,
              comparison: &ComparisonType,
              filter: Option<&SearchFilter>,
              limit: usize,
          ) -> StorageResult<Vec<SearchResult>>;

          /// Get index statistics.
          async fn index_stats(&self) -> StorageResult<IndexStats>;

          /// Rebuild indices (expensive operation).
          async fn rebuild_indices(&self) -> StorageResult<()>;
      }

      /// Result from a search operation.
      #[derive(Debug, Clone)]
      pub struct SearchResult {
          pub array: TeleologicalArray,
          pub similarity: f32,
          pub per_embedder_scores: [Option<f32>; 13],
      }

      /// Filter for search operations.
      #[derive(Debug, Clone, Default)]
      pub struct SearchFilter {
          pub namespace: Option<String>,
          pub created_after: Option<chrono::DateTime<chrono::Utc>>,
          pub created_before: Option<chrono::DateTime<chrono::Utc>>,
          pub min_similarity: Option<f32>,
          pub memory_types: Option<Vec<String>>,
      }

      /// Storage statistics.
      #[derive(Debug, Clone)]
      pub struct StorageStats {
          pub total_arrays: usize,
          pub total_bytes: u64,
          pub namespaces: Vec<String>,
          pub oldest_array: Option<chrono::DateTime<chrono::Utc>>,
          pub newest_array: Option<chrono::DateTime<chrono::Utc>>,
      }

      /// Per-embedder index statistics.
      #[derive(Debug, Clone)]
      pub struct IndexStats {
          pub indices_active: usize,
          pub total_indexed: usize,
          pub per_embedder: [EmbedderIndexStats; 13],
      }

      #[derive(Debug, Clone, Default)]
      pub struct EmbedderIndexStats {
          pub vectors_indexed: usize,
          pub index_size_bytes: u64,
          pub index_type: String,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Traits must be object-safe (dyn TeleologicalArrayStore works)</constraint>
    <constraint>All methods are async</constraint>
    <constraint>Batch operations must be atomic</constraint>
    <constraint>Send + Sync bounds for thread safety</constraint>
    <constraint>No blocking I/O in trait implementations</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-storage</command>
    <command>cargo test -p context-graph-storage store_trait</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/store.rs

use async_trait::async_trait;
use uuid::Uuid;
use thiserror::Error;
use chrono::{DateTime, Utc};

// Import from core crate
use context_graph_core::teleology::array::TeleologicalArray;
use context_graph_core::teleology::comparison::ComparisonType;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Array not found: {0}")]
    NotFound(Uuid),
    #[error("Storage backend error: {0}")]
    Backend(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Index error: {0}")]
    Index(String),
    #[error("Transaction error: {0}")]
    Transaction(String),
}

#[async_trait]
pub trait TeleologicalArrayStore: Send + Sync {
    async fn store(&self, array: &TeleologicalArray) -> StorageResult<()>;
    async fn store_batch(&self, arrays: &[TeleologicalArray]) -> StorageResult<()>;
    async fn retrieve(&self, id: Uuid) -> StorageResult<TeleologicalArray>;
    async fn retrieve_batch(&self, ids: &[Uuid]) -> StorageResult<Vec<TeleologicalArray>>;
    async fn delete(&self, id: Uuid) -> StorageResult<()>;
    async fn exists(&self, id: Uuid) -> StorageResult<bool>;
    async fn count(&self) -> StorageResult<usize>;
    async fn stats(&self) -> StorageResult<StorageStats>;
}

#[async_trait]
pub trait IndexedTeleologicalStore: TeleologicalArrayStore {
    async fn search(
        &self,
        query: &TeleologicalArray,
        comparison: &ComparisonType,
        filter: Option<&SearchFilter>,
        limit: usize,
    ) -> StorageResult<Vec<SearchResult>>;

    async fn index_stats(&self) -> StorageResult<IndexStats>;
    async fn rebuild_indices(&self) -> StorageResult<()>;
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub array: TeleologicalArray,
    pub similarity: f32,
    pub per_embedder_scores: [Option<f32>; 13],
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub namespace: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub min_similarity: Option<f32>,
    pub memory_types: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_arrays: usize,
    pub total_bytes: u64,
    pub namespaces: Vec<String>,
    pub oldest_array: Option<DateTime<Utc>>,
    pub newest_array: Option<DateTime<Utc>>,
}

// Verify trait is object-safe
fn _assert_object_safe(_: &dyn TeleologicalArrayStore) {}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/store.rs">
    Storage trait definitions
  </file>
  <file path="crates/context-graph-storage/src/teleological/mod.rs">
    Module definition
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/lib.rs">
    Add: pub mod teleological;
  </file>
  <file path="crates/context-graph-storage/Cargo.toml">
    Add: async-trait, thiserror, chrono dependencies
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>TeleologicalArrayStore is object-safe</criterion>
  <criterion>IndexedTeleologicalStore extends base trait</criterion>
  <criterion>All methods are async</criterion>
  <criterion>StorageError covers all failure modes</criterion>
  <criterion>SearchFilter enables flexible querying</criterion>
</validation_criteria>

<test_commands>
  <command>cargo check -p context-graph-storage</command>
</test_commands>
</task_spec>
```
