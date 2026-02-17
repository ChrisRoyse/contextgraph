//! Causal discovery tool implementations.
//!
//! Provides MCP tools for triggering and monitoring the LLM-based
//! causal discovery agent (Qwen2.5/Hermes 2 Pro).
//!
//! # Tools
//!
//! - `trigger_causal_discovery` - Manually trigger causal analysis on memories
//! - `get_causal_discovery_status` - Get agent status and statistics
//!
//! # Architecture
//!
//! Supports two modes:
//! - `mode: "pairs"` - Pair analysis using GraphDiscoveryService (existing)
//! - `mode: "extract"` - Multi-relationship extraction per memory (NEW)
//!
//! The "extract" mode uses the CausalHintProvider's `extract_all_relationships`
//! method to find ALL cause-effect relationships in individual memories,
//! generating E5 dual embeddings for each.

use serde_json::json;
use tracing::{debug, error, info, warn};

use context_graph_core::causal::asymmetric::{detect_causal_query_intent, CausalDirection};
use context_graph_core::traits::{CausalDirectionHint, CausalHint, EmbeddingMetadata};
use context_graph_core::types::audit::{AuditOperation, AuditRecord};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_core::types::SourceMetadata;
#[cfg(feature = "llm")]
use context_graph_graph_agent::MemoryForGraphAnalysis;

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

/// Non-LLM stubs: When `llm` feature is disabled, these tools return an error.
#[cfg(not(feature = "llm"))]
impl Handlers {
    pub(crate) async fn call_trigger_causal_discovery(
        &self,
        id: Option<JsonRpcId>,
        _args: serde_json::Value,
    ) -> JsonRpcResponse {
        self.tool_error(id, "trigger_causal_discovery requires the 'llm' feature (CausalDiscoveryLLM + GraphDiscoveryService)")
    }

    pub(crate) async fn call_get_causal_discovery_status(
        &self,
        id: Option<JsonRpcId>,
        _args: serde_json::Value,
    ) -> JsonRpcResponse {
        self.tool_error(id, "get_causal_discovery_status requires the 'llm' feature (CausalDiscoveryLLM + GraphDiscoveryService)")
    }
}

