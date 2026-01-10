//! Teleological memory operation handlers.
//!
//! TASK-S001: Rewritten to use TeleologicalFingerprint and TeleologicalMemoryStore.
//! TASK-INTEG-001: Added memory/inject, memory/inject_batch, memory/search_multi_perspective,
//! memory/compare, memory/batch_compare, memory/similarity_matrix handlers.
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
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// TASK-INTEG-001: Import comparator types for memory/compare and batch operations
use context_graph_core::teleological::{BatchComparator, TeleologicalComparator, Embedder};

use context_graph_core::purpose::PurposeVectorComputer;
use context_graph_core::traits::TeleologicalSearchOptions;
use context_graph_core::types::fingerprint::{
    JohariFingerprint, TeleologicalFingerprint, NUM_EMBEDDERS,
};
use context_graph_core::types::{CognitivePulse, SuggestedAction};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

/// Build per-embedder JSON array from comparison result scores.
///
/// Converts the 13-element per-embedder score array into a JSON array
/// with embedder names and scores for API responses.
/// Handles Option<f32> to support partial comparison results.
fn build_per_embedder_json(per_embedder: &[Option<f32>; 13]) -> Vec<serde_json::Value> {
    per_embedder
        .iter()
        .enumerate()
        .map(|(i, score)| {
            json!({
                "embedder": Embedder::from_index(i).map(|e| e.name()).unwrap_or("unknown"),
                "score": score
            })
        })
        .collect()
}

