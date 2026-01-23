//! E4 Hybrid Session Clustering Metrics.
//!
//! This module provides metrics for evaluating the E4 hybrid session+position encoding.
//! The E4 embedder uses both session signatures and positional encodings to create
//! embeddings that cluster by session while maintaining ordering within sessions.
//!
//! ## Key Metrics
//!
//! - **Session Clustering**: How well E4 separates different sessions
//! - **Position Ordering**: How well E4 preserves position order within sessions
//! - **Hybrid Effectiveness**: Improvement over legacy E4 and timestamp baseline
//!
//! ## Targets (from CLAUDE.md)
//!
//! - Session separation ratio: ≥ 2.0x
//! - Intra-session ordering accuracy: ≥ 80%
//! - Before/after symmetry: ≥ 0.8
//! - Hybrid vs legacy improvement: ≥ +10%

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::datasets::temporal_sessions::TemporalSession;

// ============================================================================
// Core Metrics Structures
// ============================================================================

/// E4 Hybrid Session Clustering Metrics.
///
/// Top-level structure containing all metrics for evaluating
/// the E4 hybrid session+position encoding implementation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E4HybridSessionMetrics {
    /// Session clustering quality metrics.
    pub clustering: SessionClusteringMetrics,
    /// Position ordering within sessions metrics.
    pub ordering: IntraSessionOrderingMetrics,
    /// Hybrid mode effectiveness vs baselines.
    pub hybrid_effectiveness: HybridEffectivenessMetrics,
    /// Composite score combining all metrics.
    pub composite: CompositeE4HybridMetrics,
}

impl E4HybridSessionMetrics {
    /// Check if all targets are met.
    pub fn all_targets_met(&self) -> bool {
        self.clustering.session_separation_ratio >= 2.0
            && self.ordering.ordering_accuracy >= 0.80
            && self.ordering.symmetry_score >= 0.80
    }

    /// Get the overall quality score (0.0-1.0).
    pub fn overall_score(&self) -> f64 {
        self.composite.overall_score
    }
}

/// Session clustering quality metrics.
///
/// Measures how well E4 embeddings cluster by session - chunks from
/// the same session should have higher E4 similarity than chunks
/// from different sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionClusteringMetrics {
    /// Average E4 cosine similarity within same session.
    pub intra_session_similarity: f64,
    /// Average E4 cosine similarity across different sessions.
    pub inter_session_similarity: f64,
    /// Separation ratio: intra / inter (target: ≥ 2.0).
    ///
    /// A ratio of 2.0 means intra-session similarity is twice
    /// the inter-session similarity, indicating good clustering.
    pub session_separation_ratio: f64,
    /// Silhouette score for session clustering (-1.0 to 1.0).
    ///
    /// Higher values indicate better-defined clusters.
    pub silhouette_score: f64,
    /// Session boundary detection F1 score.
    ///
    /// How well E4 can classify whether two chunks are from
    /// the same session based on their similarity.
    pub boundary_f1: f64,
    /// Number of sessions evaluated.
    pub num_sessions: usize,
    /// Number of chunk pairs evaluated.
    pub num_pairs_evaluated: usize,
}

impl SessionClusteringMetrics {
    /// Check if clustering quality meets the target.
    pub fn meets_target(&self) -> bool {
        self.session_separation_ratio >= 2.0
    }

    /// Get a normalized clustering score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        // Normalize separation ratio: 1.0 -> 0.0, 2.0 -> 0.5, 3.0+ -> 1.0
        let separation_score = ((self.session_separation_ratio - 1.0) / 2.0).clamp(0.0, 1.0);
        // Silhouette is already -1 to 1, map to 0-1
        let silhouette_norm = (self.silhouette_score + 1.0) / 2.0;
        // Weighted average
        0.5 * separation_score + 0.3 * silhouette_norm + 0.2 * self.boundary_f1
    }
}

