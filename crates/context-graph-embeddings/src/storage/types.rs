//! Quantized storage types for per-embedder HNSW indexing.
//!
//! These types support the Constitution's 5-stage retrieval pipeline, specifically:
//! - Stage 3: Multi-space reranking with RRF fusion across 13 embedders
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! - All deserialization errors panic with full context
//! - No silent fallbacks or default values
//! - Version mismatches are fatal (no migration support)
//!
//! # Storage Size Targets (Constitution)
//!
//! - Unquantized TeleologicalFingerprint: ~63KB
//! - Quantized StoredQuantizedFingerprint: ~17KB (63% reduction)
//! - This 17KB target achieved via per-embedder quantization

use crate::quantization::{QuantizationMethod, QuantizedEmbedding};
use crate::types::ModelId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// CONSTITUTION CONSTANTS
// =============================================================================
// From constitution.yaml `embeddings.storage_per_memory`

/// Expected size in bytes for a complete quantized fingerprint.
/// From Constitution: "~17KB (quantized) vs 46KB uncompressed"
///
/// Breakdown (approximate):
/// - PQ-8 embeddings (E1, E5, E7, E10): 4 × 8 bytes = 32 bytes
/// - Float8 embeddings (E2, E3, E4, E8, E11): 5 × (dim/4) bytes ≈ 2,740 bytes
/// - Binary embedding (E9): 10,000 bits / 8 = 1,250 bytes
/// - Sparse embeddings (E6, E13): ~10KB combined (variable)
/// - Token pruning (E12): ~2KB (50% of original)
/// - Metadata: ~1KB
///
/// Total: ~17KB
pub const EXPECTED_QUANTIZED_SIZE_BYTES: usize = 17_000;

/// Maximum allowed size for a quantized fingerprint.
/// Allow 50% overhead for sparse vectors with many non-zeros.
pub const MAX_QUANTIZED_SIZE_BYTES: usize = 25_000;

/// Minimum valid size - catches empty or corrupted fingerprints.
pub const MIN_QUANTIZED_SIZE_BYTES: usize = 5_000;

/// Number of embedders in the multi-array system.
pub const NUM_EMBEDDERS: usize = 13;

/// Storage format version. Bump when struct layout changes.
/// Version mismatches will panic (no migration support).
pub const STORAGE_VERSION: u8 = 1;

/// RRF constant k for multi-space fusion.
/// From Constitution: "RRF(d) = Σᵢ 1/(k + rankᵢ(d)) where k=60"
pub const RRF_K: f32 = 60.0;

// =============================================================================
// STORED QUANTIZED FINGERPRINT
// =============================================================================

