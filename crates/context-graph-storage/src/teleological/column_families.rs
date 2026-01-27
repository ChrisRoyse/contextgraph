//! Extended column families for teleological fingerprint storage.
//!
//! These CFs extend the base 8 CFs defined in ../column_families.rs.
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
/// - TopicProfile (13D, 52 bytes)
/// - Metadata (timestamps, hash, etc.)
pub const CF_FINGERPRINTS: &str = "fingerprints";

/// Column family for 13D topic profile vectors (52 bytes each).
///
/// Stored separately from full fingerprints for fast profile-only queries.
/// Key: UUID (16 bytes) → Value: 13 × f32 = 52 bytes
pub const CF_TOPIC_PROFILES: &str = "topic_profiles";

/// Column family for E13 SPLADE inverted index.
///
/// Enables fast term-based retrieval for the 5-stage pipeline.
/// Key: term_id (u16, 2 bytes) → Value: Vec<Uuid> (memory IDs with that term)
///
/// SPLADE vocabulary size: 30,522 terms (per semantic.rs E13_SPLADE_VOCAB)
pub const CF_E13_SPLADE_INVERTED: &str = "e13_splade_inverted";

/// Column family for E6 Sparse (V_selectivity) inverted index.
///
/// Enables fast exact keyword matching for Stage 1 dual recall (per e6upgrade.md).
/// Key: term_id (u16, 2 bytes) → Value: Vec<Uuid> (memory IDs with that term)
///
/// E6 vocabulary size: 30,522 terms (BERT tokenizer, same as E13)
/// Typical active terms per doc: ~235 (0.77% sparsity)
///
/// # Usage
/// - Stage 1: Co-pilot with E13 for dual sparse recall
/// - Stage 3.5: Tie-breaker when E1 scores are close
pub const CF_E6_SPARSE_INVERTED: &str = "e6_sparse_inverted";

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

/// Column family for source metadata.
///
/// Stores provenance information for each fingerprint, enabling context
/// injection to display where memories originated from (e.g., file paths
/// for MDFileChunk memories).
///
/// Key: UUID (16 bytes) -> Value: SourceMetadata serialized via bincode (~100-500 bytes)
///
/// # Storage Details
/// - LZ4 compression (JSON-like data compresses well)
/// - Bloom filter for fast lookups
/// - Point lookups only by UUID
pub const CF_SOURCE_METADATA: &str = "source_metadata";

/// Column family for file path to fingerprint ID index.
///
/// Secondary index enabling O(1) lookup of fingerprints by file path.
/// Used by file watcher management tools for efficient cleanup and reconciliation.
///
/// Key: file_path bytes (UTF-8, variable length)
/// Value: FileIndexEntry serialized via bincode (Vec<Uuid> + metadata)
///
/// # Storage Details
/// - LZ4 compression (file paths and UUID lists compress well)
/// - Bloom filter for fast path existence checks
/// - Prefix iteration for path-based queries
pub const CF_FILE_INDEX: &str = "file_index";

/// Column family for persisted topic portfolio storage.
///
/// Stores serialized PersistedTopicPortfolio for session continuity.
/// Topics are persisted at SessionEnd and loaded at SessionStart per PRD Section 9.1.
///
/// Key: session_id bytes (UTF-8, variable length) or "__latest__" for most recent
/// Value: PersistedTopicPortfolio serialized via JSON (~1KB-50KB depending on topic count)
///
/// # Storage Details
/// - LZ4 compression (JSON compresses well)
/// - Bloom filter for fast session lookups
/// - Point lookups by session_id or "__latest__" sentinel
pub const CF_TOPIC_PORTFOLIO: &str = "topic_portfolio";

// =============================================================================
// TASK-STORAGE-P2-001: E12 LATE INTERACTION COLUMN FAMILY
// =============================================================================

/// Column family for E12 ColBERT late interaction token embeddings.
///
/// Stores the token-level embeddings for MaxSim scoring in Stage 5 of the
/// 5-stage retrieval pipeline. Each memory has variable number of tokens,
/// each token is a 128D vector.
///
/// # Storage Details
/// - Key: UUID (16 bytes) for memory_id
/// - Value: Vec<Vec<f32>> serialized via bincode (variable size)
///   - Typical: 20-50 tokens × 128D × 4 bytes = 10-25KB per memory
///   - Maximum: 512 tokens × 128D × 4 bytes = 256KB per memory
/// - Compression: LZ4 (good compression for repeated float patterns)
/// - Access pattern: Point lookup by UUID during Stage 5 rerank
///
/// # Performance Target
/// - Retrieve 50 token sets in <5ms
/// - MaxSim scoring of 50 candidates in <15ms
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub const CF_E12_LATE_INTERACTION: &str = "e12_late_interaction";

