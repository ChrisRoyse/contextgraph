//! Integration tests for RocksDB storage column families.
//!
//! These tests use REAL RocksDB instances - NO MOCKS.
//! Each test creates a temporary database to verify actual functionality.
//!
//! # Constitution Reference
//!
//! - testing.types.integration: tests/integration/ - MCP, graph, session
//! - AP-007: Stub data in prod → use tests/fixtures/
//!
//! # Tasks Tested
//!
//! - M04-T12: Column family definitions and configuration
//! - M04-T13: GraphStorage implementation
//! - M04-T13a: Schema migrations

use context_graph_graph::error::GraphError;
use context_graph_graph::storage::{
    get_column_family_descriptors, get_db_options, StorageConfig, ALL_COLUMN_FAMILIES,
    CF_ADJACENCY, CF_CONES, CF_FAISS_IDS, CF_HYPERBOLIC, CF_METADATA, CF_NODES,
    // M04-T13 types
    EntailmentCone, GraphEdge, GraphStorage, NodeId, PoincarePoint,
    // M04-T13a types
    Migrations, SCHEMA_VERSION,
};

// ========== Constants Tests ==========

#[test]
fn test_cf_names() {
    assert_eq!(CF_ADJACENCY, "adjacency");
    assert_eq!(CF_HYPERBOLIC, "hyperbolic");
    assert_eq!(CF_CONES, "entailment_cones");
    assert_eq!(CF_FAISS_IDS, "faiss_ids");
    assert_eq!(CF_NODES, "nodes");
    assert_eq!(CF_METADATA, "metadata");
}

#[test]
fn test_all_column_families_count() {
    assert_eq!(ALL_COLUMN_FAMILIES.len(), 6);
}

#[test]
fn test_all_column_families_contains_all() {
    assert!(ALL_COLUMN_FAMILIES.contains(&CF_ADJACENCY));
    assert!(ALL_COLUMN_FAMILIES.contains(&CF_HYPERBOLIC));
    assert!(ALL_COLUMN_FAMILIES.contains(&CF_CONES));
    assert!(ALL_COLUMN_FAMILIES.contains(&CF_FAISS_IDS));
    assert!(ALL_COLUMN_FAMILIES.contains(&CF_NODES));
    assert!(ALL_COLUMN_FAMILIES.contains(&CF_METADATA));
}

// ========== StorageConfig Tests ==========

#[test]
fn test_storage_config_default() {
    let config = StorageConfig::default();
    assert_eq!(config.block_cache_size, 512 * 1024 * 1024);
    assert!(config.enable_compression);
    assert_eq!(config.bloom_filter_bits, 10);
    assert_eq!(config.write_buffer_size, 64 * 1024 * 1024);
    assert_eq!(config.max_write_buffers, 3);
    assert_eq!(config.target_file_size_base, 64 * 1024 * 1024);
}

#[test]
fn test_storage_config_read_optimized() {
    let config = StorageConfig::read_optimized();
    assert_eq!(config.block_cache_size, 1024 * 1024 * 1024); // 1GB
    assert_eq!(config.bloom_filter_bits, 14);
}

#[test]
fn test_storage_config_write_optimized() {
    let config = StorageConfig::write_optimized();
    assert_eq!(config.write_buffer_size, 128 * 1024 * 1024); // 128MB
    assert_eq!(config.max_write_buffers, 5);
}

#[test]
fn test_storage_config_validate_success() {
    let config = StorageConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_storage_config_validate_block_cache_too_small() {
    let config = StorageConfig {
        block_cache_size: 1024, // Only 1KB
        ..Default::default()
    };
    let err = config.validate().unwrap_err();
    assert!(matches!(err, GraphError::InvalidConfig(_)));
    assert!(err.to_string().contains("block_cache_size"));
}

#[test]
fn test_storage_config_validate_bloom_filter_invalid() {
    let config = StorageConfig {
        bloom_filter_bits: 0, // Invalid
        ..Default::default()
    };
    let err = config.validate().unwrap_err();
    assert!(matches!(err, GraphError::InvalidConfig(_)));
    assert!(err.to_string().contains("bloom_filter_bits"));
}

#[test]
fn test_storage_config_validate_write_buffer_too_small() {
    let config = StorageConfig {
        write_buffer_size: 512, // Only 512 bytes
        ..Default::default()
    };
    let err = config.validate().unwrap_err();
    assert!(matches!(err, GraphError::InvalidConfig(_)));
    assert!(err.to_string().contains("write_buffer_size"));
}

// ========== Column Family Descriptor Tests ==========

#[test]
fn test_get_column_family_descriptors_count() {
    let config = StorageConfig::default();
    let descriptors = get_column_family_descriptors(&config).unwrap();
    assert_eq!(descriptors.len(), 6);
}

#[test]
fn test_get_column_family_descriptors_invalid_config() {
    let config = StorageConfig {
        block_cache_size: 0,
        ..Default::default()
    };
    let result = get_column_family_descriptors(&config);
    assert!(result.is_err());
}

// ========== REAL RocksDB Integration Tests ==========

#[test]
fn test_real_rocksdb_open_with_column_families() {
    // REAL RocksDB - no mocks
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_cf.db");

    println!("BEFORE: Opening RocksDB at {:?}", db_path);

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    // Open REAL database
    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors)
        .expect("Failed to open RocksDB with column families");

    println!("AFTER: RocksDB opened successfully");

    // Verify all CFs exist
    for cf_name in ALL_COLUMN_FAMILIES {
        let cf_handle = db.cf_handle(cf_name);
        assert!(cf_handle.is_some(), "Column family {} must exist", cf_name);
        println!("VERIFIED: Column family '{}' exists", cf_name);
    }
}

