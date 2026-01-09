//! RocksDB-backed storage for autonomous North Star system.
//!
//! This module provides persistent storage for the autonomous system including:
//! - Singleton AutonomousConfig and AdaptiveThresholdState
//! - Time-series DriftDataPoint history
//! - Per-goal GoalActivityMetrics
//! - Lineage events for traceability
//! - Consolidation history records
//! - Memory curation state
//!
//! # FAIL FAST Policy
//!
//! **NO FALLBACKS. NO MOCK DATA. ERRORS ARE FATAL.**
//!
//! Every RocksDB operation that fails returns a detailed error with:
//! - The operation that failed
//! - The column family involved
//! - The key being accessed
//! - The underlying RocksDB error
//!
//! # Thread Safety
//!
//! The store is thread-safe for concurrent reads and writes via RocksDB's internal locking.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rocksdb::{Cache, ColumnFamily, Options, DB};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::autonomous::{
    AdaptiveThresholdState, AutonomousConfig, DriftDataPoint, GoalActivityMetrics, GoalId,
    MemoryCurationState, MemoryId,
};

use super::column_families::{
    get_autonomous_cf_descriptors, AUTONOMOUS_CFS, CF_ADAPTIVE_THRESHOLD_STATE,
    CF_AUTONOMOUS_CONFIG, CF_AUTONOMOUS_LINEAGE, CF_CONSOLIDATION_HISTORY, CF_DRIFT_HISTORY,
    CF_GOAL_ACTIVITY_METRICS, CF_MEMORY_CURATION,
};
use super::schema::{
    autonomous_lineage_key, autonomous_lineage_timestamp_prefix, consolidation_history_key,
    consolidation_history_timestamp_prefix, drift_history_key, drift_history_timestamp_prefix,
    goal_activity_metrics_key, memory_curation_key, parse_autonomous_lineage_key,
    parse_consolidation_history_key, parse_drift_history_key, parse_goal_activity_metrics_key,
    parse_memory_curation_key, ADAPTIVE_THRESHOLD_STATE_KEY, AUTONOMOUS_CONFIG_KEY,
};

// ============================================================================
// Storage-local Types (not defined in context-graph-core)
// ============================================================================

/// Lineage event for traceability of autonomous operations.
///
/// Records significant system events for audit and debugging.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineageEvent {
    /// Unique event identifier.
    pub id: Uuid,
    /// Event timestamp.
    pub timestamp: DateTime<Utc>,
    /// Event type (e.g., "bootstrap", "drift_correction", "goal_evolution").
    pub event_type: String,
    /// Human-readable description.
    pub description: String,
    /// Associated goal ID, if applicable.
    pub goal_id: Option<GoalId>,
    /// Associated memory ID, if applicable.
    pub memory_id: Option<MemoryId>,
    /// Additional metadata as JSON string.
    pub metadata: Option<String>,
}

impl LineageEvent {
    /// Create a new lineage event with auto-generated ID and current timestamp.
    pub fn new(event_type: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: event_type.into(),
            description: description.into(),
            goal_id: None,
            memory_id: None,
            metadata: None,
        }
    }

    /// Create a lineage event with goal association.
    pub fn with_goal(mut self, goal_id: GoalId) -> Self {
        self.goal_id = Some(goal_id);
        self
    }

    /// Create a lineage event with memory association.
    pub fn with_memory(mut self, memory_id: MemoryId) -> Self {
        self.memory_id = Some(memory_id);
        self
    }

    /// Create a lineage event with metadata.
    pub fn with_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.metadata = Some(metadata.into());
        self
    }

    /// Get the timestamp in milliseconds for key generation.
    pub fn timestamp_ms(&self) -> i64 {
        self.timestamp.timestamp_millis()
    }
}

/// Record of a memory consolidation operation.
///
/// Tracks when and how memories were merged or consolidated.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsolidationRecord {
    /// Unique record identifier.
    pub id: Uuid,
    /// Consolidation timestamp.
    pub timestamp: DateTime<Utc>,
    /// Source memory IDs that were consolidated.
    pub source_memories: Vec<MemoryId>,
    /// Target memory ID (merged result).
    pub target_memory: MemoryId,
    /// Similarity score that triggered consolidation.
    pub similarity_score: f32,
    /// Alignment difference (theta_diff) at consolidation.
    pub theta_diff: f32,
    /// Whether consolidation was successful.
    pub success: bool,
    /// Error message if consolidation failed.
    pub error_message: Option<String>,
}

