//! Topic tool handlers.
//!
//! Per PRD Section 10.2, implements:
//! - get_topic_portfolio: Get all discovered topics with profiles
//! - get_topic_stability: Get portfolio-level stability metrics
//! - detect_topics: Force topic detection recalculation
//! - get_divergence_alerts: Check for divergence from recent activity
//!
//! Constitution Compliance:
//! - AP-60: Temporal embedders (E2-E4) weight = 0.0 in topic detection
//! - AP-62: Only SEMANTIC embedders for divergence alerts
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//!
//! TASK-INTEG-TOPIC: Integrated with MultiSpaceClusterManager and TopicStabilityTracker.

use chrono::Utc;
use tracing::{debug, error, info, warn};

use context_graph_core::clustering::Topic;
use context_graph_core::retrieval::config::low_thresholds;
use context_graph_core::retrieval::divergence::DIVERGENCE_SPACES;
use context_graph_core::teleological::Embedder;
use context_graph_core::traits::TeleologicalSearchOptions;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::topic_dtos::{
    DetectTopicsRequest, DetectTopicsResponse, DivergenceAlert, DivergenceAlertsResponse,
    GetDivergenceAlertsRequest, GetTopicPortfolioRequest, GetTopicStabilityRequest, PhaseBreakdown,
    StabilityMetricsSummary, TopicPortfolioResponse, TopicStabilityResponse, TopicSummary,
};

/// Minimum memories required for clustering (per constitution min_cluster_size).
const MIN_MEMORIES_FOR_CLUSTERING: usize = 3;

/// Minimum memories required for divergence detection.
const MIN_MEMORIES_FOR_DIVERGENCE: usize = 2;

/// Maximum words in memory summary for divergence alerts.
const MAX_SUMMARY_WORDS: usize = 50;

/// Convert Embedder to DTO semantic space string.
///
/// Maps embedder variants to the format used in DivergenceAlert DTO.
/// Per AP-62, only SEMANTIC embedders should be passed here.
fn embedder_to_dto_space(embedder: Embedder) -> &'static str {
    match embedder {
        Embedder::Semantic => "E1_Semantic",
        Embedder::Causal => "E5_Causal",
        Embedder::Sparse => "E6_Sparse",
        Embedder::Code => "E7_Code",
        Embedder::Multimodal => "E10_Multimodal",
        Embedder::LateInteraction => "E12_LateInteraction",
        Embedder::KeywordSplade => "E13_SPLADE",
        // Temporal, Relational, Structural embedders should never reach here per AP-62
        _ => {
            warn!(
                embedder = ?embedder,
                "Non-semantic embedder passed to divergence detection (AP-62 violation)"
            );
            "Unknown"
        }
    }
}

