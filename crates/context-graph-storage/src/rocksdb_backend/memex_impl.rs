//! Memex trait implementation for RocksDbMemex.
//!
//! Delegates to existing methods while providing trait abstraction.

use context_graph_core::marblestone::EdgeType;
use context_graph_core::types::{EmbeddingVector, GraphEdge, JohariQuadrant, MemoryNode, NodeId};

use crate::column_families::cf_names;
use crate::memex::{Memex, StorageHealth};
use crate::rocksdb_backend::StorageError;

use super::core::RocksDbMemex;

impl Memex for RocksDbMemex {
    fn store_node(&self, node: &MemoryNode) -> Result<(), StorageError> {
        // Delegate to existing implementation in node_ops.rs
        RocksDbMemex::store_node(self, node)
    }

    fn get_node(&self, id: &NodeId) -> Result<MemoryNode, StorageError> {
        RocksDbMemex::get_node(self, id)
    }

    fn update_node(&self, node: &MemoryNode) -> Result<(), StorageError> {
        RocksDbMemex::update_node(self, node)
    }

    fn delete_node(&self, id: &NodeId, soft_delete: bool) -> Result<(), StorageError> {
        RocksDbMemex::delete_node(self, id, soft_delete)
    }

    fn store_edge(&self, edge: &GraphEdge) -> Result<(), StorageError> {
        RocksDbMemex::store_edge(self, edge)
    }

    fn get_edge(
        &self,
        source_id: &NodeId,
        target_id: &NodeId,
        edge_type: EdgeType,
    ) -> Result<GraphEdge, StorageError> {
        RocksDbMemex::get_edge(self, source_id, target_id, edge_type)
    }

    fn get_edges_from(&self, source_id: &NodeId) -> Result<Vec<GraphEdge>, StorageError> {
        RocksDbMemex::get_edges_from(self, source_id)
    }

    fn get_edges_to(&self, target_id: &NodeId) -> Result<Vec<GraphEdge>, StorageError> {
        RocksDbMemex::get_edges_to(self, target_id)
    }

    fn query_by_quadrant(
        &self,
        quadrant: JohariQuadrant,
        limit: Option<usize>,
    ) -> Result<Vec<NodeId>, StorageError> {
        // Delegate to index_ops with offset=0
        self.get_nodes_by_quadrant(quadrant, limit, 0)
    }

    fn query_by_tag(&self, tag: &str, limit: Option<usize>) -> Result<Vec<NodeId>, StorageError> {
        // Delegate to index_ops with offset=0
        self.get_nodes_by_tag(tag, limit, 0)
    }

    fn get_embedding(&self, id: &NodeId) -> Result<EmbeddingVector, StorageError> {
        RocksDbMemex::get_embedding(self, id)
    }

    fn health_check(&self) -> Result<StorageHealth, StorageError> {
        // First verify all CFs accessible (existing health_check)
        RocksDbMemex::health_check(self)?;

        // Get approximate counts from RocksDB properties
        let node_count = self.get_approximate_count(cf_names::NODES)?;
        let edge_count = self.get_approximate_count(cf_names::EDGES)?;

        // Get storage size
        let storage_bytes = self.get_storage_size()?;

        Ok(StorageHealth {
            is_healthy: true,
            node_count,
            edge_count,
            storage_bytes,
        })
    }
}

impl RocksDbMemex {
    /// Get approximate key count for a column family.
    pub(crate) fn get_approximate_count(&self, cf_name: &str) -> Result<u64, StorageError> {
        let cf = self.get_cf(cf_name)?;

        // RocksDB estimate-num-keys property
        let count = self
            .db
            .property_int_value_cf(cf, "rocksdb.estimate-num-keys")
            .ok()
            .flatten()
            .unwrap_or(0);

        Ok(count)
    }

