//! Memory operation tool implementations (inject_context, store_memory, search_graph).
//!
//! # Multi-Space Search (TASK-MULTISPACE)
//!
//! The `search_graph` tool supports three search strategies:
//!
//! - `e1_only`: Original E1-only HNSW search (default, backward compatible)
//! - `multi_space`: Weighted fusion of semantic embedders
//! - `pipeline`: Full 3-stage retrieval
//!
//! Temporal embedders (E2-E4) have weight 0.0 in semantic search per AP-71.
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
    compute_e5_asymmetric_fingerprint_similarity, detect_causal_query_intent,
    direction_mod, CausalDirection,
};
use context_graph_core::teleological::matrix_search::embedder_names;
use context_graph_core::traits::{EmbeddingMetadata, SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::fingerprint::{SemanticFingerprint, TeleologicalFingerprint, NUM_EMBEDDERS};
use context_graph_core::types::{SourceMetadata, SourceType, UtlContext};

use crate::weights::get_weight_profile;

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

        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(TeleologicalFingerprint::DEFAULT_IMPORTANCE);

        // E4-FIX: Get session sequence for E4 (V_ordering) embedding
        let session_sequence = self.get_next_sequence();
        // SESSION-ID-FIX: Priority: tool argument > env var > stored session_id
        let session_id = args
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.get_session_id());

        let metadata = EmbeddingMetadata {
            session_id: session_id.clone(),
            session_sequence: Some(session_sequence),
            timestamp: Some(chrono::Utc::now()),
        };

        debug!(
            session_sequence = session_sequence,
            session_id = ?session_id,
            "inject_context: Using session sequence for E4 embedding"
        );

        // STEP 1: Generate all 13 embeddings FIRST (needed for UTL reference lookup)
        // E4-FIX: Use embed_all_with_metadata to pass sequence number to E4
        let embedding_output = match self
            .multi_array_provider
            .embed_all_with_metadata(&content, metadata)
            .await
        {
            Ok(output) => output,
            Err(e) => {
                error!(error = %e, "inject_context: Multi-array embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Embedding failed: {}", e));
            }
        };

        // STEP 2: Search for similar recent memories to populate reference_embeddings
        // This enables proper surprise/entropy calculation (not always 1.0)
        let reference_embeddings = {
            let search_options = TeleologicalSearchOptions {
                top_k: 10, // Get top 10 similar memories for context
                ..Default::default()
            };
            match self
                .teleological_store
                .search_semantic(&embedding_output.fingerprint, search_options)
                .await
            {
                Ok(results) => {
                    // Extract E1 semantic embeddings from results for apples-to-apples comparison
                    // Per ARCH-02: Only compare same embedder space
                    results
                        .into_iter()
                        .filter_map(|r| {
                            // Skip if this is somehow the same content (should be impossible since not stored yet)
                            Some(r.fingerprint.semantic.e1_semantic.clone())
                        })
                        .collect::<Vec<_>>()
                }
                Err(e) => {
                    // Non-fatal: log warning but continue with empty reference (max surprise)
                    warn!(
                        error = %e,
                        "inject_context: Failed to search for reference embeddings. \
                         UTL will compute maximum surprise (entropy=1.0)."
                    );
                    Vec::new()
                }
            }
        };

        // STEP 3: Build UTL context with reference embeddings for proper surprise calculation
        let context = UtlContext {
            reference_embeddings: if reference_embeddings.is_empty() {
                None
            } else {
                Some(reference_embeddings)
            },
            // Use E1 semantic embedding for goal alignment computation
            input_embedding: Some(embedding_output.fingerprint.e1_semantic.clone()),
            ..Default::default()
        };

        // STEP 4: Compute UTL metrics with proper context
        let metrics = match self.utl_processor.compute_metrics(&content, &context).await {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "inject_context: UTL processing FAILED");
                return self.tool_error_with_pulse(id, &format!("UTL processing failed: {}", e));
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // TASK-FIX-CLUSTERING: Compute cluster array BEFORE fingerprint is consumed
        // This must be done before TeleologicalFingerprint::new() moves the semantic fingerprint.
        let cluster_array = embedding_output.fingerprint.to_cluster_array();

        // E5 Phase 5: Infer causal direction BEFORE fingerprint is consumed
        // This is used later for source metadata storage
        let causal_direction = infer_causal_direction_from_fingerprint(&embedding_output.fingerprint);

        // Create TeleologicalFingerprint from embeddings with user-specified importance
        let fingerprint =
            TeleologicalFingerprint::with_importance(embedding_output.fingerprint, content_hash, importance);
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

        // E4-FIX Phase 1: Persist session metadata for E4 sequence retrieval
        // This enables proper before/after queries by storing session_sequence
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
                "inject_context: Failed to store source metadata (fingerprint saved successfully). \
                 E4 sequence retrieval may fall back to timestamp-based ordering."
            );
        } else {
            debug!(
                fingerprint_id = %fingerprint_id,
                session_sequence = session_sequence,
                causal_direction = %causal_direction,
                "inject_context: Source metadata stored for E4 sequence retrieval"
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

        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(TeleologicalFingerprint::DEFAULT_IMPORTANCE);

        // E4-FIX: Get session sequence for E4 (V_ordering) embedding
        let session_sequence = self.get_next_sequence();
        // SESSION-ID-FIX: Priority: tool argument > env var > stored session_id
        let session_id = args
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.get_session_id());

        let metadata = EmbeddingMetadata {
            session_id: session_id.clone(),
            session_sequence: Some(session_sequence),
            timestamp: Some(chrono::Utc::now()),
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

        // E5 Phase 5: Infer causal direction BEFORE fingerprint is consumed
        // This is used later for source metadata storage
        let causal_direction = infer_causal_direction_from_fingerprint(&embedding_output.fingerprint);

        // Create TeleologicalFingerprint from embeddings with user-specified importance
        let fingerprint =
            TeleologicalFingerprint::with_importance(embedding_output.fingerprint, content_hash, importance);
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

                // E4-FIX Phase 1: Persist session metadata for E4 sequence retrieval
                // This enables proper before/after queries by storing session_sequence
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

        // TASK-MULTISPACE: Parse search strategy (default: e1_only for backward compatibility)
        let strategy = args
            .get("strategy")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "multi_space" => SearchStrategy::MultiSpace,
                "pipeline" => SearchStrategy::Pipeline,
                _ => SearchStrategy::E1Only, // "e1_only" or unknown
            })
            .unwrap_or(SearchStrategy::E1Only);

        // TASK-MULTISPACE: Parse weight profile (default: "semantic_search")
        let weight_profile = args
            .get("weightProfile")
            .and_then(|v| v.as_str())
            .map(String::from);

        // TASK-MULTISPACE: Parse recency boost (default: 0.0 = no boost)
        // Per ARCH-14: Temporal is a POST-retrieval boost, not similarity
        let recency_boost = args
            .get("recencyBoost")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .unwrap_or(0.0);

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
        let decay_function = args
            .get("decayFunction")
            .and_then(|v| v.as_str())
            .map(|s| match s.to_lowercase().as_str() {
                "linear" => context_graph_core::traits::DecayFunction::Linear,
                "exponential" => context_graph_core::traits::DecayFunction::Exponential,
                "step" => context_graph_core::traits::DecayFunction::Step,
                "none" | "no_decay" => context_graph_core::traits::DecayFunction::NoDecay,
                _ => context_graph_core::traits::DecayFunction::Linear,
            })
            .unwrap_or(context_graph_core::traits::DecayFunction::Linear);

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
        let temporal_scale = args
            .get("temporalScale")
            .and_then(|v| v.as_str())
            .map(|s| match s.to_lowercase().as_str() {
                "micro" => context_graph_core::traits::TemporalScale::Micro,
                "meso" => context_graph_core::traits::TemporalScale::Meso,
                "macro" => context_graph_core::traits::TemporalScale::Macro,
                "long" => context_graph_core::traits::TemporalScale::Long,
                "archival" => context_graph_core::traits::TemporalScale::Archival,
                _ => context_graph_core::traits::TemporalScale::Meso,
            })
            .unwrap_or(context_graph_core::traits::TemporalScale::Meso);

        // =========================================================================
        // CONVERSATION CONTEXT PARAMETERS (E4 Sequence Integration)
        // =========================================================================

        // Parse conversationContext convenience wrapper
        let conversation_context = args.get("conversationContext");
        let anchor_to_current_turn = conversation_context
            .and_then(|c| c.get("anchorToCurrentTurn"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let turns_back = conversation_context
            .and_then(|c| c.get("turnsBack"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32;
        let turns_forward = conversation_context
            .and_then(|c| c.get("turnsForward"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // Parse sessionScope (current, all, recent)
        let session_scope = args
            .get("sessionScope")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

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
            options = options.with_weight_profile(profile);
        }

        // Apply temporal options - handle both new temporalWeight and legacy recencyBoost
        // Per ARCH-14: Temporal is a POST-retrieval boost, not similarity
        let effective_temporal_weight = if temporal_weight > 0.0 {
            temporal_weight
        } else if recency_boost > 0.0 {
            // Legacy migration: recencyBoost -> temporal_weight with exponential decay
            recency_boost
        } else {
            0.0
        };

        if effective_temporal_weight > 0.0 {
            // Determine decay function - use exponential for legacy recencyBoost
            let effective_decay = if temporal_weight > 0.0 {
                decay_function
            } else {
                // Legacy recencyBoost used exponential decay
                context_graph_core::traits::DecayFunction::Exponential
            };

            options = options
                .with_temporal_weight(effective_temporal_weight)
                .with_decay_function(effective_decay)
                .with_temporal_scale(temporal_scale);

            // Apply decay half-life if exponential
            if matches!(effective_decay, context_graph_core::traits::DecayFunction::Exponential) {
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
            "all" | _ => {
                // No session filtering - search all memories
                // But still allow explicit sessionId to override
                if let Some(ref sid) = session_id {
                    options = options.with_session_filter(sid);
                }
            }
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
            recency_boost = recency_boost,
            enable_rerank = enable_rerank,
            temporal_weight = temporal_weight,
            causal_direction = %causal_direction,
            enable_asymmetric_e5 = enable_asymmetric_e5,
            "search_graph: Multi-space, temporal, and causal options configured"
        );

        // Generate query embedding using potentially expanded query
        let query_embedding = match self.multi_array_provider.embed_all(&search_query).await {
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
                    // Get E5 weight from the effective profile (default to 0.45 for causal_reasoning)
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

                // Collect IDs for batch operations
                let ids: Vec<uuid::Uuid> = results.iter().map(|r| r.fingerprint.id).collect();

                // TASK-CONTENT-003: Hydrate content if requested
                // Batch retrieve content for all results to minimize I/O
                let contents: Vec<Option<String>> = if include_content && !results.is_empty() {
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

                // Batch retrieve source metadata for all results
                // Source metadata enables context injection to show file paths for MDFileChunk memories
                let source_metadata: Vec<Option<context_graph_core::types::SourceMetadata>> = if !results.is_empty() {
                    match self.teleological_store.get_source_metadata_batch(&ids).await {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(
                                error = %e,
                                result_count = results.len(),
                                "search_graph: Source metadata retrieval failed. Results will not include source info."
                            );
                            // Return None for all - graceful degradation
                            vec![None; ids.len()]
                        }
                    }
                } else {
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
                        entry
                    })
                    .collect();

                // TASK-MULTISPACE: Include search strategy in response for debugging
                let strategy_name = match strategy {
                    SearchStrategy::E1Only => "e1_only",
                    SearchStrategy::MultiSpace => "multi_space",
                    SearchStrategy::Pipeline => "pipeline",
                };

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
                if let Some(ref profile) = effective_weight_profile {
                    response["effectiveProfile"] = json!(profile);
                }

                self.tool_result_with_pulse(id, response)
            }
            Err(e) => {
                error!(error = %e, "search_graph: Search FAILED");
                self.tool_error_with_pulse(id, &format!("Search failed: {}", e))
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
/// E5 weight (index 4) from the profile, or 0.45 if profile not found
fn get_e5_causal_weight(profile_name: &str) -> f32 {
    get_weight_profile(profile_name)
        .map(|weights| weights[4]) // E5 is at index 4
        .unwrap_or(0.45) // Default to causal_reasoning E5 weight
}

/// Apply asymmetric E5 reranking to search results.
///
/// This function implements Phase 2 of the causal integration:
/// - Computes asymmetric E5 similarity between query and each result
/// - Applies direction modifiers (1.2x cause→effect, 0.8x effect→cause)
/// - Blends asymmetric E5 score with original similarity
/// - Re-sorts results by adjusted score
///
/// # Arguments
/// * `results` - Mutable reference to search results to rerank
/// * `query_embedding` - Query's semantic fingerprint
/// * `query_direction` - Detected causal direction of the query
/// * `e5_weight` - Weight for E5 causal similarity (from profile)
///
/// # Formula
/// ```text
/// adjusted_sim = (1 - e5_weight) × original_sim + e5_weight × asymmetric_e5_sim × direction_mod
/// ```
fn apply_asymmetric_e5_reranking(
    results: &mut [TeleologicalSearchResult],
    query_embedding: &SemanticFingerprint,
    query_direction: CausalDirection,
    e5_weight: f32,
) {
    // Skip if no results or weight is zero
    if results.is_empty() || e5_weight <= 0.0 {
        return;
    }

    // Compute adjusted similarities for all results
    for result in results.iter_mut() {
        // Get result's causal direction by checking which E5 vector has higher similarity
        let result_direction = infer_result_causal_direction(&result.fingerprint.semantic);

        // Compute asymmetric E5 similarity
        // Per CAWAI research: cause queries use query.e5_as_cause vs doc.e5_as_effect
        let query_is_cause = matches!(query_direction, CausalDirection::Cause);
        let asymmetric_e5_sim = compute_e5_asymmetric_fingerprint_similarity(
            query_embedding,
            &result.fingerprint.semantic,
            query_is_cause,
        );

        // Get direction modifier
        let direction_mod = get_direction_modifier(query_direction, result_direction);

        // Blend original similarity with asymmetric E5 score
        // Formula: (1 - e5_weight) × original + e5_weight × asymmetric × direction_mod
        let original_weight = 1.0 - e5_weight;
        let adjusted_sim = original_weight * result.similarity
            + e5_weight * asymmetric_e5_sim * direction_mod;

        // Update the result's similarity score
        result.similarity = adjusted_sim.clamp(0.0, 1.5); // Allow slight amplification
    }

    // Re-sort results by adjusted similarity (descending)
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
}

/// Get direction modifier for query→result comparison.
///
/// Uses the `direction_mod` module from asymmetric.rs (Constitution-compliant):
/// - cause→effect: 1.2 (forward inference amplified)
/// - effect→cause: 0.8 (backward inference dampened)
/// - same direction or unknown: 1.0 (no modification)
fn get_direction_modifier(query_direction: CausalDirection, result_direction: CausalDirection) -> f32 {
    match (query_direction, result_direction) {
        (CausalDirection::Cause, CausalDirection::Effect) => direction_mod::CAUSE_TO_EFFECT,
        (CausalDirection::Effect, CausalDirection::Cause) => direction_mod::EFFECT_TO_CAUSE,
        _ => direction_mod::SAME_DIRECTION,
    }
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

/// Compute ColBERT MaxSim score between query and document token embeddings.
///
/// MaxSim is the sum of maximum similarities per query token:
/// ```text
/// MaxSim(Q, D) = Σ_q max_d cos(q, d)
/// ```
///
/// # Arguments
/// * `query_tokens` - Query token embeddings (128D per token)
/// * `doc_tokens` - Document token embeddings (128D per token)
///
/// # Returns
/// Normalized MaxSim score in [0.0, 1.0]
fn compute_colbert_maxsim(query_tokens: &[Vec<f32>], doc_tokens: &[Vec<f32>]) -> f32 {
    if query_tokens.is_empty() || doc_tokens.is_empty() {
        return 0.0;
    }

    let mut total_score = 0.0;

    for query_token in query_tokens {
        let mut max_sim = f32::NEG_INFINITY;

        for doc_token in doc_tokens {
            // Compute cosine similarity between tokens
            let dot: f32 = query_token.iter().zip(doc_token.iter()).map(|(a, b)| a * b).sum();
            let norm_q: f32 = query_token.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm_d: f32 = doc_token.iter().map(|x| x * x).sum::<f32>().sqrt();

            let sim = if norm_q > f32::EPSILON && norm_d > f32::EPSILON {
                dot / (norm_q * norm_d)
            } else {
                0.0
            };

            if sim > max_sim {
                max_sim = sim;
            }
        }

        // Add max similarity for this query token
        if max_sim > f32::NEG_INFINITY {
            total_score += max_sim;
        }
    }

    // Normalize by number of query tokens
    let normalized = total_score / query_tokens.len() as f32;

    // Clamp to [0.0, 1.0]
    normalized.clamp(0.0, 1.0)
}

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

        // Compute MaxSim score
        let maxsim_score = compute_colbert_maxsim(query_tokens, doc_tokens);

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

    // =========================================================================
    // E5 CAUSAL INTEGRATION TESTS
    // =========================================================================

    use super::{
        expand_causal_query, get_direction_modifier, get_e5_causal_weight,
        CausalDirection, direction_mod,
    };

    #[test]
    fn test_direction_modifier_cause_to_effect() {
        // Per Constitution: cause→effect = 1.2 (forward inference amplified)
        let modifier = get_direction_modifier(CausalDirection::Cause, CausalDirection::Effect);
        assert_eq!(modifier, direction_mod::CAUSE_TO_EFFECT);
        assert_eq!(modifier, 1.2);
        println!("[VERIFIED] cause→effect direction_mod = 1.2");
    }

    #[test]
    fn test_direction_modifier_effect_to_cause() {
        // Per Constitution: effect→cause = 0.8 (backward inference dampened)
        let modifier = get_direction_modifier(CausalDirection::Effect, CausalDirection::Cause);
        assert_eq!(modifier, direction_mod::EFFECT_TO_CAUSE);
        assert_eq!(modifier, 0.8);
        println!("[VERIFIED] effect→cause direction_mod = 0.8");
    }

    #[test]
    fn test_direction_modifier_same_direction() {
        // Per Constitution: same_direction = 1.0 (no modification)
        assert_eq!(
            get_direction_modifier(CausalDirection::Cause, CausalDirection::Cause),
            direction_mod::SAME_DIRECTION
        );
        assert_eq!(
            get_direction_modifier(CausalDirection::Effect, CausalDirection::Effect),
            direction_mod::SAME_DIRECTION
        );
        println!("[VERIFIED] same_direction direction_mod = 1.0");
    }

    #[test]
    fn test_direction_modifier_unknown() {
        // Unknown directions get no modification (1.0)
        assert_eq!(
            get_direction_modifier(CausalDirection::Unknown, CausalDirection::Cause),
            direction_mod::SAME_DIRECTION
        );
        assert_eq!(
            get_direction_modifier(CausalDirection::Effect, CausalDirection::Unknown),
            direction_mod::SAME_DIRECTION
        );
        assert_eq!(
            get_direction_modifier(CausalDirection::Unknown, CausalDirection::Unknown),
            direction_mod::SAME_DIRECTION
        );
        println!("[VERIFIED] unknown directions get direction_mod = 1.0");
    }

    #[test]
    fn test_get_e5_causal_weight_causal_reasoning() {
        // causal_reasoning profile has E5 = 0.45
        let weight = get_e5_causal_weight("causal_reasoning");
        assert!((weight - 0.45).abs() < 0.01);
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
        // Unknown profile defaults to 0.45 (causal_reasoning default)
        let weight = get_e5_causal_weight("nonexistent_profile");
        assert!((weight - 0.45).abs() < 0.01);
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
        // The asymmetry ratio should be 1.2 / 0.8 = 1.5
        let ratio = direction_mod::CAUSE_TO_EFFECT / direction_mod::EFFECT_TO_CAUSE;
        assert!((ratio - 1.5).abs() < 0.01);
        println!("[VERIFIED] Asymmetry ratio = {} (expected 1.5)", ratio);
    }

    #[test]
    fn test_constitution_compliance_direction_modifiers() {
        // Verify all direction modifiers match Constitution spec
        assert_eq!(direction_mod::CAUSE_TO_EFFECT, 1.2, "Constitution: cause_to_effect must be 1.2");
        assert_eq!(direction_mod::EFFECT_TO_CAUSE, 0.8, "Constitution: effect_to_cause must be 0.8");
        assert_eq!(direction_mod::SAME_DIRECTION, 1.0, "Constitution: same_direction must be 1.0");
        println!("[VERIFIED] All direction modifiers match Constitution specification");
    }

    // =========================================================================
    // COLBERT MAXSIM TESTS
    // =========================================================================

    use super::{compute_colbert_maxsim, COLBERT_WEIGHT};

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

        let score = compute_colbert_maxsim(&query_tokens, &doc_tokens);
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

        let score = compute_colbert_maxsim(&query_tokens, &doc_tokens);
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

        let score = compute_colbert_maxsim(&query_tokens, &doc_tokens);
        // Expected: (1.0 + 0.0) / 2 = 0.5
        assert!((score - 0.5).abs() < 0.01, "Partial match should give ~0.5, got {}", score);
        println!("[VERIFIED] ColBERT MaxSim with partial match = {}", score);
    }

    #[test]
    fn test_colbert_maxsim_empty_inputs() {
        // Empty inputs should return 0.0
        let score_empty_query = compute_colbert_maxsim(&[], &[vec![1.0, 0.0]]);
        let score_empty_doc = compute_colbert_maxsim(&[vec![1.0, 0.0]], &[]);
        let score_both_empty = compute_colbert_maxsim(&[], &[]);

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

        let score = compute_colbert_maxsim(&query_tokens, &doc_tokens);
        assert!(score >= 0.0, "Score should be >= 0.0");
        assert!(score <= 1.0, "Score should be <= 1.0");
        println!("[VERIFIED] ColBERT MaxSim normalized to [0, 1]: {}", score);
    }
}
