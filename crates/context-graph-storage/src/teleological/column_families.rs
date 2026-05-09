//! Extended column families for teleological fingerprint storage.
//!
//! These CFs extend the base 11 CFs defined in ../column_families.rs.
//!
//! # FAIL FAST Policy
//!
//! All option builders are infallible at construction time. Errors only
//! occur at DB open time, and those are surfaced by RocksDB itself.
//!
//! # Audit-14 STOR-L5 Note on Repetitive CF Option Builders
//!
//! This file contains ~25 CF option builder functions that follow similar patterns
//! (create BlockBasedOptions, set cache/bloom/compression, create Options, apply
//! write buffer limits). This repetition is **intentional** — each CF has unique
//! settings (block size, prefix extractor, compression type, point lookup hints)
//! that must be independently tunable. Extracting a generic builder would either
//! (a) require a config struct with as many fields as the current code has lines,
//! or (b) obscure the per-CF tuning behind indirection. The current approach
//! trades DRY for clarity and independent tunability.

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, SliceTransform};

// Audit-14 STOR-L2 FIX: Reuse the canonical apply_write_buffer_limits from the base
// column_families module instead of duplicating it here.
use crate::column_families::apply_write_buffer_limits;

/// Shared options builder for CFs whose keys are always 16-byte UUIDs.
///
/// Introduced for `CF_TRAINING_RECORDS`; intentionally NOT retrofitted onto the
/// other CFs (each one has bespoke block sizes / prefix lengths / compression
/// choices that would collapse into a sprawling config struct if merged).
///
/// Configuration applied:
/// - Shared block cache (argument).
/// - Bloom filter (10 bits/key) for fast point-lookups by UUID.
/// - Index/filter blocks live in the shared cache.
/// - `block_size_kb`-sized blocks (caller-tunable; 64KB is a reasonable default
///   for large mixed-payload values).
/// - 16-byte fixed prefix extractor for UUID keys.
/// - LZ4 compression (good fit for mixed binary/text).
/// - Level compaction (mostly-append write pattern).
/// - `write_buffer_mb` write buffer.
pub fn uuid_prefixed_cf_options(
    cache: &Cache,
    block_size_kb: usize,
    write_buffer_mb: usize,
) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);
    block_opts.set_block_size(block_size_kb * 1024);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16));
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    apply_write_buffer_limits(&mut opts, write_buffer_mb);
    opts.create_if_missing(true);
    opts
}

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
// CONTENT AND METADATA COLUMN FAMILIES
// =============================================================================

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
/// Key: UUID (16 bytes) -> Value: SourceMetadata serialized via JSON (~100-500 bytes)
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
// ENTITY PROVENANCE COLUMN FAMILIES (Phase 3a Provenance)
// =============================================================================

/// Reserved — not yet populated. Opened for forward compatibility.
///
/// Column family for entity provenance storage.
///
/// Maps entity canonical_id + memory_id to EntityProvenance records.
/// Enables "show me where this entity was extracted from" queries.
///
/// Key: `{canonical_id_bytes}_{memory_uuid_bytes}` (variable + 16 bytes)
/// Value: EntityProvenance serialized via bincode (~200-2000 bytes)
///
/// # Storage Details
/// - LZ4 compression (text excerpts compress well)
/// - Bloom filter for fast existence checks
/// - Prefix scan for all provenances of a given entity
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub const CF_ENTITY_PROVENANCE: &str = "entity_provenance";

// =============================================================================
// AUDIT LOG COLUMN FAMILIES (Phase 1.1 Provenance)
// =============================================================================
// Append-only audit log for tracking all provenance-relevant operations.
// NO update or delete operations -- append-only by design.
// =============================================================================

/// Column family for the primary audit log storage.
///
/// Stores `AuditRecord` entries in chronological order. Append-only --
/// records are never updated or deleted.
///
/// Key: `{timestamp_nanos_be}_{uuid_bytes}` (8 + 16 = 24 bytes)
/// Value: AuditRecord serialized via JSON (~200-2000 bytes)
///
/// # Storage Details
/// - LZ4 compression (JSON parameters and strings compress well)
/// - Bloom filter for fast existence checks
/// - Big-endian timestamp prefix ensures chronological iteration
/// - Append-only: NO update or delete operations
pub const CF_AUDIT_LOG: &str = "audit_log";

/// Column family for audit log secondary index by target entity.
///
/// Enables efficient "show me all operations on memory X" queries.
/// Append-only -- mirrors the primary log's immutability contract.
///
/// Key: `{target_uuid_bytes}_{timestamp_nanos_be}` (16 + 8 = 24 bytes)
/// Value: Primary key bytes (24 bytes) for joining back to CF_AUDIT_LOG
///
/// # Storage Details
/// - LZ4 compression
/// - Bloom filter for fast target existence checks
/// - 16-byte prefix extractor for UUID-based prefix scans
/// - Append-only: NO update or delete operations
pub const CF_AUDIT_BY_TARGET: &str = "audit_by_target";

// =============================================================================
// LIFECYCLE PROVENANCE COLUMN FAMILIES (Phase 4)
// =============================================================================
// Permanent storage for merge history and importance change history.
// Unlike ReversalRecords (30-day), these are PERMANENT -- never expire, never deleted.
// =============================================================================

/// Column family for permanent merge history storage (Phase 4, item 5.10).
///
/// Stores `MergeRecord` entries permanently. Unlike reversal records which
/// expire after 30 days, merge history is retained indefinitely for
/// complete lineage tracking.
///
/// Key: `{merged_uuid_bytes}_{timestamp_nanos_be}` (16 + 8 = 24 bytes)
/// Value: MergeRecord serialized via JSON (~500-5000 bytes)
///
/// # Storage Details
/// - LZ4 compression (JSON fingerprint data compresses well)
/// - Bloom filter for fast merged_id lookups
/// - PERMANENT: never expires, never deleted
pub const CF_MERGE_HISTORY: &str = "merge_history";