/// Intra-session position ordering metrics.
///
/// Measures how well E4 embeddings preserve position ordering
/// within a single session - chunks closer together in sequence
/// should have higher E4 similarity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntraSessionOrderingMetrics {
    /// Fraction of position pairs correctly ordered within sessions.
    ///
    /// For pairs (A, B) where A comes before B, checks if
    /// sim(anchor, A) and sim(anchor, B) have the expected relationship.
    pub ordering_accuracy: f64,
    /// Kendall's tau correlation for position order.
    ///
    /// Measures rank correlation between sequence position and
    /// E4 similarity to anchor. Range: -1.0 to 1.0.
    pub kendalls_tau: f64,
    /// Before retrieval accuracy.
    ///
    /// For "get chunks before X" queries, how many retrieved
    /// chunks are actually before X in sequence.
    pub before_accuracy: f64,
    /// After retrieval accuracy.
    ///
    /// For "get chunks after X" queries, how many retrieved
    /// chunks are actually after X in sequence.
    pub after_accuracy: f64,
    /// Symmetry score: 1.0 - |before - after| (target: ≥ 0.8).
    ///
    /// Measures whether before/after queries have similar accuracy.
    pub symmetry_score: f64,
    /// Mean reciprocal rank for position-based retrieval.
    pub position_mrr: f64,
    /// Number of sessions evaluated.
    pub num_sessions: usize,
    /// Number of queries evaluated.
    pub num_queries: usize,
}

impl IntraSessionOrderingMetrics {
    /// Check if ordering meets targets.
    pub fn meets_targets(&self) -> bool {
        self.ordering_accuracy >= 0.80 && self.symmetry_score >= 0.80
    }

    /// Get a normalized ordering score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        // Weighted average of ordering metrics
        0.4 * self.ordering_accuracy
            + 0.2 * self.symmetry_score
            + 0.2 * ((self.kendalls_tau + 1.0) / 2.0) // Normalize tau from -1..1 to 0..1
            + 0.2 * self.position_mrr
    }
}

/// Hybrid mode effectiveness vs baselines.
///
/// Compares the E4 hybrid (session+position) encoding against:
/// - Legacy E4 (position-only, no session awareness)
/// - Timestamp baseline (raw timestamp embedding)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HybridEffectivenessMetrics {
    /// Improvement over legacy E4 (position-only).
    ///
    /// Calculated as: (hybrid_separation - legacy_separation) / legacy_separation
    /// Target: ≥ +10% (+0.10)
    pub vs_legacy_improvement: f64,
    /// Improvement over timestamp baseline.
    ///
    /// Calculated as: (hybrid_separation - timestamp_separation) / timestamp_separation
    pub vs_timestamp_improvement: f64,
    /// Session signature distinctness.
    ///
    /// Measures how orthogonal session signatures are to each other.
    /// Range: 0.0 (identical) to 1.0 (fully orthogonal).
    pub signature_distinctness: f64,
    /// Position encoding preservation.
    ///
    /// How much of the position information is preserved after
    /// combining with session signature.
    pub position_preservation: f64,
    /// Legacy E4 separation ratio (for reference).
    pub legacy_separation_ratio: f64,
    /// Timestamp baseline separation ratio (for reference).
    pub timestamp_separation_ratio: f64,
}

impl HybridEffectivenessMetrics {
    /// Check if hybrid improvement meets target.
    pub fn meets_target(&self) -> bool {
        self.vs_legacy_improvement >= 0.10
    }

    /// Get a normalized effectiveness score (0.0-1.0).
    pub fn normalized_score(&self) -> f64 {
        // Cap improvements at 100% for normalization
        let legacy_score = (self.vs_legacy_improvement / 0.5).clamp(0.0, 1.0);
        let distinctness_score = self.signature_distinctness;
        0.6 * legacy_score + 0.4 * distinctness_score
    }
}

