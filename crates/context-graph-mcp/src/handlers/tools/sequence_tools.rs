//! Sequence tool implementations for E4 (V_ordering) integration.
//!
//! These tools provide first-class conversational context capabilities:
//! - `get_conversation_context`: Get memories around current turn
//! - `get_session_timeline`: Ordered session memory timeline
//! - `traverse_memory_chain`: Multi-hop memory navigation
//! - `compare_session_states`: Before/after state comparison

use serde_json::json;
use tracing::{debug, error};

use context_graph_core::teleological::matrix_search::embedder_names;
use context_graph_core::traits::{
    SequenceDirection, SequenceOptions, TeleologicalSearchOptions, TeleologicalSearchResult,
};
use context_graph_core::types::SourceMetadata;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::super::Handlers;

/// Input validation constants for sequence tools
mod validation {
    pub const MIN_WINDOW_SIZE: u64 = 1;
    pub const MAX_WINDOW_SIZE: u64 = 50;
    pub const DEFAULT_WINDOW_SIZE: u64 = 10;

    pub const MIN_LIMIT: u64 = 1;
    pub const MAX_LIMIT: u64 = 200;
    pub const DEFAULT_LIMIT: u64 = 50;

    pub const MIN_HOPS: u64 = 1;
    pub const MAX_HOPS: u64 = 20;
    pub const DEFAULT_HOPS: u64 = 5;

    pub const COMPARISON_BATCH_SIZE: usize = 500;
}
use validation::*;

/// Validate windowSize parameter.
/// Returns Ok(value) or Err(error_message).
fn validate_window_size(value: Option<&serde_json::Value>) -> Result<u64, String> {
    match value.and_then(|v| v.as_u64()) {
        Some(v) if v < MIN_WINDOW_SIZE => Err(
            format!("windowSize {} below minimum {}", v, MIN_WINDOW_SIZE)
        ),
        Some(v) if v > MAX_WINDOW_SIZE => Err(
            format!("windowSize {} exceeds maximum {}", v, MAX_WINDOW_SIZE)
        ),
        Some(v) => Ok(v),
        None => Ok(DEFAULT_WINDOW_SIZE),
    }
}

/// Validate limit parameter.
/// Returns Ok(value) or Err(error_message).
fn validate_limit(value: Option<&serde_json::Value>) -> Result<u64, String> {
    match value.and_then(|v| v.as_u64()) {
        Some(v) if v < MIN_LIMIT => Err(
            format!("limit {} below minimum {}", v, MIN_LIMIT)
        ),
        Some(v) if v > MAX_LIMIT => Err(
            format!("limit {} exceeds maximum {}", v, MAX_LIMIT)
        ),
        Some(v) => Ok(v),
        None => Ok(DEFAULT_LIMIT),
    }
}

/// Validate hops parameter.
/// Returns Ok(value) or Err(error_message).
fn validate_hops(value: Option<&serde_json::Value>) -> Result<u64, String> {
    match value.and_then(|v| v.as_u64()) {
        Some(v) if v < MIN_HOPS => Err(
            format!("hops {} below minimum {}", v, MIN_HOPS)
        ),
        Some(v) if v > MAX_HOPS => Err(
            format!("hops {} exceeds maximum {}", v, MAX_HOPS)
        ),
        Some(v) => Ok(v),
        None => Ok(DEFAULT_HOPS),
    }
}

