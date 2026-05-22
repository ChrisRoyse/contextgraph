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
//! # E5 Causal Retirement
//!
//! E5 causal was an unfinished experimental embedder and is retired. Search may still
//! use active embedders and optional lexical query expansion, but E5 vectors, E5 HNSW
//! indexes, causal hints, and asymmetric E5 reranking are rejected fail-closed.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Instant;
use tracing::{debug, error, info, warn};

use context_graph_core::causal::asymmetric::{
    CausalDirection, apply_causal_gate, causal_gate, compute_e5_asymmetric_fingerprint_similarity,
    detect_causal_query_intent,
};
use context_graph_core::teleological::matrix_search::embedder_names;
use context_graph_core::traits::{
    EmbeddingHintProvenance, EmbeddingMetadata, SearchStrategy, TeleologicalSearchOptions,
};
use context_graph_core::types::audit::{AuditOperation, AuditRecord, EmbeddingVersionRecord};
use context_graph_core::types::fingerprint::{
    NUM_EMBEDDERS, SemanticFingerprint, TeleologicalFingerprint,
};
use context_graph_core::types::{SourceMetadata, SourceType};
use context_graph_core::weights::{
    active_embedder_count, disabled_embedder_names, select_state_conditioned_weight_profile,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use uuid::Uuid;

use crate::weights::{E11_ENTITY_ENABLED, apply_e11_disable, get_effective_weight_profile};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::super::Handlers;
use super::graph_link_dtos::{EMBEDDER_NAMES, RRF_K, embedder_name_to_index};
use super::helpers::{ToolErrorKind, compute_position_label};

// Validation constants for store_memory rationale (merged from inject_context)
// When rationale is provided, validate: 1-1024 chars
const MIN_RATIONALE_LEN: usize = 1;
const MAX_RATIONALE_LEN: usize = 1024;
const MAX_CONTENT_LEN: usize = 10_000;
const MAX_STORE_MEMORIES_BATCH: usize = 64;

// Validation constants for search_graph (BUG-001)
// Per PRD Section 10: topK must be 1-100
const MIN_TOP_K: u64 = 1;
const MAX_TOP_K: u64 = 100;

// E5 Causal Direction inference threshold
// Per Phase 5: Infer causal direction from E5 embedding norms
const CAUSAL_DIRECTION_THRESHOLD: f32 = 0.1;

/// Infer causal direction from E5 asymmetric embeddings.
///
/// MCP-L2 FIX: Uses component variance instead of L2 norms. L2 norms of E5 dual
/// vectors are nearly identical regardless of direction (both are unit-normalized
/// by the model). Component variance captures distributional differences: causal
/// content activates different dimensions than effect content.
///
/// # Returns
/// - "cause" if cause vector has significantly higher variance (>10% difference)
/// - "effect" if effect vector has significantly higher variance
/// - "unknown" if variances are similar or both are near zero
fn infer_causal_direction_from_fingerprint(fingerprint: &SemanticFingerprint) -> String {
    let cause_variance = super::helpers::component_variance_f32(&fingerprint.e5_causal_as_cause);
    let effect_variance = super::helpers::component_variance_f32(&fingerprint.e5_causal_as_effect);

    let max_var = cause_variance.max(effect_variance);
    if max_var < f32::EPSILON {
        return "unknown".to_string(); // Both zero vectors
    }

    let diff_ratio = (cause_variance - effect_variance) / max_var;

    if diff_ratio > CAUSAL_DIRECTION_THRESHOLD {
        "cause".to_string()
    } else if diff_ratio < -CAUSAL_DIRECTION_THRESHOLD {
        "effect".to_string()
    } else {
        "unknown".to_string()
    }
}

#[derive(Debug, Clone)]
struct StoreMemoriesItem {
    content: String,
    session_id: Option<String>,
    session_sequence: u64,
    importance: f32,
    rationale: Option<String>,
    operator_id: Option<String>,
}

struct BatchStoreRecord {
    index: usize,
    fingerprint_id: Uuid,
    content: String,
    session_id: Option<String>,
    session_sequence: u64,
    importance: f32,
    rationale: Option<String>,
    operator_id: Option<String>,
    causal_direction: String,
    cluster_array: [Vec<f32>; 14],
    model_ids: [String; NUM_EMBEDDERS],
    embedding_latency_ms: u64,
    embedding_hint_provenance: Option<EmbeddingHintProvenance>,
    audit_tool: &'static str,
}

fn validate_content_arg(content: &str) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("Content cannot be empty or whitespace-only".to_string());
    }
    if content.len() > MAX_CONTENT_LEN {
        return Err(format!(
            "Content must be at most {} characters, got {}",
            MAX_CONTENT_LEN,
            content.len()
        ));
    }
    Ok(())
}

fn validate_rationale_arg(rationale: Option<&str>) -> Result<(), String> {
    if let Some(r) = rationale {
        if r.len() < MIN_RATIONALE_LEN {
            return Err("rationale must be at least 1 character".to_string());
        }
        if r.len() > MAX_RATIONALE_LEN {
            return Err(format!(
                "rationale must be at most {} characters, got {}",
                MAX_RATIONALE_LEN,
                r.len()
            ));
        }
    }
    Ok(())
}

fn validate_importance_arg(value: Option<&Value>) -> Result<f32, String> {
    match value.and_then(Value::as_f64) {
        Some(v) if !(0.0..=1.0).contains(&v) => {
            Err(format!("importance must be between 0.0 and 1.0, got {}", v))
        }
        Some(v) => Ok(v as f32),
        None if value.is_some() => Err("importance must be a number".to_string()),
        None => Ok(TeleologicalFingerprint::DEFAULT_IMPORTANCE),
    }
}

