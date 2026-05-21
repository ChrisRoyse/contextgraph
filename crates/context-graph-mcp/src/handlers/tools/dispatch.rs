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

    pub(crate) async fn call_e5_retired_tool(
        &self,
        id: Option<JsonRpcId>,
        _arguments: serde_json::Value,
        tool_name: &str,
    ) -> JsonRpcResponse {
        JsonRpcResponse::error(
            id,
            error_codes::INVALID_PARAMS,
            format!(
                "{} is unavailable because the E5 causal embedder is retired and disabled",
                tool_name
            ),
        )
    }

    pub(crate) async fn call_e11_disabled_tool(
        &self,
        id: Option<JsonRpcId>,
        _arguments: serde_json::Value,
        tool_name: &str,
    ) -> JsonRpcResponse {
        JsonRpcResponse::error(
            id,
            error_codes::INVALID_PARAMS,
            format!(
                "{} is unavailable because E11 entity/Kepler is disabled: the available assets are incomplete/incompatible with the runtime, and no verified code-symbol entity replacement has been installed",
                tool_name
            ),
        )
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

        if super::mejepa_phase7_tools::is_retired_cgreality_tool(tool_name) {
            return super::mejepa_phase7_tools::retired_tool_response(id, tool_name);
        }

        if !context_graph_core::weights::E11_ENTITY_ENABLED
            && matches!(
                tool_name,
                tool_names::EXTRACT_ENTITIES
                    | tool_names::SEARCH_BY_ENTITIES
                    | tool_names::INFER_RELATIONSHIP
                    | tool_names::FIND_RELATED_ENTITIES
                    | tool_names::VALIDATE_KNOWLEDGE
                    | tool_names::GET_ENTITY_GRAPH
            )
        {
            return self.call_e11_disabled_tool(id, arguments, tool_name).await;
        }

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
            // Capability discovery
            tool_names::GET_CAPABILITY_MATRIX => call_get_capability_matrix(arguments),
            // Core tools (PRD Section 10.1)
            tool_names::STORE_MEMORY => call_store_memory(arguments),
            tool_names::STORE_MEMORIES => call_store_memories(arguments),
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
            // Causal tools (E5 retired)
            tool_names::SEARCH_CAUSES => call_e5_retired_tool(arguments, tool_names::SEARCH_CAUSES),
            tool_names::SEARCH_EFFECTS => call_e5_retired_tool(arguments, tool_names::SEARCH_EFFECTS),
            tool_names::GET_CAUSAL_CHAIN => call_e5_retired_tool(arguments, tool_names::GET_CAUSAL_CHAIN),
            tool_names::SEARCH_CAUSAL_RELATIONSHIPS => call_e5_retired_tool(arguments, tool_names::SEARCH_CAUSAL_RELATIONSHIPS),
            // Graph tools (E8)
            tool_names::SEARCH_CONNECTIONS => call_search_connections(arguments),
            tool_names::GET_GRAPH_PATH => call_get_graph_path(arguments),
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
            tool_names::MEJEPA_LOAD_LEARNER_STATE => call_mejepa_load_learner_state(arguments),
            tool_names::MEJEPA_ROUTING_LOOKUP => call_mejepa_routing_lookup(arguments),
            tool_names::MEJEPA_VRAM_BUDGET_REPORT => call_mejepa_vram_budget_report(arguments),
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
            // Graph learning tools
            tool_names::RECORD_GRAPH_LEARNING_EVENT => call_record_graph_learning_event(arguments),
            tool_names::RESOLVE_GRAPH_LEARNING_POLICY => call_resolve_graph_learning_policy(arguments),
            // Maintenance tools
            tool_names::REPAIR_CAUSAL_RELATIONSHIPS => call_repair_causal_relationships(),
            // ME-JEPA Phase 4 inference tools
            tool_names::MEJEPA_VERIFY => call_mejepa_verify(arguments),
            tool_names::MEJEPA_PROJECT_INGEST => call_mejepa_project_ingest(arguments),
            tool_names::MEJEPA_PROJECT_REPORT => call_mejepa_project_report(arguments),
            tool_names::MEJEPA_PREDICT_LATEST => call_mejepa_predict_latest(arguments),
            tool_names::MEJEPA_PREDICT_WHAT_IF => call_mejepa_predict_what_if(arguments),
            tool_names::MEJEPA_SEARCH_LATENT_ACTIONS => call_mejepa_search_latent_actions(arguments),
            tool_names::MEJEPA_RANK_CANDIDATES => call_mejepa_rank_candidates(arguments),
            tool_names::MEJEPA_MINCUT_PANEL => call_mejepa_mincut_panel(arguments),
            tool_names::MEJEPA_CHECK_BEDROCK_CONSISTENCY => call_mejepa_check_bedrock_consistency(arguments),
            tool_names::MEJEPA_LIBRARY_FOUNDATIONALITY => call_mejepa_library_foundationality(arguments),
            tool_names::MEJEPA_PROPOSE_INSTRUMENT => call_mejepa_propose_instrument(arguments),
            tool_names::MEJEPA_PENDING_EMBEDDER_PROPOSALS => call_mejepa_pending_embedder_proposals(arguments),
            tool_names::MEJEPA_PENDING_EMBEDDER_APPROVALS => call_mejepa_pending_embedder_approvals(arguments),
            tool_names::MEJEPA_PROMOTE_INSTRUMENT_PROPOSAL => call_mejepa_promote_instrument_proposal(arguments),
            tool_names::MEJEPA_EXPLAIN_PREDICTION => call_mejepa_explain_prediction(arguments),
            tool_names::MEJEPA_INSPECT_PREDICTION => call_mejepa_inspect_prediction(arguments),
            tool_names::MEJEPA_CONSEQUENCE_TRACE => call_mejepa_consequence_trace(arguments),
            tool_names::MEJEPA_EVIDENCE_TO_CONSEQUENCES => call_mejepa_evidence_to_consequences(arguments),
            tool_names::MEJEPA_REPLAY_PREDICTION => call_mejepa_replay_prediction(arguments),
            tool_names::MEJEPA_REALITY_IMPACT => call_mejepa_reality_impact(arguments),
            tool_names::MEJEPA_RECORD_AGENT_FEEDBACK => call_mejepa_record_agent_feedback(arguments),
            tool_names::MEJEPA_OBSERVE_SHIFT => call_mejepa_observe_shift(arguments),
            tool_names::MEJEPA_SUBSCRIBER_STATUS => call_mejepa_subscriber_status(arguments),
            tool_names::MEJEPA_CAPTURE_AUDIT => call_mejepa_capture_audit(arguments),
            tool_names::MEJEPA_CONSTELLATION_INSPECT => call_mejepa_constellation_inspect(arguments),
            tool_names::MEJEPA_HEAL_STATUS => call_mejepa_heal_status(arguments),
            tool_names::MEJEPA_DAEMON_STATUS => call_mejepa_daemon_status(arguments),
            tool_names::MEJEPA_PAUSE_PREDICTIONS => call_mejepa_pause_predictions(arguments),
            tool_names::MEJEPA_OPERATOR_OVERRIDE_PREDICTION => call_mejepa_operator_override_prediction(arguments),
            tool_names::MEJEPA_OPERATOR_CONTRIBUTIONS => call_mejepa_operator_contributions(arguments),
            tool_names::MEJEPA_ROLLBACK_TO => call_mejepa_rollback_to(arguments),
            tool_names::MEJEPA_PROMOTE_APPROVAL => call_mejepa_promote_approval(arguments),
            tool_names::MEJEPA_GC_RUN => call_mejepa_gc_run(arguments),
            tool_names::MEJEPA_QUOTA_STATUS => call_mejepa_quota_status(arguments),
            tool_names::MEJEPA_WITNESS_COMPRESS => call_mejepa_witness_compress(arguments),
            tool_names::MEJEPA_EVAL_RUN => call_mejepa_eval_run(arguments),
            tool_names::MEJEPA_SHIP_GATE_STATUS => call_mejepa_ship_gate_status(arguments),
            tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD => call_mejepa_weekly_eval_dashboard(arguments),
            tool_names::MEJEPA_ACTIVE_LEARNING_QUEUE => call_mejepa_active_learning_queue(arguments),
            tool_names::MEJEPA_FINGERPRINT_LIST => call_mejepa_fingerprint_list(arguments),
            tool_names::MEJEPA_FINGERPRINT_INSPECT => call_mejepa_fingerprint_inspect(arguments),
            tool_names::MEJEPA_FINGERPRINT_CLASSIFY => call_mejepa_fingerprint_classify(arguments),
            tool_names::MEJEPA_FINGERPRINT_SUGGEST_NEW => call_mejepa_fingerprint_suggest_new(arguments),
            tool_names::MEJEPA_FINGERPRINT_LABEL => call_mejepa_fingerprint_label(arguments),
            tool_names::MEJEPA_FINGERPRINT_PROMOTE_CANONICAL => call_mejepa_fingerprint_promote_canonical(arguments),
            tool_names::MEJEPA_FINGERPRINT_RECALIBRATE => call_mejepa_fingerprint_recalibrate(arguments),
            tool_names::MEJEPA_FINGERPRINT_CATALOG_STABILITY => call_mejepa_fingerprint_catalog_stability(arguments),
            tool_names::MEJEPA_BOOTSTRAP_STATUS => call_mejepa_bootstrap_status(arguments),
            tool_names::MEJEPA_AUDIT_REWARD_SIGNALS => call_mejepa_audit_reward_signals(arguments),
            tool_names::MEJEPA_COMPRESSION_PROGRESS => call_mejepa_compression_progress(arguments),
            tool_names::MEJEPA_EVAL_BUILD_GRAPH => call_mejepa_eval_build_graph(arguments),
            tool_names::MEJEPA_AUDIT_PAIRWISE_MI => call_mejepa_audit_pairwise_mi(arguments),
            tool_names::MEJEPA_SKILL_TO_CODE => call_mejepa_skill_to_code(arguments),
            tool_names::MEJEPA_CODE_TO_SKILL => call_mejepa_code_to_skill(arguments),
            tool_names::MEJEPA_SKILL_SET_QUERY => call_mejepa_skill_set_query(arguments),
            tool_names::MEJEPA_SKILL_COVERAGE_AUDIT => call_mejepa_skill_coverage_audit(arguments),
            tool_names::MEJEPA_CHUNK_AS_STAR => call_mejepa_chunk_as_star(arguments),
            tool_names::MEJEPA_CONSTELLATION_MEMBERSHIP => call_mejepa_constellation_membership(arguments),
            tool_names::MEJEPA_SKILL_IMPACT => call_mejepa_skill_impact(arguments),
            tool_names::MEJEPA_SKILL_GRAPH_INSPECT => call_mejepa_skill_graph_inspect(arguments),
            tool_names::MEJEPA_SKILL_CONFLICT_GRAPH => call_mejepa_skill_conflict_graph(arguments),
            tool_names::MEJEPA_SKILL_BROWSE => call_mejepa_skill_browse(arguments),
            tool_names::MEJEPA_RECORD_MISTAKE => call_mejepa_record_mistake(arguments),
            tool_names::MEJEPA_MISTAKE_HISTORY => call_mejepa_mistake_history(arguments),
            tool_names::MEJEPA_MISTAKE_LOOP_STATUS => call_mejepa_mistake_loop_status(arguments),
            tool_names::MEJEPA_PATHWAY_SURFACE => call_mejepa_pathway_surface(arguments),
            tool_names::MEJEPA_PATHWAY_INSPECT => call_mejepa_pathway_inspect(arguments),
            tool_names::MEJEPA_PATHWAY_RECORD_CHOICE => call_mejepa_pathway_record_choice(arguments),
            tool_names::MEJEPA_PATHWAY_HISTORY => call_mejepa_pathway_history(arguments),
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
            tool_names::LIST_LEARNING_SIGNAL_EMBEDDERS => call_list_learning_signal_embedders(),
            tool_names::COMPUTE_LEARNING_SIGNALS => call_compute_learning_signals(arguments),
            tool_names::EMBED_LEARNING_EVENT_SIGNALS => call_embed_learning_event_signals(arguments),
            tool_names::ESTIMATE_LEARNING_OUTCOME => call_estimate_learning_outcome(arguments),
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
            tool_names::RESOLVE_LEARNER_RETRIEVAL_POLICY => call_resolve_learner_retrieval_policy(arguments),
            // Constellation tools (Phase 2 compiler)
            tool_names::COMPILE_CONSTELLATION => call_compile_constellation(arguments),
            tool_names::LIST_CONSTELLATIONS => call_list_constellations(arguments),
            tool_names::GET_CONSTELLATION => call_get_constellation(arguments),
            tool_names::SCORE_AGAINST_CONSTELLATION => call_score_against_constellation(arguments),
            tool_names::DERIVE_CONSTELLATION => call_derive_constellation(arguments),
            tool_names::DELETE_CONSTELLATION => call_delete_constellation(arguments),
            // Contrastive tools (Phase 3 pair miner)
            tool_names::MINE_CONTRASTIVE_PAIRS => call_mine_contrastive_pairs(arguments),
            tool_names::LIST_CONTRASTIVE_PAIRS => call_list_contrastive_pairs(arguments),
            tool_names::GET_CONTRASTIVE_PAIR => call_get_contrastive_pair(arguments),
            tool_names::COUNT_CONTRASTIVE_PAIRS => call_count_contrastive_pairs(arguments),
            // Typed-edge training tools (Phase 4 — F1/F2 + list)
            tool_names::EXPORT_TYPED_EDGES_CORPUS => call_export_typed_edges_corpus(arguments),
            tool_names::DERIVE_ANOMALIES_FROM_EDGES => call_derive_anomalies_from_edges(arguments),
            tool_names::LIST_TYPED_EDGE_RECORDS => call_list_typed_edge_records(arguments),
            // DynamicJEPA tools (5090jepa Phase 9)
            tool_names::DYNAMICJEPA_REGISTER_DOMAIN_PACK => call_dynamicjepa_register_domain_pack(arguments),
            tool_names::DYNAMICJEPA_LIST_DOMAIN_PACKS => call_dynamicjepa_list_domain_packs(arguments),
            tool_names::DYNAMICJEPA_GET_DOMAIN_PACK => call_dynamicjepa_get_domain_pack(arguments),
            tool_names::DYNAMICJEPA_INGEST_EVENT => call_dynamicjepa_ingest_event(arguments),
            tool_names::DYNAMICJEPA_RUN_ADAPTER => call_dynamicjepa_run_adapter(arguments),
            tool_names::DYNAMICJEPA_MATERIALIZE_PANEL => call_dynamicjepa_materialize_panel(arguments),
            tool_names::DYNAMICJEPA_GET_PANEL => call_dynamicjepa_get_panel(arguments),
            tool_names::DYNAMICJEPA_LIST_INSTRUMENT_READINGS => call_dynamicjepa_list_instrument_readings(arguments),
            tool_names::DYNAMICJEPA_CREATE_BINDING => call_dynamicjepa_create_binding(arguments),
            tool_names::DYNAMICJEPA_LIST_BINDINGS => call_dynamicjepa_list_bindings(arguments),
            tool_names::DYNAMICJEPA_COMPILE_TRAJECTORIES => call_dynamicjepa_compile_trajectories(arguments),
            tool_names::DYNAMICJEPA_GET_TRAJECTORY => call_dynamicjepa_get_trajectory(arguments),
            tool_names::DYNAMICJEPA_LIST_TRAJECTORIES => call_dynamicjepa_list_trajectories(arguments),
            tool_names::DYNAMICJEPA_COMPILE_DATASET => call_dynamicjepa_compile_dataset(arguments),
            tool_names::DYNAMICJEPA_GET_DATASET_SHARD => call_dynamicjepa_get_dataset_shard(arguments),
            tool_names::DYNAMICJEPA_INSPECT_DATASET_ROW => call_dynamicjepa_inspect_dataset_row(arguments),
            tool_names::DYNAMICJEPA_TRAIN => call_dynamicjepa_train(arguments),
            tool_names::DYNAMICJEPA_GET_TRAINING_RUN => call_dynamicjepa_get_training_run(arguments),
            tool_names::DYNAMICJEPA_GET_ARTIFACT => call_dynamicjepa_get_artifact(arguments),
            tool_names::DYNAMICJEPA_PREDICT => call_dynamicjepa_predict(arguments),
            tool_names::DYNAMICJEPA_PLAN => call_dynamicjepa_plan(arguments),
            tool_names::DYNAMICJEPA_RECORD_SURPRISE => call_dynamicjepa_record_surprise(arguments),
            tool_names::DYNAMICJEPA_BUILD_CONSTELLATION => call_dynamicjepa_build_constellation(arguments),
            tool_names::DYNAMICJEPA_LIST_CONSTELLATIONS => call_dynamicjepa_list_constellations(arguments),
            tool_names::DYNAMICJEPA_GET_CONSTELLATION => call_dynamicjepa_get_constellation(arguments),
            tool_names::DYNAMICJEPA_CALIBRATE_THRESHOLD => call_dynamicjepa_calibrate_threshold(arguments),
            tool_names::DYNAMICJEPA_RECALIBRATE_THRESHOLD => call_dynamicjepa_recalibrate_threshold(arguments),
            tool_names::DYNAMICJEPA_COMPUTE_MC_RATIO => call_dynamicjepa_compute_mc_ratio(arguments),
            tool_names::DYNAMICJEPA_AUDIT_PAIRWISE_MI => call_dynamicjepa_audit_pairwise_mi(arguments),
            tool_names::DYNAMICJEPA_CROSS_DOMAIN_TRANSFER => call_dynamicjepa_cross_domain_transfer(arguments),
            tool_names::DYNAMICJEPA_BUILD_SEMANTIC_INDEX => call_dynamicjepa_build_semantic_index(arguments),
            tool_names::DYNAMICJEPA_VALIDATE_CORPUS_DIVERSITY => call_dynamicjepa_validate_corpus_diversity(arguments),
            tool_names::DYNAMICJEPA_ATTRIBUTE_TEST_DELTA => call_dynamicjepa_attribute_test_delta(arguments),
            tool_names::DYNAMICJEPA_COMPARE_SHADOW_UTILITY => call_dynamicjepa_compare_shadow_utility(arguments),
            tool_names::DYNAMICJEPA_GET_PREDICTION => call_dynamicjepa_get_prediction(arguments),
            tool_names::DYNAMICJEPA_GET_PLAN_TRACE => call_dynamicjepa_get_plan_trace(arguments),
            tool_names::DYNAMICJEPA_GET_SURPRISE => call_dynamicjepa_get_surprise(arguments),
            tool_names::DYNAMICJEPA_INSPECT_COUNTS => call_dynamicjepa_inspect_counts(arguments),
            tool_names::DYNAMICJEPA_INSPECT_CF => call_dynamicjepa_inspect_cf(arguments),
            // ccreality reality-loop tools
            tool_names::REALITY_LATEST_ROOT => call_reality_latest_root(arguments),
            tool_names::REALITY_ATTEMPT_SUMMARY => call_reality_attempt_summary(arguments),
            tool_names::REALITY_OFFICIAL_REPORT => call_reality_official_report(arguments),
            tool_names::REALITY_PROBLEM_PACKET => call_reality_problem_packet(arguments),
            tool_names::REALITY_SIGNAL => call_reality_signal(arguments),
            tool_names::DYNAMICJEPA_REALITY_FOR_ATTEMPT => call_dynamicjepa_reality_for_attempt(arguments),
            tool_names::REALITY_FAILURE => call_reality_failure(arguments),
            tool_names::REALITY_TRIGGER_DECISION => call_reality_trigger_decision(arguments),
            tool_names::REALITY_HARNESS_TRANSITIONS => call_reality_harness_transitions(arguments),
            tool_names::REALITY_COMPARE_ATTEMPTS => call_reality_compare_attempts(arguments),
            tool_names::REALITY_AUDIT_TRAIL => call_reality_audit_trail(arguments),
            tool_names::REALITY_REPLAY_ARTIFACT => call_reality_replay_artifact(arguments),
            tool_names::REALITY_QUERY_LEDGER => call_reality_query_ledger(arguments),
            tool_names::HARNESS_OPEN_WINDOW => call_harness_open_window(arguments),
            tool_names::HARNESS_APPLY_LINE_WINDOW_EDIT => call_harness_apply_line_window_edit(arguments),
            tool_names::HARNESS_RUN_COMMAND => call_harness_run_command(arguments),
            tool_names::HARNESS_GIT_DIFF => call_harness_git_diff(arguments),
            tool_names::HARNESS_GIT_STATUS => call_harness_git_status(arguments),
            tool_names::HARNESS_VERIFY_STATE => call_harness_verify_state(arguments),
            tool_names::OPTIMIZER_RECORD_DECISION => call_optimizer_record_decision(arguments),
            tool_names::OPTIMIZER_RECORD_RECOMMENDATION => call_optimizer_record_recommendation(arguments),
            tool_names::OPTIMIZER_RECORD_HARNESS_TRANSITION => call_optimizer_record_harness_transition(arguments),
            tool_names::OPTIMIZER_BANDIT_SELECT => call_optimizer_bandit_select(arguments),
            tool_names::OPTIMIZER_BANDIT_RECORD_REWARD => call_optimizer_bandit_record_reward(arguments),
            tool_names::OPTIMIZER_BANDIT_STATE => call_optimizer_bandit_state(arguments),
            tool_names::OPTIMIZER_RECALL_RECOMMENDATIONS => call_optimizer_recall_recommendations(arguments),
            tool_names::OPTIMIZER_COMPUTE_INFLUENCE => call_optimizer_compute_influence(arguments),
            tool_names::OPTIMIZER_WITNESS_CHAIN_VERIFY => call_optimizer_witness_chain_verify(arguments),
            tool_names::OPTIMIZER_WITNESS_CHAIN_DIFF => call_optimizer_witness_chain_diff(arguments),
            tool_names::OPTIMIZER_WITNESS_CHAIN_REPAIR_LEGACY => call_optimizer_witness_chain_repair_legacy(arguments),
            tool_names::REALITY_SHIFT_LOG => call_reality_shift_log(arguments),
            tool_names::REALITY_SHIFT_COMPARE_TO_MY_VIEW => call_reality_shift_compare_to_my_view(arguments),
            // Phase 15: autoresearch engine
            tool_names::EXPERIMENT_REGISTRY_LIST => call_experiment_registry_list(arguments),
            tool_names::EXPERIMENT_REGISTRY_GET => call_experiment_registry_get(arguments),
            tool_names::CHAMPION_STATE_GET => call_champion_state_get(arguments),
            tool_names::ATTEMPTS_HISTORY_QUERY => call_attempts_history_query(arguments),
            tool_names::ATTEMPTS_QUERY_REFLEXION => call_attempts_query_reflexion(arguments),
            tool_names::ATTEMPTS_CRITIQUE_SUMMARY => call_attempts_critique_summary(arguments),
            tool_names::ATTEMPTS_SUCCESS_STRATEGIES => call_attempts_success_strategies(arguments),
            tool_names::ATTEMPTS_SYNTHESIZE => call_attempts_synthesize(arguments),
            tool_names::EXPERIMENT_REGISTRY_PROPOSE => call_experiment_registry_propose(arguments),
            tool_names::EXPERIMENT_REGISTRY_UPDATE_OUTCOME => call_experiment_registry_update_outcome(arguments),
            tool_names::CHAMPION_STATE_PROMOTE => call_champion_state_promote(arguments),
        )
    }
}
