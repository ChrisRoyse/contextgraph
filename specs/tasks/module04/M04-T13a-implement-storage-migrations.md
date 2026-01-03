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
status: "pending"
priority: "medium"
estimated_hours: 2
sequence: 18
depends_on:
  - "M04-T13"
spec_refs:
  - "TECH-GRAPH-004 Section 4.2"
files_to_create:
  - path: "crates/context-graph-graph/src/storage/migrations.rs"
    description: "Schema migration system with version tracking"
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "Add migrations module"
  - path: "crates/context-graph-graph/src/storage/rocksdb.rs"
    description: "Add migration support to GraphStorage"
test_file: "crates/context-graph-graph/tests/storage_tests.rs"
---

## Context

Schema migrations ensure database compatibility across software versions. The migration system tracks schema versions in a dedicated metadata CF and applies necessary transformations when opening databases created by older versions. This enables safe evolution of storage formats without data loss.

## Scope

### In Scope
- Schema version constant and tracking
- get_schema_version() from metadata CF
- set_schema_version() to metadata CF
- migrate() method that applies migrations incrementally
- Migration v1 (initial schema)
- Idempotent migration execution

### Out of Scope
- Actual data transformations (v1 is initial)
- Rollback support
- Online migrations

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/storage/migrations.rs

use crate::error::{GraphError, GraphResult};
use super::rocksdb::GraphStorage;
use super::CF_METADATA;

/// Current schema version
/// Increment when making incompatible changes
pub const SCHEMA_VERSION: u32 = 1;

/// Key for schema version in metadata CF
const SCHEMA_VERSION_KEY: &[u8] = b"schema_version";

/// Migration function type
type MigrationFn = fn(&GraphStorage) -> GraphResult<()>;

/// Schema migration system
pub struct Migrations {
    /// Registered migrations (version -> migration function)
    migrations: Vec<(u32, MigrationFn)>,
}

impl Migrations {
    /// Create migrations registry with all known migrations
    pub fn new() -> Self {
        let mut migrations = Self {
            migrations: Vec::new(),
        };

        // Register all migrations
        migrations.register(1, migration_v1);

        migrations
    }

    /// Register a migration for a specific version
    fn register(&mut self, version: u32, migration: MigrationFn) {
        self.migrations.push((version, migration));
        // Keep sorted by version
        self.migrations.sort_by_key(|(v, _)| *v);
    }

    /// Apply all pending migrations
    ///
    /// # Arguments
    /// * `storage` - Graph storage instance
    ///
    /// # Returns
    /// * `GraphResult<u32>` - Final schema version after migrations
    pub fn migrate(&self, storage: &GraphStorage) -> GraphResult<u32> {
        let current_version = storage.get_schema_version()?;

        if current_version >= SCHEMA_VERSION {
            // Already at or above current version
            return Ok(current_version);
        }

        // Apply each migration in order
        for (version, migration) in &self.migrations {
            if *version > current_version {
                log::info!("Applying migration v{}", version);
                migration(storage)?;
                storage.set_schema_version(*version)?;
                log::info!("Migration v{} complete", version);
            }
        }

        Ok(SCHEMA_VERSION)
    }

    /// Check if migrations are needed
    pub fn needs_migration(&self, storage: &GraphStorage) -> GraphResult<bool> {
        let current = storage.get_schema_version()?;
        Ok(current < SCHEMA_VERSION)
    }

    /// Get current schema version from storage
    pub fn current_version(&self, storage: &GraphStorage) -> GraphResult<u32> {
        storage.get_schema_version()
    }

    /// Get target schema version
    pub fn target_version(&self) -> u32 {
        SCHEMA_VERSION
    }
}

impl Default for Migrations {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Migration Functions ==========

/// Migration v1: Initial schema
///
/// Creates the foundational schema with all column families.
/// This is applied to brand new databases or pre-versioned databases.
fn migration_v1(storage: &GraphStorage) -> GraphResult<()> {
    // v1 is the initial schema
    // Column families are created during open(), so this migration
    // just validates the schema is correctly set up.

    // Verify all required CFs exist by attempting to access them
    let _ = storage.hyperbolic_count()?;
    let _ = storage.cone_count()?;
    let _ = storage.adjacency_count()?;

    log::info!("Migration v1: Verified initial schema");

    Ok(())
}

// ========== Future Migrations ==========

// fn migration_v2(storage: &GraphStorage) -> GraphResult<()> {
//     // Example: Add new column family or transform data
//     todo!("Implement when v2 schema changes are needed")
// }

// ========== GraphStorage Extensions ==========

impl GraphStorage {
    /// Get schema version from metadata CF
    ///
    /// Returns 0 if no version is stored (pre-migration database)
    pub fn get_schema_version(&self) -> GraphResult<u32> {
        let cf = self.cf_metadata()?;

        match self.db.get_cf(cf, SCHEMA_VERSION_KEY)? {
            Some(bytes) => {
                if bytes.len() != 4 {
                    return Err(GraphError::CorruptedData(
                        format!("Schema version has invalid size: {}", bytes.len())
                    ));
                }
                Ok(u32::from_le_bytes(bytes[..4].try_into().unwrap()))
            }
            None => Ok(0), // No version stored = version 0
        }
    }