/// Column family for importance change history storage (Phase 4, item 5.11).
///
/// Stores `ImportanceChangeRecord` entries permanently for auditing
/// all importance score changes over time.
///
/// Key: `{memory_uuid_bytes}_{timestamp_nanos_be}` (16 + 8 = 24 bytes)
/// Value: ImportanceChangeRecord serialized via JSON (~100-500 bytes)
///
/// # Storage Details
/// - LZ4 compression (structured data compresses well)
/// - Bloom filter for fast memory_id lookups
/// - PERMANENT: never expires, never deleted
pub const CF_IMPORTANCE_HISTORY: &str = "importance_history";

// =============================================================================
// TOOL CALL PROVENANCE COLUMN FAMILIES (Phase 5, item 5.12)
// =============================================================================

/// Reserved — not yet populated. Opened for forward compatibility.
///
/// Column family for tool call → memory mapping (Phase 5, item 5.12).
///
/// Maps tool_use_id to fingerprint IDs created by that tool call.
/// Enables "which memories were created by this tool invocation?" queries.
///
/// Key: tool_use_id bytes (UTF-8, variable length)
/// Value: Vec<Uuid> serialized via bincode (~16-160 bytes per entry)
///
/// # Storage Details
/// - LZ4 compression
/// - Bloom filter for fast tool_use_id lookups
/// - FAIL FAST: No fallback options
pub const CF_TOOL_CALL_INDEX: &str = "tool_call_index";

// =============================================================================
// CONSOLIDATION RECOMMENDATION PERSISTENCE (Phase 5, item 5.14)
// =============================================================================

/// Reserved — not yet populated. Opened for forward compatibility.
///
/// Column family for consolidation recommendation persistence (Phase 5, item 5.14).
///
/// Stores ConsolidationRecommendation records for review and tracking.
///
/// Key: `{recommendation_uuid_bytes}` (16 bytes)
/// Value: ConsolidationRecommendation serialized via bincode (~500-5000 bytes)
///
/// # Storage Details
/// - LZ4 compression
/// - Bloom filter for fast lookups
/// - FAIL FAST: No fallback options
pub const CF_CONSOLIDATION_RECOMMENDATIONS: &str = "consolidation_recommendations";

// =============================================================================
// EMBEDDING VERSION REGISTRY (Phase 6, item 5.15)
// =============================================================================

/// Column family for embedding version registry (Phase 6, item 5.15).
///
/// Tracks which embedder model versions were used to compute each fingerprint's
/// embeddings. Enables stale embedding detection and targeted re-embedding.
///
/// Key: fingerprint_uuid_bytes (16 bytes)
/// Value: EmbeddingVersionRecord serialized via JSON (~200-500 bytes)
///
/// # Storage Details
/// - LZ4 compression (structured data compresses well)
/// - Bloom filter for fast fingerprint_id lookups
/// - 16-byte prefix extractor for UUID keys
/// - PERMANENT: never expires, never deleted
pub const CF_EMBEDDING_REGISTRY: &str = "embedding_registry";

// =============================================================================
// CUSTOM WEIGHT PROFILE STORAGE
// =============================================================================

/// Column family for custom weight profiles.
///
/// Stores user-created weight profiles that can be referenced by name
/// in search_graph and get_unified_neighbors.
///
/// Key: profile_name bytes (UTF-8, variable length)
/// Value: [f32; 14] serialized via JSON (~100-200 bytes)
///
/// # Storage Details
/// - LZ4 compression
/// - Bloom filter for fast name lookups
/// - Point lookups by profile name
pub const CF_CUSTOM_WEIGHT_PROFILES: &str = "custom_weight_profiles";

/// Column family for persisted HNSW index graphs.
///
/// Stores serialized usearch HNSW graphs and their UUID-to-key mappings
/// so indexes can be restored from disk on startup instead of full O(n) rebuild.
///
/// Key: `graph:{embedder}` → Value: usearch serialized bytes (1-100MB)
/// Key: `meta:{embedder}` → Value: JSON { id_to_key, key_to_id, next_key, count }
///
/// # Storage Details
/// - LZ4 compression (binary graph data compresses ~30-50%)
/// - No bloom filter (few keys, ~30 total)
/// - Large block size (values are large)
/// - Updated periodically + at shutdown
pub const CF_HNSW_GRAPHS: &str = "hnsw_graphs";

// =============================================================================
// TRAINING DATA EXPORT STORAGE
// =============================================================================

/// Column family for exported training records (one per memory).
///
/// Populated by the `export_training_corpus` MCP tool. Each record bundles
/// the memory's content, all 13 embedding vectors, 13D topic profile, 78
/// synergy-weighted cross-correlations, 6D group alignments, typed edges,
/// K-NN neighbors, causal labels, and topic memberships for consumption by
/// ML training pipelines (in-process or dumped via sidecar tooling).
///
/// Key: UUID bytes (16) — same key as the source fingerprint in `CF_FINGERPRINTS`.
/// Value: [version: u8 = 1][bincode-encoded `TrainingRecord`].
///
/// # Storage Details
/// - LZ4 compression (~30-50% reduction for mixed binary data)
/// - Bloom filter for fast point lookups by memory UUID
/// - 16-byte prefix extractor for UUID keys
/// - 8MB write buffer (values are MB-sized when full embeddings included)
pub const CF_TRAINING_RECORDS: &str = "training_records";

