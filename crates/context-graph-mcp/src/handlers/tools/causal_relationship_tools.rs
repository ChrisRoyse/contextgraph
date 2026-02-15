//! Causal relationship search tool implementation.
//!
//! Provides the `search_causal_relationships` MCP tool for semantic search
//! of LLM-generated causal descriptions with full provenance.
//!
//! # E5 Asymmetric Search
//!
//! Supports directional causal search using E5 dual embeddings:
//! - `searchMode: "causes"` → "What caused X?" → query as effect, search cause vectors
//! - `searchMode: "effects"` → "What are effects of X?" → query as cause, search effect vectors
//! - `searchMode: "semantic"` → Fallback E1 semantic search (default)
//!
//! # Multi-Embedder Search (Maximum Accuracy)
//!
//! When `multiEmbedder: true`, uses 4 embedders for maximum accuracy:
//! - E1: Semantic foundation
//! - E5: Causal (asymmetric, directional)
//! - E8: Graph structure
//! - E11: Entity knowledge graph
//!
//! Results include consensus scores and per-embedder scores.

use serde_json::json;
use tracing::{debug, error, info};

use context_graph_core::types::MultiEmbedderConfig;

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

/// Validation constants for search_causal_relationships
const MIN_TOP_K: u64 = 1;
const MAX_TOP_K: u64 = 100;
const DEFAULT_TOP_K: u64 = 10;

/// Default hybrid search weights
/// Source-anchored embeddings prevent LLM output clustering
const DEFAULT_SOURCE_WEIGHT: f32 = 0.6;
const DEFAULT_EXPLANATION_WEIGHT: f32 = 0.4;

/// Default multi-embedder weights (optimized for causal search accuracy)
const DEFAULT_E1_WEIGHT: f32 = 0.30;
const DEFAULT_E5_WEIGHT: f32 = 0.35;
const DEFAULT_E8_WEIGHT: f32 = 0.15;
const DEFAULT_E11_WEIGHT: f32 = 0.20;

impl Handlers {
    /// search_causal_relationships tool implementation.
    ///
    /// Searches for causal relationships by semantic similarity to query.
    /// Returns matching causal descriptions with their source provenance.
    ///
    /// # Arguments (from JSON)
    /// * `query` - Natural language query about causal relationships
    /// * `searchMode` - Search strategy (default: "semantic"):
    ///   - "causes": Find what caused X (query as effect, search cause vectors)
    ///   - "effects": Find effects of X (query as cause, search effect vectors)
    ///   - "semantic": Fallback E1 semantic search
    /// * `topK` - Number of results (1-100, default: 10)
    /// * `includeSource` - Include original source content in results (default: true)
    ///
    /// # Returns
    /// Array of causal relationships with:
    /// - id: Causal relationship UUID
    /// - causeStatement: Brief statement of the cause
    /// - effectStatement: Brief statement of the effect
    /// - explanation: LLM-generated 1-2 paragraph explanation
    /// - mechanismType: "direct", "mediated", "feedback", or "temporal"
    /// - confidence: LLM confidence score
    /// - sourceContent: Original content (if includeSource=true)
    /// - sourceMemoryId: ID of source memory for provenance
    /// - similarity: Search similarity score
    pub(crate) async fn call_search_causal_relationships(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse query parameter (required)
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            Some(_) => return self.tool_error(id, "Query cannot be empty"),
            None => return self.tool_error(id, "Missing 'query' parameter"),
        };

        // Parse direction parameter (schema-aligned, replaces legacy searchMode)
        // Schema enum: ["cause", "effect", "all"], default: "all"
        let direction = args
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        // Validate direction
        if !matches!(direction, "cause" | "effect" | "all") {
            return self.tool_error(
                id,
                &format!(
                    "Invalid direction '{}'. Must be 'cause', 'effect', or 'all'",
                    direction
                ),
            );
        }

        // Map schema direction to internal search mode for the search paths below
        // "cause" → search for causes (query as effect, search cause vectors)
        // "effect" → search for effects (query as cause, search effect vectors)
        // "all" → semantic fallback (E1 search, no direction filtering)
        let search_mode = match direction {
            "cause" => "causes",
            "effect" => "effects",
            _ => "semantic",
        };