// =============================================================================
// LEGACY COLUMN FAMILIES (Backwards Compatibility)
// =============================================================================
// These column families were created in earlier versions of the codebase.
// They must be opened for backwards compatibility with existing databases,
// even though they are no longer actively used.

/// Legacy column family for session identity storage.
///
/// **DEPRECATED**: This column family exists for backwards compatibility only.
/// It is no longer used but must be opened for databases created with older versions.
pub const CF_SESSION_IDENTITY: &str = "session_identity";

/// Legacy column family for ego node storage.
///
/// **DEPRECATED**: This column family exists for backwards compatibility only.
/// It is no longer used but must be opened for databases created with older versions.
pub const CF_EGO_NODE: &str = "ego_node";

/// All teleological column family names (15 total: 5 original + 3 teleological + 1 content + 1 source_metadata + 1 file_index + 1 topic_portfolio + 1 e12_late_interaction + 2 legacy).
pub const TELEOLOGICAL_CFS: &[&str] = &[
    CF_FINGERPRINTS,
    CF_TOPIC_PROFILES,
    CF_E13_SPLADE_INVERTED,
    CF_E6_SPARSE_INVERTED, // E6 inverted index for dual Stage 1 recall (e6upgrade.md)
    CF_E1_MATRYOSHKA_128,
    // TASK-TELEO-006: New teleological vector CFs
    CF_SYNERGY_MATRIX,
    CF_TELEOLOGICAL_PROFILES,
    CF_TELEOLOGICAL_VECTORS,
    // TASK-CONTENT-001: Content storage CF
    CF_CONTENT,
    // Source metadata storage CF
    CF_SOURCE_METADATA,
    // File index for file watcher management
    CF_FILE_INDEX,
    // Topic portfolio persistence for session continuity
    CF_TOPIC_PORTFOLIO,
    // TASK-STORAGE-P2-001: E12 Late Interaction token storage CF
    CF_E12_LATE_INTERACTION,
    // Legacy CFs for backwards compatibility
    CF_SESSION_IDENTITY,
    CF_EGO_NODE,
];

/// Total count of teleological CFs (should be 15: 13 active + 2 legacy).
pub const TELEOLOGICAL_CF_COUNT: usize = 15;

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
/// Quantization: Float8E4M3 (1024 bytes from 1024D).
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

