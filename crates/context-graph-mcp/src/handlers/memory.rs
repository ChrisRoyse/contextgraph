//! Teleological memory operation handlers.
//!
//! TASK-S001: Rewritten to use TeleologicalFingerprint and TeleologicalMemoryStore.
//! NO BACKWARDS COMPATIBILITY with legacy MemoryStore/MemoryNode.
//!
//! # Error Handling
//!
//! FAIL FAST: All errors return immediately with detailed error codes.
//! NO fallbacks, NO default values, NO mock data.
//!
//! # Purpose Vector Computation (AP-007)
//!
//! Purpose vectors MUST be computed at storage time from the semantic fingerprint
//! and current goal hierarchy. This ensures every stored memory has alignment
//! metadata for retrieval and indexing. From constitution.yaml:
//! ```text
//! purpose_vector: PV = [A(E1,V), A(E2,V), ..., A(E13,V)]
//! where A(Ei, V) = cos(θ) between embedder i and North Star goal V
//! ```

use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, error, warn};

use context_graph_core::purpose::PurposeVectorComputer;
use context_graph_core::traits::TeleologicalSearchOptions;
use context_graph_core::types::{CognitivePulse, SuggestedAction};
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, TeleologicalFingerprint,
    NUM_EMBEDDERS,
};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

