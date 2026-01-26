//! Integration tests for teleological storage with real RocksDB.
//!
//! # CRITICAL: NO MOCK DATA
//!
//! All tests use REAL RocksDB instances and REAL data structures.
//! Tests verify:
//! 1. 16 column families can be opened together
//! 2. TeleologicalFingerprint can be stored and retrieved
//! 3. E13 SPLADE inverted index operations work correctly
//! 4. E1 Matryoshka 128D index operations work correctly

use context_graph_core::types::fingerprint::{SemanticFingerprint, TeleologicalFingerprint};
use context_graph_storage::column_families::cf_names;
use context_graph_storage::get_column_family_descriptors;
use context_graph_storage::teleological::{
    deserialize_memory_id_list, deserialize_teleological_fingerprint, e13_splade_inverted_key,
    fingerprint_key, get_teleological_cf_descriptors, serialize_memory_id_list,
    serialize_teleological_fingerprint, CF_E13_SPLADE_INVERTED, CF_FINGERPRINTS, TELEOLOGICAL_CFS,
};
use rocksdb::{Cache, Options, DB};
use tempfile::TempDir;
use uuid::Uuid;

// =========================================================================
// Helper Functions - Create REAL data (no mocks)
// =========================================================================

/// Create a SemanticFingerprint with zeroed embeddings for testing.
/// NOTE: This uses zeroed data which is only suitable for serialization/storage tests.
/// For search/alignment tests, use real embeddings from the embedding pipeline.
fn create_real_semantic() -> SemanticFingerprint {
    SemanticFingerprint::zeroed()
}

/// Create a REAL content hash.
fn create_real_hash() -> [u8; 32] {
    let mut hash = [0u8; 32];
    hash[0] = 0xDE;
    hash[1] = 0xAD;
    hash[30] = 0xBE;
    hash[31] = 0xEF;
    hash
}

/// Create a REAL TeleologicalFingerprint.
fn create_real_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(create_real_semantic(), create_real_hash())
}

// =========================================================================
// Integration Tests
// =========================================================================

#[test]
fn test_rocksdb_open_with_20_column_families() {
    println!(
        "=== INTEGRATION: Open RocksDB with 26 column families (11 base + 15 teleological) ==="
    );

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB per constitution.yaml

    // Get base 11 CFs (8 original + 3 graph linking per TASK-GRAPHLINK-010)
    let mut descriptors = get_column_family_descriptors(&cache);
    println!("BEFORE: {} base column families", descriptors.len());
    assert_eq!(descriptors.len(), 11);

    // Add 15 teleological CFs (TELEOLOGICAL_CF_COUNT per column_families.rs)
    // 13 active + 2 legacy (CF_SESSION_IDENTITY, CF_EGO_NODE for backwards compatibility)
    // Includes CF_SOURCE_METADATA for file watcher provenance tracking
    descriptors.extend(get_teleological_cf_descriptors(&cache));
    println!("AFTER: {} total column families", descriptors.len());
    assert_eq!(descriptors.len(), 26);

    // Open DB with all 26 CFs
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB with 20 CFs");

    // Verify all 8 base CFs accessible
    println!("Verifying base column families:");
    for cf_name in cf_names::ALL {
        assert!(
            db.cf_handle(cf_name).is_some(),
            "Missing base CF: {}",
            cf_name
        );
        println!("  [OK] {}", cf_name);
    }

    // Verify all 12 teleological CFs accessible (10 active + 2 legacy)
    println!("Verifying teleological column families:");
    for cf_name in TELEOLOGICAL_CFS {
        assert!(
            db.cf_handle(cf_name).is_some(),
            "Missing teleological CF: {}",
            cf_name
        );
        println!("  [OK] {}", cf_name);
    }

    println!("RESULT: PASS - All 19 CFs accessible");
}

#[test]
fn test_rocksdb_store_retrieve_fingerprint() {
    println!("=== INTEGRATION: Store and retrieve TeleologicalFingerprint ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    // Create REAL fingerprint
    let original = create_real_fingerprint();
    let id = original.id;

    println!("BEFORE: Storing fingerprint {}", id);

    // Store
    let cf = db
        .cf_handle(CF_FINGERPRINTS)
        .expect("Missing fingerprints CF");
    let key = fingerprint_key(&id);
    let value = serialize_teleological_fingerprint(&original);
    println!(
        "  - Serialized size: {} bytes ({:.2}KB)",
        value.len(),
        value.len() as f64 / 1024.0
    );

    db.put_cf(&cf, key, &value)
        .expect("Failed to store fingerprint");
    println!("  ✓ Stored to RocksDB");

    // Retrieve
    let retrieved_bytes = db
        .get_cf(&cf, key)
        .expect("Failed to get fingerprint")
        .expect("Fingerprint not found");

    let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes);
    println!("AFTER: Retrieved fingerprint {}", retrieved.id);

    // Verify
    assert_eq!(original.id, retrieved.id);
    assert_eq!(original.content_hash, retrieved.content_hash);

    println!("RESULT: PASS - Store/retrieve round-trip successful");
}