/// Composite E4 Hybrid Metrics.
///
/// Aggregated scores combining clustering, ordering, and effectiveness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompositeE4HybridMetrics {
    /// Overall score (0.0-1.0).
    ///
    /// Weighted combination:
    /// - 40% session clustering
    /// - 40% position ordering
    /// - 20% hybrid effectiveness
    pub overall_score: f64,
    /// Clustering component score.
    pub clustering_score: f64,
    /// Ordering component score.
    pub ordering_score: f64,
    /// Effectiveness component score.
    pub effectiveness_score: f64,
    /// All validation targets met.
    pub all_targets_met: bool,
    /// Number of targets met out of total.
    pub targets_met_count: usize,
    /// Total number of targets.
    pub targets_total: usize,
}

impl CompositeE4HybridMetrics {
    /// Compute composite metrics from components.
    pub fn compute(
        clustering: &SessionClusteringMetrics,
        ordering: &IntraSessionOrderingMetrics,
        effectiveness: &HybridEffectivenessMetrics,
    ) -> Self {
        let clustering_score = clustering.normalized_score();
        let ordering_score = ordering.normalized_score();
        let effectiveness_score = effectiveness.normalized_score();

        let overall_score = 0.4 * clustering_score + 0.4 * ordering_score + 0.2 * effectiveness_score;

        // Count targets met
        let targets = [
            clustering.session_separation_ratio >= 2.0,
            ordering.ordering_accuracy >= 0.80,
            ordering.symmetry_score >= 0.80,
            effectiveness.vs_legacy_improvement >= 0.10,
        ];
        let targets_met_count = targets.iter().filter(|&&t| t).count();
        let targets_total = targets.len();
        let all_targets_met = targets_met_count == targets_total;

        Self {
            overall_score,
            clustering_score,
            ordering_score,
            effectiveness_score,
            all_targets_met,
            targets_met_count,
            targets_total,
        }
    }
}

// ============================================================================
// Computation Functions
// ============================================================================

/// Compute session clustering metrics from E4 embeddings.
///
/// # Arguments
///
/// * `sessions` - Sessions with chunk assignments
/// * `embeddings` - Map of chunk UUID to E4 embedding
///
/// # Returns
///
/// Clustering metrics including separation ratio and silhouette score.
pub fn compute_session_clustering_metrics(
    sessions: &[TemporalSession],
    embeddings: &HashMap<Uuid, Vec<f32>>,
) -> SessionClusteringMetrics {
    if sessions.is_empty() || embeddings.is_empty() {
        return SessionClusteringMetrics::default();
    }

    let mut intra_similarities: Vec<f64> = Vec::new();
    let mut inter_similarities: Vec<f64> = Vec::new();
    let mut pairs_evaluated = 0;

    // Build session membership map
    let chunk_to_session: HashMap<Uuid, &str> = sessions
        .iter()
        .flat_map(|s| s.chunks.iter().map(move |c| (c.id, s.session_id.as_str())))
        .collect();

    // Collect all chunk IDs with embeddings
    let chunk_ids: Vec<Uuid> = embeddings.keys().copied().collect();

    // Sample pairs for efficiency (full O(n²) is expensive for large datasets)
    let max_pairs = 10000;
    let sample_step = if chunk_ids.len() * chunk_ids.len() / 2 > max_pairs {
        (chunk_ids.len() * chunk_ids.len() / 2 / max_pairs).max(1)
    } else {
        1
    };

    let mut pair_idx = 0;
    for i in 0..chunk_ids.len() {
        for j in (i + 1)..chunk_ids.len() {
            pair_idx += 1;
            if pair_idx % sample_step != 0 {
                continue;
            }

            let id_a = chunk_ids[i];
            let id_b = chunk_ids[j];

            let (Some(emb_a), Some(emb_b)) = (embeddings.get(&id_a), embeddings.get(&id_b)) else {
                continue;
            };

            let sim = cosine_similarity(emb_a, emb_b);

            let (Some(session_a), Some(session_b)) =
                (chunk_to_session.get(&id_a), chunk_to_session.get(&id_b))
            else {
                continue;
            };

            pairs_evaluated += 1;

            if session_a == session_b {
                intra_similarities.push(sim);
            } else {
                inter_similarities.push(sim);
            }
        }
    }

    // Compute metrics
    let intra_session_similarity = if intra_similarities.is_empty() {
        0.0
    } else {
        intra_similarities.iter().sum::<f64>() / intra_similarities.len() as f64
    };

    let inter_session_similarity = if inter_similarities.is_empty() {
        0.0
    } else {
        inter_similarities.iter().sum::<f64>() / inter_similarities.len() as f64
    };

    let session_separation_ratio = if inter_session_similarity > 0.001 {
        intra_session_similarity / inter_session_similarity
    } else if intra_session_similarity > 0.0 {
        10.0 // Cap at 10x if inter is near zero
    } else {
        1.0
    };

    // Compute silhouette score
    let silhouette_score = compute_silhouette_score(sessions, embeddings);

    // Compute boundary F1
    let boundary_f1 = compute_boundary_f1(sessions, embeddings);

    SessionClusteringMetrics {
        intra_session_similarity,
        inter_session_similarity,
        session_separation_ratio,
        silhouette_score,
        boundary_f1,
        num_sessions: sessions.len(),
        num_pairs_evaluated: pairs_evaluated,
    }
}

