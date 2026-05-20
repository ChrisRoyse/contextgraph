//! ROCKSDB STRUCTURAL ROUND-TRIP TESTS
//!
//! M-H3 (GH #486, 2026-05-19): docstring corrected. These tests exercise REAL
//! RocksDB instances and REAL serialization code paths, but the fingerprint
//! *content* is a **fixed placeholder**: every fingerprint embedder slot is
//! filled with `SemanticFingerprint::stub()` (vec of `0.1`) and every
//! `content_hash` is the identical `0xDE 0xAD .. 0xBE 0xEF` filler (see
//! `create_placeholder_hash`). Fingerprint `id` (Uuid::new_v4()) is the only
//! field that varies between fingerprints.
//!
//! ## What these tests verify
//! 1. 16 column families can be opened together
//! 2. `serialize_teleological_fingerprint` / deserialize round-trip preserves
//!    `id` and `content_hash` bytes
//! 3. E13 SPLADE inverted index put/get works
//! 4. E1 Matryoshka 128D index put/get works
//! 5. Multi-fingerprint persistence keyed off `id`
//!
//! ## What these tests do NOT verify
//! - Embedder-output semantics (content is identical 0.1-fill)
//! - Deduplication / collision behavior (all hashes are identical filler)
//! - Similarity ranking (all 0.1-fill vectors are cosine-1.0 with each other)
//! For those, see `tests/full_integration_real_data/` which uses seeded
//! pseudo-random fingerprints from `context-graph-test-utils`.

use context_graph_core::types::fingerprint::{SemanticFingerprint, TeleologicalFingerprint};
use context_graph_storage::column_families::cf_names;
use context_graph_storage::get_column_family_descriptors;
use context_graph_storage::teleological::{
    deserialize_memory_id_list, deserialize_teleological_fingerprint, e13_splade_inverted_key,
    fingerprint_key, get_teleological_cf_descriptors, serialize_memory_id_list,
    serialize_teleological_fingerprint, CF_E13_SPLADE_INVERTED, CF_FINGERPRINTS, TELEOLOGICAL_CFS,
    TELEOLOGICAL_CF_COUNT,
};
use rocksdb::{Cache, Options, DB};
use tempfile::TempDir;
use uuid::Uuid;

// =========================================================================
// Helper Functions — placeholder fingerprint for RocksDB round-trip tests
// =========================================================================
//
// M-H3 (GH #486, 2026-05-19): renamed from `create_real_*`. The original
// naming over-claimed; these helpers produce a *fixed placeholder*
// fingerprint with constant 0.1-fill vectors and a constant `0xDEADBEEF`-
// style hash. Only `id` (Uuid::new_v4()) varies between fingerprints.
// Suitable for RocksDB serialize/deserialize round-trip and CF open/close
// tests; NOT suitable for any test that depends on embedder-output
// semantics, similarity ranking, or content-hash collision behavior.

/// Build a `SemanticFingerprint` whose every dense slot is `vec![0.1; N]`.
///
/// STOR-M1: 0.1-filled (non-zero) so `validate_vector()` does not reject it
/// for zero-norm-undefined-cosine reasons. All fingerprints produced by this
/// helper are cosine-1.0 with each other, so this is NOT suitable for any
/// similarity-ranking test.
fn create_placeholder_semantic() -> SemanticFingerprint {
    SemanticFingerprint::stub()
}

/// Build a 32-byte content hash with the `0xDEADBEEF`-style filler pattern.
///
/// EVERY fingerprint produced by `create_placeholder_fingerprint` has the
/// EXACT SAME hash. Tests that depend on hash uniqueness must use a different
/// helper (e.g.,
/// `context_graph_test_utils::generate_random_content_hash_with_seed`).
fn create_placeholder_hash() -> [u8; 32] {
    let mut hash = [0u8; 32];
    hash[0] = 0xDE;
    hash[1] = 0xAD;
    hash[30] = 0xBE;
    hash[31] = 0xEF;
    hash
}