/// Column family for Learning-as-UTL event records.
///
/// Each row stores one interaction-level before/after state transition with
/// deterministic UTL features and baseline signals. These records are separate
/// from E1-E14 fingerprints and training records so learning-event semantics
/// do not change search/storage contracts for production embedders.
///
/// Key: event UUID bytes (16).
/// Value: `[LEARNING_EVENT_VERSION: u8][bincode-encoded LearningEvent]`.
///
/// # Storage Details
/// - LZ4 compression for mixed text/vector payloads
/// - Bloom filter for fast point lookup by event UUID
/// - 16-byte prefix extractor for UUID keys
/// - 4MB write buffer (event records are usually small but may include text)
pub const CF_LEARNING_EVENTS: &str = "learning_events";

/// Learner registry records.
///
/// Key: learner UUID bytes (16).
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerProfile]`.
pub const CF_LEARNER_PROFILE: &str = "learner_profile";

/// Learner regulated-state constellation records.
///
/// Key: `[learner_id:16][selector_kind:u8]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode learner constellation payload]`.
pub const CF_LEARNER_CONSTELLATIONS: &str = "learner_constellations";

/// Per-session E15-E21 learner fingerprint records.
///
/// Key: `[learner_id:16][session_ts:u64 big-endian]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerFingerprint]`.
pub const CF_FINGERPRINTS_LEARNER: &str = "fingerprints_learner";

/// Per-trace consolidation values for UTL M(t).
///
/// Key: `[learner_id:16][trace_id:16]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerMTrace]`.
pub const CF_LEARNER_M_PER_TRACE: &str = "learner_m_per_trace";

/// Session-by-session state vectors.
///
/// Key: `[learner_id:16][session_ts:u64 big-endian]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerStateVector]`.
pub const CF_LEARNER_STATE_HISTORY: &str = "learner_state_history";

/// Per-learner deployment state centroids.
///
/// Key: `[learner_id:16][skill_id:16]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerGoalState]`.
pub const CF_LEARNER_GOAL_STATES: &str = "learner_goal_states";

/// Retrieval-practice event log.
///
/// Key: `[learner_id:16][trace_id:16][ts:u64 big-endian]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerRetrievalLog]`.
pub const CF_LEARNER_RETRIEVAL_LOG: &str = "learner_retrieval_log";

/// Sleep-derived consolidation multipliers.
///
/// Key: `[learner_id:16][session_ts:u64 big-endian]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerKSleep]`.
pub const CF_LEARNER_K_SLEEP: &str = "learner_k_sleep";

/// Per-skill expert-answer centroids for Delta C references.
///
/// Key: `[skill_id:16][modality:u8]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode GoalCentroid]`.
pub const CF_GOAL_CENTROIDS: &str = "goal_centroids";

/// Per-session UTL Delta/L diagnostic log.
///
/// Key: `[learner_id:16][session_ts:u64 big-endian]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerDeltaLog]`.
pub const CF_LEARNER_DELTA_LOG: &str = "learner_delta_log";

/// Learner-specific audit/provenance chain.
///
/// Key: `[learner_id:16][ts:u64 big-endian][audit_id:16]`.
/// Value: `[LEARNER_RECORD_VERSION: u8][bincode LearnerAuditEntry]`.
pub const CF_LEARNER_AUDIT: &str = "learner_audit";

/// Matrix-shaped learner training dataset shards.
///
/// Populated by `export_learner_training_dataset`. Each record materializes one
/// task-specific row-major feature matrix plus row metadata and provenance
/// hashes, compiled from `CF_LEARNING_EVENTS` and learner-state CFs.
///
/// Key: dataset UUID bytes (16).
/// Value: `[LEARNER_TRAINING_DATASET_VERSION: u8][bincode LearnerTrainingDataset]`.
pub const CF_LEARNER_TRAINING_DATASETS: &str = "learner_training_datasets";

/// Column family for compiled constellation records (Phase 2).
///
/// Each constellation aggregates per-embedder centroids + spread statistics
/// over a caller-selected set of memories (topic / session / tag / time /
/// explicit id list). Centroids are dense for symmetric/asymmetric embedders
/// (E1/E2/E3/E4/E5/E7/E8/E9/E10/E11), sparse top-K terms for E6/E13, and a
/// pooled 128D vector for E12 token-level.
///
/// Key: constellation_id (UUID, 16 bytes) — fresh per compilation, not the
/// member id.
/// Value: `[CONSTELLATION_VERSION: u8][bincode-encoded Constellation]`.
///
/// # Storage Details
/// - LZ4 compression (centroids are mostly f32 arrays, modest compression)
/// - Bloom filter for fast UUID point lookups
/// - 16-byte prefix extractor for UUID keys
/// - 8MB write buffer (constellation records can reach several hundred KB)
pub const CF_CONSTELLATIONS: &str = "constellations";

/// Secondary index CF mapping selector → constellation UUID (Phase 2).
///
/// Enables O(1) "does a constellation already exist for topic T / session S /
/// tag T / explicit id set?" lookups instead of a linear scan over
/// `CF_CONSTELLATIONS`.
///
/// Key: `[selector_kind: u8][selector_hash: 16B]` (17 bytes total). The
/// selector hash is the first 16 bytes of `SHA-256(canonical_form(selector))`.
/// See `ConstellationSelector::canonical_form` for the exact text form hashed.
/// Value: constellation UUID (16 bytes).
///
/// # Storage Details
/// - LZ4 compression (tiny entries; compression mostly cosmetic)
/// - Bloom filter for fast existence checks
/// - 1-byte prefix extractor (selector kind) for per-kind range scans
/// - 2MB write buffer (secondary index is append-light)
pub const CF_CONSTELLATION_BY_SELECTOR: &str = "constellation_by_selector";

/// Column family for mined contrastive pair records (Phase 3).
///
/// Each pair bundles an `(anchor, negative, similarity_profile)` triple ready
/// for contrastive / metric-learning training, along with the classified
/// anomaly kind, high/low embedder indices and disagreement magnitude.
///
/// Key: `[anchor_uuid: 16B][negative_uuid: 16B]` (32 bytes total). The
/// composite key guarantees idempotency per `(anchor, negative)`.
/// Value: `[CONTRASTIVE_PAIR_VERSION: u8][bincode-encoded ContrastivePair]`.
///
/// # Storage Details
/// - LZ4 compression (mostly small dense/ short-text payloads)
/// - Bloom filter for fast point lookups by composite key
/// - 32-byte prefix extractor so `[anchor][negative]` stays a stable prefix
/// - 4MB write buffer (pair records are typically 1-5KB each)
pub const CF_CONTRASTIVE_PAIRS: &str = "contrastive_pairs";

/// Secondary index CF mapping `[kind:u8][anchor:16B][neg:16B]` → empty bytes
/// (Phase 3).
///
/// Enables O(log N) prefix-scan enumeration of every pair of a given anomaly
/// kind without reading the primary record payload.
///
/// # Storage Details
/// - LZ4 compression (tiny / empty values — mostly cosmetic)
/// - Bloom filter for fast existence checks
/// - 1-byte prefix extractor (kind byte)
/// - 2MB write buffer (append-light secondary index)
pub const CF_CONTRASTIVE_BY_KIND: &str = "contrastive_by_kind";

/// Secondary index CF mapping `anchor_uuid: 16B` → `bincode(Vec<Uuid>)` of
/// every mined negative (Phase 3).
///
/// Enables O(1) "list every negative mined for this anchor" lookups. Values
/// are deduplicated lazily on read.
///
/// # Storage Details
/// - LZ4 compression (UUID lists compress modestly)
/// - Bloom filter for fast anchor existence checks
/// - 16-byte prefix extractor (anchor UUID)
/// - 2MB write buffer
pub const CF_CONTRASTIVE_BY_ANCHOR: &str = "contrastive_by_anchor";

/// Column family for exported typed-edge training records (F1).
///
/// Populated by the `export_typed_edges_corpus` MCP tool. Each record is one
/// (source, target, edge_type) observation joined with content, source
/// metadata, optional mechanism_type, and (if present) an LLM validation.
///
/// Key: `[source_uuid: 16B][target_uuid: 16B][edge_type: u8]` (33 bytes). The
/// edge-type suffix lets the same `(source, target)` pair be exported under
/// multiple classifications when an LLM reclassification exists.
///
/// Value: `[version: u8 = TYPED_EDGE_RECORD_VERSION][bincode-encoded
/// TypedEdgeTrainingRecord]`.
///
/// # Storage Details
/// - LZ4 compression
/// - Bloom filter for fast composite-key lookups
/// - 33-byte prefix extractor so the whole composite key is a stable prefix
/// - 4MB write buffer (records are typically 2-10 KB each)
pub const CF_TYPED_EDGE_RECORDS: &str = "typed_edge_records";

/// Column family for LLM-validated typed edges (F4).
///
/// Written by external typed-edge validation after a validator confirms,
/// rejects, or reclassifies an auto-derived edge.
///
/// Key: `[source_uuid: 16B][target_uuid: 16B][edge_type: u8]` (33 bytes) —
/// matches the primary-key shape of the edge being validated.
/// Value: `[version: u8 = LLM_EDGE_VALIDATION_VERSION][bincode-encoded
/// LLMEdgeValidation]`.
///
/// # Storage Details
/// - LZ4 compression (rationale text compresses)
/// - Bloom filter for fast existence checks
/// - 33-byte prefix extractor
/// - 2MB write buffer (append-light)
pub const CF_TYPED_EDGE_VALIDATIONS: &str = "typed_edge_validations";

/// All teleological column family names.
pub const TELEOLOGICAL_CFS: &[&str] = &[
    CF_FINGERPRINTS,
    CF_TOPIC_PROFILES,
    CF_E13_SPLADE_INVERTED,
    CF_E6_SPARSE_INVERTED,
    CF_E1_MATRYOSHKA_128,
    CF_CONTENT,
    CF_SOURCE_METADATA,
    CF_FILE_INDEX,
    CF_TOPIC_PORTFOLIO,
    CF_E12_LATE_INTERACTION,
    CF_ENTITY_PROVENANCE,
    CF_AUDIT_LOG,
    CF_AUDIT_BY_TARGET,
    CF_MERGE_HISTORY,
    CF_IMPORTANCE_HISTORY,
    CF_TOOL_CALL_INDEX,
    CF_CONSOLIDATION_RECOMMENDATIONS,
    CF_EMBEDDING_REGISTRY,
    CF_CUSTOM_WEIGHT_PROFILES,
    CF_HNSW_GRAPHS,
    CF_TRAINING_RECORDS,
    CF_LEARNING_EVENTS,
    CF_LEARNER_PROFILE,
    CF_LEARNER_CONSTELLATIONS,
    CF_FINGERPRINTS_LEARNER,
    CF_LEARNER_M_PER_TRACE,
    CF_LEARNER_STATE_HISTORY,
    CF_LEARNER_GOAL_STATES,
    CF_LEARNER_RETRIEVAL_LOG,
    CF_LEARNER_K_SLEEP,
    CF_GOAL_CENTROIDS,
    CF_LEARNER_DELTA_LOG,
    CF_LEARNER_AUDIT,
    CF_LEARNER_TRAINING_DATASETS,
    CF_CONSTELLATIONS,
    CF_CONSTELLATION_BY_SELECTOR,
    CF_CONTRASTIVE_PAIRS,
    CF_CONTRASTIVE_BY_KIND,
    CF_CONTRASTIVE_BY_ANCHOR,
    CF_TYPED_EDGE_RECORDS,
    CF_TYPED_EDGE_VALIDATIONS,
];

/// Total count of teleological CFs.
pub const TELEOLOGICAL_CF_COUNT: usize = 41;

// =============================================================================
// QUANTIZED EMBEDDER COLUMN FAMILIES (13 CFs for per-embedder storage)
// =============================================================================
// Reserved — quantized storage not yet wired into production write path.
// CFs opened for forward compatibility.
//
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
/// Quantization: PQ-8 (8 bytes compressed from 1536D).
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

/// Column family for E11_Entity/Kepler (production slot 10) quantized embeddings.
/// Quantization: PQ-8 (8 bytes compressed from 768D).
pub const CF_EMB_10: &str = "emb_10";

/// Column family for E12_LateInteraction (ModelId=11) quantized embeddings.
/// Quantization: TokenPruning (variable size, ~50% of original).
pub const CF_EMB_11: &str = "emb_11";

/// Column family for E13_SPLADE (ModelId=12) quantized embeddings.
/// Quantization: SparseNative (variable size, indices+values).
pub const CF_EMB_12: &str = "emb_12";

/// Column family for E14_BgeM3Dense (ModelId=13) quantized embeddings.
/// Quantization: PQ-8 (8 bytes compressed from 1024D).
pub const CF_EMB_13: &str = "emb_13";

/// All 14 quantized embedder column family names.
/// Maps to ModelId indices 0-13 for per-embedder HNSW storage.
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
    CF_EMB_13, // E14_BgeM3Dense
];

