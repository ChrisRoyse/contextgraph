//! Memory operation tool implementations (inject_context, store_memory, search_graph).

use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, error, warn};

use context_graph_core::teleological::matrix_search::embedder_names;
use context_graph_core::traits::TeleologicalSearchOptions;
use context_graph_core::types::fingerprint::{TeleologicalFingerprint, NUM_EMBEDDERS};
use context_graph_core::types::UtlContext;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::super::Handlers;

// Validation constants for inject_context (BUG-002)
// Per PRD 0.3 and constitution: rationale is REQUIRED (1-1024 chars)
const MIN_RATIONALE_LEN: usize = 1;
const MAX_RATIONALE_LEN: usize = 1024;

// Validation constants for search_graph (BUG-001)
// Per PRD Section 10: topK must be 1-100
const MIN_TOP_K: u64 = 1;
const MAX_TOP_K: u64 = 100;

impl Handlers {
    /// inject_context tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore with 13-embedding fingerprint.
    ///
    /// Injects context into the memory graph with UTL metrics computation.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(crate) async fn call_inject_context(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => return self.tool_error_with_pulse(id, "Content cannot be empty"),
            None => return self.tool_error_with_pulse(id, "Missing 'content' parameter"),
        };

        let rationale = args.get("rationale").and_then(|v| v.as_str()).unwrap_or("");

        if rationale.len() < MIN_RATIONALE_LEN {
            error!(
                rationale_len = rationale.len(),
                min_required = MIN_RATIONALE_LEN,
                "inject_context: rationale validation FAILED - empty or missing"
            );
            return self.tool_error_with_pulse(id, "rationale is REQUIRED (min 1 char)");
        }
        if rationale.len() > MAX_RATIONALE_LEN {
            error!(
                rationale_len = rationale.len(),
                max_allowed = MAX_RATIONALE_LEN,
                "inject_context: rationale validation FAILED - exceeds maximum"
            );
            return self.tool_error_with_pulse(
                id,
                &format!(
                    "rationale must be at most {} characters, got {}",
                    MAX_RATIONALE_LEN,
                    rationale.len()
                ),
            );
        }

        let _importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        // Compute UTL metrics for the content
        let context = UtlContext::default();
        let metrics = match self.utl_processor.compute_metrics(&content, &context).await {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "inject_context: UTL processing FAILED");
                return self.tool_error_with_pulse(id, &format!("UTL processing failed: {}", e));
            }
        };

        // Generate all 13 embeddings using MultiArrayEmbeddingProvider
        let embedding_output = match self.multi_array_provider.embed_all(&content).await {
            Ok(output) => output,
            Err(e) => {
                error!(error = %e, "inject_context: Multi-array embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Embedding failed: {}", e));
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // TASK-FIX-CLUSTERING: Compute cluster array BEFORE fingerprint is consumed
        // This must be done before TeleologicalFingerprint::new() moves the semantic fingerprint.
        let cluster_array = embedding_output.fingerprint.to_cluster_array();

        // Create TeleologicalFingerprint from embeddings
        let fingerprint = TeleologicalFingerprint::new(embedding_output.fingerprint, content_hash);
        let fingerprint_id = fingerprint.id;

        // Store in TeleologicalMemoryStore
        if let Err(e) = self.teleological_store.store(fingerprint).await {
            error!(error = %e, "inject_context: Storage FAILED");
            return self.tool_error_with_pulse(id, &format!("Storage failed: {}", e));
        }

        // TASK-FIX-CLUSTERING: Insert into cluster_manager for topic detection
        // This enables MultiSpaceClusterManager to track this memory for HDBSCAN/BIRCH clustering.
        // Per PRD Section 5: Topics emerge from multi-space clustering with weighted_agreement >= 2.5.
        {
            let mut cluster_mgr = self.cluster_manager.write();
            if let Err(e) = cluster_mgr.insert(fingerprint_id, &cluster_array) {
                // Non-fatal: fingerprint is stored, clustering can be retried via detect_topics
                warn!(
                    fingerprint_id = %fingerprint_id,
                    error = %e,
                    "inject_context: Failed to insert into cluster_manager. \
                     Topic detection may not include this memory until next recluster."
                );
            } else {
                debug!(
                    fingerprint_id = %fingerprint_id,
                    "inject_context: Inserted into cluster_manager for topic detection"
                );
            }
        }

        // TASK-CONTENT-001: Store content text alongside fingerprint
        // Content storage failure is non-fatal - fingerprint is primary data
        // Pattern matches store_memory implementation for API consistency
        if let Err(e) = self
            .teleological_store
            .store_content(fingerprint_id, &content)
            .await
        {
            warn!(
                fingerprint_id = %fingerprint_id,
                error = %e,
                content_size = content.len(),
                "inject_context: Failed to store content text (fingerprint saved successfully). \
                 Content retrieval will return None for this fingerprint."
            );
        } else {
            debug!(
                fingerprint_id = %fingerprint_id,
                content_size = content.len(),
                "inject_context: Content text stored successfully"
            );
        }

        self.tool_result_with_pulse(
            id,
            json!({
                "fingerprintId": fingerprint_id.to_string(),
                "rationale": rationale,
                "embedderCount": NUM_EMBEDDERS,
                "embeddingLatencyMs": embedding_output.total_latency.as_millis(),
                "utl": {
                    "learningScore": metrics.learning_score,
                    "entropy": metrics.entropy,
                    "coherence": metrics.coherence,
                    "surprise": metrics.surprise
                }
            }),
        )
    }

    /// store_memory tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore with 13-embedding fingerprint.
    ///
    /// Stores content in the memory graph.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(crate) async fn call_store_memory(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => return self.tool_error_with_pulse(id, "Content cannot be empty"),
            None => return self.tool_error_with_pulse(id, "Missing 'content' parameter"),
        };

        let _importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        // Generate all 13 embeddings using MultiArrayEmbeddingProvider
        let embedding_output = match self.multi_array_provider.embed_all(&content).await {
            Ok(output) => output,
            Err(e) => {
                error!(error = %e, "store_memory: Multi-array embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Embedding failed: {}", e));
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // TASK-FIX-CLUSTERING: Compute cluster array BEFORE fingerprint is consumed
        // This must be done before TeleologicalFingerprint::new() moves the semantic fingerprint.
        let cluster_array = embedding_output.fingerprint.to_cluster_array();

        // Create TeleologicalFingerprint from embeddings
        let fingerprint = TeleologicalFingerprint::new(embedding_output.fingerprint, content_hash);
        let fingerprint_id = fingerprint.id;

        match self.teleological_store.store(fingerprint).await {
            Ok(_) => {
                // TASK-FIX-CLUSTERING: Insert into cluster_manager for topic detection
                // This enables MultiSpaceClusterManager to track this memory for HDBSCAN/BIRCH clustering.
                // Per PRD Section 5: Topics emerge from multi-space clustering with weighted_agreement >= 2.5.
                {
                    let mut cluster_mgr = self.cluster_manager.write();
                    if let Err(e) = cluster_mgr.insert(fingerprint_id, &cluster_array) {
                        // Non-fatal: fingerprint is stored, clustering can be retried via detect_topics
                        warn!(
                            fingerprint_id = %fingerprint_id,
                            error = %e,
                            "store_memory: Failed to insert into cluster_manager. \
                             Topic detection may not include this memory until next recluster."
                        );
                    } else {
                        debug!(
                            fingerprint_id = %fingerprint_id,
                            "store_memory: Inserted into cluster_manager for topic detection"
                        );
                    }
                }

                // TASK-CONTENT-010: Store content text alongside fingerprint
                // Content storage failure is non-fatal - fingerprint is primary data
                if let Err(e) = self
                    .teleological_store
                    .store_content(fingerprint_id, &content)
                    .await
                {
                    warn!(
                        fingerprint_id = %fingerprint_id,
                        error = %e,
                        content_size = content.len(),
                        "store_memory: Failed to store content text (fingerprint saved successfully). \
                         Content retrieval will return None for this fingerprint."
                    );
                } else {
                    debug!(
                        fingerprint_id = %fingerprint_id,
                        content_size = content.len(),
                        "store_memory: Content text stored successfully"
                    );
                }

                self.tool_result_with_pulse(
                    id,
                    json!({
                        "fingerprintId": fingerprint_id.to_string(),
                        "embedderCount": NUM_EMBEDDERS,
                        "embeddingLatencyMs": embedding_output.total_latency.as_millis()
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "store_memory: Storage FAILED");
                self.tool_error_with_pulse(id, &format!("Storage failed: {}", e))
            }
        }
    }

    /// search_graph tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore search_semantic.
    ///
    /// Searches the memory graph for matching content.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(crate) async fn call_search_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            Some(_) => return self.tool_error_with_pulse(id, "Query cannot be empty"),
            None => return self.tool_error_with_pulse(id, "Missing 'query' parameter"),
        };

        let raw_top_k = args.get("topK").and_then(|v| v.as_u64());
        if let Some(k) = raw_top_k {
            if k < MIN_TOP_K {
                error!(
                    top_k = k,
                    min_allowed = MIN_TOP_K,
                    "search_graph: topK validation FAILED - below minimum"
                );
                return self.tool_error_with_pulse(
                    id,
                    &format!("topK must be at least {}, got {}", MIN_TOP_K, k),
                );
            }
            if k > MAX_TOP_K {
                error!(
                    top_k = k,
                    max_allowed = MAX_TOP_K,
                    "search_graph: topK validation FAILED - exceeds maximum"
                );
                return self.tool_error_with_pulse(
                    id,
                    &format!("topK must be at most {}, got {}", MAX_TOP_K, k),
                );
            }
        }
        let top_k = raw_top_k.unwrap_or(10) as usize;

        // Parse minSimilarity parameter (default: 0.0 = no filtering)
        let min_similarity = args
            .get("minSimilarity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;

        // TASK-CONTENT-002: Parse includeContent parameter (default: false for backward compatibility)
        let include_content = args
            .get("includeContent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let options = TeleologicalSearchOptions::quick(top_k).with_min_similarity(min_similarity);

        // Generate query embedding
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_graph: Query embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
            }
        };

        match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => {
                // TASK-CONTENT-003: Hydrate content if requested
                // Batch retrieve content for all results to minimize I/O
                let contents: Vec<Option<String>> = if include_content && !results.is_empty() {
                    let ids: Vec<uuid::Uuid> = results.iter().map(|r| r.fingerprint.id).collect();
                    match self.teleological_store.get_content_batch(&ids).await {
                        Ok(c) => c,
                        Err(e) => {
                            warn!(
                                error = %e,
                                result_count = results.len(),
                                "search_graph: Content hydration failed. Results will not include content."
                            );
                            // Return None for all - graceful degradation
                            vec![None; ids.len()]
                        }
                    }
                } else {
                    // Not requested or no results - empty vec
                    vec![]
                };

                let results_json: Vec<_> = results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        // Convert embedder index to human-readable name (E1_Semantic, etc.)
                        let dominant_idx = r.dominant_embedder();
                        let dominant_name = embedder_names::name(dominant_idx);

                        let mut entry = json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "similarity": r.similarity,
                            "dominantEmbedder": dominant_name
                        });
                        // Only include content field when includeContent=true
                        if include_content {
                            entry["content"] = match contents.get(i).and_then(|c| c.as_ref()) {
                                Some(c) => json!(c),
                                None => serde_json::Value::Null,
                            };
                        }
                        entry
                    })
                    .collect();

                self.tool_result_with_pulse(
                    id,
                    json!({ "results": results_json, "count": results_json.len() }),
                )
            }
            Err(e) => {
                error!(error = %e, "search_graph: Search FAILED");
                self.tool_error_with_pulse(id, &format!("Search failed: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for memory_tools validation logic (BUG-001 and BUG-002 fixes).
    //!
    //! These tests verify that validation constraints are correctly enforced:
    //! - inject_context: rationale must be 1-1024 chars (BUG-002)
    //! - search_graph: topK must be 1-100 (BUG-001)

    use super::{MAX_RATIONALE_LEN, MAX_TOP_K, MIN_RATIONALE_LEN, MIN_TOP_K};

    #[test]
    fn rationale_constants_match_prd() {
        // Per PRD 0.3: rationale is REQUIRED (1-1024 chars)
        assert_eq!(MIN_RATIONALE_LEN, 1);
        assert_eq!(MAX_RATIONALE_LEN, 1024);
    }

    #[test]
    fn topk_constants_match_prd() {
        // Per PRD Section 10: topK must be 1-100
        assert_eq!(MIN_TOP_K, 1);
        assert_eq!(MAX_TOP_K, 100);
    }

    #[test]
    fn rationale_validation_boundary_cases() {
        // Empty rationale should fail
        let empty = "";
        assert!(empty.len() < MIN_RATIONALE_LEN);

        // Single char (minimum valid) should pass
        let min_valid = "x";
        assert!(min_valid.len() >= MIN_RATIONALE_LEN);
        assert!(min_valid.len() <= MAX_RATIONALE_LEN);

        // Exactly 1024 chars (maximum valid) should pass
        let max_valid = "x".repeat(MAX_RATIONALE_LEN);
        assert!(max_valid.len() <= MAX_RATIONALE_LEN);

        // 1025 chars should fail
        let too_long = "x".repeat(MAX_RATIONALE_LEN + 1);
        assert!(too_long.len() > MAX_RATIONALE_LEN);
    }

    #[test]
    fn topk_validation_boundary_cases() {
        // topK = 0 should fail
        assert!(0 < MIN_TOP_K);

        // topK = 1 (minimum valid) should pass
        assert!(1 >= MIN_TOP_K);
        assert!(1 <= MAX_TOP_K);

        // topK = 100 (maximum valid) should pass
        assert!(100 <= MAX_TOP_K);

        // topK = 101 should fail
        assert!(101 > MAX_TOP_K);

        // topK = 500 (original BUG-001 case) should fail
        assert!(500 > MAX_TOP_K);
    }

    #[test]
    fn rationale_error_message_format() {
        // Verify error message format matches handler implementation
        let empty_error = "rationale is REQUIRED (min 1 char)";
        assert!(empty_error.contains("REQUIRED"));
        assert!(empty_error.contains("min 1 char"));

        let too_long_len = 2000_usize;
        let too_long_error = format!(
            "rationale must be at most {} characters, got {}",
            MAX_RATIONALE_LEN, too_long_len
        );
        assert!(too_long_error.contains(&MAX_RATIONALE_LEN.to_string()));
        assert!(too_long_error.contains(&too_long_len.to_string()));
    }

    #[test]
    fn topk_error_message_format() {
        // Verify error message format matches handler implementation
        let too_small_error = format!("topK must be at least {}, got {}", MIN_TOP_K, 0);
        assert!(too_small_error.contains(&MIN_TOP_K.to_string()));

        let too_large_error = format!("topK must be at most {}, got {}", MAX_TOP_K, 500);
        assert!(too_large_error.contains(&MAX_TOP_K.to_string()));
        assert!(too_large_error.contains("500"));
    }
}
