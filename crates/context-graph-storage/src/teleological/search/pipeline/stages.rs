//! Individual pipeline stage implementations.
//!
//! This module contains the implementation of each of the 6 pipeline stages:
//! 1. SPLADE Filter (inverted index, NOT HNSW)
//! 2. Matryoshka ANN (128D HNSW)
//! 3. RRF Rerank (multi-space)
//!    3.5. Graph Expansion (K-NN edges)
//!    3.75. GNN Enhancement (R-GCN message passing)
//! 4. MaxSim Rerank (ColBERT, NOT HNSW)

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use tracing::debug;
use uuid::Uuid;

use super::super::super::indexes::EmbedderIndex;
use super::super::maxsim::compute_maxsim_direct;
use super::super::single::SingleEmbedderSearch;
use super::traits::{SpladeIndex, TokenStorage};
use super::types::{
    GraphExpansionConfig, PipelineCandidate, PipelineConfig, PipelineError, PipelineStage,
    StageConfig, StageResult,
};
use crate::graph_edges::EdgeRepository;

/// Stage execution helper that encapsulates stage logic.
pub(crate) struct StageExecutor<'a> {
    pub single_search: &'a SingleEmbedderSearch,
    pub splade_index: &'a Arc<dyn SpladeIndex>,
    pub token_storage: &'a Arc<dyn TokenStorage>,
    pub config: &'a PipelineConfig,
}

impl<'a> StageExecutor<'a> {
    // ========================================================================
    // STAGE 1: SPLADE FILTER (Inverted Index, NOT HNSW)
    // ========================================================================

    /// Stage 1: SPLADE sparse pre-filter using inverted index.
    /// NOT HNSW - uses BM25 scoring on inverted index.
    pub fn stage_splade_filter(
        &self,
        query: &[(usize, f32)],
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let stage_start = Instant::now();
        let candidates_in = 0; // Stage 1 starts from full corpus

        // Calculate target count based on k and multiplier
        let target_count = (self.config.k as f32 * config.candidate_multiplier * 10.0) as usize;

        // Search inverted index (NOT HNSW)
        let results = self.splade_index.search(query, target_count);

        // Convert to pipeline candidates
        let mut candidates: Vec<PipelineCandidate> = results
            .into_iter()
            .filter(|(_, score)| *score >= config.min_score_threshold)
            .map(|(id, score)| {
                let mut c = PipelineCandidate::new(id, score);
                c.add_stage_score(PipelineStage::SpladeFilter, score);
                c
            })
            .collect();

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let latency_us = stage_start.elapsed().as_micros() as u64;
        let latency_ms = latency_us / 1000;

        // Check timeout - FAIL FAST
        if latency_ms > config.max_latency_ms {
            return Err(PipelineError::Timeout {
                stage: PipelineStage::SpladeFilter,
                elapsed_ms: latency_ms,
                max_ms: config.max_latency_ms,
            });
        }

        let candidates_out = candidates.len();

        Ok(StageResult {
            candidates,
            latency_us,
            candidates_in,
            candidates_out,
            stage: PipelineStage::SpladeFilter,
        })
    }

    // ========================================================================
    // STAGE 2: MATRYOSHKA ANN (HNSW 128D)
    // ========================================================================

