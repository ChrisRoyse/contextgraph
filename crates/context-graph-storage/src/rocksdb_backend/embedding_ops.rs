//! Embedding storage operations for RocksDB backend.
//!
//! Provides direct access to embedding vectors stored in the `embeddings` CF.
//! While embeddings are stored as part of `store_node()`, these methods allow:
//! - Independent embedding updates without full node serialization
//! - Batch retrieval for vector search preparation
//! - Single embedding retrieval for similarity calculations
//!
//! # Performance
//! - Single embedding: ~6KB read (1536D x 4 bytes/f32)
//! - Batch retrieval uses RocksDB `multi_get_cf` for efficiency
//!
//! # Key Format
//! Embeddings are keyed by 16-byte raw UUID (same as nodes CF)

use crate::column_families::cf_names;
use crate::serialization::{deserialize_embedding, serialize_embedding, serialize_uuid};
use context_graph_core::types::{EmbeddingVector, NodeId};

use super::core::RocksDbMemex;
use super::error::StorageError;

impl RocksDbMemex {
    /// Stores an embedding vector for a node.
    ///
    /// This is typically called as part of `store_node()`, but can be
    /// used to update embeddings independently when:
    /// - Embeddings are regenerated (new model, re-encoding)
    /// - Only embedding needs updating without full node overhead
    ///
    /// # Arguments
    /// * `node_id` - The node ID to associate the embedding with
    /// * `embedding` - The embedding vector (typically 1536 dimensions)
    ///
    /// # Returns
    /// * `Ok(())` - Embedding stored successfully
    /// * `Err(StorageError::WriteFailed)` - RocksDB write error
    /// * `Err(StorageError::ColumnFamilyNotFound)` - CF missing (should never happen)
    ///
    /// # Performance
    /// ~6KB write for 1536-dimensional vector
    ///
    /// # Example
    /// ```
    /// // Embeddings are 1536-dimensional vectors
    /// let embedding = vec![0.1_f32; 1536];
    /// assert_eq!(embedding.len(), 1536);
    /// ```
    pub fn store_embedding(
        &self,
        node_id: &NodeId,
        embedding: &EmbeddingVector,
    ) -> Result<(), StorageError> {
        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
        let key = serialize_uuid(node_id);
        let value = serialize_embedding(embedding);

        self.db
            .put_cf(cf_embeddings, key.as_slice(), &value)
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Retrieves an embedding vector by node ID.
    ///
    /// # Arguments
    /// * `node_id` - The node ID to retrieve the embedding for
    ///
    /// # Returns
    /// * `Ok(Vec<f32>)` - The embedding vector
    /// * `Err(StorageError::NotFound)` - No embedding exists for this node
    /// * `Err(StorageError::Serialization)` - Corrupted embedding data
    /// * `Err(StorageError::ReadFailed)` - RocksDB read error
    ///
    /// # Example
    /// ```
    /// use context_graph_core::types::EmbeddingVector;
    ///
    /// // Embedding vectors are Vec<f32> with 1536 dimensions
    /// let embedding: EmbeddingVector = vec![0.1_f32; 1536];
    /// assert_eq!(embedding.len(), 1536);
    /// ```
    pub fn get_embedding(&self, node_id: &NodeId) -> Result<EmbeddingVector, StorageError> {
        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
        let key = serialize_uuid(node_id);

        let value = self
            .db
            .get_cf(cf_embeddings, key.as_slice())
            .map_err(|e| StorageError::ReadFailed(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound {
                id: node_id.to_string(),
            })?;

        deserialize_embedding(&value).map_err(StorageError::from)
    }

    /// Retrieves multiple embeddings in a single batch operation.
    ///
    /// Returns a Vec of `Option<Vec<f32>>` in the same order as input IDs.
    /// None indicates the embedding was not found for that ID.
    ///
    /// More efficient than multiple `get_embedding()` calls because:
    /// 1. Single RocksDB batch operation
    /// 2. Better cache utilization
    /// 3. Reduced lock contention
    ///
    /// # Arguments
    /// * `node_ids` - Slice of node IDs to retrieve embeddings for
    ///
    /// # Returns
    /// * `Ok(Vec<Option<Vec<f32>>>)` - Embeddings in same order as input IDs
    ///   - `Some(embedding)` if found
    ///   - `None` if not found for that ID
    /// * `Err(StorageError::Serialization)` - If any found embedding is corrupted
    /// * `Err(StorageError::ReadFailed)` - RocksDB batch read error
    ///
    /// # Example
    /// ```
    /// use uuid::Uuid;
    ///
    /// // Create sample node IDs for batch operations
    /// let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn batch_get_embeddings(
        &self,
        node_ids: &[NodeId],
    ) -> Result<Vec<Option<EmbeddingVector>>, StorageError> {
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }

        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;

        // Build keys for multi_get
        let keys: Vec<[u8; 16]> = node_ids.iter().map(serialize_uuid).collect();

        // Create CF references for multi_get_cf
        let cf_key_pairs: Vec<(&rocksdb::ColumnFamily, &[u8])> = keys
            .iter()
            .map(|k| (cf_embeddings, k.as_slice()))
            .collect();

        // Execute batch read
        let results = self.db.multi_get_cf(cf_key_pairs);

        // Process results, preserving order
        let mut embeddings = Vec::with_capacity(node_ids.len());
        for result in results {
            match result {
                Ok(Some(bytes)) => {
                    let embedding = deserialize_embedding(&bytes)?;
                    embeddings.push(Some(embedding));
                }
                Ok(None) => {
                    embeddings.push(None);
                }
                Err(e) => {
                    return Err(StorageError::ReadFailed(e.to_string()));
                }
            }
        }

        Ok(embeddings)
    }

    /// Deletes an embedding for a node.
    ///
    /// NOTE: This is typically called as part of `delete_node()`.
    /// Use this only for independent embedding cleanup.
    ///
    /// # Arguments
    /// * `node_id` - The node ID whose embedding to delete
    ///
    /// # Returns
    /// * `Ok(())` - Embedding deleted (or didn't exist)
    /// * `Err(StorageError::WriteFailed)` - RocksDB delete error
    pub fn delete_embedding(&self, node_id: &NodeId) -> Result<(), StorageError> {
        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
        let key = serialize_uuid(node_id);

        self.db
            .delete_cf(cf_embeddings, key.as_slice())
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Checks if an embedding exists for a node.
    ///
    /// More efficient than `get_embedding()` when you only need existence check.
    ///
    /// # Arguments
    /// * `node_id` - The node ID to check
    ///
    /// # Returns
    /// * `Ok(true)` - Embedding exists
    /// * `Ok(false)` - No embedding for this node
    /// * `Err(StorageError::ReadFailed)` - RocksDB read error
    pub fn embedding_exists(&self, node_id: &NodeId) -> Result<bool, StorageError> {
        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
        let key = serialize_uuid(node_id);

        // Use get_cf for existence check
        match self.db.get_cf(cf_embeddings, key.as_slice()) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(StorageError::ReadFailed(e.to_string())),
        }
    }
}
