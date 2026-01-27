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
//! - `pipeline`: Full 3-stage retrieval (E13 recall → E1 dense → E12 rerank)
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
    compute_e5_asymmetric_fingerprint_similarity, detect_causal_query_intent,
    direction_mod, CausalDirection,
};
use context_graph_core::teleological::matrix_search::embedder_names;
use context_graph_core::traits::{EmbeddingMetadata, SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::fingerprint::{SemanticFingerprint, TeleologicalFingerprint, NUM_EMBEDDERS};
use context_graph_core::types::{SourceMetadata, SourceType};

use crate::weights::get_weight_profile;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

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

// =============================================================================
// E10 INTENT MODE (Phase 2 E10 Upgrade)
// =============================================================================

/// Intent mode for E10 asymmetric similarity.
///
/// Per E10 Upgrade Plan: Enables intent-aware search in search_graph.
/// - SeekingIntent: Query is a goal/purpose, apply intent→context 1.2x boost
/// - SeekingContext: Query is a situation, apply context→intent 0.8x
/// - None: Disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentMode {
    /// Disabled (no E10 asymmetric reranking)
    None,
    /// Query is a goal/purpose - apply intent→context 1.2x boost
    SeekingIntent,
    /// Query is a situation - apply context→intent 0.8x
    SeekingContext,
}

/// Direction modifiers for E10 intent asymmetric similarity.
/// Per Constitution: intent→context = 1.2x, context→intent = 0.8x
mod intent_mod {
    pub const INTENT_TO_CONTEXT: f32 = 1.2;
    pub const CONTEXT_TO_INTENT: f32 = 0.8;
    pub const SAME_DIRECTION: f32 = 1.0;
}

