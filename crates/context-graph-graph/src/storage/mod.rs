//! RocksDB storage backend for graph data.
//!
//! This module provides persistent storage for the knowledge graph using
//! RocksDB with column families for efficient data organization.
//!
//! # Column Families
//!
//! | Column Family | Key | Value | Optimization |
//! |---------------|-----|-------|--------------|
//! | adjacency | NodeId (8B i64) | Vec<GraphEdge> (bincode) | Prefix scans |
//! | hyperbolic | NodeId (8B i64) | [f32; 64] = 256 bytes | Point lookups |
//! | entailment_cones | NodeId (8B i64) | EntailmentCone = 268 bytes | Bloom filter |
//! | faiss_ids | NodeId (8B i64) | i64 = 8 bytes | Point lookups |
//! | nodes | NodeId (8B i64) | MemoryNode (bincode) | Point lookups |
//! | metadata | key string | JSON value | Small CF |
//!
//! # GPU Integration
//!
//! Data stored here is loaded into GPU memory for processing:
//! - Hyperbolic coordinates → GPU for Poincaré ball operations
//! - FAISS IDs → GPU FAISS index for vector similarity
//! - Entailment cones → GPU for hierarchy queries
//!
//! # Constitution Reference
//!
//! - db.vector: faiss_gpu
//! - storage: RocksDB 0.22
//! - SEC-06: Soft delete 30-day recovery
//! - perf.latency.faiss_1M_k100: <2ms (storage must not bottleneck)
//!
//! # Module Structure
//!
//! - [`rocksdb`]: GraphStorage implementation (M04-T13)
//! - [`migrations`]: Schema migration system (M04-T13a)

// ========== Submodules ==========

pub mod migrations;
pub mod storage_impl;  // renamed to avoid conflict with rocksdb crate

// ========== Re-exports ==========

// GraphStorage and types (M04-T13)
pub use storage_impl::{EntailmentCone, GraphEdge, GraphStorage, NodeId, PoincarePoint};

// Migrations (M04-T13a)
pub use migrations::{MigrationInfo, Migrations, SCHEMA_VERSION};

// ========== Dependencies ==========

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, DBCompressionType, SliceTransform};

use crate::error::{GraphError, GraphResult};

// ========== Column Family Names ==========

/// Column family for adjacency lists (edge data).
/// Key: node_id (16 bytes UUID)
/// Value: Vec<GraphEdge> (variable length, bincode)
/// Optimized for: prefix scans (listing all edges from a node)
pub const CF_ADJACENCY: &str = "adjacency";

/// Column family for hyperbolic coordinates.
/// Key: node_id (16 bytes UUID)
/// Value: [f32; 64] = 256 bytes (Poincaré ball coordinates)
/// Optimized for: point lookups, GPU batch loading
pub const CF_HYPERBOLIC: &str = "hyperbolic";

/// Column family for entailment cones.
/// Key: node_id (16 bytes UUID)
/// Value: EntailmentCone = 268 bytes (256 coords + 4 aperture + 4 factor + 4 depth)
/// Optimized for: range scans with bloom filter
pub const CF_CONES: &str = "entailment_cones";

/// Column family for FAISS ID mapping.
/// Key: node_id (16 bytes UUID)
/// Value: FAISS internal ID (i64 = 8 bytes)
/// Optimized for: point lookups, bidirectional mapping
pub const CF_FAISS_IDS: &str = "faiss_ids";

/// Column family for node data.
/// Key: node_id (16 bytes UUID)
/// Value: MemoryNode (variable length, bincode)
/// Optimized for: point lookups
pub const CF_NODES: &str = "nodes";

/// Column family for metadata (schema version, stats, etc.).
/// Key: key string
/// Value: JSON value
/// Optimized for: small dataset, infrequent access
pub const CF_METADATA: &str = "metadata";

/// All column family names in order.
/// Order matters for RocksDB - must match descriptor generation order.
pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    CF_ADJACENCY,
    CF_HYPERBOLIC,
    CF_CONES,
    CF_FAISS_IDS,
    CF_NODES,
    CF_METADATA,
];

// ========== Storage Configuration ==========