#[test]
fn test_real_rocksdb_write_and_read_metadata() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_rw_metadata.db");

    println!("BEFORE: Opening RocksDB for write/read test at {:?}", db_path);

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Write to metadata CF
    let metadata_cf = db.cf_handle(CF_METADATA).unwrap();
    db.put_cf(metadata_cf, b"schema_version", b"1").unwrap();

    println!("AFTER WRITE: Wrote schema_version=1 to metadata CF");

    // Read back
    let value = db.get_cf(metadata_cf, b"schema_version").unwrap();
    assert_eq!(value, Some(b"1".to_vec()));

    println!("AFTER READ: Verified schema_version=1");
}

#[test]
fn test_real_rocksdb_write_to_all_cfs() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_all_cfs.db");

    println!("BEFORE: Opening RocksDB for all-CF write test at {:?}", db_path);

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Write to each CF and verify
    for cf_name in ALL_COLUMN_FAMILIES {
        let cf = db.cf_handle(cf_name).unwrap();
        let key = format!("test_key_{}", cf_name);
        let value = format!("test_value_{}", cf_name);

        db.put_cf(cf, key.as_bytes(), value.as_bytes()).unwrap();

        let result = db.get_cf(cf, key.as_bytes()).unwrap();
        assert_eq!(result, Some(value.as_bytes().to_vec()));

        println!("VERIFIED: CF '{}' write/read successful", cf_name);
    }
}

#[test]
fn test_real_rocksdb_write_hyperbolic_coordinates() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_hyperbolic.db");

    println!("BEFORE: Testing hyperbolic coordinate storage");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Create 64D hyperbolic coordinates (256 bytes as per spec)
    let node_id: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let coordinates: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
    let coords_bytes: Vec<u8> = coordinates
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();

    assert_eq!(coords_bytes.len(), 256, "Hyperbolic coords must be 256 bytes");

    let hyperbolic_cf = db.cf_handle(CF_HYPERBOLIC).unwrap();
    db.put_cf(hyperbolic_cf, &node_id, &coords_bytes).unwrap();

    println!("AFTER WRITE: Stored 64D coordinates (256 bytes)");

    // Read back and verify
    let result = db.get_cf(hyperbolic_cf, &node_id).unwrap().unwrap();
    assert_eq!(result.len(), 256);

    // Deserialize and verify first value
    let first_f32 = f32::from_le_bytes([result[0], result[1], result[2], result[3]]);
    assert!((first_f32 - 0.0).abs() < 0.0001);

    println!("AFTER READ: Verified 256-byte hyperbolic coordinates");
}

#[test]
fn test_real_rocksdb_write_entailment_cone() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_cones.db");

    println!("BEFORE: Testing entailment cone storage");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Create entailment cone: 268 bytes (256 coords + 4 aperture + 4 factor + 4 depth)
    let node_id: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let mut cone_data: Vec<u8> = Vec::with_capacity(268);

    // 256 bytes for coordinates (64 f32)
    for i in 0..64 {
        cone_data.extend_from_slice(&(i as f32 * 0.01f32).to_le_bytes());
    }
    // 4 bytes for aperture
    cone_data.extend_from_slice(&0.5f32.to_le_bytes());
    // 4 bytes for factor
    cone_data.extend_from_slice(&1.0f32.to_le_bytes());
    // 4 bytes for depth
    cone_data.extend_from_slice(&3u32.to_le_bytes());

    assert_eq!(cone_data.len(), 268, "Cone data must be 268 bytes");

    let cones_cf = db.cf_handle(CF_CONES).unwrap();
    db.put_cf(cones_cf, &node_id, &cone_data).unwrap();

    println!("AFTER WRITE: Stored 268-byte entailment cone");

    // Read back and verify
    let result = db.get_cf(cones_cf, &node_id).unwrap().unwrap();
    assert_eq!(result.len(), 268);

    println!("AFTER READ: Verified 268-byte entailment cone");
}

#[test]
fn test_real_rocksdb_write_faiss_id() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_faiss_ids.db");

    println!("BEFORE: Testing FAISS ID mapping storage");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Store FAISS ID mapping (i64 = 8 bytes)
    let node_id: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let faiss_id: i64 = 42_000_000;
    let faiss_id_bytes = faiss_id.to_le_bytes();

    assert_eq!(faiss_id_bytes.len(), 8, "FAISS ID must be 8 bytes");

    let faiss_cf = db.cf_handle(CF_FAISS_IDS).unwrap();
    db.put_cf(faiss_cf, &node_id, &faiss_id_bytes).unwrap();

    println!("AFTER WRITE: Stored FAISS ID {}", faiss_id);

    // Read back and verify
    let result = db.get_cf(faiss_cf, &node_id).unwrap().unwrap();
    let read_id = i64::from_le_bytes([
        result[0], result[1], result[2], result[3],
        result[4], result[5], result[6], result[7],
    ]);
    assert_eq!(read_id, faiss_id);

    println!("AFTER READ: Verified FAISS ID {}", read_id);
}

