# TASK-CORE-008: RocksDB Schema Migration

```xml
<task_spec id="TASK-CORE-008" version="1.0">
<metadata>
  <title>Implement RocksDB Storage for TeleologicalArrays</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>8</sequence>
  <implements>
    <requirement_ref>REQ-STORAGE-ROCKSDB-01</requirement_ref>
    <requirement_ref>REQ-STORAGE-ATOMIC-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-007</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
Implements the TeleologicalArrayStore trait using RocksDB as the backend. This is
the production storage implementation with column families for data separation,
WriteBatch for atomicity, and integration with per-embedder indices.
</context>

<objective>
Create a RocksDB implementation of TeleologicalArrayStore with column families
for arrays, metadata, and indices, ensuring atomic operations and efficient
batch processing.
</objective>

<rationale>
RocksDB is chosen for:
1. High write throughput for memory injection
2. Efficient range scans for namespace queries
3. Column families for logical separation
4. WriteBatch for atomic multi-key updates
5. SST files for bulk loading
6. Proven reliability at scale
</rationale>

<input_context_files>
  <file purpose="store_trait">crates/context-graph-storage/src/teleological/store.rs</file>
  <file purpose="index_trait">crates/context-graph-storage/src/teleological/index.rs</file>
  <file purpose="existing_rocksdb">crates/context-graph-storage/src/db/</file>
  <file purpose="storage_spec">docs2/refactor/02-STORAGE.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-007 complete (index structures defined)</check>
  <check>RocksDB crate available in Cargo.toml</check>
  <check>TeleologicalArrayStore trait defined</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create RocksDbTeleologicalStore struct</item>
    <item>Implement TeleologicalArrayStore trait</item>
    <item>Create column families: arrays, metadata, indices</item>
    <item>Implement batch operations with WriteBatch</item>
    <item>Add transaction support for consistency</item>
    <item>Implement all 13 per-embedder indices</item>
    <item>Handle index updates on store/delete</item>
  </in_scope>
  <out_of_scope>
    <item>IndexedTeleologicalStore (search) - separate task</item>
    <item>HNSW library integration (separate crate)</item>
    <item>Migration from old schema (migration script)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/rocksdb_store.rs">
      use crate::teleological::store::{
          TeleologicalArrayStore, StorageResult, StorageError, StorageStats
      };
      use crate::teleological::index::{EmbedderIndex, EmbedderIndexConfig};
      use context_graph_core::teleology::array::TeleologicalArray;
      use context_graph_core::teleology::embedder::Embedder;
      use rocksdb::{DB, Options, ColumnFamilyDescriptor, WriteBatch};
      use async_trait::async_trait;
      use uuid::Uuid;
      use std::sync::Arc;

      /// Column family names
      pub const CF_ARRAYS: &str = "teleological_arrays";
      pub const CF_METADATA: &str = "array_metadata";
      pub const CF_NAMESPACE_INDEX: &str = "namespace_index";

      /// RocksDB-backed teleological array storage.
      pub struct RocksDbTeleologicalStore {
          db: Arc<DB>,
          indices: [Box<dyn EmbedderIndex>; 13],
      }

      impl RocksDbTeleologicalStore {
          /// Open or create the store at the given path.
          pub fn open(path: &str, config: StoreConfig) -> StorageResult<Self>;

          /// Open with custom index configurations.
          pub fn open_with_indices(
              path: &str,
              config: StoreConfig,
              index_configs: [EmbedderIndexConfig; 13],
          ) -> StorageResult<Self>;

          /// Get underlying DB handle (for advanced operations).
          pub fn db(&self) -> &DB;

          /// Update all indices for an array.
          async fn update_indices(&self, array: &TeleologicalArray) -> StorageResult<()>;

          /// Remove from all indices.
          async fn remove_from_indices(&self, id: Uuid) -> StorageResult<()>;
      }

      #[derive(Debug, Clone)]
      pub struct StoreConfig {
          pub create_if_missing: bool,
          pub max_open_files: i32,
          pub cache_size_mb: usize,
          pub enable_compression: bool,
      }

      impl Default for StoreConfig {
          fn default() -> Self {
              Self {
                  create_if_missing: true,
                  max_open_files: 1000,
                  cache_size_mb: 256,
                  enable_compression: true,
              }
          }
      }

      #[async_trait]
      impl TeleologicalArrayStore for RocksDbTeleologicalStore {
          async fn store(&self, array: &TeleologicalArray) -> StorageResult<()>;
          async fn store_batch(&self, arrays: &[TeleologicalArray]) -> StorageResult<()>;
          async fn retrieve(&self, id: Uuid) -> StorageResult<TeleologicalArray>;
          async fn retrieve_batch(&self, ids: &[Uuid]) -> StorageResult<Vec<TeleologicalArray>>;
          async fn delete(&self, id: Uuid) -> StorageResult<()>;
          async fn exists(&self, id: Uuid) -> StorageResult<bool>;
          async fn count(&self) -> StorageResult<usize>;
          async fn stats(&self) -> StorageResult<StorageStats>;
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>store_batch must use WriteBatch for atomicity</constraint>
    <constraint>All column families created on open</constraint>
    <constraint>Index updates happen within same transaction</constraint>
    <constraint>No partial array storage (all 13 embeddings)</constraint>
    <constraint>Serialization via MessagePack for compactness</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-storage</command>
    <command>cargo test -p context-graph-storage rocksdb_store</command>
    <command>cargo test -p context-graph-storage integration -- --ignored</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/rocksdb_store.rs

use async_trait::async_trait;
use rocksdb::{DB, Options, ColumnFamily, ColumnFamilyDescriptor, WriteBatch};
use std::sync::Arc;
use uuid::Uuid;

use context_graph_core::teleology::array::TeleologicalArray;
use context_graph_core::teleology::embedder::Embedder;
use crate::teleological::store::*;
use crate::teleological::index::*;

pub const CF_ARRAYS: &str = "teleological_arrays";
pub const CF_METADATA: &str = "array_metadata";
pub const CF_NAMESPACE_INDEX: &str = "namespace_index";

pub struct RocksDbTeleologicalStore {
    db: Arc<DB>,
    indices: [Box<dyn EmbedderIndex>; 13],
}

impl RocksDbTeleologicalStore {
    pub fn open(path: &str, config: StoreConfig) -> StorageResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(config.create_if_missing);
        opts.set_max_open_files(config.max_open_files);

        // Create column families
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_ARRAYS, Options::default()),
            ColumnFamilyDescriptor::new(CF_METADATA, Options::default()),
            ColumnFamilyDescriptor::new(CF_NAMESPACE_INDEX, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // Initialize indices with default configs
        let indices = Self::create_default_indices(&db)?;

        Ok(Self {
            db: Arc::new(db),
            indices,
        })
    }

    fn create_default_indices(db: &DB) -> StorageResult<[Box<dyn EmbedderIndex>; 13]> {
        // Create one index per embedder
        let configs: Vec<_> = Embedder::all()
            .map(EmbedderIndexConfig::default_for)
            .collect();

        // ... instantiate appropriate index type per config
        todo!("Implement index instantiation")
    }

    async fn update_indices(&self, array: &TeleologicalArray) -> StorageResult<()> {
        for (idx, embedding) in array.embeddings.iter().enumerate() {
            let embedder = Embedder::from_index(idx).unwrap();
            self.indices[idx].add(array.id, embedding).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl TeleologicalArrayStore for RocksDbTeleologicalStore {
    async fn store(&self, array: &TeleologicalArray) -> StorageResult<()> {
        let cf = self.db.cf_handle(CF_ARRAYS)
            .ok_or_else(|| StorageError::Backend("CF not found".into()))?;

        let key = array.id.as_bytes();
        let value = rmp_serde::to_vec(array)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.db.put_cf(cf, key, value)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // Update all indices
        self.update_indices(array).await?;

        Ok(())
    }

    async fn store_batch(&self, arrays: &[TeleologicalArray]) -> StorageResult<()> {
        let cf = self.db.cf_handle(CF_ARRAYS)
            .ok_or_else(|| StorageError::Backend("CF not found".into()))?;

        let mut batch = WriteBatch::default();

        for array in arrays {
            let key = array.id.as_bytes();
            let value = rmp_serde::to_vec(array)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            batch.put_cf(cf, key, value);
        }

        self.db.write(batch)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // Update indices for all arrays
        for array in arrays {
            self.update_indices(array).await?;
        }

        Ok(())
    }

    async fn retrieve(&self, id: Uuid) -> StorageResult<TeleologicalArray> {
        let cf = self.db.cf_handle(CF_ARRAYS)
            .ok_or_else(|| StorageError::Backend("CF not found".into()))?;

        let key = id.as_bytes();
        let value = self.db.get_cf(cf, key)
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .ok_or(StorageError::NotFound(id))?;

        rmp_serde::from_slice(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    // ... other trait methods
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let dir = tempdir().unwrap();
        let store = RocksDbTeleologicalStore::open(
            dir.path().to_str().unwrap(),
            StoreConfig::default(),
        ).unwrap();

        let array = TeleologicalArray::new(Uuid::new_v4());
        store.store(&array).await.unwrap();

        let retrieved = store.retrieve(array.id).await.unwrap();
        assert_eq!(array.id, retrieved.id);
    }

    #[tokio::test]
    async fn test_batch_atomicity() {
        // Test that batch is atomic - all or nothing
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/rocksdb_store.rs">
    RocksDB implementation of TeleologicalArrayStore
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/mod.rs">
    Add: pub mod rocksdb_store;
  </file>
  <file path="crates/context-graph-storage/Cargo.toml">
    Ensure rocksdb, rmp-serde, tempfile (dev) dependencies
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>store/retrieve roundtrip preserves data</criterion>
  <criterion>store_batch is atomic (all or nothing)</criterion>
  <criterion>NotFound error for missing IDs</criterion>
  <criterion>Column families created on open</criterion>
  <criterion>Integration tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage rocksdb -- --nocapture</command>
  <command>cargo test -p context-graph-storage integration -- --ignored --nocapture</command>
</test_commands>
</task_spec>
```
