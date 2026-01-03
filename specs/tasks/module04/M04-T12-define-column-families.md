---
id: "M04-T12"
title: "Define Graph Storage Column Families"
description: |
  Define RocksDB column families for knowledge graph storage.
  CFs: adjacency (edge lists), hyperbolic (64D coordinates), entailment_cones (cone data).
  Include get_column_family_descriptors() returning optimized CF options.
  Hyperbolic CF: 256 bytes per point (64 * 4), LZ4 compression.
  Cones CF: 268 bytes per cone, bloom filter enabled.
layer: "logic"
status: "pending"
priority: "high"
estimated_hours: 2
sequence: 16
depends_on:
  - "M04-T08a"
spec_refs:
  - "TECH-GRAPH-004 Section 4"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "Define column family constants and descriptors"
test_file: "crates/context-graph-graph/tests/storage_tests.rs"
---

## Context

RocksDB column families provide logical separation of data with independent configuration. The knowledge graph requires three specialized column families: adjacency lists for graph structure, hyperbolic coordinates for Poincare ball embeddings, and entailment cones for hierarchy queries. Each CF is optimized for its access pattern: prefix scans for adjacency, point lookups for hyperbolic, and range scans with bloom filters for cones.

## Scope

### In Scope
- Column family name constants
- CF descriptor generation with optimized options
- Compression configuration (LZ4)
- Bloom filter settings
- Block cache configuration

### Out of Scope
- GraphStorage implementation (see M04-T13)
- Schema migrations (see M04-T13a)
- Actual data serialization formats

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/storage/mod.rs

pub mod edges;
pub mod migrations;
pub mod rocksdb;

use rocksdb::{ColumnFamilyDescriptor, Options, BlockBasedOptions, Cache};

// ========== Column Family Names ==========

/// Column family for adjacency lists (edge data)
/// Key: node_id (8 bytes)
/// Value: Vec<GraphEdge> (variable length)
pub const CF_ADJACENCY: &str = "adjacency";

/// Column family for hyperbolic coordinates
/// Key: node_id (8 bytes)
/// Value: [f32; 64] = 256 bytes (Poincare ball coordinates)
pub const CF_HYPERBOLIC: &str = "hyperbolic";

/// Column family for entailment cones
/// Key: node_id (8 bytes)
/// Value: EntailmentCone = 268 bytes (256 coords + 4 aperture + 4 factor + 4 depth)
pub const CF_CONES: &str = "entailment_cones";

/// Column family for metadata (schema version, stats, etc.)
pub const CF_METADATA: &str = "metadata";

/// All column family names
pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    CF_ADJACENCY,
    CF_HYPERBOLIC,
    CF_CONES,
    CF_METADATA,
];

// ========== Column Family Configuration ==========

/// Configuration for graph storage
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Block cache size in bytes (default: 512MB)
    pub block_cache_size: usize,
    /// Enable compression (default: true)
    pub enable_compression: bool,
    /// Bloom filter bits per key (default: 10)
    pub bloom_filter_bits: i32,
    /// Write buffer size in bytes (default: 64MB)
    pub write_buffer_size: usize,
    /// Max write buffers (default: 3)
    pub max_write_buffers: i32,
    /// Target file size base (default: 64MB)
    pub target_file_size_base: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            block_cache_size: 512 * 1024 * 1024,  // 512MB
            enable_compression: true,
            bloom_filter_bits: 10,
            write_buffer_size: 64 * 1024 * 1024,   // 64MB
            max_write_buffers: 3,
            target_file_size_base: 64 * 1024 * 1024, // 64MB
        }
    }
}

impl StorageConfig {
    /// Create config optimized for read-heavy workloads
    pub fn read_optimized() -> Self {
        Self {
            block_cache_size: 1024 * 1024 * 1024,  // 1GB
            bloom_filter_bits: 14,  // Higher for better read performance
            ..Default::default()
        }
    }

    /// Create config optimized for write-heavy workloads
    pub fn write_optimized() -> Self {
        Self {
            write_buffer_size: 128 * 1024 * 1024,  // 128MB
            max_write_buffers: 5,
            ..Default::default()
        }
    }
}

// ========== Column Family Descriptors ==========

/// Get column family descriptors for all graph storage CFs
pub fn get_column_family_descriptors(config: &StorageConfig) -> Vec<ColumnFamilyDescriptor> {
    let cache = Cache::new_lru_cache(config.block_cache_size);

    vec![
        adjacency_cf_descriptor(config, &cache),
        hyperbolic_cf_descriptor(config, &cache),
        cones_cf_descriptor(config, &cache),
        metadata_cf_descriptor(config, &cache),
    ]
}