/// Compute silhouette score for session clustering.
fn compute_silhouette_score(
    sessions: &[TemporalSession],
    embeddings: &HashMap<Uuid, Vec<f32>>,
) -> f64 {
    if sessions.len() < 2 {
        return 0.0;
    }

    let mut silhouette_sum = 0.0;
    let mut count = 0;

    // Build session membership and embeddings lists
    let session_embeddings: Vec<Vec<(&Uuid, &Vec<f32>)>> = sessions
        .iter()
        .map(|s| {
            s.chunks
                .iter()
                .filter_map(|c| embeddings.get(&c.id).map(|e| (&c.id, e)))
                .collect()
        })
        .collect();

    for (session_idx, session) in session_embeddings.iter().enumerate() {
        if session.len() < 2 {
            continue;
        }

        for (id, emb) in session {
            // a(i) = average distance to other points in same cluster
            let a_i: f64 = session
                .iter()
                .filter(|(other_id, _)| *other_id != *id)
                .map(|(_, other_emb)| 1.0 - cosine_similarity(emb, other_emb))
                .sum::<f64>()
                / (session.len() - 1) as f64;

            // b(i) = minimum average distance to points in other clusters
            let b_i: f64 = session_embeddings
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != session_idx)
                .filter(|(_, other_session)| !other_session.is_empty())
                .map(|(_, other_session)| {
                    other_session
                        .iter()
                        .map(|(_, other_emb)| 1.0 - cosine_similarity(emb, other_emb))
                        .sum::<f64>()
                        / other_session.len() as f64
                })
                .fold(f64::INFINITY, f64::min);

            if b_i.is_finite() {
                let s_i = if a_i < b_i {
                    1.0 - a_i / b_i
                } else if a_i > b_i {
                    b_i / a_i - 1.0
                } else {
                    0.0
                };

                silhouette_sum += s_i;
                count += 1;
            }
        }
    }

    if count > 0 {
        silhouette_sum / count as f64
    } else {
        0.0
    }
}

