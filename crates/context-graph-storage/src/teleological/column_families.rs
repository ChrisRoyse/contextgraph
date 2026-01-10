//! Extended column families for teleological fingerprint storage.
//!
//! These 4 CFs extend the base 12 CFs defined in ../column_families.rs.
//! Total after integration: 16 column families.
//!
//! # FAIL FAST Policy
//!
//! All option builders are infallible at construction time. Errors only
//! occur at DB open time, and those are surfaced by RocksDB itself.

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, SliceTransform};

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

// =============================================================================
// TELEOLOGICAL VECTOR COLUMN FAMILIES (TASK-TELEO-006)
// =============================================================================

/// Column family for synergy matrix singleton storage.
///
/// Stores the global 13x13 synergy matrix that captures co-occurrence
/// patterns between teleological dimensions. Singleton with key "synergy".
/// Key: "synergy" (7 bytes fixed)
/// Value: SynergyMatrix serialized via bincode (~700 bytes: 13x13 f32 + metadata)
pub const CF_SYNERGY_MATRIX: &str = "synergy_matrix";

/// Column family for teleological profiles.
///
/// Stores task-specific teleological profiles that define purpose weighting.
/// Key: profile_id string bytes (variable length, typically 1-64 bytes)
/// Value: TeleologicalProfile serialized via bincode (~200-500 bytes)
pub const CF_TELEOLOGICAL_PROFILES: &str = "teleological_profiles";

/// Column family for teleological vectors per-memory storage.
///
/// Stores the 13D TeleologicalVector for each memory, capturing purpose alignment.
/// Key: memory_id UUID (16 bytes)
/// Value: TeleologicalVector serialized via bincode (52 bytes: 13 x f32)
pub const CF_TELEOLOGICAL_VECTORS: &str = "teleological_vectors";

/// Column family for original content text.
///
/// Stores the original text content associated with each fingerprint.
/// Content is stored separately from embeddings for efficient retrieval.
/// Key: UUID (16 bytes) -> Value: UTF-8 text (LZ4 compressed by RocksDB)
///
/// # Storage Details
/// - Maximum content size: 1MB (enforced at application layer)
/// - Compression: LZ4 (~50% reduction for typical text)
/// - No bloom filter (point lookups only by UUID)
pub const CF_CONTENT: &str = "content";

/// All teleological column family names (8 total: 4 original + 3 teleological + 1 content).
pub const TELEOLOGICAL_CFS: &[&str] = &[
    CF_FINGERPRINTS,
    CF_PURPOSE_VECTORS,
    CF_E13_SPLADE_INVERTED,
    CF_E1_MATRYOSHKA_128,
    // TASK-TELEO-006: New teleological vector CFs
    CF_SYNERGY_MATRIX,
    CF_TELEOLOGICAL_PROFILES,
    CF_TELEOLOGICAL_VECTORS,
    // TASK-CONTENT-001: Content storage CF
    CF_CONTENT,
];

/// Total count of teleological CFs (should be 8).
pub const TELEOLOGICAL_CF_COUNT: usize = 8;

// =============================================================================
// QUANTIZED EMBEDDER COLUMN FAMILIES (13 CFs for per-embedder storage)
// =============================================================================
// TASK-EMB-022: Per-embedder quantized fingerprint storage
// Each CF stores QuantizedEmbedding data serialized via bincode.
// Key: UUID (16 bytes) → Value: QuantizedEmbedding (variable size, ~1-2KB per embedder)
// =============================================================================

/// Column family for E1_Semantic (ModelId=0) quantized embeddings.
/// Quantization: PQ-8 (8 bytes compressed from 1024D).
pub const CF_EMB_0: &str = "emb_0";

/// Column family for E2_TemporalRecent (ModelId=1) quantized embeddings.
/// Quantization: Float8E4M3 (512 bytes from 512D).
pub const CF_EMB_1: &str = "emb_1";

