---
id: "M04-T13a"
title: "Implement Storage Schema Migrations"
description: |
  Define migration system from old CF schema to new.
  Add schema version tracking in a metadata CF.
  Migrations needed:
  - v1: Initial schema with adjacency, hyperbolic, entailment_cones CFs
  - Future: Placeholder for v2 migrations
  Include migrate() method that checks version and applies migrations.
layer: "logic"
status: "completed"
priority: "medium"
estimated_hours: 2
sequence: 18
depends_on:
  - "M04-T13"  # GraphStorage backend (COMPLETED)
spec_refs:
  - "TECH-GRAPH-004 Section 4.2"
files_created:
  - path: "crates/context-graph-graph/src/storage/migrations.rs"
    description: "Schema migration system with version tracking (231 lines)"
files_modified:
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "Added migrations module and re-exports"
  - path: "crates/context-graph-graph/src/storage/storage_impl.rs"
    description: "Added migration support to GraphStorage"
test_file: "crates/context-graph-graph/tests/storage_tests.rs"
sherlock_verified: "2026-01-03"
sherlock_verdict: "VERIFIED"
tests_passing: 76
---

## CRITICAL: Read Before Starting

### Constitution Reference (docs2/constitution.yaml)

```yaml
rules:
  - AP-001: "Never unwrap() in prod - use expect() with context"
  - SEC-06: "Soft delete 30-day recovery"

tech:
  lang: "Rust 1.75+, edition 2021"
  db:
    storage: "RocksDB 0.22"
    vector: "faiss_gpu"
```

### MANDATORY RULES

1. **NO BACKWARDS COMPATIBILITY** - Fail fast with robust error logging
2. **NO MOCK DATA IN TESTS** - Use real RocksDB instances only
3. **NO unwrap()** - Use `?` operator or `expect("context")`
4. **Result<T, GraphError>** for all fallible operations
5. **FAIL FAST** - If migration fails, log error and propagate immediately

---

## Current Codebase State

### M04-T12 Column Families (COMPLETED)

Already implemented in `crates/context-graph-graph/src/storage/mod.rs`:

```rust
pub const CF_ADJACENCY: &str = "adjacency";
pub const CF_HYPERBOLIC: &str = "hyperbolic";
pub const CF_CONES: &str = "entailment_cones";
pub const CF_FAISS_IDS: &str = "faiss_ids";
pub const CF_NODES: &str = "nodes";
pub const CF_METADATA: &str = "metadata";  // Schema version stored here

pub const ALL_COLUMN_FAMILIES: &[&str] = &[...6 CFs...];
```

### M04-T13 GraphStorage (DEPENDENCY - Must be complete)

After M04-T13 is complete, the following will exist in `storage/rocksdb.rs`:

```rust
pub struct GraphStorage {
    db: Arc<DB>,
}

impl GraphStorage {
    pub fn open<P: AsRef<Path>>(path: P, config: StorageConfig) -> GraphResult<Self>;
    pub fn open_default<P: AsRef<Path>>(path: P) -> GraphResult<Self>;

    // Column family accessors
    pub(crate) fn cf_metadata(&self) -> GraphResult<&ColumnFamily>;

    // These will be added by M04-T13a:
    // pub fn get_schema_version(&self) -> GraphResult<u32>;
    // pub fn set_schema_version(&self, version: u32) -> GraphResult<()>;
    // pub fn apply_migrations(&self) -> GraphResult<u32>;
    // pub fn open_and_migrate(...) -> GraphResult<Self>;
}
```

### Error Types Available (crates/context-graph-graph/src/error.rs)

```rust
pub enum GraphError {
    // Migration-relevant errors
    MigrationFailed(String),  // Use this for migration failures
    CorruptedData { location: String, details: String },
    Storage(String),
    ColumnFamilyNotFound(String),
}

pub type GraphResult<T> = Result<T, GraphError>;
```

### Existing Metadata CF Tests

The M04-T12 tests already verify:
- Metadata CF exists and is writable
- Key-value storage works (`schema_version` key tested)
- Data persists across reopen

---

## Context