/// Compute boundary detection F1 score.
fn compute_boundary_f1(sessions: &[TemporalSession], embeddings: &HashMap<Uuid, Vec<f32>>) -> f64 {
    if sessions.len() < 2 {
        return 0.0;
    }

    // Compute optimal threshold based on intra/inter similarity distributions
    let mut intra_sims: Vec<f64> = Vec::new();
    let mut inter_sims: Vec<f64> = Vec::new();

    let chunk_to_session: HashMap<Uuid, &str> = sessions
        .iter()
        .flat_map(|s| s.chunks.iter().map(move |c| (c.id, s.session_id.as_str())))
        .collect();

    // Sample pairs
    let all_chunks: Vec<Uuid> = sessions
        .iter()
        .flat_map(|s| s.chunks.iter().map(|c| c.id))
        .filter(|id| embeddings.contains_key(id))
        .collect();

    let max_pairs = 1000;
    let step = (all_chunks.len() * all_chunks.len() / 2 / max_pairs).max(1);
    let mut idx = 0;

    for i in 0..all_chunks.len() {
        for j in (i + 1)..all_chunks.len() {
            idx += 1;
            if idx % step != 0 {
                continue;
            }

            let id_a = all_chunks[i];
            let id_b = all_chunks[j];

            let (Some(emb_a), Some(emb_b)) = (embeddings.get(&id_a), embeddings.get(&id_b)) else {
                continue;
            };

            let sim = cosine_similarity(emb_a, emb_b);

            let (Some(session_a), Some(session_b)) =
                (chunk_to_session.get(&id_a), chunk_to_session.get(&id_b))
            else {
                continue;
            };

            if session_a == session_b {
                intra_sims.push(sim);
            } else {
                inter_sims.push(sim);
            }
        }
    }

    if intra_sims.is_empty() || inter_sims.is_empty() {
        return 0.0;
    }

    // Use midpoint of means as threshold
    let intra_mean: f64 = intra_sims.iter().sum::<f64>() / intra_sims.len() as f64;
    let inter_mean: f64 = inter_sims.iter().sum::<f64>() / inter_sims.len() as f64;
    let threshold = (intra_mean + inter_mean) / 2.0;

    // Compute F1 with this threshold
    // True positive: same session AND sim >= threshold
    // False positive: different session AND sim >= threshold
    // False negative: same session AND sim < threshold
    let tp = intra_sims.iter().filter(|&&s| s >= threshold).count() as f64;
    let fp = inter_sims.iter().filter(|&&s| s >= threshold).count() as f64;
    let fn_ = intra_sims.iter().filter(|&&s| s < threshold).count() as f64;

    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };

    if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    }
}

