//! E9 robustness/blind-spot detection tool implementation (search_robust).
//!
//! # E9 Blind-Spot Detection Philosophy
//!
//! Per the 13-embedder philosophy:
//! - E1 is the semantic foundation (finds semantically similar content)
//! - E9 (HDC) finds what E1 MISSES due to character-level issues
//! - E9 doesn't compete with E1; it discovers E1's blind spots
//!
//! # The Algorithm
//!
//! 1. Embed query using all 13 embedders
//! 2. Search E1 space → get E1's top results
//! 3. Search E9 space → get E9's top results
//! 4. Find blind spots: results where E9 is high AND E1 is low
//! 5. Combine: E1 results + E9 discoveries (labeled)
//! 6. Return with provenance showing what E9 discovered
//!
//! # What E9 Finds That E1 Misses
//!
//! - Typos: "authetication" → finds "authentication"
//! - Code identifiers: "parseConfig" → finds "parse_config"
//! - Character variations: Novel words, OCR errors, etc.
//!
//! # Constitution Compliance
//!
//! - ARCH-12: E1 is foundation; E9 enhances by finding blind spots
//! - Philosophy: E9 finds what E1 misses, doesn't compete with E1
//! - FAIL FAST: All errors propagate immediately with logging

use std::collections::HashSet;

use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::robustness_dtos::{
    BlindSpotCandidate, ResultSource, RobustSearchMetadata, RobustSearchResult, RobustSourceInfo,
    SearchRobustRequest, SearchRobustResponse,
};

use super::helpers::{ToolErrorKind, cosine_similarity};
use super::super::Handlers;

impl Handlers {
    /// search_robust tool implementation.
    ///
    /// Finds memories using E9 blind-spot detection. Returns E1's semantic results
    /// PLUS memories that E9 found but E1 missed (blind spots).
    ///
    /// # Algorithm
    ///
    /// 1. Embed the query using all 13 embedders
    /// 2. Search E1 space (semantic foundation) → top 3*topK candidates
    /// 3. Search E9 space (structural/noise-robust) → top 3*topK candidates
    /// 4. Find blind spots: E9 score >= threshold AND E1 score < weakness threshold
    /// 5. Combine E1 results + E9 discoveries (deduplicated)
    /// 6. Return with metadata showing what E9 discovered
    ///
    /// # Parameters
    ///
    /// - `query`: Text to search for (typos are OK - E9 is noise-tolerant)
    /// - `topK`: Maximum results to return (1-50, default: 10)
    /// - `minScore`: Minimum score threshold (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
    /// - `includeE9Score`: Include E9 and E1 scores separately (default: true)
    /// - `e9DiscoveryThreshold`: Min E9 score for discovery (default: 0.7)
    /// - `e1WeaknessThreshold`: Max E1 score to be "missed" (default: 0.5)
    pub(crate) async fn call_search_robust(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: SearchRobustRequest = match self.parse_request(id.clone(), args, "search_robust") {
            Ok(req) => req,
            Err(resp) => return resp,
        };

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let e9_threshold = request.e9_discovery_threshold;
        let e1_threshold = request.e1_weakness_threshold;

        // Parse strategy from request - Pipeline enables E13 recall + E12 reranking
        let strategy = request.parse_strategy();
        let enable_rerank = matches!(strategy, SearchStrategy::Pipeline);

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            e9_threshold = e9_threshold,
            e1_threshold = e1_threshold,
            strategy = ?strategy,
            enable_rerank = enable_rerank,
            "search_robust: Starting E9 blind-spot detection search"
        );