Schema migrations ensure database compatibility across software versions. The migration system tracks schema versions in a dedicated metadata CF and applies necessary transformations when opening databases created by older versions. This enables safe evolution of storage formats without data loss.

**Key Principle**: Version 0 means no version stored (brand new or pre-versioned database). All new databases start at version 0 and are migrated to SCHEMA_VERSION.

## Scope

### In Scope
- Schema version constant and tracking
- `get_schema_version()` from metadata CF
- `set_schema_version()` to metadata CF
- `migrate()` method that applies migrations incrementally
- Migration v1 (initial schema verification)
- Idempotent migration execution
- `open_and_migrate()` convenience method

### Out of Scope
- Actual data transformations (v1 is initial)
- Rollback support (fail fast instead)
- Online migrations (require downtime)

---

## Definition of Done

### File to Create: `crates/context-graph-graph/src/storage/migrations.rs`

```rust
//! Schema migration system with version tracking.
//!
//! Ensures database compatibility across software versions by tracking
//! schema versions and applying incremental migrations.
//!
//! # Constitution Reference
//!
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - rules: Result<T,E> for fallible ops
//!
//! # Migration Philosophy
//!
//! - Version 0: No version stored (brand new or pre-versioned DB)
//! - Migrations applied incrementally: 0 → 1 → 2 → ...
//! - Each migration is idempotent (running twice is safe)
//! - Fail fast on errors - no partial migrations

use crate::error::{GraphError, GraphResult};
use super::rocksdb::GraphStorage;
use super::CF_METADATA;

/// Current schema version.
///
/// Increment this when making incompatible storage changes.
/// Each increment requires a corresponding migration function.
pub const SCHEMA_VERSION: u32 = 1;

/// Key for schema version in metadata CF.
const SCHEMA_VERSION_KEY: &[u8] = b"schema_version";

/// Migration function signature.
type MigrationFn = fn(&GraphStorage) -> GraphResult<()>;

/// Schema migration registry.
///
/// Holds all registered migrations and applies them incrementally.
pub struct Migrations {
    /// Registered migrations: (target_version, migration_function)
    migrations: Vec<(u32, MigrationFn)>,
}

impl Migrations {
    /// Create migration registry with all known migrations.
    pub fn new() -> Self {
        let mut migrations = Self {
            migrations: Vec::new(),
        };

        // Register all migrations in order
        migrations.register(1, migration_v1);
        // Future: migrations.register(2, migration_v2);

        migrations
    }

    /// Register a migration for a specific version.
    fn register(&mut self, version: u32, migration: MigrationFn) {
        self.migrations.push((version, migration));
        // Keep sorted by version for incremental application
        self.migrations.sort_by_key(|(v, _)| *v);
    }

    /// Apply all pending migrations.
    ///
    /// # Arguments
    /// * `storage` - GraphStorage instance (already opened)
    ///
    /// # Returns
    /// * `GraphResult<u32>` - Final schema version after migrations
    ///
    /// # Errors
    /// * `GraphError::MigrationFailed` - Migration failed (fail fast)
    /// * `GraphError::CorruptedData` - Invalid version data
    pub fn migrate(&self, storage: &GraphStorage) -> GraphResult<u32> {
        let current_version = storage.get_schema_version()?;

        log::info!(
            "Migration check: current_version={}, target_version={}",
            current_version,
            SCHEMA_VERSION
        );

        if current_version >= SCHEMA_VERSION {
            log::info!("No migration needed - already at version {}", current_version);
            return Ok(current_version);
        }

        // Apply each migration in order
        for (version, migration) in &self.migrations {
            if *version > current_version {
                log::info!("BEFORE: Applying migration v{}", version);

                migration(storage).map_err(|e| {
                    log::error!("MIGRATION FAILED at v{}: {}", version, e);
                    GraphError::MigrationFailed(format!(
                        "Migration to v{} failed: {}",
                        version, e
                    ))
                })?;

                storage.set_schema_version(*version)?;
                log::info!("AFTER: Migration v{} complete, version set", version);
            }
        }

        let final_version = storage.get_schema_version()?;
        log::info!("Migration complete: final_version={}", final_version);

        Ok(final_version)
    }

    /// Check if migrations are needed.
    pub fn needs_migration(&self, storage: &GraphStorage) -> GraphResult<bool> {
        let current = storage.get_schema_version()?;
        Ok(current < SCHEMA_VERSION)
    }

    /// Get current schema version from storage.
    pub fn current_version(&self, storage: &GraphStorage) -> GraphResult<u32> {
        storage.get_schema_version()
    }

    /// Get target schema version.
    pub fn target_version(&self) -> u32 {
        SCHEMA_VERSION
    }

    /// Get information about all registered migrations.
    pub fn list_migrations(&self) -> Vec<MigrationInfo> {
        self.migrations
            .iter()
            .map(|(version, _)| MigrationInfo {
                version: *version,
                description: match version {
                    1 => "Initial schema: adjacency, hyperbolic, entailment_cones, faiss_ids, nodes, metadata CFs",
                    _ => "Unknown migration",
                },
            })
            .collect()
    }
}

impl Default for Migrations {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Migration Functions ==========

/// Migration v1: Initial schema.
///
/// Creates the foundational schema with all column families.
/// For new databases, this validates that CFs were created correctly.
/// For pre-versioned databases, this verifies the schema is compatible.
fn migration_v1(storage: &GraphStorage) -> GraphResult<()> {
    log::info!("Migration v1: Verifying initial schema");

    // Column families are created during open(), so this migration
    // validates the schema is correctly set up by attempting to access each CF.

    // Verify hyperbolic CF exists and is accessible
    let hyperbolic_count = storage.hyperbolic_count()?;
    log::debug!("  hyperbolic CF: {} entries", hyperbolic_count);

    // Verify cones CF exists and is accessible
    let cone_count = storage.cone_count()?;
    log::debug!("  entailment_cones CF: {} entries", cone_count);

    // Verify adjacency CF exists and is accessible
    let adjacency_count = storage.adjacency_count()?;
    log::debug!("  adjacency CF: {} entries", adjacency_count);

    log::info!("Migration v1: Initial schema verified successfully");

    Ok(())
}

// ========== Future Migrations (Placeholder) ==========

// fn migration_v2(storage: &GraphStorage) -> GraphResult<()> {
//     log::info!("Migration v2: <description>");
//     // Example: Add new column family, transform data, etc.
//     todo!("Implement when v2 schema changes are needed")
// }

// ========== Migration Metadata ==========

/// Information about a migration.
#[derive(Debug, Clone)]
pub struct MigrationInfo {
    /// Schema version this migration produces.
    pub version: u32,
    /// Human-readable description.
    pub description: &'static str,
}
```