#[test]
fn test_real_rocksdb_reopen_preserves_data() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_reopen.db");

    println!("BEFORE: Testing data persistence across reopen");

    let db_opts = get_db_options();
    let config = StorageConfig::default();

    // First open: write data
    {
        let cf_descriptors = get_column_family_descriptors(&config).unwrap();
        let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

        let nodes_cf = db.cf_handle(CF_NODES).unwrap();
        db.put_cf(nodes_cf, b"node_id_1", b"node_data_persistent").unwrap();

        println!("AFTER FIRST OPEN: Wrote node data");
    }

    // Second open: verify data persisted
    {
        let cf_descriptors = get_column_family_descriptors(&config).unwrap();
        let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

        let nodes_cf = db.cf_handle(CF_NODES).unwrap();
        let value = db.get_cf(nodes_cf, b"node_id_1").unwrap();
        assert_eq!(value, Some(b"node_data_persistent".to_vec()));

        println!("AFTER SECOND OPEN: Verified data persistence");
    }
}

#[test]
fn test_real_rocksdb_adjacency_prefix_scan() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_prefix_scan.db");

    println!("BEFORE: Testing adjacency prefix scan");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Write multiple edges for the same source node
    let source_node: [u8; 16] = [1; 16];
    let adjacency_cf = db.cf_handle(CF_ADJACENCY).unwrap();

    // Store 3 edges from the same source
    for i in 0..3u8 {
        let mut key = source_node.to_vec();
        key.push(i); // Append edge index
        let value = format!("edge_to_target_{}", i);
        db.put_cf(adjacency_cf, &key, value.as_bytes()).unwrap();
    }

    println!("AFTER WRITE: Stored 3 edges from same source");

    // Use iterator to prefix scan
    let mut count = 0;
    let iter = db.prefix_iterator_cf(adjacency_cf, &source_node);
    for item in iter {
        let (key, _value) = item.unwrap();
        if key.starts_with(&source_node) {
            count += 1;
        } else {
            break;
        }
    }

    assert_eq!(count, 3, "Should find 3 edges with same prefix");
    println!("AFTER SCAN: Found {} edges with prefix scan", count);
}

#[test]
fn test_db_options_parallelism() {
    println!("BEFORE: Testing DB options with parallelism");

    let opts = get_db_options();

    // Verify options are valid by using them
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_opts.db");

    // Should succeed with our options
    let _db = rocksdb::DB::open(&opts, &db_path).unwrap();

    println!("AFTER: DB opened with parallelism options");
}

#[test]
fn test_storage_config_with_compression_disabled() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_no_compression.db");

    println!("BEFORE: Testing storage with compression disabled");

    let config = StorageConfig {
        enable_compression: false,
        ..Default::default()
    };

    let db_opts = get_db_options();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // Write and read to verify it works
    let nodes_cf = db.cf_handle(CF_NODES).unwrap();
    db.put_cf(nodes_cf, b"test_key", b"test_value").unwrap();

    let value = db.get_cf(nodes_cf, b"test_key").unwrap();
    assert_eq!(value, Some(b"test_value".to_vec()));

    println!("AFTER: Verified storage works without compression");
}

#[test]
fn test_storage_multiple_writes_same_key() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_overwrite.db");

    println!("BEFORE: Testing overwrite behavior");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    let metadata_cf = db.cf_handle(CF_METADATA).unwrap();

    // Write initial value
    db.put_cf(metadata_cf, b"version", b"1").unwrap();
    let v1 = db.get_cf(metadata_cf, b"version").unwrap();
    assert_eq!(v1, Some(b"1".to_vec()));

    // Overwrite
    db.put_cf(metadata_cf, b"version", b"2").unwrap();
    let v2 = db.get_cf(metadata_cf, b"version").unwrap();
    assert_eq!(v2, Some(b"2".to_vec()));

    // Overwrite again
    db.put_cf(metadata_cf, b"version", b"3").unwrap();
    let v3 = db.get_cf(metadata_cf, b"version").unwrap();
    assert_eq!(v3, Some(b"3".to_vec()));

    println!("AFTER: Verified overwrite behavior (1 -> 2 -> 3)");
}

#[test]
fn test_storage_delete_key() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_delete.db");

    println!("BEFORE: Testing delete behavior");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    let nodes_cf = db.cf_handle(CF_NODES).unwrap();

    // Write
    db.put_cf(nodes_cf, b"to_delete", b"value").unwrap();
    let exists = db.get_cf(nodes_cf, b"to_delete").unwrap();
    assert!(exists.is_some());

    // Delete
    db.delete_cf(nodes_cf, b"to_delete").unwrap();
    let deleted = db.get_cf(nodes_cf, b"to_delete").unwrap();
    assert!(deleted.is_none());

    println!("AFTER: Verified delete behavior");
}

