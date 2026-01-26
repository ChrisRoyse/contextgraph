//! Transitive Causal Chain Reasoning
//!
//! Implements multi-hop causal chain scoring and abductive reasoning.
//!
//! # Overview
//!
//! Causal chains A → B → C require transitive scoring that accounts for:
//! - Attenuation per hop (distant causes have less direct influence)
//! - Proper direction alignment across hops
//! - Abductive reasoning (finding most likely cause given effect)
//!
//! # Constitution Reference
//!
//! - E5 asymmetric similarity: sim = base_cos × direction_mod × (0.7 + 0.3 × intervention_overlap)
//! - Direction modifiers: cause→effect=1.2, effect→cause=0.8
//!
//! # Research Basis
//!
//! Chain attenuation inspired by causal inference literature:
//! - Pearl's Causality: propagation strength diminishes across mediators
//! - Do-calculus: interventional effects attenuate through mechanism chains

use std::cmp::Ordering;
use std::collections::HashMap;

use uuid::Uuid;

use super::asymmetric::{
    compute_asymmetric_similarity_simple, compute_e5_asymmetric_fingerprint_similarity,
    direction_mod, CausalDirection,
};
use crate::types::fingerprint::SemanticFingerprint;

/// Attenuation factor per hop in causal chains.
///
/// Each hop reduces the overall chain strength by this factor,
/// preventing infinite chain inflation and modeling real-world
/// causal distance effects.
pub const HOP_ATTENUATION: f32 = 0.9;

/// Minimum chain score to consider valid (prevents noise chains).
pub const MIN_CHAIN_SCORE: f32 = 0.1;

/// Maximum chain length to consider (prevents combinatorial explosion).
pub const MAX_CHAIN_LENGTH: usize = 10;

/// Represents a single hop in a causal chain.
#[derive(Debug, Clone)]
pub struct CausalHop {
    /// Base semantic similarity for this hop
    pub base_similarity: f32,
    /// Direction of the source entity
    pub from_direction: CausalDirection,
    /// Direction of the target entity
    pub to_direction: CausalDirection,
}

impl CausalHop {
    /// Create a new causal hop.
    pub fn new(
        base_similarity: f32,
        from_direction: CausalDirection,
        to_direction: CausalDirection,
    ) -> Self {
        Self {
            base_similarity,
            from_direction,
            to_direction,
        }
    }

    /// Compute the asymmetric similarity for this hop.
    pub fn hop_score(&self) -> f32 {
        compute_asymmetric_similarity_simple(
            self.base_similarity,
            self.from_direction,
            self.to_direction,
        )
    }
}

/// Compute transitive causal score for a chain A → B → C → ... → Z.
///
/// # Formula
///
/// ```text
/// chain_score = ∏(hop_score_i × ATTENUATION^i)
/// ```
///
/// Each hop's score is multiplied by an attenuation factor raised to the
/// hop index, ensuring distant causes contribute less to the overall score.
///
/// # Arguments
///
/// * `hops` - Slice of causal hops representing the chain
///
/// # Returns
///
/// Transitive chain score in (0, 1]. Returns 1.0 for empty chains.
///
/// # Example
///
/// ```
/// use context_graph_core::causal::chain::{compute_chain_score, CausalHop};
/// use context_graph_core::causal::asymmetric::CausalDirection;
///
/// let hops = vec![
///     CausalHop::new(0.8, CausalDirection::Cause, CausalDirection::Effect),
///     CausalHop::new(0.7, CausalDirection::Cause, CausalDirection::Effect),
/// ];
///
/// let score = compute_chain_score(&hops);
/// // Each hop is attenuated: hop1 * 0.9^0 * hop2 * 0.9^1
/// assert!(score > 0.0 && score < 1.0);
/// ```
pub fn compute_chain_score(hops: &[CausalHop]) -> f32 {
    if hops.is_empty() {
        return 1.0;
    }

    if hops.len() > MAX_CHAIN_LENGTH {
        return 0.0; // Reject overly long chains
    }

    let mut chain_score = 1.0;
    for (i, hop) in hops.iter().enumerate() {
        let hop_score = hop.hop_score();
        let attenuation = HOP_ATTENUATION.powi(i as i32);
        chain_score *= hop_score * attenuation;
    }

    chain_score.max(0.0)
}