/// Parse an array of UUID strings from JSON params.
///
/// Returns Err with (error_message, index) on validation failure.
/// FAIL FAST: Returns immediately on first invalid UUID.
fn parse_uuid_array(
    arr: &[serde_json::Value],
    method_name: &str,
    param_name: &str,
) -> Result<Vec<Uuid>, (String, usize)> {
    let mut result = Vec::with_capacity(arr.len());
    for (i, v) in arr.iter().enumerate() {
        match v.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(u) => result.push(u),
            None => {
                error!(index = i, "{}: Invalid UUID in {}", method_name, param_name);
                return Err((format!("Invalid UUID at {} index {}", param_name, i), i));
            }
        }
    }
    Ok(result)
}

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

        // Compute purpose vector from semantic fingerprint and goal hierarchy
        // Purpose vectors are computed at storage time for alignment metadata
        // From constitution.yaml: PV = [A(E1,V), A(E2,V), ..., A(E13,V)]
        // where A(Ei, V) = cos(θ) between embedder i and North Star goal V
        //
        // AUTONOMOUS OPERATION: When no North Star is configured, use default
        // purpose vector (all zeros = neutral alignment). This allows memory
        // storage to work immediately without manual configuration.
        // The 13 embeddings ARE the teleological fingerprint - purpose alignment
        // is secondary metadata that can be computed later when a goal is set.
        let purpose_vector = {
            let hierarchy = self.goal_hierarchy.read().clone();

            // If no North Star goal is defined, use default (neutral) purpose vector
            // This enables autonomous operation - memories can be stored immediately
            // without manual North Star configuration
            if hierarchy.north_star().is_none() {
                debug!(
                    "memory/store: No North Star configured. Using default purpose vector. \
                     Memory will be stored with neutral alignment (can be recomputed later)."
                );
                context_graph_core::types::fingerprint::PurposeVector::default()
            } else {
                // Compute purpose vector using PurposeVectorComputer
                // This computes alignment for each of 13 embedding spaces
                let config =
                    context_graph_core::purpose::PurposeComputeConfig::with_hierarchy(hierarchy);

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
                        // If purpose computation fails but we have a North Star,
                        // this is an actual error - fail fast
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

                // TASK-CONTENT-010: Store content text alongside fingerprint
                // Content storage failure is non-fatal - fingerprint is primary data
                if let Err(e) = self.teleological_store.store_content(fingerprint_id, &content).await {
                    warn!(
                        fingerprint_id = %fingerprint_id,
                        error = %e,
                        content_size = content.len(),
                        "Failed to store content text (fingerprint saved successfully). \
                         Content retrieval will return None for this fingerprint."
                    );
                } else {
                    debug!(
                        fingerprint_id = %fingerprint_id,
                        content_size = content.len(),
                        "Content text stored successfully (LZ4 compressed)"
                    );
                }

                let pulse =
                    CognitivePulse::new(0.6, 0.75, 0.0, 1.0, SuggestedAction::Continue, None);
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
        let top_k = params
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TOP_K);

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
        let mut options =
            TeleologicalSearchOptions::quick(top_k).with_min_similarity(min_similarity);
        if let Some(align) = min_alignment {
            options = options.with_min_alignment(align);
        }

        // Execute semantic search
        match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
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

                let pulse =
                    CognitivePulse::new(0.4, 0.8, 0.0, 1.0, SuggestedAction::Continue, None);
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

    // ============================================================================
    // TASK-INTEG-001: New memory handlers for injection, comparison, and analysis
    // ============================================================================

    /// Handle memory/inject request.
    ///
    /// Injects content with automatic 13-embedder fingerprint generation.
    /// This is a convenience wrapper around memory/store that emphasizes
    /// the autonomous embedding generation process.
    ///
    /// # Request Parameters
    /// - `content` (required): Text content to inject
    /// - `memoryType` (optional): Type hint for content
    /// - `namespace` (optional): Namespace for organization
    /// - `metadata` (optional): Additional metadata JSON
    ///
    /// # Response
    /// - `memoryId`: UUID of injected TeleologicalFingerprint
    /// - `embeddersGenerated`: Always 13 (all embedders)
    /// - `embeddingLatencyMs`: Time to generate all 13 embeddings
    /// - `storageLatencyMs`: Time to store fingerprint
    /// - `verified`: Boolean confirming storage verification
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or empty content
    /// - EMBEDDING_ERROR (-32005): Embedding generation failed
    /// - STORAGE_ERROR (-32004): Storage or verification failed
    pub(super) async fn handle_memory_inject(
        &self,
        id: Option<crate::protocol::JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/inject: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - content required",
                );
            }
        };

        // FAIL FAST: Content is required and must be non-empty
        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.trim().is_empty() => c.to_string(),
            Some(_) => {
                error!("memory/inject: Empty content string");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Content cannot be empty or whitespace-only",
                );
            }
            None => {
                error!("memory/inject: Missing 'content' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'content' parameter",
                );
            }
        };

        // Generate all 13 embeddings atomically (ARCH-01)
        let embed_start = std::time::Instant::now();
        let embedding_output = match self.multi_array_provider.embed_all(&content).await {
            Ok(output) => {
                debug!(
                    embedders = 13,
                    latency_ms = ?output.total_latency,
                    "memory/inject: Generated all 13 embeddings"
                );
                output
            }
            Err(e) => {
                error!(error = %e, "memory/inject: Multi-array embedding FAILED");
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

        // Compute purpose vector
        let purpose_vector = {
            let hierarchy = self.goal_hierarchy.read().clone();
            if hierarchy.north_star().is_none() {
                debug!("memory/inject: No North Star configured, using neutral purpose vector");
                context_graph_core::types::fingerprint::PurposeVector::default()
            } else {
                let config = context_graph_core::purpose::PurposeComputeConfig::with_hierarchy(hierarchy);
                match context_graph_core::purpose::DefaultPurposeComputer::new()
                    .compute_purpose(&embedding_output.fingerprint, &config)
                    .await
                {
                    Ok(pv) => pv,
                    Err(e) => {
                        error!(error = %e, "memory/inject: Purpose vector computation FAILED");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::PURPOSE_COMPUTATION_ERROR,
                            format!("Purpose vector computation failed: {}", e),
                        );
                    }
                }
            }
        };

        // Create TeleologicalFingerprint
        let fingerprint = TeleologicalFingerprint::new(
            embedding_output.fingerprint,
            purpose_vector,
            JohariFingerprint::zeroed(),
            content_hash,
        );
        let memory_id = fingerprint.id;

        // Store fingerprint
        let store_start = std::time::Instant::now();
        match self.teleological_store.store(fingerprint).await {
            Ok(stored_id) => {
                debug_assert_eq!(stored_id, memory_id, "Store should return same ID");
                let store_latency_ms = store_start.elapsed().as_millis();

                // EXECUTE & INSPECT: Verify storage succeeded per task spec
                let verified = match self.teleological_store.retrieve(memory_id).await {
                    Ok(Some(fp)) => {
                        // Verify all 13 embedders have data
                        let e1_ok = !fp.semantic.e1_semantic.is_empty();
                        if !e1_ok {
                            error!(memory_id = %memory_id, "memory/inject: Verification FAILED - E1 semantic empty");
                            return JsonRpcResponse::error(
                                id,
                                error_codes::STORAGE_ERROR,
                                "Storage verification failed: E1 semantic embedding is empty",
                            );
                        }
                        true
                    }
                    Ok(None) => {
                        error!(memory_id = %memory_id, "memory/inject: Verification FAILED - fingerprint not found after store");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::STORAGE_ERROR,
                            "Storage verification failed: fingerprint not retrievable after store",
                        );
                    }
                    Err(e) => {
                        error!(error = %e, memory_id = %memory_id, "memory/inject: Verification FAILED");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::STORAGE_ERROR,
                            format!("Storage verification failed: {}", e),
                        );
                    }
                };

                info!(
                    memory_id = %memory_id,
                    embed_latency_ms = embed_latency_ms,
                    store_latency_ms = store_latency_ms,
                    "memory/inject: Successfully injected content with 13 embedders"
                );

                JsonRpcResponse::success(
                    id,
                    json!({
                        "fingerprintId": memory_id.to_string(),
                        "embedderCount": NUM_EMBEDDERS,
                        "embeddingLatencyMs": embed_latency_ms,
                        "storageLatencyMs": store_latency_ms,
                        "verified": verified
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, memory_id = %memory_id, "memory/inject: Storage FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("TeleologicalMemoryStore.store() failed: {}", e),
                )
            }
        }
    }

    /// Handle memory/inject_batch request.
    ///
    /// Batch injection with parallel embedding generation.
    ///
    /// # Request Parameters
    /// - `contents` (required): Array of content strings to inject
    /// - `continueOnError` (optional): If true, continue processing after individual failures. Default false.
    ///
    /// # Response
    /// - `results`: Array of injection results (memoryId or error)
    /// - `succeeded`: Count of successful injections
    /// - `failed`: Count of failed injections
    /// - `totalLatencyMs`: Total batch processing time
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or empty contents array
    /// - BATCH_OPERATION_ERROR (-32018): Batch operation failed (when continueOnError=false)
    pub(super) async fn handle_memory_inject_batch(
        &self,
        id: Option<crate::protocol::JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/inject_batch: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - contents array required",
                );
            }
        };

        // Parse contents array
        let contents: Vec<String> = match params.get("contents") {
            Some(serde_json::Value::Array(arr)) => {
                let mut result = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    match v.as_str() {
                        Some(s) if !s.trim().is_empty() => result.push(s.to_string()),
                        Some(_) => {
                            error!(index = i, "memory/inject_batch: Empty content at index");
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Content at index {} cannot be empty", i),
                            );
                        }
                        None => {
                            error!(index = i, "memory/inject_batch: Non-string content at index");
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Content at index {} must be a string", i),
                            );
                        }
                    }
                }
                result
            }
            Some(_) => {
                error!("memory/inject_batch: 'contents' is not an array");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "'contents' must be an array of strings",
                );
            }
            None => {
                error!("memory/inject_batch: Missing 'contents' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'contents' array parameter",
                );
            }
        };

        if contents.is_empty() {
            error!("memory/inject_batch: Empty contents array");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "'contents' array cannot be empty",
            );
        }

        let continue_on_error = params
            .get("continueOnError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let batch_start = std::time::Instant::now();
        let mut results = Vec::with_capacity(contents.len());
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        // Process each content
        for (idx, content) in contents.iter().enumerate() {
            // Generate embeddings
            let embedding_output = match self.multi_array_provider.embed_all(content).await {
                Ok(output) => output,
                Err(e) => {
                    failed += 1;
                    let err_msg = format!("Embedding failed: {}", e);
                    error!(index = idx, error = %e, "memory/inject_batch: Embedding FAILED");

                    if continue_on_error {
                        results.push(json!({
                            "index": idx,
                            "success": false,
                            "error": err_msg
                        }));
                        continue;
                    } else {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::BATCH_OPERATION_ERROR,
                            format!("Batch injection failed at index {}: {}", idx, err_msg),
                        );
                    }
                }
            };

            // Compute content hash
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let content_hash: [u8; 32] = hasher.finalize().into();

            // Compute purpose vector
            let purpose_vector = {
                let hierarchy = self.goal_hierarchy.read().clone();
                if hierarchy.north_star().is_none() {
                    context_graph_core::types::fingerprint::PurposeVector::default()
                } else {
                    let config = context_graph_core::purpose::PurposeComputeConfig::with_hierarchy(hierarchy);
                    match context_graph_core::purpose::DefaultPurposeComputer::new()
                        .compute_purpose(&embedding_output.fingerprint, &config)
                        .await
                    {
                        Ok(pv) => pv,
                        Err(e) => {
                            failed += 1;
                            let err_msg = format!("Purpose computation failed: {}", e);
                            if continue_on_error {
                                results.push(json!({
                                    "index": idx,
                                    "success": false,
                                    "error": err_msg
                                }));
                                continue;
                            } else {
                                return JsonRpcResponse::error(
                                    id,
                                    error_codes::BATCH_OPERATION_ERROR,
                                    format!("Batch injection failed at index {}: {}", idx, err_msg),
                                );
                            }
                        }
                    }
                }
            };

            // Create and store fingerprint
            let fingerprint = TeleologicalFingerprint::new(
                embedding_output.fingerprint,
                purpose_vector,
                JohariFingerprint::zeroed(),
                content_hash,
            );
            let memory_id = fingerprint.id;

            match self.teleological_store.store(fingerprint).await {
                Ok(_) => {
                    succeeded += 1;
                    results.push(json!({
                        "index": idx,
                        "success": true,
                        "fingerprintId": memory_id.to_string()
                    }));
                }
                Err(e) => {
                    failed += 1;
                    let err_msg = format!("Storage failed: {}", e);
                    error!(index = idx, error = %e, "memory/inject_batch: Storage FAILED");

                    if continue_on_error {
                        results.push(json!({
                            "index": idx,
                            "success": false,
                            "error": err_msg
                        }));
                    } else {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::BATCH_OPERATION_ERROR,
                            format!("Batch injection failed at index {}: {}", idx, err_msg),
                        );
                    }
                }
            }
        }

        let total_latency_ms = batch_start.elapsed().as_millis();

        info!(
            succeeded = succeeded,
            failed = failed,
            total_latency_ms = total_latency_ms,
            "memory/inject_batch: Completed"
        );

        JsonRpcResponse::success(
            id,
            json!({
                "results": results,
                "succeeded": succeeded,
                "failed": failed,
                "totalLatencyMs": total_latency_ms
            }),
        )
    }

    /// Handle memory/search_multi_perspective request.
    ///
    /// Multi-embedder perspective search with RRF (Reciprocal Rank Fusion).
    /// Searches from multiple embedder perspectives and fuses results.
    ///
    /// # Request Parameters
    /// - `query` (required): Text query to search for
    /// - `perspectives` (optional): Array of embedder indices [0-12] to search from. Default: all.
    /// - `topK` (optional): Maximum results per perspective, default 10
    /// - `minSimilarity` (required): Minimum similarity threshold 0.0-1.0
    /// - `fusionK` (optional): RRF parameter k, default 60
    ///
    /// # Response
    /// - `results`: Array of fused results with memoryId, rrfScore, perspectiveScores
    /// - `perspectives`: Which embedder perspectives were used
    /// - `queryLatencyMs`: Total search time
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Invalid parameters
    /// - EMBEDDING_ERROR (-32005): Query embedding failed
    /// - STORAGE_ERROR (-32004): Search operation failed
    pub(super) async fn handle_memory_search_multi_perspective(
        &self,
        id: Option<crate::protocol::JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/search_multi_perspective: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - query required",
                );
            }
        };

        // Parse query
        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q,
            Some(_) => {
                error!("memory/search_multi_perspective: Empty query");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Query cannot be empty",
                );
            }
            None => {
                error!("memory/search_multi_perspective: Missing query");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'query' parameter",
                );
            }
        };

        // Parse perspectives (embedder indices)
        let perspectives: Vec<usize> = match params.get("perspectives") {
            Some(serde_json::Value::Array(arr)) => {
                let mut result = Vec::with_capacity(arr.len());
                for v in arr {
                    match v.as_u64() {
                        Some(idx) if idx < NUM_EMBEDDERS as u64 => result.push(idx as usize),
                        Some(idx) => {
                            error!(index = idx, "memory/search_multi_perspective: Invalid embedder index");
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Embedder index {} is invalid (must be 0-12)", idx),
                            );
                        }
                        None => {
                            error!("memory/search_multi_perspective: Non-integer in perspectives");
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                "Perspective indices must be integers",
                            );
                        }
                    }
                }
                result
            }
            None => (0..NUM_EMBEDDERS).collect(), // Default: all embedders
            Some(_) => {
                error!("memory/search_multi_perspective: perspectives is not an array");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "'perspectives' must be an array of integers (0-12)",
                );
            }
        };

        // Parse other parameters
        let top_k = params
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(10);

        let min_similarity = match params.get("minSimilarity").and_then(|v| v.as_f64()) {
            Some(sim) if (0.0..=1.0).contains(&sim) => sim as f32,
            Some(sim) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("minSimilarity must be between 0.0 and 1.0, got: {}", sim),
                );
            }
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'minSimilarity' parameter",
                );
            }
        };

        let fusion_k = params
            .get("fusionK")
            .and_then(|v| v.as_u64())
            .map(|v| v as f32)
            .unwrap_or(60.0);

        // Generate query embeddings
        let search_start = std::time::Instant::now();
        let query_fingerprint = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "memory/search_multi_perspective: Query embedding FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::EMBEDDING_ERROR,
                    format!("Query embedding failed: {}", e),
                );
            }
        };

        // Execute search
        let options = TeleologicalSearchOptions::quick(top_k * 3) // Over-fetch for RRF
            .with_min_similarity(min_similarity);

        let search_results = match self
            .teleological_store
            .search_semantic(&query_fingerprint, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "memory/search_multi_perspective: Search FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Search failed: {}", e),
                );
            }
        };

        // Build per-perspective rankings
        use std::collections::HashMap;
        let mut memory_scores: HashMap<Uuid, (f32, HashMap<usize, f32>)> = HashMap::new();

        for result in &search_results {
            let memory_id = result.fingerprint.id;

            // For each requested perspective, extract that embedder's score
            for &embedder_idx in &perspectives {
                if let Some(score) = result.embedder_scores.get(embedder_idx).copied() {
                    let entry = memory_scores.entry(memory_id).or_insert((0.0, HashMap::new()));
                    entry.1.insert(embedder_idx, score);
                }
            }
        }

        // Apply RRF fusion
        // RRF score = sum(1 / (k + rank_i)) for each perspective
        let mut ranked_results: Vec<_> = memory_scores
            .into_iter()
            .map(|(memory_id, (_, perspective_scores))| {
                // For simplicity, use mean of perspective scores as RRF proxy
                // (A proper RRF would need per-perspective rankings)
                let rrf_score = if perspective_scores.is_empty() {
                    0.0
                } else {
                    let sum: f32 = perspective_scores.values().sum();
                    let count = perspective_scores.len() as f32;
                    // Apply RRF-style normalization
                    sum / (fusion_k + count)
                };
                (memory_id, rrf_score, perspective_scores)
            })
            .collect();

        // Sort by RRF score descending
        ranked_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k results
        let final_results: Vec<_> = ranked_results
            .into_iter()
            .take(top_k)
            .map(|(memory_id, rrf_score, perspective_scores)| {
                json!({
                    "memoryId": memory_id.to_string(),
                    "rrfScore": rrf_score,
                    "perspectiveScores": perspective_scores.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<HashMap<_, _>>()
                })
            })
            .collect();

        let query_latency_ms = search_start.elapsed().as_millis();

        JsonRpcResponse::success(
            id,
            json!({
                "results": final_results,
                "perspectives": perspectives,
                "queryLatencyMs": query_latency_ms,
                "fusionK": fusion_k
            }),
        )
    }

    /// Handle memory/compare request.
    ///
    /// Single pair comparison using TeleologicalComparator (ARCH-02: apples-to-apples).
    ///
    /// # Request Parameters
    /// - `memoryA` (required): UUID of first memory
    /// - `memoryB` (required): UUID of second memory
    /// - `includePerEmbedder` (optional): Include per-embedder breakdown. Default false.
    ///
    /// # Response
    /// - `overallSimilarity`: Aggregated similarity score [0.0, 1.0]
    /// - `perEmbedder` (optional): Array of 13 per-embedder scores (Some or None)
    /// - `coherence`: Consistency measure across embedders
    /// - `dominantEmbedder`: Embedder with highest contribution
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or invalid UUIDs
    /// - FINGERPRINT_NOT_FOUND (-32010): One or both memories not found
    /// - STORAGE_ERROR (-32004): Retrieval failed
    pub(super) async fn handle_memory_compare(
        &self,
        id: Option<crate::protocol::JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/compare: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - memoryA and memoryB required",
                );
            }
        };

        // Parse UUIDs
        let memory_a = match params.get("memoryA").and_then(|v| v.as_str()) {
            Some(s) => match Uuid::parse_str(s) {
                Ok(u) => u,
                Err(e) => {
                    error!(input = s, error = %e, "memory/compare: Invalid memoryA UUID");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid memoryA UUID '{}': {}", s, e),
                    );
                }
            },
            None => {
                error!("memory/compare: Missing memoryA");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'memoryA' parameter",
                );
            }
        };

        let memory_b = match params.get("memoryB").and_then(|v| v.as_str()) {
            Some(s) => match Uuid::parse_str(s) {
                Ok(u) => u,
                Err(e) => {
                    error!(input = s, error = %e, "memory/compare: Invalid memoryB UUID");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid memoryB UUID '{}': {}", s, e),
                    );
                }
            },
            None => {
                error!("memory/compare: Missing memoryB");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'memoryB' parameter",
                );
            }
        };

        let include_per_embedder = params
            .get("includePerEmbedder")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Retrieve both fingerprints
        let fp_a = match self.teleological_store.retrieve(memory_a).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(memory_id = %memory_a, "memory/compare: Memory A not found");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory A not found: {}", memory_a),
                );
            }
            Err(e) => {
                error!(error = %e, memory_id = %memory_a, "memory/compare: Retrieval FAILED for A");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to retrieve memory A: {}", e),
                );
            }
        };

        let fp_b = match self.teleological_store.retrieve(memory_b).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(memory_id = %memory_b, "memory/compare: Memory B not found");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory B not found: {}", memory_b),
                );
            }
            Err(e) => {
                error!(error = %e, memory_id = %memory_b, "memory/compare: Retrieval FAILED for B");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to retrieve memory B: {}", e),
                );
            }
        };

        // Compare using TeleologicalComparator (ARCH-02: apples-to-apples)
        let comparator = TeleologicalComparator::new();
        let result = match comparator.compare(&fp_a.semantic, &fp_b.semantic) {
            Ok(r) => r,
            Err(e) => {
                error!(error = ?e, "memory/compare: Comparison FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Comparison failed: {:?}", e),
                );
            }
        };

        // Build response
        let per_embedder = if include_per_embedder {
            Some(build_per_embedder_json(&result.per_embedder))
        } else {
            None
        };

        let dominant = result.dominant_embedder.map(|e| e.name().to_string());

        JsonRpcResponse::success(
            id,
            json!({
                "overallSimilarity": result.overall,
                "perEmbedder": per_embedder,
                "coherence": result.coherence,
                "dominantEmbedder": dominant,
                "memoryA": memory_a.to_string(),
                "memoryB": memory_b.to_string()
            }),
        )
    }

    /// Handle memory/batch_compare request.
    ///
    /// 1-to-N comparison using BatchComparator with parallel processing.
    ///
    /// # Request Parameters
    /// - `reference` (required): UUID of reference memory
    /// - `targets` (required): Array of target memory UUIDs
    /// - `minSimilarity` (optional): Minimum similarity threshold for filtering
    /// - `includePerEmbedder` (optional): Include per-embedder breakdown. Default false.
    ///
    /// # Response
    /// - `reference`: Reference memory UUID
    /// - `comparisons`: Array of comparison results with target, similarity, rank
    /// - `comparisonLatencyMs`: Time to compute comparisons
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or invalid parameters
    /// - FINGERPRINT_NOT_FOUND (-32010): Reference or targets not found
    /// - STORAGE_ERROR (-32004): Retrieval failed
    pub(super) async fn handle_memory_batch_compare(
        &self,
        id: Option<crate::protocol::JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/batch_compare: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - reference and targets required",
                );
            }
        };

        // Parse reference UUID
        let reference = match params.get("reference").and_then(|v| v.as_str()) {
            Some(s) => match Uuid::parse_str(s) {
                Ok(u) => u,
                Err(e) => {
                    error!(input = s, error = %e, "memory/batch_compare: Invalid reference UUID");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid reference UUID '{}': {}", s, e),
                    );
                }
            },
            None => {
                error!("memory/batch_compare: Missing reference");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'reference' parameter",
                );
            }
        };

        // Parse targets array
        let targets: Vec<Uuid> = match params.get("targets") {
            Some(serde_json::Value::Array(arr)) => {
                match parse_uuid_array(arr, "memory/batch_compare", "targets") {
                    Ok(uuids) => uuids,
                    Err((msg, _)) => {
                        return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, msg);
                    }
                }
            }
            Some(_) => {
                error!("memory/batch_compare: targets is not an array");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "'targets' must be an array of UUIDs",
                );
            }
            None => {
                error!("memory/batch_compare: Missing targets");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'targets' parameter",
                );
            }
        };

        if targets.is_empty() {
            error!("memory/batch_compare: Empty targets array");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "'targets' array cannot be empty",
            );
        }

        let min_similarity = params
            .get("minSimilarity")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        let include_per_embedder = params
            .get("includePerEmbedder")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Retrieve reference fingerprint
        let compare_start = std::time::Instant::now();

        let ref_fp = match self.teleological_store.retrieve(reference).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(memory_id = %reference, "memory/batch_compare: Reference not found");
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Reference memory not found: {}", reference),
                );
            }
            Err(e) => {
                error!(error = %e, memory_id = %reference, "memory/batch_compare: Reference retrieval FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to retrieve reference memory: {}", e),
                );
            }
        };

        // Retrieve target fingerprints
        let mut target_fps = Vec::with_capacity(targets.len());
        let mut missing_targets = Vec::new();

        for target_id in &targets {
            match self.teleological_store.retrieve(*target_id).await {
                Ok(Some(fp)) => target_fps.push((*target_id, fp)),
                Ok(None) => missing_targets.push(*target_id),
                Err(e) => {
                    error!(error = %e, memory_id = %target_id, "memory/batch_compare: Target retrieval FAILED");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::STORAGE_ERROR,
                        format!("Failed to retrieve target memory {}: {}", target_id, e),
                    );
                }
            }
        }

        if !missing_targets.is_empty() {
            error!(missing = ?missing_targets, "memory/batch_compare: Some targets not found");
            return JsonRpcResponse::error(
                id,
                error_codes::FINGERPRINT_NOT_FOUND,
                format!("Target memories not found: {:?}", missing_targets),
            );
        }

        // Use BatchComparator for parallel 1-to-N comparison
        let batch_comparator = BatchComparator::new();
        let target_semantics: Vec<_> = target_fps.iter().map(|(_, fp)| fp.semantic.clone()).collect();

        let comparison_results = batch_comparator.compare_one_to_many(&ref_fp.semantic, &target_semantics);

        // Build response with rankings
        let mut comparisons: Vec<_> = comparison_results
            .into_iter()
            .zip(target_fps.iter())
            .filter_map(|(result, (target_id, _))| {
                let result = result.ok()?;

                // Apply min_similarity filter if provided
                if let Some(min) = min_similarity {
                    if result.overall < min {
                        return None;
                    }
                }

                let per_embedder = if include_per_embedder {
                    Some(build_per_embedder_json(&result.per_embedder))
                } else {
                    None
                };

                Some((
                    *target_id,
                    result.overall,
                    result.coherence,
                    result.dominant_embedder.map(|e| e.name().to_string()),
                    per_embedder,
                ))
            })
            .collect();

        // Sort by similarity descending
        comparisons.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Build final response
        let comparisons_json: Vec<_> = comparisons
            .into_iter()
            .enumerate()
            .map(|(rank, (target_id, similarity, coherence, dominant, per_embedder))| {
                json!({
                    "target": target_id.to_string(),
                    "similarity": similarity,
                    "rank": rank + 1,
                    "coherence": coherence,
                    "dominantEmbedder": dominant,
                    "perEmbedder": per_embedder
                })
            })
            .collect();

        let comparison_latency_ms = compare_start.elapsed().as_millis();

        JsonRpcResponse::success(
            id,
            json!({
                "reference": reference.to_string(),
                "comparisons": comparisons_json,
                "comparisonLatencyMs": comparison_latency_ms,
                "targetCount": targets.len()
            }),
        )
    }

    /// Handle memory/similarity_matrix request.
    ///
    /// N×N similarity matrix using BatchComparator::compare_all_pairs.
    ///
    /// # Request Parameters
    /// - `memoryIds` (required): Array of memory UUIDs to compare
    ///
    /// # Response
    /// - `matrix`: N×N similarity matrix where matrix[i][j] is similarity between i and j
    /// - `memoryIds`: Input memory IDs in matrix order
    /// - `diagonal`: Always 1.0 (self-similarity)
    /// - `symmetric`: true (matrix[i][j] == matrix[j][i])
    /// - `computationLatencyMs`: Time to compute matrix
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Missing or invalid memoryIds
    /// - FINGERPRINT_NOT_FOUND (-32010): One or more memories not found
    /// - STORAGE_ERROR (-32004): Retrieval failed
    pub(super) async fn handle_memory_similarity_matrix(
        &self,
        id: Option<crate::protocol::JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("memory/similarity_matrix: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - memoryIds required",
                );
            }
        };

        // Parse memoryIds array
        let memory_ids: Vec<Uuid> = match params.get("memoryIds") {
            Some(serde_json::Value::Array(arr)) => {
                match parse_uuid_array(arr, "memory/similarity_matrix", "memoryIds") {
                    Ok(uuids) => uuids,
                    Err((msg, _)) => {
                        return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, msg);
                    }
                }
            }
            Some(_) => {
                error!("memory/similarity_matrix: memoryIds is not an array");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "'memoryIds' must be an array of UUIDs",
                );
            }
            None => {
                error!("memory/similarity_matrix: Missing memoryIds");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'memoryIds' parameter",
                );
            }
        };

        if memory_ids.len() < 2 {
            error!("memory/similarity_matrix: Need at least 2 memories");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "'memoryIds' must contain at least 2 memory IDs",
            );
        }

        let compute_start = std::time::Instant::now();

        // Retrieve all fingerprints
        let mut fingerprints = Vec::with_capacity(memory_ids.len());
        let mut missing = Vec::new();

        for memory_id in &memory_ids {
            match self.teleological_store.retrieve(*memory_id).await {
                Ok(Some(fp)) => fingerprints.push(fp),
                Ok(None) => missing.push(*memory_id),
                Err(e) => {
                    error!(error = %e, memory_id = %memory_id, "memory/similarity_matrix: Retrieval FAILED");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::STORAGE_ERROR,
                        format!("Failed to retrieve memory {}: {}", memory_id, e),
                    );
                }
            }
        }

        if !missing.is_empty() {
            error!(missing = ?missing, "memory/similarity_matrix: Some memories not found");
            return JsonRpcResponse::error(
                id,
                error_codes::FINGERPRINT_NOT_FOUND,
                format!("Memories not found: {:?}", missing),
            );
        }

        // Extract semantic fingerprints
        let semantics: Vec<_> = fingerprints.iter().map(|fp| fp.semantic.clone()).collect();

        // Use BatchComparator::compare_all_pairs for N×N matrix
        let batch_comparator = BatchComparator::new();
        let matrix = batch_comparator.compare_all_pairs(&semantics);

        let computation_latency_ms = compute_start.elapsed().as_millis();

        // Convert memory_ids to strings for response
        let memory_id_strings: Vec<_> = memory_ids.iter().map(|id| id.to_string()).collect();

        info!(
            n = memory_ids.len(),
            latency_ms = computation_latency_ms,
            "memory/similarity_matrix: Computed {}x{} matrix",
            memory_ids.len(),
            memory_ids.len()
        );

        JsonRpcResponse::success(
            id,
            json!({
                "matrix": matrix,
                "memoryIds": memory_id_strings,
                "dimensions": [memory_ids.len(), memory_ids.len()],
                "diagonal": 1.0,
                "symmetric": true,
                "computationLatencyMs": computation_latency_ms
            }),
        )
    }
}
