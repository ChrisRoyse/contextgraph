//! Causal relationship search tool implementation.
//!
//! Provides the `search_causal_relationships` MCP tool for semantic search
//! of LLM-generated causal descriptions with full provenance.

use serde_json::json;
use tracing::{debug, error, info};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

/// Validation constants for search_causal_relationships
const MIN_TOP_K: u64 = 1;
const MAX_TOP_K: u64 = 100;
const DEFAULT_TOP_K: u64 = 10;

impl Handlers {
    /// search_causal_relationships tool implementation.
    ///
    /// Searches for causal relationships by semantic similarity to query.
    /// Returns matching causal descriptions with their source provenance.
    ///
    /// # Arguments (from JSON)
    /// * `query` - Natural language query about causal relationships
    /// * `direction` - Optional filter: "cause", "effect", or "all" (default: "all")
    /// * `topK` - Number of results (1-100, default: 10)
    /// * `includeSource` - Include original source content in results (default: true)
    ///
    /// # Returns
    /// Array of causal relationships with:
    /// - id: Causal relationship UUID
    /// - description: LLM-generated 1-3 paragraph description
    /// - direction: "cause" or "effect"
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

        // Parse direction filter (optional, default: "all")
        let direction_filter = args
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        // Validate direction filter
        if !matches!(direction_filter, "cause" | "effect" | "all") {
            return self.tool_error(
                id,
                &format!(
                    "Invalid direction '{}'. Must be 'cause', 'effect', or 'all'",
                    direction_filter
                ),
            );
        }

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

        info!(
            query_len = query.len(),
            direction_filter = direction_filter,
            top_k = top_k,
            include_source = include_source,
            "search_causal_relationships: Starting search"
        );

        // Step 1: Embed the query using E1 (1024D semantic)
        let query_embedding = match self.multi_array_provider.embed_e1_only(query).await {
            Ok(embedding) => embedding,
            Err(e) => {
                error!(error = %e, "search_causal_relationships: Failed to embed query");
                return self.tool_error(id, &format!("Failed to embed query: {}", e));
            }
        };

        debug!(
            embedding_dim = query_embedding.len(),
            "search_causal_relationships: Query embedded"
        );

        // Step 2: Search causal relationships by similarity
        let direction_opt = if direction_filter == "all" {
            None
        } else {
            Some(direction_filter)
        };

        let search_results = match self
            .teleological_store
            .search_causal_relationships(&query_embedding, top_k, direction_opt)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_causal_relationships: Search failed");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        debug!(
            results_count = search_results.len(),
            "search_causal_relationships: Search complete"
        );

        // Step 3: Fetch full causal relationships and build response
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
                        "description": rel.description,
                        "direction": rel.direction,
                        "confidence": rel.confidence,
                        "keyPhrases": rel.key_phrases,
                        "sourceMemoryId": rel.source_fingerprint_id.to_string(),
                        "similarity": similarity,
                        "createdAt": rel.created_at
                    });

                    // Include source content if requested
                    if include_source {
                        result["sourceContent"] = json!(rel.source_content);
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
            direction_filter = direction_filter,
            top_k = top_k,
            results_count = results.len(),
            "search_causal_relationships: Returning results"
        );

        self.tool_result(
            id,
            json!({
                "results": results,
                "query": query,
                "directionFilter": direction_filter,
                "topK": top_k
            }),
        )
    }
}