impl Handlers {
    /// get_conversation_context tool implementation.
    ///
    /// Gets memories around the current conversation turn with auto-anchoring.
    /// Uses E4 (V_ordering) for sequence-based retrieval via conversation_history weight profile.
    pub(crate) async fn call_get_conversation_context(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse direction (before, after, both)
        let direction_str = args
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("before");
        let direction = match direction_str {
            "before" => SequenceDirection::Before,
            "after" => SequenceDirection::After,
            _ => SequenceDirection::Both,
        };

        // Parse and validate window size (FAIL FAST)
        let window_size = match validate_window_size(args.get("windowSize")) {
            Ok(v) => v as u32,
            Err(msg) => {
                error!(error = %msg, "get_conversation_context: windowSize validation FAILED");
                return self.tool_error(id, &msg);
            }
        };

        // Parse session filter
        let session_only = args
            .get("sessionOnly")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Parse include content
        let include_content = args
            .get("includeContent")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Parse optional semantic filter
        let query = args.get("query").and_then(|v| v.as_str());
        let min_similarity = args
            .get("minSimilarity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;

        // Get current sequence number for anchoring
        let current_seq = self.current_sequence();
        let session_id = self.get_session_id();

        debug!(
            current_seq = current_seq,
            direction = ?direction,
            window_size = window_size,
            session_only = session_only,
            "get_conversation_context: Building sequence-based query"
        );

        // Build sequence options
        let seq_opts = SequenceOptions::from_sequence(current_seq, direction, window_size);

        // Build search options with conversation_history weight profile for E1+E4 balance
        let mut options = TeleologicalSearchOptions::quick(window_size as usize * 2)
            .with_min_similarity(min_similarity)
            .with_weight_profile("conversation_history");

        // Apply session filter
        if session_only {
            if let Some(ref sid) = session_id {
                options = options.with_session_filter(sid);
            }
        }

        // Apply sequence options
        options.temporal_options.sequence_options = Some(seq_opts);

        // MCP-08 FIX: When no explicit query is provided, skip semantic search entirely
        // and return results purely based on sequence ordering (which is what the user
        // expects from a conversation context tool). Using a hardcoded "conversation context"
        // string would bias retrieval toward memories that happen to match that phrase.
        let results: Vec<TeleologicalSearchResult> = if let Some(query_text) = query {
            // User provided an explicit query - use semantic search with it
            let query_embedding = match self.embed_query(id.clone(), query_text, "get_conversation_context").await {
                Ok(fp) => fp,
                Err(resp) => return resp,
            };

            match self
                .teleological_store
                .search_semantic(&query_embedding, options)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    error!(error = %e, "get_conversation_context: Search FAILED");
                    return self.tool_error(id, &format!("Search failed: {}", e));
                }
            }
        } else {
            // No query provided - use unbiased fingerprint listing with sequence ordering
            // Fetch extra to account for session filtering
            let fetch_limit = if session_only { window_size as usize * 4 } else { window_size as usize * 2 };
            let unbiased = match self
                .teleological_store
                .list_fingerprints_unbiased(fetch_limit)
                .await
            {
                Ok(fps) => fps,
                Err(e) => {
                    error!(error = %e, "get_conversation_context: Unbiased scan FAILED");
                    return self.tool_error(id, &format!("Fingerprint scan failed: {}", e));
                }
            };

            unbiased
                .into_iter()
                .map(|fp| TeleologicalSearchResult {
                    fingerprint: fp,
                    similarity: 1.0, // No semantic bias - ordered by sequence
                    embedder_scores: [0.0; 13],
                    stage_scores: [0.0; 5],
                    content: None,
                    temporal_breakdown: None,
                })
                .collect()
        };

        // SEC-SESSION: Post-filter by session_id when sessionOnly=true.
        // The store layer doesn't filter by session during search, so we do it here
        // using source_metadata to check session_id matches.
        let results = if session_only {
            if let Some(ref target_sid) = session_id {
                let result_ids: Vec<uuid::Uuid> = results.iter().map(|r| r.fingerprint.id).collect();
                let metadata_batch = self
                    .teleological_store
                    .get_source_metadata_batch(&result_ids)
                    .await
                    .unwrap_or_else(|_| vec![None; result_ids.len()]);

                results
                    .into_iter()
                    .zip(metadata_batch.into_iter())
                    .filter(|(_r, meta)| {
                        meta.as_ref()
                            .and_then(|m| m.session_id.as_ref())
                            .map(|sid| sid == target_sid)
                            .unwrap_or(false)
                    })
                    .map(|(r, _)| r)
                    .collect()
            } else {
                results
            }
        } else {
            results
        };

        // Limit results to window size
        let filtered: Vec<_> = results.into_iter().take(window_size as usize).collect();

