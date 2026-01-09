//! RocksDB column family definitions.
//!
//! Column families provide logical separation of data types and
//! enable efficient range queries within each category.
//!
//! # Column Families (12 total per constitution.yaml)
//! | Name | Purpose | Key Format | Optimization |
//! |------|---------|------------|--------------|
//! | nodes | Primary node storage | UUID (16 bytes) | Bloom filter, cache |
//! | edges | Graph edge storage | UUID (16 bytes) | Prefix extractor |
//! | embeddings | Embedding vectors (1536D×f32=6144 bytes) | UUID (16 bytes) | Large blocks |
//! | metadata | Node metadata | UUID (16 bytes) | Cache |
//! | johari_open | Open quadrant index | UUID | Prefix extractor |
//! | johari_hidden | Hidden quadrant index | UUID | Prefix extractor |
//! | johari_blind | Blind quadrant index | UUID | Prefix extractor |
//! | johari_unknown | Unknown quadrant index | UUID | Prefix extractor |
//! | temporal | Time-based index | timestamp_ms:UUID | Prefix extractor |
//! | tags | Tag index | tag:UUID | Prefix extractor |
//! | sources | Source index | source_uri:UUID | Prefix extractor |
//! | system | System metadata | string key | No compression |
//!
//! # Shared Block Cache
//! All column families share a single 256MB block cache for efficient memory usage.
//! Per constitution.yaml: "shared 256MB block cache with bloom filter"

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, SliceTransform};

/// Column family name constants.
///
/// These names MUST match `JohariQuadrant::column_family()` from context-graph-core.
pub mod cf_names {
    /// Primary node storage column family.
    pub const NODES: &str = "nodes";

    /// Graph edge storage column family.
    pub const EDGES: &str = "edges";

    /// Embedding vectors column family (1536D × f32 = 6144 bytes per embedding).
    pub const EMBEDDINGS: &str = "embeddings";

    /// Node metadata column family.
    pub const METADATA: &str = "metadata";

    /// Open quadrant index - matches JohariQuadrant::Open.column_family().
    pub const JOHARI_OPEN: &str = "johari_open";

    /// Hidden quadrant index - matches JohariQuadrant::Hidden.column_family().
    pub const JOHARI_HIDDEN: &str = "johari_hidden";

    /// Blind quadrant index - matches JohariQuadrant::Blind.column_family().
    pub const JOHARI_BLIND: &str = "johari_blind";

    /// Unknown quadrant index - matches JohariQuadrant::Unknown.column_family().
    pub const JOHARI_UNKNOWN: &str = "johari_unknown";

    /// Temporal index column family for time-based queries.
    pub const TEMPORAL: &str = "temporal";

    /// Tag index column family.
    pub const TAGS: &str = "tags";

    /// Source index column family.
    pub const SOURCES: &str = "sources";

    /// System metadata column family (rare access, no compression).
    pub const SYSTEM: &str = "system";

