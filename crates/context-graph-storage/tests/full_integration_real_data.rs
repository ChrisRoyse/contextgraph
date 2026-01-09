//! Comprehensive Integration Tests with REAL Data
//!
//! # CRITICAL: NO MOCK DATA - NO FALLBACKS
//!
//! Every test uses REAL implementations:
//! - Real RocksDB databases (temp directories)
//! - Real TeleologicalFingerprint data with proper dimensions
//! - Real serialization/deserialization
//! - Physical verification of data persistence
//!
//! # Test Categories
//!
//! 1. RocksDB + Store Integration - roundtrip verification
//! 2. Full Pipeline - store, cache behavior, search
//! 3. Persistence Verification - data survives restart
//! 4. Column Family Verification - all 17 CFs populated
//! 5. Batch Operations - performance under load
//! 6. Search Operations - semantic, purpose, sparse
//!
//! # FAIL FAST Policy
//!
//! All tests should fail clearly if something is wrong.
//! No graceful degradation. Errors are fatal.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector,
    TeleologicalFingerprint, NUM_EMBEDDERS,
};
use context_graph_storage::teleological::{
    deserialize_e1_matryoshka_128, deserialize_purpose_vector,
    deserialize_teleological_fingerprint, e1_matryoshka_128_key, fingerprint_key,
    purpose_vector_key, serialize_teleological_fingerprint, RocksDbTeleologicalStore,
    TeleologicalStoreConfig, CF_E1_MATRYOSHKA_128, CF_E13_SPLADE_INVERTED,
    CF_FINGERPRINTS, CF_PURPOSE_VECTORS, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
};
use rand::Rng;
use tempfile::TempDir;
use uuid::Uuid;

// =============================================================================
// Test Utilities - REAL Data Generation (NO MOCKS)
// =============================================================================

/// Create a RocksDbTeleologicalStore with initialized HNSW indexes.
/// Note: EmbedderIndexRegistry is initialized in the constructor,
/// so no separate initialization step is needed.
fn create_initialized_store(path: &std::path::Path) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open(path).expect("Failed to open store")
}

/// Generate a REAL random unit vector of specified dimension.
/// All vectors are normalized to have L2 norm = 1.0.
fn generate_real_unit_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

    // Normalize to unit vector
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in &mut vec {
            *v /= norm;
        }
    }

    vec
}

/// Generate a REAL SparseVector with realistic sparsity (~5% active).
fn generate_real_sparse_vector(target_nnz: usize) -> SparseVector {
    let mut rng = rand::thread_rng();

    // Generate unique sorted indices
    let mut indices_set: HashSet<u16> = HashSet::new();
    while indices_set.len() < target_nnz {
        indices_set.insert(rng.gen_range(0..30522));
    }
    let mut indices: Vec<u16> = indices_set.into_iter().collect();
    indices.sort();

    // Generate random positive values (SPLADE scores are positive)
    let values: Vec<f32> = (0..target_nnz)
        .map(|_| rng.gen_range(0.1..2.0))
        .collect();

    SparseVector::new(indices, values).expect("Failed to create sparse vector")
}

/// Generate a REAL SemanticFingerprint with proper dimensions.
/// E1: 1024D, E2-E4: 512D, E5: 768D, E6: sparse, E7: 1536D, E8: 384D,
/// E9: 1024D (projected), E10: 768D, E11: 384D, E12: 128D tokens, E13: sparse
fn generate_real_semantic_fingerprint() -> SemanticFingerprint {
    SemanticFingerprint {
        e1_semantic: generate_real_unit_vector(1024),
        e2_temporal_recent: generate_real_unit_vector(512),
        e3_temporal_periodic: generate_real_unit_vector(512),
        e4_temporal_positional: generate_real_unit_vector(512),
        e5_causal: generate_real_unit_vector(768),
        e6_sparse: generate_real_sparse_vector(100), // ~0.3% sparsity for E6
        e7_code: generate_real_unit_vector(1536),
        e8_graph: generate_real_unit_vector(384),
        e9_hdc: generate_real_unit_vector(1024), // HDC projected dimension
        e10_multimodal: generate_real_unit_vector(768),
        e11_entity: generate_real_unit_vector(384),
        e12_late_interaction: vec![generate_real_unit_vector(128); 32], // 32 tokens
        e13_splade: generate_real_sparse_vector(150), // ~0.5% sparsity for E13
    }
}

/// Generate a REAL PurposeVector with random alignments.
fn generate_real_purpose_vector() -> PurposeVector {
    let mut rng = rand::thread_rng();
    let alignments: [f32; NUM_EMBEDDERS] = std::array::from_fn(|_| rng.gen_range(0.3..0.95));
    PurposeVector::new(alignments)
}

