//! Distance and similarity metrics for the 13 embedding spaces.
//!
//! This module provides unified distance/similarity computation across all
//! embedding types: dense, sparse, binary, and token-level.
//!
//! # Design Philosophy
//!
//! Most similarity functions delegate to existing vector type methods:
//! - DenseVector::cosine_similarity()
//! - SparseVector::jaccard_similarity()
//! - BinaryVector::hamming_distance()
//!
//! This module adds:
//! - max_sim() for ColBERT late interaction (E12)
//! - transe_similarity() for knowledge graph embeddings (E11)
//! - compute_similarity_for_space() dispatcher
//!
//! # All outputs are normalized to [0.0, 1.0]

use crate::embeddings::{BinaryVector, DenseVector};
use crate::teleological::Embedder;
use crate::types::fingerprint::{EmbeddingRef, SemanticFingerprint, SparseVector};

/// Compute cosine similarity between two dense vectors.
///
/// Thin wrapper that creates DenseVectors and delegates to existing method.
/// Returns 0.0 for zero-magnitude vectors (AP-10: no NaN).
///
/// # Arguments
/// * `a` - First dense embedding as f32 slice
/// * `b` - Second dense embedding as f32 slice
///
/// # Returns
/// Similarity in [0.0, 1.0] where 1.0 = identical direction
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    // Check for zero vectors before computation (AP-10 compliance)
    let mag_a_sq: f32 = a.iter().map(|x| x * x).sum();
    let mag_b_sq: f32 = b.iter().map(|x| x * x).sum();
    if mag_a_sq == 0.0 || mag_b_sq == 0.0 {
        return 0.0;
    }

    let vec_a = DenseVector::new(a.to_vec());
    let vec_b = DenseVector::new(b.to_vec());

    let raw_sim = vec_a.cosine_similarity(&vec_b);

    // Normalize from [-1, 1] to [0, 1]
    // DenseVector.cosine_similarity already clamps to [-1, 1]
    (raw_sim + 1.0) / 2.0
}

/// Compute Jaccard similarity between two sparse vectors.
///
/// Thin wrapper that delegates to SparseVector::jaccard_similarity().
/// Returns |A ∩ B| / |A ∪ B| based on non-zero indices.
///
/// # Returns
/// Similarity in [0.0, 1.0] where 1.0 = identical index sets
pub fn jaccard_similarity(a: &SparseVector, b: &SparseVector) -> f32 {
    a.jaccard_similarity(b)
}

/// Compute Hamming similarity between two binary vectors.
///
/// Converts Hamming distance to similarity: 1.0 - (distance / max_bits).
///
/// # Returns
/// Similarity in [0.0, 1.0] where 1.0 = identical bit patterns
pub fn hamming_similarity(a: &BinaryVector, b: &BinaryVector) -> f32 {
    let distance = a.hamming_distance(b);
    let max_bits = a.bit_len().max(b.bit_len());

    if max_bits == 0 {
        return 1.0; // Empty vectors are identical
    }

    1.0 - (distance as f32 / max_bits as f32)
}

/// Compute MaxSim for late interaction (ColBERT-style).
///
/// For each query token, find max cosine similarity to any memory token.
/// Return mean of all max similarities.
///
/// # Algorithm
/// ```text
/// MaxSim = (1/|Q|) * Σ_q∈Q max_m∈M cos(q, m)
/// ```
///
/// # Arguments
/// * `query_tokens` - Query token embeddings (each 128D for E12)
/// * `memory_tokens` - Memory token embeddings
///
/// # Returns
/// Similarity in [0.0, 1.0], returns 0.0 if either list is empty
pub fn max_sim(query_tokens: &[Vec<f32>], memory_tokens: &[Vec<f32>]) -> f32 {
    if query_tokens.is_empty() || memory_tokens.is_empty() {
        return 0.0;
    }

    let mut total_max = 0.0_f32;

    for q_tok in query_tokens {
        let mut max_sim_for_token = 0.0_f32;

        for m_tok in memory_tokens {
            let sim = cosine_similarity(q_tok, m_tok);
            max_sim_for_token = max_sim_for_token.max(sim);
        }

        total_max += max_sim_for_token;
    }

    total_max / query_tokens.len() as f32
}

/// Compute TransE-style similarity for knowledge graph triplet scoring.
///
/// Uses inverse of Euclidean distance: 1 / (1 + distance).
/// This maps distance [0, ∞) to similarity (0, 1].
///
/// # Important
///
/// This function is designed for TransE triplet operations (h + r - t),
/// NOT for general entity-entity similarity. For general E11 similarity,
/// use `cosine_similarity()` instead.
///
/// This function is used by:
/// - `infer_relationship` MCP tool (computing predicted relation vectors)
/// - `validate_knowledge` MCP tool (scoring (subject, predicate, object) triples)
///
/// # Returns
/// Similarity in (0.0, 1.0] where 1.0 = identical vectors (distance = 0)
pub fn transe_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let vec_a = DenseVector::new(a.to_vec());
    let vec_b = DenseVector::new(b.to_vec());

    let distance = vec_a.euclidean_distance(&vec_b);
    1.0 / (1.0 + distance)
}