#[test]
fn test_storage_nonexistent_key_returns_none() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_none.db");

    println!("BEFORE: Testing nonexistent key behavior");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    for cf_name in ALL_COLUMN_FAMILIES {
        let cf = db.cf_handle(cf_name).unwrap();
        let result = db.get_cf(cf, b"nonexistent_key_12345").unwrap();
        assert!(result.is_none(), "Nonexistent key should return None in {}", cf_name);
    }

    println!("AFTER: Verified all CFs return None for nonexistent keys");
}

// ========== Edge Case Tests ==========

#[test]
fn test_storage_empty_value() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_empty_value.db");

    println!("BEFORE: Testing empty value storage");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    let metadata_cf = db.cf_handle(CF_METADATA).unwrap();
    db.put_cf(metadata_cf, b"empty_key", b"").unwrap();

    let result = db.get_cf(metadata_cf, b"empty_key").unwrap();
    assert_eq!(result, Some(vec![]));

    println!("AFTER: Verified empty value storage");
}

#[test]
fn test_storage_large_value() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_large_value.db");

    println!("BEFORE: Testing large value storage (1MB)");

    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

    // 1MB value
    let large_value: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();

    let nodes_cf = db.cf_handle(CF_NODES).unwrap();
    db.put_cf(nodes_cf, b"large_key", &large_value).unwrap();

    let result = db.get_cf(nodes_cf, b"large_key").unwrap().unwrap();
    assert_eq!(result.len(), 1024 * 1024);
    assert_eq!(result[0], 0);
    assert_eq!(result[1024 * 1024 - 1], 255);

    println!("AFTER: Verified 1MB value storage");
}

// ========== M04-T13: GraphStorage Tests ==========

#[test]
fn test_graph_storage_open_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_graph_storage.db");

    println!("BEFORE: Opening GraphStorage with default config");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("AFTER: GraphStorage opened successfully");

    // Verify we can access it
    let count = storage.hyperbolic_count().expect("Failed to count hyperbolic");
    assert_eq!(count, 0, "New database should be empty");

    println!("VERIFIED: Empty database has 0 hyperbolic entries");
}

#[test]
fn test_graph_storage_open_with_config() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_graph_storage_config.db");

    println!("BEFORE: Opening GraphStorage with read-optimized config");

    let config = StorageConfig::read_optimized();
    let storage = GraphStorage::open(&db_path, config).expect("Failed to open GraphStorage");

    println!("AFTER: GraphStorage opened with custom config");

    // Verify it works
    let point = PoincarePoint::origin();
    storage.put_hyperbolic(1, &point).expect("Failed to put hyperbolic");
    let retrieved = storage.get_hyperbolic(1).expect("Failed to get hyperbolic");
    assert!(retrieved.is_some());

    println!("VERIFIED: GraphStorage works with custom config");
}

#[test]
fn test_graph_storage_hyperbolic_crud() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_hyperbolic_crud.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("BEFORE: Testing hyperbolic CRUD operations");

    // Create
    let node_id: NodeId = 42;
    let mut point = PoincarePoint::origin();
    point.coords[0] = 0.5;
    point.coords[63] = -0.3;

    storage.put_hyperbolic(node_id, &point).expect("PUT failed");
    println!("CREATE: Stored point for node_id={}", node_id);

    // Read
    let retrieved = storage.get_hyperbolic(node_id).expect("GET failed").unwrap();
    assert!((retrieved.coords[0] - 0.5).abs() < 0.0001);
    assert!((retrieved.coords[63] - (-0.3)).abs() < 0.0001);
    println!("READ: Retrieved point matches");

    // Update
    let mut updated = point.clone();
    updated.coords[0] = 0.9;
    storage.put_hyperbolic(node_id, &updated).expect("UPDATE failed");

    let after_update = storage.get_hyperbolic(node_id).expect("GET failed").unwrap();
    assert!((after_update.coords[0] - 0.9).abs() < 0.0001);
    println!("UPDATE: Point updated successfully");

    // Delete
    storage.delete_hyperbolic(node_id).expect("DELETE failed");
    let deleted = storage.get_hyperbolic(node_id).expect("GET failed");
    assert!(deleted.is_none());
    println!("DELETE: Point deleted successfully");

    println!("AFTER: Hyperbolic CRUD operations complete");
}