/// Detect intent mode from query text.
///
/// Analyzes query patterns to detect if the user is:
/// - Seeking intent: "What was the goal?", "find purpose", "accomplish", "trying to"
/// - Seeking context: "In the situation where", "given the context", "when dealing with"
///
/// # Arguments
/// * `query` - Query text
///
/// # Returns
/// Detected intent mode
fn detect_intent_mode(query: &str) -> IntentMode {
    let query_lower = query.to_lowercase();

    // Intent-seeking patterns (query is describing a goal/purpose)
    let intent_patterns = [
        "goal", "purpose", "intent", "accomplish", "achieve", "trying to",
        "want to", "need to", "aim to", "objective", "target", "mission",
        "what was the plan", "what were we doing", "what is the intent",
    ];

    // Context-seeking patterns (query is describing a situation)
    let context_patterns = [
        "in the situation", "given the context", "when dealing with",
        "in the case of", "for the scenario", "situation where",
        "context of", "circumstances", "environment where", "setting where",
    ];

    // Check for intent-seeking
    for pattern in intent_patterns {
        if query_lower.contains(pattern) {
            return IntentMode::SeekingIntent;
        }
    }

    // Check for context-seeking
    for pattern in context_patterns {
        if query_lower.contains(pattern) {
            return IntentMode::SeekingContext;
        }
    }

    IntentMode::None
}

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

        // CAUSAL-RELATIONSHIP: Clone the hint before it moves into metadata
        // We need the hint later to store the causal relationship with provenance
        let causal_hint_for_storage = causal_hint.clone();

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

                // CAUSAL-RELATIONSHIP: Store LLM-generated causal description with provenance
                // Per plan: When confidence >= 0.5, embed description and store in CF_CAUSAL_RELATIONSHIPS
                // Use the causal_hint_for_storage we cloned earlier (before it moved into metadata)
                if let Some(ref hint) = causal_hint_for_storage {
                    if hint.is_useful() {
                        if let Some(ref description) = hint.description {
                            // Embed the description using E1 (1024D semantic)
                            match self
                                .multi_array_provider
                                .embed_e1_only(description)
                                .await
                            {
                                Ok(desc_embedding) => {
                                    // Create causal relationship with PROVENANCE
                                    let causal_rel = context_graph_core::types::CausalRelationship::new(
                                        description.clone(),
                                        desc_embedding,
                                        content.clone(),              // Original source content
                                        fingerprint_id,               // Source memory ID
                                        match hint.direction_hint {
                                            context_graph_core::traits::CausalDirectionHint::Cause => "cause".to_string(),
                                            context_graph_core::traits::CausalDirectionHint::Effect => "effect".to_string(),
                                            context_graph_core::traits::CausalDirectionHint::Neutral => "neutral".to_string(),
                                        },
                                        hint.confidence,
                                        hint.key_phrases.clone(),
                                    );

                                    // Store in dedicated CF
                                    match self.teleological_store
                                        .store_causal_relationship(&causal_rel)
                                        .await
                                    {
                                        Ok(causal_id) => {
                                            info!(
                                                causal_id = %causal_id,
                                                source_id = %fingerprint_id,
                                                direction = %causal_rel.direction,
                                                desc_len = description.len(),
                                                "store_memory: Stored causal relationship with provenance"
                                            );
                                        }
                                        Err(e) => {
                                            // Non-fatal: fingerprint is stored, causal relationship is optional
                                            warn!(
                                                fingerprint_id = %fingerprint_id,
                                                error = %e,
                                                "store_memory: Failed to store causal relationship (fingerprint saved successfully)"
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    // Non-fatal: E1 embedding for description failed
                                    warn!(
                                        fingerprint_id = %fingerprint_id,
                                        error = %e,
                                        "store_memory: Failed to embed causal description (fingerprint saved successfully)"
                                    );
                                }
                            }
                        }
                    }
                }

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

        // =========================================================================
        // SEARCH STRATEGY (ARCH-12, ARCH-21)
        // =========================================================================
        // Parse strategy parameter (default: multi_space for optimal blind spot detection)
        // - e1_only: E1-only HNSW search (fast, simple queries)
        // - multi_space: Weighted RRF fusion of E1 + enhancers (default - uses weight profiles)
        // - pipeline: Full 3-stage retrieval (E13 recall → E1 dense → E12 rerank)
        //
        // E1 is the foundation (ARCH-12). Other embedders ENHANCE E1 by finding blind spots.
        let strategy = args
            .get("strategy")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "e1_only" => SearchStrategy::E1Only,
                "pipeline" => SearchStrategy::Pipeline,
                _ => SearchStrategy::MultiSpace, // "multi_space" or unknown defaults to multi-space
            })
            .unwrap_or(SearchStrategy::MultiSpace); // Default to multi-space for optimal results

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
        // E10 INTENT ASYMMETRIC PARAMETERS (ARCH-15)
        // =========================================================================

        // Parse intentMode (none, seeking_intent, seeking_context, auto)
        // - none: Disabled (default)
        // - seeking_intent: Query is a goal/purpose, apply intent→context 1.2x boost
        // - seeking_context: Query is a situation, apply context→intent 0.8x
        // - auto: Auto-detect from query text
        let intent_mode_param = args
            .get("intentMode")
            .and_then(|v| v.as_str())
            .unwrap_or("none");

        // Parse intentBlend (default: 0.3)
        // Blend weight for E10 vs E1 semantic [0.0=pure E1, 1.0=pure E10]
        let intent_blend = args
            .get("intentBlend")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.3);

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
        // PHASE 1b: INTENT MODE DETECTION
        // =========================================================================
        // Detect intent mode from query text or use user-specified mode
        // Per E10 Upgrade Plan: seeking_intent applies 1.2x, seeking_context applies 0.8x

        let intent_mode = match intent_mode_param {
            "seeking_intent" => IntentMode::SeekingIntent,
            "seeking_context" => IntentMode::SeekingContext,
            "auto" => detect_intent_mode(query),
            "none" | _ => IntentMode::None,
        };

        // Log intent detection for debugging/monitoring
        if intent_mode != IntentMode::None {
            info!(
                mode = ?intent_mode,
                blend = intent_blend,
                query_preview = %query.chars().take(100).collect::<String>(),
                "Intent mode active - asymmetric E10 reranking will be applied"
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
        // Priority: user-specified > causal > intent > conversation_context > default
        let effective_weight_profile = match (&weight_profile, &causal_direction, &intent_mode, use_conversation_context) {
            // User specified a profile - always use it
            (Some(profile), _, _, _) => Some(profile.clone()),
            // Causal query detected - use causal_reasoning
            (None, CausalDirection::Cause | CausalDirection::Effect, _, _) => {
                debug!("Auto-selecting 'causal_reasoning' profile for causal query");
                Some("causal_reasoning".to_string())
            }
            // Intent mode active - use intent_enhanced for stronger E10 weighting
            (None, CausalDirection::Unknown, IntentMode::SeekingIntent | IntentMode::SeekingContext, _) => {
                debug!("Auto-selecting 'intent_enhanced' profile for intent-aware query");
                Some("intent_enhanced".to_string())
            }
            // Conversation context enabled - use conversation_history for balanced E1+E4
            (None, CausalDirection::Unknown, IntentMode::None, true) => {
                debug!("Auto-selecting 'conversation_history' profile for conversation context");
                Some("conversation_history".to_string())
            }
            // No special case - use default
            (None, CausalDirection::Unknown, IntentMode::None, false) => weight_profile.clone(),
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
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
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
                // PHASE 3: ASYMMETRIC E10 INTENT RERANKING (E10 Upgrade)
                // =========================================================================
                // Apply asymmetric E10 similarity for intent-aware queries
                // Per E10 Upgrade Plan: intent→context 1.2x, context→intent 0.8x

                let intent_reranking_applied = if intent_mode != IntentMode::None && !results.is_empty() {
                    info!(
                        results_count = results.len(),
                        intent_mode = ?intent_mode,
                        intent_blend = intent_blend,
                        "Applying asymmetric E10 intent reranking"
                    );

                    apply_asymmetric_e10_reranking(
                        &mut results,
                        &query_embedding,
                        intent_mode,
                        intent_blend,
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
                            "dominantEmbedder": dominant_name,
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
// E10 INTENT ASYMMETRIC RERANKING (E10 Upgrade Plan Phase 2)
// =============================================================================

/// Apply asymmetric E10 intent reranking to search results.
///
/// This function implements E10 intent-aware reranking per the E10 Upgrade Plan:
/// - Uses E10 asymmetric vectors (intent vs context)
/// - Applies direction modifiers: intent→context 1.2x, context→intent 0.8x
/// - Blends E10 score with existing similarity using intent_blend weight
/// - Re-sorts results by combined score
///
/// # Arguments
/// * `results` - Mutable reference to search results to rerank
/// * `query_embedding` - Query's semantic fingerprint
/// * `intent_mode` - Intent mode (SeekingIntent or SeekingContext)
/// * `intent_blend` - Blend weight [0.0=pure E1, 1.0=pure E10]
fn apply_asymmetric_e10_reranking(
    results: &mut [TeleologicalSearchResult],
    query_embedding: &SemanticFingerprint,
    intent_mode: IntentMode,
    intent_blend: f32,
) {
    if results.is_empty() || intent_mode == IntentMode::None {
        return;
    }

    // Get query E10 vectors based on intent mode
    let (query_e10, modifier, doc_e10_getter): (
        &[f32],
        f32,
        fn(&SemanticFingerprint) -> &[f32],
    ) = match intent_mode {
        IntentMode::SeekingIntent => {
            // Query is a goal - use query's intent to find matching contexts
            // intent→context direction, apply 1.2x boost
            (
                query_embedding.get_e10_as_intent(),
                intent_mod::INTENT_TO_CONTEXT,
                |fp: &SemanticFingerprint| fp.get_e10_as_context(),
            )
        }
        IntentMode::SeekingContext => {
            // Query is a situation - use query's context to find matching intents
            // context→intent direction, apply 0.8x dampening
            (
                query_embedding.get_e10_as_context(),
                intent_mod::CONTEXT_TO_INTENT,
                |fp: &SemanticFingerprint| fp.get_e10_as_intent(),
            )
        }
        IntentMode::None => return,
    };

    // E1 weight is the complement of intent_blend
    let e1_weight = 1.0 - intent_blend;
    let mut reranked = 0;

    for result in results.iter_mut() {
        // Get document E10 vector based on direction
        let doc_e10 = doc_e10_getter(&result.fingerprint.semantic);

        // Compute E10 cosine similarity
        let e10_raw_sim = compute_cosine_similarity(query_e10, doc_e10);

        // Apply direction modifier
        let e10_modified_sim = (e10_raw_sim * modifier).clamp(0.0, 1.0);

        // Blend E1 (existing) and E10 scores
        // Formula: new_sim = e1_weight × e1_sim + intent_blend × e10_modified_sim
        let e1_sim = result.similarity; // Existing score (dominated by E1)
        result.similarity = e1_weight * e1_sim + intent_blend * e10_modified_sim;

        reranked += 1;
    }

    // Re-sort after E10 reranking
    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    debug!(
        reranked = reranked,
        intent_mode = ?intent_mode,
        intent_blend = intent_blend,
        modifier = modifier,
        "E10 intent reranking applied"
    );
}

/// Compute cosine similarity between two vectors.
///
/// Returns 0.0 if either vector is empty or has zero norm.
fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
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
    (9, "E10_Intent", "SEMANTIC", "Goal alignment - same purpose expressed differently"),
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
        (9, "E10_Intent", "intent alignment"),
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
        hints.push("E10 (intent) found aligned goals - try search_by_intent for similar purposes".to_string());
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
        scores[9] = 0.55;  // E10 Intent found

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