/// Total count of quantized embedder CFs (14 after E14 BGE-M3 was added).
pub const QUANTIZED_EMBEDDER_CF_COUNT: usize = 14;

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
    apply_write_buffer_limits(&mut opts, 8); // 63KB values — needs larger buffer
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
    apply_write_buffer_limits(&mut opts, 2); // 52-byte values
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
    apply_write_buffer_limits(&mut opts, 4); // posting lists grow with corpus
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
    apply_write_buffer_limits(&mut opts, 4); // posting lists grow with corpus
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
    apply_write_buffer_limits(&mut opts, 2); // 512-byte values
    opts.create_if_missing(true);
    opts
}

/// Options for custom weight profile storage (variable size per profile).
///
/// # Configuration
/// - LZ4 compression (profiles may have repetitive string patterns)
/// - Bloom filter for fast lookups
/// - Cache index and filter blocks
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn custom_weight_profiles_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    apply_write_buffer_limits(&mut opts, 2); // small JSON
    opts.create_if_missing(true);
    opts
}

/// Options for HNSW graph persistence (large binary blobs).
///
/// # Configuration
/// - LZ4 compression (~30-50% reduction for binary graph data)
/// - No bloom filter (very few keys, ~30 total)
/// - Large block size (values are MB-sized)
///
/// # Key Format
/// `graph:{embedder_name}` or `meta:{embedder_name}` (UTF-8 string)
///
/// # Value Format
/// - graph keys: usearch serialized bytes (1-100MB per index)
/// - meta keys: JSON { id_to_key, key_to_id, next_key, count }
pub fn hnsw_graphs_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    // No bloom filter — very few keys
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    apply_write_buffer_limits(&mut opts, 8); // MB-sized values
    opts.create_if_missing(true);
    opts
}

