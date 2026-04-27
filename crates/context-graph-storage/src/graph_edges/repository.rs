//! RocksDB repository for graph edge storage.
//!
//! Provides persistent storage for:
//! - K-NN edges per embedder (embedder_edges CF)
//! - Multi-relation typed edges (typed_edges CF)
//! - Secondary index by edge type (typed_edges_by_type CF)
//!
//! # Column Families Used
//!
//! - `embedder_edges`: Key = [embedder_id: u8][source: 16 bytes], Value = Vec<EmbedderEdge>
//! - `typed_edges`: Key = [source: 16 bytes][target: 16 bytes], Value = TypedEdge
//! - `typed_edges_by_type`: Key = [edge_type: u8][source: 16 bytes][target: 16 bytes], Value = target UUID

use context_graph_core::graph_linking::{
    EdgeStorageKey, EmbedderEdge, GraphLinkEdgeType, TypedEdge, TypedEdgeStorageKey,
};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use rocksdb::{DBIteratorWithThreadMode, IteratorMode, WriteBatch, DB};
use std::sync::Arc;
use uuid::Uuid;

use super::serialization::{
    deserialize_embedder_edges, deserialize_typed_edge, serialize_embedder_edges,
    serialize_typed_edge,
};
use super::types::{GraphEdgeStats, GraphEdgeStorageError, GraphEdgeStorageResult};
use crate::column_families::cf_names;

/// Repository for graph edge storage operations.
///
/// Provides CRUD operations for K-NN edges and typed edges.
/// All operations follow the FAIL FAST principle - errors are returned, never swallowed.
#[derive(Clone)]
pub struct EdgeRepository {
    db: Arc<DB>,
}

impl EdgeRepository {
    /// Create a new edge repository with the given RocksDB instance.
    ///
    /// # Arguments
    ///
    /// * `db` - Arc-wrapped RocksDB instance with graph edge column families
    ///
    /// # Panics
    ///
    /// Panics if the required column families are not present.
    pub fn new(db: Arc<DB>) -> Self {
        // Verify required column families exist
        let required_cfs = [
            cf_names::EMBEDDER_EDGES,
            cf_names::TYPED_EDGES,
            cf_names::TYPED_EDGES_BY_TYPE,
        ];

        for cf_name in &required_cfs {
            if db.cf_handle(cf_name).is_none() {
                panic!(
                    "Required column family '{}' not found - database may need migration",
                    cf_name
                );
            }
        }

        Self { db }
    }

    // =========================================================================
    // K-NN Edge Operations (embedder_edges CF)
    // =========================================================================

    /// Store K-NN edges for a source node in a specific embedder space.
    ///
    /// Replaces any existing edges for this (embedder, source) pair.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Embedder index (0-13)
    /// * `source` - Source node UUID
    /// * `edges` - K-NN neighbor edges (typically k=20)
    pub fn store_embedder_edges(
        &self,
        embedder_id: u8,
        source: Uuid,
        edges: &[EmbedderEdge],
    ) -> GraphEdgeStorageResult<()> {
        if embedder_id as usize >= NUM_EMBEDDERS {
            return Err(GraphEdgeStorageError::InvalidEmbedderId { embedder_id });
        }

        let cf = self.db.cf_handle(cf_names::EMBEDDER_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::EMBEDDER_EDGES,
            },
        )?;

        let key = EdgeStorageKey::new(embedder_id, source);
        let value = serialize_embedder_edges(edges)?;