/// Build a `TeleologicalFingerprint` with placeholder semantic + hash.
///
/// `id` is `Uuid::new_v4()` (via `TeleologicalFingerprint::new`); all other
/// fields are constant filler.
fn create_placeholder_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(create_placeholder_semantic(), create_placeholder_hash())
}

// =========================================================================
// Integration Tests
// =========================================================================

#[test]
fn test_rocksdb_open_with_teleological_column_families() {
    // Teleological CF count is authoritative in TELEOLOGICAL_CF_COUNT.
    let expected_total = 11 + TELEOLOGICAL_CF_COUNT;
    println!(
        "=== INTEGRATION: Open RocksDB with {expected_total} column families (11 base + {TELEOLOGICAL_CF_COUNT} teleological) ==="
    );

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB per constitution.yaml

    // Get base 11 CFs (8 original + 3 graph linking per TASK-GRAPHLINK-010)
    let mut descriptors = get_column_family_descriptors(&cache);
    println!("BEFORE: {} base column families", descriptors.len());
    assert_eq!(descriptors.len(), 11);

    // Add teleological CFs.
    descriptors.extend(get_teleological_cf_descriptors(&cache));
    println!("AFTER: {} total column families", descriptors.len());
    assert_eq!(descriptors.len(), expected_total);

    // Open DB with all base + teleological CFs.
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB with base + teleological CFs");

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

    println!("RESULT: PASS - All base + teleological CFs accessible");
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

    // Build placeholder fingerprint (constant 0.1-fill content, unique id).
    let original = create_placeholder_fingerprint();
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

    let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes)
        .expect("Failed to deserialize fingerprint");
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

    let retrieved_ids =
        deserialize_memory_id_list(&retrieved_bytes).expect("Failed to deserialize memory ID list");
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
        (0..10).map(|_| create_placeholder_fingerprint()).collect();

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

        let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes)
            .expect("Failed to deserialize fingerprint");

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

        let retrieved_ids = deserialize_memory_id_list(&retrieved_bytes)
            .expect("Failed to deserialize memory ID list");
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
    let fp = create_placeholder_fingerprint();
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

    let fp = create_placeholder_fingerprint();
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

        let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes)
            .expect("Failed to deserialize fingerprint");
        assert_eq!(fp.id, retrieved.id);

        println!(
            "AFTER: Reopened DB and retrieved fingerprint {}",
            retrieved.id
        );
    }

    println!("RESULT: PASS - Data persists across DB reopen");
}

#[test]
fn test_total_column_families_matches_teleological_constants() {
    let expected_total = 11 + TELEOLOGICAL_CF_COUNT;
    println!(
        "=== INTEGRATION: Verify exactly {expected_total} column families (11 base + {TELEOLOGICAL_CF_COUNT} teleological) ==="
    );

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);

    // Count base CFs (8 original + 3 graph linking per TASK-GRAPHLINK-010)
    let base_descriptors = get_column_family_descriptors(&cache);
    println!("Base column families: {}", base_descriptors.len());
    assert_eq!(
        base_descriptors.len(),
        11,
        "Expected 11 base CFs (8 original + 3 graph linking)"
    );

    // Count teleological CFs.
    let teleological_descriptors = get_teleological_cf_descriptors(&cache);
    println!(
        "Teleological column families: {}",
        teleological_descriptors.len()
    );
    assert_eq!(
        teleological_descriptors.len(),
        TELEOLOGICAL_CF_COUNT,
        "Expected TELEOLOGICAL_CF_COUNT teleological CFs"
    );

    // Total
    let total = base_descriptors.len() + teleological_descriptors.len();
    println!("Total column families: {}", total);
    assert_eq!(
        total, expected_total,
        "Expected base CFs + TELEOLOGICAL_CF_COUNT"
    );

    // Verify by opening DB
    let mut all_descriptors = base_descriptors;
    all_descriptors.extend(teleological_descriptors);

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let _db = DB::open_cf_descriptors(&opts, temp_dir.path(), all_descriptors)
        .expect("Failed to open RocksDB with base + teleological CFs");

    println!(
        "RESULT: PASS - Exactly {expected_total} column families confirmed (11 base + {TELEOLOGICAL_CF_COUNT} teleological)"
    );
}
