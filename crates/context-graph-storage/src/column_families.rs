//! RocksDB column family definitions.
//!
//! Column families provide logical separation of data types and
//! enable efficient range queries within each category.
//!
//! # Column Families (8 total per PRD v6)
//! | Name | Purpose | Key Format | Optimization |
//! |------|---------|------------|--------------|
//! | nodes | Primary node storage | UUID (16 bytes) | Bloom filter, cache |
//! | edges | Graph edge storage | UUID (16 bytes) | Prefix extractor |
//! | embeddings | Embedding vectors (1536D×f32=6144 bytes) | UUID (16 bytes) | Large blocks |
//! | metadata | Node metadata | UUID (16 bytes) | Cache |
//! | temporal | Time-based index | timestamp_ms:UUID | Prefix extractor |
//! | tags | Tag index | tag:UUID | Prefix extractor |
//! | sources | Source index | source_uri:UUID | Prefix extractor |
//! | system | System metadata | string key | No compression |
//!
//! # Shared Block Cache
//! All column families share a single 256MB block cache for efficient memory usage.
//! Per constitution.yaml: "shared 256MB block cache with bloom filter"
//!
//! # PRD v6 Note
//! Topic detection uses embedder category weights per PRD v6.

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, SliceTransform};

/// Column family name constants.
pub mod cf_names {
    /// Primary node storage column family.
    pub const NODES: &str = "nodes";

    /// Graph edge storage column family.
    pub const EDGES: &str = "edges";

    /// Embedding vectors column family (1536D × f32 = 6144 bytes per embedding).
    pub const EMBEDDINGS: &str = "embeddings";

    /// Node metadata column family.
    pub const METADATA: &str = "metadata";

    /// Temporal index column family for time-based queries.
    pub const TEMPORAL: &str = "temporal";

    /// Tag index column family.
    pub const TAGS: &str = "tags";

    /// Source index column family.
    pub const SOURCES: &str = "sources";

    /// System metadata column family (rare access, no compression).
    pub const SYSTEM: &str = "system";

    // ========== Graph Linking Column Families (TASK-GRAPHLINK-010) ==========

    /// K-NN edges per embedder.
    /// Key: [embedder_id: u8][source_uuid: 16 bytes] = 17 bytes
    /// Value: serialized Vec<EmbedderEdge> (k neighbors)
    pub const EMBEDDER_EDGES: &str = "embedder_edges";

    /// Multi-relation typed edges.
    /// Key: [source_uuid: 16 bytes][target_uuid: 16 bytes] = 32 bytes
    /// Value: serialized TypedEdge
    pub const TYPED_EDGES: &str = "typed_edges";

    /// Typed edges indexed by edge type (secondary index).
    /// Key: [edge_type: u8][source_uuid: 16 bytes] = 17 bytes
    /// Value: target_uuid (16 bytes)
    pub const TYPED_EDGES_BY_TYPE: &str = "typed_edges_by_type";

    /// All column family names as a slice (11 total: 8 base + 3 graph linking).
    ///
    /// This is the canonical list per constitution.yaml specification.
    pub const ALL: &[&str] = &[
        NODES,
        EDGES,
        EMBEDDINGS,
        METADATA,
        TEMPORAL,
        TAGS,
        SOURCES,
        SYSTEM,
        EMBEDDER_EDGES,
        TYPED_EDGES,
        TYPED_EDGES_BY_TYPE,
    ];
}

/// Create options optimized for node storage (point lookups).
///
/// # Configuration
/// - Bloom filter: 10 bits per key (reduces disk reads for non-existent keys)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled (fast compression)
/// - Optimized for point lookups (256MB cache hint)
/// - Cache index and filter blocks
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Performance
/// Per constitution.yaml: inject_context P95 < 25ms
pub fn nodes_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false); // 10 bits per key, not block-based
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.optimize_for_point_lookup(256); // 256MB hint for point lookups
    opts.create_if_missing(true);

    opts
}

/// Create options optimized for edge storage (range scans with prefix).
///
/// # Configuration
/// - Prefix extractor: 16 bytes (UUID source_id)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Key Format
/// Edges are keyed by source UUID, enabling efficient range scans for outgoing edges.
pub fn edges_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID is 16 bytes
    opts.create_if_missing(true);

    opts
}

