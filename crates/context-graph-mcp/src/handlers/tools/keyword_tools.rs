//! E6 keyword search tool implementation (search_by_keywords).
//!
//! # E6 Keyword Search (V_selectivity)
//!
//! This tool leverages E6 sparse embeddings for exact keyword matching:
//! - E6 finds exact term matches that E1 dilutes through semantic averaging
//! - Optionally uses E13 SPLADE for term expansion (fast→quick)
//! - Blends E6 keyword scores with E1 semantic for comprehensive results
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation, E6 enhances with keyword precision
//! - ARCH-13: Uses multi-space search with E1+E6
//! - E6 finds: "Exact keyword matches" that E1 misses by "Diluting via averaging"
//! - AP-02: All comparisons within respective spaces (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging

use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions, TeleologicalSearchResult};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::keyword_dtos::{
    KeywordSearchMetadata, KeywordSearchResult, KeywordSourceInfo, SearchByKeywordsRequest,
    SearchByKeywordsResponse,
};

use super::helpers::{ToolErrorKind, cosine_similarity};
use super::super::Handlers;

impl Handlers {
    /// search_by_keywords tool implementation.
    ///
    /// Finds memories matching specific keywords using E6 sparse embeddings.
    /// ENHANCES E1 semantic search with keyword-level precision.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the keyword query using all 13 embedders
    /// 2. Search using E1 semantic as primary (over-fetch for blending)
    /// 3. Extract keywords and compute E6 sparse similarity
    /// 4. Optionally expand with E13 SPLADE for term coverage
    /// 5. Blend E1 and E6 scores: (1-blend)*E1 + blend*E6
    /// 6. Filter by minScore and return top-K results
    ///
    /// # Parameters
    ///
    /// - `query`: The keyword query to search for (required)
    /// - `topK`: Maximum results to return (1-50, default: 10)
    /// - `minScore`: Minimum blended score threshold (0-1, default: 0.1)
    /// - `blendWithSemantic`: E6 weight in blend (0-1, default: 0.3)
    /// - `useSpladeExpansion`: Use E13 SPLADE term expansion (default: true)
    /// - `includeContent`: Include full content text (default: false)
    pub(crate) async fn call_search_by_keywords(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: SearchByKeywordsRequest = match self.parse_request(id.clone(), args, "search_by_keywords") {
            Ok(req) => req,
            Err(resp) => return resp,
        };

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let e6_blend = request.blend_with_semantic;
        let e1_weight = 1.0 - e6_blend;

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            e6_blend = e6_blend,
            use_splade = request.use_splade_expansion,
            "search_by_keywords: Starting keyword-enhanced search"
        );

        // Step 1: Embed the keyword query
        let query_embedding = match self.embed_query(id.clone(), query, "search_by_keywords").await {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Extract keywords for metadata and scoring
        let extracted_keywords = extract_keywords(query);

        debug!(
            keywords = ?extracted_keywords,
            keyword_count = extracted_keywords.len(),
            "search_by_keywords: Extracted keywords from query"
        );

        // Step 2a: E6 inverted index recall - finds exact keyword matches E1 might miss
        // Per Constitution: E6 finds "exact keyword matches" that E1 misses by "diluting through averaging"
        let e6_recall_limit = top_k * 5; // Over-fetch for union
        let e6_candidates = match self
            .teleological_store
            .search_e6_sparse(&query_embedding.e6_sparse, e6_recall_limit)
            .await
        {
            Ok(results) => {
                info!(
                    e6_recall_count = results.len(),
                    "search_by_keywords: E6 inverted index returned candidates"
                );
                results
            }
            Err(e) => {
                // FAIL FAST: E6 recall failure is a fatal error
                // Per user requirements: no workarounds or fallbacks
                error!(error = %e, "search_by_keywords: E6 inverted index recall FAILED");
                return self.tool_error(id, &format!("E6 inverted index search failed: {}", e));
            }
        };

        // Step 2b: E1 semantic search - the foundation per ARCH-12
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        // Parse strategy from request - Pipeline enables E13 recall + E12 reranking
        let strategy = request.parse_strategy();
        let enable_rerank = matches!(strategy, SearchStrategy::Pipeline);

        info!(
            strategy = ?strategy,
            enable_rerank = enable_rerank,
            "search_by_keywords: Using search strategy"
        );

        // Use keyword_search weight profile with E6 emphasis
        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile("semantic_search") // E1 foundation
            .with_min_similarity(0.0) // Get all candidates, filter later
            .with_rerank(enable_rerank); // Auto-enable E12 for pipeline

        let e1_candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_by_keywords: E1 semantic search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        // Step 2c: Union E6 and E1 candidates - E6 finds what E1 misses!
        // Build a map of all candidates by ID
        let mut candidate_map: std::collections::HashMap<Uuid, &TeleologicalSearchResult> =
            std::collections::HashMap::with_capacity(e1_candidates.len() + e6_candidates.len());

        // Add all E1 candidates
        for candidate in &e1_candidates {
            candidate_map.insert(candidate.fingerprint.id, candidate);
        }

        // E6 candidates that aren't in E1 results need to be fetched
        // These are the "blind spot" discoveries - exact keyword matches E1 missed
        let e6_only_ids: Vec<Uuid> = e6_candidates
            .iter()
            .filter(|(id, _)| !candidate_map.contains_key(id))
            .map(|(id, _)| *id)
            .collect();

        let e6_blind_spot_count = e6_only_ids.len();
        if e6_blind_spot_count > 0 {
            info!(
                e6_blind_spot_count = e6_blind_spot_count,
                "search_by_keywords: E6 found {} candidates that E1 missed (blind spots)",
                e6_blind_spot_count
            );
        }

        // Note: e6_term_counts would be used for more detailed E6 scoring if needed
        // Currently we use the e6_candidates directly in the scoring loop

        let candidates_evaluated = candidate_map.len() + e6_only_ids.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            e1_count = e1_candidates.len(),
            e6_recall_count = e6_candidates.len(),
            e6_blind_spots = e6_only_ids.len(),
            "search_by_keywords: Evaluating union of E1+E6 candidates"
        );

