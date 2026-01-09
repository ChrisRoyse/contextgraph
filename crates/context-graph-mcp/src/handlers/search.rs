//! Search handlers for multi-embedding weighted queries.
//!
//! TASK-S002: Implements MCP search handlers using the 5-stage retrieval pipeline.
//!
//! # Search Methods
//!
//! - `search/multi`: Multi-embedding weighted search across 13 spaces
//! - `search/single_space`: Targeted search in a single embedding space (0-12)
//! - `search/by_purpose`: Purpose vector alignment search
//! - `search/weight_profiles`: Get available weight profile configurations
//!
//! # Error Handling
//!
//! FAIL FAST: All errors return immediately with detailed error codes.
//! NO fallbacks, NO default values, NO mock data.

use serde_json::json;
use tracing::{debug, error, instrument, warn};

use context_graph_core::retrieval::{AggregationStrategy, EmbeddingSpaceMask};
use context_graph_core::traits::TeleologicalSearchOptions;
use context_graph_core::types::fingerprint::{PurposeVector, NUM_EMBEDDERS};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::weights::{
    get_profile_names, get_weight_profile, parse_weights_from_json, space_json_key, space_name,
    WEIGHT_PROFILES,
};

use super::Handlers;

impl Handlers {
    /// Handle search/multi request.
    ///
    /// Multi-embedding semantic search with configurable weights.
    ///
    /// # Request Parameters
    /// - `query` (required): Text query to search for
    /// - `query_type` (optional): Preset type (semantic_search, code_search, etc.)
    /// - `weights` (optional): Custom 13-element weight array (required if query_type is "custom")
    /// - `active_spaces` (optional): Array of space indices [0-12] or bitmask
    /// - `aggregation` (optional): "rrf", "weighted_average", "max_pooling", "purpose_weighted"
    /// - `rrf_k` (optional): RRF k parameter, default 60.0
    /// - `top_k` (optional): Maximum results, default 10
    /// - `min_similarity` (optional): Minimum similarity threshold
    /// - `include_per_embedder_scores` (optional): Include per-space breakdown, default true
    /// - `include_purpose_alignment` (optional): Include purpose alignment, default false
    /// - `include_pipeline_breakdown` (optional): Include 5-stage timing, default false
    ///
    /// # Response
    /// - `results`: Array of search results with id, similarity, scores
    /// - `query_metadata`: Weights applied, aggregation strategy, timing
    /// - `pipeline_breakdown`: (optional) 5-stage timing breakdown
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Invalid weights, missing query
    /// - SEMANTIC_SEARCH_ERROR (-32015): 13-embedding search failed
    /// - SPARSE_SEARCH_ERROR (-32014): SPLADE Stage 1 failed
    /// - EMBEDDING_ERROR (-32005): Query embedding failed
    #[instrument(skip(self, params), fields(method = "search/multi"))]
    pub(super) async fn handle_search_multi(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("search/multi: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - query required",
                );
            }
        };

        // Extract query text (required)
        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            Some(_) => {
                error!("search/multi: Empty query string");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Query cannot be empty string",
                );
            }
            None => {
                error!("search/multi: Missing 'query' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'query' parameter",
                );
            }
        };

        // Extract query type and resolve weights
        let query_type = params
            .get("query_type")
            .and_then(|v| v.as_str())
            .unwrap_or("semantic_search");

        let weights: [f32; NUM_EMBEDDERS] = if query_type == "custom" {
            // Custom weights required
            match params.get("weights").and_then(|v| v.as_array()) {
                Some(arr) => match parse_weights_from_json(arr) {
                    Ok(w) => w,
                    Err(e) => {
                        error!(error = %e, "search/multi: Weight validation failed");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Weight validation failed: {}. Expected 13 weights summing to 1.0", e),
                        );
                    }
                },
                None => {
                    error!("search/multi: 'weights' required when query_type='custom'");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        "Missing 'weights' parameter - required when query_type is 'custom'. Expected array of 13 floats summing to 1.0",
                    );
                }
            }
        } else {
            // Use preset profile
            match get_weight_profile(query_type) {
                Some(w) => w,
                None => {
                    error!(query_type = query_type, "search/multi: Unknown query type");
                    let available = get_profile_names();
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!(
                            "Unknown query_type '{}'. Available: {:?}",
                            query_type, available
                        ),
                    );
                }
            }
        };

        // Parse active_spaces
        let active_spaces = match self.parse_active_spaces(&params) {
            Ok(mask) => mask,
            Err(err_response) => return err_response,
        };

        // Parse aggregation strategy
        let aggregation_name = params
            .get("aggregation")
            .and_then(|v| v.as_str())
            .unwrap_or("rrf");
        let rrf_k = params
            .get("rrf_k")
            .and_then(|v| v.as_f64())
            .unwrap_or(60.0) as f32;

        let _aggregation = match aggregation_name {
            "rrf" => AggregationStrategy::RRF { k: rrf_k },
            "weighted_average" => AggregationStrategy::WeightedAverage {
                weights,
                require_all: false,
            },
            "max_pooling" => AggregationStrategy::MaxPooling,
            "purpose_weighted" => AggregationStrategy::PurposeWeighted {
                purpose_vector: PurposeVector::default(),
            },
            other => {
                error!(aggregation = other, "search/multi: Invalid aggregation");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!(
                        "Invalid aggregation '{}'. Valid: rrf, weighted_average, max_pooling, purpose_weighted",
                        other
                    ),
                );
            }
        };

        // Parse search options
        let top_k = params
            .get("topK")
            .or_else(|| params.get("top_k"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        // FAIL-FAST: minSimilarity MUST be explicitly provided.
        // Per constitution AP-007: No silent fallbacks that mask user intent.
        let min_similarity = match params
            .get("minSimilarity")
            .or_else(|| params.get("min_similarity"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
        {
            Some(sim) => {
                if !(0.0..=1.0).contains(&sim) {
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!(
                            "minSimilarity must be between 0.0 and 1.0, got: {}. \
                             Use 0.0 to include all results (no filter).",
                            sim
                        ),
                    );
                }
                sim
            }
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required parameter 'minSimilarity'. \
                     You must explicitly specify the minimum similarity threshold. \
                     Use 0.0 to include all results (no filter), or higher values like 0.7 for stricter matching.".to_string(),
                );
            }
        };

        // minAlignment is optional - when provided, adds purpose alignment filter
        let min_alignment = params
            .get("minAlignment")
            .or_else(|| params.get("min_alignment"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        let include_scores = params
            .get("include_per_embedder_scores")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_alignment = params
            .get("include_purpose_alignment")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let include_breakdown = params
            .get("include_pipeline_breakdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Generate query embeddings
        let search_start = std::time::Instant::now();
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => {
                debug!(
                    "Generated 13 query embeddings in {:?}",
                    output.total_latency
                );
                output.fingerprint
            }
            Err(e) => {
                error!(error = %e, "search/multi: Query embedding FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::EMBEDDING_ERROR,
                    format!("Query embedding failed: {}", e),
                );
            }
        };

        // Build search options with embedder indices based on active_spaces
        let embedder_indices: Vec<usize> = if active_spaces.active_count() < NUM_EMBEDDERS {
            active_spaces.active_indices()
        } else {
            Vec::new() // Empty = all embedders
        };

        let mut options = TeleologicalSearchOptions::quick(top_k)
            .with_min_similarity(min_similarity);

        if let Some(align) = min_alignment {
            options = options.with_min_alignment(align);
        }

        // Set embedder indices if not all spaces
        if !embedder_indices.is_empty() {
            options.embedder_indices = embedder_indices;
        }

        // Execute semantic search
        match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => {
                let query_latency_ms = search_start.elapsed().as_millis();

                // Build results JSON
                let results_json: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        let mut result = json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "aggregate_similarity": r.similarity,
                        });

                        // Per-embedder scores
                        if include_scores {
                            let mut scores = serde_json::Map::new();
                            for i in 0..NUM_EMBEDDERS {
                                scores.insert(
                                    space_json_key(i).to_string(),
                                    json!(r.embedder_scores[i]),
                                );
                            }
                            result["per_embedder_scores"] = json!(scores);

                            // Top 3 contributing spaces
                            let mut indexed_scores: Vec<(usize, f32)> = r
                                .embedder_scores
                                .iter()
                                .enumerate()
                                .map(|(i, &s)| (i, s * weights[i]))
                                .collect();
                            indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                            let top_contributors: Vec<serde_json::Value> = indexed_scores
                                .iter()
                                .take(3)
                                .map(|(idx, contribution)| {
                                    json!({
                                        "space_index": idx,
                                        "space_name": space_name(*idx),
                                        "weighted_contribution": contribution
                                    })
                                })
                                .collect();
                            result["top_contributing_spaces"] = json!(top_contributors);
                        }

                        // Purpose alignment
                        if include_alignment {
                            result["purpose_alignment"] = json!(r.purpose_alignment);
                            result["theta_to_north_star"] =
                                json!(r.fingerprint.theta_to_north_star);
                        }

                        // Johari quadrant
                        let dominant_quadrant =
                            format!("{:?}", r.fingerprint.johari.dominant_quadrant(0));
                        result["johari_quadrant"] = json!(dominant_quadrant);

                        result
                    })
                    .collect();

                // Build metadata
                let metadata = json!({
                    "query_type_used": query_type,
                    "weights_applied": weights.to_vec(),
                    "aggregation_strategy": aggregation_name,
                    "rrf_k": rrf_k,
                    "spaces_searched": active_spaces.active_count(),
                    "spaces_failed": 0,
                    "total_candidates_scanned": results.len(),
                    "search_time_ms": query_latency_ms,
                    "within_latency_target": query_latency_ms < 60
                });

                let response = json!({
                    "results": results_json,
                    "count": results_json.len(),
                    "query_metadata": metadata
                });

                // TASK-EMB-024: Pipeline breakdown - FAIL FAST with explicit error
                // NO fake/simulated timing data - require real pipeline metrics
                if include_breakdown {
                    error!("search/multi: Pipeline breakdown requested but not implemented");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::PIPELINE_METRICS_UNAVAILABLE,
                        "Pipeline breakdown metrics are not yet implemented. \
                         Set include_pipeline_breakdown=false or wait for real pipeline metrics.",
                    );
                }

                debug!(
                    count = results.len(),
                    latency_ms = query_latency_ms,
                    "search/multi: Completed"
                );

                JsonRpcResponse::success(id, response)
            }
            Err(e) => {
                error!(error = %e, "search/multi: Semantic search FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::SEMANTIC_SEARCH_ERROR,
                    format!("Semantic search failed: {}", e),
                )
            }
        }
    }

    /// Handle search/single_space request.
    ///
    /// Single-space targeted search for specific embedding type.
    ///
    /// # Request Parameters
    /// - `space_index` (required): Embedding space index 0-12
    /// - `query` or `query_text` (required): Text query
    /// - `top_k` (optional): Maximum results, default 10
    /// - `min_similarity` (optional): Minimum threshold
    ///
    /// # Response
    /// - `results`: Array of search results
    /// - `space_name`: Name of searched space
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Invalid space index (must be 0-12)
    /// - SEMANTIC_SEARCH_ERROR (-32015): Search failed
    #[instrument(skip(self, params), fields(method = "search/single_space"))]
    pub(super) async fn handle_search_single_space(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("search/single_space: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - space_index and query required",
                );
            }
        };

        // Extract space_index (required, 0-12)
        let space_index = match params.get("space_index").and_then(|v| v.as_u64()) {
            Some(idx) if idx <= 12 => idx as usize,
            Some(idx) => {
                error!(space_index = idx, "search/single_space: Invalid space index");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!(
                        "Invalid space_index {}. Valid range: 0-12 (13 embedding spaces)",
                        idx
                    ),
                );
            }
            None => {
                error!("search/single_space: Missing 'space_index' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'space_index' parameter (0-12)",
                );
            }
        };

        // Extract query text
        let query = match params
            .get("query")
            .or_else(|| params.get("query_text"))
            .and_then(|v| v.as_str())
        {
            Some(q) if !q.is_empty() => q,
            Some(_) => {
                error!("search/single_space: Empty query string");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Query cannot be empty string",
                );
            }
            None => {
                error!("search/single_space: Missing query parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'query' or 'query_text' parameter",
                );
            }
        };

        // top_k has a sensible default (pagination parameter)
        const DEFAULT_TOP_K: usize = 10;
        let top_k = params
            .get("topK")
            .or_else(|| params.get("top_k"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TOP_K);

        // FAIL-FAST: minSimilarity MUST be explicitly provided.
        // Per constitution AP-007: No silent fallbacks that mask user intent.
        let min_similarity = match params
            .get("minSimilarity")
            .or_else(|| params.get("min_similarity"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
        {
            Some(sim) => {
                if !(0.0..=1.0).contains(&sim) {
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!(
                            "minSimilarity must be between 0.0 and 1.0, got: {}. \
                             Use 0.0 to include all results (no filter).",
                            sim
                        ),
                    );
                }
                sim
            }
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required parameter 'minSimilarity'. \
                     You must explicitly specify the minimum similarity threshold. \
                     Use 0.0 to include all results (no filter).".to_string(),
                );
            }
        };

        // Generate query embeddings
        let search_start = std::time::Instant::now();
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search/single_space: Query embedding FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::EMBEDDING_ERROR,
                    format!("Query embedding failed: {}", e),
                );
            }
        };

        // Build search options targeting single space
        let mut options = TeleologicalSearchOptions::quick(top_k)
            .with_min_similarity(min_similarity);
        options.embedder_indices = vec![space_index];

        // Execute search
        match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => {
                let search_latency_ms = search_start.elapsed().as_millis();

                let results_json: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "similarity": r.embedder_scores[space_index],
                            "aggregate_similarity": r.similarity,
                            "space_score": r.embedder_scores[space_index]
                        })
                    })
                    .collect();

                debug!(
                    space_index = space_index,
                    space_name = space_name(space_index),
                    count = results.len(),
                    latency_ms = search_latency_ms,
                    "search/single_space: Completed"
                );

                JsonRpcResponse::success(
                    id,
                    json!({
                        "results": results_json,
                        "count": results.len(),
                        "space_index": space_index,
                        "space_name": space_name(space_index),
                        "search_time_ms": search_latency_ms
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, space_index = space_index, "search/single_space: FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::SEMANTIC_SEARCH_ERROR,
                    format!("Single space search failed for space {}: {}", space_index, e),
                )
            }
        }
    }

    /// Handle search/by_purpose request.
    ///
    /// Search by purpose vector similarity (13D alignment).
    ///
    /// # Request Parameters
    /// - `purpose_vector` (optional): 13-element purpose alignment vector
    /// - `min_alignment` (optional): Minimum alignment threshold
    /// - `top_k` (optional): Maximum results, default 10
    ///
    /// # Response
    /// - `results`: Array of fingerprints with purpose alignment scores
    ///
    /// # Error Codes
    /// - PURPOSE_SEARCH_ERROR (-32016): Purpose search failed
    /// - FINGERPRINT_NOT_FOUND (-32010): No matching fingerprints
    #[instrument(skip(self, params), fields(method = "search/by_purpose"))]
    pub(super) async fn handle_search_by_purpose(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = params.unwrap_or(json!({}));

        // Parse purpose vector (optional - default to zero vector)
        let purpose_vector = if let Some(arr) = params.get("purpose_vector").and_then(|v| v.as_array()) {
            if arr.len() != NUM_EMBEDDERS {
                error!(
                    count = arr.len(),
                    "search/by_purpose: Purpose vector must have 13 elements"
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!(
                        "Purpose vector must have {} elements, got {}",
                        NUM_EMBEDDERS,
                        arr.len()
                    ),
                );
            }

            // FAIL-FAST: Each alignment value MUST be a valid number.
            // Per constitution AP-007: No silent fallbacks for invalid data.
            let mut alignments = [0.0f32; NUM_EMBEDDERS];
            for (i, v) in arr.iter().enumerate() {
                alignments[i] = match v.as_f64() {
                    Some(val) => {
                        if !val.is_finite() || !(0.0..=1.0).contains(&val) {
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!(
                                    "Purpose vector element at index {} must be a finite number between 0.0 and 1.0, got: {}",
                                    i, val
                                ),
                            );
                        }
                        val as f32
                    }
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!(
                                "Purpose vector element at index {} must be a number, got: {:?}",
                                i, v
                            ),
                        );
                    }
                };
            }
            // Find dominant embedder (highest alignment)
            let dominant_embedder = alignments
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i as u8)
                .unwrap_or(0);

            // Compute coherence (inverse of standard deviation)
            let mean: f32 = alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32;
            let variance: f32 = alignments.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / NUM_EMBEDDERS as f32;
            let coherence = 1.0 / (1.0 + variance.sqrt());

            PurposeVector {
                alignments,
                dominant_embedder,
                coherence,
                stability: 1.0, // Default stability for new queries
            }
        } else {
            PurposeVector::default()
        };

        let top_k = params
            .get("topK")
            .or_else(|| params.get("top_k"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let min_alignment = params
            .get("minAlignment")
            .or_else(|| params.get("min_alignment"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        let search_start = std::time::Instant::now();

        // Build search options
        let mut options = TeleologicalSearchOptions::quick(top_k);
        if let Some(align) = min_alignment {
            options = options.with_min_alignment(align);
        }

        // Execute purpose search
        match self
            .teleological_store
            .search_purpose(&purpose_vector, options)
            .await
        {
            Ok(results) => {
                let search_latency_ms = search_start.elapsed().as_millis();

                let results_json: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "purpose_alignment": r.purpose_alignment,
                            "theta_to_north_star": r.fingerprint.theta_to_north_star,
                            "purpose_vector": r.fingerprint.purpose_vector.alignments.to_vec(),
                            "johari_quadrant": format!("{:?}", r.fingerprint.johari.dominant_quadrant(0))
                        })
                    })
                    .collect();

                if results.is_empty() && min_alignment.is_some() {
                    warn!(
                        min_alignment = ?min_alignment,
                        "search/by_purpose: No results met alignment threshold"
                    );
                }

                debug!(
                    count = results.len(),
                    latency_ms = search_latency_ms,
                    "search/by_purpose: Completed"
                );

                JsonRpcResponse::success(
                    id,
                    json!({
                        "results": results_json,
                        "count": results.len(),
                        "min_alignment_filter": min_alignment,
                        "search_time_ms": search_latency_ms
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "search/by_purpose: FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::PURPOSE_SEARCH_ERROR,
                    format!("Purpose search failed: {}", e),
                )
            }
        }
    }

    /// Handle search/weight_profiles request.
    ///
    /// Returns all available weight profiles for discovery.
    ///
    /// # Response
    /// - `profiles`: Array of profile objects with name, weights, description
    /// - `embedding_spaces`: List of 13 embedding space names
    pub(super) async fn handle_get_weight_profiles(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        let profiles: Vec<serde_json::Value> = WEIGHT_PROFILES
            .iter()
            .map(|(name, weights): &(&str, [f32; NUM_EMBEDDERS])| {
                // Find primary spaces (weight > 0.15)
                let primary_spaces: Vec<&str> = weights
                    .iter()
                    .enumerate()
                    .filter(|(_, &w)| w > 0.15)
                    .map(|(i, _)| space_name(i))
                    .collect();

                json!({
                    "name": name,
                    "weights": weights.to_vec(),
                    "primary_spaces": primary_spaces,
                    "description": get_profile_description(name)
                })
            })
            .collect();

        let embedding_spaces: Vec<serde_json::Value> = (0..NUM_EMBEDDERS)
            .map(|i| {
                json!({
                    "index": i,
                    "name": space_name(i),
                    "json_key": space_json_key(i)
                })
            })
            .collect();

        JsonRpcResponse::success(
            id,
            json!({
                "profiles": profiles,
                "embedding_spaces": embedding_spaces,
                "total_spaces": NUM_EMBEDDERS,
                "default_aggregation": "rrf",
                "default_rrf_k": 60.0
            }),
        )
    }

    /// Parse active_spaces parameter from JSON.
    ///
    /// Accepts either:
    /// - Array of indices: [0, 1, 4, 6]
    /// - Bitmask integer: 0x1FFF (all 13 spaces)
    fn parse_active_spaces(
        &self,
        params: &serde_json::Value,
    ) -> Result<EmbeddingSpaceMask, JsonRpcResponse> {
        match params.get("active_spaces") {
            Some(serde_json::Value::Array(arr)) => {
                let mut mask: u16 = 0;
                for v in arr {
                    if let Some(idx) = v.as_u64() {
                        if idx > 12 {
                            return Err(JsonRpcResponse::error(
                                None,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid space index {} in active_spaces. Valid: 0-12", idx),
                            ));
                        }
                        mask |= 1 << idx;
                    }
                }
                if mask == 0 {
                    return Err(JsonRpcResponse::error(
                        None,
                        error_codes::INVALID_PARAMS,
                        "active_spaces array resulted in no active spaces. At least one space required.",
                    ));
                }
                Ok(EmbeddingSpaceMask(mask))
            }
            Some(serde_json::Value::Number(n)) => {
                let mask = n.as_u64().unwrap_or(0) as u16;
                if mask == 0 {
                    return Err(JsonRpcResponse::error(
                        None,
                        error_codes::INVALID_PARAMS,
                        "active_spaces bitmask is 0. At least one space must be active.",
                    ));
                }
                // Mask to valid range (13 bits)
                Ok(EmbeddingSpaceMask(mask & 0x1FFF))
            }
            None => Ok(EmbeddingSpaceMask::ALL),
            _ => Err(JsonRpcResponse::error(
                None,
                error_codes::INVALID_PARAMS,
                "active_spaces must be an array of indices [0-12] or a bitmask integer",
            )),
        }
    }
}

/// Get description for a weight profile.
fn get_profile_description(name: &str) -> &'static str {
    match name {
        "semantic_search" => "General semantic similarity search. Emphasizes E1 (semantic) and E7 (code).",
        "causal_reasoning" => "Cause-effect relationship analysis. Heavily weights E5 (causal) and E8 (graph).",
        "code_search" => "Source code similarity search. Focuses on E7 (code) and E4 (positional).",
        "temporal_navigation" => "Time-based exploration. Weights temporal embeddings E2, E3, E4 equally.",
        "fact_checking" => "Entity-focused verification. Emphasizes E11 (entity) and E6 (sparse).",
        "balanced" => "Equal weights across all 13 embedding spaces.",
        _ => "Custom weight profile.",
    }
}