### File to Modify: `crates/context-graph-graph/src/storage/rocksdb.rs`

Add these methods to the `GraphStorage` impl block:

```rust
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
                bytes[..4].try_into().expect("verified 4 bytes above")
            );
            log::trace!("get_schema_version: {}", version);
            Ok(version)
        }
        None => {
            log::trace!("get_schema_version: 0 (not set)");
            Ok(0)  // No version stored = version 0
        }
    }
}

/// Set schema version in metadata CF.
pub fn set_schema_version(&self, version: u32) -> GraphResult<()> {
    let cf = self.cf_metadata()?;
    self.db.put_cf(cf, b"schema_version", version.to_le_bytes())?;
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
pub fn open_and_migrate<P: AsRef<std::path::Path>>(
    path: P,
    config: super::StorageConfig,
) -> GraphResult<Self> {
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
```

### File to Modify: `crates/context-graph-graph/src/storage/mod.rs`

Add after existing module declarations:

```rust
pub mod migrations;

// Re-export migration types
pub use migrations::{Migrations, MigrationInfo, SCHEMA_VERSION};
```

---

## Constraints

| Constraint | Value | Enforcement |
|------------|-------|-------------|
| Schema version type | u32 (4 bytes LE) | `to_le_bytes()` / `from_le_bytes()` |
| Version key | `b"schema_version"` | Constant |
| Initial version | 0 (not stored) | `get_schema_version()` returns 0 for None |
| Current version | 1 | `SCHEMA_VERSION` constant |
| Migration order | Ascending by version | `sort_by_key` in register |
| Idempotency | Required | Running twice = same result |
| Error handling | Fail fast | `MigrationFailed` error on failure |