impl Handlers {
    /// store_memory tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore with 14-embedding fingerprint.
    ///
    /// Stores content in the memory graph. Generates all 14 embeddings for the content
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
            Some(c) if !c.trim().is_empty() && c.len() <= MAX_CONTENT_LEN => c.to_string(),
            Some(c) if c.len() > MAX_CONTENT_LEN => {
                error!(
                    content_len = c.len(),
                    max_allowed = MAX_CONTENT_LEN,
                    "store_memory: content validation FAILED - exceeds maximum"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Content must be at most {} characters, got {}",
                        MAX_CONTENT_LEN,
                        c.len()
                    ),
                );
            }
            Some(c) => {
                error!(
                    content_len = c.len(),
                    trimmed_len = c.trim().len(),
                    "store_memory: content validation FAILED - empty or whitespace-only"
                );
                return self.tool_error(id, "Content cannot be empty or whitespace-only");
            }
            None => {
                error!("store_memory: content validation FAILED - missing content");
                return self.tool_error(id, "Missing 'content' parameter");
            }
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

        let importance = match args.get("importance").and_then(|v| v.as_f64()) {
            Some(v) if !(0.0..=1.0).contains(&v) => {
                return self.tool_error(
                    id,
                    &format!("importance must be between 0.0 and 1.0, got {}", v),
                );
            }
            Some(v) => v as f32,
            None => TeleologicalFingerprint::DEFAULT_IMPORTANCE,
        };

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
        // Audit-11 SA-7 FIX: Schema has additionalProperties:false so only "operatorId" passes.
        // Removed dead snake_case fallback that was unreachable.
        let operator_id = args
            .get("operatorId")
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
                        debug!(
                            "store_memory: Causal hint provider returned None (timeout or low confidence)"
                        );
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
                    context_graph_core::traits::CausalDirectionHint::Neutral => {
                        "unknown".to_string()
                    }
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

        // Generate all 14 embeddings using MultiArrayEmbeddingProvider
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
        // Infer causal direction from LLM hint or E5 norms (no keyword heuristics).
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
        let fingerprint = TeleologicalFingerprint::with_importance(
            embedding_output.fingerprint,
            content_hash,
            importance,
        )
        .with_e6_sparse(e6_sparse);
        let fingerprint_id = fingerprint.id;

        if let Err(e) = self.teleological_store.store(fingerprint).await {
            error!(error = %e, "store_memory: Storage FAILED");
            return self.tool_error(id, &format!("Storage failed: {}", e));
        }

        let records = vec![BatchStoreRecord {
            index: 0,
            fingerprint_id,
            content,
            session_id,
            session_sequence,
            importance,
            rationale: rationale.map(String::from),
            operator_id,
            causal_direction,
            cluster_array,
            model_ids: embedding_output.model_ids,
            embedding_latency_ms: embedding_output.total_latency.as_millis() as u64,
            embedding_hint_provenance: embedding_output.e5_hint_provenance,
            audit_tool: "store_memory",
        }];

        if let Err(e) = self.write_batch_sidecars(&records).await {
            self.rollback_stored_memories(&[fingerprint_id], "store_memory sidecar write failed")
                .await;
            error!(error = %e, "store_memory: sidecar writes FAILED and fingerprint was rolled back");
            return self.tool_error_typed(id, ToolErrorKind::Storage, &e);
        }

        if let Err(e) = self.verify_batch_readback(&records).await {
            self.rollback_stored_memories(
                &[fingerprint_id],
                "store_memory readback verification failed",
            )
            .await;
            error!(error = %e, "store_memory: readback verification FAILED and fingerprint was rolled back");
            return self.tool_error_typed(id, ToolErrorKind::Storage, &e);
        }

        if let Some(builder) = self.graph_builder() {
            builder.enqueue(fingerprint_id).await;
            debug!(
                fingerprint_id = %fingerprint_id,
                "store_memory: Enqueued for K-NN graph building"
            );
        }

        let record = &records[0];
        let mut response = json!({
            "fingerprintId": record.fingerprint_id.to_string(),
            "embedderCount": active_embedder_count(),
            "activeEmbedderCount": active_embedder_count(),
            "storageSlotCount": NUM_EMBEDDERS,
            "disabledEmbedders": disabled_embedder_names(),
            "embeddingLatencyMs": record.embedding_latency_ms,
            "sourceOfTruth": {
                "fingerprints": "CF_FINGERPRINTS",
                "content": "CF_CONTENT",
                "sourceMetadata": "CF_SOURCE_METADATA",
                "embeddingVersions": "CF_EMBEDDING_REGISTRY",
                "audit": "CF_AUDIT_LOG"
            }
        });

        if let Some(r) = record.rationale.as_deref() {
            response["rationale"] = json!(r);
        }

        self.tool_result(id, response)
    }

    /// store_memories tool implementation.
    ///
    /// Batch variant of `store_memory` for corpus ingestion. This still uses the
    /// real 14-embedder pipeline and real RocksDB source-of-truth CFs. The core
    /// write path is all-or-error for fingerprints/content/source metadata,
    /// embedding-version provenance, and memory-created audit rows: validation
    /// happens before embedding, and any post-store failure triggers hard-delete
    /// rollback of stored fingerprints.
    pub(crate) async fn call_store_memories(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let started = Instant::now();
        let (items, causal_hints_enabled) = match self.prepare_store_memories_items(args) {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "store_memories: validation FAILED");
                return self.tool_error_typed(id, ToolErrorKind::Validation, &e);
            }
        };

        let mut causal_hints = Vec::with_capacity(items.len());
        if causal_hints_enabled {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "causalHints=true is not supported because E5 causal is retired and disabled",
            );
        } else {
            causal_hints.resize_with(items.len(), || None);
        }

        let contents: Vec<String> = items.iter().map(|item| item.content.clone()).collect();
        let metadata: Vec<EmbeddingMetadata> = items
            .iter()
            .zip(causal_hints.iter())
            .map(|(item, hint)| EmbeddingMetadata {
                session_id: item.session_id.clone(),
                session_sequence: Some(item.session_sequence),
                timestamp: Some(chrono::Utc::now()),
                causal_hint: hint.clone(),
            })
            .collect();

        let embedding_started = Instant::now();
        let embedding_outputs = match self
            .multi_array_provider
            .embed_batch_all(&contents, &metadata)
            .await
        {
            Ok(outputs) if outputs.len() == items.len() => outputs,
            Ok(outputs) => {
                error!(
                    expected = items.len(),
                    actual = outputs.len(),
                    "store_memories: embed_batch_all returned wrong output count"
                );
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!(
                        "Embedding batch returned {} outputs for {} inputs",
                        outputs.len(),
                        items.len()
                    ),
                );
            }
            Err(e) => {
                error!(error = %e, "store_memories: embed_batch_all FAILED");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Embedding batch failed: {}", e),
                );
            }
        };
        let embedding_elapsed_ms = embedding_started.elapsed().as_millis() as u64;

        let mut fingerprints = Vec::with_capacity(items.len());
        let mut records = Vec::with_capacity(items.len());
        for (index, ((item, hint), embedding_output)) in items
            .into_iter()
            .zip(causal_hints)
            .zip(embedding_outputs)
            .enumerate()
        {
            let mut hasher = Sha256::new();
            hasher.update(item.content.as_bytes());
            let content_hash: [u8; 32] = hasher.finalize().into();

            let cluster_array = embedding_output.fingerprint.to_cluster_array();
            let causal_direction = hint
                .as_ref()
                .and_then(|h| {
                    if h.is_useful() {
                        Some(match h.direction_hint {
                            context_graph_core::traits::CausalDirectionHint::Cause => {
                                "cause".to_string()
                            }
                            context_graph_core::traits::CausalDirectionHint::Effect => {
                                "effect".to_string()
                            }
                            context_graph_core::traits::CausalDirectionHint::Neutral => {
                                "unknown".to_string()
                            }
                        })
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    infer_causal_direction_from_fingerprint(&embedding_output.fingerprint)
                });
            let e6_sparse = embedding_output.fingerprint.e6_sparse.clone();
            let fingerprint = TeleologicalFingerprint::with_importance(
                embedding_output.fingerprint,
                content_hash,
                item.importance,
            )
            .with_e6_sparse(e6_sparse);
            let fingerprint_id = fingerprint.id;
            records.push(BatchStoreRecord {
                index,
                fingerprint_id,
                content: item.content,
                session_id: item.session_id,
                session_sequence: item.session_sequence,
                importance: item.importance,
                rationale: item.rationale,
                operator_id: item.operator_id,
                causal_direction,
                cluster_array,
                model_ids: embedding_output.model_ids,
                embedding_latency_ms: embedding_output.total_latency.as_millis() as u64,
                embedding_hint_provenance: embedding_output.e5_hint_provenance,
                audit_tool: "store_memories",
            });
            fingerprints.push(fingerprint);
        }

        let expected_ids: Vec<Uuid> = records.iter().map(|record| record.fingerprint_id).collect();
        let stored_ids = match self.teleological_store.store_batch(fingerprints).await {
            Ok(ids) if ids == expected_ids => ids,
            Ok(ids) => {
                self.rollback_stored_memories(
                    &ids,
                    "store_batch returned partial or reordered IDs",
                )
                .await;
                error!(
                    expected = ?expected_ids,
                    actual = ?ids,
                    "store_memories: store_batch returned unexpected IDs"
                );
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Storage,
                    "Batch store returned partial or reordered IDs; rolled back stored fingerprints",
                );
            }
            Err(e) => {
                error!(error = %e, "store_memories: store_batch FAILED");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Storage,
                    &format!("Batch store failed: {}", e),
                );
            }
        };

        if let Err(e) = self.write_batch_sidecars(&records).await {
            self.rollback_stored_memories(&stored_ids, "batch sidecar write failed")
                .await;
            error!(error = %e, "store_memories: sidecar writes FAILED and fingerprints were rolled back");
            return self.tool_error_typed(id, ToolErrorKind::Storage, &e);
        }

        if let Err(e) = self.verify_batch_readback(&records).await {
            self.rollback_stored_memories(&stored_ids, "batch readback verification failed")
                .await;
            error!(error = %e, "store_memories: readback verification FAILED and fingerprints were rolled back");
            return self.tool_error_typed(id, ToolErrorKind::Storage, &e);
        }

        if let Some(builder) = self.graph_builder() {
            for id in &stored_ids {
                builder.enqueue(*id).await;
            }
        }

        let item_response: Vec<Value> = records
            .iter()
            .map(|record| {
                json!({
                    "index": record.index,
                    "fingerprintId": record.fingerprint_id.to_string(),
                    "sessionId": record.session_id,
                    "sessionSequence": record.session_sequence,
                    "contentChars": record.content.len(),
                    "importance": record.importance,
                    "causalDirection": record.causal_direction,
                    "embeddingLatencyMs": record.embedding_latency_ms,
                })
            })
            .collect();

        self.tool_result(
            id,
            json!({
                "requestedCount": item_response.len(),
                "storedCount": stored_ids.len(),
                "embedderCount": active_embedder_count(),
                "activeEmbedderCount": active_embedder_count(),
                "storageSlotCount": NUM_EMBEDDERS,
                "disabledEmbedders": disabled_embedder_names(),
                "causalHintsRequested": causal_hints_enabled,
                "causalRelationshipExtraction": "deferred_to_trigger_causal_discovery",
                "embeddingLatencyMs": embedding_elapsed_ms,
                "roundTripMs": started.elapsed().as_millis() as u64,
                "sourceOfTruth": {
                    "fingerprints": "CF_FINGERPRINTS",
                    "content": "CF_CONTENT",
                    "sourceMetadata": "CF_SOURCE_METADATA",
                    "embeddingVersions": "CF_EMBEDDING_REGISTRY",
                    "audit": "CF_AUDIT_LOG"
                },
                "items": item_response
            }),
        )
    }

    fn prepare_store_memories_items(
        &self,
        args: serde_json::Value,
    ) -> Result<(Vec<StoreMemoriesItem>, bool), String> {
        let obj = args
            .as_object()
            .ok_or_else(|| "store_memories arguments must be an object".to_string())?;
        for key in obj.keys() {
            if key != "items" && key != "sessionId" && key != "operatorId" && key != "causalHints" {
                return Err(format!("unknown argument '{}'", key));
            }
        }

        let batch_session_id = match obj.get("sessionId") {
            Some(v) => {
                let s = v
                    .as_str()
                    .ok_or_else(|| "sessionId must be a string".to_string())?;
                if s.trim().is_empty() {
                    return Err("sessionId cannot be empty or whitespace-only".to_string());
                }
                Some(s.to_string())
            }
            None => None,
        };
        let batch_operator_id = match obj.get("operatorId") {
            Some(v) => {
                let s = v
                    .as_str()
                    .ok_or_else(|| "operatorId must be a string".to_string())?;
                if s.trim().is_empty() {
                    return Err("operatorId cannot be empty or whitespace-only".to_string());
                }
                Some(s.to_string())
            }
            None => None,
        };
        let causal_hints = match obj.get("causalHints") {
            Some(v) => v
                .as_bool()
                .ok_or_else(|| "causalHints must be a boolean".to_string())?,
            None => false,
        };

        let item_values = obj
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| "items must be an array".to_string())?;
        if item_values.is_empty() {
            return Err("items must contain at least one memory".to_string());
        }
        if item_values.len() > MAX_STORE_MEMORIES_BATCH {
            return Err(format!(
                "items must contain at most {} memories, got {}",
                MAX_STORE_MEMORIES_BATCH,
                item_values.len()
            ));
        }

        let fallback_session_id = batch_session_id
            .clone()
            .or_else(|| Some(self.get_or_init_session_id()));
        let mut items = Vec::with_capacity(item_values.len());
        for (index, value) in item_values.iter().enumerate() {
            let item_obj = value
                .as_object()
                .ok_or_else(|| format!("items[{index}] must be an object"))?;
            for key in item_obj.keys() {
                if key != "content"
                    && key != "sessionId"
                    && key != "operatorId"
                    && key != "importance"
                    && key != "rationale"
                {
                    return Err(format!("items[{index}] unknown field '{}'", key));
                }
            }
            let content = item_obj
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("items[{index}].content must be a string"))?;
            validate_content_arg(content)
                .map_err(|e| format!("items[{index}].content invalid: {e}"))?;
            let rationale = match item_obj.get("rationale") {
                Some(v) => Some(
                    v.as_str()
                        .ok_or_else(|| format!("items[{index}].rationale must be a string"))?
                        .to_string(),
                ),
                None => None,
            };
            validate_rationale_arg(rationale.as_deref())
                .map_err(|e| format!("items[{index}].rationale invalid: {e}"))?;
            let importance = validate_importance_arg(item_obj.get("importance"))
                .map_err(|e| format!("items[{index}].importance invalid: {e}"))?;
            let session_id = match item_obj.get("sessionId") {
                Some(v) => {
                    let s = v
                        .as_str()
                        .ok_or_else(|| format!("items[{index}].sessionId must be a string"))?;
                    if s.trim().is_empty() {
                        return Err(format!(
                            "items[{index}].sessionId cannot be empty or whitespace-only"
                        ));
                    }
                    Some(s.to_string())
                }
                None => fallback_session_id.clone(),
            };
            let operator_id = match item_obj.get("operatorId") {
                Some(v) => {
                    let s = v
                        .as_str()
                        .ok_or_else(|| format!("items[{index}].operatorId must be a string"))?;
                    if s.trim().is_empty() {
                        return Err(format!(
                            "items[{index}].operatorId cannot be empty or whitespace-only"
                        ));
                    }
                    Some(s.to_string())
                }
                None => batch_operator_id.clone(),
            };

            items.push(StoreMemoriesItem {
                content: content.to_string(),
                session_id,
                session_sequence: self.get_next_sequence(),
                importance,
                rationale,
                operator_id,
            });
        }

        Ok((items, causal_hints))
    }

    async fn write_batch_sidecars(&self, records: &[BatchStoreRecord]) -> Result<(), String> {
        for record in records {
            {
                let mut cluster_mgr = self.cluster_manager.write();
                cluster_mgr
                    .insert(record.fingerprint_id, &record.cluster_array)
                    .map_err(|e| {
                        format!(
                            "cluster_manager insert failed for {}: {}",
                            record.fingerprint_id, e
                        )
                    })?;
            }

            self.teleological_store
                .store_content(record.fingerprint_id, &record.content)
                .await
                .map_err(|e| {
                    format!("store_content failed for {}: {}", record.fingerprint_id, e)
                })?;

            let entity_names = {
                let entity_meta = super::entity_tools::extract_entity_mentions(&record.content);
                let ids: Vec<String> = entity_meta
                    .canonical_ids()
                    .into_iter()
                    .map(String::from)
                    .collect();
                if ids.is_empty() { None } else { Some(ids) }
            };

            let source_metadata = SourceMetadata {
                source_type: SourceType::Manual,
                session_id: record.session_id.clone(),
                session_sequence: Some(record.session_sequence),
                causal_direction: Some(record.causal_direction.clone()),
                created_by: record.operator_id.clone(),
                created_at: Some(chrono::Utc::now()),
                embedding_hint_provenance: record.embedding_hint_provenance.clone(),
                entity_names,
                ..SourceMetadata::default()
            };
            self.teleological_store
                .store_source_metadata(record.fingerprint_id, &source_metadata)
                .await
                .map_err(|e| {
                    format!(
                        "store_source_metadata failed for {}: {}",
                        record.fingerprint_id, e
                    )
                })?;

            let mut embedder_versions = std::collections::HashMap::new();
            for (i, label) in EMBEDDER_NAMES.iter().enumerate() {
                embedder_versions.insert(label.to_string(), record.model_ids[i].clone());
            }
            let version_record = EmbeddingVersionRecord {
                fingerprint_id: record.fingerprint_id,
                computed_at: chrono::Utc::now(),
                embedder_versions,
                e7_model_version: Some(record.model_ids[6].clone()),
                computation_time_ms: Some(record.embedding_latency_ms),
            };
            self.teleological_store
                .store_embedding_version(&version_record)
                .await
                .map_err(|e| {
                    format!(
                        "store_embedding_version failed for {}: {}",
                        record.fingerprint_id, e
                    )
                })?;

            let mut audit_record =
                AuditRecord::new(AuditOperation::MemoryCreated, record.fingerprint_id);
            if let Some(ref op_id) = record.operator_id {
                audit_record = audit_record.with_operator(op_id.clone());
            }
            if let Some(ref session_id) = record.session_id {
                audit_record = audit_record.with_session(session_id.clone());
            }
            if let Some(ref rationale) = record.rationale {
                audit_record = audit_record.with_rationale(rationale);
            }
            let mut audit_parameters = json!({
                "importance": record.importance,
                "content_size": record.content.len(),
                "causal_direction": record.causal_direction,
                "tool": record.audit_tool,
                "embedding_hint_provenance": record.embedding_hint_provenance.clone(),
            });
            if record.audit_tool == "store_memories" {
                audit_parameters["batch_tool"] = json!("store_memories");
            }
            audit_record = audit_record.with_parameters(audit_parameters);
            self.teleological_store
                .append_audit_record(&audit_record)
                .await
                .map_err(|e| {
                    format!(
                        "append_audit_record failed for {}: {}",
                        record.fingerprint_id, e
                    )
                })?;
        }
        Ok(())
    }

    async fn verify_batch_readback(&self, records: &[BatchStoreRecord]) -> Result<(), String> {
        let ids: Vec<Uuid> = records.iter().map(|record| record.fingerprint_id).collect();
        let fingerprints = self
            .teleological_store
            .retrieve_batch(&ids)
            .await
            .map_err(|e| format!("retrieve_batch readback failed: {e}"))?;
        if fingerprints.len() != records.len() {
            return Err(format!(
                "retrieve_batch returned {} rows for {} records",
                fingerprints.len(),
                records.len()
            ));
        }
        for (record, maybe_fp) in records.iter().zip(fingerprints.iter()) {
            if maybe_fp.is_none() {
                return Err(format!(
                    "fingerprint readback missing for {}",
                    record.fingerprint_id
                ));
            }
        }

        let contents = self
            .teleological_store
            .get_content_batch(&ids)
            .await
            .map_err(|e| format!("get_content_batch readback failed: {e}"))?;
        if contents.len() != records.len() {
            return Err(format!(
                "get_content_batch returned {} rows for {} records",
                contents.len(),
                records.len()
            ));
        }
        for (record, maybe_content) in records.iter().zip(contents.iter()) {
            match maybe_content {
                Some(content) if content == &record.content => {}
                Some(_) => {
                    return Err(format!(
                        "content readback mismatch for {}",
                        record.fingerprint_id
                    ));
                }
                None => {
                    return Err(format!(
                        "content readback missing for {}",
                        record.fingerprint_id
                    ));
                }
            }
        }

        let source_metadata = self
            .teleological_store
            .get_source_metadata_batch(&ids)
            .await
            .map_err(|e| format!("get_source_metadata_batch readback failed: {e}"))?;
        if source_metadata.len() != records.len() {
            return Err(format!(
                "get_source_metadata_batch returned {} rows for {} records",
                source_metadata.len(),
                records.len()
            ));
        }
        for (record, maybe_meta) in records.iter().zip(source_metadata.iter()) {
            let Some(meta) = maybe_meta else {
                return Err(format!(
                    "source metadata readback missing for {}",
                    record.fingerprint_id
                ));
            };
            if meta.session_id != record.session_id
                || meta.session_sequence != Some(record.session_sequence)
            {
                return Err(format!(
                    "source metadata readback mismatch for {}",
                    record.fingerprint_id
                ));
            }
        }

        for record in records {
            let Some(version) = self
                .teleological_store
                .get_embedding_version(record.fingerprint_id)
                .await
                .map_err(|e| {
                    format!(
                        "get_embedding_version readback failed for {}: {}",
                        record.fingerprint_id, e
                    )
                })?
            else {
                return Err(format!(
                    "embedding version readback missing for {}",
                    record.fingerprint_id
                ));
            };
            if version.embedder_versions.len() != NUM_EMBEDDERS {
                return Err(format!(
                    "embedding version readback for {} has {} embedders, expected {}",
                    record.fingerprint_id,
                    version.embedder_versions.len(),
                    NUM_EMBEDDERS
                ));
            }
            for label in EMBEDDER_NAMES {
                if !version.embedder_versions.contains_key(label) {
                    return Err(format!(
                        "embedding version readback for {} missing {}",
                        record.fingerprint_id, label
                    ));
                }
            }

            let audit = self
                .teleological_store
                .get_audit_by_target(record.fingerprint_id, 20)
                .await
                .map_err(|e| {
                    format!(
                        "get_audit_by_target readback failed for {}: {}",
                        record.fingerprint_id, e
                    )
                })?;
            let has_expected_create = audit.iter().any(|entry| {
                matches!(entry.operation, AuditOperation::MemoryCreated)
                    && entry.session_id == record.session_id
                    && entry.operator_id == record.operator_id
                    && entry.parameters.get("tool").and_then(Value::as_str)
                        == Some(record.audit_tool)
            });
            if !has_expected_create {
                return Err(format!(
                    "memory-created {} audit readback missing for {}",
                    record.audit_tool, record.fingerprint_id
                ));
            }
        }
        Ok(())
    }

    async fn rollback_stored_memories(&self, ids: &[Uuid], reason: &str) {
        for id in ids {
            match self.teleological_store.delete(*id, false).await {
                Ok(true) => {
                    warn!(fingerprint_id = %id, reason = reason, "store_memories: rolled back fingerprint")
                }
                Ok(false) => {
                    warn!(fingerprint_id = %id, reason = reason, "store_memories: rollback found no fingerprint")
                }
                Err(e) => {
                    error!(fingerprint_id = %id, reason = reason, error = %e, "store_memories: rollback FAILED")
                }
            }
        }
    }

    /// search_graph tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore search_semantic.
    ///
    /// Searches the memory graph for matching content using all 14 embedding spaces.
    ///
    /// TODO(MCP-M4): This function is 1094 lines with 13+ concerns. Split into subfunctions:
    /// - parameter parsing/validation
    /// - strategy selection
    /// - weight profile resolution
    /// - E5 causal direction detection
    /// - HNSW search execution
    /// - ColBERT MaxSim reranking
    /// - temporal post-retrieval scoring
    /// - blind spot detection
    /// - response formatting
    /// - navigation hints
    /// - embedder score breakdown
    /// - audit record emission
    /// - error handling
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

        if !(0.0..=1.0).contains(&min_similarity) {
            return self.tool_error(
                id,
                &format!(
                    "minSimilarity must be between 0.0 and 1.0, got {}",
                    min_similarity
                ),
            );
        }

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
                    &format!(
                        "Unknown strategy '{}'. Valid: e1_only, multi_space, pipeline",
                        unknown
                    ),
                );
            }
        };

        // TASK-MULTISPACE: Parse weight profile (default: "semantic_search")
        let weight_profile = args
            .get("weightProfile")
            .and_then(|v| v.as_str())
            .map(String::from);

        let state_conditioned_profile = args
            .get("stateConditionedProfile")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let learner_id = match args.get("learnerId").and_then(|v| v.as_str()) {
            Some(raw) => match Uuid::parse_str(raw) {
                Ok(value) => Some(value),
                Err(_) => return self.tool_error(id, "learnerId must be a valid UUID"),
            },
            None => None,
        };
        let learner_session_ts = args.get("learnerSessionTs").and_then(|v| v.as_u64());

        // GAP-1: Parse custom weights (overrides weightProfile when provided)
        // AP-NAV-01: FAIL FAST on invalid embedder names
        let custom_weights: Option<[f32; 14]> = match args
            .get("customWeights")
            .and_then(|v| v.as_object())
        {
            Some(obj) => {
                // Reject any keys that are not valid embedder names
                for key in obj.keys() {
                    if !EMBEDDER_NAMES.contains(&key.as_str()) {
                        error!(invalid_key = %key, "search_graph: customWeights contains invalid embedder name");
                        return self.tool_error(
                            id,
                            &format!(
                                "Invalid embedder name '{}' in customWeights. Valid names: E1-E14.",
                                key
                            ),
                        );
                    }
                }
                let mut weights = [0.0f32; 14];
                for (i, name) in EMBEDDER_NAMES.iter().enumerate() {
                    if let Some(val) = obj.get(*name).and_then(|v| v.as_f64()) {
                        weights[i] = val as f32;
                    }
                }
                if !context_graph_core::weights::E5_CAUSAL_ENABLED && weights[4] > 0.0 {
                    return self.tool_error_typed(
                        id,
                        ToolErrorKind::Validation,
                        "E5 causal embedder is retired and disabled; customWeights.E5 must be 0.0",
                    );
                }
                if !E11_ENTITY_ENABLED && weights[10] > 0.0 {
                    return self.tool_error_typed(
                        id,
                        ToolErrorKind::Validation,
                        "E11 entity embedder is disabled; customWeights.E11 must be 0.0",
                    );
                }
                Some(weights)
            }
            None => None,
        };

        // GAP-8: Parse exclude embedders
        let mut exclude_embedder_names: Vec<String> = Vec::new();
        let exclude_embedders: Vec<usize> = match args
            .get("excludeEmbedders")
            .and_then(|v| v.as_array())
        {
            Some(arr) => {
                let mut indices = Vec::new();
                for v in arr {
                    let s = match v.as_str() {
                        Some(s) => s,
                        None => {
                            error!("search_graph: excludeEmbedders contains non-string value");
                            return self
                                .tool_error(id, "excludeEmbedders must contain strings (E1-E14)");
                        }
                    };
                    let idx = match embedder_name_to_index(s) {
                        Ok(i) => i,
                        Err(msg) => {
                            error!(embedder = %s, "search_graph: Invalid embedder in excludeEmbedders");
                            return self.tool_error(id, &msg);
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

        // Parse useQuantizedPrefilter (default: false)
        let use_quantized_prefilter = args
            .get("useQuantizedPrefilter")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // =========================================================================
        // TEMPORAL SEARCH PARAMETERS (ARCH-14)
        // =========================================================================

        // Parse temporalWeight (master weight for all temporal boosts)
        let temporal_weight = match args.get("temporalWeight").and_then(|v| v.as_f64()) {
            Some(v) => {
                let w = v as f32;
                if !(0.0..=1.0).contains(&w) {
                    return self.tool_error_typed(
                        id,
                        ToolErrorKind::Validation,
                        &format!("temporalWeight must be between 0.0 and 1.0, got {}", v),
                    );
                }
                w
            }
            None => 0.0,
        };

        // Parse decayFunction (linear, exponential, step, none)
        // Default is exponential to match the schema definition in core.rs
        let decay_function = match args.get("decayFunction").and_then(|v| v.as_str()) {
            Some("exponential") | None => context_graph_core::traits::DecayFunction::Exponential,
            Some("linear") => context_graph_core::traits::DecayFunction::Linear,
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
        let sequence_anchor = match args.get("sequenceAnchor").and_then(|v| v.as_str()) {
            Some(s) => match uuid::Uuid::parse_str(s) {
                Ok(uuid) => Some(uuid),
                Err(_) => {
                    return self.tool_error_typed(
                        id,
                        ToolErrorKind::Validation,
                        &format!("Invalid sequenceAnchor UUID format: '{}'", s),
                    );
                }
            },
            None => None,
        };

        // Parse sequenceDirection (before, after, around/both)
        let sequence_direction = match args.get("sequenceDirection").and_then(|v| v.as_str()) {
            Some("before") => context_graph_core::traits::SequenceDirection::Before,
            Some("after") => context_graph_core::traits::SequenceDirection::After,
            Some("both") | Some("around") | None => {
                context_graph_core::traits::SequenceDirection::Both
            }
            Some(unknown) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "Unknown sequenceDirection '{}'. Valid: before, after, around, both",
                        unknown
                    ),
                );
            }
        };

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
                    &format!(
                        "Unknown temporalScale '{}'. Valid: micro, meso, macro, long, archival",
                        unknown
                    ),
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
                    &format!(
                        "Unknown sessionScope '{}'. Valid: current, recent, all",
                        unknown
                    ),
                );
            }
        };

        // Auto-select sequence_navigation profile if conversationContext is used
        // This ensures E4 (V_ordering) is prioritized for sequence-based retrieval
        let use_conversation_context = conversation_context.is_some() && anchor_to_current_turn;

        // =========================================================================
        // RETIRED E5 CAUSAL PARAMETERS
        // =========================================================================

        // Parse enableAsymmetricE5 (default: false). Passing true is rejected
        // because E5 causal is retired and must not be used by ME-JEPA/search.
        let enable_asymmetric_e5 = args
            .get("enableAsymmetricE5")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enable_asymmetric_e5 {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "enableAsymmetricE5=true is not supported because E5 causal is retired and disabled",
            );
        }

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
            _ => detect_causal_query_intent(query),
        };

        // Log causal detection for debugging/monitoring. This no longer enables
        // E5 scoring; it only supports optional lexical query expansion.
        if causal_direction != CausalDirection::Unknown {
            info!(
                direction = %causal_direction,
                query_preview = %query.chars().take(100).collect::<String>(),
                "Causal language detected; E5 causal is retired, no E5 reranking will be applied"
            );
        }

        // =========================================================================
        // PHASE 5: QUERY EXPANSION (Optional)
        // =========================================================================

        // Expand causal queries with related terms for better recall
        let search_query = if enable_query_expansion && causal_direction != CausalDirection::Unknown
        {
            expand_causal_query(query, causal_direction)
        } else {
            query.to_string()
        };

        let mut adaptive_policy = json!(null);
        let effective_weight_profile = if state_conditioned_profile {
            if custom_weights.is_some() {
                return self.tool_error(
                    id,
                    "stateConditionedProfile cannot be combined with customWeights because customWeights override profile routing",
                );
            }
            let (Some(learner_id), Some(session_ts)) = (learner_id, learner_session_ts) else {
                return self.tool_error(
                    id,
                    "stateConditionedProfile=true requires learnerId and learnerSessionTs",
                );
            };
            let Some(rocksdb_store) = self
                .teleological_store
                .as_any()
                .downcast_ref::<RocksDbTeleologicalStore>()
            else {
                return self.tool_error(
                    id,
                    "stateConditionedProfile requires RocksDbTeleologicalStore.",
                );
            };
            match rocksdb_store.get_learner_profile(learner_id).await {
                Ok(Some(profile)) if profile.consent_state == "revoked" => return self.tool_error(
                    id,
                    "stateConditionedProfile refuses to use learner state after consent revocation",
                ),
                Ok(_) => {}
                Err(e) => return self.tool_error(id, &format!("get_learner_profile failed: {e}")),
            }
            let state = match rocksdb_store
                .get_learner_state_vector(learner_id, session_ts)
                .await
            {
                Ok(Some(state)) => state,
                Ok(None) => {
                    return self.tool_error(
                        id,
                        "No learner state vector found in CF_LEARNER_STATE_HISTORY for learnerId+learnerSessionTs",
                    )
                }
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_state_vector failed: {e}"))
                }
            };
            let base = weight_profile.as_deref().unwrap_or("semantic_search");
            let selection =
                match select_state_conditioned_weight_profile(Some(base), &state.components) {
                    Ok(value) => value,
                    Err(e) => {
                        return self
                            .tool_error(id, &format!("state-conditioned profile failed: {e}"));
                    }
                };
            adaptive_policy = json!({
                "enabled": true,
                "source_of_truth": {
                    "backend": "rocksdb",
                    "format": "version_byte + bincode",
                    "column_families": ["learner_state_history"]
                },
                "learner_id": learner_id.to_string(),
                "session_ts": session_ts,
                "state_vector_len": state.values.len(),
                "components": {
                    "plasticity_window": state.components.plasticity_window,
                    "hrv_coherence": state.components.hrv_coherence,
                    "valence": state.components.valence,
                    "arousal": state.components.arousal,
                    "stress_floor": state.components.stress_floor,
                    "k_sleep": state.components.k_sleep,
                },
                "base_profile": selection.base_profile,
                "selected_profile": selection.selected_profile,
                "reason": selection.reason,
            });
            Some(
                adaptive_policy
                    .get("selected_profile")
                    .and_then(|v| v.as_str())
                    .expect("selected_profile just set")
                    .to_string(),
            )
        } else {
            weight_profile.clone()
        };

        // Build search options with multi-space parameters
        let fetch_top_k = top_k;

        let mut options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_min_similarity(min_similarity)
            .with_strategy(strategy)
            .with_rerank(enable_rerank)
            .with_causal_direction(CausalDirection::Unknown); // E5 retired: never route storage through E5.

        // Map weight profile to synergy weights for cross-embedder correlation boost
        if let Some(sw) = match effective_weight_profile.as_deref() {
            Some("code_search") => Some([
                0.3, 0.0, 0.0, 0.0, 0.1, 0.2, 1.0, 0.2, 0.1, 0.1, 0.0, 0.0, 0.0, 0.0,
            ]),
            Some("semantic_search") => Some([
                1.0, 0.0, 0.0, 0.0, 0.3, 0.2, 0.1, 0.2, 0.2, 0.3, 0.1, 0.0, 0.0, 0.0,
            ]),
            Some("temporal_navigation") => Some([
                0.3, 0.8, 0.8, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ]),
            Some("causal_reasoning") => Some([
                0.3, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.3, 0.0, 0.0, 0.1, 0.0, 0.0, 0.0,
            ]),
            Some("entity_focused") => Some([
                0.3, 0.0, 0.0, 0.0, 0.0, 0.2, 0.0, 0.2, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
            ]),
            _ => None,
        } {
            options = options.with_synergy_weights(sw);
        }

        // Apply quantized pre-filter option
        options = options.with_quantized_prefilter(use_quantized_prefilter);

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
            if !context_graph_core::weights::E5_CAUSAL_ENABLED && weights[4] > 0.0 {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    "E5 causal embedder is retired and disabled; customWeights.E5 must be 0.0",
                );
            }
            if !E11_ENTITY_ENABLED && weights[10] > 0.0 {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    "E11 entity embedder is disabled; customWeights.E11 must be 0.0",
                );
            }
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
            if matches!(
                decay_function,
                context_graph_core::traits::DecayFunction::Exponential
            ) {
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
            _ => unreachable!("sessionScope validated above"),
        }

        // Apply periodic boost if configured
        if let Some(weight) = periodic_boost {
            let mut periodic = context_graph_core::traits::PeriodicOptions {
                weight,
                ..Default::default()
            };
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
        let query_embedding = match self
            .embed_query(id.clone(), &search_query, "search_graph")
            .await
        {
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
        // - All 14 embedder scores computed for each result
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
                // E5 causal is retired; asymmetric E5 reranking is intentionally disabled.

                let asymmetric_applied = false;

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

                    apply_colbert_reranking(&mut results, &query_embedding, top_k);
                    true
                } else {
                    false
                };

                // Truncate to requested top_k after reranking
                results.truncate(top_k);

                // M6 FIX: Update in-memory fingerprints first, then persist to RocksDB in background.
                // Each update writes ~50KB fingerprint — doing 50+ synchronously inflates latency.
                for result in &mut results {
                    result.fingerprint.record_access();
                }
                {
                    let store = self.teleological_store.clone();
                    let updates: Vec<_> = results.iter().map(|r| r.fingerprint.clone()).collect();
                    // L6 FIX: Remove double-clone — `for fp in updates` gives ownership,
                    // so save the ID before moving fp into update().
                    tokio::spawn(async move {
                        for fp in updates {
                            let memory_id = fp.id;
                            if let Err(e) = store.update(fp).await {
                                tracing::warn!(
                                    error = %e,
                                    memory_id = %memory_id,
                                    "search_graph: Failed to persist access count update (background)"
                                );
                            }
                        }
                    });
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
                                &format!(
                                    "Content retrieval failed for {} results: {}",
                                    results.len(),
                                    e
                                ),
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
                let source_metadata: Vec<Option<context_graph_core::types::SourceMetadata>> =
                    if !results.is_empty() {
                        match self
                            .teleological_store
                            .get_source_metadata_batch(&ids)
                            .await
                        {
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
                                    &format!(
                                        "Source metadata retrieval failed for {} results: {}",
                                        results.len(),
                                        e
                                    ),
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
                // MCP-4 FIX: Error on invalid weight profile name instead of silent uniform fallback.
                let resolved_weights: [f32; 14] = if let Some(cw) = custom_weights {
                    cw
                } else if let Some(ref profile_name) = effective_weight_profile {
                    // Audit-11 MCP-H3: get_weight_profile now returns Result, propagate errors.
                    match self.custom_profiles.read().get(profile_name).copied() {
                        Some(weights) => weights,
                        None => match get_effective_weight_profile(profile_name) {
                            Ok(weights) => weights,
                            Err(e) => {
                                return self.tool_error_typed(
                                    id,
                                    ToolErrorKind::Validation,
                                    &format!(
                                        "Unknown weightProfile '{}': {}. Use one of the built-in profiles or create a custom one via create_weight_profile.",
                                        profile_name, e
                                    ),
                                );
                            }
                        },
                    }
                } else {
                    // WEIGHT-1 FIX: Use semantic_search profile (matches DEFAULT_SEMANTIC_WEIGHTS
                    // used in actual search). Was [1/13; 14] which showed misleading 46% utilization.
                    get_effective_weight_profile("semantic_search")
                        .expect("semantic_search profile must exist")
                };

                // Apply E11 disable to resolved weights for display consistency.
                // get_effective_weight_profile already handles this, but custom weights
                // and custom profiles bypass it, so apply unconditionally.
                let mut resolved_weights = resolved_weights;
                if !E11_ENTITY_ENABLED {
                    apply_e11_disable(&mut resolved_weights);
                }

                // LOW-16: Removed dead `query_analysis: Option<QueryClassification> = None`.
                // The field was always None and the schema advertised a perpetually null
                // `queryClassification`. When query classification is implemented, add it
                // back with actual values.

                let results_json: Vec<_> = results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        // =================================================================
                        // 14-EMBEDDER VISIBILITY FOR AI NAVIGATION
                        // =================================================================
                        // Per Constitution v6.5: Give AI models FULL visibility into all
                        // 14 embedders so they can navigate massive datasets effectively.

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
                        //
                        // MCP-M3 FIX: scoreDelta must reflect ONLY the causal gate contribution
                        // (boost/demotion), not the direction-aware reranking boost that was
                        // applied separately. We undo the direction boost from r.similarity
                        // to recover the post-gate-only score before computing the delta.
                        if asymmetric_applied {
                            let query_is_cause = matches!(causal_direction, CausalDirection::Cause);
                            let e5_sim = compute_e5_asymmetric_fingerprint_similarity(
                                &query_embedding,
                                &r.fingerprint.semantic,
                                query_is_cause,
                            );

                            // Undo direction-aware boost to isolate causal gate contribution.
                            // Direction reranking applies 1.08x when query and result directions match.
                            const DIRECTION_MATCH_BOOST: f32 = 1.08;
                            let result_dir = infer_result_causal_direction(&r.fingerprint.semantic);
                            let direction_multiplier = match (&causal_direction, &result_dir) {
                                (CausalDirection::Cause, CausalDirection::Cause) => DIRECTION_MATCH_BOOST,
                                (CausalDirection::Effect, CausalDirection::Effect) => DIRECTION_MATCH_BOOST,
                                _ => 1.0,
                            };
                            let score_before_direction = r.similarity / direction_multiplier;

                            // Audit-7 MCP-H3 FIX: scoreDelta = score * (multiplier - 1.0)
                            // Previously used s - s/multiplier which is mathematically wrong
                            // (gave ~9% error for boost, ~17% for demotion).
                            let (action, score_delta) = if e5_sim >= causal_gate::CAUSAL_THRESHOLD {
                                ("boost", score_before_direction * (causal_gate::CAUSAL_BOOST - 1.0))
                            } else if e5_sim <= causal_gate::NON_CAUSAL_THRESHOLD {
                                ("demote", score_before_direction * (causal_gate::NON_CAUSAL_DEMOTION - 1.0))
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
                                    let rrf_contribution = weight / (RRF_K + rank as f32 + 1.0);
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
                                    let rrf_contrib = weight / (RRF_K + approx_rank as f32 + 1.0);
                                    json!({
                                        "embedder": name,
                                        "similarity": score,
                                        "rank": approx_rank,
                                        "rrfContribution": rrf_contrib,
                                        "weight": weight
                                    })
                                })
                                .collect();

                            let provenance = json!({
                                "strategy": strategy_name,
                                "weightProfile": effective_weight_profile.as_deref().unwrap_or("default"),
                                "embedderContributions": contributions,
                                "consensusScore": agreement_count as f32 / NUM_EMBEDDERS as f32,
                                "primaryEmbedder": embedder_names::name(r.dominant_embedder()),
                                "isBlindSpotDiscovery": !blind_spots.is_empty() && agreement_count <= 1
                            });

                            // LOW-16: Removed dead queryClassification block (was always None).

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
                    "e5Retired": true,
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
                    "rrfConstant": RRF_K,
                    "resolvedWeightProfile": effective_weight_profile.clone(),
                    "stateConditionedProfile": adaptive_policy,
                });

                // =========================================================================
                // SEARCH TRANSPARENCY: Show which embedders actually participated
                // =========================================================================
                // Per GAP-1: Make it transparent which of the 14 embedders
                // participated in RRF fusion vs. which weights were ignored.
                {
                    // P6: Reuse pre-resolved weights (single lock acquisition above).
                    // resolved_weights already has exclusions applied by resolve_weights_sync(),
                    // so no need to re-apply exclude_embedders here.

                    // Active embedders depend on search strategy, filtered by exclusions.
                    // E11 is conditionally included based on E11_ENTITY_ENABLED toggle.
                    let (strategy_indices, strategy_label) = match strategy {
                        SearchStrategy::E1Only => (vec![0usize], "E1 HNSW only"),
                        SearchStrategy::MultiSpace => {
                            // E2(1), E3(2), E4(3) are weight-gated in search_multi_space_sync
                            // E6(5) participates in scoring (not HNSW recall) when weight > 0
                            let mut indices = vec![0, 1, 2, 3, 5, 6, 7, 8, 9, 13];
                            let label = if E11_ENTITY_ENABLED {
                                indices.push(10);
                                "E1+E2*+E3*+E4*+E6+E7+E8+E9+E10+E11+E14 RRF fusion (* = weight-gated)"
                            } else {
                                "E1+E2*+E3*+E4*+E6+E7+E8+E9+E10+E14 RRF fusion (* = weight-gated, E5 retired, E11 disabled)"
                            };
                            (indices, label)
                        }
                        SearchStrategy::Pipeline => {
                            // E6(5) participates in Stage 2 scoring when weight > 0
                            let mut indices = vec![0, 1, 2, 3, 5, 6, 7, 8, 9, 13];
                            let label = if E11_ENTITY_ENABLED {
                                indices.push(10);
                                "E13 recall -> E1+E2*+E3*+E4*+E6+E7+E8+E9+E10+E11+E14 RRF scoring (* = weight-gated)"
                            } else {
                                "E13 recall -> E1+E2*+E3*+E4*+E6+E7+E8+E9+E10+E14 RRF scoring (* = weight-gated, E5 retired, E11 disabled)"
                            };
                            (indices, label)
                        }
                    };
                    let active_indices: Vec<usize> = strategy_indices
                        .into_iter()
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
                        "totalEmbedderCount": NUM_EMBEDDERS,
                        "weightUtilization": active_sum,
                        "strategyDescription": strategy_label,
                    });
                }

                // Emit SearchPerformed audit (non-fatal)
                {
                    let result_ids: Vec<uuid::Uuid> =
                        results.iter().map(|r| r.fingerprint.id).collect();
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
                        "weight_profile": effective_weight_profile.clone(),
                    }));

                    if let Err(e) = self
                        .teleological_store
                        .append_audit_record(&audit_record)
                        .await
                    {
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
/// E5 weight (index 4) from the profile, or 0.10 if profile not found.
/// Audit-11 MCP-H3: Now uses Result-based get_effective_weight_profile.
/// Uses effective profile so E5 weight reflects E11 redistribution.
#[cfg(test)]
fn get_e5_causal_weight(profile_name: &str) -> f32 {
    get_effective_weight_profile(profile_name)
        .map(|weights| weights[4]) // E5 is at index 4
        .unwrap_or_else(|e| {
            tracing::warn!(profile = %profile_name, error = %e, "Weight profile not found, using default E5 weight 0.10");
            0.10
        })
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
#[allow(dead_code)]
fn apply_asymmetric_e5_reranking(
    results: &mut [TeleologicalSearchResult],
    query_embedding: &SemanticFingerprint,
    query_direction: CausalDirection,
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
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Direction-aware reranking using keyword-detected query direction.
///
/// Uses infer_result_causal_direction() (E5 vector norm comparison) to determine
/// if a result describes a cause or effect, then boosts results whose direction
/// matches what the query seeks.
///
/// This is applied AFTER the binary causal gate to provide a secondary ranking signal.
#[allow(dead_code)]
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
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Infer a document's causal direction by analyzing its E5 embeddings.
///
/// Delegates to the core `infer_direction_from_fingerprint` which uses component
/// variance (not L2 norms) to detect direction in L2-normalized vectors.
/// L2 norms are always ~1.0 for normalized vectors, making norm comparison useless.
/// Component variance detects which projection head produced a more concentrated
/// (peaked) representation, which is the correct signal for direction inference.
///
/// # Arguments
/// * `fingerprint` - Document's semantic fingerprint
///
/// # Returns
/// Inferred causal direction of the document
fn infer_result_causal_direction(fingerprint: &SemanticFingerprint) -> CausalDirection {
    context_graph_core::causal::asymmetric::infer_direction_from_fingerprint(fingerprint)
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
        result.similarity =
            result.similarity * (1.0 - COLBERT_WEIGHT) + maxsim_score * COLBERT_WEIGHT;

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

// MT-L1: compute_position_label moved to helpers.rs to eliminate duplication
// with sequence_tools.rs. Import via `use super::helpers::compute_position_label;`

// =============================================================================
// 14-EMBEDDER VISIBILITY SYSTEM
// =============================================================================
// Per Constitution v6.5: AI models must have FULL visibility into all 14 embedders
// to navigate massive datasets from multiple angles. Each embedder is a unique
// perspective that finds what others miss.
//
// GOAL: Enable AI models to use all 14 embedders as navigation guides through
// massive datasets, understanding which perspectives found what and why.

/// Threshold for E1 "miss" - below this, E1 would have missed the result.
const E1_MISS_THRESHOLD: f32 = 0.3;

/// Threshold for enhancer "find" - above this, the enhancer found something useful.
const ENHANCER_FIND_THRESHOLD: f32 = 0.5;

/// Embedder metadata for AI visibility.
/// Each embedder has a specific signal it captures that E1 might miss.
const EMBEDDER_INFO: [(usize, &str, &str, &str); 14] = [
    // (index, name, category, what_it_finds)
    (
        0,
        "E1_Semantic",
        "FOUNDATION",
        "Dense semantic similarity - the foundation",
    ),
    (
        1,
        "E2_Recency",
        "TEMPORAL",
        "Temporal freshness - recent memories (post-retrieval only)",
    ),
    (
        2,
        "E3_Periodic",
        "TEMPORAL",
        "Time-of-day patterns - daily/weekly cycles (post-retrieval only)",
    ),
    (
        3,
        "E4_Sequence",
        "TEMPORAL",
        "Conversation order - before/after relationships (post-retrieval only)",
    ),
    (
        4,
        "E5_Causal",
        "SEMANTIC",
        "Causal chains - why X caused Y (direction preserved)",
    ),
    (
        5,
        "E6_Sparse",
        "SEMANTIC",
        "Exact keyword matches - precise terminology E1 dilutes",
    ),
    (
        6,
        "E7_Code",
        "SEMANTIC",
        "Code patterns - function signatures, syntax E1 treats as noise",
    ),
    (
        7,
        "E8_Graph",
        "RELATIONAL",
        "Structural relationships - imports, dependencies",
    ),
    (
        8,
        "E9_HDC",
        "STRUCTURAL",
        "Noise-robust structure - survives typos, variations",
    ),
    (
        9,
        "E10_Multimodal",
        "SEMANTIC",
        "Paraphrase detection - same meaning expressed differently",
    ),
    (
        10,
        "E11_Entity",
        "RELATIONAL",
        "Entity knowledge - 'Diesel' = database ORM for Rust",
    ),
    (
        11,
        "E12_ColBERT",
        "SEMANTIC",
        "Exact phrase matches - token-level precision (reranking)",
    ),
    (
        12,
        "E13_SPLADE",
        "SEMANTIC",
        "Term expansions - fast→quick, db→database (recall)",
    ),
    (
        13,
        "E14_BGE_M3_Dense",
        "SEMANTIC",
        "Multilingual semantic/style via BGE-M3 (XLM-RoBERTa)",
    ),
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
/// * `embedder_scores` - All 14 embedder scores [E1, E2, ..., E14]
/// * `e1_score` - E1 semantic score (passed separately for clarity)
///
/// # Returns
/// Vector of blind spot objects with name, score, and what the embedder finds
fn compute_blind_spots(embedder_scores: &[f32; 14], e1_score: f32) -> Vec<serde_json::Value> {
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
        (
            13,
            "E14_BGE_M3_Dense",
            "multilingual/long-context semantic match",
        ),
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

/// Build FULL 14-embedder visibility JSON with all scores and metadata.
///
/// Per Constitution v6.5: AI models need FULL visibility into all 14 embedders
/// to navigate massive datasets. We include ALL scores (not just significant ones)
/// grouped by category with explanations of what each embedder finds.
///
/// # Arguments
/// * `embedder_scores` - All 14 embedder scores
///
/// # Returns
/// JSON object with categorized embedder scores and metadata
fn build_embedder_scores_json(embedder_scores: &[f32; 14]) -> serde_json::Value {
    let mut semantic = serde_json::Map::new();
    let mut relational = serde_json::Map::new();
    let mut structural = serde_json::Map::new();
    let mut temporal = serde_json::Map::new();

    for &(idx, name, category, _) in &EMBEDDER_INFO {
        let score = embedder_scores[idx];
        // E5 sentinel (-1.0) means no causal direction detected — show as null
        let entry = if score < 0.0 {
            serde_json::Value::Null
        } else {
            serde_json::Number::from_f64(score as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::from(0))
        };

        match category {
            "FOUNDATION" | "SEMANTIC" => {
                semantic.insert(name.to_string(), entry);
            }
            "RELATIONAL" => {
                relational.insert(name.to_string(), entry);
            }
            "STRUCTURAL" => {
                structural.insert(name.to_string(), entry);
            }
            "TEMPORAL" => {
                temporal.insert(name.to_string(), entry);
            }
            unknown => {
                warn!(category = %unknown, "Unknown weight category '{}' — not included in breakdown", unknown);
            }
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
/// * `embedder_scores` - All 14 embedder scores
///
/// # Returns
/// Vector of navigation suggestions
fn compute_navigation_hints(embedder_scores: &[f32; 14]) -> Vec<String> {
    let mut hints = Vec::new();

    let e1 = embedder_scores[0];
    let e5 = embedder_scores[4];
    let e6 = embedder_scores[5];
    let e7 = embedder_scores[6];
    let e8 = embedder_scores[7];
    let e10 = embedder_scores[9];
    let e11 = embedder_scores[10];

    // E5 sentinel: no causal direction detected
    if e5 < 0.0 {
        hints.push(
            "E5 (causal): no signal — use causalDirection param for causal queries".to_string(),
        );
    }

    // Suggest based on what's strong vs weak
    if e7 > e1 + 0.2 {
        hints.push("E7 (code) found more than E1 - try search_code for code patterns".to_string());
    }
    if e11 > e1 + 0.2 {
        hints.push(
            "E11 (entity) found more than E1 - try search_by_entities for relationships"
                .to_string(),
        );
    }
    if e5 > e1 + 0.2 {
        hints.push(
            "E5 (causal) found more than E1 - try search_causes for causal chains".to_string(),
        );
    }
    if e8 > 0.5 {
        hints.push(
            "E8 (graph) is strong - try search_connections for imports/dependencies".to_string(),
        );
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
    use super::{
        COLBERT_WEIGHT, CausalDirection, apply_causal_gate, build_embedder_scores_json,
        compute_blind_spots, compute_navigation_hints, expand_causal_query, get_e5_causal_weight,
    };
    use super::{MAX_RATIONALE_LEN, MAX_TOP_K, MIN_RATIONALE_LEN, MIN_TOP_K};
    use context_graph_core::causal::asymmetric::{causal_gate, direction_mod};
    use context_graph_storage::compute_maxsim_direct;

    #[test]
    fn test_validation_and_causal_gate() {
        // BUG-001/002: rationale and topK validation (inline checks after validation.rs removal)
        assert!("".chars().count() < MIN_RATIONALE_LEN);
        assert!(
            "x".chars().count() >= MIN_RATIONALE_LEN && "x".chars().count() <= MAX_RATIONALE_LEN
        );
        const { assert!(0_u64 < MIN_TOP_K) };
        const { assert!(1_u64 >= MIN_TOP_K && 1_u64 <= MAX_TOP_K) };
        const { assert!(101_u64 > MAX_TOP_K) };
        // Causal gate: boost, demotion, passthrough, dead zone
        let boosted = apply_causal_gate(0.80, 0.05, true);
        assert!((boosted - 0.80 * causal_gate::CAUSAL_BOOST).abs() < 1e-6);
        let demoted = apply_causal_gate(0.80, 0.005, true);
        assert!((demoted - 0.80 * causal_gate::NON_CAUSAL_DEMOTION).abs() < 1e-6);
        assert!((apply_causal_gate(0.80, 0.99, false) - 0.80).abs() < 1e-6);
        assert!((apply_causal_gate(0.80, 0.02, true) - 0.80).abs() < 1e-6);
        // E5 is retired, so effective profiles expose zero E5 weight.
        assert_eq!(get_e5_causal_weight("causal_reasoning"), 0.0);
        assert_eq!(get_e5_causal_weight("semantic_search"), 0.0);
        // Direction modifiers
        assert_eq!(direction_mod::CAUSE_TO_EFFECT, 1.2);
        assert_eq!(direction_mod::EFFECT_TO_CAUSE, 0.8);
        // Causal query expansion
        let expanded = expand_causal_query("what happened to the server", CausalDirection::Cause);
        assert!(expanded.contains("cause"));
        let expanded = expand_causal_query("delete the file", CausalDirection::Effect);
        assert!(expanded.contains("effect"));
        assert_eq!(
            expand_causal_query("show me the code", CausalDirection::Unknown),
            "show me the code"
        );
    }

    #[test]
    fn test_colbert_maxsim_and_blind_spots() {
        // ColBERT MaxSim
        let score = compute_maxsim_direct(
            &[vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]],
            &[vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]],
        );
        assert!((score - 1.0).abs() < 0.01);
        let score = compute_maxsim_direct(&[vec![1.0, 0.0, 0.0]], &[vec![0.0, 1.0, 0.0]]);
        assert!((score - 0.5).abs() < 0.01);
        assert_eq!(compute_maxsim_direct(&[], &[]), 0.0);
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(COLBERT_WEIGHT >= 0.1 && COLBERT_WEIGHT <= 0.2);
        }
        // Blind spot detection
        let mut scores = [0.0_f32; 14];
        scores[0] = 0.2;
        scores[6] = 0.8;
        let blind_spots = compute_blind_spots(&scores, scores[0]);
        assert_eq!(blind_spots.len(), 1);
        assert_eq!(blind_spots[0]["embedder"], "E7_Code");
        scores[0] = 0.5;
        assert!(compute_blind_spots(&scores, scores[0]).is_empty());
        // Embedder scores JSON
        let json = build_embedder_scores_json(&[0.15_f32; 14]);
        let total = json["semantic"].as_object().unwrap().len()
            + json["relational"].as_object().unwrap().len()
            + json["structural"].as_object().unwrap().len()
            + json["temporal"].as_object().unwrap().len();
        assert_eq!(total, 14);
        // Navigation hints
        let mut scores2 = [0.0_f32; 14];
        scores2[0] = 0.3;
        scores2[6] = 0.7;
        let hints = compute_navigation_hints(&scores2);
        assert!(hints.iter().any(|h| h.contains("search_code")));
    }
}