/// Compute transitive score from raw similarity/direction tuples.
///
/// Convenience function when you have raw data without constructing CausalHop objects.
///
/// # Arguments
///
/// * `hops` - Slice of (base_sim, from_direction, to_direction) tuples
///
/// # Returns
///
/// Transitive chain score.
pub fn compute_chain_score_raw(hops: &[(f32, CausalDirection, CausalDirection)]) -> f32 {
    let causal_hops: Vec<CausalHop> = hops
        .iter()
        .map(|(base_sim, from_dir, to_dir)| CausalHop::new(*base_sim, *from_dir, *to_dir))
        .collect();
    compute_chain_score(&causal_hops)
}

/// Causal pair for chain scoring with embeddings.
#[derive(Debug, Clone)]
pub struct CausalPairEmbedding {
    /// Unique identifier
    pub id: Uuid,
    /// Cause embedding
    pub cause_embedding: Vec<f32>,
    /// Effect embedding
    pub effect_embedding: Vec<f32>,
    /// Causal strength (0.0 to 1.0)
    pub strength: f32,
}

impl CausalPairEmbedding {
    /// Create a new causal pair with embeddings.
    pub fn new(
        id: Uuid,
        cause_embedding: Vec<f32>,
        effect_embedding: Vec<f32>,
        strength: f32,
    ) -> Self {
        Self {
            id,
            cause_embedding,
            effect_embedding,
            strength,
        }
    }
}

/// Score a causal chain using embedded representations.
///
/// For a chain of causal pairs [P1, P2, P3, ...], validates that:
/// - Effect of P1 connects to Cause of P2
/// - Effect of P2 connects to Cause of P3
/// - etc.
///
/// # Arguments
///
/// * `chain` - Ordered sequence of causal pairs forming a chain
///
/// # Returns
///
/// Average hop similarity across the chain. Returns 1.0 for single-element chains.
///
/// # Example
///
/// ```
/// use context_graph_core::causal::chain::{score_causal_chain, CausalPairEmbedding};
/// use uuid::Uuid;
///
/// let chain = vec![
///     CausalPairEmbedding::new(
///         Uuid::new_v4(),
///         vec![0.1, 0.2, 0.3], // cause
///         vec![0.4, 0.5, 0.6], // effect
///         0.8,
///     ),
///     CausalPairEmbedding::new(
///         Uuid::new_v4(),
///         vec![0.4, 0.5, 0.6], // cause (should match prev effect)
///         vec![0.7, 0.8, 0.9], // effect
///         0.7,
///     ),
/// ];
///
/// let score = score_causal_chain(&chain);
/// assert!(score >= 0.0);
/// ```
pub fn score_causal_chain(chain: &[CausalPairEmbedding]) -> f32 {
    if chain.len() < 2 {
        return 1.0;
    }

    let mut total_score = 0.0;
    let mut valid_hops = 0;

    for window in chain.windows(2) {
        let current = &window[0];
        let next = &window[1];

        // Effect of current should semantically match cause of next
        let hop_sim = cosine_similarity(&current.effect_embedding, &next.cause_embedding);

        // Apply cause→effect direction boost (effect connecting to cause)
        let boosted_sim = hop_sim * direction_mod::CAUSE_TO_EFFECT;

        total_score += boosted_sim;
        valid_hops += 1;
    }

    if valid_hops > 0 {
        total_score / valid_hops as f32
    } else {
        0.0
    }
}

