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
            // E12/E13 standalone search tools
            tool_names::SEARCH_BY_TOKENS => call_search_by_tokens(arguments),
            tool_names::SEARCH_BY_EXPANSION => call_search_by_expansion(arguments),
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
            // Training tools (Training data export)
            tool_names::EXPORT_TRAINING_CORPUS => call_export_training_corpus(arguments),
            tool_names::LIST_TRAINING_RECORDS => call_list_training_records(arguments),
            tool_names::GET_TRAINING_RECORD => call_get_training_record(arguments),
            tool_names::COUNT_TRAINING_RECORDS => call_count_training_records(),
            // Learning-as-UTL tools
            tool_names::RECORD_LEARNING_EVENT => call_record_learning_event(arguments),
            tool_names::LIST_LEARNING_EVENTS => call_list_learning_events(arguments),
            tool_names::GET_LEARNING_EVENT => call_get_learning_event(arguments),
            tool_names::COUNT_LEARNING_EVENTS => call_count_learning_events(),
            tool_names::EXPORT_LEARNER_TRAINING_DATASET => call_export_learner_training_dataset(arguments),
            tool_names::LIST_LEARNER_TRAINING_DATASETS => call_list_learner_training_datasets(arguments),
            tool_names::GET_LEARNER_TRAINING_DATASET => call_get_learner_training_dataset(arguments),
            tool_names::COUNT_LEARNER_TRAINING_DATASETS => call_count_learner_training_datasets(),
            // UTL learner-state tools
            tool_names::REGISTER_LEARNER => call_register_learner(arguments),
            tool_names::RECORD_SESSION_OBSERVATION => call_record_session_observation(arguments),
            tool_names::COMPUTE_DELTA_S => call_compute_delta_s(arguments),
            tool_names::COMPUTE_DELTA_C => call_compute_delta_c(arguments),
            tool_names::COMPUTE_DELTA_E => call_compute_delta_e(arguments),
            tool_names::COMPUTE_L => call_compute_l(arguments),
            tool_names::GET_LEARNER_M => call_get_learner_m(arguments),
            tool_names::NEXT_REVIEW_FOR_TRACE => call_next_review_for_trace(arguments),
            tool_names::LIST_LEARNER_EMBEDDERS => call_list_learner_embedders(),
            tool_names::PREFLIGHT_LEARNER_ASSETS => call_preflight_learner_assets(arguments),
            tool_names::COUNT_LEARNER_STATE => call_count_learner_state(),
            tool_names::GET_LEARNER_STATE => call_get_learner_state(arguments),
            tool_names::RECORD_LEARNER_K_SLEEP => call_record_learner_k_sleep(arguments),
            tool_names::RECORD_LEARNER_RETRIEVAL => call_record_learner_retrieval(arguments),
            tool_names::UPSERT_GOAL_CENTROID => call_upsert_goal_centroid(arguments),
            tool_names::GET_GOAL_DISTANCE => call_get_goal_distance(arguments),
            tool_names::COMPILE_LEARNER_CONSTELLATION => call_compile_learner_constellation(arguments),
            // Constellation tools (Phase 2 compiler)
            tool_names::COMPILE_CONSTELLATION => call_compile_constellation(arguments),
            tool_names::LIST_CONSTELLATIONS => call_list_constellations(arguments),
            tool_names::GET_CONSTELLATION => call_get_constellation(arguments),
            tool_names::SCORE_AGAINST_CONSTELLATION => call_score_against_constellation(arguments),
            tool_names::DELETE_CONSTELLATION => call_delete_constellation(arguments),
            // Contrastive tools (Phase 3 pair miner)
            tool_names::MINE_CONTRASTIVE_PAIRS => call_mine_contrastive_pairs(arguments),
            tool_names::LIST_CONTRASTIVE_PAIRS => call_list_contrastive_pairs(arguments),
            tool_names::GET_CONTRASTIVE_PAIR => call_get_contrastive_pair(arguments),
            tool_names::COUNT_CONTRASTIVE_PAIRS => call_count_contrastive_pairs(arguments),
            // Typed-edge training tools (Phase 4 — F1/F2/F4 + list)
            tool_names::EXPORT_TYPED_EDGES_CORPUS => call_export_typed_edges_corpus(arguments),
            tool_names::DERIVE_ANOMALIES_FROM_EDGES => call_derive_anomalies_from_edges(arguments),
            tool_names::VALIDATE_LOW_CONFIDENCE_EDGES => call_validate_low_confidence_edges(arguments),
            tool_names::LIST_TYPED_EDGE_RECORDS => call_list_typed_edge_records(arguments),
        )
    }
}