/// Options for training record storage.
///
/// # Configuration
/// - LZ4 compression (~30-50% reduction for mixed float/string content)
/// - Bloom filter for fast UUID lookups
/// - 16-byte prefix extractor (UUID keys)
/// - 8MB write buffer (values are large when full embeddings are included)
/// - Level compaction for mostly-append write pattern
///
/// # Key / Value Format
/// Key: UUID bytes (16).
/// Value: `[version: u8 = TRAINING_RECORD_VERSION][bincode-encoded TrainingRecord]`.
pub fn training_records_cf_options(cache: &Cache) -> Options {
    // 64KB blocks pack many ~46KB records per block;
    // 8MB write buffer accommodates MB-sized per-record values.
    uuid_prefixed_cf_options(cache, 64, 8)
}

/// Options for learning event storage.
///
/// Learning events are UUID-keyed and bincode-encoded. They contain a small
/// amount of text plus 14D/91D vector state, so 32KB blocks and a 4MB write
/// buffer are sufficient while preserving point-lookup behavior.
pub fn learning_events_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for learner profile records keyed by learner UUID.
pub fn learner_profile_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 16, 2)
}

/// Options for learner constellation records keyed by learner UUID prefix.
pub fn learner_constellations_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for learner fingerprints keyed by learner UUID + session timestamp.
pub fn fingerprints_learner_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 64, 8)
}

/// Options for per-trace M(t) records keyed by learner UUID + trace UUID.
pub fn learner_m_per_trace_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 16, 2)
}

/// Options for learner state history keyed by learner UUID + session timestamp.
pub fn learner_state_history_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for deployment goal states keyed by learner UUID + skill UUID.
pub fn learner_goal_states_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for retrieval-practice logs keyed by learner UUID + trace UUID + ts.
pub fn learner_retrieval_log_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for sleep multiplier records keyed by learner UUID + session ts.
pub fn learner_k_sleep_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 16, 2)
}

/// Options for expert-answer goal centroids keyed by skill UUID + modality.
pub fn goal_centroids_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for per-session UTL delta logs keyed by learner UUID + session ts.
pub fn learner_delta_log_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 32, 4)
}

/// Options for learner audit rows keyed by learner UUID + timestamp + audit UUID.
pub fn learner_audit_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 16, 2)
}

/// Options for compiled learner training matrix datasets keyed by dataset UUID.
pub fn learner_training_datasets_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 64, 8)
}