impl ConsolidationRecord {
    /// Create a new successful consolidation record.
    pub fn success(
        source_memories: Vec<MemoryId>,
        target_memory: MemoryId,
        similarity_score: f32,
        theta_diff: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source_memories,
            target_memory,
            similarity_score,
            theta_diff,
            success: true,
            error_message: None,
        }
    }

    /// Create a new failed consolidation record.
    pub fn failure(
        source_memories: Vec<MemoryId>,
        target_memory: MemoryId,
        similarity_score: f32,
        theta_diff: f32,
        error: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source_memories,
            target_memory,
            similarity_score,
            theta_diff,
            success: false,
            error_message: Some(error.into()),
        }
    }

    /// Get the timestamp in milliseconds for key generation.
    pub fn timestamp_ms(&self) -> i64 {
        self.timestamp.timestamp_millis()
    }
}

// ============================================================================
// Serialization Version
// ============================================================================

/// Serialization version for autonomous storage types.
///
/// Bump this when struct layout changes. Version mismatches will cause errors.
const AUTONOMOUS_STORAGE_VERSION: u8 = 1;

// ============================================================================
// Error Types - FAIL FAST with detailed context
// ============================================================================

/// Detailed error type for autonomous store operations.
///
/// Every error includes enough context for immediate debugging:
/// - Operation name
/// - Column family
/// - Key (if applicable)
/// - Underlying cause
#[derive(Debug, Error)]
pub enum AutonomousStoreError {
    /// RocksDB operation failed.
    #[error("RocksDB {operation} failed on CF '{cf}' with key '{key:?}': {source}")]
    RocksDbOperation {
        operation: &'static str,
        cf: &'static str,
        key: Option<String>,
        #[source]
        source: rocksdb::Error,
    },

    /// Database failed to open.
    #[error("Failed to open RocksDB at '{path}': {message}")]
    OpenFailed { path: String, message: String },

    /// Column family not found.
    #[error("Column family '{name}' not found in database")]
    ColumnFamilyNotFound { name: String },

    /// Serialization error.
    #[error("Serialization error for {type_name}: {message}")]
    Serialization {
        type_name: &'static str,
        message: String,
    },

    /// Deserialization error.
    #[error("Deserialization error for key '{key}' in CF '{cf}': {message}")]
    Deserialization {
        cf: &'static str,
        key: String,
        message: String,
    },

    /// Version mismatch error.
    #[error("Version mismatch in CF '{cf}': expected {expected}, got {actual}")]
    VersionMismatch {
        cf: &'static str,
        expected: u8,
        actual: u8,
    },
}

impl AutonomousStoreError {
    /// Create a RocksDB operation error.
    fn rocksdb_op(
        operation: &'static str,
        cf: &'static str,
        key: Option<&str>,
        source: rocksdb::Error,
    ) -> Self {
        Self::RocksDbOperation {
            operation,
            cf,
            key: key.map(String::from),
            source,
        }
    }
}

/// Result type for autonomous store operations.
pub type AutonomousStoreResult<T> = Result<T, AutonomousStoreError>;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for RocksDbAutonomousStore.
#[derive(Debug, Clone)]
pub struct AutonomousStoreConfig {
    /// Block cache size in bytes (default: 64MB).
    pub block_cache_size: usize,
    /// Maximum number of open files (default: 500).
    pub max_open_files: i32,
    /// Enable WAL (write-ahead log) for durability (default: true).
    pub enable_wal: bool,
    /// Create database if it doesn't exist (default: true).
    pub create_if_missing: bool,
}

impl Default for AutonomousStoreConfig {
    fn default() -> Self {
        Self {
            block_cache_size: 64 * 1024 * 1024, // 64MB
            max_open_files: 500,
            enable_wal: true,
            create_if_missing: true,
        }
    }
}

// ============================================================================
// Main Store Implementation
// ============================================================================

/// RocksDB-backed storage for autonomous North Star system.
///
/// Provides persistent storage for all autonomous system state across 7 column families.
///
/// # Thread Safety
///
/// The store is thread-safe for concurrent access via RocksDB's internal locking.
///
/// # Example
///
/// ```ignore
/// use context_graph_storage::autonomous::RocksDbAutonomousStore;
/// use tempfile::TempDir;
///
/// let tmp = TempDir::new().unwrap();
/// let store = RocksDbAutonomousStore::open(tmp.path()).unwrap();
///
/// // Store config
/// let config = AutonomousConfig::default();
/// store.store_autonomous_config(&config).unwrap();
///
/// // Retrieve it
/// let retrieved = store.get_autonomous_config().unwrap();
/// assert!(retrieved.is_some());
/// ```
pub struct RocksDbAutonomousStore {
    /// The RocksDB database instance.
    db: DB,
    /// Shared block cache across column families.
    #[allow(dead_code)]
    cache: Cache,
    /// Database path.
    path: PathBuf,
}