    /// Stage 2: Matryoshka 128D fast ANN.
    /// Uses E1Matryoshka128 HNSW index.
    pub fn stage_matryoshka_ann(
        &self,
        query: &[f32],
        candidates: Vec<PipelineCandidate>,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let stage_start = Instant::now();
        let candidates_in = candidates.len();

        // If no candidates from Stage 1, do full index search
        let target_count = if candidates.is_empty() {
            (self.config.k as f32 * config.candidate_multiplier * 5.0) as usize
        } else {
            (candidates.len() as f32 * 0.1).max(self.config.k as f32 * config.candidate_multiplier)
                as usize
        };

        // Search using Matryoshka 128D HNSW
        let search_result = self.single_search.search(
            EmbedderIndex::E1Matryoshka128,
            query,
            target_count,
            Some(config.min_score_threshold),
        )?;

        // Create candidate set from Stage 1 for filtering
        let candidate_ids: HashSet<_> = candidates.iter().map(|c| c.id).collect();

        // Filter and convert to pipeline candidates
        let mut new_candidates: Vec<PipelineCandidate> = if candidates.is_empty() {
            // No Stage 1, use all results
            search_result
                .hits
                .into_iter()
                .map(|hit| {
                    let mut c = PipelineCandidate::new(hit.id, hit.similarity);
                    c.add_stage_score(PipelineStage::MatryoshkaAnn, hit.similarity);
                    c
                })
                .collect()
        } else {
            // Filter to only candidates from Stage 1
            search_result
                .hits
                .into_iter()
                .filter(|hit| candidate_ids.contains(&hit.id))
                .map(|hit| {
                    // Find the original candidate to preserve stage scores
                    let prev = candidates.iter().find(|c| c.id == hit.id);
                    if let Some(p) = prev {
                        let mut new_c = p.clone();
                        new_c.add_stage_score(PipelineStage::MatryoshkaAnn, hit.similarity);
                        new_c
                    } else {
                        let mut new_c = PipelineCandidate::new(hit.id, hit.similarity);
                        new_c.add_stage_score(PipelineStage::MatryoshkaAnn, hit.similarity);
                        new_c
                    }
                })
                .collect()
        };

        // Sort by score descending
        new_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let latency_us = stage_start.elapsed().as_micros() as u64;
        let latency_ms = latency_us / 1000;

        // Check timeout - FAIL FAST
        if latency_ms > config.max_latency_ms {
            return Err(PipelineError::Timeout {
                stage: PipelineStage::MatryoshkaAnn,
                elapsed_ms: latency_ms,
                max_ms: config.max_latency_ms,
            });
        }

        let candidates_out = new_candidates.len();

        Ok(StageResult {
            candidates: new_candidates,
            latency_us,
            candidates_in,
            candidates_out,
            stage: PipelineStage::MatryoshkaAnn,
        })
    }

    // ========================================================================
    // STAGE 3: RRF RERANK
    // ========================================================================

    /// Stage 3: Multi-space RRF rerank.
    /// Uses MultiEmbedderSearch with RRF aggregation.
    pub fn stage_rrf_rerank(
        &self,
        query_semantic: &[f32],
        candidates: Vec<PipelineCandidate>,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let stage_start = Instant::now();
        let candidates_in = candidates.len();

        if candidates.is_empty() {
            return Ok(StageResult {
                candidates: Vec::new(),
                latency_us: stage_start.elapsed().as_micros() as u64,
                candidates_in: 0,
                candidates_out: 0,
                stage: PipelineStage::RrfRerank,
            });
        }

        // Create candidate ID set for filtering
        let candidate_ids: HashSet<_> = candidates.iter().map(|c| c.id).collect();
        let target_count = (candidates.len() as f32 * 0.1)
            .max(self.config.k as f32 * config.candidate_multiplier)
            as usize;

        // Compute RRF scores
        // RRF(d) = Σ 1/(k + rank_i(d)) for each ranking i
        // Pre-allocate for candidates to avoid reallocations in hot path
        let mut rrf_scores: HashMap<Uuid, f32> = HashMap::with_capacity(candidates.len());

        // Search semantic embedder
        let semantic_results = self.single_search.search(
            EmbedderIndex::E1Semantic,
            query_semantic,
            target_count * 2, // Search wider to ensure coverage
            None,
        )?;

        // Compute RRF scores
        for (rank, hit) in semantic_results.hits.iter().enumerate() {
            if candidate_ids.contains(&hit.id) {
                let rrf_score = 1.0 / (self.config.rrf_k + rank as f32 + 1.0);
                *rrf_scores.entry(hit.id).or_insert(0.0) += rrf_score;
            }
        }

        // Convert to pipeline candidates
        let mut new_candidates: Vec<PipelineCandidate> = rrf_scores
            .into_iter()
            .map(|(id, rrf_score)| {
                let prev = candidates.iter().find(|c| c.id == id);
                if let Some(p) = prev {
                    let mut new_c = p.clone();
                    new_c.add_stage_score(PipelineStage::RrfRerank, rrf_score);
                    new_c
                } else {
                    let mut new_c = PipelineCandidate::new(id, rrf_score);
                    new_c.add_stage_score(PipelineStage::RrfRerank, rrf_score);
                    new_c
                }
            })
            .filter(|c| c.score >= config.min_score_threshold)
            .collect();

        // Sort by RRF score descending
        new_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        new_candidates.truncate(target_count);

        let latency_us = stage_start.elapsed().as_micros() as u64;
        let latency_ms = latency_us / 1000;

        // Check timeout - FAIL FAST
        if latency_ms > config.max_latency_ms {
            return Err(PipelineError::Timeout {
                stage: PipelineStage::RrfRerank,
                elapsed_ms: latency_ms,
                max_ms: config.max_latency_ms,
            });
        }

        let candidates_out = new_candidates.len();

        Ok(StageResult {
            candidates: new_candidates,
            latency_us,
            candidates_in,
            candidates_out,
            stage: PipelineStage::RrfRerank,
        })
    }