    /// Set schema version in metadata CF
    pub fn set_schema_version(&self, version: u32) -> GraphResult<()> {
        let cf = self.cf_metadata()?;
        self.db.put_cf(cf, SCHEMA_VERSION_KEY, version.to_le_bytes())?;
        Ok(())
    }

    /// Apply all pending migrations
    ///
    /// Should be called after open() to ensure database is up to date.
    pub fn apply_migrations(&self) -> GraphResult<u32> {
        let migrations = Migrations::new();
        migrations.migrate(self)
    }

    /// Check if database needs migrations
    pub fn needs_migrations(&self) -> GraphResult<bool> {
        let migrations = Migrations::new();
        migrations.needs_migration(self)
    }

    /// Open storage and apply migrations
    ///
    /// Convenience method that combines open() with migrations.
    pub fn open_and_migrate<P: AsRef<std::path::Path>>(
        path: P,
        config: super::StorageConfig,
    ) -> GraphResult<Self> {
        let storage = Self::open(path, config)?;
        storage.apply_migrations()?;
        Ok(storage)
    }
}

// ========== Migration Metadata ==========

/// Information about a migration
#[derive(Debug, Clone)]
pub struct MigrationInfo {
    /// Schema version this migration produces
    pub version: u32,
    /// Description of what this migration does
    pub description: &'static str,
}

impl Migrations {
    /// Get information about all registered migrations
    pub fn list_migrations(&self) -> Vec<MigrationInfo> {
        self.migrations
            .iter()
            .map(|(version, _)| MigrationInfo {
                version: *version,
                description: match version {
                    1 => "Initial schema with adjacency, hyperbolic, entailment_cones CFs",
                    _ => "Unknown migration",
                },
            })
            .collect()
    }
}
```

### Constraints
- SCHEMA_VERSION is the current/target version
- Version 0 means no version stored (brand new or pre-versioned DB)
- Migrations are applied in order from current+1 to SCHEMA_VERSION
- Each migration is idempotent (running twice is safe)
- Migration failures should leave DB in consistent state

### Acceptance Criteria
- [ ] SCHEMA_VERSION constant defined (v1 = 1)
- [ ] get_schema_version() reads from metadata CF
- [ ] migrate() applies migrations incrementally
- [ ] migrate() is idempotent (running twice is safe)
- [ ] Migration v1 creates/validates all required CFs
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. get_schema_version():
   - Read from metadata CF with SCHEMA_VERSION_KEY
   - If not found, return 0 (version 0)
   - Parse u32 from bytes

2. migrate():
   - Get current version
   - For each registered migration where version > current:
     - Run migration function
     - Update schema version in metadata
   - Return final version

3. migration_v1():
   - Verify all CFs exist (they're created during open)
   - No data transformations needed for initial schema

### Edge Cases
- Brand new database: Start from version 0, apply v1
- Already migrated: Skip all migrations
- Corrupted version bytes: Return CorruptedData error
- Migration failure: Leave version at last successful migration

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph migration
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] New database gets version 1 after migration
- [ ] Existing database skips migration
- [ ] Running migrate() twice doesn't change anything
- [ ] Version is persisted to disk

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_schema_version_constant() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn test_new_database_gets_migrated() {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        // New database has version 0
        assert_eq!(storage.get_schema_version().unwrap(), 0);

        // Apply migrations
        let final_version = storage.apply_migrations().unwrap();
        assert_eq!(final_version, SCHEMA_VERSION);

        // Version is now current
        assert_eq!(storage.get_schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn test_migration_is_idempotent() {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        // Apply migrations twice
        let v1 = storage.apply_migrations().unwrap();
        let v2 = storage.apply_migrations().unwrap();

        // Both return same version
        assert_eq!(v1, v2);
        assert_eq!(v1, SCHEMA_VERSION);
    }

    #[test]
    fn test_open_and_migrate() {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_and_migrate(
            dir.path(),
            StorageConfig::default(),
        ).unwrap();

        // Already migrated
        assert_eq!(storage.get_schema_version().unwrap(), SCHEMA_VERSION);
        assert!(!storage.needs_migrations().unwrap());
    }

    #[test]
    fn test_list_migrations() {
        let migrations = Migrations::new();
        let list = migrations.list_migrations();

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].version, 1);
        assert!(list[0].description.contains("Initial schema"));
    }
}
```