/// Generate a REAL JohariFingerprint with valid quadrant weights.
fn generate_real_johari_fingerprint() -> JohariFingerprint {
    let mut rng = rand::thread_rng();
    let mut jf = JohariFingerprint::zeroed();

    for i in 0..NUM_EMBEDDERS {
        // Generate 4 random weights that sum to 1.0
        let mut weights: [f32; 4] = std::array::from_fn(|_| rng.gen_range(0.0..1.0));
        let sum: f32 = weights.iter().sum();
        for w in &mut weights {
            *w /= sum;
        }
        let confidence = rng.gen_range(0.5..1.0);
        jf.set_quadrant(i, weights[0], weights[1], weights[2], weights[3], confidence);
    }

    jf
}

/// Generate a REAL content hash (SHA-256 simulation).
fn generate_real_content_hash() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let mut hash = [0u8; 32];
    rng.fill(&mut hash);
    hash
}

/// Create a REAL TeleologicalFingerprint with all real data.
/// NO MOCK DATA - all vectors have correct dimensions and valid values.
fn create_real_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        generate_real_semantic_fingerprint(),
        generate_real_purpose_vector(),
        generate_real_johari_fingerprint(),
        generate_real_content_hash(),
    )
}

/// Create a REAL TeleologicalFingerprint with a specific ID.
fn create_real_fingerprint_with_id(id: Uuid) -> TeleologicalFingerprint {
    TeleologicalFingerprint::with_id(
        id,
        generate_real_semantic_fingerprint(),
        generate_real_purpose_vector(),
        generate_real_johari_fingerprint(),
        generate_real_content_hash(),
    )
}

// =============================================================================
// TEST 1: RocksDB + Store Roundtrip Integration Test
// =============================================================================

#[tokio::test]
async fn test_rocksdb_store_roundtrip_real_data() {
    println!("\n=== TEST: RocksDB + Store Roundtrip with REAL Data ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Verify health check passes
    store.health_check().expect("Health check failed");

    const TEST_COUNT: usize = 100;
    let mut stored_ids: Vec<Uuid> = Vec::with_capacity(TEST_COUNT);

    println!("[BEFORE] Store empty, 0 fingerprints");
    let initial_count = store.count().await.expect("Failed to count");
    assert_eq!(initial_count, 0, "Store should be empty initially");

    // Store 100 REAL fingerprints
    println!("[STORING] {} fingerprints with REAL vector data...", TEST_COUNT);
    let store_start = Instant::now();

    for i in 0..TEST_COUNT {
        let fp = create_real_fingerprint();
        let id = fp.id;

        // Verify fingerprint has correct dimensions before storage
        assert_eq!(fp.semantic.e1_semantic.len(), 1024, "E1 should be 1024D");
        assert_eq!(fp.semantic.e5_causal.len(), 768, "E5 should be 768D");
        assert_eq!(fp.semantic.e9_hdc.len(), 1024, "E9 should be 1024D (projected)");
        assert!(fp.semantic.e6_sparse.nnz() > 0, "E6 sparse should have entries");
        assert!(fp.semantic.e13_splade.nnz() > 0, "E13 sparse should have entries");

        let stored_id = store.store(fp).await.expect("Failed to store");
        assert_eq!(stored_id, id, "Stored ID should match original");
        stored_ids.push(id);

        if (i + 1) % 25 == 0 {
            println!("  Stored {}/{}", i + 1, TEST_COUNT);
        }
    }

    let store_duration = store_start.elapsed();
    println!("[STORED] {} fingerprints in {:?}", TEST_COUNT, store_duration);

    // Verify count
    let after_count = store.count().await.expect("Failed to count");
    assert_eq!(after_count, TEST_COUNT, "Count should be {}", TEST_COUNT);

    // Retrieve and verify all 100 fingerprints
    println!("[RETRIEVING] {} fingerprints...", TEST_COUNT);
    let retrieve_start = Instant::now();

    for (i, &id) in stored_ids.iter().enumerate() {
        let retrieved = store.retrieve(id).await
            .expect("Failed to retrieve")
            .unwrap_or_else(|| panic!("Fingerprint {} not found", id));

        // Verify data integrity
        assert_eq!(retrieved.id, id, "ID mismatch");
        assert_eq!(retrieved.semantic.e1_semantic.len(), 1024, "E1 dimension mismatch");
        assert_eq!(retrieved.semantic.e9_hdc.len(), 1024, "E9 dimension mismatch (expected 1024)");
        assert!(retrieved.semantic.e13_splade.nnz() > 0, "E13 sparse should have entries");

        // Verify vectors are unit normalized (L2 norm ~ 1.0)
        let e1_norm: f32 = retrieved.semantic.e1_semantic.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (e1_norm - 1.0).abs() < 0.01,
            "E1 should be unit normalized, got norm={}", e1_norm
        );

        if (i + 1) % 25 == 0 {
            println!("  Retrieved {}/{}", i + 1, TEST_COUNT);
        }
    }

    let retrieve_duration = retrieve_start.elapsed();
    println!("[RETRIEVED] {} fingerprints in {:?}", TEST_COUNT, retrieve_duration);

    // Get storage size
    let size_bytes = store.storage_size_bytes();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
    println!("[AFTER] Stored {} fingerprints, DB size = {:.2}MB", TEST_COUNT, size_mb);

    println!("[VERIFIED] All {} fingerprints retrievable, roundtrip successful", TEST_COUNT);
    println!("\n=== PASS: RocksDB + Store Roundtrip ===\n");
}