        // Step 3: Compute keyword-enhanced scores
        // Get query E6 sparse vector for keyword matching
        let query_e6 = &query_embedding.e6_sparse;
        let query_e1 = &query_embedding.e1_semantic;

        // Optional: Get E13 SPLADE for term expansion
        let query_e13 = if request.use_splade_expansion {
            Some(&query_embedding.e13_splade)
        } else {
            None
        };

        let mut scored_results: Vec<(Uuid, f32, f32, f32, u32)> = Vec::with_capacity(e1_candidates.len());

        // Score E1 candidates (they have full fingerprints)
        for candidate in &e1_candidates {
            let cand_id = candidate.fingerprint.id;
            let cand_e1 = &candidate.fingerprint.semantic.e1_semantic;
            let cand_e6 = &candidate.fingerprint.semantic.e6_sparse;

            // E1 cosine similarity (THE semantic foundation per ARCH-12)
            let e1_sim = cosine_similarity(query_e1, cand_e1);

            // E6 sparse keyword similarity using Jaccard-like overlap
            let (e6_sim, matching_count) = sparse_keyword_similarity(query_e6, cand_e6);

            // Optional E13 SPLADE expansion boost
            let e6_with_expansion = if let Some(q_e13) = query_e13 {
                let cand_e13 = &candidate.fingerprint.semantic.e13_splade;
                let (e13_sim, _) = sparse_keyword_similarity(q_e13, cand_e13);
                // Average E6 and E13 for expanded keyword coverage
                (e6_sim + e13_sim) / 2.0
            } else {
                e6_sim
            };

            // Blend scores: (1-blend)*E1 + blend*E6
            // E6 enhances E1 for keyword-specific queries
            let blended_score = e1_weight * e1_sim + e6_blend * e6_with_expansion;

            if blended_score >= min_score {
                scored_results.push((cand_id, blended_score, e1_sim, e6_with_expansion, matching_count));
            }
        }

        // Score E6-only candidates (blind spots that E1 missed)
        // These don't have full fingerprints loaded, so we use term overlap count as E6 score
        // and 0.0 for E1 (since E1 didn't find them, their E1 similarity is likely low)
        // The E6 score is normalized: term_count / query_terms to get [0, 1] range
        let query_term_count = query_e6.nnz() as f32;
        if query_term_count > 0.0 {
            for (cand_id, term_overlap) in &e6_candidates {
                // Skip if already scored from E1 results
                if candidate_map.contains_key(cand_id) {
                    continue;
                }

                // E6-only: Use normalized term overlap as E6 score
                // E1 score is 0 since E1 didn't find this candidate
                let e1_sim = 0.0;
                let e6_sim = (*term_overlap as f32) / query_term_count;
                let matching_count = *term_overlap as u32;

                // Blend scores: (1-blend)*E1 + blend*E6
                // For E6-only candidates, E6 contribution dominates (E1 is 0)
                let blended_score = e1_weight * e1_sim + e6_blend * e6_sim;

                if blended_score >= min_score {
                    scored_results.push((*cand_id, blended_score, e1_sim, e6_sim, matching_count));
                }
            }
        }