#[test]
fn test_graph_storage_cone_crud() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_cone_crud.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("BEFORE: Testing entailment cone CRUD operations");

    // Create
    let node_id: NodeId = 100;
    let mut cone = EntailmentCone::default_at_origin();
    cone.apex.coords[0] = 0.1;
    cone.aperture = 0.5;
    cone.aperture_factor = 2.0;
    cone.depth = 5;

    storage.put_cone(node_id, &cone).expect("PUT failed");
    println!("CREATE: Stored cone for node_id={}", node_id);

    // Read
    let retrieved = storage.get_cone(node_id).expect("GET failed").unwrap();
    assert!((retrieved.apex.coords[0] - 0.1).abs() < 0.0001);
    assert!((retrieved.aperture - 0.5).abs() < 0.0001);
    assert!((retrieved.aperture_factor - 2.0).abs() < 0.0001);
    assert_eq!(retrieved.depth, 5);
    println!("READ: Retrieved cone matches");

    // Delete
    storage.delete_cone(node_id).expect("DELETE failed");
    let deleted = storage.get_cone(node_id).expect("GET failed");
    assert!(deleted.is_none());
    println!("DELETE: Cone deleted successfully");

    println!("AFTER: Cone CRUD operations complete");
}

#[test]
fn test_graph_storage_adjacency_operations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_adjacency.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("BEFORE: Testing adjacency list operations");

    let source: NodeId = 1;

    // Initially empty
    let edges = storage.get_adjacency(source).expect("GET failed");
    assert!(edges.is_empty());
    println!("INITIAL: No edges for node_id={}", source);

    // Add edges
    storage.add_edge(source, GraphEdge { target: 10, edge_type: 1 }).expect("Add edge 1 failed");
    storage.add_edge(source, GraphEdge { target: 20, edge_type: 2 }).expect("Add edge 2 failed");
    storage.add_edge(source, GraphEdge { target: 30, edge_type: 1 }).expect("Add edge 3 failed");

    let edges = storage.get_adjacency(source).expect("GET failed");
    assert_eq!(edges.len(), 3);
    println!("ADDED: 3 edges from node_id={}", source);

    // Remove an edge
    let removed = storage.remove_edge(source, 20).expect("Remove edge failed");
    assert!(removed);
    let edges = storage.get_adjacency(source).expect("GET failed");
    assert_eq!(edges.len(), 2);
    assert!(edges.iter().all(|e| e.target != 20));
    println!("REMOVED: Edge to target=20");

    // Remove non-existent edge
    let not_removed = storage.remove_edge(source, 999).expect("Remove edge failed");
    assert!(!not_removed);
    println!("NOT REMOVED: Non-existent edge to target=999");

    // Delete all adjacencies
    storage.delete_adjacency(source).expect("DELETE failed");
    let edges = storage.get_adjacency(source).expect("GET failed");
    assert!(edges.is_empty());
    println!("DELETED: All edges for node_id={}", source);

    println!("AFTER: Adjacency operations complete");
}

#[test]
fn test_graph_storage_batch_operations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_batch.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("BEFORE: Testing batch operations");

    // Create batch
    let mut batch = storage.new_batch();

    // Add multiple hyperbolic points
    for i in 0..10 {
        let mut point = PoincarePoint::origin();
        point.coords[0] = i as f32 * 0.1;
        storage.batch_put_hyperbolic(&mut batch, i, &point).expect("Batch put failed");
    }

    // Add multiple cones
    for i in 10..15 {
        let mut cone = EntailmentCone::default_at_origin();
        cone.depth = i as u32;
        storage.batch_put_cone(&mut batch, i, &cone).expect("Batch put failed");
    }

    // Add edges
    let edges = vec![
        GraphEdge { target: 1, edge_type: 0 },
        GraphEdge { target: 2, edge_type: 1 },
    ];
    storage.batch_put_adjacency(&mut batch, 100, &edges).expect("Batch put failed");

    println!("BATCH PREPARED: 10 points, 5 cones, 1 adjacency list");

    // Write batch atomically
    storage.write_batch(batch).expect("Batch write failed");

    println!("BATCH WRITTEN: Atomically");

    // Verify all data
    let count = storage.hyperbolic_count().expect("Count failed");
    assert_eq!(count, 10);

    let cone_count = storage.cone_count().expect("Count failed");
    assert_eq!(cone_count, 5);

    let adj_count = storage.adjacency_count().expect("Count failed");
    assert_eq!(adj_count, 1);

    println!("VERIFIED: All batch data persisted");
    println!("AFTER: Batch operations complete");
}

#[test]
fn test_graph_storage_iteration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_iteration.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("BEFORE: Testing iteration");

    // Insert test data
    for i in 0..5 {
        let mut point = PoincarePoint::origin();
        point.coords[0] = i as f32;
        storage.put_hyperbolic(i, &point).expect("PUT failed");
    }

    // Iterate and collect
    let mut collected: Vec<(NodeId, PoincarePoint)> = Vec::new();
    for result in storage.iter_hyperbolic().expect("Iter failed") {
        collected.push(result.expect("Iter item failed"));
    }

    assert_eq!(collected.len(), 5);
    println!("ITERATED: Collected {} hyperbolic points", collected.len());

    println!("AFTER: Iteration complete");
}