/// Options for `CF_CONSTELLATIONS` (Phase 2).
///
/// Constellation records are UUID-keyed and bincode-encoded; the record
/// shape is similar to a large-payload training record (several hundred KB
/// with full 13-embedder centroids). Reuses `uuid_prefixed_cf_options` with
/// 64KB block / 8MB write buffer tuning.
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn constellations_cf_options(cache: &Cache) -> Options {
    uuid_prefixed_cf_options(cache, 64, 8)
}

/// Options for `CF_CONSTELLATION_BY_SELECTOR` (Phase 2 secondary index).
///
/// Key layout: `[selector_kind: u8][selector_hash: 16B]`. We use a 1-byte
/// prefix extractor so `prefix_iterator_cf(kind_byte)` can enumerate every
/// constellation of a given selector kind (e.g. "list all topic-based
/// constellations").
///
/// # Configuration
/// - LZ4 compression (tiny 16B values compress trivially)
/// - Bloom filter for fast existence checks
/// - 1-byte prefix extractor (selector kind)
/// - 2MB write buffer (append-light secondary index)
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn constellation_by_selector_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    // 1-byte prefix = selector kind byte. Enables per-kind prefix iteration.
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(1));
    apply_write_buffer_limits(&mut opts, 2);
    opts.create_if_missing(true);
    opts
}

/// Options for `CF_CONTRASTIVE_PAIRS` (Phase 3 primary store).
///
/// Key layout: `[anchor_uuid: 16B][negative_uuid: 16B]` (32 bytes). A 32-byte
/// fixed-prefix extractor pins the whole composite key as the prefix — each
/// row stands alone, and prefix-seek is a fast lookup for the full key.
///
/// # Configuration
/// - 16KB blocks (pair payloads are 1-5KB after compression)
/// - LZ4 compression (text + float arrays)
/// - Bloom filter for fast composite-key lookups
/// - 32-byte prefix extractor (full composite key)
/// - 4MB write buffer
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn contrastive_pairs_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(16 * 1024);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    // 32-byte prefix = full composite key `[anchor][neg]`.
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(32));
    apply_write_buffer_limits(&mut opts, 4);
    opts.create_if_missing(true);
    opts
}

/// Options for `CF_CONTRASTIVE_BY_KIND` (Phase 3 secondary index).
///
/// Key layout: `[kind: u8][anchor: 16B][neg: 16B]` (33 bytes). A 1-byte
/// prefix extractor enables `prefix_iterator_cf(kind_byte)` to enumerate
/// every pair of a given anomaly kind.
///
/// # Configuration
/// - LZ4 compression (values are empty — compression cosmetic)
/// - Bloom filter for fast existence checks
/// - 1-byte prefix extractor (kind byte)
/// - 2MB write buffer
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn contrastive_by_kind_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    // 1-byte prefix = anomaly kind byte.
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(1));
    apply_write_buffer_limits(&mut opts, 2);
    opts.create_if_missing(true);
    opts
}

/// Options for `CF_CONTRASTIVE_BY_ANCHOR` (Phase 3 secondary index).
///
/// Key layout: anchor UUID (16 bytes). Value is a bincode-encoded
/// `Vec<Uuid>` of every negative mined against that anchor, deduplicated on
/// read (append-write to avoid a read-modify-write round-trip during
/// mining).
///
/// # Configuration
/// - LZ4 compression (UUID lists compress modestly)
/// - Bloom filter for fast anchor existence checks
/// - 16-byte prefix extractor (anchor UUID)
/// - 2MB write buffer
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn contrastive_by_anchor_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16));
    apply_write_buffer_limits(&mut opts, 2);
    opts.create_if_missing(true);
    opts
}

/// Options for `CF_TYPED_EDGE_RECORDS` (typed-edge training-data factory F1).
///
/// Key layout: `[source:16][target:16][edge_type:u8]` (33 bytes). Records are
/// typically 2-10 KB each (content + embedder scores + metadata joins).
///
/// # Configuration
/// - 16KB blocks (payloads are 2-10 KB after compression)
/// - LZ4 compression (text + float arrays)
/// - Bloom filter for fast composite-key lookups
/// - 33-byte prefix extractor (full composite key)
/// - 4MB write buffer
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn typed_edge_records_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(16 * 1024);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    // 33-byte prefix = full composite key `[source][target][edge_type]`.
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(33));
    apply_write_buffer_limits(&mut opts, 4);
    opts.create_if_missing(true);
    opts
}

/// Options for `CF_TYPED_EDGE_VALIDATIONS` (typed-edge training-data factory F4).
///
/// Key layout: `[source:16][target:16][edge_type:u8]` (33 bytes). Values are
/// small (rationale + verdict + metadata, ~200-500 B). Append-light write
/// pattern since validations are usually produced in bounded batches.
///
/// # Configuration
/// - LZ4 compression (rationale text compresses)
/// - Bloom filter for fast existence checks
/// - 33-byte prefix extractor (full composite key)
/// - 2MB write buffer
///
/// # FAIL FAST Policy
/// No fallback options — let RocksDB error on open if misconfigured.
pub fn typed_edge_validations_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(33));
    apply_write_buffer_limits(&mut opts, 2);
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
    apply_write_buffer_limits(&mut opts, 8); // up to 1MB values
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for source metadata storage (~100-500 bytes per entry).
///
/// # Configuration
/// - LZ4 compression (JSON data compresses well)
/// - Bloom filter for fast lookups
/// - Point lookups only by UUID
///
/// # Key Format
/// UUID (16 bytes) for fingerprint_id.
///
/// # Value Format
/// SourceMetadata serialized via JSON (~100-500 bytes).
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
    apply_write_buffer_limits(&mut opts, 2); // 100-500 byte values
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
    apply_write_buffer_limits(&mut opts, 2); // small index entries
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
    apply_write_buffer_limits(&mut opts, 2); // session data
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
    apply_write_buffer_limits(&mut opts, 4); // 10-25KB values
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