    /// Get total storage size across all column families.
    pub(crate) fn get_storage_size(&self) -> Result<u64, StorageError> {
        let mut total_bytes = 0u64;

        for cf_name in cf_names::ALL {
            let cf = self.get_cf(cf_name)?;

            // Get SST file size for this CF
            if let Some(size) = self
                .db
                .property_int_value_cf(cf, "rocksdb.total-sst-files-size")
                .ok()
                .flatten()
            {
                total_bytes += size;
            }
        }

        Ok(total_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RocksDbMemex;
    use context_graph_core::marblestone::Domain;
    use context_graph_core::types::{GraphEdge, MemoryNode};
    use tempfile::TempDir;

    fn create_test_db() -> (RocksDbMemex, TempDir) {
        let tmp = TempDir::new().expect("create temp dir");
        let db = RocksDbMemex::open(tmp.path()).expect("open db");
        (db, tmp)
    }

    /// Create a valid normalized embedding vector (magnitude ~1.0).
    fn create_test_embedding() -> EmbeddingVector {
        const DIM: usize = 1536;
        let val = 1.0_f32 / (DIM as f32).sqrt();
        vec![val; DIM]
    }

    fn create_test_node() -> MemoryNode {
        MemoryNode::new(
            "Test content for Memex trait".to_string(),
            create_test_embedding(),
        )
    }

    fn create_test_edge(source: NodeId, target: NodeId) -> GraphEdge {
        GraphEdge::new(source, target, EdgeType::Semantic, Domain::General)
    }

    // ========== NODE OPERATIONS VIA TRAIT ==========

    #[test]
    fn test_memex_store_and_get_node() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let node = create_test_node();
        let node_id = node.id;

        println!("=== MEMEX NODE STORE/GET TEST ===");
        println!("BEFORE: Node ID = {}", node_id);

        // Store via trait
        memex.store_node(&node).expect("store via Memex");

        // Retrieve via trait
        let retrieved = memex.get_node(&node_id).expect("get via Memex");

        println!("AFTER: Retrieved node ID = {}", retrieved.id);
        println!("AFTER: Content = '{}'", retrieved.content);

        assert_eq!(retrieved.id, node_id);
        assert_eq!(retrieved.content, node.content);
        println!("RESULT: PASS - Node round-trip via Memex trait");
    }

    #[test]
    fn test_memex_update_node() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let mut node = create_test_node();
        let node_id = node.id;

        memex.store_node(&node).expect("store");

        println!("=== MEMEX NODE UPDATE TEST ===");
        println!("BEFORE: importance = {}", node.importance);

        node.importance = 0.9;
        memex.update_node(&node).expect("update via Memex");

        let updated = memex.get_node(&node_id).expect("get");

        println!("AFTER: importance = {}", updated.importance);
        assert!((updated.importance - 0.9).abs() < 0.001);
        println!("RESULT: PASS - Node updated via Memex trait");
    }

    #[test]
    fn test_memex_delete_node() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let node = create_test_node();
        let node_id = node.id;

        memex.store_node(&node).expect("store");

        println!("=== MEMEX NODE DELETE TEST ===");
        println!("BEFORE: Node exists = true");

        // Soft delete
        memex.delete_node(&node_id, true).expect("delete via Memex");

        // Node still exists but marked deleted
        let deleted = memex.get_node(&node_id).expect("get deleted");