---

## Acceptance Criteria

### Build & Test
- [x] `cargo build -p context-graph-graph` compiles without errors
- [x] `cargo test -p context-graph-graph migration` passes all tests
- [x] `cargo clippy -p context-graph-graph -- -D warnings` passes

### Functional
- [x] `SCHEMA_VERSION` constant equals 1
- [x] `get_schema_version()` returns 0 for new database
- [x] `get_schema_version()` returns stored version for migrated database
- [x] `migrate()` applies migrations incrementally
- [x] `migrate()` is idempotent (running twice is safe)
- [x] `open_and_migrate()` opens and migrates in one call
- [x] Migration v1 validates all required CFs exist
- [x] Version persists to disk after migration

---

## Full State Verification Requirements

### 1. Define Source of Truth

| Data Type | Source of Truth | Verification Method |
|-----------|-----------------|---------------------|
| Schema Version | RocksDB `metadata` CF, key `schema_version` | `db.get_cf(metadata_cf, b"schema_version")` |
| Migration Applied | Version incremented in metadata | `get_schema_version() == expected_version` |

### 2. Execute & Inspect Pattern

Every migration MUST be followed by version verification:

```rust
// CORRECT: Migrate then verify
let before = storage.get_schema_version()?;
let after = storage.apply_migrations()?;
let verified = storage.get_schema_version()?;
assert_eq!(after, verified, "Version must match after migration");
println!("MIGRATED: {} -> {} (verified: {})", before, after, verified);

// WRONG: Assume migration succeeded
storage.apply_migrations()?;  // No verification!
```

### 3. Required Edge Case Tests (3 minimum with BEFORE/AFTER logging)

```rust
#[test]
fn test_edge_case_new_database_migration() {
    // Edge case: Brand new database should migrate to v1
    let temp_dir = tempfile::tempdir().unwrap();

    println!("BEFORE: Opening new database");
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let before_version = storage.get_schema_version().unwrap();
    println!("BEFORE MIGRATION: version = {}", before_version);
    assert_eq!(before_version, 0, "New DB should have version 0");

    let final_version = storage.apply_migrations().unwrap();
    println!("AFTER MIGRATION: version = {}", final_version);

    // VERIFY: Read version from storage directly
    let verified = storage.get_schema_version().unwrap();
    println!("VERIFIED: stored version = {}", verified);

    assert_eq!(final_version, SCHEMA_VERSION);
    assert_eq!(verified, SCHEMA_VERSION);
}

#[test]
fn test_edge_case_idempotent_migration() {
    // Edge case: Running migration twice should be safe
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: First migration");
    let v1 = storage.apply_migrations().unwrap();
    println!("AFTER FIRST: version = {}", v1);

    println!("BEFORE: Second migration (idempotent check)");
    let v2 = storage.apply_migrations().unwrap();
    println!("AFTER SECOND: version = {}", v2);

    assert_eq!(v1, v2, "Idempotent migration must return same version");
    assert_eq!(v1, SCHEMA_VERSION);
}

#[test]
fn test_edge_case_corrupted_version() {
    // Edge case: Corrupted schema version should fail fast
    let temp_dir = tempfile::tempdir().unwrap();

    // Write invalid version data directly
    {
        let db_opts = get_db_options();
        let config = StorageConfig::default();
        let cf_descriptors = get_column_family_descriptors(&config).unwrap();
        let db = rocksdb::DB::open_cf_descriptors(&db_opts, temp_dir.path(), cf_descriptors).unwrap();

        let metadata_cf = db.cf_handle(CF_METADATA).unwrap();
        // Write 3 bytes instead of 4 (corrupted)
        db.put_cf(metadata_cf, b"schema_version", &[1, 2, 3]).unwrap();
        println!("BEFORE: Wrote corrupted version (3 bytes instead of 4)");
    }

    // Reopen and try to read version
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: Attempting to read corrupted version");
    let result = storage.get_schema_version();
    println!("AFTER: Result = {:?}", result.is_err());

    assert!(result.is_err(), "Must detect corrupted version data");
    match result {
        Err(GraphError::CorruptedData { location, details }) => {
            assert!(location.contains("schema_version"));
            assert!(details.contains("4 bytes") || details.contains("3"));
        }
        _ => panic!("Expected CorruptedData error"),
    }
}
```