    /// All column family names as a slice (12 total).
    ///
    /// This is the canonical list per constitution.yaml specification.
    pub const ALL: &[&str] = &[
        NODES,
        EDGES,
        EMBEDDINGS,
        METADATA,
        JOHARI_OPEN,
        JOHARI_HIDDEN,
        JOHARI_BLIND,
        JOHARI_UNKNOWN,
        TEMPORAL,
        TAGS,
        SOURCES,
        SYSTEM,
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
/// Per constitution.yaml: hopfield < 1ms, inject_context P95 < 25ms
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
/// Used for: johari_*, temporal, tags, sources, metadata
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

/// Get all column family descriptors with optimized options.
///
/// # Arguments
/// * `block_cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 12 `ColumnFamilyDescriptor`s with optimized options per CF type:
/// - `nodes`: Point lookup optimized with bloom filter
/// - `edges`: Prefix-based range scans
/// - `embeddings`: Large block size for sequential reads
/// - `metadata`, `johari_*`, `temporal`, `tags`, `sources`: Index optimized
/// - `system`: No compression for small metadata
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::column_families::get_column_family_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_column_family_descriptors(&cache);
/// assert_eq!(descriptors.len(), 12);
/// ```
pub fn get_column_family_descriptors(block_cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(cf_names::NODES, nodes_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::EDGES, edges_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::EMBEDDINGS, embeddings_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::METADATA, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::JOHARI_OPEN, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::JOHARI_HIDDEN, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::JOHARI_BLIND, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::JOHARI_UNKNOWN, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::TEMPORAL, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::TAGS, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::SOURCES, index_options(block_cache)),
        ColumnFamilyDescriptor::new(cf_names::SYSTEM, system_options()),
    ]
}

/// Get ALL column family descriptors: base (12) + teleological (20) + autonomous (7).
///
/// Returns 39 total column families for a fully configured Context Graph database.
///
/// # Arguments
/// * `block_cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 39 `ColumnFamilyDescriptor`s:
/// - 12 base CFs (nodes, edges, embeddings, etc.)
/// - 7 teleological CFs (fingerprints, purpose_vectors, synergy_matrix, etc.)
/// - 13 quantized embedder CFs (emb_0 through emb_12)
/// - 7 autonomous CFs (autonomous_config, drift_history, etc.)
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::column_families::get_all_column_family_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_all_column_family_descriptors(&cache);
/// assert_eq!(descriptors.len(), 39);
/// ```
pub fn get_all_column_family_descriptors(block_cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    use crate::autonomous::get_autonomous_cf_descriptors;
    use crate::teleological::get_all_teleological_cf_descriptors;

    let mut descriptors = get_column_family_descriptors(block_cache);
    descriptors.extend(get_all_teleological_cf_descriptors(block_cache));
    descriptors.extend(get_autonomous_cf_descriptors(block_cache));
    descriptors
}

/// Total number of column families in a fully configured Context Graph database.
/// Base (12) + Teleological (7) + Quantized Embedder (13) + Autonomous (7) = 39
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
            12,
            "Must have exactly 12 column families"
        );
    }

    #[test]
    fn test_all_contains_all_names() {
        assert!(cf_names::ALL.contains(&cf_names::NODES));
        assert!(cf_names::ALL.contains(&cf_names::EDGES));
        assert!(cf_names::ALL.contains(&cf_names::EMBEDDINGS));
        assert!(cf_names::ALL.contains(&cf_names::METADATA));
        assert!(cf_names::ALL.contains(&cf_names::JOHARI_OPEN));
        assert!(cf_names::ALL.contains(&cf_names::JOHARI_HIDDEN));
        assert!(cf_names::ALL.contains(&cf_names::JOHARI_BLIND));
        assert!(cf_names::ALL.contains(&cf_names::JOHARI_UNKNOWN));
        assert!(cf_names::ALL.contains(&cf_names::TEMPORAL));
        assert!(cf_names::ALL.contains(&cf_names::TAGS));
        assert!(cf_names::ALL.contains(&cf_names::SOURCES));
        assert!(cf_names::ALL.contains(&cf_names::SYSTEM));
    }

    #[test]
    fn test_cf_names_match_johari() {
        use context_graph_core::types::JohariQuadrant;
        assert_eq!(cf_names::JOHARI_OPEN, JohariQuadrant::Open.column_family());
        assert_eq!(
            cf_names::JOHARI_HIDDEN,
            JohariQuadrant::Hidden.column_family()
        );
        assert_eq!(
            cf_names::JOHARI_BLIND,
            JohariQuadrant::Blind.column_family()
        );
        assert_eq!(
            cf_names::JOHARI_UNKNOWN,
            JohariQuadrant::Unknown.column_family()
        );
    }

    #[test]
    fn test_cf_names_unique() {
        use std::collections::HashSet;
        let set: HashSet<_> = cf_names::ALL.iter().collect();
        assert_eq!(set.len(), 12, "All CF names must be unique");
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
    fn test_get_descriptors_returns_12() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_column_family_descriptors(&cache);
        assert_eq!(descriptors.len(), 12, "Must return exactly 12 descriptors");
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
        assert_eq!(descriptors.len(), 12);
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
        assert_eq!(descriptors.len(), 12);
        println!("RESULT: PASS - Zero cache handled gracefully");
    }

    // =========================================================================
    // Additional Verification Tests
    // =========================================================================

    #[test]
    fn test_all_johari_quadrants_have_cf() {
        use context_graph_core::types::JohariQuadrant;

        // Verify all 4 Johari quadrants map to our CF names
        let quadrants = JohariQuadrant::all();
        assert_eq!(quadrants.len(), 4, "Should have 4 Johari quadrants");

        for quadrant in quadrants {
            let cf_name = quadrant.column_family();
            assert!(
                cf_names::ALL.contains(&cf_name),
                "Johari quadrant {:?} maps to '{}' which is not in cf_names::ALL",
                quadrant,
                cf_name
            );
            println!("Johari {:?} -> CF '{}'", quadrant, cf_name);
        }
    }

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

        assert_eq!(desc1.len(), 12);
        assert_eq!(desc2.len(), 12);
        println!("RESULT: PASS - Cache can be reused across multiple descriptor creations");
    }

    #[test]
    fn test_cf_name_values_match_spec() {
        // Verify exact string values match constitution.yaml specification
        assert_eq!(cf_names::NODES, "nodes");
        assert_eq!(cf_names::EDGES, "edges");
        assert_eq!(cf_names::EMBEDDINGS, "embeddings");
        assert_eq!(cf_names::METADATA, "metadata");
        assert_eq!(cf_names::JOHARI_OPEN, "johari_open");
        assert_eq!(cf_names::JOHARI_HIDDEN, "johari_hidden");
        assert_eq!(cf_names::JOHARI_BLIND, "johari_blind");
        assert_eq!(cf_names::JOHARI_UNKNOWN, "johari_unknown");
        assert_eq!(cf_names::TEMPORAL, "temporal");
        assert_eq!(cf_names::TAGS, "tags");
        assert_eq!(cf_names::SOURCES, "sources");
        assert_eq!(cf_names::SYSTEM, "system");
    }

    #[test]
    fn test_descriptors_have_unique_names() {
        use std::collections::HashSet;
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_column_family_descriptors(&cache);

        let names: HashSet<_> = descriptors.iter().map(|d| d.name()).collect();
        assert_eq!(
            names.len(),
            12,
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
        // Verify the constant is correct: 12 base + 7 teleological + 13 quantized + 7 autonomous = 39
        assert_eq!(
            TOTAL_COLUMN_FAMILIES, 39,
            "Total column families should be 39 (12 base + 7 teleological + 13 quantized + 7 autonomous)"
        );
    }

    #[test]
    fn test_get_all_column_family_descriptors_returns_39() {
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
            names.contains(&"purpose_vectors"),
            "Missing CF: purpose_vectors"
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

    #[test]
    fn test_all_cf_descriptors_includes_autonomous_cfs() {
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        let descriptors = get_all_column_family_descriptors(&cache);
        let names: Vec<_> = descriptors.iter().map(|d| d.name()).collect();

        // Verify all 7 autonomous CFs are present
        assert!(
            names.contains(&"autonomous_config"),
            "Missing CF: autonomous_config"
        );
        assert!(
            names.contains(&"adaptive_threshold_state"),
            "Missing CF: adaptive_threshold_state"
        );
        assert!(
            names.contains(&"drift_history"),
            "Missing CF: drift_history"
        );
        assert!(
            names.contains(&"goal_activity_metrics"),
            "Missing CF: goal_activity_metrics"
        );
        assert!(
            names.contains(&"autonomous_lineage"),
            "Missing CF: autonomous_lineage"
        );
        assert!(
            names.contains(&"consolidation_history"),
            "Missing CF: consolidation_history"
        );
        assert!(
            names.contains(&"memory_curation"),
            "Missing CF: memory_curation"
        );
    }
}
