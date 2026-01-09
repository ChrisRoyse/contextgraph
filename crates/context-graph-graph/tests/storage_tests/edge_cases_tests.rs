//! Edge cases, error handling, and concurrent access tests.
//!
//! Tests for corrupted data handling, dimension validation, and thread safety.

use context_graph_graph::error::GraphError;
use context_graph_graph::storage::{
    get_column_family_descriptors, get_db_options, EntailmentCone, GraphStorage, LegacyGraphEdge,
    PoincarePoint, StorageConfig, CF_METADATA,
};

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
fn test_legacy_graph_edge_serialization() {
    let edge = LegacyGraphEdge {
        target: 42,
        edge_type: 7,
    };

    // Serialize and deserialize with bincode
    let bytes = bincode::serialize(&edge).expect("Serialize failed");
    let deserialized: LegacyGraphEdge = bincode::deserialize(&bytes).expect("Deserialize failed");

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
        db.put_cf(metadata_cf, b"schema_version", [0x01, 0x02])
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

    assert_eq!(
        hyperbolic_count, 0,
        "New DB should have 0 hyperbolic entries"
    );
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