/// Configuration for graph storage.
///
/// All parameters are validated before use via `validate()`.
/// Invalid configurations fail fast with `GraphError::InvalidConfig`.
///
/// # Constitution Reference
///
/// - perf.memory.gpu: <24GB (8GB headroom) - storage supports GPU batch loading
/// - perf.memory.graph_cap: >10M nodes
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Block cache size in bytes (default: 512MB).
    /// Shared across all column families for memory efficiency.
    pub block_cache_size: usize,

    /// Enable compression (default: true, uses LZ4).
    /// LZ4 provides fast decompression for GPU batch loading.
    pub enable_compression: bool,

    /// Bloom filter bits per key (default: 10).
    /// Higher values improve read performance at cost of memory.
    pub bloom_filter_bits: i32,

    /// Write buffer size in bytes (default: 64MB).
    /// Larger buffers improve write throughput.
    pub write_buffer_size: usize,

    /// Max write buffers (default: 3).
    /// More buffers allow concurrent writes during flush.
    pub max_write_buffers: i32,

    /// Target file size base in bytes (default: 64MB).
    /// Affects SST file sizes and compaction.
    pub target_file_size_base: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            block_cache_size: 512 * 1024 * 1024,      // 512MB
            enable_compression: true,
            bloom_filter_bits: 10,
            write_buffer_size: 64 * 1024 * 1024,     // 64MB
            max_write_buffers: 3,
            target_file_size_base: 64 * 1024 * 1024, // 64MB
        }
    }
}

impl StorageConfig {
    /// Create config optimized for read-heavy workloads.
    ///
    /// Best for: inference, search, GPU batch loading
    /// - Larger block cache (1GB)
    /// - Higher bloom filter bits (14)
    #[must_use]
    pub fn read_optimized() -> Self {
        Self {
            block_cache_size: 1024 * 1024 * 1024, // 1GB
            bloom_filter_bits: 14,                 // Higher for better read performance
            ..Default::default()
        }
    }

    /// Create config optimized for write-heavy workloads.
    ///
    /// Best for: bulk loading, training data ingestion
    /// - Larger write buffers (128MB)
    /// - More write buffers (5)
    #[must_use]
    pub fn write_optimized() -> Self {
        Self {
            write_buffer_size: 128 * 1024 * 1024, // 128MB
            max_write_buffers: 5,
            ..Default::default()
        }
    }

    /// Validate configuration, returning GraphError if invalid.
    ///
    /// Fails fast with clear error messages per constitution AP-001.
    ///
    /// # Errors
    ///
    /// Returns `GraphError::InvalidConfig` if:
    /// - `block_cache_size` < 1MB
    /// - `bloom_filter_bits` not in 1..=20
    /// - `write_buffer_size` < 1MB
    /// - `max_write_buffers` < 1
    /// - `target_file_size_base` < 1MB
    pub fn validate(&self) -> GraphResult<()> {
        const MIN_SIZE: usize = 1024 * 1024; // 1MB

        if self.block_cache_size < MIN_SIZE {
            return Err(GraphError::InvalidConfig(format!(
                "block_cache_size must be >= 1MB, got {} bytes",
                self.block_cache_size
            )));
        }

        if self.bloom_filter_bits < 1 || self.bloom_filter_bits > 20 {
            return Err(GraphError::InvalidConfig(format!(
                "bloom_filter_bits must be 1..=20, got {}",
                self.bloom_filter_bits
            )));
        }

        if self.write_buffer_size < MIN_SIZE {
            return Err(GraphError::InvalidConfig(format!(
                "write_buffer_size must be >= 1MB, got {} bytes",
                self.write_buffer_size
            )));
        }

        if self.max_write_buffers < 1 {
            return Err(GraphError::InvalidConfig(format!(
                "max_write_buffers must be >= 1, got {}",
                self.max_write_buffers
            )));
        }

        if self.target_file_size_base < MIN_SIZE as u64 {
            return Err(GraphError::InvalidConfig(format!(
                "target_file_size_base must be >= 1MB, got {} bytes",
                self.target_file_size_base
            )));
        }

        Ok(())
    }
}

// ========== Column Family Descriptors ==========