    // ========================================================================
    // STAGE 4: MAXSIM RERANK (ColBERT, NOT HNSW)
    // ========================================================================

    /// Stage 4: Late interaction MaxSim.
    /// Uses ColBERT-style token matching, NOT HNSW.
    pub fn stage_maxsim_rerank(
        &self,
        query_tokens: &[Vec<f32>],
        candidates: Vec<PipelineCandidate>,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let stage_start = Instant::now();
        let candidates_in = candidates.len();

        if candidates.is_empty() || query_tokens.is_empty() {
            return Ok(StageResult {
                candidates: Vec::new(),
                latency_us: stage_start.elapsed().as_micros() as u64,
                candidates_in,
                candidates_out: 0,
                stage: PipelineStage::MaxSimRerank,
            });
        }

        // Compute MaxSim scores in parallel using rayon
        let token_storage = &self.token_storage;
        let scored: Vec<(PipelineCandidate, f32)> = candidates
            .into_par_iter()
            .filter_map(|mut c| {
                if let Some(doc_tokens) = token_storage.get_tokens(c.id) {
                    let maxsim_score = compute_maxsim_direct(query_tokens, &doc_tokens);
                    c.add_stage_score(PipelineStage::MaxSimRerank, maxsim_score);
                    Some((c, maxsim_score))
                } else {
                    None // Skip candidates without token embeddings
                }
            })
            .collect();

        // Sort by MaxSim score descending
        let mut new_candidates: Vec<PipelineCandidate> = scored
            .into_iter()
            .filter(|(_, score)| *score >= config.min_score_threshold)
            .map(|(c, _)| c)
            .collect();

        new_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        new_candidates.truncate(self.config.k);

        let latency_us = stage_start.elapsed().as_micros() as u64;
        let latency_ms = latency_us / 1000;

        // Check timeout - FAIL FAST
        if latency_ms > config.max_latency_ms {
            return Err(PipelineError::Timeout {
                stage: PipelineStage::MaxSimRerank,
                elapsed_ms: latency_ms,
                max_ms: config.max_latency_ms,
            });
        }

        let candidates_out = new_candidates.len();

        Ok(StageResult {
            candidates: new_candidates,
            latency_us,
            candidates_in,
            candidates_out,
            stage: PipelineStage::MaxSimRerank,
        })
    }

    // ========================================================================
    // STAGE 3.5: GRAPH EXPANSION (K-NN Edges)
    // ========================================================================