/// Complete stored fingerprint with quantized embeddings.
///
/// This struct is used for STORAGE in layer1_primary (RocksDB/ScyllaDB).
/// The actual 13× HNSW indexes (layer2c) use `IndexEntry` for dequantized vectors.
///
/// # Storage Layout
/// Each embedder's quantized embedding is stored separately for:
/// 1. Per-embedder HNSW indexing (requires dequantization)
/// 2. Lazy loading (only fetch needed embedders)
/// 3. Independent quantization per embedder
///
/// # Size Target
/// ~17KB per fingerprint (Constitution requirement)
///
/// # Difference from TeleologicalFingerprint
/// - `TeleologicalFingerprint` (in context-graph-core): ~63KB UNQUANTIZED, includes:
///   - `SemanticFingerprint` with raw f32 arrays
///   - `JohariFingerprint` per-embedder classification
///   - `PurposeSnapshot` evolution history
///
/// - `StoredQuantizedFingerprint` (this type): ~17KB QUANTIZED for storage
///   - Uses `QuantizedEmbedding` (compressed bytes)
///   - Johari summarized to 4 quadrant weights
///   - No evolution history (kept in TimescaleDB temporal store)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredQuantizedFingerprint {
    /// UUID of the fingerprint (primary key).
    pub id: Uuid,

    /// Storage format version (for future migration detection).
    pub version: u8,

    /// Per-embedder quantized embeddings.
    /// Key: embedder index (0-12)
    /// Value: Quantized embedding with method-specific metadata
    ///
    /// # Invariant
    /// All 13 embedders MUST be present. Missing embedder = panic on load.
    pub embeddings: HashMap<u8, QuantizedEmbedding>,

    /// 13D purpose vector (NOT quantized - only 52 bytes).
    /// Each dimension = alignment of that embedder's output to North Star.
    /// From Constitution: "PV = [A(E1,V), A(E2,V), ..., A(E13,V)]"
    pub purpose_vector: [f32; 13],

    /// Aggregate alignment to North Star (theta).
    /// Pre-computed from purpose_vector for fast filtering in Stage 4.
    pub theta_to_north_star: f32,

    /// Johari quadrant weights [Open, Hidden, Blind, Unknown].
    /// Aggregated from per-embedder JohariFingerprint.
    /// Used for fast filtering without loading full fingerprint.
    pub johari_quadrants: [f32; 4],

    /// Dominant Johari quadrant index (0=Open, 1=Hidden, 2=Blind, 3=Unknown).
    /// Pre-computed for fast classification queries.
    pub dominant_quadrant: u8,

    /// Johari confidence score [0.0, 1.0].
    /// Higher = more certain about quadrant classification.
    pub johari_confidence: f32,

    /// SHA-256 content hash (32 bytes).
    /// Used for deduplication and integrity verification.
    pub content_hash: [u8; 32],

    /// Creation timestamp (Unix millis since epoch).
    pub created_at_ms: i64,

    /// Last update timestamp (Unix millis since epoch).
    pub last_updated_ms: i64,

    /// Access count for LRU/importance scoring.
    pub access_count: u64,

    /// Soft-delete flag.
    /// True = marked for deletion but recoverable (30-day window per Constitution).
    pub deleted: bool,
}

impl StoredQuantizedFingerprint {
    /// Create a new StoredQuantizedFingerprint.
    ///
    /// # Arguments
    /// * `id` - UUID for this fingerprint
    /// * `embeddings` - HashMap of quantized embeddings (must have all 13)
    /// * `purpose_vector` - 13D alignment signature
    /// * `johari_quadrants` - Aggregated Johari weights
    /// * `content_hash` - SHA-256 of source content
    ///
    /// # Panics
    /// Panics if `embeddings` doesn't contain exactly 13 entries.
    #[must_use]
    pub fn new(
        id: Uuid,
        embeddings: HashMap<u8, QuantizedEmbedding>,
        purpose_vector: [f32; 13],
        johari_quadrants: [f32; 4],
        content_hash: [u8; 32],
    ) -> Self {
        // FAIL FAST: All 13 embedders required
        if embeddings.len() != NUM_EMBEDDERS {
            panic!(
                "CONSTRUCTION ERROR: StoredQuantizedFingerprint requires exactly {} embeddings, got {}. \
                 Missing embedder indices: {:?}. \
                 This indicates incomplete fingerprint generation.",
                NUM_EMBEDDERS,
                embeddings.len(),
                (0..13).filter(|i| !embeddings.contains_key(&(*i as u8))).collect::<Vec<_>>()
            );
        }

        // Verify all indices are valid (0-12)
        for idx in embeddings.keys() {
            if *idx >= NUM_EMBEDDERS as u8 {
                panic!(
                    "CONSTRUCTION ERROR: Invalid embedder index {}. Valid range: 0-12. \
                     This indicates embedding pipeline bug.",
                    idx
                );
            }
        }

        let theta_to_north_star = purpose_vector.iter().sum::<f32>() / 13.0;
        let (dominant_quadrant, johari_confidence) =
            Self::compute_dominant_quadrant(&johari_quadrants);
        let now = chrono::Utc::now().timestamp_millis();

        Self {
            id,
            version: STORAGE_VERSION,
            embeddings,
            purpose_vector,
            theta_to_north_star,
            johari_quadrants,
            dominant_quadrant,
            johari_confidence,
            content_hash,
            created_at_ms: now,
            last_updated_ms: now,
            access_count: 0,
            deleted: false,
        }
    }

    /// Compute dominant quadrant and confidence from weights.
    fn compute_dominant_quadrant(quadrants: &[f32; 4]) -> (u8, f32) {
        let total: f32 = quadrants.iter().sum();
        if total < f32::EPSILON {
            return (0, 0.0); // Default to Open with zero confidence
        }

        let mut max_idx = 0u8;
        let mut max_val = quadrants[0];
        for (i, &v) in quadrants.iter().enumerate().skip(1) {
            if v > max_val {
                max_val = v;
                max_idx = i as u8;
            }
        }

        let confidence = max_val / total;
        (max_idx, confidence)
    }