/// Create options optimized for embedding storage (large sequential reads).
///
/// # Configuration
/// - Block size: 64KB (larger blocks for sequential reads of 6KB embeddings)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Embedding Size
/// Each embedding is 1536 × f32 = 6144 bytes, so 64KB blocks hold ~10 embeddings.
pub fn embeddings_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(64 * 1024); // 64KB for large sequential reads
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);

    opts
}

/// Create options optimized for index column families.
///
/// Used for: temporal, tags, sources, metadata
///
/// # Configuration
/// - Prefix extractor: 16 bytes (UUID)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled
/// - Bloom filter: 10 bits
///
/// # Arguments
/// * `cache` - Shared block cache
pub fn index_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.create_if_missing(true);

    opts
}

/// Create options for system metadata (small, infrequent access).
///
/// # Configuration
/// - No compression (data is small, compression overhead not worth it)
/// - No bloom filter (rarely accessed)
/// - No prefix extractor (string keys, not UUID-based)
///
/// # Returns
/// Options configured for system metadata storage.
pub fn system_options() -> Options {
    let mut opts = Options::default();
    opts.set_compression_type(rocksdb::DBCompressionType::None);
    opts.create_if_missing(true);

    opts
}

/// Create options optimized for K-NN embedder edges (prefix scans by embedder).
///
/// # Configuration
/// - Prefix extractor: 1 byte (embedder_id)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Key Format
/// [embedder_id: u8][source_uuid: 16 bytes] = 17 bytes
pub fn embedder_edges_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(1)); // embedder_id prefix
    opts.create_if_missing(true);

    opts
}

/// Create options optimized for typed edges (source UUID prefix scans).
///
/// # Configuration
/// - Prefix extractor: 16 bytes (source UUID)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Key Format
/// [source_uuid: 16 bytes][target_uuid: 16 bytes] = 32 bytes
pub fn typed_edges_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // source UUID prefix
    opts.create_if_missing(true);

    opts
}

/// Create options optimized for typed edges by type index (edge type prefix scans).
///
/// # Configuration
/// - Prefix extractor: 1 byte (edge_type)
/// - Block cache: enabled (shared cache reference)
/// - LZ4 compression: enabled
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Key Format
/// [edge_type: u8][source_uuid: 16 bytes] = 17 bytes
pub fn typed_edges_by_type_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(1)); // edge_type prefix
    opts.create_if_missing(true);

    opts
}

/// Get all column family descriptors with optimized options.
///
/// # Arguments
/// * `block_cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 11 `ColumnFamilyDescriptor`s with optimized options per CF type:
/// - `nodes`: Point lookup optimized with bloom filter
/// - `edges`: Prefix-based range scans
/// - `embeddings`: Large block size for sequential reads
/// - `metadata`, `temporal`, `tags`, `sources`: Index optimized
/// - `system`: No compression for small metadata
/// - `embedder_edges`: K-NN edges per embedder (1-byte prefix)
/// - `typed_edges`: Multi-relation typed edges (16-byte prefix)
/// - `typed_edges_by_type`: Secondary index by edge type (1-byte prefix)
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::column_families::get_column_family_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_column_family_descriptors(&cache);
/// assert_eq!(descriptors.len(), 11);
/// ```
pub fn get_column_family_descriptors(block_cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(cf_names::NODES, nodes_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::EDGES, edges_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::EMBEDDINGS, embeddings_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::METADATA, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::TEMPORAL, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::TAGS, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::SOURCES, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::SYSTEM, system_options()),
        // Graph linking column families (TASK-GRAPHLINK-010)
        ColumnFamilyDescriptor::new(cf_names::EMBEDDER_EDGES, embedder_edges_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::TYPED_EDGES, typed_edges_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::TYPED_EDGES_BY_TYPE, typed_edges_by_type_options(block_cache)),
    ]
}