/// Options for 52-byte topic profiles.
///
/// Configuration:
/// - Default block size (4KB)
/// - No compression (too small to benefit)
/// - Bloom filter for fast lookups
/// - Optimized for point lookups
pub fn topic_profile_cf_options(cache: &Cache) -> Options {
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

/// Options for E6 Sparse (V_selectivity) inverted index.
///
/// Configuration (same as E13 SPLADE inverted):
/// - LZ4 compression (posting lists can be large)
/// - Bloom filter on term_id for fast lookups
/// - Suitable for both point and range queries
///
/// # Usage
/// - Stage 1: Dual sparse recall with E13 (union of candidates)
/// - Stage 3.5: E6 tie-breaker for close E1 scores
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn e6_sparse_inverted_cf_options(cache: &Cache) -> Options {
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

/// Options for source metadata storage (~100-500 bytes per entry).
///
/// # Configuration
/// - LZ4 compression (bincode data compresses well)
/// - Bloom filter for fast lookups
/// - Point lookups only by UUID
///
/// # Key Format
/// UUID (16 bytes) for fingerprint_id.
///
/// # Value Format
/// SourceMetadata serialized via bincode (~100-500 bytes).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn source_metadata_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.optimize_for_point_lookup(32); // 32MB hint for point lookups
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for file index storage (file_path -> Vec<Uuid> mapping).
///
/// # Configuration
/// - LZ4 compression (file paths and UUID lists compress well)
/// - Bloom filter for fast path existence checks
/// - Prefix scan support for listing all files
///
/// # Key Format
/// UTF-8 file path bytes (variable length).
///
/// # Value Format
/// FileIndexEntry serialized via bincode (~100-2000 bytes depending on chunk count).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn file_index_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for topic portfolio storage (~1KB-50KB per portfolio).
///
/// # Configuration
/// - LZ4 compression (JSON data compresses well, ~50% reduction)
/// - Bloom filter for fast session lookups
/// - Point lookups by session_id or "__latest__" sentinel
///
/// # Key Format
/// UTF-8 session_id bytes (variable length) or "__latest__" (9 bytes).
///
/// # Value Format
/// PersistedTopicPortfolio serialized via JSON (~1KB-50KB depending on topic count).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn topic_portfolio_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    // LZ4 compression - JSON compresses well
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.optimize_for_point_lookup(32); // 32MB hint for point lookups
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

// =============================================================================
// TASK-STORAGE-P2-001: CF OPTION BUILDER FOR E12 LATE INTERACTION
// =============================================================================

/// Options for E12 ColBERT late interaction token storage (~10-25KB typical).
///
/// # Configuration
/// - 32KB block size (fits 1-2 token sets per block)
/// - LZ4 compression (good for repeated float patterns, ~40% reduction)
/// - Bloom filter for fast point lookups
/// - 16-byte prefix extractor for UUID keys
/// - Optimized for point lookups (Stage 5 retrieval pattern)
///
/// # Value Size
/// - Minimum: 1 token × 128D × 4 bytes = 512 bytes
/// - Typical: 30 tokens × 128D × 4 bytes = ~15KB
/// - Maximum: 512 tokens × 128D × 4 bytes = 256KB
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn e12_late_interaction_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(32 * 1024); // 32KB blocks for ~15KB typical token sets
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    // LZ4 compression - good for float arrays with repeated patterns
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    // UUID prefix extractor for efficient key lookup
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16));
    opts.optimize_for_point_lookup(64); // 64MB hint for point lookups
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for legacy column families (backwards compatibility only).
///
/// These column families are no longer actively used but must be openable
/// for databases created with older versions of the software.
///
/// # Configuration
/// - Default options with minimal overhead
/// - No compression (these CFs are not written to)
/// - Bloom filter disabled (no reads expected)
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn legacy_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::None);
    opts.create_if_missing(true);
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

/// Get all 15 teleological column family descriptors.
///
/// Returns 15 descriptors: 5 original + 3 teleological + 1 content + 1 source_metadata + 1 file_index + 1 topic_portfolio + 1 e12_late_interaction + 2 legacy.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 15 `ColumnFamilyDescriptor`s for teleological storage.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_teleological_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_teleological_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), 15);
/// ```
pub fn get_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_FINGERPRINTS, fingerprint_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_TOPIC_PROFILES, topic_profile_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_E13_SPLADE_INVERTED,
            e13_splade_inverted_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_E6_SPARSE_INVERTED,
            e6_sparse_inverted_cf_options(cache),
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
        // Source metadata storage CF
        ColumnFamilyDescriptor::new(CF_SOURCE_METADATA, source_metadata_cf_options(cache)),
        // File index for file watcher management
        ColumnFamilyDescriptor::new(CF_FILE_INDEX, file_index_cf_options(cache)),
        // Topic portfolio persistence for session continuity
        ColumnFamilyDescriptor::new(CF_TOPIC_PORTFOLIO, topic_portfolio_cf_options(cache)),
        // TASK-STORAGE-P2-001: E12 Late Interaction token storage CF
        ColumnFamilyDescriptor::new(
            CF_E12_LATE_INTERACTION,
            e12_late_interaction_cf_options(cache),
        ),
        // Legacy column families for backwards compatibility
        ColumnFamilyDescriptor::new(CF_SESSION_IDENTITY, legacy_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_EGO_NODE, legacy_cf_options(cache)),
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
/// Returns 28 descriptors total: 15 teleological (13 active + 2 legacy) + 13 quantized embedder.
/// Use this when opening a database that needs both fingerprint and per-embedder storage.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 28 `ColumnFamilyDescriptor`s.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_all_teleological_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_all_teleological_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), 28); // 15 teleological + 13 embedder
/// ```
pub fn get_all_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    let mut descriptors = get_teleological_cf_descriptors(cache);
    descriptors.extend(get_quantized_embedder_cf_descriptors(cache));
    descriptors
}

// =============================================================================
// CODE EMBEDDING COLUMN FAMILIES (Separate code-specific storage)
// =============================================================================
// These column families store code entities and embeddings separately from
// regular text content. Code uses E7 (Qodo-Embed) as primary embedder and
// AST-based chunking instead of word-based chunking.
// =============================================================================