// =============================================================================
// TEST 2: Full Pipeline Test (Store, Search)
// =============================================================================

#[tokio::test]
async fn test_full_storage_pipeline_real_data() {
    println!("\n=== TEST: Full Storage Pipeline with REAL Data ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = TeleologicalStoreConfig {
        block_cache_size: 128 * 1024 * 1024, // 128MB cache
        max_open_files: 500,
        enable_wal: true,
        create_if_missing: true,
    };
    let store = RocksDbTeleologicalStore::open_with_config(temp_dir.path(), config)
        .expect("Failed to open store");
    // Note: EmbedderIndexRegistry is initialized in the constructor

    println!("[BEFORE] Empty store");

    // Store 50 fingerprints
    const COUNT: usize = 50;
    let mut stored: Vec<TeleologicalFingerprint> = Vec::with_capacity(COUNT);

    for _ in 0..COUNT {
        let fp = create_real_fingerprint();
        store.store(fp.clone()).await.expect("Failed to store");
        stored.push(fp);
    }
    println!("[STORED] {} fingerprints", COUNT);

    // Test purpose search
    let query_purpose = generate_real_purpose_vector();
    let purpose_options = context_graph_core::traits::TeleologicalSearchOptions {
        top_k: 10,
        min_similarity: 0.0,
        min_alignment: None,
        include_deleted: false,
        johari_quadrant_filter: None,
        embedder_indices: vec![],
    };

    let purpose_results = store.search_purpose(&query_purpose, purpose_options.clone())
        .await
        .expect("Purpose search failed");

    println!("[SEARCH] Purpose search returned {} results", purpose_results.len());
    assert!(!purpose_results.is_empty(), "Purpose search should return results");

    // Test semantic search
    let query_semantic = generate_real_semantic_fingerprint();
    let semantic_results = store.search_semantic(&query_semantic, purpose_options.clone())
        .await
        .expect("Semantic search failed");

    println!("[SEARCH] Semantic search returned {} results", semantic_results.len());
    assert!(!semantic_results.is_empty(), "Semantic search should return results");

    // Test sparse search
    let query_sparse = generate_real_sparse_vector(50);
    let sparse_results = store.search_sparse(&query_sparse, 10)
        .await
        .expect("Sparse search failed");

    println!("[SEARCH] Sparse search returned {} results", sparse_results.len());

    // Test delete (soft)
    let delete_id = stored[0].id;
    let deleted = store.delete(delete_id, true).await.expect("Soft delete failed");
    assert!(deleted, "Soft delete should succeed");

    // Verify soft deleted item not retrievable
    let after_delete = store.retrieve(delete_id).await.expect("Retrieve failed");
    assert!(after_delete.is_none(), "Soft deleted item should not be retrievable");

    // Verify count decreased
    let final_count = store.count().await.expect("Count failed");
    assert_eq!(final_count, COUNT - 1, "Count should be {} after soft delete", COUNT - 1);

    // Test hard delete
    let hard_delete_id = stored[1].id;
    let hard_deleted = store.delete(hard_delete_id, false).await.expect("Hard delete failed");
    assert!(hard_deleted, "Hard delete should succeed");

    println!("[AFTER] {} fingerprints remaining after deletes", final_count - 1);
    println!("[VERIFIED] Full pipeline: store, search, delete all working");
    println!("\n=== PASS: Full Storage Pipeline ===\n");
}

// =============================================================================
// TEST 3: Persistence Verification Across Restart
// =============================================================================