#[cfg(feature = "llm")]
impl Handlers {
    /// trigger_causal_discovery tool implementation.
    ///
    /// Manually triggers causal analysis on memories using the LLM.
    /// Discovered relationships are stored with E5 asymmetric embeddings.
    ///
    /// # Parameters
    ///
    /// - `mode`: Analysis mode ("pairs" for pair analysis, "extract" for multi-relationship extraction, default: "extract")
    /// - `maxMemories`: Maximum memories to analyze (1-200, default: 50)
    /// - `minConfidence`: Minimum LLM confidence to accept (0.5-1.0, default: 0.7)
    /// - `sessionScope`: Scope of memories ("current", "all", "recent", default: "all")
    /// - `dryRun`: Analyze but don't store relationships (default: false)
    ///
    /// # Modes
    ///
    /// - `"extract"`: Uses multi-relationship extraction to find ALL cause-effect
    ///   relationships in each memory. Best for dense causal content.
    /// - `"pairs"`: Uses pair analysis via GraphDiscoveryService. Best for finding
    ///   relationships BETWEEN memories.
    pub(crate) async fn call_trigger_causal_discovery(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse mode parameter (default: "extract")
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("extract");

        // Validate mode
        if !matches!(mode, "extract" | "pairs") {
            return self.tool_error(id, "mode must be 'extract' or 'pairs'");
        }

        // Parse parameters with defaults per schema
        let max_memories = args
            .get("maxMemories")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;
        let min_confidence = args
            .get("minConfidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7) as f32;
        let session_scope = args
            .get("sessionScope")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let dry_run = args
            .get("dryRun")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate parameters
        if !(1..=200).contains(&max_memories) {
            return self.tool_error(id, "maxMemories must be between 1 and 200");
        }
        if !(0.5..=1.0).contains(&min_confidence) {
            return self.tool_error(id, "minConfidence must be between 0.5 and 1.0");
        }
        if !matches!(session_scope, "current" | "all" | "recent") {
            return self.tool_error(id, "sessionScope must be 'current', 'all', or 'recent'");
        }

        info!(
            mode = mode,
            max_memories = max_memories,
            min_confidence = min_confidence,
            session_scope = session_scope,
            dry_run = dry_run,
            "trigger_causal_discovery: Starting"
        );

        // Route to appropriate mode handler
        if mode == "extract" {
            return self
                .trigger_causal_discovery_extract(id, max_memories, min_confidence, dry_run)
                .await;
        }

        let service = match self.graph_discovery_service() {
            Some(s) => s,
            None => {
                return self.tool_error(
                    id,
                    "LLM not loaded — trigger_causal_discovery unavailable. \
                     Server started in degraded mode (check VRAM/model).",
                );
            }
        };

        // For now, we'll get recent fingerprints from the store and run discovery
        // In the future, this could be optimized with a dedicated candidate finder

        // Get list of indexed files as a proxy for available content
        let indexed_files = match self.teleological_store.list_indexed_files().await {
            Ok(files) => files,
            Err(e) => {
                warn!(error = %e, "trigger_causal_discovery: Could not list indexed files");
                Vec::new()
            }
        };

        // MCP-01 FIX: Apply sessionScope filter to indexed files.
        // "current" = last 10 files, "recent" = last 50 files, "all" = no filter.
        if indexed_files.is_empty() {
            return self.tool_error(
                id,
                "No files are indexed - cannot perform causal discovery. Store some memories first.",
            );
        }

        let scoped_files: Vec<_> = match session_scope {
            "current" => indexed_files.into_iter().rev().take(10).collect(),
            "recent" => indexed_files.into_iter().rev().take(50).collect(),
            _ => indexed_files, // "all"
        };

        // Collect memory IDs from indexed files (up to max_memories * 2 to have enough pairs)
        let mut memory_ids: Vec<uuid::Uuid> = Vec::new();
        let mut file_read_errors: Vec<String> = Vec::new();
        for file in scoped_files.iter().take(max_memories * 2) {
            match self
                .teleological_store
                .get_fingerprints_for_file(&file.file_path)
                .await
            {
                Ok(ids) => {
                    memory_ids.extend(ids);
                    if memory_ids.len() >= max_memories * 2 {
                        break;
                    }
                }
                Err(e) => {
                    error!(
                        error = %e,
                        file_path = %file.file_path,
                        "trigger_causal_discovery: Failed to read fingerprints for file"
                    );
                    file_read_errors.push(format!("{}: {}", file.file_path, e));
                }
            }
        }

        debug!(
            memory_ids_count = memory_ids.len(),
            "trigger_causal_discovery: Collected memory IDs"
        );

        // If we don't have enough memories, return early
        if memory_ids.len() < 2 {
            info!("trigger_causal_discovery: Not enough memories for analysis (need at least 2)");
            let mut msg = "Not enough memories for causal analysis (need at least 2)".to_string();
            if !file_read_errors.is_empty() {
                msg = format!("{}. {} file(s) could not be read: {}", msg, file_read_errors.len(), file_read_errors.join("; "));
            }
            return self.tool_result(
                id,
                json!({
                    "status": "completed",
                    "pairsAnalyzed": 0,
                    "relationshipsFound": 0,
                    "message": msg,
                    "fileReadErrors": file_read_errors,
                    "sessionScope": session_scope,
                    "dryRun": dry_run
                }),
            );
        }

        // Limit to max_memories * 2 memories (to create max_memories pairs)
        memory_ids.truncate(max_memories * 2);

        // Fetch memory data for analysis
        let mut memories_for_analysis: Vec<MemoryForGraphAnalysis> = Vec::new();
        let mut fetch_errors = 0;

        for uuid in &memory_ids {
            // Get fingerprint
            let fingerprint = match self.teleological_store.retrieve(*uuid).await {
                Ok(Some(fp)) => fp,
                Ok(None) => {
                    debug!(uuid = %uuid, "causal_discovery: Fingerprint not found (deleted or expired)");
                    fetch_errors += 1;
                    continue;
                }
                Err(e) => {
                    error!(uuid = %uuid, error = %e, "causal_discovery: Failed to retrieve fingerprint");
                    fetch_errors += 1;
                    continue;
                }
            };

            // Get content
            let content = match self.teleological_store.get_content(*uuid).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    debug!(uuid = %uuid, "causal_discovery: Content not found for fingerprint");
                    fetch_errors += 1;
                    continue;
                }
                Err(e) => {
                    error!(uuid = %uuid, error = %e, "causal_discovery: Failed to get content");
                    fetch_errors += 1;
                    continue;
                }
            };

            // Get source metadata (optional)
            // MED-21 FIX: Log errors instead of silently swallowing with .ok()
            let source_metadata = match self
                .teleological_store
                .get_source_metadata(*uuid)
                .await
            {
                Ok(meta) => meta,
                Err(e) => {
                    warn!(error = %e, memory_id = %uuid, "trigger_causal_discovery: Failed to read source_metadata");
                    None
                }
            };

            // Gap 4: E5 pre-filter — skip memories without causal language indicators
            // before sending to LLM pair analysis. Saves LLM inference time by filtering
            // non-causal content before expensive pair generation.
            let content_causal_signal = detect_causal_query_intent(&content);
            if content_causal_signal == CausalDirection::Unknown {
                debug!(
                    memory_id = %uuid,
                    "trigger_causal_discovery: E5 pre-filter skipped non-causal memory"
                );
                continue;
            }

            memories_for_analysis.push(MemoryForGraphAnalysis {
                id: *uuid,
                content,
                created_at: fingerprint.created_at,
                session_id: source_metadata.as_ref().and_then(|m| m.session_id.clone()),
                e1_embedding: fingerprint.semantic.e1_semantic.to_vec(),
                source_type: source_metadata.as_ref().map(|m| format!("{}", m.source_type)),
                file_path: source_metadata.and_then(|m| m.file_path),
            });

            if memories_for_analysis.len() >= max_memories * 2 {
                break;
            }
        }

