//! GraphStorage backend wrapping RocksDB.
//!
//! Provides type-safe persistence for hyperbolic coordinates, entailment cones,
//! and edge adjacency lists.
//!
//! # Constitution Reference
//!
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - rules: Result<T,E> for fallible ops, thiserror for derivation
//!
//! # Binary Formats
//!
//! - PoincarePoint: 256 bytes (64 f32 little-endian)
//! - EntailmentCone: 268 bytes (256 apex + 4 aperture + 4 factor + 4 depth)
//! - NodeId: 8 bytes (i64 little-endian)
//! - Edges: bincode serialized Vec<GraphEdge>

use std::path::Path;
use std::sync::Arc;

use rocksdb::{ColumnFamily, IteratorMode, WriteBatch, DB};

use super::{
    get_column_family_descriptors, get_db_options, StorageConfig, CF_ADJACENCY, CF_CONES,
    CF_HYPERBOLIC, CF_METADATA,
};
use crate::error::{GraphError, GraphResult};

// ========== Type Aliases ==========

/// Node ID type (8 bytes, little-endian)
pub type NodeId = i64;

// ========== Core Types ==========

/// 64D Poincaré ball coordinates
///
/// Represents a point in hyperbolic space using the Poincaré ball model.
/// All coordinates must satisfy ||x|| < 1 (open ball constraint).
///
/// # Binary Format
///
/// 256 bytes: 64 f32 values in little-endian order.
#[derive(Debug, Clone, PartialEq)]
pub struct PoincarePoint {
    /// 64-dimensional coordinates in Poincaré ball.
    pub coords: [f32; 64],
}

impl PoincarePoint {
    /// Create a point at the origin (all zeros).
    #[must_use]
    pub fn origin() -> Self {
        Self { coords: [0.0; 64] }
    }

    /// Create a point from a slice of f32 values.
    ///
    /// # Errors
    ///
    /// Returns `GraphError::DimensionMismatch` if slice length != 64.
    pub fn from_slice(slice: &[f32]) -> GraphResult<Self> {
        if slice.len() != 64 {
            return Err(GraphError::DimensionMismatch {
                expected: 64,
                actual: slice.len(),
            });
        }
        let mut coords = [0.0f32; 64];
        coords.copy_from_slice(slice);
        Ok(Self { coords })
    }

    /// Compute the Euclidean norm of the point.
    #[must_use]
    pub fn norm(&self) -> f32 {
        self.coords
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
    }
}

/// Entailment cone for hierarchical reasoning.
///
/// Represents an entailment cone in hyperbolic space, used for
/// efficient O(1) hierarchy queries via cone containment.
///
/// # Binary Format
///
/// 268 bytes total:
/// - apex: 256 bytes (64 f32 little-endian)
/// - aperture: 4 bytes (f32 little-endian)
/// - aperture_factor: 4 bytes (f32 little-endian)
/// - depth: 4 bytes (u32 little-endian)
#[derive(Debug, Clone)]
pub struct EntailmentCone {
    /// Apex point of the cone in hyperbolic space.
    pub apex: PoincarePoint,
    /// Half-angle aperture in radians (0, π).
    pub aperture: f32,
    /// Factor for adaptive aperture computation.
    pub aperture_factor: f32,
    /// Hierarchy depth (0 = root).
    pub depth: u32,
}

impl EntailmentCone {
    /// Create a default cone at origin with standard aperture.
    #[must_use]
    pub fn default_at_origin() -> Self {
        Self {
            apex: PoincarePoint::origin(),
            aperture: std::f32::consts::FRAC_PI_4, // 45 degrees
            aperture_factor: 1.0,
            depth: 0,
        }
    }
}

/// Graph edge (placeholder until M04-T15).
///
/// Represents a directed edge in the knowledge graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GraphEdge {
    /// Target node ID.
    pub target: NodeId,
    /// Edge type identifier.
    pub edge_type: u8,
    // NT weights to be added in M04-T15
}

// ========== GraphStorage ==========

