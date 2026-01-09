//! Schema migration tests.
//!
//! M04-T13a: Schema migrations tests

use context_graph_graph::storage::{
    EntailmentCone, GraphStorage, LegacyGraphEdge, Migrations, PoincarePoint, StorageConfig,
    SCHEMA_VERSION,
};

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

    println!(
        "AFTER: Migrations registry targets version {}",
        SCHEMA_VERSION
    );
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

    storage
        .set_schema_version(1)
        .expect("Failed to set version");
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

    println!(
        "AFTER: Migration is idempotent (applied 3 times, all returned v{})",
        SCHEMA_VERSION
    );
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

        storage
            .put_adjacency(
                3,
                &[LegacyGraphEdge {
                    target: 4,
                    edge_type: 1,
                }],
            )
            .expect("PUT failed");

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