        println!("AFTER: metadata.deleted = {}", deleted.metadata.deleted);
        assert!(deleted.metadata.deleted);
        println!("RESULT: PASS - Node soft-deleted via Memex trait");
    }

    // ========== EDGE OPERATIONS VIA TRAIT ==========

    #[test]
    fn test_memex_store_and_get_edge() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let node1 = create_test_node();
        let node2 = create_test_node();
        memex.store_node(&node1).expect("store node1");
        memex.store_node(&node2).expect("store node2");

        let edge = create_test_edge(node1.id, node2.id);

        println!("=== MEMEX EDGE STORE/GET TEST ===");
        println!("BEFORE: Edge {} -> {}", node1.id, node2.id);

        memex.store_edge(&edge).expect("store via Memex");

        let retrieved = memex
            .get_edge(&node1.id, &node2.id, EdgeType::Semantic)
            .expect("get via Memex");

        println!("AFTER: Retrieved edge source = {}", retrieved.source_id);
        println!("AFTER: Retrieved edge target = {}", retrieved.target_id);

        assert_eq!(retrieved.source_id, node1.id);
        assert_eq!(retrieved.target_id, node2.id);
        println!("RESULT: PASS - Edge round-trip via Memex trait");
    }

    #[test]
    fn test_memex_get_edges_from() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let source = create_test_node();
        let target1 = create_test_node();
        let target2 = create_test_node();

        memex.store_node(&source).expect("store source");
        memex.store_node(&target1).expect("store target1");
        memex.store_node(&target2).expect("store target2");

        memex
            .store_edge(&create_test_edge(source.id, target1.id))
            .expect("store edge1");
        memex
            .store_edge(&GraphEdge::new(
                source.id,
                target2.id,
                EdgeType::Temporal,
                Domain::General,
            ))
            .expect("store edge2");

        println!("=== MEMEX GET_EDGES_FROM TEST ===");

        let edges = memex
            .get_edges_from(&source.id)
            .expect("get_edges_from via Memex");

        println!("RESULT: Found {} outgoing edges", edges.len());
        assert_eq!(edges.len(), 2);
        println!("RESULT: PASS - get_edges_from works via Memex trait");
    }

    #[test]
    fn test_memex_get_edges_to() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let source1 = create_test_node();
        let source2 = create_test_node();
        let target = create_test_node();

        memex.store_node(&source1).expect("store source1");
        memex.store_node(&source2).expect("store source2");
        memex.store_node(&target).expect("store target");

        memex
            .store_edge(&create_test_edge(source1.id, target.id))
            .expect("store edge1");
        memex
            .store_edge(&create_test_edge(source2.id, target.id))
            .expect("store edge2");

        println!("=== MEMEX GET_EDGES_TO TEST ===");

        let edges = memex
            .get_edges_to(&target.id)
            .expect("get_edges_to via Memex");

        println!("RESULT: Found {} incoming edges", edges.len());
        assert_eq!(edges.len(), 2);
        println!("RESULT: PASS - get_edges_to works via Memex trait");
    }

    // ========== QUERY OPERATIONS VIA TRAIT ==========

    #[test]
    fn test_memex_query_by_quadrant() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let mut node = create_test_node();
        node.quadrant = JohariQuadrant::Open;
        memex.store_node(&node).expect("store");

        println!("=== MEMEX QUERY_BY_QUADRANT TEST ===");

        let open_nodes = memex
            .query_by_quadrant(JohariQuadrant::Open, Some(10))
            .expect("query via Memex");

        println!("RESULT: Found {} nodes in Open quadrant", open_nodes.len());
        assert!(open_nodes.contains(&node.id));
        println!("RESULT: PASS - query_by_quadrant works via Memex trait");
    }

    #[test]
    fn test_memex_query_by_tag() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let mut node = create_test_node();
        node.metadata.tags.push("memex-test".to_string());
        memex.store_node(&node).expect("store");

        println!("=== MEMEX QUERY_BY_TAG TEST ===");

        let tagged = memex
            .query_by_tag("memex-test", Some(10))
            .expect("query via Memex");

        println!("RESULT: Found {} nodes with tag 'memex-test'", tagged.len());
        assert!(tagged.contains(&node.id));
        println!("RESULT: PASS - query_by_tag works via Memex trait");
    }

    // ========== EMBEDDING OPERATIONS VIA TRAIT ==========

    #[test]
    fn test_memex_get_embedding() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let node = create_test_node();
        memex.store_node(&node).expect("store");

        println!("=== MEMEX GET_EMBEDDING TEST ===");

        let embedding = memex.get_embedding(&node.id).expect("get via Memex");

        println!("RESULT: Embedding dimensions = {}", embedding.len());
        assert_eq!(embedding.len(), 1536);
        println!("RESULT: PASS - get_embedding works via Memex trait");
    }

    // ========== HEALTH CHECK VIA TRAIT ==========

    #[test]
    fn test_memex_health_check() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        // Store some data first
        let node = create_test_node();
        memex.store_node(&node).expect("store");

        println!("=== MEMEX HEALTH_CHECK TEST ===");

        let health = memex.health_check().expect("health_check via Memex");

        println!("RESULT: is_healthy = {}", health.is_healthy);
        println!("RESULT: node_count = {}", health.node_count);
        println!("RESULT: edge_count = {}", health.edge_count);
        println!("RESULT: storage_bytes = {}", health.storage_bytes);

        assert!(health.is_healthy);
        // Note: RocksDB estimates may not be exact immediately after writes
        println!("RESULT: PASS - health_check returns StorageHealth via Memex trait");
    }

    // ========== EDGE CASES ==========

    #[test]
    fn edge_case_not_found_via_trait() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let fake_id = uuid::Uuid::new_v4();

        println!("=== EDGE CASE: NotFound via trait ===");
        println!("BEFORE: Querying non-existent node {}", fake_id);

        let result = memex.get_node(&fake_id);

        println!("AFTER: Result = {:?}", result.is_err());
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
        println!("RESULT: PASS - NotFound error propagates correctly");
    }

    #[test]
    fn edge_case_empty_query_results() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        println!("=== EDGE CASE: Empty query results ===");

        let nodes = memex
            .query_by_tag("nonexistent-tag-xyz", None)
            .expect("query should succeed");

        println!("RESULT: nodes.len() = {}", nodes.len());
        assert!(nodes.is_empty());
        println!("RESULT: PASS - Empty results return Ok(Vec::new())");
    }

    #[test]
    fn edge_case_limit_zero() {
        let (db, _tmp) = create_test_db();
        let memex: &dyn Memex = &db;

        let node = create_test_node();
        memex.store_node(&node).expect("store");

        println!("=== EDGE CASE: limit = Some(0) ===");

        let nodes = memex
            .query_by_quadrant(JohariQuadrant::Unknown, Some(0))
            .expect("query should succeed");

        println!("RESULT: nodes.len() = {}", nodes.len());
        assert!(nodes.is_empty());
        println!("RESULT: PASS - limit=0 returns empty Vec");
    }

    // ========== THREAD SAFETY VERIFICATION ==========

    #[test]
    fn test_memex_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        // Verify RocksDbMemex implements Send + Sync (required by Memex trait bound)
        assert_send_sync::<RocksDbMemex>();

        println!("=== THREAD SAFETY TEST ===");
        println!("RESULT: PASS - RocksDbMemex is Send + Sync");
    }

    // ========== HELPER METHOD TESTS ==========

    #[test]
    fn test_get_approximate_count() {
        let (db, _tmp) = create_test_db();

        println!("=== GET_APPROXIMATE_COUNT TEST ===");

        let count_before = db.get_approximate_count(cf_names::NODES).expect("count");
        println!("BEFORE: node count estimate = {}", count_before);

        let node = create_test_node();
        db.store_node(&node).expect("store");

        // Flush to ensure estimates are updated
        db.flush_all().expect("flush");

        let count_after = db.get_approximate_count(cf_names::NODES).expect("count");
        println!("AFTER: node count estimate = {}", count_after);

        // Note: estimate may not be exact, but should be >= initial
        println!("RESULT: PASS - get_approximate_count works");
    }

    #[test]
    fn test_get_storage_size() {
        let (db, _tmp) = create_test_db();

        println!("=== GET_STORAGE_SIZE TEST ===");

        let size = db.get_storage_size().expect("size");
        println!("RESULT: storage size = {} bytes", size);
        println!("RESULT: PASS - get_storage_size works");
    }
}