/// Score a causal chain with attenuation applied.
///
/// Similar to `score_causal_chain` but applies hop attenuation,
/// giving closer causes more weight than distant ones.
///
/// # Returns
///
/// Attenuated chain score in (0, 1].
pub fn score_causal_chain_attenuated(chain: &[CausalPairEmbedding]) -> f32 {
    if chain.len() < 2 {
        return 1.0;
    }

    let mut total_score = 0.0;
    let mut total_weight = 0.0;

    for (i, window) in chain.windows(2).enumerate() {
        let current = &window[0];
        let next = &window[1];

        let hop_sim = cosine_similarity(&current.effect_embedding, &next.cause_embedding);
        let boosted_sim = hop_sim * direction_mod::CAUSE_TO_EFFECT;

        let weight = HOP_ATTENUATION.powi(i as i32);
        total_score += boosted_sim * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        total_score / total_weight
    } else {
        0.0
    }
}

// =============================================================================
// Abductive Reasoning
// =============================================================================

/// Result of abductive reasoning (ranking causes by likelihood).
#[derive(Debug, Clone)]
pub struct AbductionResult {
    /// Candidate cause ID
    pub cause_id: Uuid,
    /// Abductive score (likelihood this is the cause)
    pub score: f32,
    /// Raw similarity before dampening
    pub raw_similarity: f32,
}

