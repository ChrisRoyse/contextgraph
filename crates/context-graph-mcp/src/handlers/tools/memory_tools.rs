//! Memory operation tool implementations (store_memory, search_graph).
//!
//! Note: inject_context was merged into store_memory. When `rationale` is provided,
//! the same validation (1-1024 chars) and response format is used.
//!
//! # Multi-Space Search (ARCH-12, ARCH-21)
//!
//! The `search_graph` tool uses the storage layer directly with three strategies:
//!
//! - `e1_only`: E1-only HNSW search (fast, simple queries)
//! - `multi_space`: Weighted RRF fusion of E1 + enhancers (default - uses weight profiles)
//! - `pipeline`: Multi-stage retrieval (E13 sparse recall → multi-space scoring)
//!
//! E1 is the foundation (ARCH-12). Other embedders ENHANCE E1 by finding blind spots.
//! Weight profiles control how much each embedder contributes (e.g., code_search boosts E7).
//!
//! Temporal embedders (E2-E4) are POST-RETRIEVAL only per ARCH-25, AP-73.
//!
//! # E5 Causal Asymmetric Similarity (ARCH-15, AP-77)
//!
//! For causal queries ("why", "what happens"), asymmetric E5 reranking is applied:
//! - Direction detection: Detects if query seeks causes or effects
//! - Asymmetric vectors: Uses `query.e5_as_cause` vs `doc.e5_as_effect` (or reverse)
//! - Direction modifiers: cause→effect (1.2x), effect→cause (0.8x)
//! - Auto-profile selection: Causal queries auto-select "causal_reasoning" profile

