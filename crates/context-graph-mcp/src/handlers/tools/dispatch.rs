//! Tool dispatch logic for MCP tool calls.
//!
//! Per PRD v6 Section 10, only these MCP tools are exposed:
//! - Core: inject_context, search_graph, store_memory, get_memetic_status
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
            tool_names::INJECT_CONTEXT => self.call_inject_context(id, arguments).await,
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

            // ========== DREAM TOOLS (Constitution dream layer) ==========
            tool_names::TRIGGER_DREAM => self.call_trigger_dream(id, arguments).await,
            tool_names::GET_DREAM_STATUS => self.call_get_dream_status(id, arguments).await,

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
            tool_names::GET_CAUSAL_CHAIN => self.call_get_causal_chain(id, arguments).await,

            // Unknown tool
            _ => JsonRpcResponse::error(
                id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", tool_name),
            ),
        }
    }
}