/// Rank candidate causes by abductive reasoning given an observed effect.
///
/// Abduction: Given effect E, find most likely cause C.
///
/// Uses E5 asymmetric pairing with effect→cause direction (0.8 modifier)
/// to dampen scores, reflecting the inherent uncertainty in backward inference.
///
/// # Arguments
///
/// * `effect_fingerprint` - Fingerprint of the observed effect
/// * `candidate_causes` - Vector of (UUID, fingerprint) pairs for candidate causes
///
/// # Returns
///
/// Vector of AbductionResult sorted by score (highest first).
///
/// # Example
///
/// ```
/// # #[cfg(feature = "test-utils")]
/// # {
/// use context_graph_core::causal::chain::rank_causes_by_abduction;
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
/// use uuid::Uuid;
///
/// let effect = SemanticFingerprint::zeroed();
/// let candidates = vec![
///     (Uuid::new_v4(), SemanticFingerprint::zeroed()),
///     (Uuid::new_v4(), SemanticFingerprint::zeroed()),
/// ];
///
/// let ranked = rank_causes_by_abduction(&effect, &candidates);
/// assert_eq!(ranked.len(), 2);
/// // Results are sorted by score descending
/// assert!(ranked[0].score >= ranked[1].score);
/// # }
/// ```
pub fn rank_causes_by_abduction(
    effect_fingerprint: &SemanticFingerprint,
    candidate_causes: &[(Uuid, SemanticFingerprint)],
) -> Vec<AbductionResult> {
    let mut results: Vec<AbductionResult> = candidate_causes
        .iter()
        .map(|(id, cause_fp)| {
            // Effect looking for cause: use effect→cause direction
            // query_is_cause = false because we ARE the effect looking for causes
            let raw_sim =
                compute_e5_asymmetric_fingerprint_similarity(effect_fingerprint, cause_fp, false);

            // Apply abductive dampening (effect→cause modifier)
            let adjusted_score = raw_sim * direction_mod::EFFECT_TO_CAUSE;

            AbductionResult {
                cause_id: *id,
                score: adjusted_score,
                raw_similarity: raw_sim,
            }
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    results
}

/// Rank candidate causes using raw embeddings (without SemanticFingerprint).
///
/// Simplified version for cases where you have raw E5 embeddings.
///
/// # Arguments
///
/// * `effect_embedding` - E5 embedding of the observed effect
/// * `candidate_causes` - Vector of (UUID, embedding) pairs
///
/// # Returns
///
/// Vector of (UUID, score) pairs sorted by score descending.
pub fn rank_causes_by_abduction_raw(
    effect_embedding: &[f32],
    candidate_causes: &[(Uuid, Vec<f32>)],
) -> Vec<(Uuid, f32)> {
    let mut results: Vec<(Uuid, f32)> = candidate_causes
        .iter()
        .map(|(id, cause_emb)| {
            let raw_sim = cosine_similarity(effect_embedding, cause_emb);
            let adjusted_score = raw_sim * direction_mod::EFFECT_TO_CAUSE;
            (*id, adjusted_score.max(0.0))
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    results
}

// =============================================================================
// Predictive Reasoning (Forward Causal Search)
// =============================================================================

/// Result of predictive reasoning (ranking effects by likelihood).
#[derive(Debug, Clone)]
pub struct PredictionResult {
    /// Candidate effect ID
    pub effect_id: Uuid,
    /// Predictive score (likelihood this is the effect)
    pub score: f32,
    /// Raw similarity before boost
    pub raw_similarity: f32,
}

/// Rank candidate effects by predictive reasoning given a cause.
///
/// Prediction: Given cause C, find most likely effects E.
///
/// Uses E5 asymmetric pairing with cause→effect direction (1.2 modifier)
/// to boost scores, reflecting the forward causal relationship.
///
/// # Arguments
///
/// * `cause_fingerprint` - Fingerprint of the observed cause
/// * `candidate_effects` - Vector of (UUID, fingerprint) pairs for candidate effects
///
/// # Returns
///
/// Vector of PredictionResult sorted by score (highest first).
pub fn rank_effects_by_prediction(
    cause_fingerprint: &SemanticFingerprint,
    candidate_effects: &[(Uuid, SemanticFingerprint)],
) -> Vec<PredictionResult> {
    let mut results: Vec<PredictionResult> = candidate_effects
        .iter()
        .map(|(id, effect_fp)| {
            // Cause looking for effects: use cause→effect direction
            // query_is_cause = true because we ARE the cause looking for effects
            let raw_sim =
                compute_e5_asymmetric_fingerprint_similarity(cause_fingerprint, effect_fp, true);

            // Apply predictive boost (cause→effect modifier)
            let adjusted_score = (raw_sim * direction_mod::CAUSE_TO_EFFECT).clamp(0.0, 1.0);

            PredictionResult {
                effect_id: *id,
                score: adjusted_score,
                raw_similarity: raw_sim,
            }
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    results
}

/// Rank candidate effects using raw embeddings (without SemanticFingerprint).
///
/// Simplified version for cases where you have raw E5 embeddings.
///
/// # Arguments
///
/// * `cause_embedding` - E5 embedding of the observed cause
/// * `candidate_effects` - Vector of (UUID, embedding) pairs
///
/// # Returns
///
/// Vector of (UUID, score) pairs sorted by score descending.
pub fn rank_effects_by_prediction_raw(
    cause_embedding: &[f32],
    candidate_effects: &[(Uuid, Vec<f32>)],
) -> Vec<(Uuid, f32)> {
    let mut results: Vec<(Uuid, f32)> = candidate_effects
        .iter()
        .map(|(id, effect_emb)| {
            let raw_sim = cosine_similarity(cause_embedding, effect_emb);
            let adjusted_score = (raw_sim * direction_mod::CAUSE_TO_EFFECT).clamp(0.0, 1.0);
            (*id, adjusted_score)
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    results
}

// =============================================================================
// Deductive Chain Building
// =============================================================================

/// Build a causal chain from root cause to final effect.
///
/// Given a set of causal pairs and a starting cause, attempts to construct
/// the longest valid chain that follows cause→effect relationships.
///
/// # Arguments
///
/// * `pairs` - Available causal pairs (as embeddings)
/// * `start_cause_embedding` - Embedding of the starting cause
/// * `similarity_threshold` - Minimum similarity to consider a valid hop
///
/// # Returns
///
/// Vector of pair IDs forming the chain, in order from start to end.
pub fn build_causal_chain(
    pairs: &HashMap<Uuid, CausalPairEmbedding>,
    start_cause_embedding: &[f32],
    similarity_threshold: f32,
) -> Vec<Uuid> {
    let mut chain = Vec::new();
    let mut current_effect = start_cause_embedding.to_vec();
    let mut visited: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    for _ in 0..MAX_CHAIN_LENGTH {
        // Find best matching pair whose cause matches current_effect
        let mut best_match: Option<(Uuid, f32)> = None;

        for (id, pair) in pairs.iter() {
            if visited.contains(id) {
                continue;
            }

            let sim = cosine_similarity(&current_effect, &pair.cause_embedding);
            if sim >= similarity_threshold {
                if best_match.is_none() || sim > best_match.unwrap().1 {
                    best_match = Some((*id, sim));
                }
            }
        }

        match best_match {
            Some((id, _)) => {
                chain.push(id);
                visited.insert(id);
                // Move to the effect of this pair
                current_effect = pairs[&id].effect_embedding.clone();
            }
            None => break, // No valid continuation found
        }
    }

    chain
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Compute cosine similarity between two f32 slices.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let norm_product = (norm_a * norm_b).sqrt();

    if norm_product < f32::EPSILON {
        0.0
    } else {
        (dot / norm_product).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Chain Score Tests
    // ============================================================================

    #[test]
    fn test_empty_chain_returns_one() {
        let score = compute_chain_score(&[]);
        assert_eq!(score, 1.0);
        println!("[VERIFIED] Empty chain returns 1.0");
    }

    #[test]
    fn test_single_hop_chain() {
        let hops = vec![CausalHop::new(
            0.8,
            CausalDirection::Cause,
            CausalDirection::Effect,
        )];

        let score = compute_chain_score(&hops);
        // Single hop: 0.8 * 1.2 * 0.85 * 0.9^0 = 0.8 * 1.02 = 0.816
        // (direction_mod=1.2, overlap_factor=0.85, attenuation=1.0)
        assert!(score > 0.7 && score < 1.0);
        println!("[VERIFIED] Single hop chain: {}", score);
    }

    #[test]
    fn test_multi_hop_attenuation() {
        let hops = vec![
            CausalHop::new(0.8, CausalDirection::Cause, CausalDirection::Effect),
            CausalHop::new(0.8, CausalDirection::Cause, CausalDirection::Effect),
            CausalHop::new(0.8, CausalDirection::Cause, CausalDirection::Effect),
        ];

        let score = compute_chain_score(&hops);
        // Each subsequent hop is attenuated by 0.9^i
        // Score should be less than single hop but positive
        assert!(score > 0.0 && score < 0.7);
        println!("[VERIFIED] Multi-hop attenuation: {}", score);
    }

    #[test]
    fn test_direction_affects_chain_score() {
        // Cause→Effect chain (amplified)
        let forward_hops = vec![
            CausalHop::new(0.8, CausalDirection::Cause, CausalDirection::Effect),
            CausalHop::new(0.8, CausalDirection::Cause, CausalDirection::Effect),
        ];

        // Effect→Cause chain (dampened)
        let backward_hops = vec![
            CausalHop::new(0.8, CausalDirection::Effect, CausalDirection::Cause),
            CausalHop::new(0.8, CausalDirection::Effect, CausalDirection::Cause),
        ];

        let forward_score = compute_chain_score(&forward_hops);
        let backward_score = compute_chain_score(&backward_hops);

        assert!(forward_score > backward_score);
        println!(
            "[VERIFIED] Forward chain ({}) > Backward chain ({})",
            forward_score, backward_score
        );
    }

    #[test]
    fn test_long_chain_rejected() {
        let hops: Vec<CausalHop> = (0..15)
            .map(|_| CausalHop::new(0.9, CausalDirection::Cause, CausalDirection::Effect))
            .collect();

        let score = compute_chain_score(&hops);
        assert_eq!(score, 0.0);
        println!("[VERIFIED] Overly long chain rejected");
    }

    // ============================================================================
    // Chain Scoring with Embeddings Tests
    // ============================================================================

    #[test]
    fn test_score_causal_chain_single_pair() {
        let chain = vec![CausalPairEmbedding::new(
            Uuid::new_v4(),
            vec![0.1, 0.2, 0.3],
            vec![0.4, 0.5, 0.6],
            0.8,
        )];

        let score = score_causal_chain(&chain);
        assert_eq!(score, 1.0); // Single pair = no hops = 1.0
        println!("[VERIFIED] Single pair chain: {}", score);
    }

    #[test]
    fn test_score_causal_chain_connected() {
        // Create a chain where effect of P1 matches cause of P2
        let chain = vec![
            CausalPairEmbedding::new(
                Uuid::new_v4(),
                vec![0.1, 0.2, 0.3],
                vec![0.4, 0.5, 0.6], // effect
                0.8,
            ),
            CausalPairEmbedding::new(
                Uuid::new_v4(),
                vec![0.4, 0.5, 0.6], // cause = prev effect (perfect match)
                vec![0.7, 0.8, 0.9],
                0.7,
            ),
        ];

        let score = score_causal_chain(&chain);
        // Perfect connection: cosine sim = 1.0, boosted by 1.2
        assert!(score > 1.0); // Can exceed 1.0 due to direction boost
        println!("[VERIFIED] Connected chain: {}", score);
    }

    #[test]
    fn test_score_causal_chain_disconnected() {
        // Create a chain where effect of P1 is orthogonal to cause of P2
        let chain = vec![
            CausalPairEmbedding::new(
                Uuid::new_v4(),
                vec![0.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0], // effect
                0.8,
            ),
            CausalPairEmbedding::new(
                Uuid::new_v4(),
                vec![0.0, 1.0, 0.0], // cause = orthogonal to prev effect
                vec![0.0, 0.0, 1.0],
                0.7,
            ),
        ];

        let score = score_causal_chain(&chain);
        // Orthogonal vectors: cosine sim = 0
        assert!(score.abs() < 0.1);
        println!("[VERIFIED] Disconnected chain: {}", score);
    }

    // ============================================================================
    // Abduction Tests
    // ============================================================================

    #[test]
    fn test_rank_causes_raw_basic() {
        let effect = vec![0.5, 0.5, 0.0];

        let candidates = vec![
            (Uuid::new_v4(), vec![0.5, 0.5, 0.0]), // Same direction
            (Uuid::new_v4(), vec![0.0, 0.0, 1.0]), // Orthogonal
            (Uuid::new_v4(), vec![0.3, 0.3, 0.3]), // Partial match
        ];

        let ranked = rank_causes_by_abduction_raw(&effect, &candidates);

        assert_eq!(ranked.len(), 3);
        // Same direction should rank highest
        assert_eq!(ranked[0].0, candidates[0].0);
        // Orthogonal should rank lowest
        assert_eq!(ranked[2].0, candidates[1].0);
        println!(
            "[VERIFIED] Abduction ranking: {:?}",
            ranked.iter().map(|(_, s)| *s).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_abduction_applies_dampening() {
        let effect = vec![1.0, 0.0, 0.0];
        let candidates = vec![(Uuid::new_v4(), vec![1.0, 0.0, 0.0])];

        let ranked = rank_causes_by_abduction_raw(&effect, &candidates);

        // Perfect match cosine = 1.0, dampened by 0.8 (effect→cause)
        assert!((ranked[0].1 - 0.8).abs() < 0.01);
        println!("[VERIFIED] Abduction dampening applied: {}", ranked[0].1);
    }

    // ============================================================================
    // Helper Tests
    // ============================================================================

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-6);

        println!("[VERIFIED] Cosine similarity helper works correctly");
    }

    #[test]
    fn test_build_causal_chain() {
        let mut pairs = HashMap::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        // Chain: start → P1 → P2 → P3
        pairs.insert(
            id1,
            CausalPairEmbedding::new(
                id1,
                vec![0.1, 0.0, 0.0], // cause
                vec![0.5, 0.5, 0.0], // effect
                0.8,
            ),
        );
        pairs.insert(
            id2,
            CausalPairEmbedding::new(
                id2,
                vec![0.5, 0.5, 0.0], // cause = P1 effect
                vec![0.0, 1.0, 0.0], // effect
                0.7,
            ),
        );
        pairs.insert(
            id3,
            CausalPairEmbedding::new(
                id3,
                vec![0.0, 1.0, 0.0], // cause = P2 effect
                vec![0.0, 0.0, 1.0], // effect
                0.6,
            ),
        );

        // Start with embedding that matches P1's cause
        let start = vec![0.1, 0.0, 0.0];
        let chain = build_causal_chain(&pairs, &start, 0.5);

        // Should find all three pairs in order
        assert!(!chain.is_empty());
        println!("[VERIFIED] Chain building: {} pairs found", chain.len());
    }
}
