//! Temporal tool implementations (search_recent, search_periodic).
//!
//! Per Constitution v6.5 and ARCH-25:
//! - E2 (V_freshness) finds recency patterns
//! - E3 (V_periodicity) finds time-of-day and day-of-week patterns
//! - Temporal boost is POST-RETRIEVAL only, NOT in similarity fusion
//!
//! Tools:
//! - search_recent: E2 freshness boost with configurable decay functions
//! - search_periodic: E3 periodic pattern boost for hour/day-of-week matching

use chrono::{Datelike, Timelike, Utc};
use tracing::{debug, error};

use context_graph_core::traits::{DecayFunction, SearchStrategy, TeleologicalSearchOptions};
use context_graph_storage::teleological::search::temporal_boost::{
    compute_e3_periodic_score, compute_periodic_match_fallback,
};

use super::temporal_dtos::{
    compute_recency_score, day_name, format_age, PeriodicConfigSummary, PeriodicSearchResultEntry,
    SearchPeriodicParams, SearchPeriodicResponse, SearchRecentParams, SearchRecentResponse,
    TemporalConfigSummary, TemporalSearchResultEntry,
};
use crate::handlers::core::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

impl Handlers {
    /// search_recent tool implementation.
    ///
    /// Searches with E2 temporal boost applied POST-RETRIEVAL per ARCH-25.
    /// Returns results sorted by boosted score (semantic * temporal boost).
    pub(crate) async fn call_search_recent(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse parameters
        let params: SearchRecentParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "search_recent: Parameter parsing FAILED");
                return self.tool_error(id, &format!("Invalid parameters: {}", e));
            }
        };

        if params.query.is_empty() {
            return self.tool_error(id, "Query cannot be empty");
        }

        // Clamp temporal weight
        let temporal_weight = params.temporal_weight.clamp(0.1, 1.0);
        let decay_function: DecayFunction = params.decay_function.into();

        debug!(
            query_preview = %params.query.chars().take(50).collect::<String>(),
            top_k = params.top_k,
            temporal_weight = temporal_weight,
            decay_function = ?decay_function,
            temporal_scale = ?params.temporal_scale,
            "search_recent: Starting temporal search"
        );

        // Generate query embedding
        let query_embedding = match self.multi_array_provider.embed_all(&params.query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_recent: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Build search options - use E1Only strategy for base search
        let mut options = TeleologicalSearchOptions::default();
        options.top_k = params.top_k * 2; // Over-fetch for temporal reranking
        options.min_similarity = params.min_similarity;
        options.strategy = SearchStrategy::E1Only;
        options.include_content = params.include_content;

        // Run base semantic search
        let results = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "search_recent: Search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        if results.is_empty() {
            return self.tool_result(
                id,
                serde_json::to_value(SearchRecentResponse {
                    query: params.query,
                    results: vec![],
                    count: 0,
                    temporal_config: TemporalConfigSummary {
                        temporal_weight,
                        decay_function: format!("{:?}", decay_function),
                        temporal_scale: format!("{:?}", params.temporal_scale),
                        horizon_seconds: params.temporal_scale.horizon_seconds(),
                        half_life_seconds: params.temporal_scale.horizon_seconds() as f64 / 4.0,
                        boost_formula: "clamp(1 + weight * (recency - 0.5), 0.8, 1.2)".to_string(),
                    },
                })
                .unwrap(),
            );
        }

        // Apply temporal boost POST-RETRIEVAL
        let now_ms = Utc::now().timestamp_millis();
        let horizon_secs = params.temporal_scale.horizon_seconds();

        let mut boosted_results: Vec<TemporalSearchResultEntry> = results
            .into_iter()
            .map(|r| {
                let memory_ts = r.fingerprint.created_at.timestamp_millis();
                let recency_score =
                    compute_recency_score(memory_ts, now_ms, decay_function, horizon_secs);

                // Per ARCH-25: Temporal boost is multiplicative POST-retrieval
                // boosted_score = semantic_score * (1 + temporal_weight * (recency_score - 0.5))
                let boost_factor = (1.0 + temporal_weight * (recency_score - 0.5)).clamp(0.8, 1.2);
                let final_score = r.similarity * boost_factor;

                let age_description = format_age(memory_ts, now_ms);
                let created_at = r.fingerprint.created_at.to_rfc3339();

                TemporalSearchResultEntry {
                    id: r.fingerprint.id.to_string(),
                    semantic_score: r.similarity,
                    recency_score,
                    final_score,
                    age_description,
                    content: r.content,
                    created_at,
                }
            })
            .collect();

        // Sort by final score descending
        boosted_results.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to requested top_k
        boosted_results.truncate(params.top_k);
        let count = boosted_results.len();

        debug!(
            result_count = count,
            "search_recent: Temporal search complete"
        );

        // Build response
        let response = SearchRecentResponse {
            query: params.query,
            results: boosted_results,
            count,
            temporal_config: TemporalConfigSummary {
                temporal_weight,
                decay_function: format!("{:?}", decay_function),
                temporal_scale: format!("{:?}", params.temporal_scale),
                horizon_seconds: params.temporal_scale.horizon_seconds(),
                half_life_seconds: params.temporal_scale.horizon_seconds() as f64 / 4.0,
                boost_formula: "clamp(1 + weight * (recency - 0.5), 0.8, 1.2)".to_string(),
            },
        };

        match serde_json::to_value(&response) {
            Ok(json) => self.tool_result(id, json),
            Err(e) => {
                error!(error = %e, "search_recent: Response serialization FAILED");
                self.tool_error(id, &format!("Response serialization failed: {}", e))
            }
        }
    }

    /// search_periodic tool implementation.
    ///
    /// Searches with E3 periodic boost applied POST-RETRIEVAL per ARCH-25.
    /// Returns results sorted by boosted score (semantic * periodic boost).
    pub(crate) async fn call_search_periodic(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse parameters
        let params: SearchPeriodicParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "search_periodic: Parameter parsing FAILED");
                return self.tool_error(id, &format!("Invalid parameters: {}", e));
            }
        };

        if params.query.is_empty() {
            return self.tool_error(id, "Query cannot be empty");
        }

        // Validate hour if provided
        if let Some(hour) = params.target_hour {
            if hour > 23 {
                error!(
                    hour = hour,
                    "search_periodic: targetHour validation FAILED - must be 0-23"
                );
                return self.tool_error(
                    id,
                    &format!("targetHour must be 0-23, got {}", hour),
                );
            }
        }

        // Validate day of week if provided
        if let Some(dow) = params.target_day_of_week {
            if dow > 6 {
                error!(
                    dow = dow,
                    "search_periodic: targetDayOfWeek validation FAILED - must be 0-6"
                );
                return self.tool_error(
                    id,
                    &format!("targetDayOfWeek must be 0-6 (Sun-Sat), got {}", dow),
                );
            }
        }

        // Clamp periodic weight
        let periodic_weight = params.periodic_weight.clamp(0.1, 1.0);

        // Compute effective targets (auto-detect if needed)
        let now = Utc::now();
        let effective_hour = if params.auto_detect && params.target_hour.is_none() {
            Some(now.hour() as u8)
        } else {
            params.target_hour
        };
        let effective_dow = if params.auto_detect && params.target_day_of_week.is_none() {
            Some(now.weekday().num_days_from_sunday() as u8)
        } else {
            params.target_day_of_week
        };

        debug!(
            query_preview = %params.query.chars().take(50).collect::<String>(),
            top_k = params.top_k,
            effective_hour = ?effective_hour,
            effective_dow = ?effective_dow,
            periodic_weight = periodic_weight,
            auto_detect = params.auto_detect,
            "search_periodic: Starting periodic search"
        );

        // Generate query embedding
        let query_embedding = match self.multi_array_provider.embed_all(&params.query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_periodic: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Build search options - use E1Only strategy for base search
        let mut options = TeleologicalSearchOptions::default();
        options.top_k = params.top_k * 2; // Over-fetch for periodic reranking
        options.min_similarity = params.min_similarity;
        options.strategy = SearchStrategy::E1Only;
        options.include_content = params.include_content;

        // Run base semantic search
        let results = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "search_periodic: Search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        if results.is_empty() {
            return self.tool_result(
                id,
                serde_json::to_value(SearchPeriodicResponse {
                    query: params.query,
                    results: vec![],
                    count: 0,
                    periodic_config: PeriodicConfigSummary {
                        target_hour: effective_hour,
                        target_day_of_week: effective_dow,
                        periodic_weight,
                        auto_detected: params.auto_detect,
                    },
                })
                .unwrap(),
            );
        }

        // Apply E3 periodic boost POST-RETRIEVAL
        let mut boosted_results: Vec<PeriodicSearchResultEntry> = results
            .into_iter()
            .map(|r| {
                let memory_dt = r.fingerprint.created_at;
                let memory_hour = memory_dt.hour() as u8;
                let memory_dow = memory_dt.weekday().num_days_from_sunday() as u8;

                // Compute E3 periodic score using embeddings if available
                // TeleologicalFingerprint.semantic has the 13-embedder SemanticFingerprint
                let periodic_score = if !query_embedding.e3_temporal_periodic.is_empty()
                    && !r.fingerprint.semantic.e3_temporal_periodic.is_empty()
                {
                    // Use E3 embedding similarity
                    compute_e3_periodic_score(
                        &query_embedding.e3_temporal_periodic,
                        &r.fingerprint.semantic.e3_temporal_periodic,
                    )
                } else {
                    // Fall back to hour/day matching
                    compute_periodic_match_fallback(
                        effective_hour,
                        memory_hour,
                        effective_dow,
                        memory_dow,
                    )
                };

                // Per ARCH-25: Periodic boost is multiplicative POST-retrieval
                // boost_factor = 1 + weight * (score - 0.5)
                // This gives neutral at 0.5, boost above, reduce below
                let boost_factor = (1.0 + periodic_weight * (periodic_score - 0.5)).clamp(0.8, 1.2);
                let final_score = r.similarity * boost_factor;

                let created_at = r.fingerprint.created_at.to_rfc3339();

                PeriodicSearchResultEntry {
                    id: r.fingerprint.id.to_string(),
                    semantic_score: r.similarity,
                    periodic_score,
                    final_score,
                    memory_hour,
                    memory_day_of_week: memory_dow,
                    day_name: day_name(memory_dow).to_string(),
                    content: r.content,
                    created_at,
                }
            })
            .collect();

        // Sort by final score descending
        boosted_results.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to requested top_k
        boosted_results.truncate(params.top_k);
        let count = boosted_results.len();

        debug!(
            result_count = count,
            effective_hour = ?effective_hour,
            effective_dow = ?effective_dow,
            "search_periodic: Periodic search complete"
        );

        // Build response
        let response = SearchPeriodicResponse {
            query: params.query,
            results: boosted_results,
            count,
            periodic_config: PeriodicConfigSummary {
                target_hour: effective_hour,
                target_day_of_week: effective_dow,
                periodic_weight,
                auto_detected: params.auto_detect,
            },
        };

        match serde_json::to_value(&response) {
            Ok(json) => self.tool_result(id, json),
            Err(e) => {
                error!(error = %e, "search_periodic: Response serialization FAILED");
                self.tool_error(id, &format!("Response serialization failed: {}", e))
            }
        }
    }
}