        self.db.put_cf(&cf, key.to_bytes(), value).map_err(|e| {
            GraphEdgeStorageError::rocksdb("store_embedder_edges", cf_names::EMBEDDER_EDGES, e)
        })
    }

    /// Get K-NN edges for a source node in a specific embedder space.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Embedder index (0-13)
    /// * `source` - Source node UUID
    ///
    /// # Returns
    ///
    /// Vector of neighbor edges, or empty vector if no edges exist.
    pub fn get_embedder_edges(
        &self,
        embedder_id: u8,
        source: Uuid,
    ) -> GraphEdgeStorageResult<Vec<EmbedderEdge>> {
        if embedder_id as usize >= NUM_EMBEDDERS {
            return Err(GraphEdgeStorageError::InvalidEmbedderId { embedder_id });
        }

        let cf = self.db.cf_handle(cf_names::EMBEDDER_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::EMBEDDER_EDGES,
            },
        )?;

        let key = EdgeStorageKey::new(embedder_id, source);

        match self.db.get_cf(&cf, key.to_bytes()) {
            Ok(Some(data)) => deserialize_embedder_edges(&data, source, embedder_id),
            Ok(None) => Ok(vec![]),
            Err(e) => Err(GraphEdgeStorageError::rocksdb(
                "get_embedder_edges",
                cf_names::EMBEDDER_EDGES,
                e,
            )),
        }
    }

    /// Check if the edge repository has any stored K-NN edges.
    ///
    /// Returns `true` if the embedder_edges column family has no entries.
    pub fn is_empty(&self) -> GraphEdgeStorageResult<bool> {
        let cf = self.db.cf_handle(cf_names::EMBEDDER_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::EMBEDDER_EDGES,
            },
        )?;
        let mut iter = self.db.raw_iterator_cf(&cf);
        iter.seek_to_first();
        Ok(!iter.valid())
    }

    /// Delete K-NN edges for a source node in a specific embedder space.
    pub fn delete_embedder_edges(
        &self,
        embedder_id: u8,
        source: Uuid,
    ) -> GraphEdgeStorageResult<()> {
        if embedder_id as usize >= NUM_EMBEDDERS {
            return Err(GraphEdgeStorageError::InvalidEmbedderId { embedder_id });
        }

        let cf = self.db.cf_handle(cf_names::EMBEDDER_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::EMBEDDER_EDGES,
            },
        )?;

        let key = EdgeStorageKey::new(embedder_id, source);

        self.db.delete_cf(&cf, key.to_bytes()).map_err(|e| {
            GraphEdgeStorageError::rocksdb("delete_embedder_edges", cf_names::EMBEDDER_EDGES, e)
        })
    }

    /// Iterate over all K-NN edges for a specific embedder.
    ///
    /// Returns an iterator that yields (source_uuid, Vec<EmbedderEdge>) pairs.
    pub fn iter_embedder_edges(
        &self,
        embedder_id: u8,
    ) -> GraphEdgeStorageResult<
        impl Iterator<Item = GraphEdgeStorageResult<(Uuid, Vec<EmbedderEdge>)>> + '_,
    > {
        if embedder_id as usize >= NUM_EMBEDDERS {
            return Err(GraphEdgeStorageError::InvalidEmbedderId { embedder_id });
        }

        let cf = self.db.cf_handle(cf_names::EMBEDDER_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::EMBEDDER_EDGES,
            },
        )?;

        // Use prefix iterator for this embedder
        let prefix = EdgeStorageKey::embedder_prefix(embedder_id);
        let iter = self.db.prefix_iterator_cf(&cf, prefix);

        Ok(EmbedderEdgeIterator {
            inner: iter,
            embedder_id,
            prefix: prefix[0],
        })
    }

    // =========================================================================
    // Typed Edge Operations (typed_edges CF)
    // =========================================================================

    /// Store a typed edge.
    ///
    /// Also updates the secondary index (typed_edges_by_type).
    pub fn store_typed_edge(&self, edge: &TypedEdge) -> GraphEdgeStorageResult<()> {
        let typed_cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        let by_type_cf = self.db.cf_handle(cf_names::TYPED_EDGES_BY_TYPE).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES_BY_TYPE,
            },
        )?;

        let primary_key = TypedEdgeStorageKey::new(edge.source(), edge.target());
        let value = serialize_typed_edge(edge)?;

        // Secondary index key: [edge_type: u8][source: 16 bytes][target: 16 bytes]
        // Target included in key so multiple edges of same type from same source don't overwrite.
        let mut secondary_key = [0u8; 33];
        secondary_key[0] = edge.edge_type() as u8;
        secondary_key[1..17].copy_from_slice(edge.source().as_bytes());
        secondary_key[17..33].copy_from_slice(edge.target().as_bytes());

        // Batch write both the primary edge and secondary index
        let mut batch = WriteBatch::default();
        batch.put_cf(&typed_cf, primary_key.to_bytes(), &value);
        batch.put_cf(&by_type_cf, secondary_key, edge.target().as_bytes());

        self.db.write(batch).map_err(|e| {
            GraphEdgeStorageError::rocksdb("store_typed_edge", cf_names::TYPED_EDGES, e)
        })
    }

    /// Get a typed edge by source and target.
    pub fn get_typed_edge(
        &self,
        source: Uuid,
        target: Uuid,
    ) -> GraphEdgeStorageResult<Option<TypedEdge>> {
        let cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        let key = TypedEdgeStorageKey::new(source, target);

        match self.db.get_cf(&cf, key.to_bytes()) {
            Ok(Some(data)) => Ok(Some(deserialize_typed_edge(&data)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(GraphEdgeStorageError::rocksdb(
                "get_typed_edge",
                cf_names::TYPED_EDGES,
                e,
            )),
        }
    }

    /// Delete a typed edge.
    ///
    /// Also removes from secondary index if it exists.
    pub fn delete_typed_edge(&self, source: Uuid, target: Uuid) -> GraphEdgeStorageResult<()> {
        // First get the edge to know its type for secondary index
        let edge = self.get_typed_edge(source, target)?;

        let typed_cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        let by_type_cf = self.db.cf_handle(cf_names::TYPED_EDGES_BY_TYPE).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES_BY_TYPE,
            },
        )?;

        let primary_key = TypedEdgeStorageKey::new(source, target);

        let mut batch = WriteBatch::default();
        batch.delete_cf(&typed_cf, primary_key.to_bytes());

        // Delete from secondary index if edge existed
        if let Some(e) = edge {
            let mut secondary_key = [0u8; 33];
            secondary_key[0] = e.edge_type() as u8;
            secondary_key[1..17].copy_from_slice(source.as_bytes());
            secondary_key[17..33].copy_from_slice(target.as_bytes());
            batch.delete_cf(&by_type_cf, secondary_key);
        }

        self.db.write(batch).map_err(|e| {
            GraphEdgeStorageError::rocksdb("delete_typed_edge", cf_names::TYPED_EDGES, e)
        })
    }

    /// Get all typed edges from a source node.
    pub fn get_typed_edges_from(&self, source: Uuid) -> GraphEdgeStorageResult<Vec<TypedEdge>> {
        let cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        // Use prefix iterator on source UUID
        let prefix = source.as_bytes();
        let iter = self.db.prefix_iterator_cf(&cf, prefix);

        let mut edges = Vec::new();
        for item in iter {
            let (key, value) = item.map_err(|e| {
                GraphEdgeStorageError::rocksdb("get_typed_edges_from", cf_names::TYPED_EDGES, e)
            })?;

            // Stop if we've gone past this source's prefix
            if key.len() < 16 || &key[0..16] != source.as_bytes() {
                break;
            }

            let edge = deserialize_typed_edge(&value)?;
            edges.push(edge);
        }

        Ok(edges)
    }

    /// Get all typed edges of a specific type from a source.
    pub fn get_typed_edges_by_type(
        &self,
        source: Uuid,
        edge_type: GraphLinkEdgeType,
    ) -> GraphEdgeStorageResult<Vec<TypedEdge>> {
        let by_type_cf = self.db.cf_handle(cf_names::TYPED_EDGES_BY_TYPE).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES_BY_TYPE,
            },
        )?;

        let typed_cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        // Query secondary index using [edge_type][source] as prefix
        // Full key is [edge_type: u8][source: 16][target: 16] = 33 bytes,
        // but we iterate with 17-byte prefix to get all targets for this (type, source).
        let mut prefix = [0u8; 17];
        prefix[0] = edge_type as u8;
        prefix[1..17].copy_from_slice(source.as_bytes());

        let iter = self.db.prefix_iterator_cf(&by_type_cf, prefix);

        let mut edges = Vec::new();
        for item in iter {
            let (key, value) = item.map_err(|e| {
                GraphEdgeStorageError::rocksdb(
                    "get_typed_edges_by_type",
                    cf_names::TYPED_EDGES_BY_TYPE,
                    e,
                )
            })?;

            // Stop if we've gone past this (type, source) prefix
            if key.len() < 17 || key[0..17] != prefix {
                break;
            }

            // Target UUID: extract from key (bytes 17..33) if available, else from value
            let target = if key.len() >= 33 {
                let target_bytes: [u8; 16] = key[17..33].try_into().map_err(|_| {
                    GraphEdgeStorageError::deserialization(
                        "get_typed_edges_by_type",
                        "invalid target UUID in key",
                    )
                })?;
                Uuid::from_bytes(target_bytes)
            } else {
                // Legacy 17-byte key format: target is in value
                let target_bytes: [u8; 16] = value[0..16].try_into().map_err(|_| {
                    GraphEdgeStorageError::deserialization(
                        "get_typed_edges_by_type",
                        "invalid target UUID",
                    )
                })?;
                Uuid::from_bytes(target_bytes)
            };

            // Fetch full edge from primary storage
            let primary_key = TypedEdgeStorageKey::new(source, target);
            if let Some(data) = self
                .db
                .get_cf(&typed_cf, primary_key.to_bytes())
                .map_err(|e| {
                    GraphEdgeStorageError::rocksdb(
                        "get_typed_edges_by_type",
                        cf_names::TYPED_EDGES,
                        e,
                    )
                })?
            {
                let edge = deserialize_typed_edge(&data)?;
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    /// Get all typed edges pointing TO a target node (incoming edges).
    ///
    /// HIGH-11 FIX: No secondary index exists for target lookups, so this
    /// performs a full CF scan and filters by target UUID in deserialized edges.
    /// This is O(n) over all typed edges.
    pub fn get_typed_edges_to(&self, target: Uuid) -> GraphEdgeStorageResult<Vec<TypedEdge>> {
        let cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        let mut edges = Vec::new();
        for item in iter {
            let (_key, value) = item.map_err(|e| {
                GraphEdgeStorageError::rocksdb("get_typed_edges_to", cf_names::TYPED_EDGES, e)
            })?;

            let edge = deserialize_typed_edge(&value)?;
            if edge.target() == target {
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    /// Iterate all typed edges in the CF, returning them as a Vec.
    ///
    /// Used by the training-export handler to build a one-time reverse
    /// index `HashMap<target_id, Vec<TypedEdge>>` when `includeIncomingEdges`
    /// is requested (avoids an O(N²) `get_typed_edges_to` scan per memory).
    ///
    /// O(n) over `CF_TYPED_EDGES`.
    pub fn iter_all_typed_edges(&self) -> GraphEdgeStorageResult<Vec<TypedEdge>> {
        let cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        let mut edges = Vec::new();
        for item in iter {
            let (_key, value) = item.map_err(|e| {
                GraphEdgeStorageError::rocksdb("iter_all_typed_edges", cf_names::TYPED_EDGES, e)
            })?;
            edges.push(deserialize_typed_edge(&value)?);
        }
        Ok(edges)
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Store multiple typed edges in a single batch.
    pub fn store_typed_edges_batch(&self, edges: &[TypedEdge]) -> GraphEdgeStorageResult<()> {
        if edges.is_empty() {
            return Ok(());
        }

        let typed_cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        let by_type_cf = self.db.cf_handle(cf_names::TYPED_EDGES_BY_TYPE).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES_BY_TYPE,
            },
        )?;

        let mut batch = WriteBatch::default();

        for edge in edges {
            let primary_key = TypedEdgeStorageKey::new(edge.source(), edge.target());
            let value = serialize_typed_edge(edge)?;

            // Secondary index key: [edge_type: u8][source: 16][target: 16] = 33 bytes
            let mut secondary_key = [0u8; 33];
            secondary_key[0] = edge.edge_type() as u8;
            secondary_key[1..17].copy_from_slice(edge.source().as_bytes());
            secondary_key[17..33].copy_from_slice(edge.target().as_bytes());

            batch.put_cf(&typed_cf, primary_key.to_bytes(), &value);
            batch.put_cf(&by_type_cf, secondary_key, edge.target().as_bytes());
        }

        self.db.write(batch).map_err(|e| {
            GraphEdgeStorageError::rocksdb("store_typed_edges_batch", cf_names::TYPED_EDGES, e)
        })
    }

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get statistics about stored edges.
    pub fn get_stats(&self) -> GraphEdgeStorageResult<GraphEdgeStats> {
        let mut stats = GraphEdgeStats::default();

        // Count embedder edges per embedder
        for embedder_id in 0..NUM_EMBEDDERS as u8 {
            let count = self.count_embedder_edges(embedder_id)?;
            stats.embedder_edge_counts[embedder_id as usize] = count;
            stats.total_embedder_edges += count;
        }

        // Count typed edges
        stats.typed_edge_count = self.count_typed_edges()?;

        Ok(stats)
    }

    fn count_embedder_edges(&self, embedder_id: u8) -> GraphEdgeStorageResult<u64> {
        let cf = self.db.cf_handle(cf_names::EMBEDDER_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::EMBEDDER_EDGES,
            },
        )?;

        let prefix = EdgeStorageKey::embedder_prefix(embedder_id);
        let iter = self.db.prefix_iterator_cf(&cf, prefix);

        let mut count = 0u64;
        for item in iter {
            let (key, _) = item.map_err(|e| {
                GraphEdgeStorageError::rocksdb("count_embedder_edges", cf_names::EMBEDDER_EDGES, e)
            })?;

            // Stop if we've gone past this embedder's prefix
            if key.is_empty() || key[0] != embedder_id {
                break;
            }
            count += 1;
        }

        Ok(count)
    }

    fn count_typed_edges(&self) -> GraphEdgeStorageResult<u64> {
        let cf = self.db.cf_handle(cf_names::TYPED_EDGES).ok_or(
            GraphEdgeStorageError::ColumnFamilyNotFound {
                name: cf_names::TYPED_EDGES,
            },
        )?;

        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        let mut count = 0u64;
        for item in iter {
            let _ = item.map_err(|e| {
                GraphEdgeStorageError::rocksdb("count_typed_edges", cf_names::TYPED_EDGES, e)
            })?;
            count += 1;
        }

        Ok(count)
    }
}

/// Iterator for embedder edges.
struct EmbedderEdgeIterator<'a> {
    inner: DBIteratorWithThreadMode<'a, DB>,
    embedder_id: u8,
    prefix: u8,
}

impl<'a> Iterator for EmbedderEdgeIterator<'a> {
    type Item = GraphEdgeStorageResult<(Uuid, Vec<EmbedderEdge>)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok((key, value)) => {
                // Check if still within this embedder's prefix
                if key.is_empty() || key[0] != self.prefix {
                    return None;
                }

                // Parse source UUID from key
                if key.len() < 17 {
                    return Some(Err(GraphEdgeStorageError::InvalidKeyFormat {
                        expected: 17,
                        actual: key.len(),
                    }));
                }

                let uuid_bytes: [u8; 16] = match key[1..17].try_into() {
                    Ok(b) => b,
                    Err(_) => {
                        return Some(Err(GraphEdgeStorageError::deserialization(
                            "iter_embedder_edges",
                            "invalid UUID in key",
                        )))
                    }
                };
                let source = Uuid::from_bytes(uuid_bytes);

                // Deserialize edges
                match deserialize_embedder_edges(&value, source, self.embedder_id) {
                    Ok(edges) => Some(Ok((source, edges))),
                    Err(e) => Some(Err(e)),
                }
            }
            Err(e) => Some(Err(GraphEdgeStorageError::rocksdb(
                "iter_embedder_edges",
                cf_names::EMBEDDER_EDGES,
                e,
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column_families::get_column_family_descriptors;
    use context_graph_core::graph_linking::DirectedRelation;
    use rocksdb::{Cache, Options};
    use std::sync::OnceLock;
    use tempfile::TempDir;

    /// Shared test database — every test uses its own `Uuid::new_v4()` keys,
    /// so there is no cross-test interference and we amortise the ~100ms
    /// RocksDB open cost across the whole module.
    struct SharedDb {
        _temp_dir: TempDir,
        db: Arc<DB>,
    }

    fn shared_db() -> &'static SharedDb {
        static DB: OnceLock<SharedDb> = OnceLock::new();
        DB.get_or_init(|| {
            let temp_dir = TempDir::new().unwrap();
            let cache = Cache::new_lru_cache(256 * 1024 * 1024);
            let descriptors = get_column_family_descriptors(&cache);

            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);

            let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors).unwrap();
            SharedDb {
                _temp_dir: temp_dir,
                db: Arc::new(db),
            }
        })
    }

    fn default_scores() -> [f32; 14] {
        [0.0; 14]
    }

    fn default_thresholds() -> [f32; 14] {
        [0.5; 14]
    }

    #[test]
    fn test_store_and_get_embedder_edges() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let source = Uuid::new_v4();
        let edges: Vec<EmbedderEdge> = (0..5)
            .map(|i| EmbedderEdge::from_storage(source, Uuid::new_v4(), 0, 0.9 - (i as f32 * 0.1)))
            .collect();

        // Store
        repo.store_embedder_edges(0, source, &edges).unwrap();

        // Get
        let retrieved = repo.get_embedder_edges(0, source).unwrap();
        assert_eq!(retrieved.len(), 5);

        for (orig, got) in edges.iter().zip(retrieved.iter()) {
            assert_eq!(orig.target(), got.target());
            assert!((orig.similarity() - got.similarity()).abs() < 0.0001);
        }
    }

    #[test]
    fn test_get_nonexistent_embedder_edges() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let source = Uuid::new_v4();
        let edges = repo.get_embedder_edges(0, source).unwrap();
        assert!(edges.is_empty());
    }

    #[test]
    fn test_store_and_get_typed_edge() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let mut scores = default_scores();
        scores[0] = 0.85; // E1

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::SemanticSimilar,
            0.85,
            DirectedRelation::Symmetric,
            scores,
            1,
            0b0000_0000_0001, // E1 only
        )
        .unwrap();

        // Store
        repo.store_typed_edge(&edge).unwrap();

        // Get
        let retrieved = repo.get_typed_edge(source, target).unwrap().unwrap();
        assert_eq!(retrieved.source(), source);
        assert_eq!(retrieved.target(), target);
        assert_eq!(retrieved.edge_type(), GraphLinkEdgeType::SemanticSimilar);
    }

    #[test]
    fn test_get_typed_edges_from_source() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let source = Uuid::new_v4();

        // Store multiple edges from same source
        for i in 0..3 {
            let mut scores = default_scores();
            scores[0] = 0.8 - (i as f32 * 0.1);

            let edge = TypedEdge::new(
                source,
                Uuid::new_v4(),
                GraphLinkEdgeType::SemanticSimilar,
                scores[0],
                DirectedRelation::Symmetric,
                scores,
                1,
                0b0000_0000_0001, // E1 only
            )
            .unwrap();
            repo.store_typed_edge(&edge).unwrap();
        }

        let edges = repo.get_typed_edges_from(source).unwrap();
        assert_eq!(edges.len(), 3);
    }

    #[test]
    fn test_delete_typed_edge() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let mut scores = default_scores();
        scores[6] = 0.75; // E7 code

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::CodeRelated,
            0.75,
            DirectedRelation::Symmetric,
            scores,
            1,
            0b0000_0100_0000, // E7 only
        )
        .unwrap();

        repo.store_typed_edge(&edge).unwrap();
        assert!(repo.get_typed_edge(source, target).unwrap().is_some());

        repo.delete_typed_edge(source, target).unwrap();
        assert!(repo.get_typed_edge(source, target).unwrap().is_none());
    }

    #[test]
    fn test_invalid_embedder_id() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let result = repo.get_embedder_edges(14, Uuid::new_v4());
        assert!(matches!(
            result,
            Err(GraphEdgeStorageError::InvalidEmbedderId { embedder_id: 14 })
        ));
    }

    #[test]
    fn test_batch_store_typed_edges() {
        let repo = EdgeRepository::new(shared_db().db.clone());

        let source = Uuid::new_v4();
        let mut scores = default_scores();
        let thresholds = default_thresholds();

        let edges: Vec<TypedEdge> = (0..10)
            .map(|i| {
                scores[0] = 0.9 - (i as f32 * 0.05);
                TypedEdge::from_scores(
                    source,
                    Uuid::new_v4(),
                    scores,
                    &thresholds,
                    DirectedRelation::Symmetric,
                )
                .unwrap()
            })
            .collect();

        repo.store_typed_edges_batch(&edges).unwrap();

        let retrieved = repo.get_typed_edges_from(source).unwrap();
        assert_eq!(retrieved.len(), 10);
    }
}