/// Graph storage backed by RocksDB.
///
/// Thread-safe via Arc<DB>. Clone is cheap (Arc clone).
///
/// # Column Families
///
/// - `hyperbolic`: Poincaré coordinates (256 bytes per node)
/// - `entailment_cones`: Entailment cones (268 bytes per node)
/// - `adjacency`: Edge lists (bincode serialized)
/// - `metadata`: Schema version and statistics
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_graph::storage::{GraphStorage, StorageConfig, PoincarePoint};
///
/// let storage = GraphStorage::open_default("/tmp/graph.db")?;
/// storage.put_hyperbolic(1, &PoincarePoint::origin())?;
/// let point = storage.get_hyperbolic(1)?;
/// assert!(point.is_some());
/// ```
#[derive(Clone)]
pub struct GraphStorage {
    db: Arc<DB>,
}

impl GraphStorage {
    /// Open graph storage at the given path.
    ///
    /// # Arguments
    /// * `path` - Directory path for RocksDB database
    /// * `config` - Storage configuration (use StorageConfig::default())
    ///
    /// # Errors
    /// * `GraphError::StorageOpen` - Failed to open database
    /// * `GraphError::InvalidConfig` - Invalid configuration
    ///
    /// # Example
    /// ```rust,ignore
    /// let storage = GraphStorage::open("/data/graph.db", StorageConfig::default())?;
    /// ```
    pub fn open<P: AsRef<Path>>(path: P, config: StorageConfig) -> GraphResult<Self> {
        let db_opts = get_db_options();
        let cf_descriptors = get_column_family_descriptors(&config)?;

        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), cf_descriptors).map_err(|e| {
            log::error!(
                "Failed to open GraphStorage at {:?}: {}",
                path.as_ref(),
                e
            );
            GraphError::StorageOpen {
                path: path.as_ref().to_string_lossy().into_owned(),
                cause: e.to_string(),
            }
        })?;

        log::info!("GraphStorage opened at {:?}", path.as_ref());

        Ok(Self { db: Arc::new(db) })
    }

    /// Open with default configuration.
    pub fn open_default<P: AsRef<Path>>(path: P) -> GraphResult<Self> {
        Self::open(path, StorageConfig::default())
    }

    // ========== Hyperbolic Point Operations ==========

    /// Get hyperbolic coordinates for a node.
    ///
    /// # Returns
    /// * `Ok(Some(point))` - Point exists
    /// * `Ok(None)` - Node not found
    /// * `Err(GraphError::CorruptedData)` - Invalid data in storage
    pub fn get_hyperbolic(&self, node_id: NodeId) -> GraphResult<Option<PoincarePoint>> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let point = Self::deserialize_point(&bytes).map_err(|e| {
                    log::error!("CORRUPTED: hyperbolic node_id={}: {}", node_id, e);
                    e
                })?;
                Ok(Some(point))
            }
            None => Ok(None),
        }
    }

    /// Store hyperbolic coordinates for a node.
    ///
    /// Overwrites existing coordinates if present.
    pub fn put_hyperbolic(&self, node_id: NodeId, point: &PoincarePoint) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_point(point);

        self.db.put_cf(cf, key, value)?;
        log::trace!("PUT hyperbolic node_id={}", node_id);
        Ok(())
    }

    /// Delete hyperbolic coordinates for a node.
    pub fn delete_hyperbolic(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        log::trace!("DELETE hyperbolic node_id={}", node_id);
        Ok(())
    }

    // ========== Entailment Cone Operations ==========

    /// Get entailment cone for a node.
    pub fn get_cone(&self, node_id: NodeId) -> GraphResult<Option<EntailmentCone>> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let cone = Self::deserialize_cone(&bytes).map_err(|e| {
                    log::error!("CORRUPTED: cone node_id={}: {}", node_id, e);
                    e
                })?;
                Ok(Some(cone))
            }
            None => Ok(None),
        }
    }

    /// Store entailment cone for a node.
    pub fn put_cone(&self, node_id: NodeId, cone: &EntailmentCone) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_cone(cone);

        self.db.put_cf(cf, key, value)?;
        log::trace!("PUT cone node_id={}", node_id);
        Ok(())
    }

    /// Delete entailment cone for a node.
    pub fn delete_cone(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        log::trace!("DELETE cone node_id={}", node_id);
        Ok(())
    }

    // ========== Adjacency List Operations ==========

    /// Get edges for a node.
    ///
    /// Returns empty Vec if node has no edges.
    pub fn get_adjacency(&self, node_id: NodeId) -> GraphResult<Vec<GraphEdge>> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let edges: Vec<GraphEdge> =
                    bincode::deserialize(&bytes).map_err(|e| GraphError::CorruptedData {
                        location: format!("adjacency node_id={}", node_id),
                        details: e.to_string(),
                    })?;
                Ok(edges)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Store edges for a node.
    pub fn put_adjacency(&self, node_id: NodeId, edges: &[GraphEdge]) -> GraphResult<()> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();
        let value = bincode::serialize(edges)?;

        self.db.put_cf(cf, key, value)?;
        log::trace!("PUT adjacency node_id={} edges={}", node_id, edges.len());
        Ok(())
    }

    /// Add a single edge (reads existing, appends, writes back).
    pub fn add_edge(&self, source: NodeId, edge: GraphEdge) -> GraphResult<()> {
        let mut edges = self.get_adjacency(source)?;
        edges.push(edge);
        self.put_adjacency(source, &edges)
    }

    /// Remove an edge by target node.
    ///
    /// Returns true if edge was found and removed.
    pub fn remove_edge(&self, source: NodeId, target: NodeId) -> GraphResult<bool> {
        let mut edges = self.get_adjacency(source)?;
        let original_len = edges.len();
        edges.retain(|e| e.target != target);

        if edges.len() < original_len {
            self.put_adjacency(source, &edges)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete all edges for a node.
    pub fn delete_adjacency(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        log::trace!("DELETE adjacency node_id={}", node_id);
        Ok(())
    }

    // ========== Batch Operations ==========

    /// Perform multiple operations atomically.
    pub fn write_batch(&self, batch: WriteBatch) -> GraphResult<()> {
        self.db.write(batch)?;
        Ok(())
    }

    /// Create a new write batch.
    #[must_use]
    pub fn new_batch(&self) -> WriteBatch {
        WriteBatch::default()
    }

    /// Add hyperbolic point to batch.
    pub fn batch_put_hyperbolic(
        &self,
        batch: &mut WriteBatch,
        node_id: NodeId,
        point: &PoincarePoint,
    ) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_point(point);
        batch.put_cf(cf, key, value);
        Ok(())
    }

    /// Add cone to batch.
    pub fn batch_put_cone(
        &self,
        batch: &mut WriteBatch,
        node_id: NodeId,
        cone: &EntailmentCone,
    ) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_cone(cone);
        batch.put_cf(cf, key, value);
        Ok(())
    }

    /// Add adjacency to batch.
    pub fn batch_put_adjacency(
        &self,
        batch: &mut WriteBatch,
        node_id: NodeId,
        edges: &[GraphEdge],
    ) -> GraphResult<()> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();
        let value = bincode::serialize(edges)?;
        batch.put_cf(cf, key, value);
        Ok(())
    }

    // ========== Iteration ==========

    /// Iterate over all hyperbolic points.
    pub fn iter_hyperbolic(
        &self,
    ) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, PoincarePoint)>> + '_> {
        let cf = self.cf_hyperbolic()?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(
                key[..8]
                    .try_into()
                    .expect("NodeId key must be 8 bytes - storage corrupted"),
            );
            let point = Self::deserialize_point(&value)?;
            Ok((node_id, point))
        }))
    }

    /// Iterate over all cones.
    pub fn iter_cones(
        &self,
    ) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, EntailmentCone)>> + '_> {
        let cf = self.cf_cones()?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(
                key[..8]
                    .try_into()
                    .expect("NodeId key must be 8 bytes - storage corrupted"),
            );
            let cone = Self::deserialize_cone(&value)?;
            Ok((node_id, cone))
        }))
    }

    /// Iterate over all adjacency lists.
    pub fn iter_adjacency(
        &self,
    ) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, Vec<GraphEdge>)>> + '_> {
        let cf = self.cf_adjacency()?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(
                key[..8]
                    .try_into()
                    .expect("NodeId key must be 8 bytes - storage corrupted"),
            );
            let edges: Vec<GraphEdge> =
                bincode::deserialize(&value).map_err(|e| GraphError::CorruptedData {
                    location: format!("adjacency node_id={}", node_id),
                    details: e.to_string(),
                })?;
            Ok((node_id, edges))
        }))
    }

    // ========== Statistics ==========

    /// Get count of hyperbolic points stored.
    pub fn hyperbolic_count(&self) -> GraphResult<usize> {
        let cf = self.cf_hyperbolic()?;
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Get count of cones stored.
    pub fn cone_count(&self) -> GraphResult<usize> {
        let cf = self.cf_cones()?;
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Get count of nodes with adjacency lists.
    pub fn adjacency_count(&self) -> GraphResult<usize> {
        let cf = self.cf_adjacency()?;
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    // ========== Schema Version Operations ==========

    /// Get schema version from metadata CF.
    ///
    /// # Returns
    /// * `Ok(version)` - Current schema version (0 if not set)
    /// * `Err(GraphError::CorruptedData)` - Invalid version data
    pub fn get_schema_version(&self) -> GraphResult<u32> {
        let cf = self.cf_metadata()?;

        match self.db.get_cf(cf, b"schema_version")? {
            Some(bytes) => {
                if bytes.len() != 4 {
                    return Err(GraphError::CorruptedData {
                        location: "metadata/schema_version".to_string(),
                        details: format!("Expected 4 bytes, got {}", bytes.len()),
                    });
                }
                let version = u32::from_le_bytes(
                    bytes[..4]
                        .try_into()
                        .expect("verified 4 bytes above - this cannot fail"),
                );
                log::trace!("get_schema_version: {}", version);
                Ok(version)
            }
            None => {
                log::trace!("get_schema_version: 0 (not set)");
                Ok(0) // No version stored = version 0
            }
        }
    }

    /// Set schema version in metadata CF.
    pub fn set_schema_version(&self, version: u32) -> GraphResult<()> {
        let cf = self.cf_metadata()?;
        self.db
            .put_cf(cf, b"schema_version", version.to_le_bytes())?;
        log::debug!("set_schema_version: {}", version);
        Ok(())
    }

    /// Apply all pending migrations.
    ///
    /// Should be called after open() to ensure database is up to date.
    ///
    /// # Returns
    /// * `Ok(version)` - Final schema version
    /// * `Err(GraphError::MigrationFailed)` - Migration failed
    pub fn apply_migrations(&self) -> GraphResult<u32> {
        let migrations = super::migrations::Migrations::new();
        migrations.migrate(self)
    }

    /// Check if database needs migrations.
    pub fn needs_migrations(&self) -> GraphResult<bool> {
        let migrations = super::migrations::Migrations::new();
        migrations.needs_migration(self)
    }

    /// Open storage and apply migrations.
    ///
    /// Convenience method that combines open() with migrations.
    /// This is the recommended way to open a database in production.
    ///
    /// # Example
    /// ```rust,ignore
    /// let storage = GraphStorage::open_and_migrate(
    ///     "/data/graph.db",
    ///     StorageConfig::default(),
    /// )?;
    /// // Database is now at latest schema version
    /// ```
    pub fn open_and_migrate<P: AsRef<Path>>(path: P, config: StorageConfig) -> GraphResult<Self> {
        log::info!("Opening storage with migrations at {:?}", path.as_ref());

        let storage = Self::open(path, config)?;

        let before_version = storage.get_schema_version()?;
        let after_version = storage.apply_migrations()?;

        log::info!(
            "Storage ready: migrated from v{} to v{}",
            before_version,
            after_version
        );

        Ok(storage)
    }

    // ========== Column Family Helpers ==========

    fn cf_hyperbolic(&self) -> GraphResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_HYPERBOLIC)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_HYPERBOLIC.to_string()))
    }

    fn cf_cones(&self) -> GraphResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_CONES)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_CONES.to_string()))
    }

    fn cf_adjacency(&self) -> GraphResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_ADJACENCY)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_ADJACENCY.to_string()))
    }

    pub(crate) fn cf_metadata(&self) -> GraphResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_METADATA)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_METADATA.to_string()))
    }

    // ========== Serialization ==========

    /// Serialize PoincarePoint to exactly 256 bytes.
    fn serialize_point(point: &PoincarePoint) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(256);
        for coord in &point.coords {
            bytes.extend_from_slice(&coord.to_le_bytes());
        }
        debug_assert_eq!(bytes.len(), 256);
        bytes
    }

    /// Deserialize PoincarePoint from 256 bytes.
    fn deserialize_point(bytes: &[u8]) -> GraphResult<PoincarePoint> {
        if bytes.len() != 256 {
            return Err(GraphError::CorruptedData {
                location: "PoincarePoint".to_string(),
                details: format!("Expected 256 bytes, got {}", bytes.len()),
            });
        }

        let mut coords = [0.0f32; 64];
        for (i, chunk) in bytes.chunks_exact(4).enumerate() {
            coords[i] = f32::from_le_bytes(
                chunk
                    .try_into()
                    .expect("chunks_exact(4) guarantees 4 bytes"),
            );
        }

        Ok(PoincarePoint { coords })
    }

    /// Serialize EntailmentCone to exactly 268 bytes.
    fn serialize_cone(cone: &EntailmentCone) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(268);

        // Apex coordinates (256 bytes)
        for coord in &cone.apex.coords {
            bytes.extend_from_slice(&coord.to_le_bytes());
        }

        // Aperture (4 bytes)
        bytes.extend_from_slice(&cone.aperture.to_le_bytes());

        // Aperture factor (4 bytes)
        bytes.extend_from_slice(&cone.aperture_factor.to_le_bytes());

        // Depth (4 bytes)
        bytes.extend_from_slice(&cone.depth.to_le_bytes());

        debug_assert_eq!(bytes.len(), 268);
        bytes
    }

    /// Deserialize EntailmentCone from 268 bytes.
    fn deserialize_cone(bytes: &[u8]) -> GraphResult<EntailmentCone> {
        if bytes.len() != 268 {
            return Err(GraphError::CorruptedData {
                location: "EntailmentCone".to_string(),
                details: format!("Expected 268 bytes, got {}", bytes.len()),
            });
        }

        let apex = Self::deserialize_point(&bytes[..256])?;
        let aperture = f32::from_le_bytes(
            bytes[256..260]
                .try_into()
                .expect("slice bounds verified above"),
        );
        let aperture_factor = f32::from_le_bytes(
            bytes[260..264]
                .try_into()
                .expect("slice bounds verified above"),
        );
        let depth = u32::from_le_bytes(
            bytes[264..268]
                .try_into()
                .expect("slice bounds verified above"),
        );

        Ok(EntailmentCone {
            apex,
            aperture,
            aperture_factor,
            depth,
        })
    }
}