use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use context_graph_core::causal::asymmetric::{
    apply_causal_gate, causal_gate, compute_e5_asymmetric_fingerprint_similarity,
    detect_causal_query_intent, CausalDirection,
};
use context_graph_core::types::audit::{AuditOperation, AuditRecord};
use context_graph_core::teleological::matrix_search::embedder_names;
use context_graph_core::traits::{EmbeddingMetadata, SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::fingerprint::{SemanticFingerprint, TeleologicalFingerprint, NUM_EMBEDDERS};
use context_graph_core::types::{SourceMetadata, SourceType};

use crate::weights::get_weight_profile;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::helpers::ToolErrorKind;
use super::super::Handlers;

// Validation constants for store_memory rationale (merged from inject_context)
// When rationale is provided, validate: 1-1024 chars
const MIN_RATIONALE_LEN: usize = 1;
const MAX_RATIONALE_LEN: usize = 1024;

// Validation constants for search_graph (BUG-001)
// Per PRD Section 10: topK must be 1-100
const MIN_TOP_K: u64 = 1;
const MAX_TOP_K: u64 = 100;

// E5 Causal Direction inference threshold
// Per Phase 5: Infer causal direction from E5 embedding norms
const CAUSAL_DIRECTION_THRESHOLD: f32 = 0.1;

/// Infer causal direction from E5 asymmetric embeddings.
///
/// Compares the norms of the e5_causal_as_cause and e5_causal_as_effect vectors
/// to determine if the content primarily describes causes or effects.
///
/// # Returns
/// - Some("cause") if cause norm is significantly higher (>10% difference)
/// - Some("effect") if effect norm is significantly higher
/// - Some("unknown") if norms are similar or both are near zero
fn infer_causal_direction_from_fingerprint(fingerprint: &SemanticFingerprint) -> String {
    let cause_norm: f32 = fingerprint
        .e5_causal_as_cause
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt();
    let effect_norm: f32 = fingerprint
        .e5_causal_as_effect
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt();

    // Require >10% difference to be confident in direction
    let diff_ratio = if effect_norm > f32::EPSILON {
        (cause_norm - effect_norm) / effect_norm
    } else if cause_norm > f32::EPSILON {
        1.0 // All cause, no effect
    } else {
        0.0 // Both zero
    };

    if diff_ratio > CAUSAL_DIRECTION_THRESHOLD {
        "cause".to_string()
    } else if diff_ratio < -CAUSAL_DIRECTION_THRESHOLD {
        "effect".to_string()
    } else {
        "unknown".to_string()
    }
}

impl Handlers {
    /// store_memory tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore with 13-embedding fingerprint.
    ///
    /// Stores content in the memory graph. Generates all 13 embeddings for the content
    /// and stores the resulting TeleologicalFingerprint.
    ///
    /// Note: inject_context was merged into this tool. When `rationale` is provided,
    /// the same validation (1-1024 chars) and response format is used.
    pub(crate) async fn call_store_memory(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => return self.tool_error(id, "Content cannot be empty"),
            None => return self.tool_error(id, "Missing 'content' parameter"),
        };

        // Handle optional rationale (merged from inject_context)
        // When provided, validate 1-1024 chars and include in response
        let rationale = args.get("rationale").and_then(|v| v.as_str());
        if let Some(r) = rationale {
            if r.len() < MIN_RATIONALE_LEN {
                error!(
                    rationale_len = r.len(),
                    min_required = MIN_RATIONALE_LEN,
                    "store_memory: rationale validation FAILED - empty"
                );
                return self.tool_error(id, "rationale must be at least 1 character");
            }
            if r.len() > MAX_RATIONALE_LEN {
                error!(
                    rationale_len = r.len(),
                    max_allowed = MAX_RATIONALE_LEN,
                    "store_memory: rationale validation FAILED - exceeds maximum"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "rationale must be at most {} characters, got {}",
                        MAX_RATIONALE_LEN,
                        r.len()
                    ),
                );
            }
        }

        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .unwrap_or(TeleologicalFingerprint::DEFAULT_IMPORTANCE);

        // SESSION-ID-FIX: Priority: tool argument > env var > stored session_id > auto-generate
        // MUST resolve session ID BEFORE get_next_sequence() because auto-generation
        // via set_session_id() resets the sequence counter.
        // Uses get_or_init_session_id() for atomic check-and-set (no TOCTOU race).
        let session_id = args
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| Some(self.get_or_init_session_id()));
        // E4-FIX: Get session sequence AFTER session ID resolution
        let session_sequence = self.get_next_sequence();

        // PHASE-1.2: Extract operatorId for provenance tracking
        // Accepts both camelCase (schema) and snake_case (legacy) for backward compatibility
        let operator_id = args
            .get("operatorId")
            .or_else(|| args.get("operator_id"))
            .and_then(|v| v.as_str())
            .map(String::from);

        // CAUSAL-HINT: Get causal hint if provider is available (non-blocking with timeout)
        // Per Phase 5: LLM analyzes content for causal nature, provides hints to E5 embedder
        // CAUSAL-HINT-FIX: Clone hint before moving into metadata so we can use direction for storage
        let causal_hint = if let Some(provider) = &self.causal_hint_provider {
            if provider.is_available() {
                match provider.get_hint(&content).await {
                    Some(hint) => {
                        debug!(
                            is_causal = hint.is_causal,
                            direction = ?hint.direction_hint,
                            confidence = hint.confidence,
                            key_phrases = ?hint.key_phrases,
                            "store_memory: Got causal hint from LLM"
                        );
                        Some(hint)
                    }
                    None => {
                        debug!("store_memory: Causal hint provider returned None (timeout or low confidence)");
                        None
                    }
                }
            } else {
                debug!("store_memory: Causal hint provider not available");
                None
            }
        } else {
            None // No provider configured
        };

        // CAUSAL-HINT-FIX: Preserve LLM hint direction for storage (before moving into metadata)
        // The hint direction comes from the LLM and should be used directly, not inferred from E5 norms
        let llm_causal_direction: Option<String> = causal_hint.as_ref().and_then(|hint| {
            if hint.is_useful() {
                Some(match hint.direction_hint {
                    context_graph_core::traits::CausalDirectionHint::Cause => "cause".to_string(),
                    context_graph_core::traits::CausalDirectionHint::Effect => "effect".to_string(),
                    context_graph_core::traits::CausalDirectionHint::Neutral => "unknown".to_string(),
                })
            } else {
                None
            }
        });

        let metadata = EmbeddingMetadata {
            session_id: session_id.clone(),
            session_sequence: Some(session_sequence),
            timestamp: Some(chrono::Utc::now()),
            causal_hint,
        };

        debug!(
            session_sequence = session_sequence,
            session_id = ?session_id,
            "store_memory: Using session sequence for E4 embedding"
        );

        // Generate all 13 embeddings using MultiArrayEmbeddingProvider
        // E4-FIX: Use embed_all_with_metadata to pass sequence number to E4
        let embedding_output = match self
            .multi_array_provider
            .embed_all_with_metadata(&content, metadata)
            .await
        {
            Ok(output) => output,
            Err(e) => {
                error!(error = %e, "store_memory: Multi-array embedding FAILED");
                return self.tool_error(id, &format!("Embedding failed: {}", e));
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // TASK-FIX-CLUSTERING: Compute cluster array BEFORE fingerprint is consumed
        // This must be done before TeleologicalFingerprint::new() moves the semantic fingerprint.
        let cluster_array = embedding_output.fingerprint.to_cluster_array();

        // E5 Phase 5: Determine causal direction for storage
        // CAUSAL-HINT-FIX: Use LLM hint direction if available (high confidence from 7B model)
        // Fall back to E5 norm inference only when no LLM hint is available
        let causal_direction = llm_causal_direction.unwrap_or_else(|| {
            debug!("store_memory: No LLM hint, inferring causal direction from E5 norms");
            infer_causal_direction_from_fingerprint(&embedding_output.fingerprint)
        });

        // E6-FIX: Extract e6_sparse BEFORE creating TeleologicalFingerprint
        // The SemanticFingerprint.e6_sparse is a SparseVector that must be copied
        // to TeleologicalFingerprint.e6_sparse (Option<SparseVector>) for inverted index storage.
        // This enables Stage 1 E6 recall and keyword tie-breaking.
        let e6_sparse = embedding_output.fingerprint.e6_sparse.clone();

        // Create TeleologicalFingerprint from embeddings with user-specified importance
        // E6-FIX: Chain .with_e6_sparse() to propagate the E6 sparse vector
        let fingerprint =
            TeleologicalFingerprint::with_importance(embedding_output.fingerprint, content_hash, importance)
                .with_e6_sparse(e6_sparse);
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

                // TASK-GRAPHLINK-PHASE1: Enqueue fingerprint for background K-NN graph building
                // The BackgroundGraphBuilder will process this in batch every 60s (configurable)
                if let Some(builder) = self.graph_builder() {
                    builder.enqueue(fingerprint_id).await;
                    debug!(
                        fingerprint_id = %fingerprint_id,
                        "store_memory: Enqueued for K-NN graph building"
                    );
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

                // E4-FIX Phase 1: Persist session metadata for E4 sequence retrieval
                // This enables proper before/after queries by storing session_sequence
                // PHASE-1.2: Add operator attribution fields for provenance tracking
                let source_metadata = SourceMetadata {
                    source_type: SourceType::Manual,
                    session_id: session_id.clone(),
                    session_sequence: Some(session_sequence),
                    file_path: None,
                    chunk_index: None,
                    total_chunks: None,
                    start_line: None,
                    end_line: None,
                    hook_type: None,
                    tool_name: None,
                    causal_direction: Some(causal_direction.clone()),
                    source_fingerprint_id: None,
                    causal_relationship_id: None,
                    mechanism_type: None,
                    confidence: None,
                    created_by: operator_id.clone(),
                    created_at: Some(chrono::Utc::now()),
                    file_content_hash: None,
                    file_modified_at: None,
                    derived_from: None,
                    derivation_method: None,
                    tool_use_id: None,
                    mcp_request_id: None,
                    hook_execution_timestamp_ms: None,
                    embedding_hint_provenance: embedding_output.e5_hint_provenance.clone(),
                };

                if let Err(e) = self
                    .teleological_store
                    .store_source_metadata(fingerprint_id, &source_metadata)
                    .await
                {
                    warn!(
                        fingerprint_id = %fingerprint_id,
                        error = %e,
                        session_sequence = session_sequence,
                        causal_direction = %causal_direction,
                        "store_memory: Failed to store source metadata (fingerprint saved successfully). \
                         E4 sequence retrieval may fall back to timestamp-based ordering."
                    );
                } else {
                    debug!(
                        fingerprint_id = %fingerprint_id,
                        session_sequence = session_sequence,
                        causal_direction = %causal_direction,
                        "store_memory: Source metadata stored for E4 sequence retrieval"
                    );
                }

                // PHASE-1.2: Append audit record for memory creation
                {
                    use context_graph_core::types::audit::{AuditOperation, AuditRecord};
                    let mut audit_record = AuditRecord::new(AuditOperation::MemoryCreated, fingerprint_id);
                    if let Some(ref op_id) = operator_id {
                        audit_record = audit_record.with_operator(op_id.clone());
                    }
                    if let Some(ref sess_id) = session_id {
                        audit_record = audit_record.with_session(sess_id.clone());
                    }
                    if let Some(r) = rationale {
                        audit_record = audit_record.with_rationale(r);
                    }
                    audit_record = audit_record.with_parameters(json!({
                        "importance": importance,
                        "content_size": content.len(),
                        "causal_direction": causal_direction,
                        "embedding_hint_provenance": embedding_output.e5_hint_provenance,
                    }));

                    if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                        // Non-fatal: audit is secondary to the main operation
                        error!(
                            fingerprint_id = %fingerprint_id,
                            error = %e,
                            "store_memory: Failed to append audit record (memory stored successfully)"
                        );
                    } else {
                        debug!(
                            fingerprint_id = %fingerprint_id,
                            audit_id = %audit_record.id,
                            "store_memory: Audit record appended successfully"
                        );
                    }
                }

                // Phase 5: Store embedding version record for provenance tracking
                {
                    use context_graph_core::types::audit::EmbeddingVersionRecord;
                    use std::collections::HashMap;

                    // NOTE: These are static descriptors, not dynamic model versions.
                    // The embedding provider does not currently expose runtime model metadata.
                    // Stale embedding detection requires the provider to report version changes.
                    let mut embedder_versions = HashMap::new();
                    embedder_versions.insert("E1".to_string(), "pretrained-semantic-1024d".to_string());
                    embedder_versions.insert("E2".to_string(), "temporal-recent-decay".to_string());
                    embedder_versions.insert("E3".to_string(), "temporal-periodic-fourier".to_string());
                    embedder_versions.insert("E4".to_string(), "temporal-positional-sequence".to_string());
                    embedder_versions.insert("E5".to_string(), "causal-asymmetric-768d".to_string());
                    embedder_versions.insert("E6".to_string(), "sparse-tfidf-inverted".to_string());
                    embedder_versions.insert("E7".to_string(), "qodo-embed-1-1.5b".to_string());
                    embedder_versions.insert("E8".to_string(), "graph-knn-1024d".to_string());
                    embedder_versions.insert("E9".to_string(), "structural-positional".to_string());
                    embedder_versions.insert("E10".to_string(), "multimodal-modifier-768d".to_string());
                    embedder_versions.insert("E11".to_string(), "entity-kepler-768d".to_string());
                    embedder_versions.insert("E12".to_string(), "late-interaction-colbert".to_string());
                    embedder_versions.insert("E13".to_string(), "keyword-splade-sparse".to_string());

                    let record = EmbeddingVersionRecord {
                        fingerprint_id,
                        computed_at: chrono::Utc::now(),
                        embedder_versions,
                        e7_model_version: Some("qodo-embed-1-1.5b".to_string()),
                        computation_time_ms: Some(embedding_output.total_latency.as_millis() as u64),
                    };

                    if let Err(e) = self.teleological_store.store_embedding_version(&record).await {
                        warn!(
                            fingerprint_id = %fingerprint_id,
                            error = %e,
                            "store_memory: Failed to store embedding version record (non-fatal)"
                        );
                    } else {
                        debug!(
                            fingerprint_id = %fingerprint_id,
                            computation_ms = embedding_output.total_latency.as_millis(),
                            "store_memory: Embedding version record stored"
                        );
                    }
                }

                // ===== INLINE CAUSAL RELATIONSHIP EXTRACTION =====
                // INLINE-CAUSAL: Extract ALL causal relationships from content using the
                // Hermes 2 Pro 7B LLM, generate proper asymmetric E5 embeddings for each
                // (cause text → cause vec, effect text → effect vec per AP-77), and persist
                // CausalRelationship records to CF_CAUSAL_RELATIONSHIPS with full provenance.
                //
                // This runs inline with the 13-embedder pipeline — no background loop.
                // Only available when `llm` feature is enabled.
                #[cfg(feature = "llm")]
                self.extract_inline_causal_relationships(&content, fingerprint_id).await;

                // Build response, including rationale if provided
                let mut response = json!({
                    "fingerprintId": fingerprint_id.to_string(),
                    "embedderCount": NUM_EMBEDDERS,
                    "embeddingLatencyMs": embedding_output.total_latency.as_millis()
                });

                // Include rationale in response when provided (merged from inject_context)
                if let Some(r) = rationale {
                    response["rationale"] = json!(r);
                }

                self.tool_result(id, response)
            }
            Err(e) => {
                error!(error = %e, "store_memory: Storage FAILED");
                self.tool_error(id, &format!("Storage failed: {}", e))
            }
        }
    }

    /// search_graph tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore search_semantic.
    ///
    /// Searches the memory graph for matching content using all 13 embedding spaces.
    pub(crate) async fn call_search_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            Some(_) => return self.tool_error(id, "Query cannot be empty"),
            None => return self.tool_error(id, "Missing 'query' parameter"),
        };

        let raw_top_k = args.get("topK").and_then(|v| v.as_u64());
        if let Some(k) = raw_top_k {
            if k < MIN_TOP_K {
                error!(
                    top_k = k,
                    min_allowed = MIN_TOP_K,
                    "search_graph: topK validation FAILED - below minimum"
                );
                return self.tool_error(
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
                return self.tool_error(
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

        // PHASE-2-PROVENANCE: Parse includeProvenance parameter (default: false)
        // When true, each result includes a nested "provenance" object with
        // full retrieval transparency: strategy, weight profile, query classification,
        // per-embedder contributions, consensus score, and blind spot detection.
        let include_provenance = args
            .get("includeProvenance")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // =========================================================================
        // SEARCH STRATEGY (ARCH-12, ARCH-21)
        // =========================================================================
        // Parse strategy parameter (default: multi_space for optimal blind spot detection)
        // - e1_only: E1-only HNSW search (fast, simple queries)
        // - multi_space: Weighted RRF fusion of E1 + enhancers (default - uses weight profiles)
        // - pipeline: Multi-stage retrieval (E13 sparse recall → multi-space scoring)
        //
        // E1 is the foundation (ARCH-12). Other embedders ENHANCE E1 by finding blind spots.
        let strategy = match args.get("strategy").and_then(|v| v.as_str()) {
            Some("e1_only") => SearchStrategy::E1Only,
            Some("pipeline") => SearchStrategy::Pipeline,
            Some("multi_space") | None => SearchStrategy::MultiSpace,
            Some(unknown) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!("Unknown strategy '{}'. Valid: e1_only, multi_space, pipeline", unknown),
                );
            }
        };

        // TASK-MULTISPACE: Parse weight profile (default: "semantic_search")
        let weight_profile = args
            .get("weightProfile")
            .and_then(|v| v.as_str())
            .map(String::from);

        // GAP-1: Parse custom weights (overrides weightProfile when provided)
        // AP-NAV-01: FAIL FAST on invalid embedder names
        let custom_weights: Option<[f32; 13]> = match args.get("customWeights").and_then(|v| v.as_object()) {
            Some(obj) => {
                const VALID_NAMES: [&str; 13] = ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"];
                // Reject any keys that are not valid embedder names
                for key in obj.keys() {
                    if !VALID_NAMES.contains(&key.as_str()) {
                        error!(invalid_key = %key, "search_graph: customWeights contains invalid embedder name");
                        return self.tool_error(id, &format!(
                            "Invalid embedder name '{}' in customWeights. Valid names: E1-E13.", key
                        ));
                    }
                }
                let mut weights = [0.0f32; 13];
                for (i, name) in VALID_NAMES.iter().enumerate() {
                    if let Some(val) = obj.get(*name).and_then(|v| v.as_f64()) {
                        weights[i] = val as f32;
                    }
                }
                Some(weights)
            }
            None => None,
        };

        // GAP-8: Parse exclude embedders
        let mut exclude_embedder_names: Vec<String> = Vec::new();
        let exclude_embedders: Vec<usize> = match args.get("excludeEmbedders").and_then(|v| v.as_array()) {
            Some(arr) => {
                let mut indices = Vec::new();
                for v in arr {
                    let s = match v.as_str() {
                        Some(s) => s,
                        None => {
                            error!("search_graph: excludeEmbedders contains non-string value");
                            return self.tool_error(id, "excludeEmbedders must contain strings (E1-E13)");
                        }
                    };
                    let idx = match s {
                        "E1" => 0, "E2" => 1, "E3" => 2, "E4" => 3, "E5" => 4, "E6" => 5,
                        "E7" => 6, "E8" => 7, "E9" => 8, "E10" => 9, "E11" => 10, "E12" => 11,
                        "E13" => 12,
                        _ => {
                            error!(embedder = %s, "search_graph: Invalid embedder in excludeEmbedders");
                            return self.tool_error(id, &format!("Invalid embedder '{}' in excludeEmbedders. Must be E1-E13.", s));
                        }
                    };
                    exclude_embedder_names.push(s.to_string());
                    indices.push(idx);
                }
                indices
            }
            None => Vec::new(),
        };

        // GAP-6: Parse include embedder breakdown
        let include_embedder_breakdown = args
            .get("includeEmbedderBreakdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // TASK-MULTISPACE: Parse enable_rerank (default: false)
        // Per AP-73: ColBERT is for re-ranking only
        let enable_rerank = args
            .get("enableRerank")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // =========================================================================
        // TEMPORAL SEARCH PARAMETERS (ARCH-14)
        // =========================================================================

        // Parse temporalWeight (master weight for all temporal boosts)
        let temporal_weight = args
            .get("temporalWeight")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        // Parse decayFunction (linear, exponential, step, none)
        let decay_function = match args.get("decayFunction").and_then(|v| v.as_str()) {
            Some("linear") | None => context_graph_core::traits::DecayFunction::Linear,
            Some("exponential") => context_graph_core::traits::DecayFunction::Exponential,
            Some("step") => context_graph_core::traits::DecayFunction::Step,
            Some("none") | Some("no_decay") => context_graph_core::traits::DecayFunction::NoDecay,
            Some(unknown) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!("Unknown decayFunction '{}'. Valid: linear, exponential, step, none, no_decay", unknown),
                );
            }
        };

        // Parse decayHalfLifeSecs (for exponential decay)
        let decay_half_life = args
            .get("decayHalfLifeSecs")
            .and_then(|v| v.as_u64())
            .unwrap_or(86400); // 1 day default

        // Parse lastHours shortcut (filter to last N hours)
        let last_hours = args.get("lastHours").and_then(|v| v.as_u64());

        // Parse lastDays shortcut (filter to last N days)
        let last_days = args.get("lastDays").and_then(|v| v.as_u64());

        // Parse sessionId (filter to specific session)
        let session_id = args
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse periodicBoost (weight for E3 periodic matching)
        let periodic_boost = args
            .get("periodicBoost")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0));

        // Parse targetHour (0-23) for periodic matching
        let target_hour = args
            .get("targetHour")
            .and_then(|v| v.as_u64())
            .map(|v| (v as u8).min(23));

        // Parse targetDayOfWeek (0=Sun, 6=Sat) for periodic matching
        let target_day_of_week = args
            .get("targetDayOfWeek")
            .and_then(|v| v.as_u64())
            .map(|v| (v as u8).min(6));

        // Parse sequenceAnchor (UUID) for E4 sequence-based retrieval
        let sequence_anchor = args
            .get("sequenceAnchor")
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok());

        // Parse sequenceDirection (before, after, both)
        let sequence_direction = args
            .get("sequenceDirection")
            .and_then(|v| v.as_str())
            .map(|s| match s.to_lowercase().as_str() {
                "before" => context_graph_core::traits::SequenceDirection::Before,
                "after" => context_graph_core::traits::SequenceDirection::After,
                _ => context_graph_core::traits::SequenceDirection::Both,
            })
            .unwrap_or(context_graph_core::traits::SequenceDirection::Both);

        // Parse temporalScale (micro, meso, macro, long, archival)
        let temporal_scale = match args.get("temporalScale").and_then(|v| v.as_str()) {
            Some("meso") | None => context_graph_core::traits::TemporalScale::Meso,
            Some("micro") => context_graph_core::traits::TemporalScale::Micro,
            Some("macro") => context_graph_core::traits::TemporalScale::Macro,
            Some("long") => context_graph_core::traits::TemporalScale::Long,
            Some("archival") => context_graph_core::traits::TemporalScale::Archival,
            Some(unknown) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!("Unknown temporalScale '{}'. Valid: micro, meso, macro, long, archival", unknown),
                );
            }
        };

        // =========================================================================
        // CONVERSATION CONTEXT PARAMETERS (E4 Sequence Integration)
        // =========================================================================

        // Parse conversationContext convenience wrapper
        let conversation_context = args.get("conversationContext");
        let anchor_to_current_turn = conversation_context
            .and_then(|c| c.get("anchorToCurrentTurn"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let turns_back = conversation_context
            .and_then(|c| c.get("turnsBack"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32;
        let turns_forward = conversation_context
            .and_then(|c| c.get("turnsForward"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // Parse sessionScope (current, all, recent)
        let session_scope = match args.get("sessionScope").and_then(|v| v.as_str()) {
            Some(s @ ("current" | "recent" | "all")) => s,
            None => "all",
            Some(unknown) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!("Unknown sessionScope '{}'. Valid: current, recent, all", unknown),
                );
            }
        };

        // Auto-select sequence_navigation profile if conversationContext is used
        // This ensures E4 (V_ordering) is prioritized for sequence-based retrieval
        let use_conversation_context = conversation_context.is_some() && anchor_to_current_turn;

        // =========================================================================
        // E5 CAUSAL ASYMMETRIC PARAMETERS (ARCH-15, AP-77)
        // =========================================================================

        // Parse enableAsymmetricE5 (default: true)
        // When enabled, asymmetric E5 reranking is applied for causal queries
        let enable_asymmetric_e5 = args
            .get("enableAsymmetricE5")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Parse causalDirection (auto, cause, effect, none)
        // - auto: Auto-detect from query text (default)
        // - cause: Force query as seeking causes (for "why" queries)
        // - effect: Force query as seeking effects (for "what happens" queries)
        // - none: Disable causal processing
        let causal_direction_param = args
            .get("causalDirection")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        // Parse enableQueryExpansion (default: false)
        // When enabled, causal queries are expanded with related terms
        let enable_query_expansion = args
            .get("enableQueryExpansion")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // =========================================================================
        // PHASE 1: CAUSAL DIRECTION DETECTION
        // =========================================================================

        // Detect causal direction from query text or use user-specified direction
        let causal_direction = match causal_direction_param {
            "cause" => CausalDirection::Cause,
            "effect" => CausalDirection::Effect,
            "none" => CausalDirection::Unknown,
            "auto" | _ => detect_causal_query_intent(query),
        };

        // Log causal detection for debugging/monitoring
        if causal_direction != CausalDirection::Unknown {
            info!(
                direction = %causal_direction,
                query_preview = %query.chars().take(100).collect::<String>(),
                "Causal query detected - asymmetric E5 reranking will be applied"
            );
        }

        // =========================================================================
        // PHASE 5: QUERY EXPANSION (Optional)
        // =========================================================================

        // Expand causal queries with related terms for better recall
        let search_query = if enable_query_expansion && causal_direction != CausalDirection::Unknown {
            expand_causal_query(query, causal_direction)
        } else {
            query.to_string()
        };

        // Auto-select weight profile based on query type
        // Priority: user-specified > causal > conversation_context > default
        let effective_weight_profile = match (&weight_profile, &causal_direction, use_conversation_context) {
            // User specified a profile - always use it
            (Some(profile), _, _) => Some(profile.clone()),
            // Causal query detected - use causal_reasoning
            (None, CausalDirection::Cause | CausalDirection::Effect, _) => {
                debug!("Auto-selecting 'causal_reasoning' profile for causal query");
                Some("causal_reasoning".to_string())
            }
            // Conversation context enabled - use conversation_history for balanced E1+E4
            (None, CausalDirection::Unknown, true) => {
                debug!("Auto-selecting 'conversation_history' profile for conversation context");
                Some("conversation_history".to_string())
            }
            // No special case - use default
            (None, CausalDirection::Unknown, false) => weight_profile.clone(),
        };

        // Build search options with multi-space parameters
        // For causal queries, over-fetch candidates to allow for reranking
        let fetch_multiplier = if enable_asymmetric_e5 && causal_direction != CausalDirection::Unknown {
            3 // Fetch 3x candidates for asymmetric reranking
        } else {
            1
        };
        let fetch_top_k = top_k * fetch_multiplier;

        let mut options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_min_similarity(min_similarity)
            .with_strategy(strategy)
            .with_rerank(enable_rerank)
            .with_causal_direction(causal_direction); // ARCH-15, AP-77: Thread direction to retrieval

        if let Some(ref profile) = effective_weight_profile {
            // Check custom profiles first, then fall back to built-in
            let custom = self.custom_profiles.read().get(profile).copied();
            if let Some(custom_weights_from_profile) = custom {
                // Custom profile found - pass as custom_weights array (bypasses storage layer lookup)
                options = options.with_custom_weights(custom_weights_from_profile);
                // MCP-03 FIX: Must explicitly set MultiSpace for custom weights to work.
                // Previously this used the `strategy` variable which could be E1Only,
                // making the custom weights completely ignored.
                options = options.with_strategy(SearchStrategy::MultiSpace);
                debug!(profile = %profile, "Resolved custom weight profile from RocksDB cache, forced MultiSpace strategy");
            } else {
                options = options.with_weight_profile(profile);
            }
        }

        // GAP-1: Explicit custom weights override everything (including custom profiles)
        // HIGH-08 FIX: Validate weights BEFORE applying (AP-NAV-02)
        if let Some(weights) = custom_weights {
            if let Err(e) = context_graph_core::weights::validate_weights(&weights) {
                error!(error = %e, "search_graph: invalid custom weights");
                return self.tool_error(id, &format!("Invalid custom weights: {}", e));
            }
            options = options.with_custom_weights(weights);
        }

        // GAP-8: Exclude embedders
        if !exclude_embedders.is_empty() {
            options = options.with_exclude_embedders(exclude_embedders);
        }

        // Apply temporal options
        // Per ARCH-14: Temporal is a POST-retrieval boost, not similarity
        if temporal_weight > 0.0 {
            options = options
                .with_temporal_weight(temporal_weight)
                .with_decay_function(decay_function)
                .with_temporal_scale(temporal_scale);

            // Apply decay half-life if exponential
            if matches!(decay_function, context_graph_core::traits::DecayFunction::Exponential) {
                options.temporal_options.decay_half_life_secs = decay_half_life;
            }
        }

        // Apply time window filters (shortcuts)
        if let Some(hours) = last_hours {
            options = options.with_last_hours(hours);
        } else if let Some(days) = last_days {
            options = options.with_last_days(days);
        }

        // =========================================================================
        // SESSION SCOPE HANDLING (Phase 2 Enhancement)
        // =========================================================================
        // sessionScope takes precedence over explicit sessionId for convenience
        match session_scope {
            "current" => {
                // Filter to current session only
                if let Some(sid) = self.get_session_id() {
                    options = options.with_session_filter(&sid);
                    debug!(session_id = %sid, "Applying 'current' session scope");
                }
            }
            "recent" => {
                // Filter to last 24 hours across sessions
                options = options.with_last_hours(24);
                debug!("Applying 'recent' session scope (last 24h)");
            }
            "all" => {
                // No session filtering - search all memories
                // But still allow explicit sessionId to override
                if let Some(ref sid) = session_id {
                    options = options.with_session_filter(sid);
                }
            }
            _ => unreachable!("sessionScope validated above")
        }

        // Apply periodic boost if configured
        if let Some(weight) = periodic_boost {
            let mut periodic = context_graph_core::traits::PeriodicOptions::default();
            periodic.weight = weight;
            if let Some(hour) = target_hour {
                periodic.target_hour = Some(hour);
            }
            if let Some(dow) = target_day_of_week {
                periodic.target_day_of_week = Some(dow);
            }
            // Auto-detect if no specific targets set
            if periodic.target_hour.is_none() && periodic.target_day_of_week.is_none() {
                periodic.auto_detect = true;
            }
            options.temporal_options.periodic_options = Some(periodic);
        }

        // =========================================================================
        // CONVERSATION CONTEXT HANDLING (Phase 2 Enhancement)
        // =========================================================================
        // conversationContext provides a convenience wrapper for E4 sequence-based retrieval
        // It auto-anchors to current turn and sets up sequence options
        if use_conversation_context {
            // Get current sequence number for anchoring
            let current_seq = self.current_sequence();

            // Determine sequence direction based on turns_back/turns_forward
            let conv_direction = match (turns_back > 0, turns_forward > 0) {
                (true, true) => context_graph_core::traits::SequenceDirection::Both,
                (true, false) => context_graph_core::traits::SequenceDirection::Before,
                (false, true) => context_graph_core::traits::SequenceDirection::After,
                (false, false) => context_graph_core::traits::SequenceDirection::Both, // Default
            };

            // Use the new from_sequence constructor for sequence-based anchoring
            let max_dist = std::cmp::max(turns_back, turns_forward);
            let seq_opts = context_graph_core::traits::SequenceOptions::from_sequence(
                current_seq,
                conv_direction,
                max_dist,
            );
            options.temporal_options.sequence_options = Some(seq_opts);

            debug!(
                current_seq = current_seq,
                turns_back = turns_back,
                turns_forward = turns_forward,
                direction = ?conv_direction,
                "Applying conversationContext with auto-anchor"
            );
        } else if let Some(anchor_id) = sequence_anchor {
            // Fall back to explicit sequenceAnchor if provided
            let seq_opts = context_graph_core::traits::SequenceOptions::around(anchor_id)
                .with_direction(sequence_direction);
            options.temporal_options.sequence_options = Some(seq_opts);
        }

        debug!(
            strategy = ?strategy,
            weight_profile = ?options.weight_profile,
            enable_rerank = enable_rerank,
            temporal_weight = temporal_weight,
            causal_direction = %causal_direction,
            enable_asymmetric_e5 = enable_asymmetric_e5,
            "search_graph: Multi-space, temporal, and causal options configured"
        );

        // Generate query embedding using potentially expanded query
        let query_embedding = match self.embed_query(id.clone(), &search_query, "search_graph").await {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // =========================================================================
        // STORAGE LAYER SEARCH (ARCH-12, ARCH-21)
        // =========================================================================
        // All search goes through the storage layer which handles:
        // - E1 foundation search (ARCH-12)
        // - Multi-space RRF fusion when strategy=multi_space (ARCH-21)
        // - Weight profile application via resolve_weights()
        // - All 13 embedder scores computed for each result
        //
        // Blind spots and agreement metrics are derived from embedder_scores in response.
        match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(mut results) => {
                // =========================================================================
                // PHASE 2: ASYMMETRIC E5 RERANKING
                // =========================================================================
                // Apply asymmetric E5 similarity for causal queries
                // Per ARCH-15 and AP-77: E5 Causal MUST use asymmetric similarity

                let asymmetric_applied = if enable_asymmetric_e5
                    && causal_direction != CausalDirection::Unknown
                    && !results.is_empty()
                {
                    // Get E5 weight from the effective profile (default to 0.10 for causal_reasoning)
                    let e5_weight = get_e5_causal_weight(
                        effective_weight_profile.as_deref().unwrap_or("causal_reasoning")
                    );

                    info!(
                        results_count = results.len(),
                        causal_direction = %causal_direction,
                        e5_weight = e5_weight,
                        "Applying asymmetric E5 reranking"
                    );

                    apply_asymmetric_e5_reranking(
                        &mut results,
                        &query_embedding,
                        causal_direction,
                        e5_weight,
                    );
                    apply_direction_aware_reranking(&mut results, causal_direction);
                    true
                } else {
                    false
                };

                // =========================================================================
                // PHASE 4: COLBERT LATE INTERACTION RERANKING
                // =========================================================================
                // Apply ColBERT reranking if enabled (Stage 3 of pipeline)
                // This provides token-level precision for causal queries

                let colbert_applied = if enable_rerank && !results.is_empty() {
                    debug!(
                        results_count = results.len(),
                        "Applying ColBERT late interaction reranking"
                    );

                    apply_colbert_reranking(
                        &mut results,
                        &query_embedding,
                        top_k,
                    );
                    true
                } else {
                    false
                };

                // Truncate to requested top_k after reranking
                results.truncate(top_k);

                // CRIT-01 FIX: Wire record_access() into search results path.
                // This increments access_count and updates accessed_at for each
                // returned fingerprint, enabling memory decay (BM25 importance).
                // Non-blocking: failures logged but don't fail the search.
                for result in &mut results {
                    result.fingerprint.record_access();
                    if let Err(e) = self
                        .teleological_store
                        .update(result.fingerprint.clone())
                        .await
                    {
                        warn!(
                            error = %e,
                            memory_id = %result.fingerprint.id,
                            "search_graph: Failed to persist access count update"
                        );
                    }
                }

                // Collect IDs for batch operations
                let ids: Vec<uuid::Uuid> = results.iter().map(|r| r.fingerprint.id).collect();

                // TASK-CONTENT-003: Hydrate content if requested
                // Batch retrieve content for all results to minimize I/O
                // FAIL FAST: Return tool_error on retrieval failure — no silent degradation
                let contents: Vec<Option<String>> = if include_content && !results.is_empty() {
                    match self.teleological_store.get_content_batch(&ids).await {
                        Ok(c) => c,
                        Err(e) => {
                            error!(
                                error = %e,
                                result_count = results.len(),
                                "search_graph: Content hydration failed"
                            );
                            return self.tool_error_typed(
                                id,
                                ToolErrorKind::Storage,
                                &format!("Content retrieval failed for {} results: {}", results.len(), e),
                            );
                        }
                    }
                } else {
                    // Not requested or no results - empty vec
                    vec![]
                };

                // Batch retrieve source metadata for all results
                // Source metadata enables context injection to show file paths for MDFileChunk memories
                // FAIL FAST: Return tool_error on retrieval failure — no silent degradation
                let source_metadata: Vec<Option<context_graph_core::types::SourceMetadata>> = if !results.is_empty() {
                    match self.teleological_store.get_source_metadata_batch(&ids).await {
                        Ok(m) => m,
                        Err(e) => {
                            error!(
                                error = %e,
                                result_count = results.len(),
                                "search_graph: Source metadata retrieval failed"
                            );
                            return self.tool_error_typed(
                                id,
                                ToolErrorKind::Storage,
                                &format!("Source metadata retrieval failed for {} results: {}", results.len(), e),
                            );
                        }
                    }
                } else {
                    vec![]
                };

                // PHASE-2-PROVENANCE: Compute strategy name before the results loop
                // (needed both for provenance and for the response metadata)
                let strategy_name = match strategy {
                    SearchStrategy::E1Only => "e1_only",
                    SearchStrategy::MultiSpace => "multi_space",
                    SearchStrategy::Pipeline => "pipeline",
                };

                // P6: Resolve weight profile ONCE before per-result loop.
                // Was: custom_profiles.read() acquired per-embedder per-result (50*13*3 = ~1,950 locks/search).
                // Now: single lock acquisition, then direct array indexing.
                let resolved_weights: [f32; 13] = custom_weights
                    .or_else(|| effective_weight_profile.as_ref()
                        .and_then(|p| self.custom_profiles.read().get(p).copied()))
                    .or_else(|| effective_weight_profile.as_ref()
                        .and_then(|p| get_weight_profile(p)))
                    .unwrap_or([1.0 / 13.0; 13]);

                // PHASE-2-PROVENANCE: Query classification is not yet implemented.
                // Set to None until real classification (causal, code, entity, etc.) is wired.
                let query_analysis: Option<context_graph_core::retrieval::QueryClassification> = None;

                let results_json: Vec<_> = results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        // =================================================================
                        // 13-EMBEDDER VISIBILITY FOR AI NAVIGATION
                        // =================================================================
                        // Per Constitution v6.5: Give AI models FULL visibility into all
                        // 13 embedders so they can navigate massive datasets effectively.

                        let e1_score = r.embedder_scores[0];

                        // Blind spots: enhancers that found this but E1 missed
                        let blind_spots = compute_blind_spots(&r.embedder_scores, e1_score);

                        // Agreement count: how many embedders have score >= 0.5
                        let agreement_count = r.embedder_scores.iter()
                            .filter(|&&s| s >= 0.5)
                            .count();

                        // Full embedder scores categorized by type
                        let embedder_scores = build_embedder_scores_json(&r.embedder_scores);

                        // Navigation hints: suggest which embedders to explore next
                        let navigation_hints = compute_navigation_hints(&r.embedder_scores);

                        let mut entry = json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "similarity": r.similarity,
                            "e1Score": e1_score,
                            "embedderScores": embedder_scores,
                            "agreementCount": agreement_count
                        });

                        // Only include blindSpots if non-empty
                        if !blind_spots.is_empty() {
                            entry["blindSpots"] = json!(blind_spots);
                        }

                        // Only include navigationHints if non-empty
                        if !navigation_hints.is_empty() {
                            entry["navigationHints"] = json!(navigation_hints);
                        }
                        // Only include content field when includeContent=true
                        if include_content {
                            entry["content"] = match contents.get(i).and_then(|c| c.as_ref()) {
                                Some(c) => json!(c),
                                None => serde_json::Value::Null,
                            };
                        }
                        // Always include source metadata if available (enables context injection to show file paths)
                        if let Some(Some(ref metadata)) = source_metadata.get(i) {
                            entry["source"] = json!({
                                "type": format!("{}", metadata.source_type),
                                "file_path": metadata.file_path,
                                "chunk_index": metadata.chunk_index,
                                "total_chunks": metadata.total_chunks,
                                "hook_type": metadata.hook_type,
                                "tool_name": metadata.tool_name
                            });

                            // Include sequenceInfo for session-based queries (Phase 2 enhancement)
                            if let Some(seq) = metadata.session_sequence {
                                let current_seq = self.current_sequence();
                                let position_label = compute_position_label(seq, current_seq);
                                entry["sequenceInfo"] = json!({
                                    "sessionId": metadata.session_id,
                                    "sessionSequence": seq,
                                    "positionLabel": position_label
                                });
                            }
                        }

                        // =============================================================
                        // Gap 7: Per-result causal gate transparency
                        // =============================================================
                        // When asymmetric E5 was applied, show each result's gate details:
                        // e5Score, action (boost/demote/none), and score delta.
                        if asymmetric_applied {
                            let query_is_cause = matches!(causal_direction, CausalDirection::Cause);
                            let e5_sim = compute_e5_asymmetric_fingerprint_similarity(
                                &query_embedding,
                                &r.fingerprint.semantic,
                                query_is_cause,
                            );
                            let (action, score_delta) = if e5_sim >= causal_gate::CAUSAL_THRESHOLD {
                                ("boost", r.similarity - (r.similarity / causal_gate::CAUSAL_BOOST))
                            } else if e5_sim <= causal_gate::NON_CAUSAL_THRESHOLD {
                                ("demote", r.similarity - (r.similarity / causal_gate::NON_CAUSAL_DEMOTION))
                            } else {
                                ("none", 0.0)
                            };
                            entry["causalGate"] = json!({
                                "e5Score": e5_sim,
                                "action": action,
                                "scoreDelta": score_delta
                            });
                        }

                        // =============================================================
                        // GAP-6: Embedder breakdown when requested
                        // =============================================================
                        if include_embedder_breakdown {
                            let mut max_rrf: f32 = 0.0;
                            let mut dominant_idx: usize = 0;
                            let active_count = r.embedder_scores.iter()
                                .filter(|&&s| s > 0.0)
                                .count();

                            // P6: Use pre-resolved weights (no lock per-embedder)
                            let breakdown: Vec<serde_json::Value> = r.embedder_scores.iter()
                                .enumerate()
                                .filter(|(_, &score)| score > 0.0)
                                .map(|(idx, &score)| {
                                    let name = embedder_names::name(idx);
                                    let rank = r.embedder_scores.iter()
                                        .filter(|&&s| s > score)
                                        .count();
                                    let weight = resolved_weights[idx];
                                    let rrf_contribution = weight / (60.0 + rank as f32 + 1.0);
                                    if rrf_contribution > max_rrf {
                                        max_rrf = rrf_contribution;
                                        dominant_idx = idx;
                                    }
                                    json!({
                                        "embedder": name,
                                        "score": score,
                                        "rank": rank,
                                        "weight": weight,
                                        "rrfContribution": rrf_contribution
                                    })
                                })
                                .collect();
                            entry["embedderBreakdown"] = json!(breakdown);
                            entry["dominantEmbedder"] = json!(embedder_names::name(dominant_idx));
                            entry["agreementLevel"] = json!(match active_count {
                                0..=2 => "low",
                                3..=6 => "medium",
                                _ => "high",
                            });
                        }

                        // =============================================================
                        // PHASE-2-PROVENANCE: Add provenance when requested
                        // =============================================================
                        if include_provenance {
                            // P6: Reuse pre-resolved weights for provenance (was duplicate of breakdown)
                            let contributions: Vec<serde_json::Value> = r.embedder_scores.iter()
                                .enumerate()
                                .filter(|(_, &score)| score > 0.0)
                                .map(|(idx, &score)| {
                                    let name = embedder_names::name(idx);
                                    // Compute approximate rank: count embedders with higher score
                                    let approx_rank = r.embedder_scores.iter()
                                        .filter(|&&s| s > score)
                                        .count();
                                    // P6: Use pre-resolved weights (no lock per-embedder)
                                    let weight = resolved_weights[idx];
                                    // Compute RRF contribution (matches breakdown formula)
                                    let rrf_contrib = weight / (60.0 + approx_rank as f32 + 1.0);
                                    json!({
                                        "embedder": name,
                                        "similarity": score,
                                        "rank": approx_rank,
                                        "rrfContribution": rrf_contrib,
                                        "weight": weight
                                    })
                                })
                                .collect();

                            let mut provenance = json!({
                                "strategy": strategy_name,
                                "weightProfile": effective_weight_profile.as_deref().unwrap_or("default"),
                                "embedderContributions": contributions,
                                "consensusScore": agreement_count as f32 / 13.0,
                                "primaryEmbedder": embedder_names::name(r.dominant_embedder()),
                                "isBlindSpotDiscovery": !blind_spots.is_empty() && agreement_count <= 1
                            });

                            // Include query classification only if actually populated
                            if let Some(ref analysis) = query_analysis {
                                if !analysis.detected_type.is_empty() {
                                    provenance["queryClassification"] = json!({
                                        "detectedType": analysis.detected_type,
                                        "detectionPatterns": analysis.detection_patterns,
                                    });
                                }
                            }

                            entry["provenance"] = provenance;
                        }

                        entry
                    })
                    .collect();

                // Build response with causal metadata
                let mut response = json!({
                    "results": results_json,
                    "count": results_json.len(),
                    "searchStrategy": strategy_name
                });

                // Add causal search metadata for transparency and debugging
                response["causal"] = json!({
                    "direction": format!("{}", causal_direction),
                    "asymmetricE5Applied": asymmetric_applied,
                    "colbertApplied": colbert_applied,
                    "queryExpanded": search_query != query
                });

                // Add expanded query if query expansion was used
                if search_query != query {
                    response["causal"]["expandedQuery"] = json!(search_query);
                }

                // Add effective weight profile for debugging
                // When customWeights are provided, they override the profile (per constitution: customWeights > weightProfile)
                if custom_weights.is_some() {
                    response["effectiveProfile"] = json!("custom");
                } else if let Some(ref profile) = effective_weight_profile {
                    response["effectiveProfile"] = json!(profile);
                }

                // Echo back search parameters for transparency/debugging
                let temporal_config = if temporal_weight > 0.0 {
                    Some(json!({
                        "temporalWeight": temporal_weight,
                        "decayFunction": format!("{:?}", decay_function),
                        "decayHalfLifeSecs": decay_half_life,
                        "lastHours": last_hours,
                        "lastDays": last_days,
                    }))
                } else {
                    None
                };
                response["searchParameters"] = json!({
                    "customWeightsValues": custom_weights.map(|w| w.to_vec()),
                    "excludedEmbedders": exclude_embedder_names,
                    "temporalConfig": temporal_config,
                    "rrfConstant": 60.0,
                    "resolvedWeightProfile": effective_weight_profile,
                });

                // =========================================================================
                // SEARCH TRANSPARENCY: Show which embedders actually participated
                // =========================================================================
                // Per GAP-1: Make it transparent which of the 13 embedders
                // participated in RRF fusion vs. which weights were ignored.
                {
                    // P6: Reuse pre-resolved weights (single lock acquisition above)
                    let mut resolved_weights = resolved_weights;

                    // Apply exclude_embedders to resolved_weights (mirrors resolve_weights_sync)
                    if !exclude_embedder_names.is_empty() {
                        for name in &exclude_embedder_names {
                            let idx = match name.as_str() {
                                "E1" => 0, "E2" => 1, "E3" => 2, "E4" => 3, "E5" => 4,
                                "E6" => 5, "E7" => 6, "E8" => 7, "E9" => 8, "E10" => 9,
                                "E11" => 10, "E12" => 11, "E13" => 12, _ => continue,
                            };
                            resolved_weights[idx] = 0.0;
                        }
                        let sum: f32 = resolved_weights.iter().sum();
                        if sum > 0.0 {
                            for w in resolved_weights.iter_mut() {
                                *w /= sum;
                            }
                        }
                    }

                    // Active embedders depend on search strategy, filtered by exclusions
                    let (strategy_indices, strategy_label) = match strategy {
                        SearchStrategy::E1Only => (vec![0usize], "E1 HNSW only"),
                        SearchStrategy::MultiSpace => (vec![0, 4, 6, 7, 9, 10], "E1+E5+E7+E8+E10+E11 RRF fusion"),
                        SearchStrategy::Pipeline => (vec![0, 4, 6, 7, 9, 10], "E13+E1+E5+E7+E8+E11 recall → 6-embedder RRF scoring"),
                    };
                    let active_indices: Vec<usize> = strategy_indices.into_iter()
                        .filter(|idx| resolved_weights[*idx] > 0.0)
                        .collect();

                    let mut active_weights = serde_json::Map::new();
                    let mut ignored_weights = serde_json::Map::new();
                    let mut active_sum: f32 = 0.0;

                    for (idx, &w) in resolved_weights.iter().enumerate() {
                        let name = embedder_names::name(idx);
                        if active_indices.contains(&idx) {
                            active_weights.insert(name.to_string(), json!(w));
                            active_sum += w;
                        } else if w > 0.0 {
                            ignored_weights.insert(name.to_string(), json!(w));
                        }
                    }

                    response["searchTransparency"] = json!({
                        "activeEmbedders": active_weights,
                        "ignoredWeights": ignored_weights,
                        "activeEmbedderCount": active_weights.len(),
                        "totalEmbedderCount": 13,
                        "weightUtilization": active_sum,
                        "strategyDescription": strategy_label,
                    });
                }

                // Emit SearchPerformed audit (non-fatal)
                {
                    let result_ids: Vec<uuid::Uuid> = results.iter().map(|r| r.fingerprint.id).collect();
                    let audit_record = AuditRecord::new(
                        AuditOperation::SearchPerformed {
                            tool_name: "search_graph".to_string(),
                            results_returned: results.len(),
                            weight_profile: effective_weight_profile.clone(),
                            strategy: Some(format!("{:?}", strategy)),
                        },
                        result_ids.first().copied().unwrap_or(uuid::Uuid::nil()),
                    )
                    .with_operator("search_graph")
                    .with_parameters(json!({
                        "query_preview": query.chars().take(100).collect::<String>(),
                        "top_k": top_k,
                        "strategy": format!("{:?}", strategy),
                        "weight_profile": effective_weight_profile,
                    }));

                    if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                        error!(error = %e, "search_graph: Failed to write audit record (non-fatal)");
                    }
                }

                self.tool_result(id, response)
            }
            Err(e) => {
                error!(error = %e, "search_graph: Search FAILED");
                self.tool_error(id, &format!("Search failed: {}", e))
            }
        }
    }
}