/// Compute intra-session ordering metrics.
///
/// # Arguments
///
/// * `sessions` - Sessions with ordered chunks
/// * `embeddings` - Map of chunk UUID to E4 embedding
///
/// # Returns
///
/// Ordering metrics including accuracy and Kendall's tau.
pub fn compute_intra_session_ordering(
    sessions: &[TemporalSession],
    embeddings: &HashMap<Uuid, Vec<f32>>,
) -> IntraSessionOrderingMetrics {
    if sessions.is_empty() || embeddings.is_empty() {
        return IntraSessionOrderingMetrics::default();
    }

    let mut ordering_correct = 0;
    let mut ordering_total = 0;
    let mut tau_sum = 0.0;
    let mut tau_count = 0;
    let mut before_correct = 0;
    let mut before_total = 0;
    let mut after_correct = 0;
    let mut after_total = 0;
    let mut mrr_sum = 0.0;
    let mut mrr_count = 0;
    let mut sessions_evaluated = 0;

    for session in sessions {
        if session.len() < 3 {
            continue;
        }

        sessions_evaluated += 1;

        // Get embeddings for this session
        let session_embs: Vec<(usize, Uuid, &Vec<f32>)> = session
            .chunks
            .iter()
            .filter_map(|c| {
                embeddings
                    .get(&c.id)
                    .map(|e| (c.sequence_position, c.id, e))
            })
            .collect();

        if session_embs.len() < 3 {
            continue;
        }

        // Test ordering from various anchors
        for anchor_idx in 1..(session_embs.len() - 1) {
            let (anchor_pos, _anchor_id, anchor_emb) = &session_embs[anchor_idx];

            // Compute similarities and check ordering
            let mut before_sims: Vec<(usize, f64)> = Vec::new();
            let mut after_sims: Vec<(usize, f64)> = Vec::new();

            for (pos, _id, emb) in &session_embs {
                if pos == anchor_pos {
                    continue;
                }
                let sim = cosine_similarity(anchor_emb, emb);
                if *pos < *anchor_pos {
                    before_sims.push((*pos, sim));
                } else {
                    after_sims.push((*pos, sim));
                }
            }

            // Check ordering: closer positions should have higher similarity
            // For "before", positions closer to anchor (larger pos) should have higher sim
            before_sims.sort_by(|a, b| b.0.cmp(&a.0)); // Sort by position descending
            for i in 0..before_sims.len().saturating_sub(1) {
                if before_sims[i].1 >= before_sims[i + 1].1 {
                    ordering_correct += 1;
                }
                ordering_total += 1;
            }

            // For "after", positions closer to anchor (smaller pos) should have higher sim
            after_sims.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by position ascending
            for i in 0..after_sims.len().saturating_sub(1) {
                if after_sims[i].1 >= after_sims[i + 1].1 {
                    ordering_correct += 1;
                }
                ordering_total += 1;
            }

            // Before/after accuracy: retrieval accuracy
            if !before_sims.is_empty() {
                // Sort by similarity and check if top-k are correct direction
                let mut by_sim: Vec<_> = before_sims.iter().collect();
                by_sim.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                for (pos, _) in by_sim.iter().take(3) {
                    if *pos < *anchor_pos {
                        before_correct += 1;
                    }
                    before_total += 1;
                }
            }

            if !after_sims.is_empty() {
                let mut by_sim: Vec<_> = after_sims.iter().collect();
                by_sim.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                for (pos, _) in by_sim.iter().take(3) {
                    if *pos > *anchor_pos {
                        after_correct += 1;
                    }
                    after_total += 1;
                }
            }

            // MRR: rank of the immediate neighbor
            if !before_sims.is_empty() {
                let immediate_before_pos = anchor_pos - 1;
                let mut sorted: Vec<_> = before_sims.iter().collect();
                sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                if let Some(rank) = sorted.iter().position(|(pos, _)| *pos == immediate_before_pos)
                {
                    mrr_sum += 1.0 / (rank + 1) as f64;
                    mrr_count += 1;
                }
            }
        }

        // Compute Kendall's tau for this session
        if let Some(tau) = compute_kendalls_tau_for_session(&session_embs) {
            tau_sum += tau;
            tau_count += 1;
        }
    }

    let ordering_accuracy = if ordering_total > 0 {
        ordering_correct as f64 / ordering_total as f64
    } else {
        0.0
    };

    let kendalls_tau = if tau_count > 0 {
        tau_sum / tau_count as f64
    } else {
        0.0
    };

    let before_accuracy = if before_total > 0 {
        before_correct as f64 / before_total as f64
    } else {
        0.0
    };

    let after_accuracy = if after_total > 0 {
        after_correct as f64 / after_total as f64
    } else {
        0.0
    };

    let symmetry_score = 1.0 - (before_accuracy - after_accuracy).abs();

    let position_mrr = if mrr_count > 0 {
        mrr_sum / mrr_count as f64
    } else {
        0.0
    };

    IntraSessionOrderingMetrics {
        ordering_accuracy,
        kendalls_tau,
        before_accuracy,
        after_accuracy,
        symmetry_score,
        position_mrr,
        num_sessions: sessions_evaluated,
        num_queries: ordering_total,
    }
}

/// Compute Kendall's tau for a single session.
fn compute_kendalls_tau_for_session(session_embs: &[(usize, Uuid, &Vec<f32>)]) -> Option<f64> {
    if session_embs.len() < 3 {
        return None;
    }

    // Use first chunk as anchor
    let anchor_emb = session_embs[0].2;

    // Compute similarities to anchor
    let mut pos_sim_pairs: Vec<(usize, f64)> = session_embs[1..]
        .iter()
        .map(|(pos, _id, emb)| (*pos, cosine_similarity(anchor_emb, emb)))
        .collect();

    if pos_sim_pairs.len() < 2 {
        return None;
    }

    // Sort by similarity (descending)
    pos_sim_pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Count concordant and discordant pairs
    // Concordant: if sim(A) > sim(B), then pos(A) < pos(B) (closer to anchor)
    let mut concordant = 0;
    let mut discordant = 0;

    for i in 0..pos_sim_pairs.len() {
        for j in (i + 1)..pos_sim_pairs.len() {
            // Already sorted by sim descending, so i has higher sim than j
            // Concordant if i is closer to anchor (smaller position delta)
            let pos_i = pos_sim_pairs[i].0;
            let pos_j = pos_sim_pairs[j].0;

            // Closer position should have higher similarity
            if pos_i < pos_j {
                concordant += 1;
            } else if pos_i > pos_j {
                discordant += 1;
            }
            // Equal positions are ties, not counted
        }
    }

    let n = concordant + discordant;
    if n > 0 {
        Some((concordant as f64 - discordant as f64) / n as f64)
    } else {
        Some(0.0)
    }
}

