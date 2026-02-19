//! Tool dispatch logic for MCP tool calls.
//!
//! Per PRD v6 Section 10, all 56 MCP tools are dispatched here.
//! Uses `tool_dispatch!` macro to eliminate boilerplate match arms.
//!
//! ## Adding a new tool
//! 1. Add the tool name constant to `tools/names.rs`
//! 2. Add the handler method `call_X(id, args)` to the relevant `*_tools.rs`
//! 3. Add one line to the `tool_dispatch!` invocation below

use serde_json::json;
use tracing::debug;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::super::Handlers;

/// Dispatch tool calls to handler methods via generated match.
///
/// Supports handlers with or without arguments:
///   `TOOL_NAME => handler(arguments)` — calls `self.handler(id, arguments).await`
///   `TOOL_NAME => handler()`          — calls `self.handler(id).await`
macro_rules! tool_dispatch {
    ($self:expr, $id:expr, $name:expr,
        $( $tool_name:path => $method:ident ( $($param:expr),* ) ),* $(,)?
    ) => {
        match $name {
            $( $tool_name => $self.$method( $id $(, $param)* ).await, )*
            _ => JsonRpcResponse::error(
                $id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", $name),
            ),
        }
    }
}

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

        tool_dispatch!(self, id, tool_name,
            // Core tools (PRD Section 10.1)
            tool_names::STORE_MEMORY => call_store_memory(arguments),
            tool_names::GET_MEMETIC_STATUS => call_get_memetic_status(),
            tool_names::SEARCH_GRAPH => call_search_graph(arguments),
            // Consolidation tools
            tool_names::TRIGGER_CONSOLIDATION => call_trigger_consolidation(arguments),
            // Topic tools (PRD Section 10.2)
            tool_names::GET_TOPIC_PORTFOLIO => call_get_topic_portfolio(arguments),
            tool_names::GET_TOPIC_STABILITY => call_get_topic_stability(arguments),
            tool_names::DETECT_TOPICS => call_detect_topics(arguments),
            tool_names::GET_DIVERGENCE_ALERTS => call_get_divergence_alerts(arguments),
            // Curation tools (PRD Section 10.3)
            tool_names::MERGE_CONCEPTS => call_merge_concepts(arguments),
            tool_names::FORGET_CONCEPT => call_forget_concept(arguments),
            tool_names::BOOST_IMPORTANCE => call_boost_importance(arguments),
            // File watcher tools
            tool_names::LIST_WATCHED_FILES => call_list_watched_files(arguments),
            tool_names::GET_FILE_WATCHER_STATS => call_get_file_watcher_stats(),
            tool_names::DELETE_FILE_CONTENT => call_delete_file_content(arguments),
            tool_names::RECONCILE_FILES => call_reconcile_files(arguments),
            // Sequence tools (E4)
            tool_names::GET_CONVERSATION_CONTEXT => call_get_conversation_context(arguments),
            tool_names::GET_SESSION_TIMELINE => call_get_session_timeline(arguments),
            tool_names::TRAVERSE_MEMORY_CHAIN => call_traverse_memory_chain(arguments),
            tool_names::COMPARE_SESSION_STATES => call_compare_session_states(arguments),
            // Causal tools (E5)
            tool_names::SEARCH_CAUSES => call_search_causes(arguments),
            tool_names::SEARCH_EFFECTS => call_search_effects(arguments),
            tool_names::GET_CAUSAL_CHAIN => call_get_causal_chain(arguments),
            tool_names::SEARCH_CAUSAL_RELATIONSHIPS => call_search_causal_relationships(arguments),
            // Causal discovery tools (LLM)
            tool_names::TRIGGER_CAUSAL_DISCOVERY => call_trigger_causal_discovery(arguments),
            tool_names::GET_CAUSAL_DISCOVERY_STATUS => call_get_causal_discovery_status(arguments),
            // Graph tools (E8)
            tool_names::SEARCH_CONNECTIONS => call_search_connections(arguments),
            tool_names::GET_GRAPH_PATH => call_get_graph_path(arguments),
            // Graph discovery tools (LLM)
            tool_names::DISCOVER_GRAPH_RELATIONSHIPS => call_discover_graph_relationships(arguments),
            tool_names::VALIDATE_GRAPH_LINK => call_validate_graph_link(arguments),
            // Keyword tools (E6)
            tool_names::SEARCH_BY_KEYWORDS => call_search_by_keywords(arguments),
            // Code tools (E7)
            tool_names::SEARCH_CODE => call_search_code(arguments),
            // Robustness tools (E9)
            tool_names::SEARCH_ROBUST => call_search_robust(arguments),
            // Entity tools (E11)
            tool_names::EXTRACT_ENTITIES => call_extract_entities(arguments),
            tool_names::SEARCH_BY_ENTITIES => call_search_by_entities(arguments),
            tool_names::INFER_RELATIONSHIP => call_infer_relationship(arguments),
            tool_names::FIND_RELATED_ENTITIES => call_find_related_entities(arguments),
            tool_names::VALIDATE_KNOWLEDGE => call_validate_knowledge(arguments),
            tool_names::GET_ENTITY_GRAPH => call_get_entity_graph(arguments),
            // Embedder-first search tools (Constitution v6.3)
            tool_names::SEARCH_BY_EMBEDDER => call_search_by_embedder(arguments),
            tool_names::GET_EMBEDDER_CLUSTERS => call_get_embedder_clusters(arguments),
            tool_names::COMPARE_EMBEDDER_VIEWS => call_compare_embedder_views(arguments),
            tool_names::LIST_EMBEDDER_INDEXES => call_list_embedder_indexes(arguments),
            tool_names::GET_MEMORY_FINGERPRINT => call_get_memory_fingerprint(arguments),
            tool_names::CREATE_WEIGHT_PROFILE => call_create_weight_profile(arguments),
            tool_names::SEARCH_CROSS_EMBEDDER_ANOMALIES => call_search_cross_embedder_anomalies(arguments),
            // Temporal tools (E2/E3)
            tool_names::SEARCH_RECENT => call_search_recent(arguments),
            tool_names::SEARCH_PERIODIC => call_search_periodic(arguments),
            // Graph linking tools (K-NN)
            tool_names::GET_MEMORY_NEIGHBORS => call_get_memory_neighbors(arguments),
            tool_names::GET_TYPED_EDGES => call_get_typed_edges(arguments),
            tool_names::TRAVERSE_GRAPH => call_traverse_graph(arguments),
            tool_names::GET_UNIFIED_NEIGHBORS => call_get_unified_neighbors(arguments),
            // Maintenance tools
            tool_names::REPAIR_CAUSAL_RELATIONSHIPS => call_repair_causal_relationships(),
            // Provenance tools (Phase P3)
            tool_names::GET_AUDIT_TRAIL => call_get_audit_trail(arguments),
            tool_names::GET_MERGE_HISTORY => call_get_merge_history(arguments),
            tool_names::GET_PROVENANCE_CHAIN => call_get_provenance_chain(arguments),
            // Daemon tools (Multi-agent observability)
            tool_names::DAEMON_STATUS => call_daemon_status(),
        )
    }
}