    /// Compute total storage size in bytes (serialized).
    ///
    /// # Returns
    /// Estimated serialized size. Actual size may vary slightly due to encoding.
    #[must_use]
    pub fn estimated_size_bytes(&self) -> usize {
        let mut size = 0usize;

        // Fixed fields
        size += 16; // id (UUID)
        size += 1; // version
        size += 52; // purpose_vector (13 × 4 bytes)
        size += 4; // theta_to_north_star
        size += 16; // johari_quadrants (4 × 4 bytes)
        size += 1; // dominant_quadrant
        size += 4; // johari_confidence
        size += 32; // content_hash
        size += 8; // created_at_ms
        size += 8; // last_updated_ms
        size += 8; // access_count
        size += 1; // deleted

        // Variable fields: embeddings
        for qe in self.embeddings.values() {
            size += 1; // method (enum variant)
            size += 8; // original_dim
            size += qe.data.len(); // compressed data
            size += 32; // metadata (approximate)
        }

        size
    }

    /// Get quantized embedding for a specific embedder.
    ///
    /// # Arguments
    /// * `embedder_idx` - Embedder index (0-12)
    ///
    /// # Panics
    /// Panics if embedder_idx is out of range or embedding is missing.
    #[must_use]
    pub fn get_embedding(&self, embedder_idx: u8) -> &QuantizedEmbedding {
        self.embeddings.get(&embedder_idx).unwrap_or_else(|| {
            panic!(
                "STORAGE ERROR: Missing embedding for embedder {}. \
                 Fingerprint ID: {}. Available embedders: {:?}. \
                 This indicates corrupted fingerprint or storage bug.",
                embedder_idx,
                self.id,
                self.embeddings.keys().collect::<Vec<_>>()
            );
        })
    }

    /// Check if all embeddings use correct quantization methods.
    ///
    /// # Returns
    /// `true` if all embeddings match their Constitution-assigned methods.
    #[must_use]
    pub fn validate_quantization_methods(&self) -> bool {
        for (idx, qe) in &self.embeddings {
            if let Ok(model_id) = ModelId::try_from(*idx) {
                let expected = QuantizationMethod::for_model_id(model_id);
                if qe.method != expected {
                    return false;
                }
            }
        }
        true
    }
}

// =============================================================================
// INDEX ENTRY (For HNSW Indexes)
// =============================================================================

/// Entry in a per-embedder HNSW index.
///
/// This type is used for INDEXING in layer2c_per_embedder (13× HNSW).
/// Contains DEQUANTIZED vectors for similarity search.
///
/// # Usage in 5-Stage Pipeline
/// Stage 3 (Multi-space rerank): Query each HNSW index → get IndexEntry results → RRF fusion
///
/// # Memory Consideration
/// IndexEntry holds dequantized f32 vectors, so it's memory-intensive.
/// Don't hold large collections in memory - use for query-time only.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// UUID of the fingerprint this entry belongs to.
    pub id: Uuid,

    /// Embedder index this entry is from (0-12).
    pub embedder_idx: u8,

    /// Dequantized embedding vector (full precision f32).
    /// Length depends on embedder:
    /// - E1: 1024, E2-E4: 512 each, E5: 768, E6: sparse, E7: 1536
    /// - E8: 384, E9: 1024 (from 10K binary), E10: 768, E11: 384
    /// - E12: 128 per token, E13: sparse
    pub vector: Vec<f32>,

    /// Precomputed L2 norm for fast cosine similarity.
    /// norm = sqrt(sum(x_i^2))
    pub norm: f32,
}

impl IndexEntry {
    /// Create index entry with precomputed norm.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    /// * `embedder_idx` - Which embedder (0-12)
    /// * `vector` - Dequantized embedding vector
    #[must_use]
    pub fn new(id: Uuid, embedder_idx: u8, vector: Vec<f32>) -> Self {
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        Self {
            id,
            embedder_idx,
            vector,
            norm,
        }
    }