// =============================================================================
// ENTITY PROVENANCE CF OPTION BUILDER (Phase 3a Provenance)
// =============================================================================

/// Options for entity provenance storage (~200-2000 bytes per record).
///
/// # Configuration
/// - LZ4 compression (text excerpts compress well, ~50% reduction)
/// - Bloom filter for fast existence checks
/// - Prefix scan support for "all provenances of entity X" queries
///
/// # Key Format
/// `{canonical_id_bytes}_{memory_uuid_bytes}` (variable + 16 bytes).
/// Composite key enables both entity-based and memory-based queries.
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn entity_provenance_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    apply_write_buffer_limits(&mut opts, 2); // 200-2000 byte values
    opts.create_if_missing(true);
    opts
}

// =============================================================================
// AUDIT LOG CF OPTION BUILDERS (Phase 1.1 Provenance)
// =============================================================================

/// Options for audit log primary storage (~200-2000 bytes per record).
///
/// # Configuration
/// - LZ4 compression (JSON parameters and text fields compress well, ~50%)
/// - Bloom filter for fast existence checks
/// - Level compaction for append-heavy workload
/// - No prefix extractor (we use full-key range scans for time-based queries)
///
/// # Key Format
/// `{timestamp_nanos_be}_{uuid_bytes}` (24 bytes).
/// Big-endian timestamp ensures chronological ordering in RocksDB iteration.
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn audit_log_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    apply_write_buffer_limits(&mut opts, 4); // append-heavy
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for audit log by-target secondary index.
///
/// # Configuration
/// - LZ4 compression (key-value pairs compress modestly)
/// - Bloom filter for fast target existence checks
/// - 16-byte prefix extractor for UUID-based prefix scans
///
/// # Key Format
/// `{target_uuid_bytes}_{timestamp_nanos_be}` (24 bytes).
/// UUID prefix enables efficient "all records for target X" queries.
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn audit_by_target_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    apply_write_buffer_limits(&mut opts, 2); // secondary index
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

// =============================================================================
// LIFECYCLE PROVENANCE CF OPTION BUILDERS (Phase 4)
// =============================================================================

/// Options for permanent merge history storage (~500-5000 bytes per record).
///
/// # Configuration
/// - LZ4 compression (JSON fingerprint data compresses well, ~50%)
/// - Bloom filter for fast merged_id lookups
/// - Level compaction for append-heavy workload
///
/// # Key Format
/// `{merged_uuid_bytes}_{timestamp_nanos_be}` (24 bytes).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn merge_history_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    apply_write_buffer_limits(&mut opts, 2); // append, moderate volume
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Options for importance change history storage (~100-500 bytes per record).
///
/// # Configuration
/// - LZ4 compression (structured data compresses well)
/// - Bloom filter for fast memory_id lookups
/// - Level compaction for append-heavy workload
///
/// # Key Format
/// `{memory_uuid_bytes}_{timestamp_nanos_be}` (24 bytes).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn importance_history_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    apply_write_buffer_limits(&mut opts, 2); // append, moderate volume
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

// =============================================================================
// PHASE 5 PROVENANCE CF OPTION BUILDERS
// =============================================================================

/// Options for tool call index storage (Phase 5, item 5.12).
///
/// # Configuration
/// - LZ4 compression (UUID lists compress well)
/// - Bloom filter for fast tool_use_id lookups
/// - Point lookups by tool_use_id
///
/// # Key Format
/// UTF-8 tool_use_id bytes (variable length).
///
/// # Value Format
/// Vec<Uuid> serialized via bincode (~16-160 bytes per entry).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn tool_call_index_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    apply_write_buffer_limits(&mut opts, 2); // small values
    opts.create_if_missing(true);
    opts
}

/// Options for consolidation recommendations storage (Phase 5, item 5.14).
///
/// # Configuration
/// - LZ4 compression (structured data compresses well, ~50%)
/// - Bloom filter for fast recommendation_id lookups
/// - 16-byte prefix extractor for UUID keys
/// - Point lookups by recommendation UUID
///
/// # Key Format
/// UUID (16 bytes) for recommendation_id.
///
/// # Value Format
/// ConsolidationRecommendation serialized via bincode (~500-5000 bytes).
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn consolidation_recommendations_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(32); // 32MB hint for point lookups
    apply_write_buffer_limits(&mut opts, 2); // small values
    opts.create_if_missing(true);
    opts
}

/// Options for embedding version registry storage (~200-500 bytes per record).
///
/// # Configuration
/// - LZ4 compression (structured data compresses well)
/// - Bloom filter for fast fingerprint_id lookups
/// - 16-byte prefix extractor for UUID keys
/// - Optimized for point lookups
///
/// # FAIL FAST Policy
/// No fallback options - let RocksDB error on open if misconfigured.
pub fn embedding_registry_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16)); // UUID prefix
    opts.optimize_for_point_lookup(32); // 32MB hint
    apply_write_buffer_limits(&mut opts, 2); // 200-500 byte values
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
    apply_write_buffer_limits(&mut opts, 2); // 1-2KB values
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options - let RocksDB error on open if misconfigured
    opts
}