/// Get column family descriptors for all graph storage CFs.
///
/// Creates optimized descriptors for each column family based on access patterns:
/// - Adjacency: prefix scans for edge lists
/// - Hyperbolic: point lookups for GPU batch loading
/// - Cones: bloom filter for hierarchy queries
/// - FAISS IDs: point lookups for ID mapping
/// - Nodes: point lookups for node data
/// - Metadata: small, infrequent access
///
/// # Arguments
///
/// * `config` - Storage configuration (validated before use)
///
/// # Returns
///
/// Vector of `ColumnFamilyDescriptor` for all column families.
/// Order matches `ALL_COLUMN_FAMILIES`.
///
/// # Errors
///
/// Returns `GraphError::InvalidConfig` if configuration validation fails.
///
/// # Example
///
/// ```ignore
/// use context_graph_graph::storage::{StorageConfig, get_column_family_descriptors, get_db_options};
///
/// let config = StorageConfig::default();
/// let cf_descriptors = get_column_family_descriptors(&config)?;
/// let db_opts = get_db_options();
/// let db = rocksdb::DB::open_cf_descriptors(&db_opts, "path", cf_descriptors)?;
/// ```
pub fn get_column_family_descriptors(
    config: &StorageConfig,
) -> GraphResult<Vec<ColumnFamilyDescriptor>> {
    // Validate config first - fail fast
    config.validate()?;

    // Create shared LRU cache for memory efficiency
    let cache = Cache::new_lru_cache(config.block_cache_size);

    Ok(vec![
        adjacency_cf_descriptor(config, &cache),
        hyperbolic_cf_descriptor(config, &cache),
        cones_cf_descriptor(config, &cache),
        faiss_ids_cf_descriptor(config, &cache),
        nodes_cf_descriptor(config, &cache),
        metadata_cf_descriptor(&cache),
    ])
}

/// Get CF descriptor for adjacency column family.
/// Optimized for prefix scans (listing all edges from a node).
fn adjacency_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    // Write settings
    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);
    opts.set_target_file_size_base(config.target_file_size_base);

    // Compression: LZ4 for fast decompression (GPU batch loading)
    if config.enable_compression {
        opts.set_compression_type(DBCompressionType::Lz4);
    }

    // Block-based table with shared cache
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(16 * 1024); // 16KB blocks for prefix scans

    // Bloom filter for point lookups within prefix
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);

    opts.set_block_based_table_factory(&block_opts);

    // Optimize for prefix scans (16-byte UUID keys)
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16));

    ColumnFamilyDescriptor::new(CF_ADJACENCY, opts)
}

/// Get CF descriptor for hyperbolic coordinates.
/// Optimized for point lookups (256 bytes per point, GPU batch loading).
fn hyperbolic_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);

    // LZ4 compression (256 bytes of floats compress well)
    if config.enable_compression {
        opts.set_compression_type(DBCompressionType::Lz4);
    }

    // Block-based table optimized for point lookups
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024); // Smaller blocks for point lookups

    // Strong bloom filter for fast negative lookups
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);

    opts.set_block_based_table_factory(&block_opts);

    // Optimize for point lookups
    opts.optimize_for_point_lookup(64); // 64MB block cache hint

    ColumnFamilyDescriptor::new(CF_HYPERBOLIC, opts)
}

/// Get CF descriptor for entailment cones.
/// Optimized for range scans with bloom filter (268 bytes per cone).
fn cones_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);

    // LZ4 compression
    if config.enable_compression {
        opts.set_compression_type(DBCompressionType::Lz4);
    }

    // Block-based table with bloom filter
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(8 * 1024); // 8KB blocks

    // Bloom filter enabled for efficient cone lookups (per task spec)
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);
    block_opts.set_whole_key_filtering(true);

    opts.set_block_based_table_factory(&block_opts);

    ColumnFamilyDescriptor::new(CF_CONES, opts)
}