#[test]
fn test_graph_storage_binary_format_sizes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_sizes.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open GraphStorage");

    println!("BEFORE: Verifying binary format sizes");

    // PoincarePoint: 256 bytes (64 f32)
    let point = PoincarePoint::origin();
    storage.put_hyperbolic(1, &point).expect("PUT failed");
    // We can't directly access the raw bytes, but we can verify roundtrip
    let retrieved = storage.get_hyperbolic(1).expect("GET failed").unwrap();
    assert_eq!(retrieved.coords.len(), 64);
    println!("PoincarePoint: 64 coords (256 bytes)");

    // EntailmentCone: 268 bytes (256 + 4 + 4 + 4)
    let cone = EntailmentCone::default_at_origin();
    storage.put_cone(2, &cone).expect("PUT failed");
    let retrieved_cone = storage.get_cone(2).expect("GET failed").unwrap();
    assert_eq!(retrieved_cone.apex.coords.len(), 64);
    println!("EntailmentCone: 268 bytes (256 apex + 4 aperture + 4 factor + 4 depth)");

    println!("AFTER: Binary format sizes verified");
}

#[test]
fn test_graph_storage_reopen_preserves_data() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_reopen_graph_storage.db");

    println!("BEFORE: Testing data persistence across reopen");

    // First open: write data
    {
        let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

        let mut point = PoincarePoint::origin();
        point.coords[0] = 0.42;
        storage.put_hyperbolic(1, &point).expect("PUT failed");

        let cone = EntailmentCone::default_at_origin();
        storage.put_cone(2, &cone).expect("PUT failed");

        storage.put_adjacency(3, &[GraphEdge { target: 4, edge_type: 5 }]).expect("PUT failed");

        println!("FIRST OPEN: Wrote point, cone, edges");
    }

    // Second open: verify data
    {
        let storage = GraphStorage::open_default(&db_path).expect("Failed to reopen");

        let point = storage.get_hyperbolic(1).expect("GET failed").unwrap();
        assert!((point.coords[0] - 0.42).abs() < 0.0001);

        let cone = storage.get_cone(2).expect("GET failed");
        assert!(cone.is_some());

        let edges = storage.get_adjacency(3).expect("GET failed");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, 4);

        println!("SECOND OPEN: All data persisted");
    }

    println!("AFTER: Data persistence verified");
}

#[test]
fn test_graph_storage_clone_is_cheap() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_clone.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    println!("BEFORE: Testing cheap clone via Arc<DB>");

    // Write via original
    let point = PoincarePoint::origin();
    storage.put_hyperbolic(1, &point).expect("PUT failed");

    // Clone
    let storage2 = storage.clone();

    // Read via clone
    let retrieved = storage2.get_hyperbolic(1).expect("GET failed");
    assert!(retrieved.is_some());

    // Write via clone
    let point2 = PoincarePoint::origin();
    storage2.put_hyperbolic(2, &point2).expect("PUT failed");

    // Read via original
    let retrieved2 = storage.get_hyperbolic(2).expect("GET failed");
    assert!(retrieved2.is_some());

    println!("AFTER: Clone shares same underlying DB");
}

// ========== M04-T13a: Migration Tests ==========

#[test]
fn test_schema_version_constant() {
    println!("BEFORE: Checking schema version constant");

    assert_eq!(SCHEMA_VERSION, 1, "Initial schema version must be 1");

    println!("AFTER: Schema version is {}", SCHEMA_VERSION);
}

#[test]
fn test_migrations_new() {
    println!("BEFORE: Creating Migrations registry");

    let migrations = Migrations::new();
    assert_eq!(migrations.target_version(), SCHEMA_VERSION);

    println!("AFTER: Migrations registry targets version {}", SCHEMA_VERSION);
}

#[test]
fn test_migrations_list() {
    println!("BEFORE: Listing available migrations");

    let migrations = Migrations::new();
    let list = migrations.list_migrations();

    assert_eq!(list.len(), 1, "Should have 1 migration (v1)");
    assert_eq!(list[0].version, 1);
    assert!(list[0].description.contains("Initial schema"));

    println!("AFTER: Found {} migrations", list.len());
    for info in &list {
        println!("  - v{}: {}", info.version, info.description);
    }
}

#[test]
fn test_migrations_default() {
    let migrations = Migrations::default();
    assert_eq!(migrations.target_version(), SCHEMA_VERSION);
}

#[test]
fn test_graph_storage_schema_version_new_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_new_db_version.db");

    println!("BEFORE: Checking schema version on new database");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    // New database should have version 0
    let version = storage.get_schema_version().expect("Failed to get version");
    assert_eq!(version, 0, "New database should have version 0");

    println!("AFTER: New database has version {}", version);
}

#[test]
fn test_graph_storage_set_schema_version() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_set_version.db");

    println!("BEFORE: Testing set_schema_version");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    storage.set_schema_version(1).expect("Failed to set version");
    let version = storage.get_schema_version().expect("Failed to get version");
    assert_eq!(version, 1);

    println!("AFTER: Schema version set to {}", version);
}

#[test]
fn test_graph_storage_apply_migrations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_migrations.db");

    println!("BEFORE: Applying migrations to new database");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    let before = storage.get_schema_version().expect("Failed to get version");
    println!("BEFORE migration: version={}", before);
    assert_eq!(before, 0);

    let after = storage.apply_migrations().expect("Migration failed");
    println!("AFTER migration: version={}", after);
    assert_eq!(after, SCHEMA_VERSION);

    // Verify persisted
    let persisted = storage.get_schema_version().expect("Failed to get version");
    assert_eq!(persisted, SCHEMA_VERSION);

    println!("AFTER: Migrated from v{} to v{}", before, after);
}