        // ERR-4: If >50% of fetches failed, the storage layer is likely broken
        let total_attempted = memory_ids.len();
        if total_attempted > 0 && fetch_errors > total_attempted / 2 {
            error!(
                fetch_errors,
                total_attempted,
                "causal_discovery: >50% of memory fetches failed — aborting due to likely storage issue"
            );
            return self.tool_error(
                id,
                &format!(
                    "Causal discovery aborted: {} of {} memory fetches failed (>50%%). Storage may be corrupted or unavailable.",
                    fetch_errors, total_attempted
                ),
            );
        }

        if memories_for_analysis.len() < 2 {
            info!(
                fetch_errors = fetch_errors,
                "trigger_causal_discovery: Not enough valid memories after fetch"
            );
            return self.tool_result(
                id,
                json!({
                    "status": "completed",
                    "pairsAnalyzed": 0,
                    "relationshipsFound": 0,
                    "message": format!("Not enough valid memories (got {}, need 2+). Fetch errors: {}", memories_for_analysis.len(), fetch_errors),
                    "sessionScope": session_scope,
                    "dryRun": dry_run
                }),
            );
        }

        info!(
            valid_memories = memories_for_analysis.len(),
            fetch_errors = fetch_errors,
            "trigger_causal_discovery: Running discovery cycle"
        );

        // If dry run, just return what would be analyzed
        if dry_run {
            return self.tool_result(
                id,
                json!({
                    "status": "dry_run",
                    "mode": "pairs",
                    "memoriesAvailable": memories_for_analysis.len(),
                    "potentialPairs": memories_for_analysis.len() * (memories_for_analysis.len() - 1) / 2,
                    "maxPairsToAnalyze": max_memories.min(memories_for_analysis.len() * (memories_for_analysis.len() - 1) / 2),
                    "minConfidence": min_confidence,
                    "sessionScope": session_scope,
                    "dryRun": true,
                    "message": "Dry run completed - no relationships stored"
                }),
            );
        }