        // Step 1: Embed query using all 13 embedders
        let query_embedding = match self.embed_query(id.clone(), query, "search_robust").await {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Step 2: Search E1 space (semantic foundation)
        // Over-fetch 3x for blind spot detection
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        let e1_options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile("semantic_search")
            .with_min_similarity(0.0) // Get all candidates, filter later
            .with_rerank(enable_rerank); // Auto-enable E12 for pipeline

        let e1_candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, e1_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_robust: E1 search FAILED");
                return self.tool_error(id, &format!("E1 search failed: {}", e));
            }
        };

        let e1_candidates_count = e1_candidates.len();
        debug!(
            e1_candidates = e1_candidates_count,
            "search_robust: E1 semantic search completed"
        );

        // Step 3: Search E9 space (structural/noise-robust)
        // Use typo_tolerant profile which emphasizes E9
        let e9_options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile("typo_tolerant")
            .with_min_similarity(0.0)
            .with_rerank(enable_rerank); // Auto-enable E12 for pipeline

        let e9_candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, e9_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_robust: E9 search FAILED");
                return self.tool_error(id, &format!("E9 search failed: {}", e));
            }
        };

        let e9_candidates_count = e9_candidates.len();
        debug!(
            e9_candidates = e9_candidates_count,
            "search_robust: E9 structural search completed"
        );

        // Step 4: Find blind spots - results where E9 is high AND E1 is low
        // First, build a map of E1 scores by memory ID
        let query_e1 = &query_embedding.e1_semantic;
        let query_e9 = &query_embedding.e9_hdc;

        // Collect all unique memory IDs from both searches
        let mut all_memory_ids: HashSet<Uuid> = HashSet::new();
        for cand in &e1_candidates {
            all_memory_ids.insert(cand.fingerprint.id);
        }
        for cand in &e9_candidates {
            all_memory_ids.insert(cand.fingerprint.id);
        }

        // Build score maps for each embedder
        let mut e1_scores: std::collections::HashMap<Uuid, f32> =
            std::collections::HashMap::new();
        let mut e9_scores: std::collections::HashMap<Uuid, f32> =
            std::collections::HashMap::new();

        // Compute E1 scores (cosine similarity in E1 space)
        for cand in &e1_candidates {
            let cand_e1 = &cand.fingerprint.semantic.e1_semantic;
            let e1_sim = cosine_similarity(query_e1, cand_e1);
            e1_scores.insert(cand.fingerprint.id, e1_sim);
        }

        // Compute E9 scores (cosine similarity in E9 space)
        for cand in &e9_candidates {
            let cand_e9 = &cand.fingerprint.semantic.e9_hdc;
            let e9_sim = cosine_similarity(query_e9, cand_e9);
            e9_scores.insert(cand.fingerprint.id, e9_sim);
        }

        // For memories found by E9 but not E1, we need to compute their E1 score too
        for cand in &e9_candidates {
            let mem_id = cand.fingerprint.id;
            e1_scores.entry(mem_id).or_insert_with(|| {
                let cand_e1 = &cand.fingerprint.semantic.e1_semantic;
                cosine_similarity(query_e1, cand_e1)
            });
        }

        // Similarly, compute E9 scores for E1-found memories not in E9 results
        for cand in &e1_candidates {
            let mem_id = cand.fingerprint.id;
            e9_scores.entry(mem_id).or_insert_with(|| {
                let cand_e9 = &cand.fingerprint.semantic.e9_hdc;
                cosine_similarity(query_e9, cand_e9)
            });
        }

        // Identify blind spots: high E9 + low E1
        let mut blind_spot_ids: HashSet<Uuid> = HashSet::new();
        let mut blind_spot_candidates: Vec<BlindSpotCandidate> = Vec::new();

        for &mem_id in &all_memory_ids {
            let e9_score = *e9_scores.get(&mem_id).unwrap_or(&0.0);
            let e1_score = *e1_scores.get(&mem_id).unwrap_or(&0.0);
            let divergence = e9_score - e1_score;

            let candidate = BlindSpotCandidate {
                memory_id: mem_id,
                e9_score,
                e1_score,
                divergence,
            };

            if candidate.is_discovery(e9_threshold, e1_threshold) {
                blind_spot_ids.insert(mem_id);
                blind_spot_candidates.push(candidate);
            }
        }

        // Sort blind spots by divergence (highest = most significant discovery)
        blind_spot_candidates.sort_by(|a, b| {
            b.divergence
                .partial_cmp(&a.divergence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let discoveries_count = blind_spot_candidates.len();
        debug!(
            discoveries = discoveries_count,
            "search_robust: Found {} E9 blind-spot discoveries",
            discoveries_count
        );

        // Step 5: Combine E1 results + E9 discoveries
        // Start with E1's top results (sorted by E1 score)
        let mut e1_results_sorted: Vec<(Uuid, f32)> = e1_candidates
            .iter()
            .map(|c| {
                let e1_score = *e1_scores.get(&c.fingerprint.id).unwrap_or(&0.0);
                (c.fingerprint.id, e1_score)
            })
            .collect();
        e1_results_sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top E1 results that aren't blind spots
        let e1_result_ids: Vec<Uuid> = e1_results_sorted
            .iter()
            .filter(|(id, score)| !blind_spot_ids.contains(id) && *score >= min_score)
            .take(top_k)
            .map(|(id, _)| *id)
            .collect();

        // Calculate how many slots remain for E9 discoveries
        let e1_count = e1_result_ids.len();
        let e9_slots = top_k.saturating_sub(e1_count);

        // Take top E9 discoveries
        let e9_discovery_ids: Vec<Uuid> = blind_spot_candidates
            .iter()
            .filter(|c| c.e9_score >= min_score)
            .take(e9_slots)
            .map(|c| c.memory_id)
            .collect();

        // Combine for content retrieval
        let all_result_ids: Vec<Uuid> = e1_result_ids
            .iter()
            .chain(e9_discovery_ids.iter())
            .copied()
            .collect();

        // Step 6: Build response with provenance
        // Get content if requested - FAIL FAST on error
        let contents: Vec<Option<String>> =
            if request.include_content && !all_result_ids.is_empty() {
                match self.teleological_store.get_content_batch(&all_result_ids).await {
                    Ok(c) => c,
                    Err(e) => {
                        error!(
                            error = %e,
                            result_count = all_result_ids.len(),
                            "search_robust: Content retrieval FAILED"
                        );
                        return self.tool_error(
                            id,
                            &format!(
                                "Failed to retrieve content for {} results: {}",
                                all_result_ids.len(),
                                e
                            ),
                        );
                    }
                }
            } else {
                vec![None; all_result_ids.len()]
            };

        // Get source metadata - FAIL FAST on error
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&all_result_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    result_count = all_result_ids.len(),
                    "search_robust: Source metadata retrieval FAILED"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} results: {}",
                        all_result_ids.len(),
                        e
                    ),
                );
            }
        };

        // Build E1 results
        let mut results: Vec<RobustSearchResult> = Vec::with_capacity(all_result_ids.len());

        for (i, &mem_id) in e1_result_ids.iter().enumerate() {
            let e1_score = *e1_scores.get(&mem_id).unwrap_or(&0.0);
            let e9_score = *e9_scores.get(&mem_id).unwrap_or(&0.0);

            let provenance = source_metadata.get(i).and_then(|m| {
                m.as_ref().map(|meta| RobustSourceInfo {
                    source_type: format!("{}", meta.source_type),
                    file_path: meta.file_path.clone(),
                    hook_type: meta.hook_type.clone(),
                    tool_name: meta.tool_name.clone(),
                })
            });

            results.push(RobustSearchResult {
                memory_id: mem_id,
                score: e1_score,
                source: ResultSource::E1,
                e9_score: if request.include_e9_score {
                    Some(e9_score)
                } else {
                    None
                },
                e1_score: if request.include_e9_score {
                    Some(e1_score)
                } else {
                    None
                },
                divergence: None,
                discovery_reason: None,
                content: contents.get(i).and_then(|c| c.clone()),
                provenance,
            });
        }

        // Build E9 discovery results
        let e1_count_for_offset = e1_result_ids.len();
        for (j, &mem_id) in e9_discovery_ids.iter().enumerate() {
            let i = e1_count_for_offset + j;
            let e1_score = *e1_scores.get(&mem_id).unwrap_or(&0.0);
            let e9_score = *e9_scores.get(&mem_id).unwrap_or(&0.0);
            let divergence = e9_score - e1_score;

            let provenance = source_metadata.get(i).and_then(|m| {
                m.as_ref().map(|meta| RobustSourceInfo {
                    source_type: format!("{}", meta.source_type),
                    file_path: meta.file_path.clone(),
                    hook_type: meta.hook_type.clone(),
                    tool_name: meta.tool_name.clone(),
                })
            });

            results.push(RobustSearchResult {
                memory_id: mem_id,
                score: e9_score, // Use E9 score as the ranking score for discoveries
                source: ResultSource::E9Discovery,
                e9_score: if request.include_e9_score {
                    Some(e9_score)
                } else {
                    None
                },
                e1_score: if request.include_e9_score {
                    Some(e1_score)
                } else {
                    None
                },
                divergence: Some(divergence),
                discovery_reason: Some(format!(
                    "E9 found structural match (E9={:.2}, E1={:.2}, divergence={:.2}) that E1 missed",
                    e9_score, e1_score, divergence
                )),
                content: contents.get(i).and_then(|c| c.clone()),
                provenance,
            });
        }

        let response = SearchRobustResponse {
            query: query.clone(),
            count: results.len(),
            results,
            metadata: RobustSearchMetadata {
                e1_results_count: e1_result_ids.len(),
                e9_discoveries_count: e9_discovery_ids.len(),
                blind_spots_found: e9_discovery_ids.clone(),
                e1_candidates_evaluated: e1_candidates_count,
                e9_candidates_evaluated: e9_candidates_count,
                e9_discovery_threshold: e9_threshold,
                e1_weakness_threshold: e1_threshold,
            },
        };

        info!(
            e1_results = response.metadata.e1_results_count,
            e9_discoveries = response.metadata.e9_discoveries_count,
            total = response.count,
            "search_robust: Completed blind-spot detection search"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_robust: Response serialization failed");
                self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Response serialization failed: {}", e))
            }
        }
    }
}

// LOW-15: cosine_similarity moved to super::helpers (shared across 4 tool modules).

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::robustness_dtos::{E1_WEAKNESS_THRESHOLD, E9_DISCOVERY_THRESHOLD};

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_length() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_threshold_constants() {
        // E9 threshold is LOWER than E1 because projected E9 vectors have much
        // lower cosine similarity than native Hamming. This is expected.
        // E9 finds blind spots where E9 >= 0.15 AND E1 < 0.5
        assert!(E9_DISCOVERY_THRESHOLD < E1_WEAKNESS_THRESHOLD,
                "E9 projected threshold should be lower than E1 semantic threshold");
        assert!(E9_DISCOVERY_THRESHOLD >= 0.1, "E9 should be at least 0.1");
        assert!(E1_WEAKNESS_THRESHOLD <= 0.6, "E1 weakness should be <= 0.6");
    }
}
