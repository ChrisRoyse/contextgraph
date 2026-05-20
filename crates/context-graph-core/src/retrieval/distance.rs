//! Distance and similarity metrics for the 13 embedding spaces.
//!
//! This module provides unified distance/similarity computation across all
//! embedding types: dense, sparse, binary, and token-level.
//!
//! # Design Philosophy
//!
//! Most similarity functions delegate to existing vector type methods:
//! - cosine_similarity() (zero-allocation, direct slice computation)
//! - SparseVector::jaccard_similarity()
//!
//! This module adds:
//! - max_sim() for ColBERT late interaction (E12)
//! - compute_similarity_for_space() dispatcher
//!
//! # All outputs are normalized to [0.0, 1.0]

use crate::teleological::Embedder;
use crate::types::fingerprint::{EmbeddingRef, SemanticFingerprint, SparseVector};

/// Structured error variants for `try_cosine_similarity` /
/// `try_cosine_similarity_raw`.
///
/// Per F-025 (Sherlock investigation 2026-05-19): the legacy
/// `cosine_similarity` helper conflated **two distinct** failure modes —
/// (a) the AP-10 zero-magnitude case, and (b) dimension mismatch — into a
/// single sentinel `0.0`. Dim-mismatch is a structural bug (CLAUDE.md §6.2
/// slot-identity violation in panel contexts); zero-magnitude is a valid
/// runtime state. This enum lets callers distinguish them.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CosineSimilarityError {
    /// Both vectors are empty (zero-length slices).
    #[error("VECTOR_EMPTY_INPUT")]
    EmptyInput,
    /// Vector lengths disagree.
    #[error("VECTOR_DIM_MISMATCH: left={left} right={right}")]
    DimensionMismatch { left: usize, right: usize },
    /// At least one vector has zero magnitude (all-zero or numerically zero).
    #[error("VECTOR_ZERO_MAGNITUDE: left_norm_sq={left_norm_sq} right_norm_sq={right_norm_sq}")]
    ZeroMagnitude {
        left_norm_sq: f32,
        right_norm_sq: f32,
    },
}

/// Compute cosine similarity between two dense vectors.
///
/// Zero-allocation implementation using direct slice computation.
/// Returns 0.0 for zero-magnitude vectors (AP-10: no NaN).
///
/// # Failure modes (F-025 Sherlock investigation 2026-05-19)
///
/// This **legacy / panic-free** helper exists for backwards compatibility
/// with callers that cannot easily propagate `Result`. It returns `0.0` for
/// BOTH zero-magnitude inputs (AP-10) AND dimension mismatch — conflating
/// two structurally different errors. The dim-mismatch arm trips a
/// `debug_assert_eq!` so the violation is loud in dev/test builds; in
/// release builds it silently returns `0.0` for backwards compatibility.
///
/// **Prefer `try_cosine_similarity` for new code.** It returns
/// `Result<f32, CosineSimilarityError>` and lets the caller distinguish the
/// failure modes (`EmptyInput`, `DimensionMismatch`, `ZeroMagnitude`).
///
/// # Arguments
/// * `a` - First dense embedding as f32 slice
/// * `b` - Second dense embedding as f32 slice
///
/// # Returns
/// Similarity in [0.0, 1.0] where 1.0 = identical direction
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // F-025 dim-mismatch loudness in dev/test. In release the legacy 0.0
    // is preserved to avoid breaking existing callers; new code should
    // adopt `try_cosine_similarity` instead.
    debug_assert_eq!(
        a.len(),
        b.len(),
        "VECTOR_DIM_MISMATCH (F-025): cosine_similarity left.len()={} right.len()={}. \
         This is a structural bug — use try_cosine_similarity to surface it.",
        a.len(),
        b.len()
    );
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    // Compute dot product and magnitudes in a single pass (zero-allocation)
    let mut dot = 0.0_f32;
    let mut mag_a_sq = 0.0_f32;
    let mut mag_b_sq = 0.0_f32;

    for (ai, bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        mag_a_sq += ai * ai;
        mag_b_sq += bi * bi;
    }

    // AP-10: zero-magnitude vectors return 0.0 (no NaN). This is a distinct
    // failure mode from dim-mismatch above; new callers should use
    // `try_cosine_similarity` to tell them apart.
    if mag_a_sq == 0.0 || mag_b_sq == 0.0 {
        return 0.0;
    }

    let raw_sim = (dot / (mag_a_sq.sqrt() * mag_b_sq.sqrt())).clamp(-1.0, 1.0);

    // Normalize from [-1, 1] to [0, 1] (SRC-3)
    (raw_sim + 1.0) / 2.0
}