    /// Get normalized vector for cosine similarity.
    ///
    /// # Returns
    /// Unit vector (L2 norm = 1.0), or zero vector if norm is ~0.
    #[must_use]
    pub fn normalized(&self) -> Vec<f32> {
        if self.norm > 1e-10 {
            self.vector.iter().map(|x| x / self.norm).collect()
        } else {
            vec![0.0; self.vector.len()]
        }
    }

    /// Compute cosine similarity with another vector.
    ///
    /// # Arguments
    /// * `other` - Query vector (must have same length)
    ///
    /// # Returns
    /// Cosine similarity in range [-1.0, 1.0]
    ///
    /// # Panics
    /// Panics if vector lengths don't match.
    #[must_use]
    pub fn cosine_similarity(&self, other: &[f32]) -> f32 {
        if self.vector.len() != other.len() {
            panic!(
                "SIMILARITY ERROR: Vector length mismatch. Entry has {} dims, query has {} dims. \
                 Embedder index: {}. This indicates dimension mismatch bug.",
                self.vector.len(),
                other.len(),
                self.embedder_idx
            );
        }

        let dot: f32 = self
            .vector
            .iter()
            .zip(other.iter())
            .map(|(a, b)| a * b)
            .sum();
        let other_norm: f32 = other.iter().map(|x| x * x).sum::<f32>().sqrt();

        if self.norm > 1e-10 && other_norm > 1e-10 {
            dot / (self.norm * other_norm)
        } else {
            0.0
        }
    }
}

// =============================================================================
// QUERY RESULTS
// =============================================================================

/// Result from per-embedder index search (single space).
///
/// Used in Stage 3 before RRF fusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderQueryResult {
    /// Fingerprint UUID.
    pub id: Uuid,

    /// Embedder index (0-12).
    pub embedder_idx: u8,

    /// Similarity score [0.0, 1.0] for cosine, [-1.0, 1.0] for dot product.
    pub similarity: f32,

    /// Distance (metric-specific). For cosine: 1 - similarity.
    pub distance: f32,

    /// Rank in this embedder's result list (0-indexed).
    pub rank: usize,
}

impl EmbedderQueryResult {
    /// Create from similarity score.
    #[must_use]
    pub fn from_similarity(id: Uuid, embedder_idx: u8, similarity: f32, rank: usize) -> Self {
        Self {
            id,
            embedder_idx,
            similarity,
            distance: 1.0 - similarity.clamp(-1.0, 1.0),
            rank,
        }
    }

    /// Compute RRF contribution for this result.
    /// Formula: 1 / (k + rank) where k = 60
    #[must_use]
    pub fn rrf_contribution(&self) -> f32 {
        1.0 / (RRF_K + self.rank as f32)
    }
}

/// Aggregated result from multi-space retrieval (after RRF fusion).
///
/// This is the final result type after Stage 3 multi-space reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSpaceQueryResult {
    /// Fingerprint UUID.
    pub id: Uuid,

    /// Per-embedder similarities (13 values).
    /// NaN if embedder wasn't searched (e.g., sparse-only query).
    pub embedder_similarities: [f32; 13],

    /// RRF fused score from multi-space retrieval.
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d)) where k=60
    pub rrf_score: f32,

    /// Weighted average similarity (alternative to RRF).
    /// Uses Constitution-defined weights per query type.
    pub weighted_similarity: f32,

    /// Purpose alignment score (from StoredQuantizedFingerprint.theta_to_north_star).
    /// Used in Stage 4 teleological filtering.
    pub purpose_alignment: f32,

    /// Number of embedders that contributed to this result.
    /// Less than 13 if some embedders weren't searched.
    pub embedder_count: usize,
}