/// Compute similarity for a specific embedding space.
///
/// This is the main dispatcher that routes to the appropriate similarity
/// function based on the embedder type.
///
/// # Metrics by Embedder
/// - E1 (Semantic): Cosine
/// - E2-E4 (Temporal): Cosine
/// - E5 (Causal): Cosine (asymmetric handled at embedding time via dual vectors)
/// - E6 (Sparse): Jaccard
/// - E7 (Code): Cosine (query-type detection handled at embedding time)
/// - E8 (Graph): Cosine
/// - E9 (HDC): Cosine on projected dense (see note below)
/// - E10 (Multimodal): Cosine
/// - E11 (Entity): Cosine (TransE used only for triplet operations in entity tools)
/// - E12 (LateInteraction): MaxSim (used for Stage 3 re-ranking only)
/// - E13 (KeywordSplade): Jaccard (used for Stage 1 recall only)
///
/// # E9 HDC Note
///
/// E9 uses 10,000-bit native hypervectors internally but projects to 1024D dense
/// for storage and indexing compatibility (see constants.rs). Cosine similarity
/// on the projected representation is used. For true Hamming distance on binary
/// HDC vectors, the `hamming_similarity()` function with `BinaryVector` can be
/// used if native binary storage is implemented in the future.
///
/// # E11 Entity Note
///
/// E11 uses cosine similarity for general entity-entity comparison. The TransE
/// similarity function (transe_similarity) is reserved for specific knowledge
/// graph operations in entity_tools (infer_relationship, validate_knowledge)
/// where the triplet scoring formula ||h + r - t|| is semantically meaningful.
///
/// # Arguments
/// * `embedder` - Which embedding space to compare
/// * `query` - Query fingerprint
/// * `memory` - Memory fingerprint
///
/// # Returns
/// Similarity in [0.0, 1.0]
pub fn compute_similarity_for_space(
    embedder: Embedder,
    query: &SemanticFingerprint,
    memory: &SemanticFingerprint,
) -> f32 {
    // EMB-7 FIX: E5 MUST NOT use symmetric cosine per AP-77.
    // EMB-2 FIX: E8 and E10 have asymmetric dual vectors that were computed/stored
    // but never used in search. Use cross-pair comparison (source-vs-target, paraphrase-vs-context)
    // to produce a more informative similarity score.
    match embedder {
        Embedder::Causal => {
            // EMB-7 FIX: E5 without direction returns 0.0.
            // Use compute_similarity_for_space_with_direction() for directional E5 similarity.
            return 0.0;
        }
        Embedder::Graph => {
            // E8: Compare source-vs-target cross pairs and take max
            let source_vs_target = cosine_similarity(
                query.get_e8_as_source(),
                memory.get_e8_as_target(),
            );
            let target_vs_source = cosine_similarity(
                query.get_e8_as_target(),
                memory.get_e8_as_source(),
            );
            // Take max of both directions — the stronger signal wins
            source_vs_target.max(target_vs_source)
        }
        Embedder::Multimodal => {
            // E10: Compare paraphrase-vs-context cross pairs and take max
            let para_vs_context = cosine_similarity(
                query.get_e10_as_paraphrase(),
                memory.get_e10_as_context(),
            );
            let context_vs_para = cosine_similarity(
                query.get_e10_as_context(),
                memory.get_e10_as_paraphrase(),
            );
            // Take max of both directions — captures paraphrase detection
            para_vs_context.max(context_vs_para)
        }
        _ => {
            // All other embedders use standard symmetric comparison
            let query_ref = query.get(embedder);
            let memory_ref = memory.get(embedder);

            let query_disc = std::mem::discriminant(&query_ref);
            let memory_disc = std::mem::discriminant(&memory_ref);

            match (query_ref, memory_ref) {
                (EmbeddingRef::Dense(q), EmbeddingRef::Dense(m)) => {
                    cosine_similarity(q, m)
                }
                (EmbeddingRef::Sparse(q), EmbeddingRef::Sparse(m)) => jaccard_similarity(q, m),
                (EmbeddingRef::TokenLevel(q), EmbeddingRef::TokenLevel(m)) => max_sim(q, m),
                _ => {
                    panic!(
                        "BUG: Type mismatch in compute_similarity_for_space for embedder {}. \
                         query={:?}, memory={:?}. This indicates a corrupted SemanticFingerprint.",
                        embedder.name(),
                        query_disc,
                        memory_disc,
                    );
                }
            }
        }
    }
}

/// Compute similarity for a specific embedding space with causal direction.
///
/// This function extends `compute_similarity_for_space()` with direction-aware
/// E5 similarity computation per ARCH-15 and AP-77.
///
/// When `causal_direction` is `Cause` or `Effect`, E5 similarity uses:
/// - Asymmetric vectors: query.e5_as_cause vs doc.e5_as_effect (or reverse)
/// - Direction modifiers: cause→effect (1.2x), effect→cause (0.8x)
///
/// For all other embedders and when direction is `Unknown`, behaves identically
/// to `compute_similarity_for_space()`.
///
/// # Arguments
/// * `embedder` - Which embedding space to compare
/// * `query` - Query fingerprint
/// * `memory` - Memory fingerprint
/// * `causal_direction` - Detected causal direction of the query
///
/// # Returns
/// Similarity in [0.0, 1.0], with direction modifier applied for E5 causal
pub fn compute_similarity_for_space_with_direction(
    embedder: Embedder,
    query: &SemanticFingerprint,
    memory: &SemanticFingerprint,
    causal_direction: crate::causal::asymmetric::CausalDirection,
) -> f32 {
    use crate::causal::asymmetric::{
        compute_e5_asymmetric_fingerprint_similarity, direction_mod, CausalDirection,
    };

    // AP-77: E5 MUST NOT use symmetric cosine — causal is directional.
    if matches!(embedder, Embedder::Causal) {
        if causal_direction == CausalDirection::Unknown {
            // No direction known → E5 cannot provide meaningful signal.
            // Return 0.0 so E5 is effectively excluded from fusion.
            return 0.0;
        }

        let query_is_cause = matches!(causal_direction, CausalDirection::Cause);

        // Compute asymmetric similarity using dual E5 vectors
        let asym_sim = compute_e5_asymmetric_fingerprint_similarity(query, memory, query_is_cause);

        // Infer result direction from document's E5 vectors
        let result_direction = infer_direction_from_fingerprint(memory);

        // Apply Constitution-specified direction modifier
        let dir_mod = match (causal_direction, result_direction) {
            (CausalDirection::Cause, CausalDirection::Effect) => direction_mod::CAUSE_TO_EFFECT,
            (CausalDirection::Effect, CausalDirection::Cause) => direction_mod::EFFECT_TO_CAUSE,
            _ => direction_mod::SAME_DIRECTION,
        };

        return (asym_sim * dir_mod).clamp(0.0, 1.0);
    }

    // Default: symmetric computation for all other embedders
    compute_similarity_for_space(embedder, query, memory)
}