### 4. Evidence of Success

Test output MUST show:
```
BEFORE: Opening new database
BEFORE MIGRATION: version = 0
Migration v1: Verifying initial schema
AFTER MIGRATION: version = 1
VERIFIED: stored version = 1
```

---

## Test Cases (NO MOCKS - Real RocksDB Only)

Add to `crates/context-graph-graph/tests/storage_tests.rs`:

```rust
// ========== M04-T13a: Migration Tests ==========

use context_graph_graph::storage::{
    GraphStorage, StorageConfig, Migrations, SCHEMA_VERSION,
    get_db_options, get_column_family_descriptors, CF_METADATA,
};
use context_graph_graph::error::GraphError;

#[test]
fn test_schema_version_constant() {
    println!("BEFORE: Checking SCHEMA_VERSION constant");
    assert_eq!(SCHEMA_VERSION, 1, "Initial schema version must be 1");
    println!("AFTER: SCHEMA_VERSION = {}", SCHEMA_VERSION);
}

#[test]
fn test_new_database_has_version_zero() {
    let temp_dir = tempfile::tempdir().unwrap();

    println!("BEFORE: Opening new database");
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let version = storage.get_schema_version().unwrap();
    println!("AFTER: version = {}", version);

    assert_eq!(version, 0, "New database must have version 0");
}

#[test]
fn test_migration_updates_version() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: version = {}", storage.get_schema_version().unwrap());

    let final_version = storage.apply_migrations().unwrap();

    println!("AFTER: version = {}", final_version);
    assert_eq!(final_version, SCHEMA_VERSION);

    // VERIFY: Read back from storage
    let stored = storage.get_schema_version().unwrap();
    assert_eq!(stored, SCHEMA_VERSION, "Version must persist");
}

#[test]
fn test_migration_is_idempotent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: First migration");
    let v1 = storage.apply_migrations().unwrap();

    println!("BEFORE: Second migration");
    let v2 = storage.apply_migrations().unwrap();

    println!("AFTER: v1={}, v2={}", v1, v2);
    assert_eq!(v1, v2, "Idempotent: running twice returns same version");
}

#[test]
fn test_open_and_migrate() {
    let temp_dir = tempfile::tempdir().unwrap();

    println!("BEFORE: open_and_migrate");
    let storage = GraphStorage::open_and_migrate(
        temp_dir.path(),
        StorageConfig::default(),
    ).unwrap();

    let version = storage.get_schema_version().unwrap();
    println!("AFTER: version = {}", version);

    assert_eq!(version, SCHEMA_VERSION);
    assert!(!storage.needs_migrations().unwrap(), "Should not need more migrations");
}

#[test]
fn test_version_persists_across_reopen() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().to_path_buf();

    // First open: migrate
    {
        let storage = GraphStorage::open_default(&db_path).unwrap();
        storage.apply_migrations().unwrap();
        println!("BEFORE CLOSE: Migrated to v{}", storage.get_schema_version().unwrap());
    }

    // Second open: verify version persisted
    {
        let storage = GraphStorage::open_default(&db_path).unwrap();
        let version = storage.get_schema_version().unwrap();
        println!("AFTER REOPEN: version = {}", version);

        assert_eq!(version, SCHEMA_VERSION, "Version must persist across reopen");
        assert!(!storage.needs_migrations().unwrap());
    }
}

#[test]
fn test_list_migrations() {
    let migrations = Migrations::new();
    let list = migrations.list_migrations();

    println!("BEFORE: Listing migrations");
    for info in &list {
        println!("  v{}: {}", info.version, info.description);
    }
    println!("AFTER: {} migrations registered", list.len());

    assert_eq!(list.len(), 1, "Should have 1 migration (v1)");
    assert_eq!(list[0].version, 1);
    assert!(list[0].description.contains("Initial schema"));
}

#[test]
fn test_needs_migration_before_and_after() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: needs_migrations = {}", storage.needs_migrations().unwrap());
    assert!(storage.needs_migrations().unwrap(), "New DB needs migration");

    storage.apply_migrations().unwrap();

    println!("AFTER: needs_migrations = {}", storage.needs_migrations().unwrap());
    assert!(!storage.needs_migrations().unwrap(), "Migrated DB doesn't need migration");
}
```