        // Step 4: Sort by blended score and take top-K
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(top_k);

        let filtered_count = candidates_evaluated - scored_results.len();

        // Step 5: Build results with optional content
        let result_ids: Vec<Uuid> = scored_results.iter().map(|r| r.0).collect();

        // Get content if requested - FAIL FAST on error
        let contents: Vec<Option<String>> = if request.include_content && !result_ids.is_empty() {
            match self.teleological_store.get_content_batch(&result_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        result_count = result_ids.len(),
                        "search_by_keywords: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} results: {}",
                            result_ids.len(),
                            e
                        ),
                    );
                }
            }
        } else {
            vec![None; result_ids.len()]
        };

        // Get source metadata - FAIL FAST on error
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&result_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    result_count = result_ids.len(),
                    "search_by_keywords: Source metadata retrieval FAILED"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} results: {}",
                        result_ids.len(),
                        e
                    ),
                );
            }
        };

        // Build response
        let mut results: Vec<KeywordSearchResult> = Vec::with_capacity(scored_results.len());

        for (i, (memory_id, score, e1_sim, e6_sim, matching_keywords)) in
            scored_results.into_iter().enumerate()
        {
            let source = source_metadata.get(i).and_then(|m| {
                m.as_ref().map(|meta| KeywordSourceInfo {
                    source_type: format!("{}", meta.source_type),
                    file_path: meta.file_path.clone(),
                    hook_type: meta.hook_type.clone(),
                    tool_name: meta.tool_name.clone(),
                })
            });

            results.push(KeywordSearchResult {
                memory_id,
                score,
                e1_similarity: e1_sim,
                e6_keyword_score: e6_sim,
                matching_keywords,
                content: contents.get(i).and_then(|c| c.clone()),
                source,
            });
        }

        let response = SearchByKeywordsResponse {
            query: query.clone(),
            results: results.clone(),
            count: results.len(),
            metadata: KeywordSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                e6_blend_weight: e6_blend,
                e1_weight,
                used_splade_expansion: request.use_splade_expansion,
                extracted_keywords,
            },
        };

        info!(
            results_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "search_by_keywords: Completed keyword-enhanced search"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_by_keywords: Response serialization failed");
                self.tool_error_typed(id, ToolErrorKind::Execution, &format!("Response serialization failed: {}", e))
            }
        }
    }
}

// LOW-15: cosine_similarity moved to super::helpers (shared across 4 tool modules).

/// Compute sparse keyword similarity using weighted Jaccard-like overlap.
///
/// For E6/E13 sparse vectors, computes:
/// - Intersection score: sum of min(query_weight, doc_weight) for matching terms
/// - Union score: sum of max(query_weight, doc_weight) for all terms
/// - Similarity: intersection / union (Jaccard-like)
///
/// Returns (similarity, matching_term_count).
fn sparse_keyword_similarity(
    query: &context_graph_core::types::fingerprint::SparseVector,
    doc: &context_graph_core::types::fingerprint::SparseVector,
) -> (f32, u32) {
    if query.is_empty() || doc.is_empty() {
        return (0.0, 0);
    }

    let mut intersection = 0.0f32;
    let mut union = 0.0f32;
    let mut matching_count = 0u32;

    // Build hashmap of document terms for O(1) lookup
    let doc_terms: std::collections::HashMap<u16, f32> = doc
        .indices
        .iter()
        .zip(doc.values.iter())
        .map(|(&idx, &val)| (idx, val))
        .collect();

    // Compute intersection and track query terms
    for (&q_idx, &q_val) in query.indices.iter().zip(query.values.iter()) {
        if let Some(&d_val) = doc_terms.get(&q_idx) {
            // Term in both: add min to intersection, max to union
            intersection += q_val.min(d_val);
            union += q_val.max(d_val);
            matching_count += 1;
        } else {
            // Term only in query: add to union
            union += q_val;
        }
    }

    // Add doc-only terms to union
    for (&d_idx, &d_val) in doc.indices.iter().zip(doc.values.iter()) {
        if !query.indices.contains(&d_idx) {
            union += d_val;
        }
    }

    let similarity = if union > f32::EPSILON {
        intersection / union
    } else {
        0.0
    };

    (similarity.clamp(0.0, 1.0), matching_count)
}