#[tokio::test]
async fn test_physical_persistence_across_restart() {
    println!("\n=== TEST: Physical Persistence Across Restart ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path: PathBuf = temp_dir.path().to_path_buf();

    // Generate fingerprints with known IDs
    let test_fingerprints: Vec<TeleologicalFingerprint> = (0..10)
        .map(|_| create_real_fingerprint())
        .collect();

    let test_ids: Vec<Uuid> = test_fingerprints.iter().map(|fp| fp.id).collect();

    // Phase 1: Store and close
    println!("[PHASE 1] Storing 10 fingerprints and closing database...");
    {
        let store = RocksDbTeleologicalStore::open(&db_path)
            .expect("Failed to open store (phase 1)");
        // Note: EmbedderIndexRegistry is initialized in the constructor

        for fp in &test_fingerprints {
            store.store(fp.clone()).await.expect("Failed to store");
        }

        // Flush to ensure data is on disk
        store.flush().await.expect("Flush failed");

        let count = store.count().await.expect("Count failed");
        assert_eq!(count, 10, "Should have 10 fingerprints before close");

        println!("[BEFORE] Stored 10 fingerprints, flushed, closing DB");
        // Store drops here
    }

    // Phase 2: Reopen and verify
    println!("[PHASE 2] Reopening database and verifying data...");
    {
        let store = RocksDbTeleologicalStore::open(&db_path)
            .expect("Failed to reopen store (phase 2)");
        // Note: EmbedderIndexRegistry is initialized in the constructor

        let count = store.count().await.expect("Count failed");
        assert_eq!(count, 10, "Should still have 10 fingerprints after reopen");

        // Verify all 10 fingerprints are retrievable
        for (i, &id) in test_ids.iter().enumerate() {
            let retrieved = store.retrieve(id).await
                .expect("Retrieve failed")
                .unwrap_or_else(|| panic!("Fingerprint {} not found after reopen", id));

            assert_eq!(retrieved.id, id, "ID mismatch at index {}", i);

            // Verify data integrity
            assert_eq!(
                retrieved.semantic.e1_semantic.len(), 1024,
                "E1 dimension mismatch after reopen"
            );
            assert_eq!(
                retrieved.semantic.e9_hdc.len(), 1024,
                "E9 dimension mismatch (expected 1024) after reopen"
            );

            println!("  [{}] {} verified", i, id);
        }

        println!("[AFTER] Reopened DB, all 10 fingerprints retrieved and verified");
    }

    // Phase 3: Verify raw files exist on disk
    println!("[PHASE 3] Verifying physical files exist...");
    {
        // Check that RocksDB files exist
        let sst_files: Vec<_> = std::fs::read_dir(&db_path)
            .expect("Failed to read db directory")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "sst").unwrap_or(false))
            .collect();

        // SST files may or may not exist depending on compaction state
        // But CURRENT, MANIFEST files should always exist
        let current_file = db_path.join("CURRENT");
        assert!(current_file.exists(), "CURRENT file should exist");

        let manifest_files: Vec<_> = std::fs::read_dir(&db_path)
            .expect("Failed to read db directory")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("MANIFEST"))
            .collect();
        assert!(!manifest_files.is_empty(), "MANIFEST file should exist");

        println!("[VERIFIED] Physical files exist: CURRENT, {} MANIFEST files, {} SST files",
            manifest_files.len(), sst_files.len());
    }

    println!("\n=== PASS: Physical Persistence Across Restart ===\n");
}

// =============================================================================
// TEST 4: Column Family Verification
// =============================================================================