/// Get ALL column family descriptors: base (11) + teleological.
///
/// Returns total column families for a fully configured Context Graph database.
///
/// # Arguments
/// * `block_cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of `ColumnFamilyDescriptor`s:
/// - 8 base CFs (nodes, edges, embeddings, metadata, temporal, tags, sources, system)
/// - Teleological CFs (fingerprints, synergy_matrix, etc.)
/// - 13 quantized embedder CFs (emb_0 through emb_12)
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::column_families::get_all_column_family_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_all_column_family_descriptors(&cache);
/// assert_eq!(descriptors.len(), TOTAL_COLUMN_FAMILIES);
/// ```
pub fn get_all_column_family_descriptors(block_cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    use crate::teleological::get_all_teleological_cf_descriptors;

    let mut descriptors = get_column_family_descriptors(block_cache);
    descriptors.extend(get_all_teleological_cf_descriptors(block_cache));
    descriptors
}

/// Total number of column families in a fully configured Context Graph database.
/// Base (11: 8 original + 3 graph linking) + Teleological (15) + Quantized Embedder (13) = 39
/// PRD v6: Autonomous module removed - topics emerge from clustering, not goal hierarchies
/// TASK-GRAPHLINK-010: Added 3 graph linking CFs (embedder_edges, typed_edges, typed_edges_by_type)
pub const TOTAL_COLUMN_FAMILIES: usize = 39;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // CF Names Module Tests
    // =========================================================================

    #[test]
    fn test_cf_names_count() {
        assert_eq!(
            cf_names::ALL.len(),
            11,
            "Must have exactly 11 column families (8 base + 3 graph linking)"
        );
    }

    #[test]
    fn test_all_contains_all_names() {
        // Base 8 CFs
        assert!(cf_names::ALL.contains(&cf_names::NODES));
        assert!(cf_names::ALL.contains(&cf_names::EDGES));
        assert!(cf_names::ALL.contains(&cf_names::EMBEDDINGS));
        assert!(cf_names::ALL.contains(&cf_names::METADATA));
        assert!(cf_names::ALL.contains(&cf_names::TEMPORAL));
        assert!(cf_names::ALL.contains(&cf_names::TAGS));
        assert!(cf_names::ALL.contains(&cf_names::SOURCES));
        assert!(cf_names::ALL.contains(&cf_names::SYSTEM));
        // Graph linking CFs (TASK-GRAPHLINK-010)
        assert!(cf_names::ALL.contains(&cf_names::EMBEDDER_EDGES));
        assert!(cf_names::ALL.contains(&cf_names::TYPED_EDGES));
        assert!(cf_names::ALL.contains(&cf_names::TYPED_EDGES_BY_TYPE));
    }

    #[test]
    fn test_cf_names_unique() {
        use std::collections::HashSet;
        let set: HashSet<_> = cf_names::ALL.iter().collect();
        assert_eq!(set.len(), 11, "All CF names must be unique");
    }

    #[test]
    fn test_cf_names_are_snake_case() {
        for name in cf_names::ALL {
            assert!(
                name.chars().all(|c| c.is_lowercase() || c == '_'),
                "CF name '{}' should be snake_case",
                name
            );
        }
    }

    #[test]
    fn test_cf_names_non_empty() {
        for name in cf_names::ALL {
            assert!(!name.is_empty(), "CF name should not be empty");
        }
    }

    // =========================================================================
    // Option Builders Tests
    // =========================================================================

    #[test]
    fn test_nodes_options_creates_valid_options() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
        let opts = nodes_options(&cache);
        // Options object created successfully (no panic)
        drop(opts);
    }

    #[test]
    fn test_edges_options_creates_valid_options() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let opts = edges_options(&cache);
        drop(opts);
    }

    #[test]
    fn test_embeddings_options_creates_valid_options() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let opts = embeddings_options(&cache);
        drop(opts);
    }

    #[test]
    fn test_index_options_creates_valid_options() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let opts = index_options(&cache);
        drop(opts);
    }

    #[test]
    fn test_system_options_creates_valid_options() {
        let opts = system_options();
        drop(opts);
    }

    // =========================================================================
    // Descriptor Creation Tests
    // =========================================================================

    #[test]
    fn test_get_descriptors_returns_11() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_column_family_descriptors(&cache);
        assert_eq!(descriptors.len(), 11, "Must return exactly 11 descriptors (8 base + 3 graph linking)");
    }

    #[test]
    fn test_descriptors_have_correct_names() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_column_family_descriptors(&cache);
        let names: Vec<_> = descriptors.iter().map(|d| d.name()).collect();

        for cf_name in cf_names::ALL {
            assert!(names.contains(cf_name), "Missing CF: {}", cf_name);
        }
    }

    #[test]
    fn test_descriptors_in_order() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_column_family_descriptors(&cache);

        // Verify order matches cf_names::ALL
        for (i, cf_name) in cf_names::ALL.iter().enumerate() {
            assert_eq!(
                descriptors[i].name(),
                *cf_name,
                "Descriptor {} should be '{}'",
                i,
                cf_name
            );
        }
    }

    // =========================================================================
    // Edge Case Tests (REQUIRED - print before/after state)
    // =========================================================================

    #[test]
    fn edge_case_multiple_cache_references() {
        println!("=== EDGE CASE: Multiple option builders sharing same cache ===");
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);

        println!("BEFORE: Creating options with shared cache reference");
        let nodes = nodes_options(&cache);
        let edges = edges_options(&cache);
        let embeddings = embeddings_options(&cache);
        let index = index_options(&cache);

        println!("AFTER: All 4 option builders created successfully");
        println!("  - nodes_options: created");
        println!("  - edges_options: created");
        println!("  - embeddings_options: created");
        println!("  - index_options: created");
        drop(nodes);
        drop(edges);
        drop(embeddings);
        drop(index);
        println!("RESULT: PASS - Shared cache works across multiple Options");
    }

    #[test]
    fn edge_case_minimum_cache_size() {
        println!("=== EDGE CASE: Minimum cache size (1MB) ===");
        let cache = Cache::new_lru_cache(1024 * 1024); // 1MB minimum

        println!("BEFORE: Creating descriptors with 1MB cache");
        let descriptors = get_column_family_descriptors(&cache);

        println!("AFTER: {} descriptors created", descriptors.len());
        assert_eq!(descriptors.len(), 11);
        println!("RESULT: PASS - Works with minimum cache size");
    }

    #[test]
    fn edge_case_zero_cache_size() {
        println!("=== EDGE CASE: Zero cache size ===");
        // RocksDB requires minimum cache size, 0 should still create cache
        let cache = Cache::new_lru_cache(0);

        println!("BEFORE: Creating descriptors with 0-byte cache");
        let descriptors = get_column_family_descriptors(&cache);

        println!("AFTER: {} descriptors created", descriptors.len());
        assert_eq!(descriptors.len(), 11);
        println!("RESULT: PASS - Zero cache handled gracefully");
    }

    // =========================================================================
    // Additional Verification Tests
    // =========================================================================

    #[test]
    fn test_options_reusable_with_same_cache() {
        println!("=== TEST: Options can be created multiple times with same cache ===");
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);

        println!("BEFORE: Creating first batch of descriptors");
        let desc1 = get_column_family_descriptors(&cache);
        println!("  First batch: {} descriptors", desc1.len());

        println!("AFTER: Creating second batch of descriptors with same cache");
        let desc2 = get_column_family_descriptors(&cache);
        println!("  Second batch: {} descriptors", desc2.len());

        assert_eq!(desc1.len(), 11);
        assert_eq!(desc2.len(), 11);
        println!("RESULT: PASS - Cache can be reused across multiple descriptor creations");
    }

    #[test]
    fn test_cf_name_values_match_spec() {
        // Verify exact string values match specification
        // Base 8 CFs
        assert_eq!(cf_names::NODES, "nodes");
        assert_eq!(cf_names::EDGES, "edges");
        assert_eq!(cf_names::EMBEDDINGS, "embeddings");
        assert_eq!(cf_names::METADATA, "metadata");
        assert_eq!(cf_names::TEMPORAL, "temporal");
        assert_eq!(cf_names::TAGS, "tags");
        assert_eq!(cf_names::SOURCES, "sources");
        assert_eq!(cf_names::SYSTEM, "system");
        // Graph linking CFs (TASK-GRAPHLINK-010)
        assert_eq!(cf_names::EMBEDDER_EDGES, "embedder_edges");
        assert_eq!(cf_names::TYPED_EDGES, "typed_edges");
        assert_eq!(cf_names::TYPED_EDGES_BY_TYPE, "typed_edges_by_type");
    }

    #[test]
    fn test_descriptors_have_unique_names() {
        use std::collections::HashSet;
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_column_family_descriptors(&cache);

        let names: HashSet<_> = descriptors.iter().map(|d| d.name()).collect();
        assert_eq!(
            names.len(),
            11,
            "All descriptor names must be unique, got {} unique names",
            names.len()
        );
    }

    #[test]
    fn test_primary_cfs_present() {
        // Verify the 4 primary CFs are present
        assert!(cf_names::ALL.contains(&"nodes"));
        assert!(cf_names::ALL.contains(&"edges"));
        assert!(cf_names::ALL.contains(&"embeddings"));
        assert!(cf_names::ALL.contains(&"metadata"));
    }

    #[test]
    fn test_index_cfs_present() {
        // Verify index CFs are present
        assert!(cf_names::ALL.contains(&"temporal"));
        assert!(cf_names::ALL.contains(&"tags"));
        assert!(cf_names::ALL.contains(&"sources"));
        assert!(cf_names::ALL.contains(&"system"));
    }

    // =========================================================================
    // TASK-EMB-022: Full Column Family Count Tests
    // =========================================================================

    #[test]
    fn test_total_column_families_constant() {
        // Verify the constant is correct:
        // 11 base (8 original + 3 graph linking) + 15 teleological + 13 quantized = 39
        // PRD v6: Autonomous module removed - topics emerge from clustering, not goal hierarchies
        // Teleological: 13 active + 2 legacy = 15
        // TASK-GRAPHLINK-010: Added 3 graph linking CFs
        assert_eq!(
            TOTAL_COLUMN_FAMILIES, 39,
            "Total column families should be 39 (11 base + 15 teleological + 13 quantized)"
        );
    }

    #[test]
    fn test_get_all_column_family_descriptors_returns_correct_count() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_all_column_family_descriptors(&cache);

        assert_eq!(
            descriptors.len(),
            TOTAL_COLUMN_FAMILIES,
            "get_all_column_family_descriptors should return {} CFs, got {}",
            TOTAL_COLUMN_FAMILIES,
            descriptors.len()
        );
    }

    #[test]
    fn test_all_cf_descriptors_have_unique_names() {
        use std::collections::HashSet;

        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_all_column_family_descriptors(&cache);

        let names: HashSet<_> = descriptors.iter().map(|d| d.name()).collect();
        assert_eq!(
            names.len(),
            TOTAL_COLUMN_FAMILIES,
            "All {} CF names must be unique",
            TOTAL_COLUMN_FAMILIES
        );
    }

    #[test]
    fn test_all_cf_descriptors_includes_quantized_embedder_cfs() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_all_column_family_descriptors(&cache);
        let names: Vec<_> = descriptors.iter().map(|d| d.name()).collect();

        // Verify all 13 quantized embedder CFs are present
        for i in 0..13 {
            let cf_name = format!("emb_{}", i);
            assert!(
                names.contains(&cf_name.as_str()),
                "Missing quantized embedder CF: {}",
                cf_name
            );
        }
    }

    #[test]
    fn test_all_cf_descriptors_includes_teleological_cfs() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_all_column_family_descriptors(&cache);
        let names: Vec<_> = descriptors.iter().map(|d| d.name()).collect();

        // Verify all 7 teleological CFs are present (4 original + 3 TASK-TELEO-006)
        assert!(names.contains(&"fingerprints"), "Missing CF: fingerprints");
        assert!(
            names.contains(&"topic_profiles"),
            "Missing CF: topic_profiles"
        );
        assert!(
            names.contains(&"e13_splade_inverted"),
            "Missing CF: e13_splade_inverted"
        );
        assert!(
            names.contains(&"e1_matryoshka_128"),
            "Missing CF: e1_matryoshka_128"
        );
        // TASK-TELEO-006: New teleological vector CFs
        assert!(
            names.contains(&"synergy_matrix"),
            "Missing CF: synergy_matrix"
        );
        assert!(
            names.contains(&"teleological_profiles"),
            "Missing CF: teleological_profiles"
        );
        assert!(
            names.contains(&"teleological_vectors"),
            "Missing CF: teleological_vectors"
        );
    }

}