impl std::fmt::Debug for GraphStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphStorage")
            .field("hyperbolic_count", &self.hyperbolic_count().unwrap_or(0))
            .field("cone_count", &self.cone_count().unwrap_or(0))
            .field("adjacency_count", &self.adjacency_count().unwrap_or(0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poincare_point_origin() {
        let point = PoincarePoint::origin();
        assert_eq!(point.coords, [0.0; 64]);
        assert!((point.norm() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_poincare_point_from_slice() {
        let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
        let point = PoincarePoint::from_slice(&data).expect("valid 64D slice");
        assert!((point.coords[0] - 0.0).abs() < 0.0001);
        assert!((point.coords[63] - 0.63).abs() < 0.0001);
    }

    #[test]
    fn test_poincare_point_from_slice_wrong_dim() {
        let data: Vec<f32> = vec![1.0; 32];
        let result = PoincarePoint::from_slice(&data);
        assert!(result.is_err());
        match result {
            Err(GraphError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 64);
                assert_eq!(actual, 32);
            }
            _ => panic!("Expected DimensionMismatch error"),
        }
    }

    #[test]
    fn test_poincare_point_norm() {
        let mut point = PoincarePoint::origin();
        point.coords[0] = 0.6;
        point.coords[1] = 0.8;
        let norm = point.norm();
        assert!((norm - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_entailment_cone_default() {
        let cone = EntailmentCone::default_at_origin();
        assert_eq!(cone.apex.coords, [0.0; 64]);
        assert!((cone.aperture - std::f32::consts::FRAC_PI_4).abs() < 0.0001);
        assert!((cone.aperture_factor - 1.0).abs() < 0.0001);
        assert_eq!(cone.depth, 0);
    }

    #[test]
    fn test_graph_edge_serialization() {
        let edge = GraphEdge {
            target: 42,
            edge_type: 1,
        };
        let bytes = bincode::serialize(&edge).expect("serialize");
        let deserialized: GraphEdge = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(deserialized.target, 42);
        assert_eq!(deserialized.edge_type, 1);
    }

    #[test]
    fn test_serialize_point_256_bytes() {
        let point = PoincarePoint::origin();
        let bytes = GraphStorage::serialize_point(&point);
        assert_eq!(bytes.len(), 256);
    }

    #[test]
    fn test_serialize_cone_268_bytes() {
        let cone = EntailmentCone::default_at_origin();
        let bytes = GraphStorage::serialize_cone(&cone);
        assert_eq!(bytes.len(), 268);
    }

    #[test]
    fn test_point_roundtrip() {
        let mut point = PoincarePoint::origin();
        point.coords[0] = 0.5;
        point.coords[63] = -0.3;

        let bytes = GraphStorage::serialize_point(&point);
        let restored = GraphStorage::deserialize_point(&bytes).expect("deserialize");

        assert!((restored.coords[0] - 0.5).abs() < 0.0001);
        assert!((restored.coords[63] - (-0.3)).abs() < 0.0001);
    }

    #[test]
    fn test_cone_roundtrip() {
        let mut cone = EntailmentCone::default_at_origin();
        cone.apex.coords[0] = 0.1;
        cone.aperture = 0.5;
        cone.aperture_factor = 2.0;
        cone.depth = 5;

        let bytes = GraphStorage::serialize_cone(&cone);
        let restored = GraphStorage::deserialize_cone(&bytes).expect("deserialize");

        assert!((restored.apex.coords[0] - 0.1).abs() < 0.0001);
        assert!((restored.aperture - 0.5).abs() < 0.0001);
        assert!((restored.aperture_factor - 2.0).abs() < 0.0001);
        assert_eq!(restored.depth, 5);
    }

    #[test]
    fn test_deserialize_point_wrong_size() {
        let bytes = vec![0u8; 100]; // Wrong size
        let result = GraphStorage::deserialize_point(&bytes);
        assert!(result.is_err());
        match result {
            Err(GraphError::CorruptedData { details, .. }) => {
                assert!(details.contains("256"));
            }
            _ => panic!("Expected CorruptedData error"),
        }
    }

    #[test]
    fn test_deserialize_cone_wrong_size() {
        let bytes = vec![0u8; 200]; // Wrong size
        let result = GraphStorage::deserialize_cone(&bytes);
        assert!(result.is_err());
        match result {
            Err(GraphError::CorruptedData { details, .. }) => {
                assert!(details.contains("268"));
            }
            _ => panic!("Expected CorruptedData error"),
        }
    }
}