        // Run the discovery cycle
        let result = match service.run_discovery_cycle(&memories_for_analysis).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "trigger_causal_discovery: Discovery cycle failed");
                return self.tool_error(id, &format!("Discovery failed: {}", e));
            }
        };

        info!(
            candidates_found = result.candidates_found,
            relationships_confirmed = result.relationships_confirmed,
            relationships_rejected = result.relationships_rejected,
            duration_ms = result.duration.as_millis(),
            "trigger_causal_discovery: Complete"
        );

        // MCP-04 FIX: The "pairs" mode discovery cycle does not return per-relationship
        // confidence values, so we cannot compute a real average. Report null instead
        // of fabricating a value. The "extract" mode includes per-relationship confidence
        // in its response via the relationships array.
        let stats = service.activator_stats();

        self.tool_result(
            id,
            json!({
                "status": "completed",
                "pairsAnalyzed": result.candidates_found,
                "relationshipsFound": result.relationships_confirmed,
                "relationshipsRejected": result.relationships_rejected,
                "averageConfidence": serde_json::Value::Null,
                "edgesCreated": stats.edges_created,
                "embeddingsGenerated": stats.embeddings_generated,
                "durationMs": result.duration.as_millis(),
                "errors": result.errors,
                "sessionScope": session_scope,
                "dryRun": false
            }),
        )
    }

    /// get_causal_discovery_status tool implementation.
    ///
    /// Returns status and statistics of the causal discovery agent.
    ///
    /// # Parameters
    ///
    /// - `includeLastResult`: Include detailed results from last cycle (default: true)
    /// - `includeGraphStats`: Include causal graph statistics (default: true)
    pub(crate) async fn call_get_causal_discovery_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let include_last_result = args
            .get("includeLastResult")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_graph_stats = args
            .get("includeGraphStats")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        info!(
            include_last_result = include_last_result,
            include_graph_stats = include_graph_stats,
            "get_causal_discovery_status: Querying"
        );

        // Get service (None = LLM not loaded, degraded mode)
        let service = self.graph_discovery_service();
        let (service_status, is_running) = match service {
            Some(s) => (s.status(), s.is_running()),
            None => (context_graph_graph_agent::ServiceStatus::Stopped, false),
        };

        // Get LLM availability from causal hint provider
        let llm_available = self
            .causal_hint_provider
            .as_ref()
            .map(|p| p.is_available())
            .unwrap_or(false);

        // Build base response
        let status_str = match service_status {
            context_graph_graph_agent::ServiceStatus::Stopped => "stopped",
            context_graph_graph_agent::ServiceStatus::Starting => "starting",
            context_graph_graph_agent::ServiceStatus::Running => "running",
            context_graph_graph_agent::ServiceStatus::Stopping => "stopping",
        };

        // Count total stored causal relationships (covers both pairs and extract modes)
        let total_relationships_stored = match self
            .teleological_store
            .count_causal_relationships()
            .await
        {
            Ok(count) => count,
            Err(e) => {
                warn!(error = %e, "Failed to count causal relationships");
                0
            }
        };

        let mut response = json!({
            "agentStatus": if is_running { "running" } else { status_str },
            "llmAvailable": llm_available,
            "modelName": "Hermes-2-Pro-Mistral-7B",
            "estimatedVramMb": 6000,
            "pairsAnalyzedTotal": service.map(|s| s.scanner_analyzed_count()).unwrap_or(0),
            "totalRelationshipsStored": total_relationships_stored
        });

        // Include last cycle result if requested
        if include_last_result {
            if let Some(last_result) = service.and_then(|s| s.last_result()) {
                response["lastCycle"] = json!({
                    "startedAt": last_result.started_at.to_rfc3339(),
                    "completedAt": last_result.completed_at.to_rfc3339(),
                    "durationMs": last_result.duration.as_millis(),
                    "candidatesFound": last_result.candidates_found,
                    "relationshipsConfirmed": last_result.relationships_confirmed,
                    "relationshipsRejected": last_result.relationships_rejected,
                    "embeddingsGenerated": last_result.embeddings_generated,
                    "edgesCreated": last_result.edges_created,
                    "errors": last_result.errors
                });
            } else {
                response["lastCycle"] = json!(null);
            }
        }

        // Include graph stats if requested
        if include_graph_stats {
            if let Some(s) = service {
                let activator_stats = s.activator_stats();
                let graph = s.graph();
                let graph_read = graph.read();
                let edge_count = graph_read.edge_count();

                response["graphStats"] = json!({
                    "totalEdges": edge_count,
                    "totalEmbeddingsGenerated": activator_stats.embeddings_generated,
                    "totalEdgesCreated": activator_stats.edges_created,
                    "totalProcessed": activator_stats.processed,
                    "skippedLowConfidence": activator_stats.skipped_low_confidence,
                    "skippedExisting": activator_stats.skipped_existing,
                    "errors": activator_stats.errors
                });
            } else {
                response["graphStats"] = json!(null);
            }
        }

        info!(
            agent_status = status_str,
            llm_available = llm_available,
            "get_causal_discovery_status: Returning status"
        );

        self.tool_result(id, response)
    }

    /// Multi-relationship extraction mode for trigger_causal_discovery.
    ///
    /// Extracts ALL cause-effect relationships from each memory using the
    /// LLM's multi-relationship extraction, generates E5 dual embeddings,
    /// and stores each relationship.
    async fn trigger_causal_discovery_extract(
        &self,
        id: Option<JsonRpcId>,
        max_memories: usize,
        min_confidence: f32,
        dry_run: bool,
    ) -> JsonRpcResponse {
        // Check if causal hint provider is available
        let provider = match &self.causal_hint_provider {
            Some(p) if p.is_available() => p,
            _ => {
                return self.tool_error(
                    id,
                    "Causal hint provider not available - LLM not loaded",
                );
            }
        };

        // Get ALL memories from storage (not just file-indexed ones)
        // This includes both file chunks AND manually stored memories via store_memory
        let all_fingerprints = match self
            .teleological_store
            .scan_fingerprints_for_clustering(Some(max_memories))
            .await
        {
            Ok(fps) => fps,
            Err(e) => {
                error!(error = %e, "trigger_causal_discovery_extract: Failed to scan fingerprints");
                return self.tool_error(
                    id,
                    &format!("Failed to scan fingerprints for causal analysis: {}", e),
                );
            }
        };

        // Extract memory IDs from fingerprints
        let memory_ids: Vec<uuid::Uuid> = all_fingerprints.iter().map(|(id, _)| *id).collect();

        if memory_ids.is_empty() {
            return self.tool_result(
                id,
                json!({
                    "status": "completed",
                    "mode": "extract",
                    "memoriesAnalyzed": 0,
                    "relationshipsFound": 0,
                    "message": "No memories found for analysis - storage is empty",
                    "dryRun": dry_run
                }),
            );
        }

        info!(
            memory_count = memory_ids.len(),
            dry_run = dry_run,
            "trigger_causal_discovery_extract: Starting extraction"
        );

        // dry_run=true: perform full analysis but skip persistence (store operations)
        // This lets users validate LLM extraction logic without committing to storage.

        let mut memories_analyzed = 0;
        let mut total_relationships = 0;
        let mut content_fetch_errors = 0;
        let mut embedding_errors = 0;
        let mut fingerprint_errors = 0;
        let mut storage_errors = 0;
        // Track fallbacks (stored with degraded embeddings, not blocking errors)
        let mut source_e5_fallback_count = 0;
        let mut e8_fallback_count = 0;
        let mut e11_fallback_count = 0;
        // Gap 4: E5 pre-filter counter — tracks memories skipped as non-causal
        let mut e5_prefilter_skipped = 0usize;
        // Per-relationship detail accumulator for response
        let mut relationship_details: Vec<serde_json::Value> = Vec::new();
        let start_time = std::time::Instant::now();

        for memory_id in &memory_ids {
            // Get content for this memory
            // MED-7 FIX: Distinguish Ok(None) (content missing) from Err(e) (storage error).
            // Storage errors are fatal — fail fast per constitution.
            let content = match self.teleological_store.get_content(*memory_id).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    // Content missing — count but continue (non-fatal)
                    debug!(
                        memory_id = %memory_id,
                        "trigger_causal_discovery_extract: No content for memory (deleted or empty)"
                    );
                    content_fetch_errors += 1;
                    continue;
                }
                Err(e) => {
                    // Storage error — fail fast
                    error!(
                        error = %e,
                        memory_id = %memory_id,
                        "trigger_causal_discovery_extract: Content fetch FAILED — storage error"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Content fetch failed for memory {}: {}. NO FALLBACKS.",
                            memory_id, e
                        ),
                    );
                }
            };

            // Gap 4: E5 pre-filter — skip memories without causal language indicators
            // before expensive LLM extraction. Uses keyword-based intent detection
            // (97.5% accuracy) to avoid wasting LLM calls on non-causal content.
            let content_causal_signal = detect_causal_query_intent(&content);
            if content_causal_signal == CausalDirection::Unknown {
                debug!(
                    memory_id = %memory_id,
                    "trigger_causal_discovery_extract: E5 pre-filter skipped non-causal memory"
                );
                e5_prefilter_skipped += 1;
                continue;
            }

            // Extract all causal relationships from this memory
            let extracted = provider.extract_all_relationships(&content).await;
            memories_analyzed += 1;

            if extracted.is_empty() {
                debug!(
                    memory_id = %memory_id,
                    "trigger_causal_discovery_extract: No relationships found"
                );
                continue;
            }

            // Filter by confidence and store each relationship
            for relationship in extracted {
                if relationship.confidence < min_confidence {
                    continue;
                }

                // Generate E5 dual embeddings for the explanation
                let e5_result = self
                    .multi_array_provider
                    .embed_e5_dual(&relationship.explanation)
                    .await;

                let (e5_cause, e5_effect) = match e5_result {
                    Ok(dual) => dual,
                    Err(e) => {
                        warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "trigger_causal_discovery_extract: Failed to generate E5 embeddings"
                        );
                        embedding_errors += 1;
                        continue;
                    }
                };

                // Generate E1 semantic embedding
                let e1_result = self
                    .multi_array_provider
                    .embed_e1_only(&relationship.explanation)
                    .await;

                let e1_semantic = match e1_result {
                    Ok(emb) => emb,
                    Err(e) => {
                        warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "trigger_causal_discovery_extract: Failed to generate E1 embedding"
                        );
                        embedding_errors += 1;
                        continue;
                    }
                };

                // Generate source-anchored E5 embeddings to prevent explanation clustering
                // Per plan: Source content is unique per document, preventing LLM outputs from clustering
                let source_truncated: String = content.chars().take(500).collect();
                let source_anchored_text = format!(
                    "{} {} causes {}.",
                    source_truncated,
                    relationship.cause,
                    relationship.effect
                );

                let (e5_source_cause, e5_source_effect) = match self
                    .multi_array_provider
                    .embed_e5_dual(&source_anchored_text)
                    .await
                {
                    Ok(dual) => dual,
                    Err(e) => {
                        warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "trigger_causal_discovery_extract: Failed to generate source-anchored E5 embeddings - skipping relationship"
                        );
                        source_e5_fallback_count += 1;
                        // Skip this relationship - don't store with empty embeddings
                        continue;
                    }
                };

                // Generate E8 graph embeddings for causal structure search
                // Uses explanation text like E5, but captures graph connectivity patterns
                // E8 is a relational enhancer (ARCH-12) - critical for graph search
                let (e8_graph_source, e8_graph_target) = match self
                    .multi_array_provider
                    .embed_e8_dual(&relationship.explanation)
                    .await
                {
                    Ok(dual) => dual,
                    Err(e) => {
                        warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "trigger_causal_discovery_extract: Failed to generate E8 embeddings - skipping relationship"
                        );
                        e8_fallback_count += 1;
                        // Skip this relationship - don't store corrupted data (per plan: fix #3)
                        continue;
                    }
                };

                // Generate E11 KEPLER entity embedding for knowledge graph search
                // Concatenates cause|effect|explanation for entity context
                // KEPLER knows entity relationships that E1 misses (e.g., "Diesel" = Rust ORM)
                // E11 is a relational enhancer (ARCH-12) - critical for entity discovery
                let e11_entity_text = format!(
                    "{} | {} | {}",
                    relationship.cause,
                    relationship.effect,
                    relationship.explanation
                );
                let e11_entity = match self
                    .multi_array_provider
                    .embed_e11_only(&e11_entity_text)
                    .await
                {
                    Ok(emb) => emb,
                    Err(e) => {
                        warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "trigger_causal_discovery_extract: Failed to generate E11 embedding - skipping relationship"
                        );
                        e11_fallback_count += 1;
                        // Skip this relationship - don't store corrupted data (per plan: fix #3)
                        continue;
                    }
                };

                // Create and store the causal relationship with source-anchored, graph, and entity embeddings
                let causal_rel = context_graph_core::types::CausalRelationship::new(
                    relationship.cause.clone(),
                    relationship.effect.clone(),
                    relationship.explanation.clone(),
                    e5_cause,
                    e5_effect,
                    e1_semantic,
                    content.clone(),
                    *memory_id,
                    relationship.confidence,
                    relationship.mechanism_type.as_str().to_string(),
                )
                .with_source_embeddings(e5_source_cause, e5_source_effect)
                .with_graph_embeddings(e8_graph_source, e8_graph_target)
                .with_entity_embedding(e11_entity);

                // dry_run: analyze but don't persist. Live: store everything.
                if dry_run {
                    total_relationships += 1;
                    relationship_details.push(json!({
                        "id": format!("dry-run-{}", uuid::Uuid::new_v4()),
                        "sourceMemoryId": memory_id.to_string(),
                        "cause": relationship.cause,
                        "effect": relationship.effect,
                        "mechanismType": relationship.mechanism_type.as_str(),
                        "confidence": relationship.confidence,
                        "llmGuided": true,
                        "dryRun": true,
                    }));
                } else {
                    match self
                        .teleological_store
                        .store_causal_relationship(&causal_rel)
                        .await
                    {
                        Ok(causal_id) => {
                            debug!(
                                causal_id = %causal_id,
                                source_id = %memory_id,
                                cause = %causal_rel.cause_statement,
                                "trigger_causal_discovery_extract: Stored CausalRelationship"
                            );

                            // Emit RelationshipDiscovered audit (non-fatal)
                            {
                                let audit_record = AuditRecord::new(
                                    AuditOperation::RelationshipDiscovered {
                                        relationship_type: relationship.mechanism_type.as_str().to_string(),
                                        confidence: relationship.confidence,
                                    },
                                    causal_id,
                                )
                                .with_operator("trigger_causal_discovery")
                                .with_parameters(serde_json::json!({
                                    "mode": "extract",
                                    "source_memory_id": memory_id.to_string(),
                                    "cause_statement": relationship.cause.chars().take(100).collect::<String>(),
                                    "effect_statement": relationship.effect.chars().take(100).collect::<String>(),
                                }));

                                if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                                    error!(error = %e, "trigger_causal_discovery: Failed to write audit record (non-fatal)");
                                }
                            }

                            // Generate full 13-embedder fingerprint for the explanation
                            let metadata = EmbeddingMetadata::default().with_causal_hint(CausalHint {
                                is_causal: true,
                                direction_hint: CausalDirectionHint::Neutral,
                                confidence: relationship.confidence,
                                key_phrases: vec![
                                    relationship.cause.clone(),
                                    relationship.effect.clone(),
                                ],
                                description: Some(relationship.explanation.clone()),
                                cause_spans: Vec::new(),
                                effect_spans: Vec::new(),
                                asymmetry_strength: 0.0,
                            });

                            match self
                                .multi_array_provider
                                .embed_all_with_metadata(&relationship.explanation, metadata)
                                .await
                            {
                                Ok(embedding_output) => {
                                    use sha2::{Sha256, Digest};
                                    let mut hasher = Sha256::new();
                                    hasher.update(relationship.explanation.as_bytes());
                                    let content_hash: [u8; 32] = hasher.finalize().into();

                                    let fingerprint = TeleologicalFingerprint::new(
                                        embedding_output.fingerprint,
                                        content_hash,
                                    );

                                    let source_metadata = SourceMetadata::causal_explanation(
                                        *memory_id,
                                        causal_id,
                                        relationship.mechanism_type.as_str().to_string(),
                                        relationship.confidence,
                                    );

                                    match self.teleological_store.store(fingerprint.clone()).await {
                                        Ok(fp_id) => {
                                            if let Err(e) = self
                                                .teleological_store
                                                .store_content(fp_id, &relationship.explanation)
                                                .await
                                            {
                                                warn!(fp_id = %fp_id, error = %e, "Failed to store explanation content");
                                            }
                                            if let Err(e) = self
                                                .teleological_store
                                                .store_source_metadata(fp_id, &source_metadata)
                                                .await
                                            {
                                                warn!(fp_id = %fp_id, error = %e, "Failed to store source metadata");
                                            }
                                            debug!(fp_id = %fp_id, causal_id = %causal_id, "Stored 13-embedder fingerprint");
                                        }
                                        Err(e) => {
                                            warn!(causal_id = %causal_id, error = %e, "Failed to store 13-embedder fingerprint");
                                            fingerprint_errors += 1;
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(causal_id = %causal_id, error = %e, "Failed to generate 13-embedder fingerprint");
                                }
                            }

                            total_relationships += 1;

                            relationship_details.push(json!({
                                "id": causal_id.to_string(),
                                "sourceMemoryId": memory_id.to_string(),
                                "cause": relationship.cause,
                                "effect": relationship.effect,
                                "mechanismType": relationship.mechanism_type.as_str(),
                                "confidence": relationship.confidence,
                                "llmGuided": true,
                            }));
                        }
                        Err(e) => {
                            warn!(
                                memory_id = %memory_id,
                                error = %e,
                                "trigger_causal_discovery_extract: Failed to store relationship"
                            );
                            storage_errors += 1;
                        }
                    }
                }
            }
        }

        let duration = start_time.elapsed();
        let total_errors =
            content_fetch_errors + embedding_errors + fingerprint_errors + storage_errors;

        info!(
            memories_analyzed = memories_analyzed,
            relationships_found = total_relationships,
            content_fetch_errors = content_fetch_errors,
            embedding_errors = embedding_errors,
            fingerprint_errors = fingerprint_errors,
            storage_errors = storage_errors,
            total_errors = total_errors,
            source_e5_fallbacks = source_e5_fallback_count,
            e8_fallbacks = e8_fallback_count,
            e11_fallbacks = e11_fallback_count,
            e5_prefilter_skipped = e5_prefilter_skipped,
            duration_ms = duration.as_millis(),
            "trigger_causal_discovery_extract: Complete"
        );

        // Get LLM status from provider
        let llm_status = match &self.causal_hint_provider {
            Some(p) => {
                let status = p.last_extraction_status();
                json!({
                    "available": p.is_available(),
                    "lastStatus": status.as_str(),
                })
            }
            None => json!({
                "available": false,
                "lastStatus": "NoProvider",
            }),
        };

        self.tool_result(
            id,
            json!({
                "status": "completed",
                "mode": "extract",
                "memoriesAnalyzed": memories_analyzed,
                "relationshipsFound": total_relationships,
                "relationships": relationship_details,
                "minConfidence": min_confidence,
                "errorBreakdown": {
                    "contentFetch": content_fetch_errors,
                    "embedding": embedding_errors,
                    "fingerprint": fingerprint_errors,
                    "storage": storage_errors,
                    "total": total_errors
                },
                "degradedEmbeddings": {
                    "sourceE5Fallbacks": source_e5_fallback_count,
                    "e8Fallbacks": e8_fallback_count,
                    "e11Fallbacks": e11_fallback_count,
                    "total": source_e5_fallback_count + e8_fallback_count + e11_fallback_count
                },
                "e5PrefilterSkipped": e5_prefilter_skipped,
                "llmStatus": llm_status,
                "durationMs": duration.as_millis(),
                "dryRun": dry_run
            }),
        )
    }
}