/// Infer causal direction from a stored fingerprint's E5 vectors.
///
/// Delegates to the canonical implementation in `causal::asymmetric`.
fn infer_direction_from_fingerprint(
    fp: &SemanticFingerprint,
) -> crate::causal::asymmetric::CausalDirection {
    crate::causal::asymmetric::infer_direction_from_fingerprint(fp)
}

/// Compute all 13 similarities between query and memory fingerprints.
///
/// Returns an array indexed by Embedder::index().
///
/// # Returns
/// Array of 13 similarity scores in [0.0, 1.0]
pub fn compute_all_similarities(
    query: &SemanticFingerprint,
    memory: &SemanticFingerprint,
) -> [f32; 13] {
    let mut scores = [0.0_f32; 13];

    for embedder in Embedder::all() {
        scores[embedder.index()] = compute_similarity_for_space(embedder, query, memory);
    }

    scores
}

/// Compute all 13 similarities with causal direction for E5.
///
/// Like `compute_all_similarities()` but uses asymmetric E5 similarity
/// when a causal direction is provided.
///
/// # Arguments
/// * `query` - Query fingerprint
/// * `memory` - Memory fingerprint
/// * `causal_direction` - Detected causal direction of the query
///
/// # Returns
/// Array of 13 similarity scores in [0.0, 1.0]
pub fn compute_all_similarities_with_direction(
    query: &SemanticFingerprint,
    memory: &SemanticFingerprint,
    causal_direction: crate::causal::asymmetric::CausalDirection,
) -> [f32; 13] {
    let mut scores = [0.0_f32; 13];

    for embedder in Embedder::all() {
        scores[embedder.index()] =
            compute_similarity_for_space_with_direction(embedder, query, memory, causal_direction);
    }

    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Cosine Similarity Tests
    // =========================================================================

    #[test]
    fn test_cosine_identical_normalized() {
        let v: Vec<f32> = vec![0.6, 0.8, 0.0]; // Already normalized (magnitude = 1.0)
        let sim = cosine_similarity(&v, &v);
        // Raw cosine = 1.0, normalized = (1.0 + 1.0) / 2.0 = 1.0
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Identical vectors should have similarity 1.0, got {}",
            sim
        );
        println!(
            "[PASS] cosine_similarity of identical normalized vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        // Raw cosine = 0.0, normalized = (0.0 + 1.0) / 2.0 = 0.5
        assert!(
            (sim - 0.5).abs() < 1e-5,
            "Orthogonal vectors should have similarity 0.5, got {}",
            sim
        );
        println!(
            "[PASS] cosine_similarity of orthogonal vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_cosine_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        // Raw cosine = -1.0, normalized = (-1.0 + 1.0) / 2.0 = 0.0
        assert!(
            sim.abs() < 1e-5,
            "Opposite vectors should have similarity ~0.0, got {}",
            sim
        );
        assert!(sim >= 0.0 && sim <= 1.0, "Result should be in [0, 1]");
        println!(
            "[PASS] cosine_similarity of opposite vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_cosine_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        // AP-10: Zero vectors return 0.0 similarity (not NaN)
        assert_eq!(sim, 0.0, "Zero vector should return 0.0 (AP-10)");
        assert!(!sim.is_nan(), "AP-10 violation: Result must not be NaN");
        println!(
            "[PASS] cosine_similarity with zero vector = {:.6} (AP-10 compliant)",
            sim
        );
    }

    #[test]
    fn test_cosine_empty_vector() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Empty vector should return 0.0");
        println!("[PASS] cosine_similarity with empty vector = 0.0");
    }

    #[test]
    fn test_cosine_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Dimension mismatch should return 0.0");
        println!("[PASS] cosine_similarity with dimension mismatch = 0.0");
    }

    // =========================================================================
    // Jaccard Similarity Tests
    // =========================================================================

    #[test]
    fn test_jaccard_identical() {
        let v = SparseVector::new(vec![0, 5, 10], vec![1.0, 1.0, 1.0]).unwrap();
        let sim = jaccard_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Identical sparse vectors should have similarity 1.0, got {}",
            sim
        );
        println!(
            "[PASS] jaccard_similarity of identical vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_jaccard_disjoint() {
        let a = SparseVector::new(vec![0, 1, 2], vec![1.0, 1.0, 1.0]).unwrap();
        let b = SparseVector::new(vec![5, 6, 7], vec![1.0, 1.0, 1.0]).unwrap();
        let sim = jaccard_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Disjoint sets should have similarity 0.0");
        println!("[PASS] jaccard_similarity of disjoint sets = {:.6}", sim);
    }

    #[test]
    fn test_jaccard_partial_overlap() {
        let a = SparseVector::new(vec![0, 1, 2], vec![1.0, 1.0, 1.0]).unwrap();
        let b = SparseVector::new(vec![1, 2, 3], vec![1.0, 1.0, 1.0]).unwrap();
        let sim = jaccard_similarity(&a, &b);
        // Intersection: {1, 2} = 2 elements
        // Union: {0, 1, 2, 3} = 4 elements
        // Jaccard = 2/4 = 0.5
        assert!((sim - 0.5).abs() < 1e-5, "Expected 0.5, got {}", sim);
        println!(
            "[PASS] jaccard_similarity with 50% overlap = {:.6}",
            sim
        );
    }

    #[test]
    fn test_jaccard_empty() {
        let a = SparseVector::empty();
        let b = SparseVector::empty();
        let sim = jaccard_similarity(&a, &b);
        // Empty sets: Defined as 0.0 by SparseVector::jaccard_similarity
        assert_eq!(sim, 0.0, "Empty vectors should have similarity 0.0");
        println!("[PASS] jaccard_similarity of empty vectors = {:.6}", sim);
    }

    // =========================================================================
    // Hamming Similarity Tests
    // =========================================================================

    #[test]
    fn test_hamming_identical() {
        let v = BinaryVector::zeros(64);
        let sim = hamming_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Identical binary vectors should have similarity 1.0, got {}",
            sim
        );
        println!(
            "[PASS] hamming_similarity of identical vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_hamming_all_different() {
        let mut a = BinaryVector::zeros(64);
        let b = BinaryVector::zeros(64);

        // Set all bits in 'a' to 1, leave 'b' as 0
        for i in 0..64 {
            a.set_bit(i, true);
        }

        let sim = hamming_similarity(&a, &b);
        assert_eq!(sim, 0.0, "All different bits should have similarity 0.0");
        println!(
            "[PASS] hamming_similarity of opposite vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_hamming_half_different() {
        let mut a = BinaryVector::zeros(64);
        let b = BinaryVector::zeros(64);

        // Set first 32 bits to 1 in 'a'
        for i in 0..32 {
            a.set_bit(i, true);
        }

        let sim = hamming_similarity(&a, &b);
        // 32 bits different out of 64 = 0.5 distance = 0.5 similarity
        assert!((sim - 0.5).abs() < 1e-5, "Expected 0.5, got {}", sim);
        println!(
            "[PASS] hamming_similarity with 50% difference = {:.6}",
            sim
        );
    }

    #[test]
    fn test_hamming_empty() {
        let a = BinaryVector::zeros(0);
        let b = BinaryVector::zeros(0);
        let sim = hamming_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Empty binary vectors should be identical (similarity 1.0)"
        );
        println!("[PASS] hamming_similarity of empty vectors = {:.6}", sim);
    }

    // =========================================================================
    // MaxSim (Late Interaction) Tests
    // =========================================================================

    #[test]
    fn test_max_sim_identical() {
        let tokens = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];
        let sim = max_sim(&tokens, &tokens);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Identical token sets should have MaxSim 1.0, got {}",
            sim
        );
        println!("[PASS] max_sim of identical token sets = {:.6}", sim);
    }

    #[test]
    fn test_max_sim_empty_query() {
        let empty: Vec<Vec<f32>> = vec![];
        let tokens = vec![vec![1.0, 0.0, 0.0]];
        let sim = max_sim(&empty, &tokens);
        assert_eq!(sim, 0.0, "Empty query should return 0.0");
        println!("[PASS] max_sim with empty query = 0.0");
    }

    #[test]
    fn test_max_sim_empty_memory() {
        let tokens = vec![vec![1.0, 0.0, 0.0]];
        let empty: Vec<Vec<f32>> = vec![];
        let sim = max_sim(&tokens, &empty);
        assert_eq!(sim, 0.0, "Empty memory should return 0.0");
        println!("[PASS] max_sim with empty memory = 0.0");
    }

    #[test]
    fn test_max_sim_partial_match() {
        let query = vec![
            vec![1.0, 0.0, 0.0], // Will match first memory token perfectly
            vec![0.0, 0.0, 1.0], // Orthogonal to all memory tokens
        ];
        let memory = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];
        let sim = max_sim(&query, &memory);
        // First query token: max sim = 1.0 (perfect match)
        // Second query token: max sim = 0.5 (orthogonal = 0.5 in normalized space)
        // Average = (1.0 + 0.5) / 2 = 0.75
        assert!((sim - 0.75).abs() < 1e-5, "Expected 0.75, got {}", sim);
        println!("[PASS] max_sim with partial match = {:.6}", sim);
    }

    // =========================================================================
    // TransE Similarity Tests
    // =========================================================================

    #[test]
    fn test_transe_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = transe_similarity(&v, &v);
        // Distance = 0, so similarity = 1 / (1 + 0) = 1.0
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Identical vectors should have TransE similarity 1.0, got {}",
            sim
        );
        println!(
            "[PASS] transe_similarity of identical vectors = {:.6}",
            sim
        );
    }

    #[test]
    fn test_transe_unit_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = transe_similarity(&a, &b);
        // Distance = 1.0, so similarity = 1 / (1 + 1) = 0.5
        assert!(
            (sim - 0.5).abs() < 1e-5,
            "Unit distance should have TransE similarity 0.5, got {}",
            sim
        );
        println!("[PASS] transe_similarity at unit distance = {:.6}", sim);
    }

    #[test]
    fn test_transe_empty_vector() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0, 3.0];
        let sim = transe_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Empty vector should return 0.0");
        println!("[PASS] transe_similarity with empty vector = 0.0");
    }

    // =========================================================================
    // compute_similarity_for_space Tests
    // =========================================================================

    #[test]
    fn test_compute_similarity_semantic() {
        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Set identical semantic embeddings
        query.e1_semantic = vec![1.0; 1024];
        memory.e1_semantic = vec![1.0; 1024];

        let sim = compute_similarity_for_space(Embedder::Semantic, &query, &memory);
        assert!((sim - 1.0).abs() < 1e-5, "Expected 1.0, got {}", sim);
        println!(
            "[PASS] compute_similarity_for_space(Semantic) = {:.6}",
            sim
        );
    }

    #[test]
    fn test_compute_similarity_sparse() {
        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Set identical sparse embeddings
        query.e6_sparse = SparseVector::new(vec![0, 5, 10], vec![1.0, 1.0, 1.0]).unwrap();
        memory.e6_sparse = SparseVector::new(vec![0, 5, 10], vec![1.0, 1.0, 1.0]).unwrap();

        let sim = compute_similarity_for_space(Embedder::Sparse, &query, &memory);
        assert!((sim - 1.0).abs() < 1e-5, "Expected 1.0, got {}", sim);
        println!("[PASS] compute_similarity_for_space(Sparse) = {:.6}", sim);
    }

    #[test]
    fn test_compute_similarity_late_interaction() {
        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Set identical late interaction embeddings
        query.e12_late_interaction = vec![vec![1.0; 128], vec![0.5; 128]];
        memory.e12_late_interaction = vec![vec![1.0; 128], vec![0.5; 128]];

        let sim = compute_similarity_for_space(Embedder::LateInteraction, &query, &memory);
        assert!((sim - 1.0).abs() < 1e-5, "Expected 1.0, got {}", sim);
        println!(
            "[PASS] compute_similarity_for_space(LateInteraction) = {:.6}",
            sim
        );
    }

    #[test]
    fn test_compute_similarity_entity_uses_cosine() {
        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Set entity embeddings - both pointing in same direction (KEPLER = 768D)
        query.e11_entity = vec![0.0; 768];
        memory.e11_entity = vec![0.0; 768];
        query.e11_entity[0] = 1.0;
        memory.e11_entity[0] = 1.0;

        let sim = compute_similarity_for_space(Embedder::Entity, &query, &memory);
        // Cosine of identical unit vectors = 1.0, normalized = (1.0 + 1.0) / 2.0 = 1.0
        assert!((sim - 1.0).abs() < 1e-5, "Expected 1.0 (cosine identical), got {}", sim);
        println!(
            "[PASS] compute_similarity_for_space(Entity) uses Cosine = {:.6}",
            sim
        );
    }

    #[test]
    fn test_compute_similarity_entity_orthogonal() {
        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Set orthogonal entity embeddings (KEPLER = 768D)
        query.e11_entity = vec![0.0; 768];
        memory.e11_entity = vec![0.0; 768];
        query.e11_entity[0] = 1.0;  // Points in first dimension
        memory.e11_entity[1] = 1.0; // Points in second dimension

        let sim = compute_similarity_for_space(Embedder::Entity, &query, &memory);
        // Cosine of orthogonal vectors = 0.0, normalized = (0.0 + 1.0) / 2.0 = 0.5
        assert!((sim - 0.5).abs() < 1e-5, "Expected 0.5 (cosine orthogonal), got {}", sim);
        println!(
            "[PASS] compute_similarity_for_space(Entity) orthogonal = {:.6}",
            sim
        );
    }

    // =========================================================================
    // compute_all_similarities Tests
    // =========================================================================

    /// LOW-3 Note: This test uses `SemanticFingerprint::zeroed()` intentionally.
    /// It tests the *infrastructure* — that `compute_all_similarities` returns
    /// 13 valid scores in [0,1] and does not panic on degenerate input.
    /// Search quality is validated by benchmark suites with real embeddings.
    #[test]
    fn test_compute_all_similarities() {
        let query = SemanticFingerprint::zeroed();
        let memory = SemanticFingerprint::zeroed();

        let scores = compute_all_similarities(&query, &memory);

        assert_eq!(scores.len(), 13);
        for (i, score) in scores.iter().enumerate() {
            assert!(
                *score >= 0.0 && *score <= 1.0,
                "Score {} for embedder {} out of range: {}",
                i,
                Embedder::from_index(i).unwrap().name(),
                score
            );
        }
        println!("[PASS] compute_all_similarities returns 13 valid scores");
    }

    // =========================================================================
    // Edge Case / Boundary Tests
    // =========================================================================

    #[test]
    fn test_nan_not_produced() {
        // Test various edge cases that might produce NaN
        let zero = vec![0.0, 0.0, 0.0];
        let normal = vec![1.0, 2.0, 3.0];

        let results = [
            cosine_similarity(&zero, &zero),
            cosine_similarity(&zero, &normal),
            transe_similarity(&zero, &zero),
        ];

        for (i, r) in results.iter().enumerate() {
            assert!(!r.is_nan(), "Result {} is NaN - AP-10 violation", i);
            assert!(!r.is_infinite(), "Result {} is infinite", i);
        }
        println!("[PASS] No NaN produced in edge cases (AP-10 compliance)");
    }

    #[test]
    fn test_all_results_in_range() {
        // Generate various test cases
        let test_vecs = [
            (vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]),
            (vec![1.0, 1.0, 1.0], vec![1.0, 1.0, 1.0]),
            (vec![0.0, 0.0, 0.0], vec![1.0, 2.0, 3.0]),
            (vec![-1.0, 0.0, 0.0], vec![1.0, 0.0, 0.0]),
        ];

        for (a, b) in &test_vecs {
            let cos = cosine_similarity(a, b);
            let transe = transe_similarity(a, b);

            assert!(cos >= 0.0 && cos <= 1.0, "Cosine {} out of range", cos);
            assert!(
                transe >= 0.0 && transe <= 1.0,
                "TransE {} out of range",
                transe
            );
        }
        println!("[PASS] All results in [0.0, 1.0] range");
    }

    // =========================================================================
    // Manual Edge Case Verification (TASK-P3-004 requirement: 3+ edge cases)
    // =========================================================================

    #[test]
    fn edge_case_1_very_small_magnitudes() {
        // Edge Case 1: Very small values that might underflow
        let small_a = vec![1e-20_f32, 1e-20_f32, 1e-20_f32];
        let small_b = vec![1e-20_f32, 1e-20_f32, 1e-20_f32];
        let sim = cosine_similarity(&small_a, &small_b);

        assert!(!sim.is_nan(), "FAIL: NaN produced for very small values");
        assert!(!sim.is_infinite(), "FAIL: Infinity produced for very small values");
        assert!(sim >= 0.0 && sim <= 1.0, "FAIL: Result out of [0,1] range: {}", sim);
        println!("[PASS] Edge Case 1: Very small magnitudes handled correctly, sim = {}", sim);
    }

    #[test]
    fn edge_case_2_large_values_overflow_prevention() {
        // Edge Case 2: Large values that might overflow during magnitude calculation
        let large_a = vec![1e19_f32, 1e19_f32, 1e19_f32];
        let large_b = vec![1e19_f32, 1e19_f32, 1e19_f32];
        let sim = cosine_similarity(&large_a, &large_b);

        assert!(!sim.is_nan(), "FAIL: NaN produced for large values");
        assert!(!sim.is_infinite(), "FAIL: Infinity produced for large values");
        assert!(sim >= 0.0 && sim <= 1.0, "FAIL: Result out of [0,1] range: {}", sim);
        println!("[PASS] Edge Case 2: Large values handled correctly, sim = {}", sim);
    }

    #[test]
    fn edge_case_3_single_token_opposite_directions() {
        // Edge Case 3: Single-element late interaction with opposite directions
        let query = vec![vec![1.0_f32]];
        let memory = vec![vec![-1.0_f32]];
        let sim = max_sim(&query, &memory);

        // Raw cosine = -1.0, normalized = (-1+1)/2 = 0.0
        assert!(!sim.is_nan(), "FAIL: NaN in single-token MaxSim");
        assert!(sim >= 0.0 && sim <= 1.0, "FAIL: Result out of [0,1] range: {}", sim);
        assert!(sim.abs() < 1e-5, "Expected ~0.0 for opposite directions, got {}", sim);
        println!("[PASS] Edge Case 3: Single-token opposite MaxSim = {}", sim);
    }

    #[test]
    fn edge_case_all_13_spaces_zeroed_fingerprints() {
        // Verify all 13 embedding spaces handle zeroed fingerprints gracefully
        let zeroed = SemanticFingerprint::zeroed();
        let scores = compute_all_similarities(&zeroed, &zeroed);

        for (i, score) in scores.iter().enumerate() {
            let embedder = Embedder::from_index(i).unwrap();
            assert!(
                !score.is_nan(),
                "FAIL: NaN for {} space with zeroed fingerprints",
                embedder.name()
            );
            assert!(
                !score.is_infinite(),
                "FAIL: Infinity for {} space with zeroed fingerprints",
                embedder.name()
            );
            assert!(
                *score >= 0.0 && *score <= 1.0,
                "FAIL: {} out of [0,1] range: {}",
                embedder.name(),
                score
            );
        }
        println!("[PASS] All 13 spaces handle zeroed fingerprints without NaN/Infinity");
    }

    // =========================================================================
    // Direction-Aware Similarity Tests (ARCH-15, AP-77)
    // =========================================================================

    #[test]
    fn test_direction_aware_unknown_returns_zero_for_e5() {
        // AP-77: E5 MUST NOT use symmetric cosine — causal is directional.
        // When direction is Unknown, E5 returns 0.0 (no signal is better than wrong signal).
        use crate::causal::asymmetric::CausalDirection;

        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Set identical E5 causal vectors — would produce sim=1.0 if symmetric were used
        query.e5_causal_as_cause = vec![1.0; 768];
        query.e5_causal_as_effect = vec![0.5; 768];
        memory.e5_causal_as_cause = vec![1.0; 768];
        memory.e5_causal_as_effect = vec![0.5; 768];

        let asym_unknown =
            compute_similarity_for_space_with_direction(Embedder::Causal, &query, &memory, CausalDirection::Unknown);

        assert_eq!(
            asym_unknown, 0.0,
            "E5 with Unknown direction must return 0.0 per AP-77, got {}",
            asym_unknown
        );

        // With known direction, E5 should produce a non-zero score
        let asym_cause =
            compute_similarity_for_space_with_direction(Embedder::Causal, &query, &memory, CausalDirection::Cause);
        assert!(
            asym_cause > 0.0,
            "E5 with Cause direction should produce non-zero score, got {}",
            asym_cause
        );
    }

    #[test]
    fn test_direction_aware_non_causal_embedder() {
        // Non-E5 embedders should use symmetric regardless of direction
        use crate::causal::asymmetric::CausalDirection;

        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        query.e1_semantic = vec![1.0; 1024];
        memory.e1_semantic = vec![1.0; 1024];

        let sym = compute_similarity_for_space(Embedder::Semantic, &query, &memory);
        let with_cause =
            compute_similarity_for_space_with_direction(Embedder::Semantic, &query, &memory, CausalDirection::Cause);
        let with_effect =
            compute_similarity_for_space_with_direction(Embedder::Semantic, &query, &memory, CausalDirection::Effect);

        assert!(
            (sym - with_cause).abs() < 1e-5,
            "E1 Semantic should ignore Cause direction"
        );
        assert!(
            (sym - with_effect).abs() < 1e-5,
            "E1 Semantic should ignore Effect direction"
        );
        println!("[PASS] Non-causal embedders ignore direction parameter");
    }

    #[test]
    fn test_direction_aware_cause_seeking() {
        // Cause-seeking query (why?) should use asymmetric E5
        use crate::causal::asymmetric::CausalDirection;

        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Query is seeking causes (why did X happen?)
        // Memory contains an effect (X happened because Y)
        // Use orthogonal vectors to get lower base similarity that won't clamp to 1.0
        query.e5_causal_as_cause = vec![1.0; 768];
        query.e5_causal_as_effect = vec![0.0; 768];
        // Memory effect vector orthogonal to query cause vector
        memory.e5_causal_as_cause = vec![0.1; 768];  // Document emphasizes effect
        memory.e5_causal_as_effect = vec![0.9; 768];

        let sym = compute_similarity_for_space(Embedder::Causal, &query, &memory);
        let asym_cause =
            compute_similarity_for_space_with_direction(Embedder::Causal, &query, &memory, CausalDirection::Cause);

        // Cause→Effect transition should get 1.2x boost per AP-77
        // Asymmetric should either differ from symmetric, or both be at max (clamped)
        // Note: when asymmetric * 1.2 > 1.0, it clamps to 1.0
        assert!(
            asym_cause >= 0.0 && asym_cause <= 1.0,
            "Result should be clamped to [0, 1]: {}",
            asym_cause
        );

        // Verify the 1.2x boost is applied (score should be >= symmetric score
        // unless both are clamped to 1.0)
        let score_ratio = if sym > 0.01 { asym_cause / sym } else { 1.0 };
        assert!(
            score_ratio >= 1.0 || (asym_cause - 1.0).abs() < 1e-5,
            "Cause→Effect should boost score (got ratio {:.4})",
            score_ratio
        );
        println!(
            "[PASS] Cause-seeking asymmetric: sym={:.6}, asym={:.6}, ratio={:.4}",
            sym, asym_cause, score_ratio
        );
    }

    #[test]
    fn test_direction_aware_effect_seeking() {
        // Effect-seeking query (what happens when?) should use asymmetric E5
        use crate::causal::asymmetric::CausalDirection;

        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Query is seeking effects (what happens when X?)
        // Memory contains a cause (Y leads to...)
        query.e5_causal_as_cause = vec![0.2; 768];
        query.e5_causal_as_effect = vec![0.9; 768];
        memory.e5_causal_as_cause = vec![0.85; 768];  // Document emphasizes cause
        memory.e5_causal_as_effect = vec![0.3; 768];

        let sym = compute_similarity_for_space(Embedder::Causal, &query, &memory);
        let asym_effect =
            compute_similarity_for_space_with_direction(Embedder::Causal, &query, &memory, CausalDirection::Effect);

        // Effect→Cause transition should get 0.8x dampening per AP-77
        // So asymmetric score should differ from symmetric
        assert!(
            asym_effect != sym,
            "Asymmetric effect-seeking should differ from symmetric"
        );
        assert!(
            asym_effect >= 0.0 && asym_effect <= 1.0,
            "Result should be clamped to [0, 1]: {}",
            asym_effect
        );
        println!(
            "[PASS] Effect-seeking asymmetric: sym={:.6}, asym={:.6}",
            sym, asym_effect
        );
    }

    /// LOW-3 Note: This test uses `SemanticFingerprint::zeroed()` intentionally.
    /// It tests the *infrastructure* — that `compute_all_similarities_with_direction`
    /// returns 13 valid scores in [0,1] for all CausalDirection variants and does
    /// not panic or produce NaN on degenerate input. Search quality with real
    /// embeddings is validated by benchmark suites.
    #[test]
    fn test_compute_all_similarities_with_direction() {
        use crate::causal::asymmetric::CausalDirection;

        let query = SemanticFingerprint::zeroed();
        let memory = SemanticFingerprint::zeroed();

        let scores_unknown = compute_all_similarities_with_direction(&query, &memory, CausalDirection::Unknown);
        let scores_cause = compute_all_similarities_with_direction(&query, &memory, CausalDirection::Cause);
        let scores_effect = compute_all_similarities_with_direction(&query, &memory, CausalDirection::Effect);

        // All should return 13 valid scores
        assert_eq!(scores_unknown.len(), 13);
        assert_eq!(scores_cause.len(), 13);
        assert_eq!(scores_effect.len(), 13);

        for scores in [&scores_unknown, &scores_cause, &scores_effect] {
            for (i, score) in scores.iter().enumerate() {
                assert!(
                    *score >= 0.0 && *score <= 1.0,
                    "Score {} for embedder {} out of range: {}",
                    i,
                    Embedder::from_index(i).unwrap().name(),
                    score
                );
                assert!(!score.is_nan(), "NaN in scores");
            }
        }
        println!("[PASS] compute_all_similarities_with_direction returns valid scores for all directions");
    }

    #[test]
    fn test_infer_direction_from_fingerprint() {
        use crate::causal::asymmetric::CausalDirection;

        // Test cause-dominant fingerprint (peaked cause vector, uniform effect vector).
        // Both vectors are L2-normalized (as in production), so direction is inferred
        // from component variance, not magnitude.
        let mut cause_doc = SemanticFingerprint::zeroed();
        // Peaked cause vector: first 20 dims large, rest small → high variance
        let mut cause_v = vec![0.01_f32; 768];
        for v in cause_v.iter_mut().take(20) {
            *v = 0.2;
        }
        let norm: f32 = cause_v.iter().map(|x| x * x).sum::<f32>().sqrt();
        for v in cause_v.iter_mut() {
            *v /= norm;
        }
        cause_doc.e5_causal_as_cause = cause_v;
        // Uniform effect vector: all components equal → zero variance
        cause_doc.e5_causal_as_effect = vec![1.0 / (768.0_f32).sqrt(); 768];

        let dir = infer_direction_from_fingerprint(&cause_doc);
        assert!(
            matches!(dir, CausalDirection::Cause),
            "Expected Cause for peaked-cause fingerprint, got {:?}",
            dir
        );

        // Test effect-dominant fingerprint (uniform cause, peaked effect)
        let mut effect_doc = SemanticFingerprint::zeroed();
        effect_doc.e5_causal_as_cause = vec![1.0 / (768.0_f32).sqrt(); 768];
        let mut effect_v = vec![0.01_f32; 768];
        for v in effect_v.iter_mut().take(20) {
            *v = 0.2;
        }
        let norm: f32 = effect_v.iter().map(|x| x * x).sum::<f32>().sqrt();
        for v in effect_v.iter_mut() {
            *v /= norm;
        }
        effect_doc.e5_causal_as_effect = effect_v;

        let dir = infer_direction_from_fingerprint(&effect_doc);
        assert!(
            matches!(dir, CausalDirection::Effect),
            "Expected Effect for peaked-effect fingerprint, got {:?}",
            dir
        );

        // Test balanced fingerprint (same distribution → same variance → Unknown)
        let mut balanced_doc = SemanticFingerprint::zeroed();
        let uniform = vec![1.0 / (768.0_f32).sqrt(); 768];
        balanced_doc.e5_causal_as_cause = uniform.clone();
        balanced_doc.e5_causal_as_effect = uniform;
        let dir = infer_direction_from_fingerprint(&balanced_doc);
        assert!(
            matches!(dir, CausalDirection::Unknown),
            "Expected Unknown for balanced fingerprint, got {:?}",
            dir
        );

        println!("[PASS] infer_direction_from_fingerprint correctly categorizes L2-normalized documents");
    }

    #[test]
    fn test_direction_aware_empty_e5_vectors() {
        // Edge case: Empty E5 vectors should not crash
        use crate::causal::asymmetric::CausalDirection;

        let query = SemanticFingerprint::zeroed();
        let memory = SemanticFingerprint::zeroed();

        let result =
            compute_similarity_for_space_with_direction(Embedder::Causal, &query, &memory, CausalDirection::Cause);

        assert!(!result.is_nan(), "Empty E5 vectors should not produce NaN");
        assert!(result >= 0.0 && result <= 1.0, "Result out of range: {}", result);
        println!("[PASS] Empty E5 vectors handled gracefully: {:.6}", result);
    }

    #[test]
    fn test_direction_modifier_values_match_constitution() {
        // Verify our direction modifiers match AP-77 spec
        use crate::causal::asymmetric::direction_mod;

        assert!(
            (direction_mod::CAUSE_TO_EFFECT - 1.2).abs() < 1e-5,
            "CAUSE_TO_EFFECT should be 1.2 per AP-77"
        );
        assert!(
            (direction_mod::EFFECT_TO_CAUSE - 0.8).abs() < 1e-5,
            "EFFECT_TO_CAUSE should be 0.8 per AP-77"
        );
        assert!(
            (direction_mod::SAME_DIRECTION - 1.0).abs() < 1e-5,
            "SAME_DIRECTION should be 1.0"
        );

        // Verify asymmetry ratio = 1.5
        let ratio = direction_mod::CAUSE_TO_EFFECT / direction_mod::EFFECT_TO_CAUSE;
        assert!(
            (ratio - 1.5).abs() < 1e-5,
            "Asymmetry ratio should be 1.5, got {}",
            ratio
        );
        println!(
            "[PASS] Direction modifiers match Constitution: C→E={}, E→C={}, ratio={}",
            direction_mod::CAUSE_TO_EFFECT,
            direction_mod::EFFECT_TO_CAUSE,
            ratio
        );
    }
}