        // Collect IDs for batch content retrieval
        let ids: Vec<uuid::Uuid> = filtered.iter().map(|r| r.fingerprint.id).collect();

        // Batch retrieve content if requested (FAIL FAST)
        let contents: Vec<Option<String>> = if include_content && !filtered.is_empty() {
            match self.teleological_store.get_content_batch(&ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "get_conversation_context: Content retrieval FAILED");
                    return self.tool_error(id, &format!("Content retrieval failed: {}", e));
                }
            }
        } else {
            vec![None; ids.len()]
        };

        // Batch retrieve source metadata (FAIL FAST)
        let source_metadata: Vec<Option<SourceMetadata>> = if !filtered.is_empty() {
            match self.teleological_store.get_source_metadata_batch(&ids).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "get_conversation_context: Source metadata retrieval FAILED");
                    return self.tool_error(id, &format!("Source metadata retrieval failed: {}", e));
                }
            }
        } else {
            vec![]
        };

        // Build response
        let results_json: Vec<_> = filtered
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut entry = json!({
                    "fingerprintId": r.fingerprint.id.to_string(),
                    "similarity": r.similarity,
                    "dominantEmbedder": embedder_names::name(r.dominant_embedder())
                });

                if include_content {
                    entry["content"] = match contents.get(i).and_then(|c| c.as_ref()) {
                        Some(c) => json!(c),
                        None => serde_json::Value::Null,
                    };
                }

                // Add sequence info
                if let Some(Some(ref metadata)) = source_metadata.get(i) {
                    if let Some(seq) = metadata.session_sequence {
                        entry["sequenceInfo"] = json!({
                            "sessionId": metadata.session_id,
                            "sessionSequence": seq,
                            "positionLabel": compute_position_label(seq, current_seq)
                        });
                    }
                    entry["sourceType"] = json!(format!("{}", metadata.source_type));
                }

                entry
            })
            .collect();

        self.tool_result(
            id,
            json!({
                "results": results_json,
                "count": results_json.len(),
                "currentSequence": current_seq,
                "direction": direction_str,
                "windowSize": window_size
            }),
        )
    }

    /// get_session_timeline tool implementation.
    ///
    /// Returns an ordered timeline of all session memories with sequence numbers.
    pub(crate) async fn call_get_session_timeline(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse session ID (default: current)
        let session_id = args
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.get_session_id());

        // Handle missing session ID gracefully - return empty timeline with explanation
        let session_id = match session_id {
            Some(sid) => sid,
            None => {
                debug!("get_session_timeline: No session ID available, returning empty timeline");
                return self.tool_result(
                    id,
                    json!({
                        "sessionId": null,
                        "timeline": [],
                        "count": 0,
                        "currentSequence": self.current_sequence(),
                        "offset": 0,
                        "limit": 50,
                        "message": "No session ID available. Set CLAUDE_SESSION_ID environment variable or pass sessionId parameter."
                    }),
                );
            }
        };

        // Parse and validate limit (FAIL FAST)
        let limit = match validate_limit(args.get("limit")) {
            Ok(v) => v as usize,
            Err(msg) => {
                error!(error = %msg, "get_session_timeline: limit validation FAILED");
                return self.tool_error(id, &msg);
            }
        };
        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // Parse include content
        let include_content = args
            .get("includeContent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // MCP-7 FIX: Parse sourceTypes filter parameter
        let source_types: Option<Vec<String>> = args
            .get("sourceTypes")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        debug!(
            session_id = %session_id,
            limit = limit,
            offset = offset,
            source_types = ?source_types,
            "get_session_timeline: Fetching session timeline"
        );

        // MED-14 FIX: Use unbiased fingerprint scan instead of semantic search
        // with hardcoded "session timeline" query (which biased retrieval).
        // Fetch a generous batch and filter by session via source_metadata.
        let all_fingerprints = match self
            .teleological_store
            .list_fingerprints_unbiased((limit + offset) * 10) // Over-fetch for session filter
            .await
        {
            Ok(fps) => fps,
            Err(e) => {
                error!(error = %e, "get_session_timeline: Unbiased scan FAILED");
                return self.tool_error(id, &format!("Fingerprint scan failed: {}", e));
            }
        };

        // Get source metadata to filter by session
        let all_ids: Vec<uuid::Uuid> = all_fingerprints.iter().map(|fp| fp.id).collect();
        let all_metadata = if !all_ids.is_empty() {
            match self.teleological_store.get_source_metadata_batch(&all_ids).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "get_session_timeline: Source metadata retrieval FAILED");
                    return self.tool_error(id, &format!("Source metadata retrieval failed: {}", e));
                }
            }
        } else {
            vec![]
        };

        // Filter to only fingerprints in the requested session
        let session_fingerprints: Vec<&TeleologicalFingerprint> = all_fingerprints
            .iter()
            .zip(all_metadata.iter())
            .filter(|(_fp, meta): &(&TeleologicalFingerprint, &Option<SourceMetadata>)| {
                meta.as_ref()
                    .and_then(|m| m.session_id.as_deref())
                    .is_some_and(|sid| sid == session_id)
            })
            .map(|(fp, _): (&TeleologicalFingerprint, &Option<SourceMetadata>)| fp)
            .collect();

        // Convert to TeleologicalSearchResult for compatibility with downstream code
        // MCP-6 FIX: Do NOT paginate here — pagination must happen AFTER sorting
        // by session_sequence below. Pre-sorting pagination produces wrong results
        // when offset > 0 because the second skip/take on sorted data operates on
        // an already-truncated set.
        let results: Vec<TeleologicalSearchResult> = session_fingerprints
            .into_iter()
            .map(|fp| TeleologicalSearchResult {
                fingerprint: fp.clone(),
                similarity: 1.0, // No semantic bias
                embedder_scores: [0.0; 13],
                stage_scores: [0.0; 5],
                content: None,
                temporal_breakdown: None,
            })
            .collect();

        // Get current sequence for position labels
        let current_seq = self.current_sequence();

        // Collect IDs for batch operations
        let ids: Vec<uuid::Uuid> = results.iter().map(|r| r.fingerprint.id).collect();

        // Batch retrieve content if requested (FAIL FAST)
        let contents: Vec<Option<String>> = if include_content && !results.is_empty() {
            match self.teleological_store.get_content_batch(&ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "get_session_timeline: Content retrieval FAILED");
                    return self.tool_error(id, &format!("Content retrieval failed: {}", e));
                }
            }
        } else {
            vec![None; ids.len()]
        };

        // Batch retrieve source metadata (FAIL FAST)
        let source_metadata: Vec<Option<SourceMetadata>> = if !results.is_empty() {
            match self.teleological_store.get_source_metadata_batch(&ids).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "get_session_timeline: Source metadata retrieval FAILED");
                    return self.tool_error(id, &format!("Source metadata retrieval failed: {}", e));
                }
            }
        } else {
            vec![]
        };

        // Build response - filter to those with sequence info and sort
        let mut results_with_seq: Vec<(u64, serde_json::Value)> = results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let metadata = source_metadata.get(i).and_then(|m| m.as_ref())?;
                let seq = metadata.session_sequence?;

                let mut entry = json!({
                    "fingerprintId": r.fingerprint.id.to_string(),
                    "sessionSequence": seq,
                    "positionLabel": compute_position_label(seq, current_seq),
                    "sourceType": format!("{}", metadata.source_type)
                });

                if include_content {
                    entry["content"] = match contents.get(i).and_then(|c| c.as_ref()) {
                        Some(c) => json!(c),
                        None => serde_json::Value::Null,
                    };
                }

                Some((seq, entry))
            })
            .collect();

        // Sort by sequence ascending
        results_with_seq.sort_by_key(|(seq, _)| *seq);

        // MCP-7 FIX: Apply sourceTypes filter before pagination
        if let Some(ref types) = source_types {
            results_with_seq.retain(|(_, entry)| {
                entry
                    .get("sourceType")
                    .and_then(|v| v.as_str())
                    .is_some_and(|st| types.iter().any(|t| t == st))
            });
        }

        // Capture total before pagination for has_more indicator
        let total_in_session = results_with_seq.len();

        // Apply pagination
        let paginated: Vec<serde_json::Value> = results_with_seq
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(_, entry)| entry)
            .collect();

        let has_more = total_in_session > offset + limit;

        self.tool_result(
            id,
            json!({
                "sessionId": session_id,
                "timeline": paginated,
                "count": paginated.len(),
                "total_in_session": total_in_session,
                "has_more": has_more,
                "currentSequence": current_seq,
                "offset": offset,
                "limit": limit
            }),
        )
    }

    /// traverse_memory_chain tool implementation.
    ///
    /// Navigates through a chain of memories starting from an anchor point.
    pub(crate) async fn call_traverse_memory_chain(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse anchor ID (required)
        let anchor_id = match args.get("anchorId").and_then(|v| v.as_str()) {
            Some(id_str) => match uuid::Uuid::parse_str(id_str) {
                Ok(uuid) => uuid,
                Err(_) => return self.tool_error(id, "Invalid anchorId UUID format"),
            },
            None => return self.tool_error(id, "Missing required 'anchorId' parameter"),
        };

        // FAIL FAST: Verify anchor exists in storage
        match self.teleological_store.get_content(anchor_id).await {
            Ok(Some(_)) => {
                debug!(anchor_id = %anchor_id, "traverse_memory_chain: Anchor exists");
            }
            Ok(None) => {
                error!(anchor_id = %anchor_id, "traverse_memory_chain: Anchor not found");
                return self.tool_error(
                    id,
                    &format!("Anchor memory {} not found in storage", anchor_id)
                );
            }
            Err(e) => {
                error!(error = %e, anchor_id = %anchor_id, "traverse_memory_chain: Anchor verification FAILED");
                return self.tool_error(id, &format!("Failed to verify anchor: {}", e));
            }
        }

        // Parse direction
        let direction_str = args
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("backward");
        let direction = match direction_str {
            "forward" => SequenceDirection::After,
            "backward" => SequenceDirection::Before,
            _ => SequenceDirection::Both,
        };

        // Parse and validate hops (FAIL FAST)
        let hops = match validate_hops(args.get("hops")) {
            Ok(v) => v as u32,
            Err(msg) => {
                error!(error = %msg, "traverse_memory_chain: hops validation FAILED");
                return self.tool_error(id, &msg);
            }
        };

        // Parse semantic filter
        let semantic_filter = args.get("semanticFilter").and_then(|v| v.as_str());
        let min_similarity = args
            .get("minSimilarity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.3) as f32;

        // Parse include content
        let include_content = args
            .get("includeContent")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        debug!(
            anchor_id = %anchor_id,
            direction = ?direction,
            hops = hops,
            "traverse_memory_chain: Starting traversal"
        );

        // Build sequence options for traversal
        let seq_opts = SequenceOptions::around(anchor_id)
            .with_direction(direction)
            .with_max_distance(hops as usize);

        // Build search options
        let mut options = TeleologicalSearchOptions::quick(hops as usize * 2)
            .with_min_similarity(min_similarity)
            .with_weight_profile("sequence_navigation");

        options.temporal_options.sequence_options = Some(seq_opts);

        // Determine query embedding: either from user's semantic filter or from anchor's own E1 vector.
        // Using the anchor's E1 embedding finds semantically related memories with real similarity scores.
        let query_embedding = if let Some(filter_text) = semantic_filter {
            // User provided an explicit semantic filter - embed it
            match self.embed_query(id.clone(), filter_text, "traverse_memory_chain").await {
                Ok(fp) => fp,
                Err(resp) => return resp,
            }
        } else {
            // No semantic filter - use anchor's own fingerprint as the query.
            // This finds memories most similar to the anchor with real E1 similarity scores.
            match self.teleological_store.retrieve(anchor_id).await {
                Ok(Some(anchor_fp)) => anchor_fp.semantic,
                Ok(None) => {
                    return self.tool_error(
                        id,
                        &format!("Anchor memory {} not found for embedding retrieval", anchor_id),
                    );
                }
                Err(e) => {
                    error!(error = %e, "traverse_memory_chain: Anchor retrieval FAILED");
                    return self.tool_error(id, &format!("Failed to retrieve anchor: {}", e));
                }
            }
        };

        let results: Vec<TeleologicalSearchResult> = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "traverse_memory_chain: Search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        // Limit to requested hops
        let chain: Vec<_> = results.into_iter().take(hops as usize).collect();

        // Get current sequence for position labels
        let current_seq = self.current_sequence();

        // Collect IDs for batch operations
        let ids: Vec<uuid::Uuid> = chain.iter().map(|r| r.fingerprint.id).collect();

        // Batch retrieve content if requested (FAIL FAST)
        let contents: Vec<Option<String>> = if include_content && !chain.is_empty() {
            match self.teleological_store.get_content_batch(&ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "traverse_memory_chain: Content retrieval FAILED");
                    return self.tool_error(id, &format!("Content retrieval failed: {}", e));
                }
            }
        } else {
            vec![None; ids.len()]
        };

        // Batch retrieve source metadata (FAIL FAST)
        let source_metadata: Vec<Option<SourceMetadata>> = if !chain.is_empty() {
            match self.teleological_store.get_source_metadata_batch(&ids).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "traverse_memory_chain: Source metadata retrieval FAILED");
                    return self.tool_error(id, &format!("Source metadata retrieval failed: {}", e));
                }
            }
        } else {
            vec![]
        };

        // Build response
        let chain_json: Vec<_> = chain
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut entry = json!({
                    "fingerprintId": r.fingerprint.id.to_string(),
                    "hopIndex": i,
                    "similarity": r.similarity
                });

                if include_content {
                    entry["content"] = match contents.get(i).and_then(|c| c.as_ref()) {
                        Some(c) => json!(c),
                        None => serde_json::Value::Null,
                    };
                }

                if let Some(Some(ref metadata)) = source_metadata.get(i) {
                    if let Some(seq) = metadata.session_sequence {
                        entry["sequenceInfo"] = json!({
                            "sessionId": metadata.session_id,
                            "sessionSequence": seq,
                            "positionLabel": compute_position_label(seq, current_seq)
                        });
                    }
                    entry["sourceType"] = json!(format!("{}", metadata.source_type));
                }

                entry
            })
            .collect();

        self.tool_result(
            id,
            json!({
                "anchorId": anchor_id.to_string(),
                "direction": direction_str,
                "chain": chain_json,
                "chainLength": chain_json.len(),
                "requestedHops": hops
            }),
        )
    }

    /// compare_session_states tool implementation.
    ///
    /// Compares memory state at different sequence points in a session.
    pub(crate) async fn call_compare_session_states(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse session ID (default: current)
        let session_id = args
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.get_session_id());

        // Handle missing session ID gracefully - return empty comparison with explanation
        let session_id = match session_id {
            Some(sid) => sid,
            None => {
                let current_seq = self.current_sequence();
                debug!("compare_session_states: No session ID available, returning empty comparison");
                return self.tool_result(
                    id,
                    json!({
                        "sessionId": null,
                        "beforeState": {
                            "sequenceRange": [0, 0],
                            "memoryCount": 0,
                            "sourceTypeCounts": {}
                        },
                        "afterState": {
                            "sequenceRange": [0, 0],
                            "memoryCount": 0,
                            "sourceTypeCounts": {}
                        },
                        "difference": {
                            "addedMemories": 0,
                            "sequenceSpan": 0
                        },
                        "currentSequence": current_seq,
                        "message": "No session ID available. Set CLAUDE_SESSION_ID environment variable or pass sessionId parameter."
                    }),
                );
            }
        };

        // Parse beforeSequence
        let current_seq = self.current_sequence();
        let before_seq = match args.get("beforeSequence") {
            Some(serde_json::Value::String(s)) if s == "start" => 0,
            Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
            _ => 0,
        };

        // Parse afterSequence
        let after_seq = match args.get("afterSequence") {
            Some(serde_json::Value::String(s)) if s == "current" => current_seq,
            Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(current_seq),
            _ => current_seq,
        };

        debug!(
            session_id = %session_id,
            before_seq = before_seq,
            after_seq = after_seq,
            "compare_session_states: Comparing states"
        );

        // MED-15 FIX: Use unbiased fingerprint scan instead of semantic search
        // with hardcoded "session state comparison" query (which biased retrieval).
        let all_fingerprints = match self
            .teleological_store
            .list_fingerprints_unbiased(COMPARISON_BATCH_SIZE * 10)
            .await
        {
            Ok(fps) => fps,
            Err(e) => {
                error!(error = %e, "compare_session_states: Unbiased scan FAILED");
                return self.tool_error(id, &format!("Fingerprint scan failed: {}", e));
            }
        };

        // Filter by session using source metadata
        let scan_ids: Vec<uuid::Uuid> = all_fingerprints.iter().map(|fp| fp.id).collect();
        let scan_metadata = if !scan_ids.is_empty() {
            match self.teleological_store.get_source_metadata_batch(&scan_ids).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "compare_session_states: Source metadata retrieval FAILED");
                    return self.tool_error(id, &format!("Source metadata retrieval failed: {}", e));
                }
            }
        } else {
            vec![]
        };

        let all_memories: Vec<TeleologicalSearchResult> = all_fingerprints
            .iter()
            .zip(scan_metadata.iter())
            .filter(|(_fp, meta): &(&TeleologicalFingerprint, &Option<SourceMetadata>)| {
                meta.as_ref()
                    .and_then(|m| m.session_id.as_deref())
                    .is_some_and(|sid| sid == session_id)
            })
            .take(COMPARISON_BATCH_SIZE)
            .map(|(fp, _): (&TeleologicalFingerprint, &Option<SourceMetadata>)| TeleologicalSearchResult {
                fingerprint: fp.clone(),
                similarity: 1.0,
                embedder_scores: [0.0; 13],
                stage_scores: [0.0; 5],
                content: None,
                temporal_breakdown: None,
            })
            .collect();

        // Get source metadata for sequence filtering (FAIL FAST)
        let ids: Vec<uuid::Uuid> = all_memories.iter().map(|r| r.fingerprint.id).collect();
        let source_metadata: Vec<Option<SourceMetadata>> = if !all_memories.is_empty() {
            match self.teleological_store.get_source_metadata_batch(&ids).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "compare_session_states: Source metadata retrieval FAILED");
                    return self.tool_error(id, &format!("Source metadata retrieval failed: {}", e));
                }
            }
        } else {
            vec![]
        };

        // Count memories in before and after windows
        let mut before_count = 0usize;
        let mut after_count = 0usize;
        let mut before_by_type: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        let mut after_by_type: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        for (i, _) in all_memories.iter().enumerate() {
            if let Some(Some(ref metadata)) = source_metadata.get(i) {
                if let Some(seq) = metadata.session_sequence {
                    let source_type = format!("{}", metadata.source_type);
                    if seq <= before_seq {
                        before_count += 1;
                        *before_by_type.entry(source_type).or_insert(0) += 1;
                    } else if seq <= after_seq {
                        after_count += 1;
                        *after_by_type.entry(source_type).or_insert(0) += 1;
                    }
                }
            }
        }

        self.tool_result(
            id,
            json!({
                "sessionId": session_id,
                "beforeState": {
                    "sequenceRange": [0, before_seq],
                    "memoryCount": before_count,
                    "sourceTypeCounts": before_by_type
                },
                "afterState": {
                    "sequenceRange": [before_seq + 1, after_seq],
                    "memoryCount": after_count,
                    "sourceTypeCounts": after_by_type
                },
                "difference": {
                    "addedMemories": after_count,
                    "sequenceSpan": after_seq.saturating_sub(before_seq)
                },
                "currentSequence": current_seq
            }),
        )
    }
}

/// Compute human-readable position label for sequence numbers.
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
        let turns_ahead = result_seq - current_seq;
        if turns_ahead == 1 {
            "next turn".to_string()
        } else {
            format!("{} turns ahead", turns_ahead)
        }
    }
}