impl MultiSpaceQueryResult {
    /// Create from individual embedder results.
    ///
    /// # Arguments
    /// * `id` - Fingerprint UUID
    /// * `results` - Per-embedder query results
    /// * `purpose_alignment` - From stored fingerprint
    ///
    /// # Panics
    /// Panics if results is empty.
    #[must_use]
    pub fn from_embedder_results(
        id: Uuid,
        results: &[EmbedderQueryResult],
        purpose_alignment: f32,
    ) -> Self {
        if results.is_empty() {
            panic!(
                "AGGREGATION ERROR: Cannot create MultiSpaceQueryResult from empty results. \
                 Fingerprint ID: {}. This indicates query execution bug.",
                id
            );
        }

        let mut embedder_similarities = [f32::NAN; 13];
        let mut rrf_score = 0.0f32;
        let mut weighted_sum = 0.0f32;
        let mut weight_total = 0.0f32;

        for result in results {
            let idx = result.embedder_idx as usize;
            if idx < 13 {
                embedder_similarities[idx] = result.similarity;
                rrf_score += result.rrf_contribution();
                weighted_sum += result.similarity;
                weight_total += 1.0;
            }
        }

        let weighted_similarity = if weight_total > 0.0 {
            weighted_sum / weight_total
        } else {
            0.0
        };

        Self {
            id,
            embedder_similarities,
            rrf_score,
            weighted_similarity,
            purpose_alignment,
            embedder_count: results.len(),
        }
    }