/// Column family for code entity storage.
///
/// Stores code entities (functions, structs, traits, etc.) extracted via AST parsing.
/// Each entity contains metadata like language, signature, parent type, line numbers.
///
/// Key: UUID (16 bytes) → Value: CodeEntity serialized via bincode (~500-5000 bytes)
///
/// # Storage Details
/// - LZ4 compression (code text compresses well)
/// - Bloom filter for fast UUID lookups
/// - Point lookups by entity ID
pub const CF_CODE_ENTITIES: &str = "code_entities";

/// Column family for code E7 embeddings (1536D).
///
/// Stores E7 (Qodo-Embed-1-1.5B) embeddings for code entities.
/// E7 is the PRIMARY embedder for code, unlike text which uses E1.
///
/// Key: UUID (16 bytes) → Value: Vec<f32> (1536 × 4 = 6144 bytes)
///
/// # Storage Details
/// - LZ4 compression (float arrays compress moderately)
/// - Bloom filter for fast lookups
/// - Each embedding is exactly 6144 bytes
pub const CF_CODE_E7_EMBEDDINGS: &str = "code_e7_embeddings";

/// Column family for code file index.
///
/// Maps file paths to code entity IDs for efficient file-level operations.
/// Used for cleanup when files are deleted/modified.
///
/// Key: file_path bytes (UTF-8) → Value: CodeFileIndexEntry serialized via bincode
///
/// # Storage Details
/// - LZ4 compression (paths and UUID lists compress well)
/// - Bloom filter for fast path existence checks
/// - Prefix iteration for path-based queries
pub const CF_CODE_FILE_INDEX: &str = "code_file_index";

/// Column family for code entity name index.
///
/// Secondary index for searching entities by name (function name, struct name, etc.).
/// Enables fast "find function named X" queries.
///
/// Key: entity_name bytes (UTF-8) → Value: Vec<Uuid> serialized via bincode
///
/// # Storage Details
/// - LZ4 compression
/// - Prefix scan support for partial name matching
pub const CF_CODE_NAME_INDEX: &str = "code_name_index";

/// Column family for code signature index.
///
/// Secondary index for searching by function signature hash.
/// Enables "find functions with this signature pattern" queries.
///
/// Key: signature_hash (32 bytes SHA256) → Value: Vec<Uuid> serialized via bincode
///
/// # Storage Details
/// - No compression (hash keys are random, don't compress)
/// - Bloom filter for fast lookups
pub const CF_CODE_SIGNATURE_INDEX: &str = "code_signature_index";

/// All code column family names (5 total).
pub const CODE_CFS: &[&str] = &[
    CF_CODE_ENTITIES,
    CF_CODE_E7_EMBEDDINGS,
    CF_CODE_FILE_INDEX,
    CF_CODE_NAME_INDEX,
    CF_CODE_SIGNATURE_INDEX,
];

/// Total count of code CFs.
pub const CODE_CF_COUNT: usize = 5;

/// Options for code entity storage (~500-5000 bytes per entity).
///
/// # Configuration
/// - 8KB block size (fits multiple entities per block)
/// - LZ4 compression (code text compresses ~50%)
/// - Bloom filter for fast UUID lookups
/// - 16-byte prefix extractor for UUID keys
pub fn code_entities_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(8 * 1024); // 8KB blocks
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(64); // 64MB hint
    opts.create_if_missing(true);
    opts
}

/// Options for code E7 embedding storage (6144 bytes per embedding).
///
/// # Configuration
/// - 8KB block size (fits 1 embedding per block with overhead)
/// - LZ4 compression (modest compression for floats)
/// - Bloom filter for fast lookups
/// - 16-byte prefix extractor for UUID keys
pub fn code_e7_embeddings_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(8 * 1024); // 8KB for 6144-byte embeddings
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(128); // 128MB hint for embeddings
    opts.create_if_missing(true);
    opts
}

/// Options for code file index storage.
///
/// # Configuration
/// - LZ4 compression (paths and UUID lists compress well)
/// - Bloom filter for fast path existence checks
pub fn code_file_index_cf_options(cache: &Cache) -> Options {
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

/// Options for code name index storage.
///
/// # Configuration
/// - LZ4 compression
/// - No bloom filter (prefix scans need full iteration)
pub fn code_name_index_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    opts
}

