//! Extended column families for teleological fingerprint storage.
//!
//! These 4 CFs extend the base 12 CFs defined in ../column_families.rs.
//! Total after integration: 16 column families.
//!
//! # FAIL FAST Policy
//!
//! All option builders are infallible at construction time. Errors only
//! occur at DB open time, and those are surfaced by RocksDB itself.

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options};

/// Column family for ~63KB TeleologicalFingerprints.
///
/// Each fingerprint contains:
/// - SemanticFingerprint (13 embeddings, 15,120 dense dims = ~60KB)
/// - PurposeVector (13D, 52 bytes)
/// - JohariFingerprint (13×4 quadrants, ~520 bytes)
/// - PurposeEvolution (up to 100 snapshots, ~30KB max)
/// - Metadata (timestamps, hash, etc.)
pub const CF_FINGERPRINTS: &str = "fingerprints";

/// Column family for 13D purpose vectors (52 bytes each).
///
/// Stored separately from full fingerprints for fast purpose-only queries.
/// Key: UUID (16 bytes) → Value: 13 × f32 = 52 bytes
pub const CF_PURPOSE_VECTORS: &str = "purpose_vectors";

/// Column family for E13 SPLADE inverted index.
///
/// Enables fast term-based retrieval for the 5-stage pipeline.
/// Key: term_id (u16, 2 bytes) → Value: Vec<Uuid> (memory IDs with that term)
///
/// SPLADE vocabulary size: 30,522 terms (per semantic.rs E13_SPLADE_VOCAB)
pub const CF_E13_SPLADE_INVERTED: &str = "e13_splade_inverted";

/// Column family for E1 Matryoshka 128D truncated vectors.
///
/// Enables fast approximate search using truncated E1 embeddings.
/// Key: UUID (16 bytes) → Value: 128 × f32 = 512 bytes
///
/// E1 Matryoshka embeddings (1024D) can be truncated to 128D while
/// preserving reasonable accuracy for coarse filtering.
pub const CF_E1_MATRYOSHKA_128: &str = "e1_matryoshka_128";

/// All teleological column family names (4 total).
pub const TELEOLOGICAL_CFS: &[&str] = &[
    CF_FINGERPRINTS,
    CF_PURPOSE_VECTORS,
    CF_E13_SPLADE_INVERTED,
    CF_E1_MATRYOSHKA_128,
];

/// Options for ~63KB fingerprint storage.
///
/// Configuration:
/// - 64KB block size (fits one fingerprint per block)
/// - LZ4 compression (good for large values)
/// - Bloom filter for point lookups
/// - Cache index and filter blocks
pub fn fingerprint_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(64 * 1024); // 64KB for ~63KB fingerprints
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for 52-byte purpose vectors.
///
/// Configuration:
/// - Default block size (4KB)
/// - No compression (too small to benefit)
/// - Bloom filter for fast lookups
/// - Optimized for point lookups
pub fn purpose_vector_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::None); // 52 bytes, compression overhead not worth it
    opts.optimize_for_point_lookup(64); // 64MB hint
    opts.create_if_missing(true);
    opts
}

/// Options for E13 SPLADE inverted index.
///
/// Configuration:
/// - LZ4 compression (posting lists can be large)
/// - Bloom filter on term_id
/// - Suitable for both point and range queries
pub fn e13_splade_inverted_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    opts
}

/// Options for E1 Matryoshka 128D index (512 bytes per vector).
///
/// Configuration:
/// - 4KB block size (fits ~8 vectors per block)
/// - LZ4 compression
/// - Bloom filter for fast lookups
pub fn e1_matryoshka_128_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024); // 4KB blocks
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    opts
}

/// Get all 4 teleological column family descriptors.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 4 `ColumnFamilyDescriptor`s for teleological storage.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_teleological_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_teleological_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), 4);
/// ```
pub fn get_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_FINGERPRINTS, fingerprint_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_PURPOSE_VECTORS, purpose_vector_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_E13_SPLADE_INVERTED, e13_splade_inverted_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_E1_MATRYOSHKA_128, e1_matryoshka_128_cf_options(cache)),
    ]
}