        // Parse topK parameter (optional, default: 10, range: 1-100)
        let raw_top_k = args.get("topK").and_then(|v| v.as_u64());
        if let Some(k) = raw_top_k {
            if k < MIN_TOP_K {
                return self.tool_error(
                    id,
                    &format!("topK must be at least {}, got {}", MIN_TOP_K, k),
                );
            }
            if k > MAX_TOP_K {
                return self.tool_error(
                    id,
                    &format!("topK must be at most {}, got {}", MAX_TOP_K, k),
                );
            }
        }
        let top_k = raw_top_k.unwrap_or(DEFAULT_TOP_K) as usize;

        // Parse includeSource parameter (optional, default: true)
        let include_source = args
            .get("includeSource")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // PHASE-2-PROVENANCE: Parse includeProvenance parameter (default: false)
        let include_provenance = args
            .get("includeProvenance")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Parse hybrid search weight parameters (optional)
        // sourceWeight: weight for source-anchored embeddings (0.0-1.0, default 0.6)
        // explanationWeight: weight for explanation embeddings (0.0-1.0, default 0.4)
        let source_weight = args
            .get("sourceWeight")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(DEFAULT_SOURCE_WEIGHT);

        let explanation_weight = args
            .get("explanationWeight")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(DEFAULT_EXPLANATION_WEIGHT);

        // Validate weights are in range [0.0, 1.0]
        if !(0.0..=1.0).contains(&source_weight) || !(0.0..=1.0).contains(&explanation_weight) {
            return self.tool_error(
                id,
                "sourceWeight and explanationWeight must be between 0.0 and 1.0",
            );
        }

        // Parse multiEmbedder parameter (optional, default: false for backwards compatibility)
        let multi_embedder = args
            .get("multiEmbedder")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Parse multi-embedder weight parameters
        let e1_weight = args
            .get("e1Weight")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(DEFAULT_E1_WEIGHT);

        let e5_weight = args
            .get("e5Weight")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(DEFAULT_E5_WEIGHT);

        let e8_weight = args
            .get("e8Weight")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(DEFAULT_E8_WEIGHT);

        let e11_weight = args
            .get("e11Weight")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(DEFAULT_E11_WEIGHT);

        // Parse minimum consensus threshold for multi-embedder search
        let min_consensus = args
            .get("minConsensus")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        info!(
            query_len = query.len(),
            search_mode = search_mode,
            top_k = top_k,
            include_source = include_source,
            source_weight = source_weight,
            explanation_weight = explanation_weight,
            multi_embedder = multi_embedder,
            "search_causal_relationships: Starting search"
        );