/// Truncate content to max_words for divergence alert summaries.
fn truncate_summary(content: &str, max_words: usize) -> String {
    content
        .split_whitespace()
        .take(max_words)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert core Topic to TopicSummary DTO.
///
/// TASK-INTEG-TOPIC: Helper for converting between core and DTO types.
fn topic_to_summary(topic: &Topic) -> TopicSummary {
    let weighted_agreement = topic.profile.weighted_agreement();
    let confidence = TopicSummary::compute_confidence(weighted_agreement);
    // Convert Embedder enum variants to human-readable names
    let contributing_spaces: Vec<String> = topic
        .contributing_spaces
        .iter()
        .map(|e| format!("{:?}", e))
        .collect();
    let phase_str = format!("{:?}", topic.stability.phase);

    TopicSummary {
        id: topic.id,
        name: topic.name.clone(),
        confidence,
        weighted_agreement,
        member_count: topic.member_count(),
        contributing_spaces,
        phase: phase_str,
    }
}


/// Compute phase breakdown from topics.
///
/// TASK-INTEG-TOPIC: Helper to count topics by lifecycle phase.
fn compute_phase_breakdown(
    topics: &std::collections::HashMap<uuid::Uuid, Topic>,
) -> PhaseBreakdown {
    use context_graph_core::clustering::TopicPhase;

    let mut emerging = 0;
    let mut stable = 0;
    let mut declining = 0;
    let mut merging = 0;

    for topic in topics.values() {
        match topic.stability.phase {
            TopicPhase::Emerging => emerging += 1,
            TopicPhase::Stable => stable += 1,
            TopicPhase::Declining => declining += 1,
            TopicPhase::Merging => merging += 1,
        }
    }

    PhaseBreakdown {
        emerging,
        stable,
        declining,
        merging,
    }
}

impl Handlers {
    /// Handle get_topic_portfolio tool call.
    ///
    /// Returns discovered topics with profiles, stability metrics, and tier info.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (format: brief|standard|verbose)
    ///
    /// # Returns
    /// JsonRpcResponse with TopicPortfolioResponse
    ///
    /// # Implements
    /// REQ-MCP-002, REQ-MCP-004
    ///
    /// # Constitution Compliance
    /// - ARCH-09: Topic threshold is weighted_agreement >= 2.5
    /// - AP-60: Temporal embedders (E2-E4) weight = 0.0
    pub(crate) async fn call_get_topic_portfolio(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_topic_portfolio");

        // Parse request
        let request: GetTopicPortfolioRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "get_topic_portfolio: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Validate format using DTO's validate method
        if let Err(validation_error) = request.validate() {
            error!(error = %validation_error, "get_topic_portfolio: Validation failed");
            return self
                .tool_error(id, &format!("Invalid params: {}", validation_error));
        }

        // Get memory count to determine tier
        let memory_count = match self.teleological_store.count().await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "get_topic_portfolio: Failed to get memory count");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to get memory count: {}", e),
                );
            }
        };

        let tier = TopicPortfolioResponse::tier_for_memory_count(memory_count);

        debug!(
            memory_count = memory_count,
            tier = tier,
            format = %request.format,
            "get_topic_portfolio: Retrieved memory count and tier"
        );

        // Tier 0: No memories, return empty response
        if tier == 0 {
            debug!("get_topic_portfolio: Tier 0 - returning empty portfolio");
            let response = TopicPortfolioResponse::empty_tier_0();
            return self.tool_result(
                id,
                serde_json::to_value(response).expect("TopicPortfolioResponse should serialize"),
            );
        }

        // TASK-INTEG-TOPIC: Get topics from cluster_manager
        let cluster_manager = self.cluster_manager.read();
        let topics_map = cluster_manager.get_topics();

        // Convert core Topics to TopicSummary DTOs
        let topics: Vec<TopicSummary> = topics_map.values().map(topic_to_summary).collect();

        let total_topics = topics.len();

        // Get stability metrics from stability_tracker
        let stability_tracker = self.stability_tracker.read();
        let churn_rate = stability_tracker.current_churn();

        let response = TopicPortfolioResponse {
            topics,
            stability: StabilityMetricsSummary::new(churn_rate, 0.0),
            total_topics,
            tier,
        };

        info!(
            tier = tier,
            total_topics = response.total_topics,
            churn_rate = churn_rate,
            is_stable = response.stability.is_stable,
            "get_topic_portfolio: Returning portfolio with {} topics",
            total_topics
        );

        self.tool_result(
            id,
            serde_json::to_value(response).expect("TopicPortfolioResponse should serialize"),
        )
    }

    /// Handle get_topic_stability tool call.
    ///
    /// Returns stability metrics including churn, entropy, and phase breakdown.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (hours: lookback period)
    ///
    /// # Returns
    /// JsonRpcResponse with TopicStabilityResponse
    ///
    /// # Implements
    /// REQ-MCP-002
    pub(crate) async fn call_get_topic_stability(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_topic_stability");

        // Parse request
        let request: GetTopicStabilityRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "get_topic_stability: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Validate hours range using DTO's validate method
        if let Err(validation_error) = request.validate() {
            error!(error = %validation_error, "get_topic_stability: Validation failed");
            return self
                .tool_error(id, &format!("Invalid params: {}", validation_error));
        }

        debug!(
            hours = request.hours,
            "get_topic_stability: Processing stability request"
        );

        // TASK-INTEG-TOPIC: Get real stability metrics from stability_tracker
        let stability_tracker = self.stability_tracker.read();
        let churn_rate = stability_tracker.current_churn();
        let average_churn = stability_tracker.average_churn(request.hours as i64);

        // Entropy is no longer tracked via UTL processor
        let entropy = 0.0_f32;

        let high_churn_warning = TopicStabilityResponse::is_high_churn(churn_rate);

        // Get phase breakdown from cluster manager topics
        let cluster_manager = self.cluster_manager.read();
        let topics = cluster_manager.get_topics();
        let phases = compute_phase_breakdown(topics);

        let response = TopicStabilityResponse {
            churn_rate,
            entropy,
            phases,
            high_churn_warning,
            average_churn,
        };

        info!(
            churn_rate = response.churn_rate,
            entropy = response.entropy,
            average_churn = response.average_churn,
            high_churn_warning = response.high_churn_warning,
            "get_topic_stability: Returning stability response"
        );

        self.tool_result(
            id,
            serde_json::to_value(response).expect("TopicStabilityResponse should serialize"),
        )
    }

    /// Handle detect_topics tool call.
    ///
    /// Triggers topic detection/clustering. Requires minimum 3 memories.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (force: force detection)
    ///
    /// # Returns
    /// JsonRpcResponse with DetectTopicsResponse
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-003
    ///
    /// # Constitution Compliance
    /// - min_cluster_size: 3 (per clustering.parameters.min_cluster_size)
    /// - ARCH-09: Topic threshold is weighted_agreement >= 2.5
    pub(crate) async fn call_detect_topics(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling detect_topics");

        // Parse request
        let request: DetectTopicsRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "detect_topics: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        debug!(force = request.force, "detect_topics: Processing request");

        // Check minimum memory count
        let memory_count = match self.teleological_store.count().await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "detect_topics: Failed to get memory count");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to get memory count: {}", e),
                );
            }
        };

        if memory_count < MIN_MEMORIES_FOR_CLUSTERING {
            warn!(
                memory_count = memory_count,
                min_required = MIN_MEMORIES_FOR_CLUSTERING,
                "detect_topics: Insufficient memories for clustering"
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INSUFFICIENT_MEMORIES,
                format!(
                    "Need >= {} memories for topic detection (have {})",
                    MIN_MEMORIES_FOR_CLUSTERING, memory_count
                ),
            );
        }

        debug!(
            memory_count = memory_count,
            force = request.force,
            "detect_topics: Starting topic detection"
        );

        // FIX-BUG-001: Load all fingerprints from storage into cluster_manager BEFORE reclustering.
        // Previously, the cluster_manager only contained fingerprints added during the current session
        // via inject_context/store_memory, missing all existing fingerprints in storage.
        info!("detect_topics: Loading all fingerprints from storage for clustering...");

        let fingerprints = match self.teleological_store.scan_fingerprints_for_clustering(None).await {
            Ok(fps) => fps,
            Err(e) => {
                error!(error = %e, "detect_topics: Failed to scan fingerprints from storage");
                return self.tool_error(
                    id,
                    &format!("Storage error: Failed to scan fingerprints: {}", e),
                );
            }
        };

        info!(
            fingerprint_count = fingerprints.len(),
            "detect_topics: Scanned fingerprints from storage"
        );

        // TASK-INTEG-TOPIC: Trigger reclustering via cluster_manager
        let mut cluster_manager = self.cluster_manager.write();

        // Clear existing data in cluster_manager and load all fingerprints from storage
        // This ensures we cluster ALL fingerprints, not just those added during this session.
        cluster_manager.clear_all_spaces();

        let mut insert_errors = 0;
        for (fp_id, cluster_array) in &fingerprints {
            if let Err(e) = cluster_manager.insert(*fp_id, cluster_array) {
                warn!(
                    fingerprint_id = %fp_id,
                    error = %e,
                    "detect_topics: Failed to insert fingerprint into cluster_manager"
                );
                insert_errors += 1;
            }
        }

        if insert_errors > 0 {
            warn!(
                insert_errors = insert_errors,
                total = fingerprints.len(),
                "detect_topics: Some fingerprints failed to insert"
            );
        }

        info!(
            inserted = fingerprints.len() - insert_errors,
            errors = insert_errors,
            "detect_topics: Loaded fingerprints into cluster_manager"
        );

        // Track topics before reclustering
        let topics_before: std::collections::HashSet<uuid::Uuid> =
            cluster_manager.get_topics().keys().cloned().collect();

        // Run HDBSCAN reclustering
        match cluster_manager.recluster() {
            Ok(result) => {
                // Track topics after reclustering
                let topics_after = cluster_manager.get_topics();
                let total_after = topics_after.len();

                // Find new topics (in after but not in before)
                let new_topics: Vec<TopicSummary> = topics_after
                    .values()
                    .filter(|t| !topics_before.contains(&t.id))
                    .map(topic_to_summary)
                    .collect();

                // Note: Merged topics detection would require tracking merge operations
                // For now, we report new topics only
                let merged_topics = vec![];

                info!(
                    new_topics = new_topics.len(),
                    total_after = total_after,
                    clusters_found = result.total_clusters,
                    "detect_topics: Reclustering completed successfully"
                );

                let response = DetectTopicsResponse {
                    new_topics,
                    merged_topics,
                    total_after,
                    message: Some(format!(
                        "Topic detection completed - {} clusters across 13 spaces, {} topics with weighted_agreement >= 2.5",
                        result.total_clusters, total_after
                    )),
                };

                self.tool_result(
                    id,
                    serde_json::to_value(response).expect("DetectTopicsResponse should serialize"),
                )
            }
            Err(e) => {
                error!(error = %e, "detect_topics: Reclustering failed");
                self.tool_error(id, &format!("Clustering error: {}", e))
            }
        }
    }

    /// Handle get_divergence_alerts tool call.
    ///
    /// Checks for divergence from recent activity using SEMANTIC embedders ONLY.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (lookback_hours)
    ///
    /// # Returns
    /// JsonRpcResponse with DivergenceAlertsResponse
    ///
    /// # Implements
    /// REQ-MCP-002, REQ-MCP-005
    ///
    /// # Constitution Compliance
    /// - AP-62: Only SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13) trigger alerts
    /// - AP-63: Temporal embedders (E2-E4) NEVER trigger divergence alerts
    /// - ARCH-10: Divergence detection uses SEMANTIC embedders only
    pub(crate) async fn call_get_divergence_alerts(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_divergence_alerts");

        // Parse request
        let request: GetDivergenceAlertsRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "get_divergence_alerts: Failed to parse request");
                return self.tool_error(id, &format!("Invalid params: {}", e));
            }
        };

        // Validate lookback range using DTO's validate method
        if let Err(validation_error) = request.validate() {
            error!(error = %validation_error, "get_divergence_alerts: Validation failed");
            return self
                .tool_error(id, &format!("Invalid params: {}", validation_error));
        }

        let lookback_hours = request.lookback_hours as i64;
        debug!(
            lookback_hours = lookback_hours,
            "get_divergence_alerts: Processing request"
        );

        // Per AP-62: Only E1, E5, E6, E7, E10, E12, E13 trigger divergence alerts
        // Temporal embedders (E2-E4) are explicitly excluded per AP-63
        // Divergence detection compares current context against recent memories
        // using only SEMANTIC embedders

        // Step 1: Generate a broad query embedding to find recent memories
        // Using a neutral/common phrase that should have reasonable matches
        let broad_query = match self.multi_array_provider.embed_all("context memory").await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "get_divergence_alerts: Failed to generate query embedding");
                return self.tool_error(id, &format!("Embedding error: {}", e));
            }
        };

        let search_options = TeleologicalSearchOptions::quick(100)
            .with_min_similarity(0.0)
            .with_include_content(true);

        let all_results = match self
            .teleological_store
            .search_semantic(&broad_query, search_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "get_divergence_alerts: Failed to search memories");
                return self.tool_error(id, &format!("Search error: {}", e));
            }
        };

        // Step 2: Filter to memories within lookback window
        let cutoff = Utc::now() - chrono::Duration::hours(lookback_hours);
        let mut recent: Vec<_> = all_results
            .into_iter()
            .filter(|r| r.fingerprint.created_at >= cutoff)
            .collect();

        // Need at least 2 memories for divergence detection
        if recent.len() < MIN_MEMORIES_FOR_DIVERGENCE {
            debug!(
                memory_count = recent.len(),
                min_required = MIN_MEMORIES_FOR_DIVERGENCE,
                "get_divergence_alerts: Insufficient memories for divergence detection"
            );
            let response = DivergenceAlertsResponse::no_alerts();
            return self.tool_result(
                id,
                serde_json::to_value(response).expect("DivergenceAlertsResponse should serialize"),
            );
        }

        // Step 3: Sort by created_at descending (most recent first)
        recent.sort_by(|a, b| b.fingerprint.created_at.cmp(&a.fingerprint.created_at));

        // Step 4: Use the most recent memory as the "current" query
        let current = recent.remove(0);
        let current_semantic = &current.fingerprint.semantic;
        let current_id = current.fingerprint.id;

        debug!(
            current_id = %current_id,
            comparison_count = recent.len(),
            "get_divergence_alerts: Comparing most recent memory against {} others",
            recent.len()
        );

        // Step 5: Search again using the most recent memory as the query
        // This gives us similarity scores against the "current" context
        let comparison_options = TeleologicalSearchOptions::quick(50)
            .with_min_similarity(0.0)
            .with_include_content(true);

        let comparison_results = match self
            .teleological_store
            .search_semantic(current_semantic, comparison_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "get_divergence_alerts: Failed to search for comparisons");
                return self.tool_error(id, &format!("Search error: {}", e));
            }
        };

        // Step 6: Filter comparison results to lookback window (excluding the current memory)
        let comparisons: Vec<_> = comparison_results
            .into_iter()
            .filter(|r| r.fingerprint.id != current_id && r.fingerprint.created_at >= cutoff)
            .collect();

        // Step 7: Check each SEMANTIC embedding space for divergence
        let low = low_thresholds();
        let mut alerts: Vec<DivergenceAlert> = Vec::new();

        for result in comparisons {
            // Check each SEMANTIC embedder (per AP-62)
            for &embedder in &DIVERGENCE_SPACES {
                let idx = embedder.index();
                let score = result.embedder_scores[idx];
                let threshold = low.get_threshold(embedder);

                // Alert if score is BELOW low threshold (divergent)
                if score < threshold {
                    let summary = result
                        .content
                        .as_ref()
                        .map(|c| truncate_summary(c, MAX_SUMMARY_WORDS))
                        .unwrap_or_else(|| format!("Memory {}", result.fingerprint.id));

                    alerts.push(DivergenceAlert {
                        semantic_space: embedder_to_dto_space(embedder).to_string(),
                        similarity_score: score,
                        recent_memory_summary: summary,
                        threshold,
                    });
                }
            }
        }

        // Step 8: Compute severity and build response
        let severity = DivergenceAlertsResponse::compute_severity(&alerts);
        let response = DivergenceAlertsResponse { alerts, severity };

        info!(
            alert_count = response.alerts.len(),
            severity = %response.severity,
            lookback_hours = lookback_hours,
            "get_divergence_alerts: Detected {} divergence alerts with severity '{}'",
            response.alerts.len(),
            response.severity
        );

        self.tool_result(
            id,
            serde_json::to_value(response).expect("DivergenceAlertsResponse should serialize"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::topic_dtos::{DivergenceAlert, MAX_WEIGHTED_AGREEMENT, TOPIC_THRESHOLD};
    use super::*;

    #[test]
    fn test_constants_match_constitution() {
        // ARCH-09: Topic threshold is weighted_agreement >= 2.5
        assert!(
            (TOPIC_THRESHOLD - 2.5).abs() < f32::EPSILON,
            "TOPIC_THRESHOLD must be 2.5 per ARCH-09"
        );

        // Max weighted agreement = 7*1.0 + 2*0.5 + 1*0.5 = 8.5
        assert!(
            (MAX_WEIGHTED_AGREEMENT - 8.5).abs() < f32::EPSILON,
            "MAX_WEIGHTED_AGREEMENT must be 8.5"
        );

        // Minimum memories for clustering = 3 per constitution
        assert_eq!(
            MIN_MEMORIES_FOR_CLUSTERING, 3,
            "MIN_MEMORIES_FOR_CLUSTERING must be 3"
        );
    }

    #[test]
    fn test_semantic_embedders_for_divergence() {
        // Per AP-62: SEMANTIC embedders for divergence detection
        let spaces = DivergenceAlert::VALID_SEMANTIC_SPACES;
        assert_eq!(spaces.len(), 7, "Should have 7 SEMANTIC embedders");

        // Per AP-63: Temporal embedders must NOT be in the list
        assert!(
            !DivergenceAlert::is_valid_semantic_space("E2_TemporalRecent"),
            "Must NOT include E2 (temporal)"
        );
        assert!(
            !DivergenceAlert::is_valid_semantic_space("E3_TemporalPeriodic"),
            "Must NOT include E3 (temporal)"
        );
        assert!(
            !DivergenceAlert::is_valid_semantic_space("E4_TemporalPositional"),
            "Must NOT include E4 (temporal)"
        );
    }

    #[test]
    fn test_insufficient_memories_error_code() {
        // Per TECH_SPEC_PRD_GAPS.md Section 11.1
        assert_eq!(
            error_codes::INSUFFICIENT_MEMORIES,
            -32021,
            "INSUFFICIENT_MEMORIES error code must be -32021"
        );
    }

    // =========================================================================
    // Divergence Detection Helper Tests
    // =========================================================================

    #[test]
    fn test_embedder_to_dto_space() {
        // Verify all SEMANTIC embedders map correctly
        assert_eq!(embedder_to_dto_space(Embedder::Semantic), "E1_Semantic");
        assert_eq!(embedder_to_dto_space(Embedder::Causal), "E5_Causal");
        assert_eq!(embedder_to_dto_space(Embedder::Sparse), "E6_Sparse");
        assert_eq!(embedder_to_dto_space(Embedder::Code), "E7_Code");
        assert_eq!(
            embedder_to_dto_space(Embedder::Multimodal),
            "E10_Multimodal"
        );
        assert_eq!(
            embedder_to_dto_space(Embedder::LateInteraction),
            "E12_LateInteraction"
        );
        assert_eq!(embedder_to_dto_space(Embedder::KeywordSplade), "E13_SPLADE");
    }

    #[test]
    fn test_embedder_to_dto_space_non_semantic() {
        // Non-semantic embedders should return "Unknown" (with warning)
        // These should never be used in divergence detection per AP-62
        assert_eq!(embedder_to_dto_space(Embedder::TemporalRecent), "Unknown");
        assert_eq!(embedder_to_dto_space(Embedder::TemporalPeriodic), "Unknown");
        assert_eq!(
            embedder_to_dto_space(Embedder::TemporalPositional),
            "Unknown"
        );
    }

    #[test]
    fn test_truncate_summary_short() {
        let content = "Hello world";
        assert_eq!(truncate_summary(content, 50), "Hello world");
    }

    #[test]
    fn test_truncate_summary_long() {
        let content =
            "This is a very long piece of text that should be truncated to only a few words";
        let truncated = truncate_summary(content, 5);
        assert_eq!(truncated, "This is a very long");
        assert!(!truncated.contains("truncated"));
    }

    #[test]
    fn test_truncate_summary_empty() {
        let content = "";
        assert_eq!(truncate_summary(content, 50), "");
    }

    #[test]
    fn test_min_memories_for_divergence() {
        // Need at least 2 memories to detect divergence (1 current, 1 to compare)
        assert_eq!(
            MIN_MEMORIES_FOR_DIVERGENCE, 2,
            "MIN_MEMORIES_FOR_DIVERGENCE must be 2"
        );
    }

    #[test]
    fn test_max_summary_words() {
        // Verify summary truncation limit
        assert_eq!(MAX_SUMMARY_WORDS, 50, "MAX_SUMMARY_WORDS should be 50");
    }

    #[test]
    fn test_divergence_spaces_match_ap62() {
        // AP-62: Only SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13) trigger alerts
        assert_eq!(
            DIVERGENCE_SPACES.len(),
            7,
            "Should have 7 DIVERGENCE_SPACES"
        );

        // Verify each expected embedder is present
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::Semantic),
            "Must include E1"
        );
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::Causal),
            "Must include E5"
        );
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::Sparse),
            "Must include E6"
        );
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::Code),
            "Must include E7"
        );
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::Multimodal),
            "Must include E10"
        );
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::LateInteraction),
            "Must include E12"
        );
        assert!(
            DIVERGENCE_SPACES.contains(&Embedder::KeywordSplade),
            "Must include E13"
        );

        // AP-63: Temporal embedders must NOT be included
        assert!(
            !DIVERGENCE_SPACES.contains(&Embedder::TemporalRecent),
            "Must NOT include E2"
        );
        assert!(
            !DIVERGENCE_SPACES.contains(&Embedder::TemporalPeriodic),
            "Must NOT include E3"
        );
        assert!(
            !DIVERGENCE_SPACES.contains(&Embedder::TemporalPositional),
            "Must NOT include E4"
        );
    }

    #[test]
    fn test_low_thresholds_for_divergence() {
        // Verify low thresholds are configured for divergence detection
        let low = low_thresholds();

        // All thresholds should be in valid range [0.0, 1.0]
        for embedder in Embedder::all() {
            let threshold = low.get_threshold(embedder);
            assert!(
                threshold >= 0.0 && threshold <= 1.0,
                "{:?} threshold {} out of range",
                embedder,
                threshold
            );
        }

        // Verify specific thresholds from TECH-PHASE3-SIMILARITY-DIVERGENCE.md
        assert!((low.get_threshold(Embedder::Semantic) - 0.30).abs() < 0.01);
        assert!((low.get_threshold(Embedder::Causal) - 0.25).abs() < 0.01);
        assert!((low.get_threshold(Embedder::Code) - 0.35).abs() < 0.01);
    }
}