/// Result-returning cosine similarity. **Prefer this for new code.**
///
/// Per F-025 (Sherlock investigation 2026-05-19): the legacy
/// `cosine_similarity` returns `0.0` for both `DimensionMismatch` and
/// `ZeroMagnitude`. This variant surfaces the failure mode explicitly so the
/// caller can route correctly (dim-mismatch = structural slot-identity
/// violation in panel contexts per CLAUDE.md §6.2; zero-magnitude = valid
/// runtime state that may legitimately produce no signal).
///
/// # Returns
/// `Ok(similarity)` in `[0.0, 1.0]` where `1.0` = identical direction.
/// `Err(CosineSimilarityError::{EmptyInput | DimensionMismatch | ZeroMagnitude})`
/// for the three distinguishable failure modes.
pub fn try_cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, CosineSimilarityError> {
    if a.is_empty() && b.is_empty() {
        return Err(CosineSimilarityError::EmptyInput);
    }
    if a.len() != b.len() {
        return Err(CosineSimilarityError::DimensionMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    if a.is_empty() {
        // Same length AND both empty: caller passed two zero-length slices.
        return Err(CosineSimilarityError::EmptyInput);
    }

    let mut dot = 0.0_f32;
    let mut mag_a_sq = 0.0_f32;
    let mut mag_b_sq = 0.0_f32;
    for (ai, bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        mag_a_sq += ai * ai;
        mag_b_sq += bi * bi;
    }

    if mag_a_sq == 0.0 || mag_b_sq == 0.0 {
        return Err(CosineSimilarityError::ZeroMagnitude {
            left_norm_sq: mag_a_sq,
            right_norm_sq: mag_b_sq,
        });
    }

    let raw_sim = (dot / (mag_a_sq.sqrt() * mag_b_sq.sqrt())).clamp(-1.0, 1.0);
    Ok((raw_sim + 1.0) / 2.0)
}

/// Compute raw cosine similarity between two dense vectors.
///
/// CORE-M3: Canonical raw cosine implementation returning [-1.0, 1.0].
/// Use this when raw cosine is needed (e.g., code search, causal chain scoring)
/// without the SRC-3 normalization to [0, 1] that `cosine_similarity()` applies.
///
/// Zero-allocation, NaN-safe, clamped to [-1.0, 1.0].
///
/// # F-025 dim-mismatch loudness
///
/// Same caveats as `cosine_similarity`: dim-mismatch is loud in dev/test
/// (via `debug_assert_eq!`) and silently returns `0.0` in release for legacy
/// caller compatibility. Prefer `try_cosine_similarity_raw` for new code.
pub fn cosine_similarity_raw(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(
        a.len(),
        b.len(),
        "VECTOR_DIM_MISMATCH (F-025): cosine_similarity_raw left.len()={} right.len()={}. \
         This is a structural bug — use try_cosine_similarity_raw to surface it.",
        a.len(),
        b.len()
    );
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut mag_a_sq = 0.0_f32;
    let mut mag_b_sq = 0.0_f32;

    for (ai, bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        mag_a_sq += ai * ai;
        mag_b_sq += bi * bi;
    }

    if mag_a_sq == 0.0 || mag_b_sq == 0.0 {
        return 0.0;
    }

    (dot / (mag_a_sq.sqrt() * mag_b_sq.sqrt())).clamp(-1.0, 1.0)
}

/// Result-returning raw cosine similarity in `[-1.0, 1.0]`.
///
/// See `try_cosine_similarity` (F-025) for rationale.
pub fn try_cosine_similarity_raw(a: &[f32], b: &[f32]) -> Result<f32, CosineSimilarityError> {
    if a.is_empty() && b.is_empty() {
        return Err(CosineSimilarityError::EmptyInput);
    }
    if a.len() != b.len() {
        return Err(CosineSimilarityError::DimensionMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    if a.is_empty() {
        return Err(CosineSimilarityError::EmptyInput);
    }

    let mut dot = 0.0_f32;
    let mut mag_a_sq = 0.0_f32;
    let mut mag_b_sq = 0.0_f32;
    for (ai, bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        mag_a_sq += ai * ai;
        mag_b_sq += bi * bi;
    }

    if mag_a_sq == 0.0 || mag_b_sq == 0.0 {
        return Err(CosineSimilarityError::ZeroMagnitude {
            left_norm_sq: mag_a_sq,
            right_norm_sq: mag_b_sq,
        });
    }

    Ok((dot / (mag_a_sq.sqrt() * mag_b_sq.sqrt())).clamp(-1.0, 1.0))
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
/// on the projected representation is used.
///
/// # E11 Entity Note
///
/// E11 uses cosine similarity for general entity-entity comparison.
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
            // EMB-7 FIX: E5 without direction returns -1.0 (sentinel = "no signal").
            // Use compute_similarity_for_space_with_direction() for directional E5 similarity.
            // CORE-H1 FIX: -1.0 sentinel distinguishes "no signal" from 0.0 (anti-correlated).
            -1.0
        }
        Embedder::Graph => {
            // E8: Compare source-vs-target cross pairs and take max
            let source_vs_target =
                cosine_similarity(query.get_e8_as_source(), memory.get_e8_as_target());
            let target_vs_source =
                cosine_similarity(query.get_e8_as_target(), memory.get_e8_as_source());
            // Take max of both directions — the stronger signal wins
            source_vs_target.max(target_vs_source)
        }
        Embedder::Contextual => {
            // E10: Compare paraphrase-vs-context cross pairs and take max
            let para_vs_context =
                cosine_similarity(query.get_e10_as_paraphrase(), memory.get_e10_as_context());
            let context_vs_para =
                cosine_similarity(query.get_e10_as_context(), memory.get_e10_as_paraphrase());
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
                (EmbeddingRef::Dense(q), EmbeddingRef::Dense(m)) => cosine_similarity(q, m),
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
    use crate::weights::E5_CAUSAL_ENABLED;

    // AP-77: E5 MUST NOT use symmetric cosine — causal is directional.
    if matches!(embedder, Embedder::Causal) {
        if !E5_CAUSAL_ENABLED {
            return -1.0;
        }
        if causal_direction == CausalDirection::Unknown {
            // No direction known → E5 cannot provide meaningful signal.
            // CORE-H1 FIX: Return -1.0 sentinel (not 0.0) so fusion correctly
            // distinguishes "no signal" from 0.0 (anti-correlated after normalization).
            return -1.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_core_cases() {
        // Identical
        let v: Vec<f32> = vec![0.6, 0.8, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-5);

        // Orthogonal: raw=0.0, normalized=0.5
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.5).abs() < 1e-5);

        // Zero vector: AP-10 compliance — same-dim zero vs nonzero returns 0.0.
        let zero = vec![0.0, 0.0, 0.0];
        let normal = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&zero, &normal), 0.0);
        assert!(!cosine_similarity(&zero, &zero).is_nan());

        // F-025: dim-mismatch is now classified via `try_cosine_similarity`.
        // The legacy `cosine_similarity` debug_assert_eq! fires in dev/test
        // when lengths disagree, so we drive the dim-mismatch path through
        // the Result-returning variant instead. New callers should adopt
        // `try_cosine_similarity` to distinguish the failure modes.
        match try_cosine_similarity(&[], &normal) {
            Err(CosineSimilarityError::DimensionMismatch { left, right }) => {
                assert_eq!(left, 0);
                assert_eq!(right, 3);
            }
            other => panic!("expected DimensionMismatch, got {:?}", other),
        }
        match try_cosine_similarity(&[1.0, 2.0], &normal) {
            Err(CosineSimilarityError::DimensionMismatch { left, right }) => {
                assert_eq!(left, 2);
                assert_eq!(right, 3);
            }
            other => panic!("expected DimensionMismatch, got {:?}", other),
        }
    }

    // =========================================================================
    // F-025 REGRESSION TESTS (Sherlock investigation 2026-05-19)
    //
    // These tests assert that `try_cosine_similarity` distinguishes the three
    // failure modes (EmptyInput, DimensionMismatch, ZeroMagnitude) that the
    // legacy `cosine_similarity` collapsed into 0.0. If anyone re-collapses
    // them, these tests fail.
    //
    // Uses deterministic Gaussian samples via SplitMix64 + Box-Muller per
    // tier-compression test convention — NOT sin waves.
    // =========================================================================

    fn splitmix64_distance(seed: u64) -> u64 {
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn gaussian_sample_distance(seed: u64) -> f32 {
        let r1 = splitmix64_distance(seed.wrapping_mul(2)) >> 11;
        let r2 = splitmix64_distance(seed.wrapping_mul(2).wrapping_add(1)) >> 11;
        let u1 = (r1 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
        let u2 = (r2 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        let z = r * theta.cos();
        z.tanh() as f32
    }

    fn synthetic_gaussian(n: usize, seed: u64) -> Vec<f32> {
        (0..n)
            .map(|i| gaussian_sample_distance(seed.wrapping_add(i as u64)))
            .collect()
    }

    #[test]
    fn test_f025_try_cosine_similarity_empty_input_classified() {
        let result = try_cosine_similarity(&[], &[]);
        assert_eq!(result, Err(CosineSimilarityError::EmptyInput));
        let display = format!("{}", result.unwrap_err());
        assert!(display.contains("VECTOR_EMPTY_INPUT"), "got {display}");
    }

    #[test]
    fn test_f025_try_cosine_similarity_dim_mismatch_classified() {
        let a = synthetic_gaussian(128, 0xDEAD_BEEF);
        let b = synthetic_gaussian(256, 0xDEAD_BEEF);
        match try_cosine_similarity(&a, &b) {
            Err(CosineSimilarityError::DimensionMismatch { left, right }) => {
                assert_eq!(left, 128);
                assert_eq!(right, 256);
            }
            other => panic!("expected DimensionMismatch, got {:?}", other),
        }
        let err = try_cosine_similarity(&a, &b).unwrap_err();
        assert!(
            format!("{err}").contains("VECTOR_DIM_MISMATCH"),
            "got {err}"
        );
    }

    #[test]
    fn test_f025_try_cosine_similarity_zero_magnitude_classified() {
        let zero = vec![0.0_f32; 128];
        let nonzero = synthetic_gaussian(128, 0xABCDEF);
        // left zero, right finite
        match try_cosine_similarity(&zero, &nonzero) {
            Err(CosineSimilarityError::ZeroMagnitude {
                left_norm_sq,
                right_norm_sq,
            }) => {
                assert_eq!(left_norm_sq, 0.0);
                assert!(right_norm_sq > 0.0);
            }
            other => panic!("expected ZeroMagnitude, got {:?}", other),
        }
        // both zero
        match try_cosine_similarity(&zero, &zero) {
            Err(CosineSimilarityError::ZeroMagnitude {
                left_norm_sq,
                right_norm_sq,
            }) => {
                assert_eq!(left_norm_sq, 0.0);
                assert_eq!(right_norm_sq, 0.0);
            }
            other => panic!("expected ZeroMagnitude, got {:?}", other),
        }
    }

    #[test]
    fn test_f025_try_cosine_similarity_real_vectors_match_legacy() {
        // For valid same-dim inputs, the Result-returning variant must agree
        // with the legacy helper to within numerical tolerance.
        let a = synthetic_gaussian(1024, 0x123_456);
        let b = synthetic_gaussian(1024, 0x789_abc);
        let legacy = cosine_similarity(&a, &b);
        let try_ok = try_cosine_similarity(&a, &b).expect("valid inputs");
        assert!(
            (legacy - try_ok).abs() < 1e-5,
            "legacy={legacy} try={try_ok}"
        );
    }

    #[test]
    fn test_f025_try_cosine_similarity_raw_variant() {
        let a = synthetic_gaussian(64, 0xDEAD_BEEF);
        let b = a.clone();
        // Identical: raw cosine = 1.0
        let result = try_cosine_similarity_raw(&a, &b).expect("identical inputs valid");
        assert!((result - 1.0).abs() < 1e-5, "got {result}");

        // Dim mismatch surfaces structured error.
        let c = synthetic_gaussian(96, 0xDEAD_BEEF);
        match try_cosine_similarity_raw(&a, &c) {
            Err(CosineSimilarityError::DimensionMismatch { left, right }) => {
                assert_eq!(left, 64);
                assert_eq!(right, 96);
            }
            other => panic!("expected DimensionMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_f025_legacy_helper_still_returns_zero_on_zero_magnitude() {
        // Legacy contract: AP-10 zero-magnitude returns 0.0. This stays
        // intact for backwards-compat callers that cannot adopt Result.
        let zero = vec![0.0_f32; 128];
        let nonzero = synthetic_gaussian(128, 0x999);
        assert_eq!(cosine_similarity(&zero, &nonzero), 0.0);
        assert_eq!(cosine_similarity_raw(&zero, &nonzero), 0.0);
    }

    #[test]
    fn test_f025_error_variants_are_structured_screaming_snake_case() {
        let empty = CosineSimilarityError::EmptyInput;
        let mismatch = CosineSimilarityError::DimensionMismatch {
            left: 10,
            right: 20,
        };
        let zero = CosineSimilarityError::ZeroMagnitude {
            left_norm_sq: 0.0,
            right_norm_sq: 1.0,
        };
        assert!(format!("{empty}").contains("VECTOR_EMPTY_INPUT"));
        assert!(format!("{mismatch}").contains("VECTOR_DIM_MISMATCH"));
        assert!(format!("{zero}").contains("VECTOR_ZERO_MAGNITUDE"));
    }

    #[test]
    fn test_jaccard_similarity_cases() {
        // Identical
        let v = SparseVector::new(vec![0, 5, 10], vec![1.0, 1.0, 1.0]).unwrap();
        assert!((jaccard_similarity(&v, &v) - 1.0).abs() < 1e-5);

        // Partial overlap: {1,2} / {0,1,2,3} = 0.5
        let a = SparseVector::new(vec![0, 1, 2], vec![1.0, 1.0, 1.0]).unwrap();
        let b = SparseVector::new(vec![1, 2, 3], vec![1.0, 1.0, 1.0]).unwrap();
        assert!((jaccard_similarity(&a, &b) - 0.5).abs() < 1e-5);

        // Empty
        assert_eq!(
            jaccard_similarity(&SparseVector::empty(), &SparseVector::empty()),
            0.0
        );
    }

    #[test]
    fn test_max_sim_cases() {
        // Identical token sets
        let tokens = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];
        assert!((max_sim(&tokens, &tokens) - 1.0).abs() < 1e-5);

        // Partial match: (1.0 + 0.5) / 2 = 0.75
        let query = vec![vec![1.0, 0.0, 0.0], vec![0.0, 0.0, 1.0]];
        let memory = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];
        assert!((max_sim(&query, &memory) - 0.75).abs() < 1e-5);

        // Empty
        let empty: Vec<Vec<f32>> = vec![];
        assert_eq!(max_sim(&empty, &tokens), 0.0);
        assert_eq!(max_sim(&tokens, &empty), 0.0);
    }

    #[test]
    fn test_compute_similarity_for_space_dispatch() {
        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        // Semantic (dense cosine)
        query.e1_semantic = vec![1.0; 1024];
        memory.e1_semantic = vec![1.0; 1024];
        assert!(
            (compute_similarity_for_space(Embedder::Semantic, &query, &memory) - 1.0).abs() < 1e-5
        );

        // Sparse (jaccard)
        query.e6_sparse = SparseVector::new(vec![0, 5, 10], vec![1.0, 1.0, 1.0]).unwrap();
        memory.e6_sparse = SparseVector::new(vec![0, 5, 10], vec![1.0, 1.0, 1.0]).unwrap();
        assert!(
            (compute_similarity_for_space(Embedder::Sparse, &query, &memory) - 1.0).abs() < 1e-5
        );

        // Late interaction (MaxSim)
        query.e12_late_interaction = vec![vec![1.0; 128], vec![0.5; 128]];
        memory.e12_late_interaction = vec![vec![1.0; 128], vec![0.5; 128]];
        assert!(
            (compute_similarity_for_space(Embedder::LateInteraction, &query, &memory) - 1.0).abs()
                < 1e-5
        );

        // Entity uses cosine, orthogonal = 0.5
        query.e11_entity = vec![0.0; 768];
        memory.e11_entity = vec![0.0; 768];
        query.e11_entity[0] = 1.0;
        memory.e11_entity[1] = 1.0;
        assert!(
            (compute_similarity_for_space(Embedder::Entity, &query, &memory) - 0.5).abs() < 1e-5
        );
    }

    #[test]
    fn test_edge_cases_no_nan_or_overflow() {
        // Very small values
        let small = vec![1e-20_f32; 3];
        let sim = cosine_similarity(&small, &small);
        assert!(!sim.is_nan() && !sim.is_infinite() && (0.0..=1.0).contains(&sim));

        // Very large values
        let large = vec![1e19_f32; 3];
        let sim = cosine_similarity(&large, &large);
        assert!(!sim.is_nan() && !sim.is_infinite() && (0.0..=1.0).contains(&sim));

        // Single-token opposite MaxSim
        let sim = max_sim(&[vec![1.0_f32]], &[vec![-1.0_f32]]);
        assert!(sim.abs() < 1e-5 && sim >= 0.0);

        // All 13 spaces with zeroed fingerprints: verify per-space dispatch
        let zeroed = SemanticFingerprint::zeroed();
        for embedder in Embedder::all() {
            let score = compute_similarity_for_space(embedder, &zeroed, &zeroed);
            if embedder == Embedder::Causal {
                // CORE-H1: E5 returns -1.0 sentinel without direction
                assert_eq!(
                    score, -1.0,
                    "E5 should return -1.0 sentinel without direction"
                );
            } else {
                assert!(
                    !score.is_nan() && !score.is_infinite() && (0.0..=1.0).contains(&score),
                    "{} score {} out of range",
                    embedder.name(),
                    score
                );
            }
        }
    }

    #[test]
    fn test_direction_aware_e5_ap77() {
        use crate::causal::asymmetric::CausalDirection;

        let mut query = SemanticFingerprint::zeroed();
        let mut memory = SemanticFingerprint::zeroed();

        query.e5_causal_as_cause = vec![1.0; 768];
        query.e5_causal_as_effect = vec![0.5; 768];
        memory.e5_causal_as_cause = vec![1.0; 768];
        memory.e5_causal_as_effect = vec![0.5; 768];

        // Unknown direction: E5 returns -1.0 sentinel per AP-77 + CORE-H1 fix
        assert_eq!(
            compute_similarity_for_space_with_direction(
                Embedder::Causal,
                &query,
                &memory,
                CausalDirection::Unknown
            ),
            -1.0,
        );

        // Known direction still returns the retired/no-signal sentinel while E5 is disabled.
        let cause_sim = compute_similarity_for_space_with_direction(
            Embedder::Causal,
            &query,
            &memory,
            CausalDirection::Cause,
        );
        assert_eq!(cause_sim, -1.0);

        // Non-E5 embedders ignore direction
        query.e1_semantic = vec![1.0; 1024];
        memory.e1_semantic = vec![1.0; 1024];
        let sym = compute_similarity_for_space(Embedder::Semantic, &query, &memory);
        let with_cause = compute_similarity_for_space_with_direction(
            Embedder::Semantic,
            &query,
            &memory,
            CausalDirection::Cause,
        );
        assert!((sym - with_cause).abs() < 1e-5);
    }

    #[test]
    fn test_direction_modifier_values() {
        use crate::causal::asymmetric::direction_mod;

        assert!((direction_mod::CAUSE_TO_EFFECT - 1.2).abs() < 1e-5);
        assert!((direction_mod::EFFECT_TO_CAUSE - 0.8).abs() < 1e-5);
        assert!((direction_mod::SAME_DIRECTION - 1.0).abs() < 1e-5);
        let ratio = direction_mod::CAUSE_TO_EFFECT / direction_mod::EFFECT_TO_CAUSE;
        assert!((ratio - 1.5).abs() < 1e-5);
    }
}