        // Multi-embedder search path for maximum accuracy
        if multi_embedder && matches!(search_mode, "causes" | "effects") {
            let search_causes = search_mode == "causes";

            // Build multi-embedder config
            let config = MultiEmbedderConfig {
                e1_weight,
                e5_weight,
                e8_weight,
                e11_weight,
                enable_e12_rerank: false, // Not implemented yet
                min_consensus,
            };

            // Generate all embeddings in parallel
            // E1: semantic, E5: causal dual, E8: graph dual, E11: entity
            let e1_result = self.multi_array_provider.embed_e1_only(query).await;
            let e5_result = self.multi_array_provider.embed_e5_dual(query).await;
            let e8_result = self.multi_array_provider.embed_e8_dual(query).await;
            let e11_result = self.multi_array_provider.embed_e11_only(query).await;

            let e1_embedding = match e1_result {
                Ok(e) => e,
                Err(e) => {
                    error!(error = %e, "multi-embedder: Failed to embed E1");
                    return self.tool_error(id, &format!("Failed to embed query: {}", e));
                }
            };

            let (e5_cause, e5_effect) = match e5_result {
                Ok(dual) => dual,
                Err(e) => {
                    error!(error = %e, "multi-embedder: Failed to embed E5");
                    return self.tool_error(id, &format!("Failed to embed query: {}", e));
                }
            };

            // Select E5 embedding based on search direction
            let e5_embedding = if search_causes {
                e5_effect // Query as effect, looking for causes
            } else {
                e5_cause // Query as cause, looking for effects
            };

            let (e8_source, e8_target) = match e8_result {
                Ok(dual) => dual,
                Err(e) => {
                    error!(error = %e, "multi-embedder: Failed to embed E8");
                    return self.tool_error(id, &format!("Failed to embed query: {}", e));
                }
            };

            // Select E8 embedding based on search direction
            let e8_embedding = if search_causes {
                e8_target // Query as target, looking for sources
            } else {
                e8_source // Query as source, looking for targets
            };

            let e11_embedding = match e11_result {
                Ok(e) => e,
                Err(e) => {
                    error!(error = %e, "multi-embedder: Failed to embed E11");
                    return self.tool_error(id, &format!("Failed to embed query: {}", e));
                }
            };

            debug!(
                e1_dim = e1_embedding.len(),
                e5_dim = e5_embedding.len(),
                e8_dim = e8_embedding.len(),
                e11_dim = e11_embedding.len(),
                search_causes = search_causes,
                "multi-embedder: All embeddings generated"
            );

            // Run multi-embedder search
            let multi_results = match self
                .teleological_store
                .search_causal_multi_embedder(
                    &e1_embedding,
                    &e5_embedding,
                    &e8_embedding,
                    &e11_embedding,
                    search_causes,
                    top_k,
                    &config,
                )
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    error!(error = %e, "multi-embedder: Search failed");
                    return self.tool_error(id, &format!("Search failed: {}", e));
                }
            };

            debug!(
                results_count = multi_results.len(),
                "multi-embedder: Search complete"
            );

            // Build response with multi-embedder scores
            let mut results = Vec::with_capacity(multi_results.len());

            for search_result in multi_results {
                if let Some(rel) = &search_result.relationship {
                    let mut result = json!({
                        "id": rel.id.to_string(),
                        "causeStatement": rel.cause_statement,
                        "effectStatement": rel.effect_statement,
                        "explanation": rel.explanation,
                        "mechanismType": rel.mechanism_type,
                        "confidence": rel.confidence,
                        "sourceMemoryId": rel.source_fingerprint_id.to_string(),
                        "similarity": search_result.rrf_score,
                        "createdAt": rel.created_at,
                        // Multi-embedder specific fields
                        "consensusScore": search_result.consensus_score,
                        "directionConfidence": search_result.direction_confidence,
                        "perEmbedderScores": search_result.per_embedder_scores()
                    });

                    // Include source content if requested
                    if include_source {
                        result["sourceContent"] = json!(rel.source_content);
                    }

                    // Fetch source metadata for provenance tracking
                    if let Ok(Some(source_meta)) = self
                        .teleological_store
                        .get_source_metadata(rel.source_fingerprint_id)
                        .await
                    {
                        let provenance = json!({
                            "filePath": source_meta.file_path.as_deref().unwrap_or("unknown"),
                            "lineStart": source_meta.start_line.unwrap_or(0),
                            "lineEnd": source_meta.end_line.unwrap_or(0),
                            "sourceType": format!("{:?}", source_meta.source_type),
                        });
                        result["provenance"] = provenance;
                    }

                    // PHASE-2-PROVENANCE: Add retrieval provenance when requested
                    if include_provenance {
                        result["retrievalProvenance"] = json!({
                            "searchMode": search_mode,
                            "multiEmbedder": true,
                            "e5AsymmetricUsed": matches!(search_mode, "causes" | "effects"),
                            "searchDirection": if search_mode == "causes" {
                                "query_as_effect_seeking_causes"
                            } else {
                                "query_as_cause_seeking_effects"
                            },
                            "similarity": search_result.rrf_score,
                            "consensusScore": search_result.consensus_score,
                            "directionConfidence": search_result.direction_confidence,
                            "perEmbedderScores": search_result.per_embedder_scores(),
                            "embedderWeights": {
                                "e1": e1_weight,
                                "e5": e5_weight,
                                "e8": e8_weight,
                                "e11": e11_weight
                            },
                            "llmProvenance": rel.llm_provenance.as_ref().map(|p| json!({
                                "modelName": p.model_name,
                                "modelVersion": p.model_version,
                                "quantization": p.quantization,
                                "temperature": p.temperature,
                                "maxTokens": p.max_tokens,
                                "promptTemplateHash": p.prompt_template_hash,
                                "grammarType": p.grammar_type,
                                "tokensConsumed": p.tokens_consumed,
                                "generationTimeMs": p.generation_time_ms
                            }))
                        });
                    }

                    results.push(result);
                }
            }

            return JsonRpcResponse::success(
                id,
                json!({
                    "results": results,
                    "totalFound": results.len(),
                    "searchMode": search_mode,
                    "multiEmbedder": true,
                }),
            );
        }

        // Step 1 & 2: Embed query and search based on mode (single-embedder path)
        let search_results = match search_mode {
            "causes" => {
                // "What caused X?" → query as effect, search cause vectors
                // Uses hybrid scoring: 0.6 * source + 0.4 * explanation to prevent clustering
                let e5_result = self.multi_array_provider.embed_e5_dual(query).await;
                let (_as_cause, as_effect) = match e5_result {
                    Ok(dual) => dual,
                    Err(e) => {
                        error!(error = %e, "search_causal_relationships: Failed to embed query for E5");
                        return self.tool_error(id, &format!("Failed to embed query: {}", e));
                    }
                };

                debug!(
                    embedding_dim = as_effect.len(),
                    mode = "causes",
                    source_weight = source_weight,
                    explanation_weight = explanation_weight,
                    "search_causal_relationships: Query embedded as effect (hybrid search)"
                );

                match self
                    .teleological_store
                    .search_causal_e5_hybrid(
                        &as_effect,
                        true,
                        top_k,
                        source_weight,
                        explanation_weight,
                    )
                    .await
                {
                    Ok(results) => results,
                    Err(e) => {
                        error!(error = %e, "search_causal_relationships: E5 hybrid search failed");
                        return self.tool_error(id, &format!("Search failed: {}", e));
                    }
                }
            }
            "effects" => {
                // "What are effects of X?" → query as cause, search effect vectors
                // Uses hybrid scoring: 0.6 * source + 0.4 * explanation to prevent clustering
                let e5_result = self.multi_array_provider.embed_e5_dual(query).await;
                let (as_cause, _as_effect) = match e5_result {
                    Ok(dual) => dual,
                    Err(e) => {
                        error!(error = %e, "search_causal_relationships: Failed to embed query for E5");
                        return self.tool_error(id, &format!("Failed to embed query: {}", e));
                    }
                };

                debug!(
                    embedding_dim = as_cause.len(),
                    mode = "effects",
                    source_weight = source_weight,
                    explanation_weight = explanation_weight,
                    "search_causal_relationships: Query embedded as cause (hybrid search)"
                );

                match self
                    .teleological_store
                    .search_causal_e5_hybrid(
                        &as_cause,
                        false,
                        top_k,
                        source_weight,
                        explanation_weight,
                    )
                    .await
                {
                    Ok(results) => results,
                    Err(e) => {
                        error!(error = %e, "search_causal_relationships: E5 hybrid search failed");
                        return self.tool_error(id, &format!("Search failed: {}", e));
                    }
                }
            }
            _ => {
                // "semantic" - Fallback E1 semantic search
                let query_embedding = match self.multi_array_provider.embed_e1_only(query).await {
                    Ok(embedding) => embedding,
                    Err(e) => {
                        error!(error = %e, "search_causal_relationships: Failed to embed query");
                        return self.tool_error(id, &format!("Failed to embed query: {}", e));
                    }
                };

                debug!(
                    embedding_dim = query_embedding.len(),
                    mode = "semantic",
                    "search_causal_relationships: Query embedded for E1 search"
                );

                match self
                    .teleological_store
                    .search_causal_relationships(&query_embedding, top_k, None)
                    .await
                {
                    Ok(results) => results,
                    Err(e) => {
                        error!(error = %e, "search_causal_relationships: Search failed");
                        return self.tool_error(id, &format!("Search failed: {}", e));
                    }
                }
            }
        };

        debug!(
            results_count = search_results.len(),
            "search_causal_relationships: Search complete"
        );

        // Step 3: Fetch full causal relationships and build response with provenance
        let mut results = Vec::with_capacity(search_results.len());

        for (causal_id, similarity) in search_results {
            match self
                .teleological_store
                .get_causal_relationship(causal_id)
                .await
            {
                Ok(Some(rel)) => {
                    let mut result = json!({
                        "id": rel.id.to_string(),
                        "causeStatement": rel.cause_statement,
                        "effectStatement": rel.effect_statement,
                        "explanation": rel.explanation,
                        "mechanismType": rel.mechanism_type,
                        "confidence": rel.confidence,
                        "sourceMemoryId": rel.source_fingerprint_id.to_string(),
                        "similarity": similarity,
                        "createdAt": rel.created_at
                    });

                    // Include source content if requested
                    if include_source {
                        result["sourceContent"] = json!(rel.source_content);
                    }

                    // Fetch source metadata for provenance tracking
                    if let Ok(Some(source_meta)) = self
                        .teleological_store
                        .get_source_metadata(rel.source_fingerprint_id)
                        .await
                    {
                        // Build provenance object with file path and line numbers
                        let provenance = json!({
                            "sourceType": format!("{}", source_meta.source_type),
                            "filePath": source_meta.file_path,
                            "startLine": source_meta.start_line,
                            "endLine": source_meta.end_line,
                            "chunkIndex": source_meta.chunk_index,
                            "totalChunks": source_meta.total_chunks,
                            "hookType": source_meta.hook_type,
                            "toolName": source_meta.tool_name,
                            "displayString": source_meta.display_string(),
                        });
                        result["provenance"] = provenance;
                    }

                    // Include extraction spans for fine-grained provenance
                    if !rel.source_spans.is_empty() {
                        let extraction_spans: Vec<serde_json::Value> = rel
                            .source_spans
                            .iter()
                            .map(|s| {
                                json!({
                                    "startChar": s.start_char,
                                    "endChar": s.end_char,
                                    "textExcerpt": s.text_excerpt,
                                    "spanType": s.span_type,
                                })
                            })
                            .collect();
                        result["extractionSpans"] = json!(extraction_spans);
                    }

                    // PHASE-2-PROVENANCE: Add retrieval provenance when requested
                    if include_provenance {
                        result["retrievalProvenance"] = json!({
                            "searchMode": search_mode,
                            "multiEmbedder": false,
                            "e5AsymmetricUsed": matches!(search_mode, "causes" | "effects"),
                            "searchDirection": match search_mode {
                                "causes" => "query_as_effect_seeking_causes",
                                "effects" => "query_as_cause_seeking_effects",
                                _ => "semantic_undirected",
                            },
                            "similarity": similarity,
                            "hybridWeights": {
                                "sourceWeight": source_weight,
                                "explanationWeight": explanation_weight
                            },
                            "llmProvenance": rel.llm_provenance.as_ref().map(|p| json!({
                                "modelName": p.model_name,
                                "modelVersion": p.model_version,
                                "quantization": p.quantization,
                                "temperature": p.temperature,
                                "maxTokens": p.max_tokens,
                                "promptTemplateHash": p.prompt_template_hash,
                                "grammarType": p.grammar_type,
                                "tokensConsumed": p.tokens_consumed,
                                "generationTimeMs": p.generation_time_ms
                            }))
                        });
                    }

                    results.push(result);
                }
                Ok(None) => {
                    // Causal relationship not found (should be rare)
                    debug!(
                        causal_id = %causal_id,
                        "search_causal_relationships: Causal relationship not found"
                    );
                }
                Err(e) => {
                    // Log error but continue with other results
                    error!(
                        causal_id = %causal_id,
                        error = %e,
                        "search_causal_relationships: Failed to fetch causal relationship"
                    );
                }
            }
        }

        info!(
            query_preview = &query[..query.len().min(50)],
            search_mode = search_mode,
            top_k = top_k,
            results_count = results.len(),
            "search_causal_relationships: Returning results"
        );

        self.tool_result(
            id,
            json!({
                "results": results,
                "query": query,
                "searchMode": search_mode,
                "topK": top_k
            }),
        )
    }
}