/// Get CF descriptor for FAISS ID mapping.
/// Optimized for point lookups (8 bytes per entry).
fn faiss_ids_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    // Smaller buffers for small values (8 bytes each)
    opts.set_write_buffer_size(16 * 1024 * 1024); // 16MB
    opts.set_max_write_buffer_number(2);

    // Block-based table
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024); // 4KB blocks

    // Bloom filter for fast lookups
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);

    opts.set_block_based_table_factory(&block_opts);

    ColumnFamilyDescriptor::new(CF_FAISS_IDS, opts)
}

/// Get CF descriptor for node data.
/// Optimized for point lookups with variable-size values.
fn nodes_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);
    opts.set_target_file_size_base(config.target_file_size_base);

    // LZ4 compression for variable-size node data
    if config.enable_compression {
        opts.set_compression_type(DBCompressionType::Lz4);
    }

    // Block-based table
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(8 * 1024); // 8KB blocks

    // Bloom filter for point lookups
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);

    opts.set_block_based_table_factory(&block_opts);

    ColumnFamilyDescriptor::new(CF_NODES, opts)
}

/// Get CF descriptor for metadata.
/// Small CF for schema version, statistics, etc.
fn metadata_cf_descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    // Minimal write buffer for small metadata
    opts.set_write_buffer_size(4 * 1024 * 1024); // 4MB
    opts.set_max_write_buffer_number(2);

    // Block-based table
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024);

    opts.set_block_based_table_factory(&block_opts);

    ColumnFamilyDescriptor::new(CF_METADATA, opts)
}

