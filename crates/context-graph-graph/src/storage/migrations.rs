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

use super::storage_impl::GraphStorage;
use crate::error::{GraphError, GraphResult};

/// Current schema version.
///
/// Increment this when making incompatible storage changes.
/// Each increment requires a corresponding migration function.
pub const SCHEMA_VERSION: u32 = 1;

/// Key for schema version in metadata CF.
#[allow(dead_code)]
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
    #[must_use]
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
            log::info!(
                "No migration needed - already at version {}",
                current_version
            );
            return Ok(current_version);
        }

        // Apply each migration in order
        for (version, migration) in &self.migrations {
            if *version > current_version {
                log::info!("BEFORE: Applying migration v{}", version);

                migration(storage).map_err(|e| {
                    log::error!("MIGRATION FAILED at v{}: {}", version, e);
                    GraphError::MigrationFailed(format!("Migration to v{} failed: {}", version, e))
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
    #[must_use]
    pub fn target_version(&self) -> u32 {
        SCHEMA_VERSION
    }

    /// Get information about all registered migrations.
    #[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version_constant() {
        assert_eq!(SCHEMA_VERSION, 1, "Initial schema version must be 1");
    }

    #[test]
    fn test_migrations_new() {
        let migrations = Migrations::new();
        assert_eq!(migrations.target_version(), 1);
    }

    #[test]
    fn test_list_migrations() {
        let migrations = Migrations::new();
        let list = migrations.list_migrations();
        assert_eq!(list.len(), 1, "Should have 1 migration (v1)");
        assert_eq!(list[0].version, 1);
        assert!(list[0].description.contains("Initial schema"));
    }

    #[test]
    fn test_migrations_default() {
        let migrations = Migrations::default();
        assert_eq!(migrations.target_version(), SCHEMA_VERSION);
    }
}
