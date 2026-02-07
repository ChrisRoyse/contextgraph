//! Tool dispatch logic for MCP tool calls.
//!
//! Per PRD v6 Section 10, only these MCP tools are exposed:
//! - Core: store_memory, search_graph, get_memetic_status (inject_context merged into store_memory)
//! - Consolidation: trigger_consolidation
//! - Curation: merge_concepts

use serde_json::json;
use tracing::debug;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::super::Handlers;

impl Handlers {
    pub(crate) async fn handle_tools_list(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling tools/list request");
        let tools = get_tool_definitions();
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    pub(crate) async fn handle_tools_call(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing params for tools/call",
                );
            }
        };

        let raw_tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'name' parameter in tools/call",
                );
            }
        };

        let tool_name = crate::tools::aliases::resolve_alias(raw_tool_name);
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        debug!(
            "Calling tool: {} with arguments: {:?}{}",
            tool_name,
            arguments,
            if raw_tool_name != tool_name {
                format!(" (resolved from alias '{}')", raw_tool_name)
            } else {
                String::new()
            }
        );

        match tool_name {
            // ========== CORE TOOLS (PRD Section 10.1) ==========
            // Note: inject_context was merged into store_memory. When rationale is provided,
            // the same validation (1-1024 chars) and response format is used.
            tool_names::STORE_MEMORY => self.call_store_memory(id, arguments).await,
            tool_names::GET_MEMETIC_STATUS => self.call_get_memetic_status(id).await,
            tool_names::SEARCH_GRAPH => self.call_search_graph(id, arguments).await,

            // ========== CONSOLIDATION TOOLS (PRD Section 10.1) ==========
            tool_names::TRIGGER_CONSOLIDATION => {
                self.call_trigger_consolidation(id, arguments).await
            }

            // ========== TOPIC TOOLS (PRD Section 10.2) ==========
            tool_names::GET_TOPIC_PORTFOLIO => self.call_get_topic_portfolio(id, arguments).await,
            tool_names::GET_TOPIC_STABILITY => self.call_get_topic_stability(id, arguments).await,
            tool_names::DETECT_TOPICS => self.call_detect_topics(id, arguments).await,
            tool_names::GET_DIVERGENCE_ALERTS => {
                self.call_get_divergence_alerts(id, arguments).await
            }

            // ========== CURATION TOOLS (PRD Section 10.3) ==========
            tool_names::MERGE_CONCEPTS => self.call_merge_concepts(id, arguments).await,
            tool_names::FORGET_CONCEPT => self.call_forget_concept(id, arguments).await,
            tool_names::BOOST_IMPORTANCE => self.call_boost_importance(id, arguments).await,

            // ========== FILE WATCHER TOOLS (File index management) ==========
            tool_names::LIST_WATCHED_FILES => self.call_list_watched_files(id, arguments).await,
            tool_names::GET_FILE_WATCHER_STATS => self.call_get_file_watcher_stats(id).await,
            tool_names::DELETE_FILE_CONTENT => self.call_delete_file_content(id, arguments).await,
            tool_names::RECONCILE_FILES => self.call_reconcile_files(id, arguments).await,

            // ========== SEQUENCE TOOLS (E4 Integration - Phase 1) ==========
            tool_names::GET_CONVERSATION_CONTEXT => {
                self.call_get_conversation_context(id, arguments).await
            }
            tool_names::GET_SESSION_TIMELINE => {
                self.call_get_session_timeline(id, arguments).await
            }
            tool_names::TRAVERSE_MEMORY_CHAIN => {
                self.call_traverse_memory_chain(id, arguments).await
            }
            tool_names::COMPARE_SESSION_STATES => {
                self.call_compare_session_states(id, arguments).await
            }

            // ========== CAUSAL TOOLS (E5 Priority 1 Enhancement) ==========
            tool_names::SEARCH_CAUSES => self.call_search_causes(id, arguments).await,
            tool_names::SEARCH_EFFECTS => self.call_search_effects(id, arguments).await,
            tool_names::GET_CAUSAL_CHAIN => self.call_get_causal_chain(id, arguments).await,
            tool_names::SEARCH_CAUSAL_RELATIONSHIPS => {
                self.call_search_causal_relationships(id, arguments).await
            }

            // ========== CAUSAL DISCOVERY TOOLS (LLM-based causal analysis) ==========
            tool_names::TRIGGER_CAUSAL_DISCOVERY => {
                self.call_trigger_causal_discovery(id, arguments).await
            }
            tool_names::GET_CAUSAL_DISCOVERY_STATUS => {
                self.call_get_causal_discovery_status(id, arguments).await
            }

            // ========== GRAPH TOOLS (E8 Upgrade - Phase 4) ==========
            tool_names::SEARCH_CONNECTIONS => self.call_search_connections(id, arguments).await,
            tool_names::GET_GRAPH_PATH => self.call_get_graph_path(id, arguments).await,

            // ========== GRAPH DISCOVERY TOOLS (LLM-based relationship discovery) ==========
            tool_names::DISCOVER_GRAPH_RELATIONSHIPS => {
                self.call_discover_graph_relationships(id, arguments).await
            }
            tool_names::VALIDATE_GRAPH_LINK => self.call_validate_graph_link(id, arguments).await,

            // ========== INTENT TOOLS (E10 Intent/Context Upgrade) ==========
            // Note: search_by_intent now handles both query-based and context-based searches
            // (formerly separate find_contextual_matches tool was merged)
            tool_names::SEARCH_BY_INTENT => self.call_search_by_intent(id, arguments).await,

            // ========== KEYWORD TOOLS (E6 Keyword Search Enhancement) ==========
            tool_names::SEARCH_BY_KEYWORDS => self.call_search_by_keywords(id, arguments).await,

            // ========== CODE TOOLS (E7 Code Search Enhancement) ==========
            tool_names::SEARCH_CODE => self.call_search_code(id, arguments).await,

            // ========== ROBUSTNESS TOOLS (E9 HDC Blind-Spot Detection) ==========
            tool_names::SEARCH_ROBUST => self.call_search_robust(id, arguments).await,

            // ========== ENTITY TOOLS (E11 Entity Integration) ==========
            tool_names::EXTRACT_ENTITIES => self.call_extract_entities(id, arguments).await,
            tool_names::SEARCH_BY_ENTITIES => self.call_search_by_entities(id, arguments).await,
            tool_names::INFER_RELATIONSHIP => self.call_infer_relationship(id, arguments).await,
            tool_names::FIND_RELATED_ENTITIES => {
                self.call_find_related_entities(id, arguments).await
            }
            tool_names::VALIDATE_KNOWLEDGE => self.call_validate_knowledge(id, arguments).await,
            tool_names::GET_ENTITY_GRAPH => self.call_get_entity_graph(id, arguments).await,

            // ========== EMBEDDER-FIRST SEARCH TOOLS (Constitution v6.3) ==========
            tool_names::SEARCH_BY_EMBEDDER => self.call_search_by_embedder(id, arguments).await,
            tool_names::GET_EMBEDDER_CLUSTERS => {
                self.call_get_embedder_clusters(id, arguments).await
            }
            tool_names::COMPARE_EMBEDDER_VIEWS => {
                self.call_compare_embedder_views(id, arguments).await
            }
            tool_names::LIST_EMBEDDER_INDEXES => {
                self.call_list_embedder_indexes(id, arguments).await
            }
            tool_names::GET_MEMORY_FINGERPRINT => {
                self.call_get_memory_fingerprint(id, arguments).await
            }

            // ========== TEMPORAL TOOLS (E2/E3 Integration) ==========
            tool_names::SEARCH_RECENT => self.call_search_recent(id, arguments).await,
            tool_names::SEARCH_PERIODIC => self.call_search_periodic(id, arguments).await,

            // ========== GRAPH LINKING TOOLS (K-NN Navigation and Typed Edges) ==========
            tool_names::GET_MEMORY_NEIGHBORS => {
                self.call_get_memory_neighbors(id, arguments).await
            }
            tool_names::GET_TYPED_EDGES => self.call_get_typed_edges(id, arguments).await,
            tool_names::TRAVERSE_GRAPH => self.call_traverse_graph(id, arguments).await,
            tool_names::GET_UNIFIED_NEIGHBORS => {
                self.call_get_unified_neighbors(id, arguments).await
            }

            // ========== MAINTENANCE TOOLS (Data repair and cleanup) ==========
            tool_names::REPAIR_CAUSAL_RELATIONSHIPS => {
                self.call_repair_causal_relationships(id).await
            }

            // ========== PROVENANCE TOOLS (Phase P3 - Provenance Queries) ==========
            tool_names::GET_AUDIT_TRAIL => self.call_get_audit_trail(id, arguments).await,
            tool_names::GET_MERGE_HISTORY => self.call_get_merge_history(id, arguments).await,
            tool_names::GET_PROVENANCE_CHAIN => {
                self.call_get_provenance_chain(id, arguments).await
            }

            // Unknown tool
            _ => JsonRpcResponse::error(
                id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", tool_name),
            ),
        }
    }
}