impl RocksDbAutonomousStore {
    /// Open an autonomous store at the specified path with default configuration.
    ///
    /// Creates the database and all 7 column families if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    ///
    /// # Returns
    ///
    /// * `Ok(RocksDbAutonomousStore)` - Successfully opened store
    /// * `Err(AutonomousStoreError)` - Open failed with detailed error
    ///
    /// # Errors
    ///
    /// - `AutonomousStoreError::OpenFailed` - Path invalid, permissions denied, or DB locked
    pub fn open<P: AsRef<Path>>(path: P) -> AutonomousStoreResult<Self> {
        Self::open_with_config(path, AutonomousStoreConfig::default())
    }

    /// Open an autonomous store with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    /// * `config` - Custom configuration options
    ///
    /// # Returns
    ///
    /// * `Ok(RocksDbAutonomousStore)` - Successfully opened store
    /// * `Err(AutonomousStoreError)` - Open failed
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: AutonomousStoreConfig,
    ) -> AutonomousStoreResult<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let path_str = path_buf.to_string_lossy().to_string();

        info!(
            "Opening RocksDbAutonomousStore at '{}' with cache_size={}MB",
            path_str,
            config.block_cache_size / (1024 * 1024)
        );

        // Create shared block cache
        let cache = Cache::new_lru_cache(config.block_cache_size);

        // Create DB options
        let mut db_opts = Options::default();
        db_opts.create_if_missing(config.create_if_missing);
        db_opts.create_missing_column_families(true);
        db_opts.set_max_open_files(config.max_open_files);

        if !config.enable_wal {
            db_opts.set_manual_wal_flush(true);
        }

        // Get all 7 autonomous column family descriptors
        let cf_descriptors = get_autonomous_cf_descriptors(&cache);

        debug!(
            "Opening database with {} column families",
            cf_descriptors.len()
        );

        // Open database with all column families
        let db = DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors).map_err(|e| {
            error!("Failed to open RocksDB at '{}': {}", path_str, e);
            AutonomousStoreError::OpenFailed {
                path: path_str.clone(),
                message: e.to_string(),
            }
        })?;

        info!(
            "Successfully opened RocksDbAutonomousStore with {} column families",
            AUTONOMOUS_CFS.len()
        );

        Ok(Self {
            db,
            cache,
            path: path_buf,
        })
    }

    /// Get a column family handle by name.
    ///
    /// # Errors
    ///
    /// Returns `AutonomousStoreError::ColumnFamilyNotFound` if CF doesn't exist.
    fn get_cf(&self, name: &str) -> AutonomousStoreResult<&ColumnFamily> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| AutonomousStoreError::ColumnFamilyNotFound {
                name: name.to_string(),
            })
    }

    /// Serialize a value with version prefix.
    fn serialize_with_version<T: Serialize>(value: &T) -> AutonomousStoreResult<Vec<u8>> {
        let mut result = vec![AUTONOMOUS_STORAGE_VERSION];
        let encoded =
            bincode::serialize(value).map_err(|e| AutonomousStoreError::Serialization {
                type_name: std::any::type_name::<T>(),
                message: e.to_string(),
            })?;
        result.extend(encoded);
        Ok(result)
    }

    /// Deserialize a value with version check.
    fn deserialize_with_version<T: for<'de> Deserialize<'de>>(
        data: &[u8],
        cf: &'static str,
        key: &str,
    ) -> AutonomousStoreResult<T> {
        if data.is_empty() {
            return Err(AutonomousStoreError::Deserialization {
                cf,
                key: key.to_string(),
                message: "Empty data".to_string(),
            });
        }

        let version = data[0];
        if version != AUTONOMOUS_STORAGE_VERSION {
            return Err(AutonomousStoreError::VersionMismatch {
                cf,
                expected: AUTONOMOUS_STORAGE_VERSION,
                actual: version,
            });
        }

        bincode::deserialize(&data[1..]).map_err(|e| AutonomousStoreError::Deserialization {
            cf,
            key: key.to_string(),
            message: e.to_string(),
        })
    }

    /// Get the database path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Health check: verify all column families are accessible.
    pub fn health_check(&self) -> AutonomousStoreResult<()> {
        for cf_name in AUTONOMOUS_CFS {
            self.get_cf(cf_name)?;
        }
        Ok(())
    }

    // ========================================================================
    // AutonomousConfig (Singleton)
    // ========================================================================

    /// Store the autonomous configuration.
    ///
    /// Overwrites any existing configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The AutonomousConfig to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_autonomous_config(&self, config: &AutonomousConfig) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_AUTONOMOUS_CONFIG)?;
        let data = Self::serialize_with_version(config)?;

        self.db
            .put_cf(cf, AUTONOMOUS_CONFIG_KEY, &data)
            .map_err(|e| {
                AutonomousStoreError::rocksdb_op("put", CF_AUTONOMOUS_CONFIG, Some("config"), e)
            })?;

        debug!("Stored AutonomousConfig ({} bytes)", data.len());
        Ok(())
    }

    /// Retrieve the autonomous configuration.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(config))` - Configuration found
    /// * `Ok(None)` - No configuration stored
    /// * `Err(...)` - Read or deserialization error
    pub fn get_autonomous_config(&self) -> AutonomousStoreResult<Option<AutonomousConfig>> {
        let cf = self.get_cf(CF_AUTONOMOUS_CONFIG)?;

        match self.db.get_cf(cf, AUTONOMOUS_CONFIG_KEY) {
            Ok(Some(data)) => {
                let config = Self::deserialize_with_version(&data, CF_AUTONOMOUS_CONFIG, "config")?;
                Ok(Some(config))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AutonomousStoreError::rocksdb_op(
                "get",
                CF_AUTONOMOUS_CONFIG,
                Some("config"),
                e,
            )),
        }
    }

    // ========================================================================
    // AdaptiveThresholdState (Singleton)
    // ========================================================================

    /// Store the adaptive threshold state.
    ///
    /// Overwrites any existing state.
    ///
    /// # Arguments
    ///
    /// * `state` - The AdaptiveThresholdState to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_threshold_state(
        &self,
        state: &AdaptiveThresholdState,
    ) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_ADAPTIVE_THRESHOLD_STATE)?;
        let data = Self::serialize_with_version(state)?;

        self.db
            .put_cf(cf, ADAPTIVE_THRESHOLD_STATE_KEY, &data)
            .map_err(|e| {
                AutonomousStoreError::rocksdb_op(
                    "put",
                    CF_ADAPTIVE_THRESHOLD_STATE,
                    Some("state"),
                    e,
                )
            })?;

        debug!("Stored AdaptiveThresholdState ({} bytes)", data.len());
        Ok(())
    }

    /// Retrieve the adaptive threshold state.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(state))` - State found
    /// * `Ok(None)` - No state stored
    /// * `Err(...)` - Read or deserialization error
    pub fn get_threshold_state(&self) -> AutonomousStoreResult<Option<AdaptiveThresholdState>> {
        let cf = self.get_cf(CF_ADAPTIVE_THRESHOLD_STATE)?;

        match self.db.get_cf(cf, ADAPTIVE_THRESHOLD_STATE_KEY) {
            Ok(Some(data)) => {
                let state =
                    Self::deserialize_with_version(&data, CF_ADAPTIVE_THRESHOLD_STATE, "state")?;
                Ok(Some(state))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AutonomousStoreError::rocksdb_op(
                "get",
                CF_ADAPTIVE_THRESHOLD_STATE,
                Some("state"),
                e,
            )),
        }
    }

    // ========================================================================
    // DriftDataPoint (Time-series)
    // ========================================================================

    /// Store a drift data point.
    ///
    /// # Arguments
    ///
    /// * `point` - The DriftDataPoint to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_drift_point(&self, point: &DriftDataPoint) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_DRIFT_HISTORY)?;
        let id = Uuid::new_v4();
        let key = drift_history_key(point.timestamp.timestamp_millis(), &id);
        let data = Self::serialize_with_version(point)?;

        self.db.put_cf(cf, key, &data).map_err(|e| {
            AutonomousStoreError::rocksdb_op("put", CF_DRIFT_HISTORY, Some(&id.to_string()), e)
        })?;

        debug!("Stored DriftDataPoint ({} bytes)", data.len());
        Ok(())
    }

    /// Retrieve drift history since a given timestamp.
    ///
    /// # Arguments
    ///
    /// * `since` - Optional timestamp in milliseconds. If None, returns all history.
    ///
    /// # Returns
    ///
    /// Vector of DriftDataPoints sorted by timestamp (oldest first).
    pub fn get_drift_history(
        &self,
        since: Option<i64>,
    ) -> AutonomousStoreResult<Vec<DriftDataPoint>> {
        let cf = self.get_cf(CF_DRIFT_HISTORY)?;
        let mut results = Vec::new();

        let start_key = match since {
            Some(ts) => drift_history_timestamp_prefix(ts).to_vec(),
            None => Vec::new(),
        };

        let iter = if start_key.is_empty() {
            self.db.iterator_cf(cf, rocksdb::IteratorMode::Start)
        } else {
            self.db.iterator_cf(
                cf,
                rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward),
            )
        };

        for item in iter {
            let (key, value) = item.map_err(|e| {
                AutonomousStoreError::rocksdb_op("iterate", CF_DRIFT_HISTORY, None, e)
            })?;

            let (timestamp_ms, _id) = parse_drift_history_key(&key);

            // If we have a since filter, skip entries before it
            if let Some(since_ts) = since {
                if timestamp_ms < since_ts {
                    continue;
                }
            }

            let point: DriftDataPoint = Self::deserialize_with_version(
                &value,
                CF_DRIFT_HISTORY,
                &format!("ts:{}", timestamp_ms),
            )?;
            results.push(point);
        }

        debug!("Retrieved {} drift data points", results.len());
        Ok(results)
    }

    // ========================================================================
    // GoalActivityMetrics (Per-goal)
    // ========================================================================

    /// Store goal activity metrics.
    ///
    /// # Arguments
    ///
    /// * `goal_id` - The goal ID
    /// * `metrics` - The GoalActivityMetrics to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_goal_metrics(
        &self,
        goal_id: Uuid,
        metrics: &GoalActivityMetrics,
    ) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_GOAL_ACTIVITY_METRICS)?;
        let key = goal_activity_metrics_key(&goal_id);
        let data = Self::serialize_with_version(metrics)?;

        self.db.put_cf(cf, key, &data).map_err(|e| {
            AutonomousStoreError::rocksdb_op(
                "put",
                CF_GOAL_ACTIVITY_METRICS,
                Some(&goal_id.to_string()),
                e,
            )
        })?;

        debug!(
            "Stored GoalActivityMetrics for {} ({} bytes)",
            goal_id,
            data.len()
        );
        Ok(())
    }

    /// Retrieve goal activity metrics.
    ///
    /// # Arguments
    ///
    /// * `goal_id` - The goal ID
    ///
    /// # Returns
    ///
    /// * `Ok(Some(metrics))` - Metrics found
    /// * `Ok(None)` - No metrics for this goal
    /// * `Err(...)` - Read or deserialization error
    pub fn get_goal_metrics(
        &self,
        goal_id: Uuid,
    ) -> AutonomousStoreResult<Option<GoalActivityMetrics>> {
        let cf = self.get_cf(CF_GOAL_ACTIVITY_METRICS)?;
        let key = goal_activity_metrics_key(&goal_id);

        match self.db.get_cf(cf, key) {
            Ok(Some(data)) => {
                let metrics = Self::deserialize_with_version(
                    &data,
                    CF_GOAL_ACTIVITY_METRICS,
                    &goal_id.to_string(),
                )?;
                Ok(Some(metrics))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AutonomousStoreError::rocksdb_op(
                "get",
                CF_GOAL_ACTIVITY_METRICS,
                Some(&goal_id.to_string()),
                e,
            )),
        }
    }

    /// List all goal activity metrics.
    ///
    /// # Returns
    ///
    /// Vector of (goal_id, metrics) tuples.
    pub fn list_all_goal_metrics(&self) -> AutonomousStoreResult<Vec<(Uuid, GoalActivityMetrics)>> {
        let cf = self.get_cf(CF_GOAL_ACTIVITY_METRICS)?;
        let mut results = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (key, value) = item.map_err(|e| {
                AutonomousStoreError::rocksdb_op("iterate", CF_GOAL_ACTIVITY_METRICS, None, e)
            })?;

            let goal_id = parse_goal_activity_metrics_key(&key);
            let metrics: GoalActivityMetrics = Self::deserialize_with_version(
                &value,
                CF_GOAL_ACTIVITY_METRICS,
                &goal_id.to_string(),
            )?;
            results.push((goal_id, metrics));
        }

        debug!("Listed {} goal activity metrics", results.len());
        Ok(results)
    }

    // ========================================================================
    // LineageEvent (Time-series)
    // ========================================================================

    /// Store a lineage event.
    ///
    /// # Arguments
    ///
    /// * `event` - The LineageEvent to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_lineage_event(&self, event: &LineageEvent) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_AUTONOMOUS_LINEAGE)?;
        let key = autonomous_lineage_key(event.timestamp_ms(), &event.id);
        let data = Self::serialize_with_version(event)?;

        self.db.put_cf(cf, key, &data).map_err(|e| {
            AutonomousStoreError::rocksdb_op(
                "put",
                CF_AUTONOMOUS_LINEAGE,
                Some(&event.id.to_string()),
                e,
            )
        })?;

        debug!(
            "Stored LineageEvent {} ({}) - {} bytes",
            event.id,
            event.event_type,
            data.len()
        );
        Ok(())
    }

    /// Retrieve lineage history since a given timestamp.
    ///
    /// # Arguments
    ///
    /// * `since` - Optional timestamp in milliseconds. If None, returns all history.
    ///
    /// # Returns
    ///
    /// Vector of LineageEvents sorted by timestamp (oldest first).
    pub fn get_lineage_history(
        &self,
        since: Option<u64>,
    ) -> AutonomousStoreResult<Vec<LineageEvent>> {
        let cf = self.get_cf(CF_AUTONOMOUS_LINEAGE)?;
        let mut results = Vec::new();

        let start_key = match since {
            Some(ts) => autonomous_lineage_timestamp_prefix(ts as i64).to_vec(),
            None => Vec::new(),
        };

        let iter = if start_key.is_empty() {
            self.db.iterator_cf(cf, rocksdb::IteratorMode::Start)
        } else {
            self.db.iterator_cf(
                cf,
                rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward),
            )
        };

        for item in iter {
            let (key, value) = item.map_err(|e| {
                AutonomousStoreError::rocksdb_op("iterate", CF_AUTONOMOUS_LINEAGE, None, e)
            })?;

            let (timestamp_ms, event_id) = parse_autonomous_lineage_key(&key);

            // If we have a since filter, skip entries before it
            if let Some(since_ts) = since {
                if (timestamp_ms as u64) < since_ts {
                    continue;
                }
            }

            let event: LineageEvent = Self::deserialize_with_version(
                &value,
                CF_AUTONOMOUS_LINEAGE,
                &event_id.to_string(),
            )?;
            results.push(event);
        }

        debug!("Retrieved {} lineage events", results.len());
        Ok(results)
    }

    // ========================================================================
    // ConsolidationRecord (Time-series)
    // ========================================================================

    /// Store a consolidation record.
    ///
    /// # Arguments
    ///
    /// * `record` - The ConsolidationRecord to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_consolidation_record(
        &self,
        record: &ConsolidationRecord,
    ) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_CONSOLIDATION_HISTORY)?;
        let key = consolidation_history_key(record.timestamp_ms(), &record.id);
        let data = Self::serialize_with_version(record)?;

        self.db.put_cf(cf, key, &data).map_err(|e| {
            AutonomousStoreError::rocksdb_op(
                "put",
                CF_CONSOLIDATION_HISTORY,
                Some(&record.id.to_string()),
                e,
            )
        })?;

        debug!(
            "Stored ConsolidationRecord {} (success={}) - {} bytes",
            record.id,
            record.success,
            data.len()
        );
        Ok(())
    }

    /// Retrieve consolidation history since a given timestamp.
    ///
    /// # Arguments
    ///
    /// * `since` - Optional timestamp in milliseconds. If None, returns all history.
    ///
    /// # Returns
    ///
    /// Vector of ConsolidationRecords sorted by timestamp (oldest first).
    pub fn get_consolidation_history(
        &self,
        since: Option<u64>,
    ) -> AutonomousStoreResult<Vec<ConsolidationRecord>> {
        let cf = self.get_cf(CF_CONSOLIDATION_HISTORY)?;
        let mut results = Vec::new();

        let start_key = match since {
            Some(ts) => consolidation_history_timestamp_prefix(ts as i64).to_vec(),
            None => Vec::new(),
        };

        let iter = if start_key.is_empty() {
            self.db.iterator_cf(cf, rocksdb::IteratorMode::Start)
        } else {
            self.db.iterator_cf(
                cf,
                rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward),
            )
        };

        for item in iter {
            let (key, value) = item.map_err(|e| {
                AutonomousStoreError::rocksdb_op("iterate", CF_CONSOLIDATION_HISTORY, None, e)
            })?;

            let (timestamp_ms, record_id) = parse_consolidation_history_key(&key);

            // If we have a since filter, skip entries before it
            if let Some(since_ts) = since {
                if (timestamp_ms as u64) < since_ts {
                    continue;
                }
            }

            let record: ConsolidationRecord = Self::deserialize_with_version(
                &value,
                CF_CONSOLIDATION_HISTORY,
                &record_id.to_string(),
            )?;
            results.push(record);
        }

        debug!("Retrieved {} consolidation records", results.len());
        Ok(results)
    }

    // ========================================================================
    // MemoryCurationState (Per-memory)
    // ========================================================================

    /// Store memory curation state.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The memory ID
    /// * `state` - The MemoryCurationState to store
    ///
    /// # Errors
    ///
    /// Returns error if serialization or RocksDB write fails.
    pub fn store_curation_state(
        &self,
        memory_id: Uuid,
        state: &MemoryCurationState,
    ) -> AutonomousStoreResult<()> {
        let cf = self.get_cf(CF_MEMORY_CURATION)?;
        let key = memory_curation_key(&memory_id);
        let data = Self::serialize_with_version(state)?;

        self.db.put_cf(cf, key, &data).map_err(|e| {
            AutonomousStoreError::rocksdb_op(
                "put",
                CF_MEMORY_CURATION,
                Some(&memory_id.to_string()),
                e,
            )
        })?;

        debug!(
            "Stored MemoryCurationState for {} ({} bytes)",
            memory_id,
            data.len()
        );
        Ok(())
    }

    /// Retrieve memory curation state.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The memory ID
    ///
    /// # Returns
    ///
    /// * `Ok(Some(state))` - State found
    /// * `Ok(None)` - No state for this memory
    /// * `Err(...)` - Read or deserialization error
    pub fn get_curation_state(
        &self,
        memory_id: Uuid,
    ) -> AutonomousStoreResult<Option<MemoryCurationState>> {
        let cf = self.get_cf(CF_MEMORY_CURATION)?;
        let key = memory_curation_key(&memory_id);

        match self.db.get_cf(cf, key) {
            Ok(Some(data)) => {
                let state = Self::deserialize_with_version(
                    &data,
                    CF_MEMORY_CURATION,
                    &memory_id.to_string(),
                )?;
                Ok(Some(state))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AutonomousStoreError::rocksdb_op(
                "get",
                CF_MEMORY_CURATION,
                Some(&memory_id.to_string()),
                e,
            )),
        }
    }

    /// List all memory curation states.
    ///
    /// # Returns
    ///
    /// Vector of (memory_id, state) tuples.
    pub fn list_all_curation_states(
        &self,
    ) -> AutonomousStoreResult<Vec<(Uuid, MemoryCurationState)>> {
        let cf = self.get_cf(CF_MEMORY_CURATION)?;
        let mut results = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (key, value) = item.map_err(|e| {
                AutonomousStoreError::rocksdb_op("iterate", CF_MEMORY_CURATION, None, e)
            })?;

            let memory_id = parse_memory_curation_key(&key);
            let state: MemoryCurationState =
                Self::deserialize_with_version(&value, CF_MEMORY_CURATION, &memory_id.to_string())?;
            results.push((memory_id, state));
        }

        debug!("Listed {} memory curation states", results.len());
        Ok(results)
    }

    // ========================================================================
    // Maintenance Operations
    // ========================================================================

    /// Flush all column families to disk.
    pub fn flush(&self) -> AutonomousStoreResult<()> {
        debug!("Flushing all autonomous column families");

        for cf_name in AUTONOMOUS_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db
                .flush_cf(cf)
                .map_err(|e| AutonomousStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
                })?;
        }

        info!("Flushed all autonomous column families");
        Ok(())
    }

    /// Compact all column families.
    pub fn compact(&self) -> AutonomousStoreResult<()> {
        debug!("Compacting all autonomous column families");

        for cf_name in AUTONOMOUS_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        info!("Compacted all autonomous column families");
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_test_store() -> (TempDir, RocksDbAutonomousStore) {
        let tmp = TempDir::new().unwrap();
        let store = RocksDbAutonomousStore::open(tmp.path()).unwrap();
        (tmp, store)
    }

    #[test]
    fn test_open_and_health_check() {
        let (_tmp, store) = create_test_store();
        assert!(store.health_check().is_ok());
    }

    #[test]
    fn test_autonomous_config_crud() {
        let (_tmp, store) = create_test_store();

        // Initially empty
        let config = store.get_autonomous_config().unwrap();
        assert!(config.is_none());

        // Store config
        let config = AutonomousConfig::default();
        store.store_autonomous_config(&config).unwrap();

        // Retrieve config
        let retrieved = store.get_autonomous_config().unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.enabled, config.enabled);
    }

    #[test]
    fn test_threshold_state_crud() {
        let (_tmp, store) = create_test_store();

        // Initially empty
        let state = store.get_threshold_state().unwrap();
        assert!(state.is_none());

        // Store state
        let state = AdaptiveThresholdState::default();
        store.store_threshold_state(&state).unwrap();

        // Retrieve state
        let retrieved = store.get_threshold_state().unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert!((retrieved.optimal - state.optimal).abs() < f32::EPSILON);
    }

    #[test]
    fn test_drift_history() {
        let (_tmp, store) = create_test_store();

        // Store some drift points
        for i in 0..5 {
            let point = DriftDataPoint {
                alignment_mean: 0.7 + (i as f32 * 0.01),
                new_memories_count: i as u32,
                timestamp: Utc::now(),
            };
            store.store_drift_point(&point).unwrap();
        }

        // Retrieve all
        let history = store.get_drift_history(None).unwrap();
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn test_goal_metrics_crud() {
        let (_tmp, store) = create_test_store();

        let goal_id = GoalId::new();
        let metrics = GoalActivityMetrics {
            goal_id: goal_id.clone(),
            new_aligned_memories_30d: 10,
            retrievals_14d: 5,
            avg_child_alignment: 0.75,
            weight_trend: 0.02,
            last_activity: Utc::now(),
        };

        // Store metrics
        store.store_goal_metrics(goal_id.0, &metrics).unwrap();

        // Retrieve metrics
        let retrieved = store.get_goal_metrics(goal_id.0).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.new_aligned_memories_30d, 10);

        // List all
        let all_metrics = store.list_all_goal_metrics().unwrap();
        assert_eq!(all_metrics.len(), 1);
    }

    #[test]
    fn test_lineage_events() {
        let (_tmp, store) = create_test_store();

        // Store some events
        for i in 0..3 {
            let event = LineageEvent::new(format!("test_event_{}", i), format!("Test event {}", i));
            store.store_lineage_event(&event).unwrap();
        }

        // Retrieve all
        let history = store.get_lineage_history(None).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_consolidation_records() {
        let (_tmp, store) = create_test_store();

        // Store a success record
        let record = ConsolidationRecord::success(
            vec![MemoryId::new(), MemoryId::new()],
            MemoryId::new(),
            0.95,
            0.03,
        );
        store.store_consolidation_record(&record).unwrap();

        // Store a failure record
        let record = ConsolidationRecord::failure(
            vec![MemoryId::new()],
            MemoryId::new(),
            0.85,
            0.08,
            "Test error",
        );
        store.store_consolidation_record(&record).unwrap();

        // Retrieve all
        let history = store.get_consolidation_history(None).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_curation_state_crud() {
        let (_tmp, store) = create_test_store();

        let memory_id = MemoryId::new();
        let state = MemoryCurationState::Active;

        // Store state
        store.store_curation_state(memory_id.0, &state).unwrap();

        // Retrieve state
        let retrieved = store.get_curation_state(memory_id.0).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), MemoryCurationState::Active);

        // Update to different state
        let dormant_state = MemoryCurationState::Dormant { since: Utc::now() };
        store
            .store_curation_state(memory_id.0, &dormant_state)
            .unwrap();

        let retrieved = store.get_curation_state(memory_id.0).unwrap();
        assert!(retrieved.is_some());
        match retrieved.unwrap() {
            MemoryCurationState::Dormant { .. } => {}
            _ => panic!("Expected Dormant state"),
        }

        // List all
        let all_states = store.list_all_curation_states().unwrap();
        assert_eq!(all_states.len(), 1);
    }

    #[test]
    fn test_persistence_across_reopen() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Store data
        {
            let store = RocksDbAutonomousStore::open(&path).unwrap();
            let config = AutonomousConfig::default();
            store.store_autonomous_config(&config).unwrap();
            store.flush().unwrap();
        }

        // Reopen and verify
        {
            let store = RocksDbAutonomousStore::open(&path).unwrap();
            let config = store.get_autonomous_config().unwrap();
            assert!(config.is_some());
        }
    }

    #[test]
    fn test_flush_and_compact() {
        let (_tmp, store) = create_test_store();

        // Store some data
        store
            .store_autonomous_config(&AutonomousConfig::default())
            .unwrap();

        // Flush and compact should succeed
        assert!(store.flush().is_ok());
        assert!(store.compact().is_ok());
    }

    #[test]
    fn test_lineage_event_builder() {
        let goal_id = GoalId::new();
        let memory_id = MemoryId::new();

        let event = LineageEvent::new("bootstrap", "Initial bootstrap")
            .with_goal(goal_id.clone())
            .with_memory(memory_id.clone())
            .with_metadata(r#"{"source": "test"}"#);

        assert_eq!(event.event_type, "bootstrap");
        assert!(event.goal_id.is_some());
        assert!(event.memory_id.is_some());
        assert!(event.metadata.is_some());
    }

    #[test]
    fn test_consolidation_record_constructors() {
        let sources = vec![MemoryId::new(), MemoryId::new()];
        let target = MemoryId::new();

        let success = ConsolidationRecord::success(sources.clone(), target.clone(), 0.95, 0.02);
        assert!(success.success);
        assert!(success.error_message.is_none());

        let failure =
            ConsolidationRecord::failure(sources, target, 0.90, 0.05, "Test error message");
        assert!(!failure.success);
        assert!(failure.error_message.is_some());
    }
}