    /// Check if this result passes Stage 4 teleological filter.
    ///
    /// # Arguments
    /// * `min_alignment` - Minimum acceptable alignment (default: 0.55 from Constitution)
    #[must_use]
    pub fn passes_alignment_filter(&self, min_alignment: f32) -> bool {
        self.purpose_alignment >= min_alignment
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantization::{QuantizationMetadata, QuantizationMethod};

    /// Create a valid HashMap of dummy quantized embeddings for testing.
    fn create_test_embeddings() -> HashMap<u8, QuantizedEmbedding> {
        let mut map = HashMap::new();
        for i in 0..13u8 {
            let (method, dim, data_len) = match i {
                0 | 4 | 6 | 9 => (QuantizationMethod::PQ8, 1024, 8),
                1 | 2 | 3 | 7 | 10 => (QuantizationMethod::Float8E4M3, 512, 512),
                8 => (QuantizationMethod::Binary, 10000, 1250),
                5 | 12 => (QuantizationMethod::SparseNative, 30522, 100),
                11 => (QuantizationMethod::TokenPruning, 128, 64),
                _ => unreachable!(),
            };

            map.insert(
                i,
                QuantizedEmbedding {
                    method,
                    original_dim: dim,
                    data: vec![0u8; data_len],
                    metadata: match method {
                        QuantizationMethod::PQ8 => QuantizationMetadata::PQ8 {
                            codebook_id: i as u32,
                            num_subvectors: 8,
                        },
                        QuantizationMethod::Float8E4M3 => QuantizationMetadata::Float8 {
                            scale: 1.0,
                            bias: 0.0,
                        },
                        QuantizationMethod::Binary => {
                            QuantizationMetadata::Binary { threshold: 0.0 }
                        }
                        QuantizationMethod::SparseNative => QuantizationMetadata::Sparse {
                            vocab_size: 30522,
                            nnz: 50,
                        },
                        QuantizationMethod::TokenPruning => QuantizationMetadata::TokenPruning {
                            original_tokens: 128,
                            kept_tokens: 64,
                            threshold: 0.5,
                        },
                    },
                },
            );
        }
        map
    }

    #[test]
    fn test_stored_fingerprint_creation() {
        let id = Uuid::new_v4();
        let embeddings = create_test_embeddings();
        let purpose_vector = [0.5f32; 13];
        let johari_quadrants = [0.25f32; 4];
        let content_hash = [0u8; 32];

        let fp = StoredQuantizedFingerprint::new(
            id,
            embeddings,
            purpose_vector,
            johari_quadrants,
            content_hash,
        );

        assert_eq!(fp.id, id);
        assert_eq!(fp.version, STORAGE_VERSION);
        assert_eq!(fp.embeddings.len(), 13);
        assert_eq!(fp.purpose_vector.len(), 13);
        assert!((fp.theta_to_north_star - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_stored_fingerprint_missing_embeddings() {
        let mut embeddings = create_test_embeddings();
        embeddings.remove(&5); // Remove one embedder

        StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            [0.5f32; 13],
            [0.25f32; 4],
            [0u8; 32],
        );
    }

    #[test]
    fn test_index_entry_normalized() {
        let entry = IndexEntry::new(
            Uuid::new_v4(),
            0,
            vec![3.0, 4.0], // 3-4-5 right triangle
        );

        assert!((entry.norm - 5.0).abs() < f32::EPSILON);

        let normalized = entry.normalized();
        assert!((normalized[0] - 0.6).abs() < f32::EPSILON);
        assert!((normalized[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_index_entry_cosine_similarity() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 0.0]);

        // Same direction
        let sim = entry.cosine_similarity(&[1.0, 0.0]);
        assert!((sim - 1.0).abs() < 1e-6);

        // Opposite direction
        let sim = entry.cosine_similarity(&[-1.0, 0.0]);
        assert!((sim - (-1.0)).abs() < 1e-6);

        // Perpendicular
        let sim = entry.cosine_similarity(&[0.0, 1.0]);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_embedder_query_result_rrf() {
        let result = EmbedderQueryResult::from_similarity(
            Uuid::new_v4(),
            0,
            0.9,
            0, // rank 0
        );

        // RRF contribution at rank 0: 1/(60+0) = 1/60
        let expected = 1.0 / 60.0;
        assert!((result.rrf_contribution() - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_multi_space_result_aggregation() {
        let id = Uuid::new_v4();
        let results = vec![
            EmbedderQueryResult::from_similarity(id, 0, 0.9, 0),
            EmbedderQueryResult::from_similarity(id, 1, 0.8, 1),
            EmbedderQueryResult::from_similarity(id, 2, 0.7, 2),
        ];

        let multi = MultiSpaceQueryResult::from_embedder_results(id, &results, 0.75);

        assert_eq!(multi.id, id);
        assert_eq!(multi.embedder_count, 3);
        assert!((multi.embedder_similarities[0] - 0.9).abs() < f32::EPSILON);
        assert!(multi.embedder_similarities[3].is_nan()); // Not searched
        assert!((multi.purpose_alignment - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_alignment_filter() {
        let multi = MultiSpaceQueryResult {
            id: Uuid::new_v4(),
            embedder_similarities: [0.5f32; 13],
            rrf_score: 0.1,
            weighted_similarity: 0.5,
            purpose_alignment: 0.60,
            embedder_count: 13,
        };

        // Constitution default: 0.55
        assert!(multi.passes_alignment_filter(0.55));
        assert!(!multi.passes_alignment_filter(0.65));
    }

    #[test]
    fn test_serde_roundtrip() {
        let result = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 5, 0.85, 10);
        let json = serde_json::to_string(&result).expect("serialize");
        let restored: EmbedderQueryResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(result.id, restored.id);
        assert_eq!(result.embedder_idx, restored.embedder_idx);
    }

    #[test]
    fn test_estimated_size() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 13],
            [0.25f32; 4],
            [0u8; 32],
        );

        let size = fp.estimated_size_bytes();
        // Should be in reasonable range
        assert!(size > 1000, "Size too small: {}", size);
        assert!(size < MAX_QUANTIZED_SIZE_BYTES, "Size too large: {}", size);
    }

    // =============================================================================
    // EDGE CASE TESTS
    // =============================================================================

    /// Edge Case 1: Creating fingerprint with only 12 embedders MUST PANIC
    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_edge_case_missing_embedder_panics() {
        let mut embeddings = create_test_embeddings();
        embeddings.remove(&5); // Remove E6

        // This MUST panic with "CONSTRUCTION ERROR"
        StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            [0.5f32; 13],
            [0.25f32; 4],
            [0u8; 32],
        );
    }

    /// Edge Case 2: Zero-norm vector in IndexEntry
    #[test]
    fn test_edge_case_zero_norm_vector() {
        // Test: Creating index entry with zero vector
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![0.0, 0.0, 0.0]);

        // Normalized should return zero vector (not NaN/Inf)
        let normalized = entry.normalized();
        assert!(
            normalized.iter().all(|&x| x == 0.0),
            "Expected zero vector for normalized"
        );
        assert_eq!(normalized.len(), 3);

        // Cosine similarity with zero vector should be 0.0
        let sim = entry.cosine_similarity(&[1.0, 0.0, 0.0]);
        assert_eq!(sim, 0.0, "Expected 0.0 similarity for zero vector");

        // Verify norm is zero
        assert!(entry.norm.abs() < f32::EPSILON, "Expected zero norm");
    }

    /// Edge Case 3: RRF contribution at high rank
    #[test]
    fn test_edge_case_rrf_high_rank() {
        // Test: RRF contribution diminishes at high ranks
        let result_rank_0 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 0);
        let result_rank_1000 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 1000);

        let rrf_0 = result_rank_0.rrf_contribution(); // 1/60 = 0.0167
        let rrf_1000 = result_rank_1000.rrf_contribution(); // 1/1060 = 0.00094

        // Verify actual values
        let expected_rrf_0 = 1.0 / 60.0;
        let expected_rrf_1000 = 1.0 / 1060.0;

        assert!(
            (rrf_0 - expected_rrf_0).abs() < f32::EPSILON,
            "Expected rrf_0={}, got {}",
            expected_rrf_0,
            rrf_0
        );
        assert!(
            (rrf_1000 - expected_rrf_1000).abs() < f32::EPSILON,
            "Expected rrf_1000={}, got {}",
            expected_rrf_1000,
            rrf_1000
        );

        // Rank 0 contributes 10x+ more than rank 1000
        assert!(
            rrf_0 > rrf_1000 * 10.0,
            "Rank 0 ({}) should be >10x rank 1000 ({})",
            rrf_0,
            rrf_1000
        );
    }

    /// Test that invalid embedder index in embeddings map panics
    #[test]
    #[should_panic(expected = "CONSTRUCTION ERROR")]
    fn test_invalid_embedder_index() {
        let mut embeddings = create_test_embeddings();
        // Remove valid key 12 and add invalid key 13
        embeddings.remove(&12);
        embeddings.insert(
            13,
            QuantizedEmbedding {
                method: QuantizationMethod::SparseNative,
                original_dim: 30522,
                data: vec![0u8; 100],
                metadata: QuantizationMetadata::Sparse {
                    vocab_size: 30522,
                    nnz: 50,
                },
            },
        );

        StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            embeddings,
            [0.5f32; 13],
            [0.25f32; 4],
            [0u8; 32],
        );
    }

    /// Test get_embedding panics for missing index
    #[test]
    #[should_panic(expected = "STORAGE ERROR")]
    fn test_get_embedding_missing_index() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 13],
            [0.25f32; 4],
            [0u8; 32],
        );

