//! Autonomous North Star storage extensions.
//!
//! Adds 7 column families for autonomous system storage:
//! - `autonomous_config` - Singleton AutonomousConfig storage
//! - `adaptive_threshold_state` - Singleton threshold state storage
//! - `drift_history` - Time-series DriftDataPoint storage
//! - `goal_activity_metrics` - Per-goal activity tracking
//! - `autonomous_lineage` - Lineage events for traceability
//! - `consolidation_history` - Memory consolidation records
//! - `memory_curation` - Memory curation state records
//!
//! # Column Families (7 new)
//!
//! | Name | Purpose | Key Format | Value |
//! |------|---------|------------|-------|
//! | autonomous_config | Singleton config | "config" (6 bytes) | AutonomousConfig |
//! | adaptive_threshold_state | Singleton state | "state" (5 bytes) | AdaptiveThresholdState |
//! | drift_history | Drift data points | timestamp_ms:uuid (24 bytes) | DriftDataPoint |
//! | goal_activity_metrics | Per-goal activity | uuid (16 bytes) | GoalActivityMetrics |
//! | autonomous_lineage | Lineage events | timestamp_ms:uuid (24 bytes) | LineageEvent |
//! | consolidation_history | Consolidation records | timestamp_ms:uuid (24 bytes) | ConsolidationRecord |
//! | memory_curation | Memory curation state | uuid (16 bytes) | MemoryCurationState |
//!
//! # Key Formats
//!
//! - **Singleton keys**: Fixed string like "config" or "state"
//! - **UUID keys**: 16 bytes (MemoryId, GoalId)
//! - **Timestamp+UUID keys**: 24 bytes (timestamp_ms BE + uuid)
//!
//! The timestamp prefix enables efficient time-range scans with RocksDB prefix extractors.
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! All key parsing functions panic on invalid input. No silent fallbacks or default values.
//! This ensures data integrity and makes bugs immediately visible.
//!
//! # Types Reference
//!
//! All types are defined in `context-graph-core/src/autonomous/`:
//! - `AutonomousConfig` (workflow.rs)
//! - `AdaptiveThresholdState` (thresholds.rs)
//! - `DriftDataPoint`, `DriftState` (drift.rs)
//! - `GoalActivityMetrics`, `GoalState` (evolution.rs)
//! - `MemoryCurationState` (curation.rs)
//! - `GoalId`, `MemoryId` (bootstrap.rs, curation.rs)

pub mod column_families;
pub mod rocksdb_store;
pub mod schema;

#[cfg(test)]
mod tests;

// Re-export column family types
pub use column_families::{
    // CF option builders
    adaptive_threshold_state_cf_options,
    autonomous_config_cf_options,
    autonomous_lineage_cf_options,
    consolidation_history_cf_options,
    drift_history_cf_options,
    // Descriptor getter
    get_autonomous_cf_descriptors,
    goal_activity_metrics_cf_options,
    memory_curation_cf_options,
    // CF arrays and counts
    AUTONOMOUS_CFS,
    AUTONOMOUS_CF_COUNT,
    // CF name constants
    CF_ADAPTIVE_THRESHOLD_STATE,
    CF_AUTONOMOUS_CONFIG,
    CF_AUTONOMOUS_LINEAGE,
    CF_CONSOLIDATION_HISTORY,
    CF_DRIFT_HISTORY,
    CF_GOAL_ACTIVITY_METRICS,
    CF_MEMORY_CURATION,
};

// Re-export schema types
pub use schema::{
    // Autonomous lineage keys
    autonomous_lineage_key,
    autonomous_lineage_timestamp_prefix,
    // Consolidation history keys
    consolidation_history_key,
    consolidation_history_timestamp_prefix,
    // Drift history keys
    drift_history_key,
    drift_history_timestamp_prefix,
    // Goal activity metrics keys
    goal_activity_metrics_key,
    // Memory curation keys
    memory_curation_key,
    parse_autonomous_lineage_key,
    parse_consolidation_history_key,
    parse_drift_history_key,
    parse_goal_activity_metrics_key,
    parse_memory_curation_key,
    // Singleton key constants
    ADAPTIVE_THRESHOLD_STATE_KEY,
    AUTONOMOUS_CONFIG_KEY,
};

// Re-export RocksDB store types
pub use rocksdb_store::{
    // Configuration
    AutonomousStoreConfig,
    // Error types
    AutonomousStoreError,
    AutonomousStoreResult,
    ConsolidationRecord,
    // Storage-local types (not in context-graph-core)
    LineageEvent,
    // Main store
    RocksDbAutonomousStore,
};