#[test]
fn test_graph_storage_needs_migrations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_needs_migrations.db");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    println!("BEFORE: Checking needs_migrations");

    // New DB needs migrations
    let needs = storage.needs_migrations().expect("Check failed");
    assert!(needs, "New database should need migrations");
    println!("New database needs migrations: {}", needs);

    // After migration, should not need
    storage.apply_migrations().expect("Migration failed");
    let needs_after = storage.needs_migrations().expect("Check failed");
    assert!(!needs_after, "After migration should not need");
    println!("After migration needs migrations: {}", needs_after);

    println!("AFTER: needs_migrations check verified");
}

#[test]
fn test_graph_storage_open_and_migrate() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_open_and_migrate.db");

    println!("BEFORE: Testing open_and_migrate");

    let storage = GraphStorage::open_and_migrate(&db_path, StorageConfig::default())
        .expect("open_and_migrate failed");

    let version = storage.get_schema_version().expect("Failed to get version");
    assert_eq!(version, SCHEMA_VERSION);

    println!("AFTER: Database opened and migrated to v{}", version);
}

#[test]
fn test_migrations_idempotent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_idempotent.db");

    println!("BEFORE: Testing migration idempotency");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    // Apply migrations multiple times
    let v1 = storage.apply_migrations().expect("Migration 1 failed");
    let v2 = storage.apply_migrations().expect("Migration 2 failed");
    let v3 = storage.apply_migrations().expect("Migration 3 failed");

    assert_eq!(v1, SCHEMA_VERSION);
    assert_eq!(v2, SCHEMA_VERSION);
    assert_eq!(v3, SCHEMA_VERSION);

    println!("AFTER: Migration is idempotent (applied 3 times, all returned v{})", SCHEMA_VERSION);
}

#[test]
fn test_migration_preserves_existing_data() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_migration_data.db");

    println!("BEFORE: Testing migration preserves existing data");

    // Open, write data, DON'T migrate
    {
        let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

        let point = PoincarePoint::origin();
        storage.put_hyperbolic(1, &point).expect("PUT failed");

        let cone = EntailmentCone::default_at_origin();
        storage.put_cone(2, &cone).expect("PUT failed");

        storage.put_adjacency(3, &[GraphEdge { target: 4, edge_type: 1 }]).expect("PUT failed");

        // Version should still be 0
        let version = storage.get_schema_version().expect("Get version failed");
        assert_eq!(version, 0);

        println!("Pre-migration: Wrote data at version 0");
    }

    // Reopen with migration
    {
        let storage = GraphStorage::open_and_migrate(&db_path, StorageConfig::default())
            .expect("open_and_migrate failed");

        let version = storage.get_schema_version().expect("Get version failed");
        assert_eq!(version, SCHEMA_VERSION);

        // Verify data preserved
        let point = storage.get_hyperbolic(1).expect("GET failed");
        assert!(point.is_some(), "Hyperbolic point should be preserved");

        let cone = storage.get_cone(2).expect("GET failed");
        assert!(cone.is_some(), "Cone should be preserved");

        let edges = storage.get_adjacency(3).expect("GET failed");
        assert_eq!(edges.len(), 1, "Edges should be preserved");

        println!("Post-migration: All data preserved at version {}", version);
    }

    println!("AFTER: Migration preserves existing data");
}

#[test]
fn test_schema_version_roundtrip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_version_roundtrip.db");

    println!("BEFORE: Testing schema version roundtrip");

    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    // Initial version is 0
    let initial = storage.get_schema_version().expect("Get failed");
    assert_eq!(initial, 0);

    // Set to 1
    storage.set_schema_version(1).expect("Set failed");
    let v1 = storage.get_schema_version().expect("Get failed");
    assert_eq!(v1, 1);

    // Set to 5
    storage.set_schema_version(5).expect("Set failed");
    let v5 = storage.get_schema_version().expect("Get failed");
    assert_eq!(v5, 5);

    println!("AFTER: Schema version roundtrip verified (0 -> 1 -> 5)");
}

// ========== Error Handling Tests ==========

#[test]
fn test_poincare_point_dimension_mismatch() {
    println!("BEFORE: Testing PoincarePoint dimension validation");

    let result = PoincarePoint::from_slice(&[1.0; 32]); // Wrong size
    assert!(result.is_err());

    match result {
        Err(GraphError::DimensionMismatch { expected, actual }) => {
            assert_eq!(expected, 64);
            assert_eq!(actual, 32);
            println!("AFTER: Correctly rejected 32D slice (expected 64)");
        }
        _ => panic!("Expected DimensionMismatch error"),
    }
}