        // This should panic because 15 is not a valid index
        fp.get_embedding(15);
    }

    /// Test cosine similarity panics on dimension mismatch
    #[test]
    #[should_panic(expected = "SIMILARITY ERROR")]
    fn test_cosine_similarity_dimension_mismatch() {
        let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1.0, 0.0, 0.0]);

        // This should panic because query has 2 dims, entry has 3
        entry.cosine_similarity(&[1.0, 0.0]);
    }

    /// Test MultiSpaceQueryResult panics on empty results
    #[test]
    #[should_panic(expected = "AGGREGATION ERROR")]
    fn test_multi_space_empty_results() {
        MultiSpaceQueryResult::from_embedder_results(
            Uuid::new_v4(),
            &[], // Empty results
            0.75,
        );
    }

    /// Test validate_quantization_methods
    #[test]
    fn test_validate_quantization_methods() {
        let fp = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 13],
            [0.25f32; 4],
            [0u8; 32],
        );

        // Our test embeddings use the correct methods per Constitution
        assert!(
            fp.validate_quantization_methods(),
            "Test embeddings should use correct quantization methods"
        );
    }

    /// Test dominant quadrant calculation
    #[test]
    fn test_dominant_quadrant_calculation() {
        // Open quadrant dominant
        let fp1 = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 13],
            [0.6, 0.2, 0.1, 0.1], // Open=0.6 is dominant
            [0u8; 32],
        );
        assert_eq!(fp1.dominant_quadrant, 0); // Open
        assert!((fp1.johari_confidence - 0.6).abs() < f32::EPSILON);

        // Unknown quadrant dominant
        let fp2 = StoredQuantizedFingerprint::new(
            Uuid::new_v4(),
            create_test_embeddings(),
            [0.5f32; 13],
            [0.1, 0.1, 0.2, 0.6], // Unknown=0.6 is dominant
            [0u8; 32],
        );
        assert_eq!(fp2.dominant_quadrant, 3); // Unknown
    }
}