/// Get all teleological column family descriptors.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 29 `ColumnFamilyDescriptor`s for teleological storage.
pub fn get_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_FINGERPRINTS, fingerprint_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_TOPIC_PROFILES, topic_profile_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_E13_SPLADE_INVERTED,
            e13_splade_inverted_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_E6_SPARSE_INVERTED, e6_sparse_inverted_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_E1_MATRYOSHKA_128, e1_matryoshka_128_cf_options(cache)),
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
        // Phase 3a Provenance: Entity provenance CF
        ColumnFamilyDescriptor::new(CF_ENTITY_PROVENANCE, entity_provenance_cf_options(cache)),
        // Phase 1.1 Provenance: Audit log CFs (append-only)
        ColumnFamilyDescriptor::new(CF_AUDIT_LOG, audit_log_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_AUDIT_BY_TARGET, audit_by_target_cf_options(cache)),
        // Phase 4 Lifecycle Provenance: Permanent merge + importance history
        ColumnFamilyDescriptor::new(CF_MERGE_HISTORY, merge_history_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_IMPORTANCE_HISTORY, importance_history_cf_options(cache)),
        // Phase 5 Hook & Tool Call Provenance
        ColumnFamilyDescriptor::new(CF_TOOL_CALL_INDEX, tool_call_index_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_CONSOLIDATION_RECOMMENDATIONS,
            consolidation_recommendations_cf_options(cache),
        ),
        // Phase 6 Provenance: Embedding version registry
        ColumnFamilyDescriptor::new(CF_EMBEDDING_REGISTRY, embedding_registry_cf_options(cache)),
        // Custom weight profile persistence
        ColumnFamilyDescriptor::new(
            CF_CUSTOM_WEIGHT_PROFILES,
            custom_weight_profiles_cf_options(cache),
        ),
        // HNSW graph persistence for fast startup
        ColumnFamilyDescriptor::new(CF_HNSW_GRAPHS, hnsw_graphs_cf_options(cache)),
        // Training record export (export_training_corpus MCP tool)
        ColumnFamilyDescriptor::new(CF_TRAINING_RECORDS, training_records_cf_options(cache)),
        // Learning-as-UTL event storage
        ColumnFamilyDescriptor::new(CF_LEARNING_EVENTS, learning_events_cf_options(cache)),
        // UTL learner-state storage namespace
        ColumnFamilyDescriptor::new(CF_LEARNER_PROFILE, learner_profile_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_LEARNER_CONSTELLATIONS,
            learner_constellations_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_FINGERPRINTS_LEARNER,
            fingerprints_learner_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_LEARNER_M_PER_TRACE,
            learner_m_per_trace_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_LEARNER_STATE_HISTORY,
            learner_state_history_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_LEARNER_GOAL_STATES,
            learner_goal_states_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_LEARNER_RETRIEVAL_LOG,
            learner_retrieval_log_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_LEARNER_K_SLEEP, learner_k_sleep_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_GOAL_CENTROIDS, goal_centroids_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_LEARNER_DELTA_LOG, learner_delta_log_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_LEARNER_AUDIT, learner_audit_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_LEARNER_TRAINING_DATASETS,
            learner_training_datasets_cf_options(cache),
        ),
        // Phase 2: Constellation compiler (compile_constellation MCP tool)
        ColumnFamilyDescriptor::new(CF_CONSTELLATIONS, constellations_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_CONSTELLATION_BY_SELECTOR,
            constellation_by_selector_cf_options(cache),
        ),
        // Phase 3: Contrastive pair miner
        ColumnFamilyDescriptor::new(CF_CONTRASTIVE_PAIRS, contrastive_pairs_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_CONTRASTIVE_BY_KIND,
            contrastive_by_kind_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_CONTRASTIVE_BY_ANCHOR,
            contrastive_by_anchor_cf_options(cache),
        ),
        // Typed-edge training-data factory (F1/F4):
        // CF_TYPED_EDGE_RECORDS stores exported corpus rows;
        // CF_TYPED_EDGE_VALIDATIONS stores LLM verdicts on typed edges.
        ColumnFamilyDescriptor::new(CF_TYPED_EDGE_RECORDS, typed_edge_records_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_TYPED_EDGE_VALIDATIONS,
            typed_edge_validations_cf_options(cache),
        ),
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
/// Returns all teleological descriptors plus the 14 quantized embedder descriptors.
/// Use this when opening a database that needs both fingerprint and per-embedder storage.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 43 `ColumnFamilyDescriptor`s.
///
/// # Example
/// ```ignore
/// use rocksdb::Cache;
/// use context_graph_storage::teleological::get_all_teleological_cf_descriptors;
///
/// let cache = Cache::new_lru_cache(256 * 1024 * 1024); // 256MB
/// let descriptors = get_all_teleological_cf_descriptors(&cache);
/// assert_eq!(descriptors.len(), TELEOLOGICAL_CF_COUNT + QUANTIZED_EMBEDDER_CF_COUNT);
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
    apply_write_buffer_limits(&mut opts, 4); // 500-5000 byte values
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
    apply_write_buffer_limits(&mut opts, 4); // 6144-byte values
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
    apply_write_buffer_limits(&mut opts, 2); // small index
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
    apply_write_buffer_limits(&mut opts, 2); // small index
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
    apply_write_buffer_limits(&mut opts, 2); // small index
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
        ColumnFamilyDescriptor::new(
            CF_CODE_SIGNATURE_INDEX,
            code_signature_index_cf_options(cache),
        ),
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
    apply_write_buffer_limits(&mut opts, 4); // 5-15KB values
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
    apply_write_buffer_limits(&mut opts, 2); // small index
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
        ColumnFamilyDescriptor::new(
            CF_CAUSAL_RELATIONSHIPS,
            causal_relationships_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_CAUSAL_BY_SOURCE, causal_by_source_cf_options(cache)),
    ]
}

/// Get ALL column family descriptors (teleological + embedder + code + causal).
///
/// Returns 40 descriptors total: 20 teleological + 13 quantized embedder + 5 code + 2 causal.
///
/// # Arguments
/// * `cache` - Shared block cache (recommended: 256MB via `Cache::new_lru_cache`)
///
/// # Returns
/// Vector of 41 `ColumnFamilyDescriptor`s.
pub fn get_all_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    let mut descriptors = get_all_teleological_cf_descriptors(cache);
    descriptors.extend(get_code_cf_descriptors(cache));
    descriptors.extend(get_causal_cf_descriptors(cache));
    descriptors
}