/// Extract keywords from a query string.
///
/// Simple keyword extraction:
/// - Lowercases the query
/// - Splits on whitespace and punctuation
/// - Filters out common stop words
/// - Returns unique keywords
fn extract_keywords(query: &str) -> Vec<String> {
    // Simple stop words list
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "must", "shall", "can", "need", "to", "of",
        "in", "for", "on", "with", "at", "by", "from", "as", "into", "through",
        "during", "before", "after", "above", "below", "between", "under", "over",
        "again", "further", "then", "once", "here", "there", "when", "where",
        "why", "how", "all", "each", "few", "more", "most", "other", "some",
        "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too",
        "very", "just", "and", "but", "if", "or", "because", "until", "while",
        "about", "against", "this", "that", "these", "those", "what", "which",
        "who", "whom", "i", "me", "my", "we", "our", "you", "your", "he", "him",
        "his", "she", "her", "it", "its", "they", "them", "their",
    ];

    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

    let keywords: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|word| {
            let word = word.trim();
            !word.is_empty() && word.len() >= 2 && !stop_set.contains(word)
        })
        .map(|s| s.to_string())
        .collect();

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    keywords
        .into_iter()
        .filter(|k| seen.insert(k.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::types::fingerprint::SparseVector;

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
    fn test_sparse_keyword_similarity_identical() {
        let query = SparseVector::new(vec![10, 20, 30], vec![1.0, 0.5, 0.3]).unwrap();
        let doc = SparseVector::new(vec![10, 20, 30], vec![1.0, 0.5, 0.3]).unwrap();
        let (sim, count) = sparse_keyword_similarity(&query, &doc);
        assert!((sim - 1.0).abs() < 0.001);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_sparse_keyword_similarity_partial_overlap() {
        let query = SparseVector::new(vec![10, 20, 30], vec![1.0, 0.5, 0.3]).unwrap();
        let doc = SparseVector::new(vec![10, 40, 50], vec![1.0, 0.5, 0.3]).unwrap();
        let (sim, count) = sparse_keyword_similarity(&query, &doc);
        assert!(sim > 0.0 && sim < 1.0);
        assert_eq!(count, 1); // Only term 10 matches
    }

    #[test]
    fn test_sparse_keyword_similarity_no_overlap() {
        let query = SparseVector::new(vec![10, 20, 30], vec![1.0, 0.5, 0.3]).unwrap();
        let doc = SparseVector::new(vec![40, 50, 60], vec![1.0, 0.5, 0.3]).unwrap();
        let (sim, count) = sparse_keyword_similarity(&query, &doc);
        assert!((sim - 0.0).abs() < 0.001);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_extract_keywords_simple() {
        let query = "RocksDB compaction tuning parameters";
        let keywords = extract_keywords(query);
        assert!(keywords.contains(&"rocksdb".to_string()));
        assert!(keywords.contains(&"compaction".to_string()));
        assert!(keywords.contains(&"tuning".to_string()));
        assert!(keywords.contains(&"parameters".to_string()));
    }

    #[test]
    fn test_extract_keywords_filters_stop_words() {
        let query = "the quick brown fox jumps over the lazy dog";
        let keywords = extract_keywords(query);
        // "the" and "over" are stop words
        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"over".to_string()));
        // Content words should remain
        assert!(keywords.contains(&"quick".to_string()));
        assert!(keywords.contains(&"brown".to_string()));
        assert!(keywords.contains(&"fox".to_string()));
    }

    #[test]
    fn test_extract_keywords_handles_punctuation() {
        let query = "Hello, world! How are you?";
        let keywords = extract_keywords(query);
        assert!(keywords.contains(&"hello".to_string()));
        assert!(keywords.contains(&"world".to_string()));
    }

    #[test]
    fn test_extract_keywords_preserves_underscores() {
        let query = "user_id session_token";
        let keywords = extract_keywords(query);
        assert!(keywords.contains(&"user_id".to_string()));
        assert!(keywords.contains(&"session_token".to_string()));
    }

    #[test]
    fn test_extract_keywords_deduplicates() {
        let query = "test test test unique";
        let keywords = extract_keywords(query);
        assert_eq!(keywords.iter().filter(|&k| k == "test").count(), 1);
    }
}