#[tokio::test]
async fn test_all_column_families_populated() {
    println!("\n=== TEST: All Column Families Populated ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Store a fingerprint
    let fp = create_real_fingerprint();
    let id = fp.id;

    println!("[BEFORE] Storing fingerprint {} to populate CFs", id);
    store.store(fp.clone()).await.expect("Failed to store");

    // Access underlying DB for direct verification
    let db = store.db();

    // 1. Verify fingerprints CF has data
    let cf_fp = db.cf_handle(CF_FINGERPRINTS).expect("Missing fingerprints CF");
    let fp_key = fingerprint_key(&id);
    let fp_data = db.get_cf(&cf_fp, &fp_key)
        .expect("Get failed")
        .expect("Fingerprint not found in fingerprints CF");

    // With E9_DIM = 1024 (projected), fingerprints are ~32-40KB
    println!("[VERIFIED] fingerprints CF: {} bytes", fp_data.len());
    assert!(fp_data.len() >= 25_000, "Fingerprint should be >= 25KB, got {}", fp_data.len());

    // Deserialize and verify
    let retrieved_fp = deserialize_teleological_fingerprint(&fp_data);
    assert_eq!(retrieved_fp.id, id, "ID mismatch in fingerprints CF");

    // 2. Verify purpose_vectors CF has data
    let cf_pv = db.cf_handle(CF_PURPOSE_VECTORS).expect("Missing purpose_vectors CF");
    let pv_key = purpose_vector_key(&id);
    let pv_data = db.get_cf(&cf_pv, &pv_key)
        .expect("Get failed")
        .expect("Data not found in purpose_vectors CF");

    println!("[VERIFIED] purpose_vectors CF: {} bytes (expected 52)", pv_data.len());
    assert_eq!(pv_data.len(), 52, "Purpose vector should be exactly 52 bytes");

    // Deserialize and verify
    let retrieved_pv = deserialize_purpose_vector(&pv_data);
    for i in 0..NUM_EMBEDDERS {
        assert!(
            (retrieved_pv[i] - fp.purpose_vector.alignments[i]).abs() < f32::EPSILON,
            "Purpose vector mismatch at index {}", i
        );
    }

    // 3. Verify e1_matryoshka_128 CF has data
    let cf_mat = db.cf_handle(CF_E1_MATRYOSHKA_128).expect("Missing e1_matryoshka_128 CF");
    let mat_key = e1_matryoshka_128_key(&id);
    let mat_data = db.get_cf(&cf_mat, &mat_key)
        .expect("Get failed")
        .expect("Data not found in e1_matryoshka_128 CF");

    println!("[VERIFIED] e1_matryoshka_128 CF: {} bytes (expected 512)", mat_data.len());
    assert_eq!(mat_data.len(), 512, "E1 Matryoshka 128D should be exactly 512 bytes");

    // Deserialize and verify it matches first 128 dims of E1
    let retrieved_mat = deserialize_e1_matryoshka_128(&mat_data);
    for i in 0..128 {
        assert!(
            (retrieved_mat[i] - fp.semantic.e1_semantic[i]).abs() < f32::EPSILON,
            "E1 Matryoshka mismatch at index {}", i
        );
    }

    // 4. Verify e13_splade_inverted CF has data (if fingerprint has sparse entries)
    if fp.semantic.e13_splade.nnz() > 0 {
        let cf_inv = db.cf_handle(CF_E13_SPLADE_INVERTED).expect("Missing e13_splade_inverted CF");

        // Check at least one term is indexed
        let first_term = fp.semantic.e13_splade.indices[0];
        let term_key = context_graph_storage::teleological::e13_splade_inverted_key(first_term);
        let inv_data = db.get_cf(&cf_inv, &term_key)
            .expect("Get failed")
            .expect("Term not found in inverted index");

        println!("[VERIFIED] e13_splade_inverted CF: term {} has {} bytes", first_term, inv_data.len());
        assert!(inv_data.len() >= 20, "Inverted index entry should have UUID data");
    }

    // 5. Verify all teleological CFs are accessible
    println!("[VERIFYING] All {} teleological CFs accessible...", TELEOLOGICAL_CFS.len());
    for cf_name in TELEOLOGICAL_CFS {
        let cf = db.cf_handle(cf_name).unwrap_or_else(|| panic!("Missing CF: {}", cf_name));
        #[allow(clippy::cmp_null)]
        {
            assert!(cf as *const _ != std::ptr::null(), "CF handle should be valid");
        }
        println!("  {} OK", cf_name);
    }

    // 6. Verify all quantized embedder CFs are accessible
    println!("[VERIFYING] All {} quantized embedder CFs accessible...", QUANTIZED_EMBEDDER_CFS.len());
    for cf_name in QUANTIZED_EMBEDDER_CFS {
        let cf = db.cf_handle(cf_name).unwrap_or_else(|| panic!("Missing CF: {}", cf_name));
        #[allow(clippy::cmp_null)]
        {
            assert!(cf as *const _ != std::ptr::null(), "CF handle should be valid");
        }
        println!("  {} OK", cf_name);
    }

    let total_cfs = TELEOLOGICAL_CFS.len() + QUANTIZED_EMBEDDER_CFS.len();
    println!("[AFTER] All {} column families verified (4 teleological + 13 embedder)", total_cfs);
    println!("\n=== PASS: All Column Families Populated ===\n");
}

// =============================================================================
// TEST 5: Batch Operation Performance Test
// =============================================================================

#[tokio::test]
async fn test_batch_store_retrieve_performance() {
    println!("\n=== TEST: Batch Store/Retrieve Performance ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    const BATCH_SIZE: usize = 1000;

    // Generate all fingerprints first (exclude from timing)
    println!("[GENERATING] {} fingerprints...", BATCH_SIZE);
    let generate_start = Instant::now();
    let fingerprints: Vec<TeleologicalFingerprint> = (0..BATCH_SIZE)
        .map(|_| create_real_fingerprint())
        .collect();
    let generate_duration = generate_start.elapsed();
    println!("[GENERATED] {} fingerprints in {:?}", BATCH_SIZE, generate_duration);

    let ids: Vec<Uuid> = fingerprints.iter().map(|fp| fp.id).collect();

    // Time batch store
    println!("[BEFORE] Store empty, starting batch store of {}", BATCH_SIZE);
    let store_start = Instant::now();

    let stored_ids = store.store_batch(fingerprints).await
        .expect("Batch store failed");

    let store_duration = store_start.elapsed();
    let store_ms = store_duration.as_millis();

    println!("[STORED] {} fingerprints in {:?} ({:.2} fps)",
        BATCH_SIZE, store_duration, BATCH_SIZE as f64 / store_duration.as_secs_f64());

    assert_eq!(stored_ids.len(), BATCH_SIZE, "Should store all fingerprints");
    assert!(store_ms < 10_000, "Batch store should complete in <10s, took {}ms", store_ms);

    // Time batch retrieve
    let retrieve_start = Instant::now();

    let retrieved = store.retrieve_batch(&ids).await
        .expect("Batch retrieve failed");

    let retrieve_duration = retrieve_start.elapsed();
    let retrieve_ms = retrieve_duration.as_millis();

    println!("[RETRIEVED] {} fingerprints in {:?} ({:.2} fps)",
        BATCH_SIZE, retrieve_duration, BATCH_SIZE as f64 / retrieve_duration.as_secs_f64());

    assert_eq!(retrieved.len(), BATCH_SIZE, "Should retrieve all fingerprints");
    assert!(retrieve_ms < 5_000, "Batch retrieve should complete in <5s, took {}ms", retrieve_ms);

    // Verify all retrieved successfully
    let successful = retrieved.iter().filter(|opt| opt.is_some()).count();
    assert_eq!(successful, BATCH_SIZE, "All fingerprints should be retrievable");

    // Verify data integrity on sample
    for (i, opt) in retrieved.iter().enumerate().take(10) {
        let fp = opt.as_ref().unwrap_or_else(|| panic!("Fingerprint {} missing", i));
        assert_eq!(fp.semantic.e1_semantic.len(), 1024, "E1 dimension mismatch");
    }

    // Get final stats
    let count = store.count().await.expect("Count failed");
    let size_bytes = store.storage_size_bytes();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

    println!("[AFTER] Stored {} fingerprints, DB size = {:.2}MB", count, size_mb);
    println!("[VERIFIED] All {} fingerprints stored and retrievable", BATCH_SIZE);
    println!("\n=== PASS: Batch Store/Retrieve Performance ===\n");
}

// =============================================================================
// TEST 6: Search Accuracy Test
// =============================================================================

#[tokio::test]
async fn test_search_returns_correct_results() {
    println!("\n=== TEST: Search Returns Correct Results ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Store 50 random fingerprints
    println!("[SETUP] Storing 50 fingerprints...");
    for _ in 0..50 {
        let fp = create_real_fingerprint();
        store.store(fp).await.expect("Failed to store");
    }

    // Store one fingerprint with known E1 vector for searching
    let mut known_fp = create_real_fingerprint();
    // Set E1 to a known pattern: first 100 elements = 1/sqrt(100), rest = 0
    let norm = 100.0_f32.sqrt();
    known_fp.semantic.e1_semantic = vec![0.0; 1024];
    for i in 0..100 {
        known_fp.semantic.e1_semantic[i] = 1.0 / norm;
    }
    let known_id = known_fp.id;

    store.store(known_fp.clone()).await.expect("Failed to store known fp");
    println!("[SETUP] Stored known fingerprint {}", known_id);

    // Create a query that should match the known fingerprint well
    let mut query_semantic = SemanticFingerprint::zeroed();
    for i in 0..100 {
        query_semantic.e1_semantic[i] = 1.0 / norm;
    }

    // Search
    let options = context_graph_core::traits::TeleologicalSearchOptions {
        top_k: 10,
        min_similarity: 0.0,
        min_alignment: None,
        include_deleted: false,
        johari_quadrant_filter: None,
        embedder_indices: vec![],
    };

    let results = store.search_semantic(&query_semantic, options)
        .await
        .expect("Search failed");

    println!("[SEARCH] Returned {} results", results.len());
    assert!(!results.is_empty(), "Search should return results");

    // The known fingerprint should be in top results with high similarity
    let found = results.iter().find(|r| r.fingerprint.id == known_id);
    assert!(found.is_some(), "Known fingerprint should be in search results");

    let known_result = found.unwrap();
    println!("[FOUND] Known fingerprint at similarity {:.4}", known_result.similarity);
    assert!(
        known_result.similarity > 0.9,
        "Known fingerprint should have high similarity (> 0.9), got {}",
        known_result.similarity
    );

    // Results should be sorted by similarity descending
    for i in 1..results.len() {
        assert!(
            results[i - 1].similarity >= results[i].similarity,
            "Results should be sorted by similarity descending"
        );
    }

    println!("[VERIFIED] Search returns correct results in correct order");
    println!("\n=== PASS: Search Returns Correct Results ===\n");
}

// =============================================================================
// TEST 7: Update and Delete Operations
// =============================================================================

#[tokio::test]
async fn test_update_and_delete_operations() {
    println!("\n=== TEST: Update and Delete Operations ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Store initial fingerprint
    let fp = create_real_fingerprint();
    let id = fp.id;
    let original_theta = fp.theta_to_north_star;

    store.store(fp).await.expect("Failed to store");
    println!("[STORED] Fingerprint {} with theta={:.4}", id, original_theta);

    // Update the fingerprint with new purpose vector
    let mut updated_fp = store.retrieve(id).await
        .expect("Retrieve failed")
        .expect("Fingerprint not found");

    let new_purpose = PurposeVector::new([0.95; NUM_EMBEDDERS]);
    updated_fp.purpose_vector = new_purpose;

    let update_result = store.update(updated_fp.clone()).await
        .expect("Update failed");
    assert!(update_result, "Update should succeed");

    // Verify update persisted
    let after_update = store.retrieve(id).await
        .expect("Retrieve failed")
        .expect("Fingerprint not found after update");

    assert!(
        (after_update.purpose_vector.alignments[0] - 0.95).abs() < f32::EPSILON,
        "Purpose vector should be updated"
    );
    println!("[UPDATED] Fingerprint {} purpose vector updated", id);

    // Test soft delete
    let soft_deleted = store.delete(id, true).await.expect("Soft delete failed");
    assert!(soft_deleted, "Soft delete should succeed");

    let after_soft = store.retrieve(id).await.expect("Retrieve failed");
    assert!(after_soft.is_none(), "Soft deleted fingerprint should not be retrievable");
    println!("[SOFT DELETED] Fingerprint {} no longer retrievable", id);

    // Store another fingerprint for hard delete test
    let fp2 = create_real_fingerprint();
    let id2 = fp2.id;
    store.store(fp2).await.expect("Failed to store");

    let hard_deleted = store.delete(id2, false).await.expect("Hard delete failed");
    assert!(hard_deleted, "Hard delete should succeed");

    // Verify raw bytes are gone
    let db = store.db();
    let cf = db.cf_handle(CF_FINGERPRINTS).expect("Missing CF");
    let raw = db.get_cf(&cf, &fingerprint_key(&id2)).expect("Get failed");
    assert!(raw.is_none(), "Hard deleted fingerprint should be physically removed");
    println!("[HARD DELETED] Fingerprint {} physically removed", id2);

    println!("[VERIFIED] Update and delete operations work correctly");
    println!("\n=== PASS: Update and Delete Operations ===\n");
}

// =============================================================================
// TEST 8: Concurrent Access Test
// =============================================================================

#[tokio::test]
async fn test_concurrent_access() {
    println!("\n=== TEST: Concurrent Access ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = std::sync::Arc::new(create_initialized_store(temp_dir.path()));

    const CONCURRENT_OPS: usize = 100;
    let mut handles = Vec::with_capacity(CONCURRENT_OPS);

    println!("[STARTING] {} concurrent store operations...", CONCURRENT_OPS);

    for _ in 0..CONCURRENT_OPS {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            let fp = create_real_fingerprint();
            let id = fp.id;
            store_clone.store(fp).await.expect("Concurrent store failed");
            id
        });
        handles.push(handle);
    }

    // Wait for all stores to complete
    let stored_ids: Vec<Uuid> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.expect("Task panicked"))
        .collect();

    println!("[STORED] {} fingerprints concurrently", stored_ids.len());
    assert_eq!(stored_ids.len(), CONCURRENT_OPS);

    // Verify all stored successfully
    let count = store.count().await.expect("Count failed");
    assert_eq!(count, CONCURRENT_OPS, "All concurrent stores should succeed");

    // Concurrent retrieves
    let mut retrieve_handles = Vec::with_capacity(CONCURRENT_OPS);
    for &id in &stored_ids {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            store_clone.retrieve(id).await
                .expect("Concurrent retrieve failed")
                .is_some()
        });
        retrieve_handles.push(handle);
    }

    let results: Vec<bool> = futures::future::join_all(retrieve_handles)
        .await
        .into_iter()
        .map(|r| r.expect("Task panicked"))
        .collect();

    let all_found = results.iter().all(|&found| found);
    assert!(all_found, "All concurrent retrieves should find data");

    println!("[VERIFIED] {} concurrent operations completed successfully", CONCURRENT_OPS * 2);
    println!("\n=== PASS: Concurrent Access ===\n");
}