/// Get CF descriptor for adjacency column family
/// Optimized for prefix scans (listing all edges from a node)
fn adjacency_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    // Write settings
    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);
    opts.set_target_file_size_base(config.target_file_size_base);

    // Compression: LZ4 for fast decompression
    if config.enable_compression {
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    }

    // Block-based table with cache
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(16 * 1024);  // 16KB blocks for prefix scans

    // Bloom filter for point lookups
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);

    opts.set_block_based_table_factory(&block_opts);

    // Optimize for prefix scans
    opts.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(8));

    ColumnFamilyDescriptor::new(CF_ADJACENCY, opts)
}

/// Get CF descriptor for hyperbolic coordinates
/// Optimized for point lookups (256 bytes per point)
fn hyperbolic_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);

    // LZ4 compression (256 bytes of floats compress well)
    if config.enable_compression {
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    }

    // Block-based table optimized for point lookups
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024);  // Smaller blocks for point lookups

    // Strong bloom filter for fast negative lookups
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);

    opts.set_block_based_table_factory(&block_opts);

    // Optimize for point lookups
    opts.optimize_for_point_lookup(64);  // 64MB block cache hint

    ColumnFamilyDescriptor::new(CF_HYPERBOLIC, opts)
}

/// Get CF descriptor for entailment cones
/// Optimized for range scans with bloom filter (268 bytes per cone)
fn cones_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    opts.set_write_buffer_size(config.write_buffer_size);
    opts.set_max_write_buffer_number(config.max_write_buffers);

    // LZ4 compression
    if config.enable_compression {
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    }

    // Block-based table with bloom filter
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(8 * 1024);  // 8KB blocks

    // Bloom filter enabled for efficient cone lookups
    block_opts.set_bloom_filter(config.bloom_filter_bits as f64, false);
    block_opts.set_whole_key_filtering(true);

    opts.set_block_based_table_factory(&block_opts);

    ColumnFamilyDescriptor::new(CF_CONES, opts)
}

/// Get CF descriptor for metadata
/// Small CF for schema version, statistics, etc.
fn metadata_cf_descriptor(config: &StorageConfig, cache: &Cache) -> ColumnFamilyDescriptor {
    let mut opts = Options::default();

    // Minimal write buffer for small metadata
    opts.set_write_buffer_size(4 * 1024 * 1024);  // 4MB
    opts.set_max_write_buffer_number(2);

    // Block-based table
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024);

    opts.set_block_based_table_factory(&block_opts);

    ColumnFamilyDescriptor::new(CF_METADATA, opts)
}

/// Get default DB options for opening the database
pub fn get_db_options() -> Options {
    let mut opts = Options::default();

    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_max_open_files(1000);
    opts.set_keep_log_file_num(10);

    // Parallelism
    opts.increase_parallelism(num_cpus::get() as i32);
    opts.set_max_background_jobs(4);

    opts
}
```

### Constraints
- CF_ADJACENCY optimized for prefix scans
- CF_HYPERBOLIC optimized for point lookups (256 bytes/entry)
- CF_CONES uses bloom filter for efficient lookups (268 bytes/entry)
- LZ4 compression for all CFs
- Shared block cache across CFs

### Acceptance Criteria
- [ ] CF_ADJACENCY, CF_HYPERBOLIC, CF_CONES constants defined
- [ ] get_column_family_descriptors() returns 4 CFs with options
- [ ] Hyperbolic CF optimized for point lookups
- [ ] Adjacency CF optimized for prefix scans
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define CF name constants
2. Create StorageConfig with tunable parameters
3. For each CF, create optimized Options:
   - Set compression (LZ4)
   - Configure block cache
   - Set bloom filter
   - Set access pattern hints
4. Return Vec<ColumnFamilyDescriptor>

### Edge Cases
- Zero block cache size: Use reasonable minimum
- Invalid bloom filter bits: Clamp to valid range
- System with few CPUs: Limit parallelism

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph storage
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] All CFs can be created
- [ ] Compression is LZ4
- [ ] Block cache is shared
- [ ] Bloom filter is enabled for cones CF

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cf_names() {
        assert_eq!(CF_ADJACENCY, "adjacency");
        assert_eq!(CF_HYPERBOLIC, "hyperbolic");
        assert_eq!(CF_CONES, "entailment_cones");
        assert_eq!(CF_METADATA, "metadata");
    }

    #[test]
    fn test_all_column_families() {
        assert_eq!(ALL_COLUMN_FAMILIES.len(), 4);
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_ADJACENCY));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_HYPERBOLIC));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_CONES));
        assert!(ALL_COLUMN_FAMILIES.contains(&CF_METADATA));
    }

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.block_cache_size, 512 * 1024 * 1024);
        assert!(config.enable_compression);
        assert_eq!(config.bloom_filter_bits, 10);
    }

    #[test]
    fn test_get_column_family_descriptors() {
        let config = StorageConfig::default();
        let descriptors = get_column_family_descriptors(&config);

        assert_eq!(descriptors.len(), 4);
    }
}
```