/// Column family for E3_TemporalPeriodic (ModelId=2) quantized embeddings.
/// Quantization: Float8E4M3 (512 bytes from 512D).
pub const CF_EMB_2: &str = "emb_2";

/// Column family for E4_TemporalPositional (ModelId=3) quantized embeddings.
/// Quantization: Float8E4M3 (512 bytes from 512D).
pub const CF_EMB_3: &str = "emb_3";

/// Column family for E5_Causal (ModelId=4) quantized embeddings.
/// Quantization: PQ-8 (8 bytes compressed from 768D).
pub const CF_EMB_4: &str = "emb_4";

/// Column family for E6_Sparse (ModelId=5) quantized embeddings.
/// Quantization: SparseNative (variable size, indices+values).
pub const CF_EMB_5: &str = "emb_5";

/// Column family for E7_Code (ModelId=6) quantized embeddings.
/// Quantization: PQ-8 (8 bytes compressed from 256D).
pub const CF_EMB_6: &str = "emb_6";

/// Column family for E8_Graph (ModelId=7) quantized embeddings.
/// Quantization: Float8E4M3 (384 bytes from 384D).
pub const CF_EMB_7: &str = "emb_7";

/// Column family for E9_HDC (ModelId=8) quantized embeddings.
/// Quantization: Binary (1250 bytes from 10000D binary vector).
pub const CF_EMB_8: &str = "emb_8";

/// Column family for E10_Multimodal (ModelId=9) quantized embeddings.
/// Quantization: PQ-8 (8 bytes compressed from 768D).
pub const CF_EMB_9: &str = "emb_9";

/// Column family for E11_Entity (ModelId=10) quantized embeddings.
/// Quantization: Float8E4M3 (384 bytes from 384D).
pub const CF_EMB_10: &str = "emb_10";

/// Column family for E12_LateInteraction (ModelId=11) quantized embeddings.
/// Quantization: TokenPruning (variable size, ~50% of original).
pub const CF_EMB_11: &str = "emb_11";

/// Column family for E13_SPLADE (ModelId=12) quantized embeddings.
/// Quantization: SparseNative (variable size, indices+values).
pub const CF_EMB_12: &str = "emb_12";

/// All 13 quantized embedder column family names.
/// Maps to ModelId indices 0-12 for per-embedder HNSW storage.
pub const QUANTIZED_EMBEDDER_CFS: &[&str] = &[
    CF_EMB_0,  // E1_Semantic
    CF_EMB_1,  // E2_TemporalRecent
    CF_EMB_2,  // E3_TemporalPeriodic
    CF_EMB_3,  // E4_TemporalPositional
    CF_EMB_4,  // E5_Causal
    CF_EMB_5,  // E6_Sparse
    CF_EMB_6,  // E7_Code
    CF_EMB_7,  // E8_Graph
    CF_EMB_8,  // E9_HDC
    CF_EMB_9,  // E10_Multimodal
    CF_EMB_10, // E11_Entity
    CF_EMB_11, // E12_LateInteraction
    CF_EMB_12, // E13_SPLADE
];

/// Total count of quantized embedder CFs (should be 13).
pub const QUANTIZED_EMBEDDER_CF_COUNT: usize = 13;

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

// =============================================================================
// TASK-TELEO-006: CF OPTION BUILDERS FOR TELEOLOGICAL VECTOR STORAGE
// =============================================================================

/// Options for synergy matrix singleton storage (~700 bytes).
///
/// # Configuration
/// - No compression (small singleton, compression overhead not worth it)
/// - Bloom filter for fast lookups
/// - Optimized for point lookups (singleton access pattern)
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn synergy_matrix_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::None); // Small singleton
    opts.optimize_for_point_lookup(16); // 16MB hint for point lookups
    opts.create_if_missing(true);
    opts
}

/// Options for teleological profiles storage (~200-500 bytes per profile).
///
/// # Configuration
/// - LZ4 compression (profiles may have repetitive string patterns)
/// - Bloom filter for fast lookups
/// - Cache index and filter blocks
///
/// # Key Format
/// Variable-length profile_id strings (typically 1-64 bytes).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn teleological_profiles_cf_options(cache: &Cache) -> Options {
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