impl Handlers {
    /// Handle memory/store request.
    ///
    /// Stores content as a TeleologicalFingerprint with all 13 embeddings.
    ///
    /// # Request Parameters
    /// - `content` (required): Text content to store
    /// - `importance` (optional): Importance score 0.0-1.0, default 0.5
    ///
    /// # Response
    /// - `fingerprintId`: UUID of stored TeleologicalFingerprint
    /// - `embeddingLatencyMs`: Time to generate all 13 embeddings
    /// - `storageLatencyMs`: Time to store fingerprint
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing content parameter
    /// - EMBEDDING_ERROR (-32005): Multi-array embedding generation failed
    /// - STORAGE_ERROR (-32004): TeleologicalMemoryStore operation failed
    pub(super) async fn handle_memory_store(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/store: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - request body required",
                );
            }
        };

        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => {
                error!("memory/store: Empty content string");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Content cannot be empty string",
                );
            }
            None => {
                error!("memory/store: Missing 'content' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'content' parameter",
                );
            }
        };

        // Generate all 13 embeddings using MultiArrayEmbeddingProvider
        let embed_start = std::time::Instant::now();
        let embedding_output = match self.multi_array_provider.embed_all(&content).await {
            Ok(output) => {
                debug!(
                    "Generated 13 embeddings in {:?} (target <30ms)",
                    output.total_latency
                );
                output
            }
            Err(e) => {
                error!(error = %e, "memory/store: Multi-array embedding FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::EMBEDDING_ERROR,
                    format!("Multi-array embedding failed: {}", e),
                );
            }
        };
        let embed_latency_ms = embed_start.elapsed().as_millis();

        // Compute content hash (SHA-256)
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // AP-007: Compute purpose vector from semantic fingerprint and goal hierarchy
        // Purpose vectors MUST be computed at storage time, not deferred
        // From constitution.yaml: PV = [A(E1,V), A(E2,V), ..., A(E13,V)]
        // where A(Ei, V) = cos(θ) between embedder i and North Star goal V
        let purpose_vector = {
            let hierarchy = self.goal_hierarchy.read().clone();

            // If no North Star goal is defined, log error and fail (AP-007: FAIL FAST)
            if hierarchy.north_star().is_none() {
                error!(
                    "memory/store: Goal hierarchy missing North Star goal. \
                     Cannot compute purpose vector. \
                     CONFIGURATION ERROR: Use purpose/north_star_update to configure goal hierarchy."
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    "Goal hierarchy not configured. Cannot compute purpose vector. \
                     Use purpose/north_star_update endpoint to set North Star goal.",
                );
            }

            // Compute purpose vector using PurposeVectorComputer
            // This computes alignment for each of 13 embedding spaces
            let config = context_graph_core::purpose::PurposeComputeConfig::with_hierarchy(hierarchy);

            match context_graph_core::purpose::DefaultPurposeComputer::new()
                .compute_purpose(&embedding_output.fingerprint, &config)
                .await
            {
                Ok(pv) => {
                    debug!(
                        aggregate_alignment = pv.aggregate_alignment(),
                        dominant_embedder = pv.dominant_embedder,
                        coherence = pv.coherence,
                        "Purpose vector computed for semantic fingerprint"
                    );
                    pv
                }
                Err(e) => {
                    error!(
                        error = %e,
                        "memory/store: Failed to compute purpose vector. \
                         Cannot store memory without alignment metadata."
                    );
                    return JsonRpcResponse::error(
                        id,
                        error_codes::STORAGE_ERROR,
                        format!("Purpose vector computation failed: {}", e),
                    );
                }
            }
        };

        // Create TeleologicalFingerprint with computed purpose vector
        let fingerprint = TeleologicalFingerprint::new(
            embedding_output.fingerprint,
            purpose_vector,
            JohariFingerprint::zeroed(), // Will be classified by Johari system
            content_hash,
        );
        let fingerprint_id = fingerprint.id;

        // Store in TeleologicalMemoryStore
        let store_start = std::time::Instant::now();
        match self.teleological_store.store(fingerprint).await {
            Ok(stored_id) => {
                debug_assert_eq!(stored_id, fingerprint_id, "Store should return same ID");
                let store_latency_ms = store_start.elapsed().as_millis();

                let pulse = CognitivePulse::new(0.6, 0.75, 0.0, 1.0, SuggestedAction::Continue, None);
                JsonRpcResponse::success(
                    id,
                    json!({
                        "fingerprintId": fingerprint_id.to_string(),
                        "embeddingLatencyMs": embed_latency_ms,
                        "storageLatencyMs": store_latency_ms,
                        "embedderCount": NUM_EMBEDDERS
                    }),
                )
                .with_pulse(pulse)
            }
            Err(e) => {
                error!(error = %e, fingerprint_id = %fingerprint_id, "memory/store: Storage FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("TeleologicalMemoryStore.store() failed: {}", e),
                )
            }
        }
    }

    /// Handle memory/retrieve request.
    ///
    /// Retrieves a TeleologicalFingerprint by UUID.
    ///
    /// # Request Parameters
    /// - `fingerprintId` (required): UUID of fingerprint to retrieve
    ///
    /// # Response
    /// - `fingerprint`: Full TeleologicalFingerprint data including:
    ///   - id, theta_to_north_star, access_count, created_at, last_updated
    ///   - purpose_vector (13D alignment)
    ///   - johari summary (dominant quadrant)
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or invalid fingerprintId
    /// - FINGERPRINT_NOT_FOUND (-32010): No fingerprint with given ID
    /// - STORAGE_ERROR (-32004): Store operation failed
    pub(super) async fn handle_memory_retrieve(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/retrieve: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - fingerprintId required",
                );
            }
        };

        let fingerprint_id_str = match params.get("fingerprintId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                error!("memory/retrieve: Missing 'fingerprintId' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'fingerprintId' parameter",
                );
            }
        };

        let fingerprint_id = match uuid::Uuid::parse_str(fingerprint_id_str) {
            Ok(u) => u,
            Err(e) => {
                error!(input = fingerprint_id_str, error = %e, "memory/retrieve: Invalid UUID format");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format '{}': {}", fingerprint_id_str, e),
                );
            }
        };

        match self.teleological_store.retrieve(fingerprint_id).await {
            Ok(Some(fp)) => {
                // Compute dominant Johari quadrant from E1 (semantic) embedder
                let dominant_quadrant = format!("{:?}", fp.johari.dominant_quadrant(0));

                JsonRpcResponse::success(
                    id,
                    json!({
                        "fingerprint": {
                            "id": fp.id.to_string(),
                            "thetaToNorthStar": fp.theta_to_north_star,
                            "accessCount": fp.access_count,
                            "createdAt": fp.created_at.to_rfc3339(),
                            "lastUpdated": fp.last_updated.to_rfc3339(),
                            "purposeVector": fp.purpose_vector.alignments.to_vec(),
                            "johariDominant": dominant_quadrant,
                            "evolutionSnapshots": fp.purpose_evolution.len(),
                            "contentHashHex": hex::encode(fp.content_hash)
                        }
                    }),
                )
            }
            Ok(None) => {
                debug!(fingerprint_id = %fingerprint_id, "memory/retrieve: Fingerprint not found");
                JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("No fingerprint found with ID '{}'", fingerprint_id),
                )
            }
            Err(e) => {
                error!(error = %e, fingerprint_id = %fingerprint_id, "memory/retrieve: Storage FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("TeleologicalMemoryStore.retrieve() failed: {}", e),
                )
            }
        }
    }

    /// Handle memory/search request.
    ///
    /// Searches for similar TeleologicalFingerprints using the 5-stage pipeline.
    ///
    /// # Request Parameters
    /// - `query` (required): Text query to search for
    /// - `topK` (optional): Maximum results, default 10
    /// - `minSimilarity` (optional): Minimum similarity threshold 0.0-1.0
    /// - `minAlignment` (optional): Minimum purpose alignment to North Star
    ///
    /// # Response
    /// - `results`: Array of search results with:
    ///   - fingerprintId, similarity, purposeAlignment, dominantEmbedder
    /// - `queryLatencyMs`: Total search time
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing query parameter
    /// - EMBEDDING_ERROR (-32005): Query embedding failed
    /// - STORAGE_ERROR (-32004): Search operation failed
    pub(super) async fn handle_memory_search(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/search: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - query required",
                );
            }
        };

        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            Some(_) => {
                error!("memory/search: Empty query string");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Query cannot be empty string",
                );
            }
            None => {
                error!("memory/search: Missing 'query' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'query' parameter",
                );
            }
        };

        // top_k has a sensible default (pagination parameter)
        const DEFAULT_TOP_K: usize = 10;
        let top_k = params.get("topK").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(DEFAULT_TOP_K);

        // FAIL-FAST: minSimilarity MUST be explicitly provided.
        // Per constitution AP-007: No silent fallbacks that mask user intent.
        // 0.0 may seem like "no filter" but user must explicitly confirm this choice.
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

        // Generate query embeddings
        let search_start = std::time::Instant::now();
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "memory/search: Query embedding FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::EMBEDDING_ERROR,
                    format!("Query embedding failed: {}", e),
                );
            }
        };

        // Build search options
        let mut options = TeleologicalSearchOptions::quick(top_k)
            .with_min_similarity(min_similarity);
        if let Some(align) = min_alignment {
            options = options.with_min_alignment(align);
        }

        // Execute semantic search
        match self.teleological_store.search_semantic(&query_embedding, options).await {
            Ok(results) => {
                let query_latency_ms = search_start.elapsed().as_millis();

                let results_json: Vec<_> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "similarity": r.similarity,
                            "purposeAlignment": r.purpose_alignment,
                            "dominantEmbedder": r.dominant_embedder(),
                            "embedderScores": r.embedder_scores.to_vec(),
                            "thetaToNorthStar": r.fingerprint.theta_to_north_star
                        })
                    })
                    .collect();

                let pulse = CognitivePulse::new(0.4, 0.8, 0.0, 1.0, SuggestedAction::Continue, None);
                JsonRpcResponse::success(
                    id,
                    json!({
                        "results": results_json,
                        "count": results_json.len(),
                        "queryLatencyMs": query_latency_ms
                    }),
                )
                .with_pulse(pulse)
            }
            Err(e) => {
                error!(error = %e, "memory/search: Search FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("TeleologicalMemoryStore.search_semantic() failed: {}", e),
                )
            }
        }
    }

    /// Handle memory/delete request.
    ///
    /// Deletes a TeleologicalFingerprint (soft or hard delete).
    ///
    /// # Request Parameters
    /// - `fingerprintId` (required): UUID of fingerprint to delete
    /// - `soft` (optional): If true, mark as deleted but retain data. Default true.
    ///
    /// # Response
    /// - `deleted`: Boolean indicating if delete succeeded
    /// - `deleteType`: "soft" or "hard"
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or invalid fingerprintId
    /// - STORAGE_ERROR (-32004): Delete operation failed
    pub(super) async fn handle_memory_delete(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/delete: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - fingerprintId required",
                );
            }
        };

        let fingerprint_id_str = match params.get("fingerprintId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                error!("memory/delete: Missing 'fingerprintId' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'fingerprintId' parameter",
                );
            }
        };

        let fingerprint_id = match uuid::Uuid::parse_str(fingerprint_id_str) {
            Ok(u) => u,
            Err(e) => {
                error!(input = fingerprint_id_str, error = %e, "memory/delete: Invalid UUID format");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format '{}': {}", fingerprint_id_str, e),
                );
            }
        };

        let soft = params.get("soft").and_then(|v| v.as_bool()).unwrap_or(true);
        let delete_type = if soft { "soft" } else { "hard" };

        match self.teleological_store.delete(fingerprint_id, soft).await {
            Ok(deleted) => {
                debug!(
                    fingerprint_id = %fingerprint_id,
                    delete_type = delete_type,
                    deleted = deleted,
                    "memory/delete: Completed"
                );
                JsonRpcResponse::success(
                    id,
                    json!({
                        "deleted": deleted,
                        "deleteType": delete_type,
                        "fingerprintId": fingerprint_id.to_string()
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, fingerprint_id = %fingerprint_id, "memory/delete: FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("TeleologicalMemoryStore.delete() failed: {}", e),
                )
            }
        }
    }
}