---

## Verification Commands

```bash
# Build
cargo build -p context-graph-graph

# Run all migration tests
cargo test -p context-graph-graph migration -- --nocapture

# Run specific tests
cargo test -p context-graph-graph test_schema_version -- --nocapture
cargo test -p context-graph-graph test_migration -- --nocapture

# Clippy (must pass with no warnings)
cargo clippy -p context-graph-graph -- -D warnings
```

---

## Sherlock-Holmes Final Verification

**YOU MUST USE THE sherlock-holmes SUBAGENT TO VERIFY THIS TASK IS COMPLETE.**

### Verification Checklist for Sherlock

1. **File Exists**: `crates/context-graph-graph/src/storage/migrations.rs` exists and is not empty
2. **Module Exported**: `storage/mod.rs` contains `pub mod migrations;` and re-exports
3. **GraphStorage Extended**: `rocksdb.rs` contains `get_schema_version`, `set_schema_version`, `apply_migrations`, `open_and_migrate`
4. **Compiles**: `cargo build -p context-graph-graph` succeeds
5. **Tests Pass**: `cargo test -p context-graph-graph migration` all pass
6. **No Clippy Warnings**: `cargo clippy -p context-graph-graph -- -D warnings` passes
7. **Physical Verification**:
   - Open new RocksDB database
   - Verify `get_schema_version()` returns 0
   - Apply migrations
   - Verify `get_schema_version()` returns 1
   - Reopen database
   - Verify version persisted (still 1)
8. **Edge Cases Tested**:
   - New database migrates 0 → 1
   - Idempotent: running twice returns same version
   - Corrupted version data detected

### Sherlock Invocation

```
Use Task tool with subagent_type="sherlock-holmes" and prompt:

"Forensically verify M04-T13a Storage Migrations implementation:
1. Verify migrations.rs exists with Migrations struct and SCHEMA_VERSION=1
2. Verify rocksdb.rs has get_schema_version, set_schema_version, apply_migrations, open_and_migrate
3. Verify mod.rs exports the migrations module
4. Run cargo build and cargo test
5. Execute tests with --nocapture to see BEFORE/AFTER logs
6. Verify schema version lifecycle:
   - New DB: version = 0
   - After migration: version = 1
   - After reopen: version = 1 (persisted)
7. Verify idempotency: apply_migrations twice returns same result
8. Check that no unwrap() exists in production code (only in tests)
9. Report: VERIFIED or FAILED with evidence"
```

---

## Implementation Order

1. Create `storage/migrations.rs` with:
   - `SCHEMA_VERSION` constant (= 1)
   - `SCHEMA_VERSION_KEY` constant
   - `Migrations` struct with registry
   - `migration_v1()` function
   - `MigrationInfo` struct

2. Add to `storage/rocksdb.rs`:
   - `get_schema_version()` method
   - `set_schema_version()` method
   - `apply_migrations()` method
   - `needs_migrations()` method
   - `open_and_migrate()` method

3. Add to `storage/mod.rs`:
   - `pub mod migrations;`
   - Re-exports

4. Add tests to `tests/storage_tests.rs`:
   - Schema version constant test
   - New database version test
   - Migration update test
   - Idempotency test
   - Persistence test
   - Corrupted data test

5. Run sherlock-holmes verification

---

## Dependencies

This task depends on M04-T13 (GraphStorage) being complete. Specifically:
- `GraphStorage` struct must exist
- `open()` and `open_default()` must work
- `cf_metadata()` must be accessible
- `hyperbolic_count()`, `cone_count()`, `adjacency_count()` must work (for v1 migration validation)