    /// Stage 3.5: Expand candidates via pre-computed K-NN graph edges.
    ///
    /// This stage enriches the candidate set by adding neighbors from the
    /// pre-computed graph edges. Neighbors receive a decayed score based on
    /// their edge weight and the parent candidate's score.
    ///
    /// # Arguments
    ///
    /// * `candidates` - Current candidate list from Stage 3 (RRF)
    /// * `edge_repository` - Repository for typed edges
    /// * `config` - Graph expansion configuration
    ///
    /// # Returns
    ///
    /// Expanded candidate list with neighbors added.
    pub fn stage_graph_expansion(
        &self,
        candidates: Vec<PipelineCandidate>,
        edge_repository: &EdgeRepository,
        config: &GraphExpansionConfig,
    ) -> Result<StageResult, PipelineError> {
        let stage_start = Instant::now();
        let candidates_in = candidates.len();

        if candidates.is_empty() || !config.enabled {
            return Ok(StageResult {
                candidates,
                latency_us: stage_start.elapsed().as_micros() as u64,
                candidates_in,
                candidates_out: candidates_in,
                stage: PipelineStage::GraphExpansion,
            });
        }

        // Track which IDs we've already seen to avoid duplicates
        let mut seen_ids: HashSet<Uuid> = candidates.iter().map(|c| c.id).collect();
        let mut expanded_candidates = Vec::with_capacity(config.max_total_expanded);

        // Keep original candidates
        expanded_candidates.extend(candidates.iter().cloned());

        // Track expansion stats for logging
        let mut total_edges_checked = 0usize;
        let mut total_edges_followed = 0usize;

        // Expand each candidate
        for candidate in &candidates {
            // Check if we've hit the expansion limit
            if expanded_candidates.len() >= config.max_total_expanded {
                debug!(
                    limit = config.max_total_expanded,
                    "Graph expansion hit max_total_expanded limit"
                );
                break;
            }

            // Get typed edges from this candidate
            let edges = match edge_repository.get_typed_edges_from(candidate.id) {
                Ok(e) => e,
                Err(err) => {
                    // Log but don't fail - graph edges are enhancement, not critical
                    debug!(
                        source_id = %candidate.id,
                        error = %err,
                        "Failed to get typed edges, skipping expansion for this candidate"
                    );
                    continue;
                }
            };

            let mut expansion_count = 0;

            for edge in edges {
                total_edges_checked += 1;

                // Check edge type routing
                if !config.edge_type_routing.should_expand(edge.edge_type()) {
                    continue;
                }

                // Check edge weight threshold
                if edge.weight() < config.min_edge_weight {
                    continue;
                }

                // Check if we've already seen this target
                if seen_ids.contains(&edge.target()) {
                    continue;
                }

                // Check per-node expansion limit
                if expansion_count >= config.max_expansion_per_node {
                    break;
                }

                // Check total expansion limit
                if expanded_candidates.len() >= config.max_total_expanded {
                    break;
                }

                // Add the neighbor with decayed score
                // score = parent_score * edge_weight * decay
                let decayed_score = candidate.score * edge.weight() * config.expansion_decay;

                let mut new_candidate = PipelineCandidate::new(edge.target(), decayed_score);
                new_candidate.add_stage_score(PipelineStage::GraphExpansion, decayed_score);

                seen_ids.insert(edge.target());
                expanded_candidates.push(new_candidate);
                expansion_count += 1;
                total_edges_followed += 1;
            }
        }

        // Sort by score descending
        expanded_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let latency_us = stage_start.elapsed().as_micros() as u64;
        let latency_ms = latency_us / 1000;

        debug!(
            candidates_in,
            candidates_out = expanded_candidates.len(),
            edges_checked = total_edges_checked,
            edges_followed = total_edges_followed,
            latency_ms,
            "Graph expansion completed"
        );

        // Check timeout - FAIL FAST
        if latency_ms > config.max_latency_ms {
            return Err(PipelineError::Timeout {
                stage: PipelineStage::GraphExpansion,
                elapsed_ms: latency_ms,
                max_ms: config.max_latency_ms,
            });
        }

        let candidates_out = expanded_candidates.len();

        Ok(StageResult {
            candidates: expanded_candidates,
            latency_us,
            candidates_in,
            candidates_out,
            stage: PipelineStage::GraphExpansion,
        })
    }
}