// =============================================================================
// E5 CAUSAL HELPER FUNCTIONS
// =============================================================================

use context_graph_core::traits::TeleologicalSearchResult;

/// Get E5 (causal) weight from a weight profile.
///
/// # Arguments
/// * `profile_name` - Name of the weight profile
///
/// # Returns
/// E5 weight (index 4) from the profile, or 0.10 if profile not found
fn get_e5_causal_weight(profile_name: &str) -> f32 {
    get_weight_profile(profile_name)
        .map(|weights| weights[4]) // E5 is at index 4
        .unwrap_or(0.10) // Default to causal_reasoning E5 weight (demoted from 0.45)
}

/// Apply asymmetric E5 reranking to search results using binary causal gate.
///
/// After LoRA training, E5 scores are calibrated (0.05-0.58 range). Binary gate:
/// - E5 >= 0.04 (CAUSAL_THRESHOLD) → "definitely causal" → 1.10x boost
/// - E5 <= 0.008 (NON_CAUSAL_THRESHOLD) → "definitely non-causal" → 0.85x demotion
/// - E5 in (0.008, 0.04) → ambiguous dead zone → no change
///
/// This is Occam's razor: the simplest model that matches E5's actual signal.
///
/// # Arguments
/// * `results` - Mutable reference to search results to rerank
/// * `query_embedding` - Query's semantic fingerprint
/// * `query_direction` - Detected causal direction of the query
/// * `_e5_weight` - Kept for signature stability, not used in gate logic
fn apply_asymmetric_e5_reranking(
    results: &mut [TeleologicalSearchResult],
    query_embedding: &SemanticFingerprint,
    query_direction: CausalDirection,
    _e5_weight: f32,
) {
    if results.is_empty() {
        return;
    }
    let is_causal = !matches!(query_direction, CausalDirection::Unknown);

    for result in results.iter_mut() {
        let query_is_cause = matches!(query_direction, CausalDirection::Cause);
        let e5_sim = compute_e5_asymmetric_fingerprint_similarity(
            query_embedding,
            &result.fingerprint.semantic,
            query_is_cause,
        );
        result.similarity = apply_causal_gate(result.similarity, e5_sim, is_causal);
    }

    results.sort_by(|a, b| {
        b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Direction-aware reranking using keyword-detected query direction.
///
/// Uses infer_result_causal_direction() (E5 vector norm comparison) to determine
/// if a result describes a cause or effect, then boosts results whose direction
/// matches what the query seeks.
///
/// This is applied AFTER the binary causal gate to provide a secondary ranking signal.
fn apply_direction_aware_reranking(
    results: &mut [TeleologicalSearchResult],
    query_direction: CausalDirection,
) {
    if matches!(query_direction, CausalDirection::Unknown) || results.is_empty() {
        return;
    }

    const DIRECTION_MATCH_BOOST: f32 = 1.08;

    for result in results.iter_mut() {
        let result_dir = infer_result_causal_direction(&result.fingerprint.semantic);
        let boost = match (&query_direction, &result_dir) {
            (CausalDirection::Cause, CausalDirection::Cause) => DIRECTION_MATCH_BOOST,
            (CausalDirection::Effect, CausalDirection::Effect) => DIRECTION_MATCH_BOOST,
            _ => 1.0,
        };
        result.similarity *= boost;
        result.similarity = result.similarity.min(1.0);
    }

    results.sort_by(|a, b| {
        b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Infer a document's causal direction by analyzing its E5 embeddings.
///
/// Documents that describe causes tend to have stronger "as_cause" vectors,
/// while documents describing effects have stronger "as_effect" vectors.
///
/// # Arguments
/// * `fingerprint` - Document's semantic fingerprint
///
/// # Returns
/// Inferred causal direction of the document
fn infer_result_causal_direction(fingerprint: &SemanticFingerprint) -> CausalDirection {
    let cause_vec = fingerprint.get_e5_as_cause();
    let effect_vec = fingerprint.get_e5_as_effect();

    // Compare vector norms as a proxy for directional strength
    let cause_norm: f32 = cause_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    let effect_norm: f32 = effect_vec.iter().map(|x| x * x).sum::<f32>().sqrt();

    // Require >10% difference to be confident in direction
    let threshold = 0.1;
    let diff_ratio = if effect_norm > f32::EPSILON {
        (cause_norm - effect_norm) / effect_norm
    } else if cause_norm > f32::EPSILON {
        1.0 // All cause, no effect
    } else {
        0.0 // Both zero
    };

    if diff_ratio > threshold {
        CausalDirection::Cause
    } else if diff_ratio < -threshold {
        CausalDirection::Effect
    } else {
        CausalDirection::Unknown
    }
}

/// Expand a causal query with related terms for better recall.
///
/// # Arguments
/// * `query` - Original query text
/// * `direction` - Detected causal direction
///
/// # Returns
/// Expanded query with additional causal terms
fn expand_causal_query(query: &str, direction: CausalDirection) -> String {
    // Optimization: compute lowercase once to avoid multiple allocations
    let query_lower = query.to_lowercase();

    match direction {
        CausalDirection::Cause => {
            // Add cause-seeking terms (only if not already present)
            if !query_lower.contains("cause")
                && !query_lower.contains("reason")
                && !query_lower.contains("why")
            {
                format!("{} cause reason root source", query)
            } else {
                query.to_string()
            }
        }
        CausalDirection::Effect => {
            // Add effect-seeking terms (only if not already present)
            if !query_lower.contains("effect")
                && !query_lower.contains("result")
                && !query_lower.contains("happen")
            {
                format!("{} effect result consequence outcome", query)
            } else {
                query.to_string()
            }
        }
        CausalDirection::Unknown => query.to_string(),
    }
}

// =============================================================================
// PHASE 4: COLBERT LATE INTERACTION RERANKING
// =============================================================================

/// ColBERT MaxSim weight for blending with existing similarity.
/// Per research: 10-20% contribution provides precision boost without dominating.
const COLBERT_WEIGHT: f32 = 0.15;

// Import SIMD-optimized MaxSim from storage crate (TASK-STORAGE-P2-001)
use context_graph_storage::compute_maxsim_direct;

/// Apply ColBERT late interaction reranking to search results.
///
/// This function implements Phase 4 of the causal integration:
/// - Uses E12 token-level embeddings for precise semantic matching
/// - Computes MaxSim scores per document
/// - Blends with existing similarity scores
/// - Re-sorts results by combined score
///
/// # Arguments
/// * `results` - Mutable reference to search results to rerank
/// * `query_embedding` - Query's semantic fingerprint
/// * `top_k` - Maximum results to rerank (ColBERT is expensive)
///
/// # Note
/// ColBERT reranking is only applied when `enable_rerank=true` in the search options.
fn apply_colbert_reranking(
    results: &mut [TeleologicalSearchResult],
    query_embedding: &SemanticFingerprint,
    top_k: usize,
) {
    // Only rerank top-K candidates (ColBERT is computationally expensive)
    let rerank_count = results.len().min(top_k);

    if rerank_count == 0 {
        return;
    }

    // Get query ColBERT tokens (E12) - direct field access
    let query_tokens = &query_embedding.e12_late_interaction;
    if query_tokens.is_empty() {
        debug!("ColBERT reranking skipped: no query tokens");
        return;
    }

    let mut reranked = 0;

    for result in results.iter_mut().take(rerank_count) {
        // Get document ColBERT tokens - direct field access
        let doc_tokens = &result.fingerprint.semantic.e12_late_interaction;

        if doc_tokens.is_empty() {
            continue;
        }

        // Compute MaxSim score using SIMD-optimized implementation from storage crate
        let maxsim_score = compute_maxsim_direct(query_tokens, doc_tokens);

        // Blend ColBERT score with existing similarity
        // Formula: new_sim = (1 - colbert_weight) × old_sim + colbert_weight × maxsim
        result.similarity = result.similarity * (1.0 - COLBERT_WEIGHT)
            + maxsim_score * COLBERT_WEIGHT;

        reranked += 1;
    }

    // Re-sort after ColBERT reranking
    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    debug!(
        reranked = reranked,
        colbert_weight = COLBERT_WEIGHT,
        "ColBERT reranking applied"
    );
}

// =============================================================================
// INLINE CAUSAL EXTRACTION (LLM-dependent)
// =============================================================================

/// LLM-dependent inline causal extraction method.
/// Only available when `llm` feature is enabled.
#[cfg(feature = "llm")]
impl Handlers {
    /// Extract causal relationships inline during store_memory using the LLM.
    ///
    /// Generates E5 asymmetric embeddings for each discovered relationship and
    /// persists CausalRelationship records.
    async fn extract_inline_causal_relationships(
        &self,
        content: &str,
        fingerprint_id: uuid::Uuid,
    ) {
        if let Some(ref llm) = self.causal_discovery_llm {
            if llm.is_loaded() {
                debug!("store_memory: Extracting causal relationships inline via LLM");

                let extraction_result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    llm.extract_causal_relationships(content),
                )
                .await;

                match extraction_result {
                    Err(_elapsed) => {
                        warn!(
                            fingerprint_id = %fingerprint_id,
                            "store_memory: Causal relationship extraction timed out (30s)"
                        );
                    }
                    Ok(Err(e)) => {
                        warn!(
                            fingerprint_id = %fingerprint_id,
                            error = %e,
                            "store_memory: Causal relationship extraction FAILED"
                        );
                    }
                    Ok(Ok(multi_result)) if multi_result.relationships.is_empty() => {
                        debug!(
                            fingerprint_id = %fingerprint_id,
                            "store_memory: No causal relationships found in content"
                        );
                    }
                    Ok(Ok(multi_result)) => {
                        let rel_count = multi_result.relationships.len();
                        info!(
                            count = rel_count,
                            fingerprint_id = %fingerprint_id,
                            "store_memory: Found causal relationships, generating E5 embeddings"
                        );

                        if let Some(ref causal_model) = self.causal_model {
                            for relationship in &multi_result.relationships {
                                // Generate E5 asymmetric embeddings per AP-77:
                                // cause_vec = cause-encoding of cause text
                                // effect_vec = effect-encoding of effect text
                                let cause_result = causal_model
                                    .embed_dual(&relationship.cause)
                                    .await;
                                let effect_result = causal_model
                                    .embed_dual(&relationship.effect)
                                    .await;

                                let (e5_cause, e5_effect) = match (cause_result, effect_result) {
                                    (Ok((cause_vec, _)), Ok((_, effect_vec))) => {
                                        (cause_vec, effect_vec)
                                    }
                                    (Err(e), _) | (_, Err(e)) => {
                                        warn!(
                                            fingerprint_id = %fingerprint_id,
                                            error = %e,
                                            cause = %relationship.cause,
                                            "store_memory: E5 dual embedding failed for causal relationship"
                                        );
                                        continue;
                                    }
                                };

                                // Generate E1 semantic embedding for fallback search (1024D)
                                let e1_semantic = match self
                                    .multi_array_provider
                                    .embed_e1_only(&relationship.explanation)
                                    .await
                                {
                                    Ok(emb) => emb,
                                    Err(e) => {
                                        warn!(
                                            fingerprint_id = %fingerprint_id,
                                            error = %e,
                                            cause = %relationship.cause,
                                            "store_memory: E1 embedding failed for causal relationship"
                                        );
                                        continue;
                                    }
                                };

                                // Create CausalRelationship with proper asymmetric E5 vectors
                                let causal_rel = context_graph_core::types::CausalRelationship::new(
                                    relationship.cause.clone(),
                                    relationship.effect.clone(),
                                    relationship.explanation.clone(),
                                    e5_cause,
                                    e5_effect,
                                    e1_semantic,
                                    content.to_string(),
                                    fingerprint_id,
                                    relationship.confidence,
                                    relationship.mechanism_type.as_str().to_string(),
                                );

                                match self
                                    .teleological_store
                                    .store_causal_relationship(&causal_rel)
                                    .await
                                {
                                    Ok(causal_id) => {
                                        debug!(
                                            causal_id = %causal_id,
                                            source_id = %fingerprint_id,
                                            cause = %relationship.cause,
                                            effect = %relationship.effect,
                                            confidence = relationship.confidence,
                                            mechanism = %relationship.mechanism_type.as_str(),
                                            "store_memory: Persisted CausalRelationship"
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            fingerprint_id = %fingerprint_id,
                                            error = %e,
                                            cause = %relationship.cause,
                                            "store_memory: Failed to persist CausalRelationship"
                                        );
                                    }
                                }
                            }
                        } else {
                            warn!(
                                fingerprint_id = %fingerprint_id,
                                "store_memory: CausalModel not available for E5 embedding"
                            );
                        }
                    }
                }
            } else {
                warn!(
                    fingerprint_id = %fingerprint_id,
                    "store_memory: CausalDiscoveryLLM not loaded, cannot extract relationships"
                );
            }
        }
    }
}

// =============================================================================
// SEQUENCE POSITION LABEL HELPER
// =============================================================================

/// Compute human-readable position label for sequence numbers.
///
/// Returns labels like:
/// - "current turn" (same sequence)
/// - "previous turn" (1 turn ago)
/// - "2 turns ago" (2 turns ago)
/// - "N turns ago" (N turns ago)
/// - "future" (if result_seq > current_seq)
///
/// # Arguments
/// * `result_seq` - The session sequence of the result
/// * `current_seq` - The current session sequence
fn compute_position_label(result_seq: u64, current_seq: u64) -> String {
    if result_seq == current_seq {
        "current turn".to_string()
    } else if result_seq < current_seq {
        let turns_ago = current_seq - result_seq;
        if turns_ago == 1 {
            "previous turn".to_string()
        } else {
            format!("{} turns ago", turns_ago)
        }
    } else {
        // Future turn (shouldn't normally happen, but handle gracefully)
        let turns_ahead = result_seq - current_seq;
        if turns_ahead == 1 {
            "next turn".to_string()
        } else {
            format!("{} turns ahead", turns_ahead)
        }
    }
}

// =============================================================================
// 13-EMBEDDER VISIBILITY SYSTEM
// =============================================================================
// Per Constitution v6.5: AI models must have FULL visibility into all 13 embedders
// to navigate massive datasets from multiple angles. Each embedder is a unique
// perspective that finds what others miss.
//
// GOAL: Enable AI models to use all 13 embedders as navigation guides through
// massive datasets, understanding which perspectives found what and why.

/// Threshold for E1 "miss" - below this, E1 would have missed the result.
const E1_MISS_THRESHOLD: f32 = 0.3;

/// Threshold for enhancer "find" - above this, the enhancer found something useful.
const ENHANCER_FIND_THRESHOLD: f32 = 0.5;

/// Embedder metadata for AI visibility.
/// Each embedder has a specific signal it captures that E1 might miss.
const EMBEDDER_INFO: [(usize, &str, &str, &str); 13] = [
    // (index, name, category, what_it_finds)
    (0, "E1_Semantic", "FOUNDATION", "Dense semantic similarity - the foundation"),
    (1, "E2_Recency", "TEMPORAL", "Temporal freshness - recent memories (post-retrieval only)"),
    (2, "E3_Periodic", "TEMPORAL", "Time-of-day patterns - daily/weekly cycles (post-retrieval only)"),
    (3, "E4_Sequence", "TEMPORAL", "Conversation order - before/after relationships (post-retrieval only)"),
    (4, "E5_Causal", "SEMANTIC", "Causal chains - why X caused Y (direction preserved)"),
    (5, "E6_Sparse", "SEMANTIC", "Exact keyword matches - precise terminology E1 dilutes"),
    (6, "E7_Code", "SEMANTIC", "Code patterns - function signatures, syntax E1 treats as noise"),
    (7, "E8_Graph", "RELATIONAL", "Structural relationships - imports, dependencies"),
    (8, "E9_HDC", "STRUCTURAL", "Noise-robust structure - survives typos, variations"),
    (9, "E10_Multimodal", "SEMANTIC", "Paraphrase detection - same meaning expressed differently"),
    (10, "E11_Entity", "RELATIONAL", "Entity knowledge - 'Diesel' = database ORM for Rust"),
    (11, "E12_ColBERT", "SEMANTIC", "Exact phrase matches - token-level precision (reranking)"),
    (12, "E13_SPLADE", "SEMANTIC", "Term expansions - fast→quick, db→database (recall)"),
];

/// Compute blind spots: ALL enhancers that found this result but E1 missed.
///
/// A blind spot is when:
/// - An enhancer embedder has score >= 0.5 (found something)
/// - E1 (semantic) has score < 0.3 (would have missed it)
///
/// This tells the AI model: "This result would NOT have been found by E1 alone.
/// You're seeing it because E7/E10/E5/etc. found it."
///
/// Per ARCH-12: E1 is foundation, other embedders ENHANCE by finding blind spots.
/// Per Constitution v6.5: ALL enhancers are checked, not just a subset.
///
/// # Arguments
/// * `embedder_scores` - All 13 embedder scores [E1, E2, ..., E13]
/// * `e1_score` - E1 semantic score (passed separately for clarity)
///
/// # Returns
/// Vector of blind spot objects with name, score, and what the embedder finds
fn compute_blind_spots(embedder_scores: &[f32; 13], e1_score: f32) -> Vec<serde_json::Value> {
    let mut blind_spots = Vec::new();

    // Only check for blind spots if E1 would have missed this result
    if e1_score >= E1_MISS_THRESHOLD {
        return blind_spots;
    }

    // Check ALL non-foundation, non-temporal embedders
    // E2-E4 (temporal) are POST-RETRIEVAL only per ARCH-25, not for blind spot detection
    let enhancers = [
        (4, "E5_Causal", "causal chains"),
        (5, "E6_Sparse", "exact keywords"),
        (6, "E7_Code", "code patterns"),
        (7, "E8_Graph", "graph structure"),
        (8, "E9_HDC", "noise-robust matches"),
        (9, "E10_Paraphrase", "paraphrase detection"),
        (10, "E11_Entity", "entity knowledge"),
        (11, "E12_ColBERT", "phrase precision"),
        (12, "E13_SPLADE", "term expansion"),
    ];

    for (idx, name, finds) in enhancers {
        let score = embedder_scores[idx];
        if score >= ENHANCER_FIND_THRESHOLD {
            blind_spots.push(json!({
                "embedder": name,
                "score": score,
                "e1Score": e1_score,
                "finding": format!("{} found via {} but E1 missed", name, finds)
            }));
        }
    }

    blind_spots
}

/// Build FULL 13-embedder visibility JSON with all scores and metadata.
///
/// Per Constitution v6.5: AI models need FULL visibility into all 13 embedders
/// to navigate massive datasets. We include ALL scores (not just significant ones)
/// grouped by category with explanations of what each embedder finds.
///
/// # Arguments
/// * `embedder_scores` - All 13 embedder scores
///
/// # Returns
/// JSON object with categorized embedder scores and metadata
fn build_embedder_scores_json(embedder_scores: &[f32; 13]) -> serde_json::Value {
    let mut semantic = serde_json::Map::new();
    let mut relational = serde_json::Map::new();
    let mut structural = serde_json::Map::new();
    let mut temporal = serde_json::Map::new();

    for &(idx, name, category, _) in &EMBEDDER_INFO {
        let score = embedder_scores[idx];
        let entry = serde_json::Number::from_f64(score as f64)
            .unwrap_or_else(|| serde_json::Number::from(0));

        match category {
            "FOUNDATION" | "SEMANTIC" => {
                semantic.insert(name.to_string(), serde_json::Value::Number(entry));
            }
            "RELATIONAL" => {
                relational.insert(name.to_string(), serde_json::Value::Number(entry));
            }
            "STRUCTURAL" => {
                structural.insert(name.to_string(), serde_json::Value::Number(entry));
            }
            "TEMPORAL" => {
                temporal.insert(name.to_string(), serde_json::Value::Number(entry));
            }
            _ => {}
        }
    }

    json!({
        "semantic": semantic,
        "relational": relational,
        "structural": structural,
        "temporal": temporal
    })
}

/// Compute navigation suggestions based on embedder scores.
///
/// Per Constitution v6.5: Help AI models navigate massive datasets by suggesting
/// which embedders to explore based on current findings.
///
/// # Arguments
/// * `embedder_scores` - All 13 embedder scores
///
/// # Returns
/// Vector of navigation suggestions
fn compute_navigation_hints(embedder_scores: &[f32; 13]) -> Vec<String> {
    let mut hints = Vec::new();

    let e1 = embedder_scores[0];
    let e5 = embedder_scores[4];
    let e6 = embedder_scores[5];
    let e7 = embedder_scores[6];
    let e8 = embedder_scores[7];
    let e10 = embedder_scores[9];
    let e11 = embedder_scores[10];

    // Suggest based on what's strong vs weak
    if e7 > e1 + 0.2 {
        hints.push("E7 (code) found more than E1 - try search_code for code patterns".to_string());
    }
    if e11 > e1 + 0.2 {
        hints.push("E11 (entity) found more than E1 - try search_by_entities for relationships".to_string());
    }
    if e5 > e1 + 0.2 {
        hints.push("E5 (causal) found more than E1 - try search_causes for causal chains".to_string());
    }
    if e8 > 0.5 {
        hints.push("E8 (graph) is strong - try search_connections for imports/dependencies".to_string());
    }
    if e10 > e1 + 0.1 {
        hints.push("E10 (paraphrase) found similar purpose - results may use different words for same concept".to_string());
    }
    if e6 > 0.5 && e1 < 0.4 {
        hints.push("E6 (keyword) found exact terms E1 missed - try search_by_keywords".to_string());
    }

    hints
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
    fn rationale_validation_boundary_cases() {
        use crate::middleware::validation::validate_string_length;

        // Empty rationale should fail the actual validator
        assert!(validate_string_length("rationale", "", MIN_RATIONALE_LEN, MAX_RATIONALE_LEN).is_err());

        // Single char (minimum valid) should pass the actual validator
        assert!(validate_string_length("rationale", "x", MIN_RATIONALE_LEN, MAX_RATIONALE_LEN).is_ok());

        // Exactly 1024 chars (maximum valid) should pass the actual validator
        let max_valid = "x".repeat(MAX_RATIONALE_LEN);
        assert!(validate_string_length("rationale", &max_valid, MIN_RATIONALE_LEN, MAX_RATIONALE_LEN).is_ok());

        // 1025 chars should fail the actual validator
        let too_long = "x".repeat(MAX_RATIONALE_LEN + 1);
        assert!(validate_string_length("rationale", &too_long, MIN_RATIONALE_LEN, MAX_RATIONALE_LEN).is_err());
    }

    #[test]
    fn topk_validation_boundary_cases() {
        use crate::middleware::validation::validate_range;

        // topK = 0 should fail the actual validator
        assert!(validate_range("topK", 0_u64, MIN_TOP_K, MAX_TOP_K).is_err());

        // topK = 1 (minimum valid) should pass
        assert!(validate_range("topK", 1_u64, MIN_TOP_K, MAX_TOP_K).is_ok());

        // topK = 100 (maximum valid) should pass
        assert!(validate_range("topK", 100_u64, MIN_TOP_K, MAX_TOP_K).is_ok());

        // topK = 101 should fail
        assert!(validate_range("topK", 101_u64, MIN_TOP_K, MAX_TOP_K).is_err());

        // topK = 500 (original BUG-001 case) should fail
        assert!(validate_range("topK", 500_u64, MIN_TOP_K, MAX_TOP_K).is_err());
    }

    #[test]
    fn rationale_validation_error_fields() {
        use crate::middleware::validation::validate_string_length;

        // Verify the validator's error references the correct field
        let err = validate_string_length("rationale", "", MIN_RATIONALE_LEN, MAX_RATIONALE_LEN)
            .unwrap_err();
        assert_eq!(err.field_name(), "rationale");

        let long = "x".repeat(2000);
        let err = validate_string_length("rationale", &long, MIN_RATIONALE_LEN, MAX_RATIONALE_LEN)
            .unwrap_err();
        assert_eq!(err.field_name(), "rationale");
    }

    #[test]
    fn topk_validation_error_fields() {
        use crate::middleware::validation::validate_range;

        // Verify the validator's error references the correct field
        let err = validate_range("topK", 0_u64, MIN_TOP_K, MAX_TOP_K).unwrap_err();
        assert_eq!(err.field_name(), "topK");

        let err = validate_range("topK", 500_u64, MIN_TOP_K, MAX_TOP_K).unwrap_err();
        assert_eq!(err.field_name(), "topK");
    }

    // =========================================================================
    // E5 CAUSAL INTEGRATION TESTS
    // =========================================================================

    use super::{
        apply_causal_gate, expand_causal_query, get_e5_causal_weight,
        CausalDirection,
    };
    use context_graph_core::causal::asymmetric::causal_gate;

    #[test]
    fn test_causal_gate_boost_above_threshold() {
        // E5 score above CAUSAL_THRESHOLD (0.04) → 1.10x boost on causal query
        let result = apply_causal_gate(0.80, 0.05, true);
        let expected = 0.80 * causal_gate::CAUSAL_BOOST;
        assert!((result - expected).abs() < 1e-6, "got {result}, expected {expected}");
        println!("[VERIFIED] causal gate boost: 0.80 * 1.10 = {result}");
    }

    #[test]
    fn test_causal_gate_demotion_below_threshold() {
        // E5 score below NON_CAUSAL_THRESHOLD (0.008) → 0.85x demotion on causal query
        let result = apply_causal_gate(0.80, 0.005, true);
        let expected = 0.80 * causal_gate::NON_CAUSAL_DEMOTION;
        assert!((result - expected).abs() < 1e-6, "got {result}, expected {expected}");
        println!("[VERIFIED] causal gate demotion: 0.80 * 0.85 = {result}");
    }

    #[test]
    fn test_causal_gate_passthrough_non_causal_query() {
        // Non-causal query → no modification regardless of E5 score
        let result = apply_causal_gate(0.80, 0.99, false);
        assert!((result - 0.80).abs() < 1e-6, "non-causal should pass through, got {result}");
        println!("[VERIFIED] non-causal query passthrough: {result}");
    }

    #[test]
    fn test_causal_gate_dead_zone() {
        // E5 score between thresholds (0.008-0.04) → no modification
        let result = apply_causal_gate(0.80, 0.02, true);
        assert!((result - 0.80).abs() < 1e-6, "dead zone should pass through, got {result}");
        println!("[VERIFIED] causal gate dead zone: {result}");
    }

    #[test]
    fn test_get_e5_causal_weight_causal_reasoning() {
        // causal_reasoning profile has E5 = 0.10 (demoted from 0.45 — E5 is degenerate)
        let weight = get_e5_causal_weight("causal_reasoning");
        assert!((weight - 0.10).abs() < 0.01);
        println!("[VERIFIED] causal_reasoning E5 weight = {}", weight);
    }

    #[test]
    fn test_get_e5_causal_weight_semantic_search() {
        // semantic_search profile has E5 = 0.15
        let weight = get_e5_causal_weight("semantic_search");
        assert!((weight - 0.15).abs() < 0.01);
        println!("[VERIFIED] semantic_search E5 weight = {}", weight);
    }

    #[test]
    fn test_get_e5_causal_weight_unknown_profile() {
        // Unknown profile defaults to 0.10 (causal_reasoning default, demoted from 0.45)
        let weight = get_e5_causal_weight("nonexistent_profile");
        assert!((weight - 0.10).abs() < 0.01);
        println!("[VERIFIED] Unknown profile defaults to E5 weight = {}", weight);
    }

    #[test]
    fn test_expand_causal_query_cause_direction() {
        // Cause queries without existing cause terms should be expanded
        let expanded = expand_causal_query("what happened to the server", CausalDirection::Cause);
        assert!(expanded.contains("cause"));
        assert!(expanded.contains("reason"));
        assert!(expanded.contains("root"));
        println!("[VERIFIED] Cause query expanded: {}", expanded);
    }

    #[test]
    fn test_expand_causal_query_effect_direction() {
        // Effect queries without existing effect terms should be expanded
        let expanded = expand_causal_query("delete the file", CausalDirection::Effect);
        assert!(expanded.contains("effect"));
        assert!(expanded.contains("result"));
        assert!(expanded.contains("consequence"));
        println!("[VERIFIED] Effect query expanded: {}", expanded);
    }

    #[test]
    fn test_expand_causal_query_no_double_expansion() {
        // Queries already containing causal terms should not be expanded
        let original = "why does this cause the error";
        let expanded = expand_causal_query(original, CausalDirection::Cause);
        assert_eq!(expanded, original);
        println!("[VERIFIED] No double expansion for: {}", original);

        let original = "what is the effect of this change";
        let expanded = expand_causal_query(original, CausalDirection::Effect);
        assert_eq!(expanded, original);
        println!("[VERIFIED] No double expansion for: {}", original);
    }

    #[test]
    fn test_expand_causal_query_unknown_direction() {
        // Unknown direction should not expand
        let original = "show me the code";
        let expanded = expand_causal_query(original, CausalDirection::Unknown);
        assert_eq!(expanded, original);
        println!("[VERIFIED] Unknown direction not expanded");
    }

    #[test]
    fn test_direction_modifiers_asymmetric() {
        use context_graph_core::causal::asymmetric::direction_mod;
        // The asymmetry ratio should be 1.2 / 0.8 = 1.5
        let ratio = direction_mod::CAUSE_TO_EFFECT / direction_mod::EFFECT_TO_CAUSE;
        assert!((ratio - 1.5).abs() < 0.01);
        println!("[VERIFIED] Asymmetry ratio = {} (expected 1.5)", ratio);
    }

    #[test]
    fn test_constitution_compliance_direction_modifiers() {
        use context_graph_core::causal::asymmetric::direction_mod;
        // Verify all direction modifiers match Constitution spec
        assert_eq!(direction_mod::CAUSE_TO_EFFECT, 1.2, "Constitution: cause_to_effect must be 1.2");
        assert_eq!(direction_mod::EFFECT_TO_CAUSE, 0.8, "Constitution: effect_to_cause must be 0.8");
        assert_eq!(direction_mod::SAME_DIRECTION, 1.0, "Constitution: same_direction must be 1.0");
        println!("[VERIFIED] All direction modifiers match Constitution specification");
    }

    // =========================================================================
    // COLBERT MAXSIM TESTS
    // =========================================================================

    use super::COLBERT_WEIGHT;
    // Import SIMD-optimized MaxSim from storage crate
    use context_graph_storage::compute_maxsim_direct;

    #[test]
    fn test_colbert_maxsim_identical_tokens() {
        // Identical query and doc should give score of 1.0
        let query_tokens = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
        ];
        let doc_tokens = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
        ];

        let score = compute_maxsim_direct(&query_tokens, &doc_tokens);
        assert!((score - 1.0).abs() < 0.01, "Identical tokens should give ~1.0, got {}", score);
        println!("[VERIFIED] ColBERT MaxSim with identical tokens = {}", score);
    }

    #[test]
    fn test_colbert_maxsim_orthogonal_tokens() {
        // Orthogonal query and doc tokens should give score of 0.0
        let query_tokens = vec![
            vec![1.0, 0.0, 0.0],
        ];
        let doc_tokens = vec![
            vec![0.0, 1.0, 0.0],
        ];

        let score = compute_maxsim_direct(&query_tokens, &doc_tokens);
        assert!(score.abs() < 0.01, "Orthogonal tokens should give ~0.0, got {}", score);
        println!("[VERIFIED] ColBERT MaxSim with orthogonal tokens = {}", score);
    }

    #[test]
    fn test_colbert_maxsim_partial_match() {
        // Mix of matching and non-matching tokens
        let query_tokens = vec![
            vec![1.0, 0.0, 0.0],  // Matches first doc token
            vec![0.0, 0.0, 1.0],  // Doesn't match any doc token
        ];
        let doc_tokens = vec![
            vec![1.0, 0.0, 0.0],  // Matches first query token
            vec![0.0, 1.0, 0.0],  // Doesn't match any query token
        ];

        let score = compute_maxsim_direct(&query_tokens, &doc_tokens);
        // Expected: (1.0 + 0.0) / 2 = 0.5
        assert!((score - 0.5).abs() < 0.01, "Partial match should give ~0.5, got {}", score);
        println!("[VERIFIED] ColBERT MaxSim with partial match = {}", score);
    }

    #[test]
    fn test_colbert_maxsim_empty_inputs() {
        // Empty inputs should return 0.0
        let score_empty_query = compute_maxsim_direct(&[], &[vec![1.0, 0.0]]);
        let score_empty_doc = compute_maxsim_direct(&[vec![1.0, 0.0]], &[]);
        let score_both_empty = compute_maxsim_direct(&[], &[]);

        assert_eq!(score_empty_query, 0.0);
        assert_eq!(score_empty_doc, 0.0);
        assert_eq!(score_both_empty, 0.0);
        println!("[VERIFIED] Empty inputs give 0.0");
    }

    #[test]
    fn test_colbert_weight_range() {
        // ColBERT weight should be in reasonable range (10-20%)
        assert!(COLBERT_WEIGHT >= 0.1, "ColBERT weight should be >= 0.1");
        assert!(COLBERT_WEIGHT <= 0.2, "ColBERT weight should be <= 0.2");
        println!("[VERIFIED] ColBERT weight = {} is in [0.1, 0.2]", COLBERT_WEIGHT);
    }

    #[test]
    fn test_colbert_maxsim_normalization() {
        // Verify score is normalized to [0, 1]
        let query_tokens = vec![
            vec![1.0, 1.0, 1.0],
            vec![2.0, 2.0, 2.0],
            vec![3.0, 3.0, 3.0],
        ];
        let doc_tokens = vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ];

        let score = compute_maxsim_direct(&query_tokens, &doc_tokens);
        assert!(score >= 0.0, "Score should be >= 0.0");
        assert!(score <= 1.0, "Score should be <= 1.0");
        println!("[VERIFIED] ColBERT MaxSim normalized to [0, 1]: {}", score);
    }

    // =========================================================================
    // BLIND SPOT DETECTION TESTS
    // =========================================================================

    use super::{compute_blind_spots, build_embedder_scores_json, compute_navigation_hints, E1_MISS_THRESHOLD, ENHANCER_FIND_THRESHOLD};

    #[test]
    fn test_blind_spot_detection_e7_found_e1_missed() {
        // E7 (Code) found with 0.8, E1 missed with 0.2
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.2;  // E1 below threshold (0.3)
        scores[6] = 0.8;  // E7 above threshold (0.5)

        let blind_spots = compute_blind_spots(&scores, scores[0]);
        assert_eq!(blind_spots.len(), 1, "Should detect one blind spot");

        // Verify the blind spot structure
        let spot = &blind_spots[0];
        assert_eq!(spot["embedder"], "E7_Code");
        let score = spot["score"].as_f64().unwrap();
        assert!((score - 0.8).abs() < 0.001, "Score should be ~0.8, got {}", score);
        println!("[VERIFIED] Blind spot detected: {:?}", spot);
    }

    #[test]
    fn test_blind_spot_detection_multiple_enhancers() {
        // Multiple enhancers found, E1 missed
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.15;  // E1 low
        scores[4] = 0.6;   // E5 Causal found
        scores[6] = 0.75;  // E7 Code found
        scores[9] = 0.55;  // E10 Paraphrase found

        let blind_spots = compute_blind_spots(&scores, scores[0]);
        assert_eq!(blind_spots.len(), 3, "Should detect three blind spots");
        println!("[VERIFIED] Multiple blind spots detected");
    }

    #[test]
    fn test_no_blind_spot_when_e1_found() {
        // E1 found the result (>= 0.3), so no blind spots
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.5;  // E1 found it
        scores[6] = 0.9;  // E7 also found it

        let blind_spots = compute_blind_spots(&scores, scores[0]);
        assert!(blind_spots.is_empty(), "No blind spots when E1 >= 0.3");
        println!("[VERIFIED] No blind spots when E1 found the result");
    }

    #[test]
    fn test_no_blind_spot_when_enhancers_low() {
        // E1 missed but enhancers also low - not a true blind spot
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.1;  // E1 missed
        scores[6] = 0.4;  // E7 also low (< 0.5)

        let blind_spots = compute_blind_spots(&scores, scores[0]);
        assert!(blind_spots.is_empty(), "No blind spots when enhancers also low");
        println!("[VERIFIED] No false positives when enhancers are low");
    }

    #[test]
    fn test_blind_spot_thresholds() {
        // Verify thresholds match constitution expectations
        assert!((E1_MISS_THRESHOLD - 0.3).abs() < 0.001, "E1 miss threshold should be 0.3");
        assert!((ENHANCER_FIND_THRESHOLD - 0.5).abs() < 0.001, "Enhancer find threshold should be 0.5");
        println!("[VERIFIED] Thresholds: E1_miss={}, Enhancer_find={}", E1_MISS_THRESHOLD, ENHANCER_FIND_THRESHOLD);
    }

    #[test]
    fn test_build_embedder_scores_categorized_structure() {
        // Build JSON with mix of scores - now categorized by type
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.75;  // E1 Semantic (FOUNDATION/SEMANTIC category)
        scores[6] = 0.5;   // E7 Code (SEMANTIC category)
        scores[10] = 0.6;  // E11 Entity (RELATIONAL category)
        scores[1] = 0.3;   // E2 Recency (TEMPORAL category)

        let json = build_embedder_scores_json(&scores);

        // Verify categorized structure
        assert!(json["semantic"].is_object(), "Should have semantic category");
        assert!(json["relational"].is_object(), "Should have relational category");
        assert!(json["temporal"].is_object(), "Should have temporal category");
        assert!(json["structural"].is_object(), "Should have structural category");

        // Check semantic embedders are in right category
        let semantic = json["semantic"].as_object().unwrap();
        assert!(semantic.contains_key("E1_Semantic"));
        assert!(semantic.contains_key("E7_Code"));

        // Check relational embedders
        let relational = json["relational"].as_object().unwrap();
        assert!(relational.contains_key("E11_Entity"));

        // Check temporal embedders
        let temporal = json["temporal"].as_object().unwrap();
        assert!(temporal.contains_key("E2_Recency"));

        println!("[VERIFIED] Embedder scores correctly categorized");
    }

    #[test]
    fn test_build_embedder_scores_all_13_included() {
        // Verify all 13 embedders are included in the categorized structure
        let scores = [0.15_f32; 13];

        let json = build_embedder_scores_json(&scores);

        // Count total embedders across all categories
        let semantic_count = json["semantic"].as_object().unwrap().len();
        let relational_count = json["relational"].as_object().unwrap().len();
        let structural_count = json["structural"].as_object().unwrap().len();
        let temporal_count = json["temporal"].as_object().unwrap().len();

        let total = semantic_count + relational_count + structural_count + temporal_count;
        assert_eq!(total, 13, "Should have all 13 embedders");
        println!("[VERIFIED] All 13 embedders included: semantic={}, relational={}, structural={}, temporal={}",
            semantic_count, relational_count, structural_count, temporal_count);
    }

    #[test]
    fn test_navigation_hints_e7_stronger_than_e1() {
        // When E7 (code) finds more than E1, suggest search_code
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.3;  // E1 low
        scores[6] = 0.7;  // E7 high (0.4 more than E1)

        let hints = compute_navigation_hints(&scores);
        assert!(!hints.is_empty(), "Should have navigation hints");
        assert!(hints.iter().any(|h| h.contains("search_code")), "Should suggest search_code");
        println!("[VERIFIED] Navigation hints: {:?}", hints);
    }

    #[test]
    fn test_navigation_hints_multiple_suggestions() {
        // Multiple embedders stronger than E1
        let mut scores = [0.0_f32; 13];
        scores[0] = 0.2;   // E1 low
        scores[6] = 0.5;   // E7 code strong
        scores[10] = 0.5;  // E11 entity strong
        scores[4] = 0.5;   // E5 causal strong

        let hints = compute_navigation_hints(&scores);
        assert!(hints.len() >= 2, "Should have multiple navigation hints");
        println!("[VERIFIED] Multiple hints: {:?}", hints);
    }
}