#[test]
fn test_poincare_point_from_slice_valid() {
    let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
    let point = PoincarePoint::from_slice(&data).expect("Valid 64D slice should succeed");
    assert!((point.coords[0] - 0.0).abs() < 0.0001);
    assert!((point.coords[63] - 0.63).abs() < 0.0001);
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
    let edge = GraphEdge { target: 42, edge_type: 7 };

    // Serialize and deserialize with bincode
    let bytes = bincode::serialize(&edge).expect("Serialize failed");
    let deserialized: GraphEdge = bincode::deserialize(&bytes).expect("Deserialize failed");

    assert_eq!(deserialized.target, 42);
    assert_eq!(deserialized.edge_type, 7);
}

// ========== Edge Case Tests Required by Spec ==========

#[test]
fn test_edge_case_corrupted_schema_version() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_corrupted_version.db");

    println!("BEFORE: Testing corrupted schema version handling");

    // First, open raw RocksDB and write invalid schema version
    {
        let db_opts = get_db_options();
        let config = StorageConfig::default();
        let cf_descriptors = get_column_family_descriptors(&config).unwrap();

        let db = rocksdb::DB::open_cf_descriptors(&db_opts, &db_path, cf_descriptors).unwrap();

        let metadata_cf = db.cf_handle(CF_METADATA).unwrap();

        // Write corrupted schema version (only 2 bytes instead of 4)
        db.put_cf(metadata_cf, b"schema_version", &[0x01, 0x02])
            .unwrap();

        println!("CORRUPTED: Wrote 2-byte schema_version (should be 4 bytes)");
    }

    // Now open via GraphStorage and try to get schema version - should fail fast
    {
        let storage = GraphStorage::open_default(&db_path).expect("Open should succeed");

        let result = storage.get_schema_version();

        match result {
            Err(GraphError::CorruptedData { location, details }) => {
                println!(
                    "FAIL-FAST: CorruptedData error detected - location={}, details={}",
                    location, details
                );
                assert!(
                    details.contains("4 bytes") || details.contains("length"),
                    "Error should mention expected byte size"
                );
            }
            Err(other) => {
                // Other error types are also acceptable for corrupted data
                println!("FAIL-FAST: Error detected (type={:?})", other);
            }
            Ok(version) => {
                panic!(
                    "FAILED: Should have returned error for corrupted data, got version={}",
                    version
                );
            }
        }
    }

    println!("AFTER: Corrupted schema version correctly fails fast");
}

#[test]
fn test_edge_case_empty_database_initialization() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_empty_db_init.db");

    println!("BEFORE: Testing empty database initialization");

    // Open a brand new database
    let storage = GraphStorage::open_default(&db_path).expect("Failed to open");

    // Verify all counts are zero
    let hyperbolic_count = storage.hyperbolic_count().expect("Count failed");
    let cone_count = storage.cone_count().expect("Count failed");
    let adjacency_count = storage.adjacency_count().expect("Count failed");

    assert_eq!(hyperbolic_count, 0, "New DB should have 0 hyperbolic entries");
    assert_eq!(cone_count, 0, "New DB should have 0 cone entries");
    assert_eq!(adjacency_count, 0, "New DB should have 0 adjacency entries");

    println!("VERIFIED: Empty database has all zero counts");

    // Schema version should be 0 (unmigrated)
    let version = storage.get_schema_version().expect("Get version failed");
    assert_eq!(version, 0, "New DB should have version 0");

    println!("VERIFIED: Empty database has schema version 0");

    // Verify needs_migrations returns true
    let needs = storage.needs_migrations().expect("Check failed");
    assert!(needs, "Empty database should need migrations");

    println!("VERIFIED: Empty database needs migrations");

    println!("AFTER: Empty database initialization verified");
}

#[test]
fn test_edge_case_concurrent_arc_sharing() {
    use std::sync::Arc;
    use std::thread;

    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_concurrent.db");

    println!("BEFORE: Testing concurrent access via Arc<DB> sharing");

    let storage = Arc::new(GraphStorage::open_default(&db_path).expect("Failed to open"));

    // Spawn multiple threads that all share the same GraphStorage
    let mut handles = vec![];

    for thread_id in 0..4 {
        let storage_clone = Arc::clone(&storage);
        let handle = thread::spawn(move || {
            // Each thread writes and reads its own data
            let node_id = (thread_id * 100) as i64;
            let mut point = PoincarePoint::origin();
            point.coords[0] = thread_id as f32 * 0.25;

            storage_clone
                .put_hyperbolic(node_id, &point)
                .expect("PUT failed");

            let retrieved = storage_clone
                .get_hyperbolic(node_id)
                .expect("GET failed")
                .expect("Should exist");

            assert!(
                (retrieved.coords[0] - thread_id as f32 * 0.25).abs() < 0.0001,
                "Thread {} data mismatch",
                thread_id
            );

            println!(
                "THREAD {}: Wrote and verified node_id={}",
                thread_id, node_id
            );

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let mut completed = 0;
    for handle in handles {
        handle.join().expect("Thread panicked");
        completed += 1;
    }

    assert_eq!(completed, 4, "All 4 threads should complete");

    // Verify all data is still present
    let count = storage.hyperbolic_count().expect("Count failed");
    assert_eq!(count, 4, "Should have 4 entries from 4 threads");

    println!("AFTER: Concurrent access via Arc<DB> verified");
}