/// Compute hybrid effectiveness by comparing modes.
///
/// # Arguments
///
/// * `hybrid_metrics` - Metrics from hybrid E4 mode
/// * `legacy_metrics` - Metrics from legacy E4 (position-only)
/// * `timestamp_metrics` - Metrics from timestamp baseline
///
/// # Returns
///
/// Effectiveness metrics showing improvement over baselines.
pub fn compute_hybrid_effectiveness(
    hybrid_metrics: &SessionClusteringMetrics,
    legacy_metrics: Option<&SessionClusteringMetrics>,
    timestamp_metrics: Option<&SessionClusteringMetrics>,
) -> HybridEffectivenessMetrics {
    let vs_legacy_improvement = legacy_metrics
        .filter(|m| m.session_separation_ratio > 0.001)
        .map(|m| {
            (hybrid_metrics.session_separation_ratio - m.session_separation_ratio)
                / m.session_separation_ratio
        })
        .unwrap_or(0.0);

    let vs_timestamp_improvement = timestamp_metrics
        .filter(|m| m.session_separation_ratio > 0.001)
        .map(|m| {
            (hybrid_metrics.session_separation_ratio - m.session_separation_ratio)
                / m.session_separation_ratio
        })
        .unwrap_or(0.0);

    HybridEffectivenessMetrics {
        vs_legacy_improvement,
        vs_timestamp_improvement,
        signature_distinctness: 0.0, // Computed separately if session signatures available
        position_preservation: 0.0,   // Computed separately
        legacy_separation_ratio: legacy_metrics
            .map(|m| m.session_separation_ratio)
            .unwrap_or(0.0),
        timestamp_separation_ratio: timestamp_metrics
            .map(|m| m.session_separation_ratio)
            .unwrap_or(0.0),
    }
}