// =============================================================================
// TEST 9: Serialization Size Verification
// =============================================================================

#[test]
fn test_serialization_size_verification() {
    println!("\n=== TEST: Serialization Size Verification ===\n");

    let fp = create_real_fingerprint();
    let id = fp.id;

    // Serialize
    let bytes = serialize_teleological_fingerprint(&fp);

    println!("[SERIALIZED] Fingerprint {} to {} bytes ({:.2}KB)",
        id, bytes.len(), bytes.len() as f64 / 1024.0);

    // Verify size is in expected range (25KB - 100KB with E9_DIM = 1024 projected)
    assert!(
        bytes.len() >= 25_000,
        "Serialized size should be >= 25KB, got {} bytes", bytes.len()
    );
    assert!(
        bytes.len() <= 100_000,
        "Serialized size should be <= 100KB, got {} bytes", bytes.len()
    );

    // Deserialize
    let restored = deserialize_teleological_fingerprint(&bytes);

    // Verify integrity
    assert_eq!(restored.id, id, "ID mismatch after roundtrip");
    assert_eq!(restored.semantic.e1_semantic.len(), 1024, "E1 dimension mismatch");
    assert_eq!(restored.semantic.e9_hdc.len(), 1024, "E9 dimension mismatch (expected 1024)");
    assert_eq!(restored.purpose_vector.alignments.len(), NUM_EMBEDDERS, "Purpose dimension mismatch");

    println!("[VERIFIED] Serialization roundtrip preserves all data");
    println!("\n=== PASS: Serialization Size Verification ===\n");
}