/// Options for teleological vectors per-memory storage (52 bytes per vector).
///
/// # Configuration
/// - No compression (52 bytes is too small to benefit from compression)
/// - Bloom filter for fast lookups
/// - 16-byte prefix extractor for UUID keys
/// - Optimized for point lookups
///
/// # Key Format
/// UUID (16 bytes) for memory_id.
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn teleological_vectors_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::None); // 52 bytes, compression not worth it
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(64); // 64MB hint
    opts.create_if_missing(true);
    opts
}

/// Options for content text storage (variable size, up to 1MB).
///
/// # Configuration
/// - LZ4 compression (~50% reduction for typical text content)
/// - No bloom filter (point lookups only by UUID)
/// - 16-byte prefix extractor for UUID keys
/// - Level compaction for sequential write patterns
///
/// # Key Format
/// UUID (16 bytes) for fingerprint_id.
///
/// # Value Format
/// Raw UTF-8 text content (up to 1MB, enforced at application layer).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn content_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    // No bloom filter - we only do point lookups by UUID
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    // LZ4 compression for ~50% text reduction
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for quantized embedder storage (~1-2KB per embedding).
///
/// Configuration:
/// - 4KB block size (fits multiple embeddings per block)
/// - LZ4 compression (quantized data benefits from compression)
/// - Bloom filter for fast point lookups
/// - Optimized for point lookups (most queries by UUID)
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn quantized_embedder_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024); // 4KB blocks for ~1-2KB embeddings
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.optimize_for_point_lookup(64); // 64MB hint for point lookups
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Get all 8 teleological column family descriptors.
///
/// Returns 8 descriptors: 4 original + 3 teleological + 1 content (TASK-CONTENT-001).
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 8 `ColumnFamilyDescriptor`s for teleological storage.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_teleological_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_teleological_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), 8);
/// ```
pub fn get_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_FINGERPRINTS, fingerprint_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_PURPOSE_VECTORS, purpose_vector_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_E13_SPLADE_INVERTED,
            e13_splade_inverted_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_E1_MATRYOSHKA_128, e1_matryoshka_128_cf_options(cache)),
        // TASK-TELEO-006: New teleological vector CFs
        ColumnFamilyDescriptor::new(CF_SYNERGY_MATRIX, synergy_matrix_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_TELEOLOGICAL_PROFILES,
            teleological_profiles_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_TELEOLOGICAL_VECTORS,
            teleological_vectors_cf_options(cache),
        ),
        // TASK-CONTENT-001: Content storage CF
        ColumnFamilyDescriptor::new(CF_CONTENT, content_cf_options(cache)),
    ]
}

/// Get all 13 quantized embedder column family descriptors.
///
/// These CFs store per-embedder QuantizedEmbedding data for the 13-embedder
/// multi-array TeleologicalFingerprint system. Each CF maps ModelId index to storage.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 13 `ColumnFamilyDescriptor`s for quantized embedder storage.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_quantized_embedder_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_quantized_embedder_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), 13);
/// ```
pub fn get_quantized_embedder_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    QUANTIZED_EMBEDDER_CFS
        .iter()
        .map(|&cf_name| ColumnFamilyDescriptor::new(cf_name, quantized_embedder_cf_options(cache)))
        .collect()
}

/// Get ALL teleological + quantized embedder column family descriptors.
///
/// Returns 21 descriptors total: 8 teleological + 13 quantized embedder.
/// Use this when opening a database that needs both fingerprint and per-embedder storage.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 21 `ColumnFamilyDescriptor`s.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_all_teleological_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_all_teleological_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), 21); // 8 teleological + 13 embedder
/// ```
pub fn get_all_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    let mut descriptors = get_teleological_cf_descriptors(cache);
    descriptors.extend(get_quantized_embedder_cf_descriptors(cache));
    descriptors
}