/// Compute full E4 hybrid session metrics.
///
/// # Arguments
///
/// * `sessions` - Sessions with ordered chunks
/// * `embeddings` - Map of chunk UUID to E4 embedding
/// * `legacy_embeddings` - Optional legacy E4 embeddings for comparison
/// * `timestamp_embeddings` - Optional timestamp embeddings for comparison
///
/// # Returns
///
/// Complete E4 hybrid session metrics.
pub fn compute_e4_hybrid_metrics(
    sessions: &[TemporalSession],
    embeddings: &HashMap<Uuid, Vec<f32>>,
    legacy_embeddings: Option<&HashMap<Uuid, Vec<f32>>>,
    timestamp_embeddings: Option<&HashMap<Uuid, Vec<f32>>>,
) -> E4HybridSessionMetrics {
    let clustering = compute_session_clustering_metrics(sessions, embeddings);
    let ordering = compute_intra_session_ordering(sessions, embeddings);

    // Compute baseline metrics for comparison
    let legacy_clustering =
        legacy_embeddings.map(|e| compute_session_clustering_metrics(sessions, e));
    let timestamp_clustering =
        timestamp_embeddings.map(|e| compute_session_clustering_metrics(sessions, e));

    let hybrid_effectiveness = compute_hybrid_effectiveness(
        &clustering,
        legacy_clustering.as_ref(),
        timestamp_clustering.as_ref(),
    );

    let composite =
        CompositeE4HybridMetrics::compute(&clustering, &ordering, &hybrid_effectiveness);

    E4HybridSessionMetrics {
        clustering,
        ordering,
        hybrid_effectiveness,
        composite,
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasets::temporal_sessions::SessionChunk;

    fn create_mock_session(id: &str, chunks: Vec<(Uuid, usize)>) -> TemporalSession {
        TemporalSession {
            session_id: id.to_string(),
            chunks: chunks
                .into_iter()
                .map(|(uuid, pos)| SessionChunk {
                    id: uuid,
                    sequence_position: pos,
                    text: format!("Chunk {}", pos),
                    source_doc_id: "doc".to_string(),
                    original_topic: "topic".to_string(),
                    source_dataset: "test".to_string(),
                })
                .collect(),
            topic: "test_topic".to_string(),
            coherence_score: 0.8,
            source_datasets: vec!["test".to_string()],
        }
    }

    fn create_mock_embeddings(ids: &[Uuid], session_idx: usize, dim: usize) -> HashMap<Uuid, Vec<f32>> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                let mut emb = vec![0.0; dim];
                // Create embeddings that cluster by session_idx
                emb[session_idx % dim] = 1.0;
                // Add some variation by position
                emb[(i + 1) % dim] = 0.5 - (i as f32 * 0.1);
                (*id, emb)
            })
            .collect()
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_session_clustering_metrics_default() {
        let metrics = SessionClusteringMetrics::default();
        assert_eq!(metrics.session_separation_ratio, 0.0);
        assert!(!metrics.meets_target());
    }

    #[test]
    fn test_session_clustering_basic() {
        let ids_s1: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        let ids_s2: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        let session1 = create_mock_session(
            "s1",
            ids_s1.iter().enumerate().map(|(i, id)| (*id, i)).collect(),
        );
        let session2 = create_mock_session(
            "s2",
            ids_s2.iter().enumerate().map(|(i, id)| (*id, i)).collect(),
        );

        let mut embeddings = create_mock_embeddings(&ids_s1, 0, 64);
        embeddings.extend(create_mock_embeddings(&ids_s2, 1, 64));

        let sessions = vec![session1, session2];
        let metrics = compute_session_clustering_metrics(&sessions, &embeddings);

        assert!(metrics.num_sessions == 2);
        assert!(metrics.num_pairs_evaluated > 0);
    }

    #[test]
    fn test_ordering_metrics_default() {
        let metrics = IntraSessionOrderingMetrics::default();
        assert_eq!(metrics.ordering_accuracy, 0.0);
        assert!(!metrics.meets_targets());
    }

    #[test]
    fn test_composite_metrics_computation() {
        let clustering = SessionClusteringMetrics {
            session_separation_ratio: 2.5,
            silhouette_score: 0.5,
            boundary_f1: 0.8,
            ..Default::default()
        };

        let ordering = IntraSessionOrderingMetrics {
            ordering_accuracy: 0.85,
            symmetry_score: 0.9,
            kendalls_tau: 0.6,
            position_mrr: 0.7,
            ..Default::default()
        };

        let effectiveness = HybridEffectivenessMetrics {
            vs_legacy_improvement: 0.15,
            signature_distinctness: 0.8,
            ..Default::default()
        };

        let composite = CompositeE4HybridMetrics::compute(&clustering, &ordering, &effectiveness);

        assert!(composite.overall_score > 0.0);
        assert!(composite.overall_score <= 1.0);
        assert!(composite.all_targets_met);
        assert_eq!(composite.targets_met_count, 4);
    }

    #[test]
    fn test_hybrid_effectiveness_improvement() {
        let hybrid = SessionClusteringMetrics {
            session_separation_ratio: 2.5,
            ..Default::default()
        };

        let legacy = SessionClusteringMetrics {
            session_separation_ratio: 1.5,
            ..Default::default()
        };

        let effectiveness = compute_hybrid_effectiveness(&hybrid, Some(&legacy), None);

        // (2.5 - 1.5) / 1.5 = 0.667
        assert!((effectiveness.vs_legacy_improvement - 0.667).abs() < 0.01);
        assert!(effectiveness.meets_target());
    }
}