/// Options for code signature index storage.
///
/// # Configuration
/// - No compression (hash keys don't compress)
/// - Bloom filter for fast lookups
pub fn code_signature_index_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::None);
    opts.optimize_for_point_lookup(32); // 32MB hint
    opts.create_if_missing(true);
    opts
}

/// Get all 5 code column family descriptors.
///
/// These CFs store code entities and embeddings separately from regular content.
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Returns
/// Vector of 5 `ColumnFamilyDescriptor`s for code storage.
pub fn get_code_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_CODE_ENTITIES, code_entities_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_CODE_E7_EMBEDDINGS, code_e7_embeddings_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_CODE_FILE_INDEX, code_file_index_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_CODE_NAME_INDEX, code_name_index_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_CODE_SIGNATURE_INDEX, code_signature_index_cf_options(cache)),
    ]
}

// =============================================================================
// CAUSAL RELATIONSHIP COLUMN FAMILIES
// =============================================================================
// These column families store LLM-generated causal relationship descriptions
// with full provenance linking back to source content and fingerprints.
// =============================================================================

/// Column family for causal relationship storage.
///
/// Stores LLM-generated causal relationships with embedded descriptions
/// for semantic search. Each relationship contains:
/// - 1-3 paragraph description
/// - E1 1024D embedding of the description
/// - Full provenance (source content + fingerprint ID)
///
/// Key: UUID (16 bytes) → Value: CausalRelationship serialized via bincode (~5-15KB)
///
/// # Storage Details
/// - 8KB block size (fits 1 relationship per block)
/// - LZ4 compression (text + embeddings compress well)
/// - Bloom filter for fast UUID lookups
/// - Point lookups by causal_relationship_id
pub const CF_CAUSAL_RELATIONSHIPS: &str = "causal_relationships";

/// Column family for causal relationships by source fingerprint index.
///
/// Secondary index enabling "find all causal relationships from memory X" queries.
/// Essential for provenance traversal and context injection.
///
/// Key: source_fingerprint_id (16 bytes) → Value: Vec<Uuid> serialized via bincode
///
/// # Storage Details
/// - LZ4 compression (UUID lists compress well)
/// - Bloom filter for fast source existence checks
/// - Point lookups by source fingerprint ID
pub const CF_CAUSAL_BY_SOURCE: &str = "causal_by_source";

/// All causal relationship column family names (2 total).
pub const CAUSAL_CFS: &[&str] = &[CF_CAUSAL_RELATIONSHIPS, CF_CAUSAL_BY_SOURCE];

/// Total count of causal CFs.
pub const CAUSAL_CF_COUNT: usize = 2;

/// Options for causal relationship storage (~5-15KB per relationship).
///
/// # Configuration
/// - 8KB block size (fits 1 relationship per block)
/// - LZ4 compression (text + embeddings compress ~40%)
/// - Bloom filter for fast UUID lookups
/// - 16-byte prefix extractor for UUID keys
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn causal_relationships_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(8 * 1024); // 8KB blocks for ~5-15KB relationships
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(64); // 64MB hint
    opts.create_if_missing(true);
    opts
}

/// Options for causal-by-source index storage.
///
/// # Configuration
/// - LZ4 compression (UUID lists compress well)
/// - Bloom filter for fast source existence checks
/// - 16-byte prefix extractor for source fingerprint UUID keys
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn causal_by_source_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(32); // 32MB hint
    opts.create_if_missing(true);
    opts
}

/// Get all 2 causal relationship column family descriptors.
///
/// These CFs store LLM-generated causal descriptions with provenance.
///
/// # Arguments
/// * `cache` - Shared block cache
///
/// # Returns
/// Vector of 2 `ColumnFamilyDescriptor`s for causal relationship storage.
pub fn get_causal_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_CAUSAL_RELATIONSHIPS, causal_relationships_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_CAUSAL_BY_SOURCE, causal_by_source_cf_options(cache)),
    ]
}

/// Get ALL column family descriptors (teleological + embedder + code + causal).
///
/// Returns 35 descriptors total: 15 teleological + 13 quantized embedder + 5 code + 2 causal.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 35 `ColumnFamilyDescriptor`s.
pub fn get_all_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    let mut descriptors = get_all_teleological_cf_descriptors(cache);
    descriptors.extend(get_code_cf_descriptors(cache));
    descriptors.extend(get_causal_cf_descriptors(cache));
    descriptors
}