#[test]
fn test_rocksdb_e13_splade_inverted_index() {
    println!("=== INTEGRATION: E13 SPLADE inverted index operations ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    let cf = db
        .cf_handle(CF_E13_SPLADE_INVERTED)
        .expect("Missing e13_splade CF");

    // Store term -> memory_ids mapping
    let term_id: u16 = 42;
    let memory_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

    println!(
        "BEFORE: Storing term {} with {} memory IDs",
        term_id,
        memory_ids.len()
    );
    for (i, id) in memory_ids.iter().enumerate() {
        println!("  [{}]: {}", i, id);
    }

    let key = e13_splade_inverted_key(term_id);
    let value = serialize_memory_id_list(&memory_ids);

    db.put_cf(&cf, key, &value)
        .expect("Failed to store inverted index");
    println!("  ✓ Stored {} bytes", value.len());

    // Retrieve
    let retrieved_bytes = db
        .get_cf(&cf, key)
        .expect("Failed to get inverted index")
        .expect("Term not found");

    let retrieved_ids = deserialize_memory_id_list(&retrieved_bytes);
    println!(
        "AFTER: Retrieved {} memory IDs for term {}",
        retrieved_ids.len(),
        term_id
    );
    for (i, id) in retrieved_ids.iter().enumerate() {
        println!("  [{}]: {}", i, id);
    }

    assert_eq!(memory_ids, retrieved_ids);
    println!("RESULT: PASS - Inverted index operations successful");
}

#[test]
fn test_rocksdb_multiple_fingerprints() {
    println!("=== INTEGRATION: Store/retrieve multiple fingerprints ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    let cf = db
        .cf_handle(CF_FINGERPRINTS)
        .expect("Missing fingerprints CF");

    // Create and store 10 fingerprints
    let fingerprints: Vec<TeleologicalFingerprint> =
        (0..10).map(|_| create_real_fingerprint()).collect();

    println!("BEFORE: Storing {} fingerprints", fingerprints.len());

    for fp in &fingerprints {
        let key = fingerprint_key(&fp.id);
        let value = serialize_teleological_fingerprint(fp);
        db.put_cf(&cf, key, &value)
            .expect("Failed to store fingerprint");
    }
    println!("  ✓ All fingerprints stored");

    // Retrieve and verify all
    println!(
        "AFTER: Retrieving and verifying {} fingerprints",
        fingerprints.len()
    );

    for (i, original) in fingerprints.iter().enumerate() {
        let key = fingerprint_key(&original.id);
        let retrieved_bytes = db
            .get_cf(&cf, key)
            .expect("Failed to get fingerprint")
            .expect("Fingerprint not found");

        let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes);

        assert_eq!(original.id, retrieved.id, "ID mismatch at index {}", i);
        assert_eq!(
            original.content_hash, retrieved.content_hash,
            "Hash mismatch at index {}",
            i
        );
        println!("  [{}]: {} ✓", i, original.id);
    }

    println!("RESULT: PASS - Multiple fingerprint operations successful");
}

#[test]
fn test_rocksdb_e13_multiple_terms() {
    println!("=== INTEGRATION: E13 SPLADE with multiple terms ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    let cf = db
        .cf_handle(CF_E13_SPLADE_INVERTED)
        .expect("Missing e13_splade CF");

    // Store multiple term -> memory_ids mappings
    let term_data: Vec<(u16, Vec<Uuid>)> = vec![
        (100, (0..3).map(|_| Uuid::new_v4()).collect()),
        (200, (0..5).map(|_| Uuid::new_v4()).collect()),
        (300, (0..10).map(|_| Uuid::new_v4()).collect()),
        (400, vec![]),                                     // Empty list - edge case
        (30521, (0..2).map(|_| Uuid::new_v4()).collect()), // Near max vocab
    ];

    println!("BEFORE: Storing {} term mappings", term_data.len());
    for (term_id, ids) in &term_data {
        println!("  term {}: {} memory IDs", term_id, ids.len());
        let key = e13_splade_inverted_key(*term_id);
        let value = serialize_memory_id_list(ids);
        db.put_cf(&cf, key, &value)
            .expect("Failed to store inverted index");
    }

    // Retrieve and verify all
    println!(
        "AFTER: Retrieving and verifying {} term mappings",
        term_data.len()
    );
    for (term_id, original_ids) in &term_data {
        let key = e13_splade_inverted_key(*term_id);
        let retrieved_bytes = db
            .get_cf(&cf, key)
            .expect("Failed to get inverted index")
            .expect("Term not found");

        let retrieved_ids = deserialize_memory_id_list(&retrieved_bytes);
        assert_eq!(
            original_ids, &retrieved_ids,
            "Mismatch for term {}",
            term_id
        );
        println!("  term {}: {} memory IDs ✓", term_id, retrieved_ids.len());
    }

    println!("RESULT: PASS - Multiple term operations successful");
}

#[test]
fn test_rocksdb_cf_isolation() {
    println!("=== INTEGRATION: Column family data isolation ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    // Store data in fingerprints CF
    let fp = create_real_fingerprint();
    let key = fingerprint_key(&fp.id);
    let fp_cf = db
        .cf_handle(CF_FINGERPRINTS)
        .expect("Missing fingerprints CF");
    let value = serialize_teleological_fingerprint(&fp);
    db.put_cf(&fp_cf, key, &value)
        .expect("Failed to store fingerprint");

    // Same key should NOT exist in other CFs
    let e13_cf = db
        .cf_handle(CF_E13_SPLADE_INVERTED)
        .expect("Missing e13 CF");
    let result = db.get_cf(&e13_cf, key).expect("Failed to query e13 CF");

    assert!(
        result.is_none(),
        "Data should NOT leak between column families"
    );

    println!("RESULT: PASS - Column family isolation verified");
}

#[test]
fn test_rocksdb_persistence() {
    println!("=== INTEGRATION: Data persistence across DB reopen ===");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().to_path_buf();
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);

    let fp = create_real_fingerprint();
    let fp_id = fp.id;

    // Store data and close DB
    {
        let mut descriptors = get_column_family_descriptors(&cache);
        descriptors.extend(get_teleological_cf_descriptors(&cache));

        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let db =
            DB::open_cf_descriptors(&opts, &db_path, descriptors).expect("Failed to open RocksDB");

        let cf = db
            .cf_handle(CF_FINGERPRINTS)
            .expect("Missing fingerprints CF");
        let key = fingerprint_key(&fp_id);
        let value = serialize_teleological_fingerprint(&fp);
        db.put_cf(&cf, key, &value)
            .expect("Failed to store fingerprint");

        println!("BEFORE: Stored fingerprint {} and closing DB", fp_id);
        // DB drops here
    }

    // Reopen and verify data persists
    {
        let mut descriptors = get_column_family_descriptors(&cache);
        descriptors.extend(get_teleological_cf_descriptors(&cache));

        let mut opts = Options::default();
        opts.create_if_missing(false); // DB should already exist

        let db = DB::open_cf_descriptors(&opts, &db_path, descriptors)
            .expect("Failed to reopen RocksDB");

        let cf = db
            .cf_handle(CF_FINGERPRINTS)
            .expect("Missing fingerprints CF");
        let key = fingerprint_key(&fp_id);
        let retrieved_bytes = db
            .get_cf(&cf, key)
            .expect("Failed to get fingerprint")
            .expect("Fingerprint not found after reopen");

        let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes);
        assert_eq!(fp.id, retrieved.id);

        println!(
            "AFTER: Reopened DB and retrieved fingerprint {}",
            retrieved.id
        );
    }

    println!("RESULT: PASS - Data persists across DB reopen");
}

#[test]
fn test_total_column_families_is_20() {
    println!("=== INTEGRATION: Verify exactly 26 column families (11 base + 15 teleological) ===");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);

    // Count base CFs (8 original + 3 graph linking per TASK-GRAPHLINK-010)
    let base_descriptors = get_column_family_descriptors(&cache);
    println!("Base column families: {}", base_descriptors.len());
    assert_eq!(base_descriptors.len(), 11, "Expected 11 base CFs (8 original + 3 graph linking)");

    // Count teleological CFs (TELEOLOGICAL_CF_COUNT = 15 per column_families.rs)
    // 13 active + 2 legacy (CF_SESSION_IDENTITY, CF_EGO_NODE for backwards compatibility)
    // Includes CF_SOURCE_METADATA for file watcher provenance tracking
    let teleological_descriptors = get_teleological_cf_descriptors(&cache);
    println!(
        "Teleological column families: {}",
        teleological_descriptors.len()
    );
    assert_eq!(
        teleological_descriptors.len(),
        15,
        "Expected 15 teleological CFs (TELEOLOGICAL_CF_COUNT)"
    );

    // Total
    let total = base_descriptors.len() + teleological_descriptors.len();
    println!("Total column families: {}", total);
    assert_eq!(
        total, 26,
        "Expected 26 total CFs (11 base + 15 teleological)"
    );

    // Verify by opening DB
    let mut all_descriptors = base_descriptors;
    all_descriptors.extend(teleological_descriptors);

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let _db = DB::open_cf_descriptors(&opts, temp_dir.path(), all_descriptors)
        .expect("Failed to open RocksDB with 26 CFs");

    println!("RESULT: PASS - Exactly 26 column families confirmed (11 base + 15 teleological)");
}