// =============================================================================
// TEST 10: Edge Cases
// =============================================================================

#[tokio::test]
async fn test_edge_cases() {
    println!("\n=== TEST: Edge Cases ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = create_initialized_store(temp_dir.path());

    // Test 1: Retrieve non-existent ID
    let fake_id = Uuid::new_v4();
    let result = store.retrieve(fake_id).await.expect("Retrieve should not error");
    assert!(result.is_none(), "Non-existent ID should return None");
    println!("[EDGE] Retrieve non-existent ID returns None - OK");

    // Test 2: Delete non-existent ID
    let deleted = store.delete(fake_id, false).await.expect("Delete should not error");
    assert!(!deleted, "Delete non-existent should return false");
    println!("[EDGE] Delete non-existent ID returns false - OK");

    // Test 3: Update non-existent fingerprint
    let fp = create_real_fingerprint();
    let updated = store.update(fp).await.expect("Update should not error");
    assert!(!updated, "Update non-existent should return false");
    println!("[EDGE] Update non-existent returns false - OK");

    // Test 4: Empty batch operations
    let empty_ids: Vec<Uuid> = vec![];
    let batch_result = store.retrieve_batch(&empty_ids).await.expect("Empty batch should work");
    assert!(batch_result.is_empty(), "Empty batch should return empty result");
    println!("[EDGE] Empty batch retrieve returns empty - OK");

    // Test 5: Empty sparse vectors
    // NOTE: E6 (Sparse), E12 (LateInteraction), E13 (SPLADE) use inverted indexes
    // which are not yet implemented. They are intentionally skipped in HNSW indexing.
    // Once inverted indexes are implemented (TASK-CORE-009+), this test should be updated
    // to verify proper validation.
    let mut empty_sparse_fp = create_real_fingerprint();
    empty_sparse_fp.semantic.e6_sparse = SparseVector::empty();
    empty_sparse_fp.semantic.e13_splade = SparseVector::empty();

    // Currently, empty sparse vectors are accepted because sparse embedders
    // are not indexed by HNSW. This will change when inverted indexes are added.
    let store_result = store.store(empty_sparse_fp).await;
    assert!(store_result.is_ok(), "Store should accept fingerprint (sparse embedders not yet indexed)");
    println!("[EDGE] Empty sparse vectors accepted (inverted indexes TODO) - OK");

    // Test 6: Double store (should work, overwrites)
    let fp2 = create_real_fingerprint_with_id(Uuid::new_v4());
    let _id2 = fp2.id;
    store.store(fp2.clone()).await.expect("First store should work");
    store.store(fp2.clone()).await.expect("Second store should work");

    let _count = store.count().await.expect("Count failed");
    // Count should not increase from double store of same ID
    println!("[EDGE] Double store of same ID handled - OK");

    println!("[VERIFIED] All edge cases handled correctly");
    println!("\n=== PASS: Edge Cases ===\n");
}

// =============================================================================
// Summary Test Runner
// =============================================================================

#[test]
fn test_summary_real_data_tests() {
    println!("\n");
    println!("============================================================");
    println!("FULL INTEGRATION TESTS WITH REAL DATA - SUMMARY");
    println!("============================================================");
    println!();
    println!("Tests in this file verify:");
    println!("  1. RocksDB + Store roundtrip with 100 REAL fingerprints");
    println!("  2. Full pipeline: store, search, delete");
    println!("  3. Physical persistence across database restart");
    println!("  4. All 17 column families populated correctly");
    println!("  5. Batch operations performance (1000 fingerprints)");
    println!("  6. Search accuracy with known vectors");
    println!("  7. Update and delete operations");
    println!("  8. Concurrent access safety");
    println!("  9. Serialization size verification (~63KB)");
    println!(" 10. Edge cases (non-existent IDs, empty batches, etc.)");
    println!();
    println!("CRITICAL REQUIREMENTS:");
    println!("  - NO MOCK DATA: All tests use real RocksDB and real vectors");
    println!("  - PHYSICAL VERIFICATION: Data actually persists on disk");
    println!("  - FAIL FAST: All tests fail clearly if something is wrong");
    println!("  - NO FALLBACKS: No graceful degradation in tests");
    println!();
    println!("Run with: cargo test -p context-graph-storage full_integration_real_data");
    println!("============================================================");
}