/// Get default DB options for opening the database.
///
/// Configures parallelism based on CPU count and sets reasonable defaults
/// for production use. Optimized for systems with high core counts
/// (e.g., Ryzen 9 9950X3D with 16 cores / 32 threads).
///
/// # Constitution Reference
///
/// - stack.lang.rust: 1.75+
/// - AP-004: Avoid blocking I/O in async
#[must_use]
pub fn get_db_options() -> Options {
    let mut opts = Options::default();

    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_max_open_files(1000);
    opts.set_keep_log_file_num(10);

    // Parallelism based on available CPUs
    // Ryzen 9 9950X3D: 16 cores / 32 threads
    let cpu_count = num_cpus::get() as i32;

    // Use at least 2 threads, scale with CPU count
    let parallelism = cpu_count.max(2);
    opts.increase_parallelism(parallelism);

    // Background jobs: min 2, max based on CPU count (cap at reasonable level)
    let bg_jobs = cpu_count.clamp(2, 8);
    opts.set_max_background_jobs(bg_jobs);

    opts
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Constants Tests ==========

    #[test]
    fn test_cf_names() {
        assert_eq!(CF_ADJACENCY, "adjacency");
        assert_eq!(CF_HYPERBOLIC, "hyperbolic");
        assert_eq!(CF_CONES, "entailment_cones");
        assert_eq!(CF_FAISS_IDS, "faiss_ids");
        assert_eq!(CF_NODES, "nodes");
        assert_eq!(CF_METADATA, "metadata");
    }

    #[test]
    fn test_all_column_families_count() {
        assert_eq!(ALL_COLUMN_FAMILIES.len(), 6);
    }

    #[test]
    fn test_all_column_families_contains_all() {
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_ADJACENCY));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_HYPERBOLIC));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_CONES));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_FAISS_IDS));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_NODES));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_METADATA));
    }

    #[test]
    fn test_all_column_families_order() {
        // Order must match descriptor generation
        assert_eq!(ALL_COLUMN_FAMILIES[0], CF_ADJACENCY);
        assert_eq!(ALL_COLUMN_FAMILIES[1], CF_HYPERBOLIC);
        assert_eq!(ALL_COLUMN_FAMILIES[2], CF_CONES);
        assert_eq!(ALL_COLUMN_FAMILIES[3], CF_FAISS_IDS);
        assert_eq!(ALL_COLUMN_FAMILIES[4], CF_NODES);
        assert_eq!(ALL_COLUMN_FAMILIES[5], CF_METADATA);
    }

    // ========== StorageConfig Tests ==========

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.block_cache_size, 512 * 1024 * 1024);
        assert!(config.enable_compression);
        assert_eq!(config.bloom_filter_bits, 10);
        assert_eq!(config.write_buffer_size, 64 * 1024 * 1024);
        assert_eq!(config.max_write_buffers, 3);
        assert_eq!(config.target_file_size_base, 64 * 1024 * 1024);
    }

    #[test]
    fn test_storage_config_read_optimized() {
        let config = StorageConfig::read_optimized();
        assert_eq!(config.block_cache_size, 1024 * 1024 * 1024); // 1GB
        assert_eq!(config.bloom_filter_bits, 14);
        // Inherited from default
        assert!(config.enable_compression);
        assert_eq!(config.write_buffer_size, 64 * 1024 * 1024);
    }

    #[test]
    fn test_storage_config_write_optimized() {
        let config = StorageConfig::write_optimized();
        assert_eq!(config.write_buffer_size, 128 * 1024 * 1024); // 128MB
        assert_eq!(config.max_write_buffers, 5);
        // Inherited from default
        assert!(config.enable_compression);
        assert_eq!(config.block_cache_size, 512 * 1024 * 1024);
    }

    #[test]
    fn test_storage_config_validate_success() {
        let config = StorageConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_storage_config_validate_read_optimized() {
        let config = StorageConfig::read_optimized();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_storage_config_validate_write_optimized() {
        let config = StorageConfig::write_optimized();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_storage_config_validate_block_cache_too_small() {
        let config = StorageConfig {
            block_cache_size: 1024, // Only 1KB
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, GraphError::InvalidConfig(_)));
        assert!(err.to_string().contains("block_cache_size"));
    }

    #[test]
    fn test_storage_config_validate_block_cache_boundary() {
        // Exactly 1MB should pass
        let config = StorageConfig {
            block_cache_size: 1024 * 1024,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // 1 byte less should fail
        let config = StorageConfig {
            block_cache_size: 1024 * 1024 - 1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_storage_config_validate_bloom_filter_invalid_zero() {
        let config = StorageConfig {
            bloom_filter_bits: 0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, GraphError::InvalidConfig(_)));
        assert!(err.to_string().contains("bloom_filter_bits"));
    }

    #[test]
    fn test_storage_config_validate_bloom_filter_invalid_high() {
        let config = StorageConfig {
            bloom_filter_bits: 21,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, GraphError::InvalidConfig(_)));
        assert!(err.to_string().contains("bloom_filter_bits"));
    }

    #[test]
    fn test_storage_config_validate_bloom_filter_boundaries() {
        // 1 should pass
        let config = StorageConfig {
            bloom_filter_bits: 1,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // 20 should pass
        let config = StorageConfig {
            bloom_filter_bits: 20,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_storage_config_validate_write_buffer_too_small() {
        let config = StorageConfig {
            write_buffer_size: 512, // Only 512 bytes
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, GraphError::InvalidConfig(_)));
        assert!(err.to_string().contains("write_buffer_size"));
    }

    // ========== Column Family Descriptor Tests ==========

    #[test]
    fn test_get_column_family_descriptors_count() {
        let config = StorageConfig::default();
        let descriptors = get_column_family_descriptors(&config).unwrap();
        assert_eq!(descriptors.len(), 6);
    }

    #[test]
    fn test_get_column_family_descriptors_invalid_config() {
        let config = StorageConfig {
            block_cache_size: 0,
            ..Default::default()
        };
        let result = get_column_family_descriptors(&config);
        assert!(result.is_err());
        // Can't use unwrap_err() because ColumnFamilyDescriptor doesn't impl Debug
        // Instead verify the error through match
        match result {
            Err(GraphError::InvalidConfig(msg)) => {
                assert!(msg.contains("block_cache_size"));
            }
            _ => panic!("Expected GraphError::InvalidConfig"),
        }
    }

    // ========== DB Options Tests ==========

    #[test]
    fn test_db_options_valid() {
        // Should not panic
        let _opts = get_db_options();
    }

    #[test]
    fn test_db_options_parallelism_at_least_2() {
        // Even on 1-CPU system, should use at least 2 threads
        // We can't easily test this without mocking num_cpus,
        // but we verify the options are valid
        let opts = get_db_options();

        // Verify options are usable by creating a temp DB
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_opts.db");
        let _db = rocksdb::DB::open(&opts, &db_path).unwrap();
    }
}
