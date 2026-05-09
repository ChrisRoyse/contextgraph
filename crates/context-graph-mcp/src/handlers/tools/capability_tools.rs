//! Capability matrix MCP tool.

use serde_json::{json, Value};
use tracing::error;

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_storage::dynamicjepa::{
    column_families::DJ_CF_NAMES, count_model_artifacts, count_training_runs,
    count_verification_runs, list_domain_packs, snapshot_dj_counts,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;

use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::super::Handlers;
use super::helpers::ToolErrorKind;

impl Handlers {
    /// Return a compact map of everything the MCP server exposes.
    ///
    /// This is intentionally a read-only source-of-truth tool for clients: it
    /// reports embedders, tool groups, persistent CFs, and live counts without
    /// requiring clients to infer the system shape from 100+ individual tools.
    pub(crate) async fn call_get_capability_matrix(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let (include_runtime_state, include_tool_schemas) = match parse_args(&args) {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "get_capability_matrix: argument validation FAILED");
                return self.tool_error_typed(id, ToolErrorKind::Validation, &e);
            }
        };

        let tools = get_tool_definitions();
        let runtime = if include_runtime_state {
            match self.capability_runtime_state().await {
                Ok(v) => Some(v),
                Err(e) => {
                    error!(error = %e, "get_capability_matrix: runtime state read FAILED");
                    return self.tool_error_typed(id, ToolErrorKind::Storage, &e);
                }
            }
        } else {
            None
        };

        let result = json!({
            "version": 1,
            "mcp": {
                "toolCount": tools.len(),
                "toolNames": tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
                "toolGroups": tool_groups(),
                "toolSchemas": if include_tool_schemas {
                    serde_json::to_value(&tools).unwrap_or_else(|_| json!([]))
                } else {
                    json!(null)
                },
                "resultFormat": {
                    "textJson": true,
                    "structuredContent": true,
                    "typedErrors": true
                }
            },
            "embedders": {
                "contentCount": NUM_EMBEDDERS,
                "learnerStateCount": 7,
                "totalSlots": 21,
                "content": content_embedders(),
                "learnerState": learner_state_embedders()
            },
            "capabilities": capability_records(),
            "sourceOfTruth": source_of_truth(),
            "runtime": runtime,
            "operationalContract": {
                "noMockDataForVerification": true,
                "failClosed": true,
                "physicalVerification": "Use this tool for live counts, then verify CF-level persistence with the relevant read/list/count tools or ldb for full-state verification."
            }
        });

        self.tool_result(id, result)
    }

    async fn capability_runtime_state(&self) -> Result<Value, String> {
        let fingerprint_count = self
            .teleological_store
            .count()
            .await
            .map_err(|e| format!("teleological_store.count failed: {e}"))?;

        let Some(store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return Err(
                "get_capability_matrix runtime counts require RocksDbTeleologicalStore".to_string(),
            );
        };

        let graph_builder_running = self
            .graph_builder
            .as_ref()
            .map(|b| b.is_running())
            .unwrap_or(false);
        let dynamicjepa = dynamicjepa_runtime_state(store)?;

        Ok(json!({
            "storage": {
                "backend": self.teleological_store.backend_type().to_string(),
                "sizeBytes": self.teleological_store.storage_size_bytes()
            },
            "contentGraph": {
                "fingerprints": fingerprint_count,
                "trainingRecords": store.count_training_records().await.map_err(|e| format!("count_training_records failed: {e}"))?,
                "learningEvents": store.count_learning_events().await.map_err(|e| format!("count_learning_events failed: {e}"))?
            },
            "derivedTraining": {
                "constellations": store.count_constellations().await.map_err(|e| format!("count_constellations failed: {e}"))?,
                "contrastivePairs": store.count_contrastive_pairs().await.map_err(|e| format!("count_contrastive_pairs failed: {e}"))?,
                "typedEdgeRecords": store.count_typed_edge_records().await.map_err(|e| format!("count_typed_edge_records failed: {e}"))?,
                "learnerTrainingDatasets": store.count_learner_training_datasets().await.map_err(|e| format!("count_learner_training_datasets failed: {e}"))?
            },
            "learnerState": {
                "learnerProfile": store.count_learner_profiles().await.map_err(|e| format!("count_learner_profiles failed: {e}"))?,
                "learnerConstellations": store.count_learner_constellations().await.map_err(|e| format!("count_learner_constellations failed: {e}"))?,
                "fingerprintsLearner": store.count_learner_fingerprints().await.map_err(|e| format!("count_learner_fingerprints failed: {e}"))?,
                "learnerMPerTrace": store.count_learner_m_traces().await.map_err(|e| format!("count_learner_m_traces failed: {e}"))?,
                "learnerStateHistory": store.count_learner_state_history().await.map_err(|e| format!("count_learner_state_history failed: {e}"))?,
                "learnerGoalStates": store.count_learner_goal_states().await.map_err(|e| format!("count_learner_goal_states failed: {e}"))?,
                "learnerRetrievalLog": store.count_learner_retrieval_logs().await.map_err(|e| format!("count_learner_retrieval_logs failed: {e}"))?,
                "learnerKSleep": store.count_learner_k_sleep().await.map_err(|e| format!("count_learner_k_sleep failed: {e}"))?,
                "goalCentroids": store.count_goal_centroids().await.map_err(|e| format!("count_goal_centroids failed: {e}"))?,
                "learnerDeltaLog": store.count_learner_delta_logs().await.map_err(|e| format!("count_learner_delta_logs failed: {e}"))?,
                "learnerAudit": store.count_learner_audit_entries().await.map_err(|e| format!("count_learner_audit_entries failed: {e}"))?
            },
            "models": {
                "localDiscoveryRetired": true,
                "causalDiscoveryLoaded": false,
                "e5CausalLoraLoaded": false,
                "codeStoreAvailable": self.code_store.is_some(),
                "codeEmbeddingProviderAvailable": self.code_embedding_provider.is_some(),
                "edgeRepositoryAvailable": self.edge_repository.is_some(),
                "graphBuilderRunning": graph_builder_running
            },
            "dynamicjepa": dynamicjepa
        }))
    }
}

fn parse_args(args: &Value) -> Result<(bool, bool), String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "get_capability_matrix arguments must be an object".to_string())?;
    for key in obj.keys() {
        if key != "includeRuntimeState" && key != "includeToolSchemas" {
            return Err(format!("unknown argument '{key}'"));
        }
    }
    let include_runtime_state = match obj.get("includeRuntimeState") {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| "includeRuntimeState must be a boolean".to_string())?,
        None => true,
    };
    let include_tool_schemas = match obj.get("includeToolSchemas") {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| "includeToolSchemas must be a boolean".to_string())?,
        None => false,
    };
    Ok((include_runtime_state, include_tool_schemas))
}

fn tool_groups() -> Value {
    json!([
        {"group": "capability", "tools": ["get_capability_matrix"]},
        {"group": "core_memory", "tools": ["store_memory", "store_memories", "search_graph", "get_memetic_status", "trigger_consolidation"]},
        {"group": "embedder_views", "tools": ["search_by_embedder", "get_embedder_clusters", "compare_embedder_views", "list_embedder_indexes", "get_memory_fingerprint", "create_weight_profile", "search_cross_embedder_anomalies"]},
        {"group": "standalone_embedders", "tools": ["search_recent", "search_periodic", "search_by_keywords", "search_code", "search_connections", "search_robust", "extract_entities", "search_by_entities", "search_by_tokens", "search_by_expansion", "search_causes", "search_effects", "search_causal_relationships"]},
        {"group": "graph_navigation", "tools": ["get_memory_neighbors", "get_typed_edges", "traverse_graph", "get_unified_neighbors", "get_graph_path"]},
        {"group": "graph_learning", "tools": ["record_graph_learning_event", "resolve_graph_learning_policy", "get_unified_neighbors"]},
        {"group": "provenance_curation", "tools": ["merge_concepts", "forget_concept", "boost_importance", "get_audit_trail", "get_merge_history", "get_provenance_chain"]},
        {"group": "learner_state_utl", "tools": ["register_learner", "record_session_observation", "compute_delta_s", "compute_delta_c", "compute_delta_e", "compute_L", "get_learner_M", "next_review_for_trace", "list_learner_embedders", "preflight_learner_assets", "count_learner_state", "get_learner_state", "record_learner_k_sleep", "record_learner_retrieval", "upsert_goal_centroid", "get_goal_distance", "compile_learner_constellation", "resolve_learner_retrieval_policy"]},
        {"group": "learning_events", "tools": ["record_learning_event", "record_graph_learning_event", "list_learning_events", "get_learning_event", "count_learning_events", "list_learning_signal_embedders", "compute_learning_signals", "embed_learning_event_signals", "estimate_learning_outcome", "resolve_graph_learning_policy", "export_learner_training_dataset", "list_learner_training_datasets", "get_learner_training_dataset", "count_learner_training_datasets"]},
        {"group": "training_factories", "tools": ["export_training_corpus", "list_training_records", "get_training_record", "count_training_records", "compile_constellation", "list_constellations", "get_constellation", "score_against_constellation", "derive_constellation", "delete_constellation", "mine_contrastive_pairs", "list_contrastive_pairs", "get_contrastive_pair", "count_contrastive_pairs", "export_typed_edges_corpus", "derive_anomalies_from_edges", "list_typed_edge_records"]},
        {"group": "dynamicjepa", "tools": dynamicjepa_tool_names()},
        {"group": "daemon_files_maintenance", "tools": ["daemon_status", "list_watched_files", "get_file_watcher_stats", "delete_file_content", "reconcile_files", "repair_causal_relationships"]}
    ])
}

fn content_embedders() -> Value {
    json!([
        {"slot": "E1", "name": "semantic_dense", "role": "semantic foundation", "primaryTools": ["store_memory", "search_graph", "search_by_embedder"]},
        {"slot": "E2", "name": "temporal_recent", "role": "recency", "primaryTools": ["search_recent", "search_graph"]},
        {"slot": "E3", "name": "temporal_periodic", "role": "periodicity", "primaryTools": ["search_periodic", "search_graph"]},
        {"slot": "E4", "name": "temporal_positional", "role": "session ordering", "primaryTools": ["get_session_timeline", "traverse_memory_chain", "compare_session_states"]},
        {"slot": "E5", "name": "causal_asymmetric", "role": "cause/effect direction", "primaryTools": ["search_causes", "search_effects", "get_causal_chain", "search_causal_relationships"]},
        {"slot": "E6", "name": "keyword_sparse", "role": "lexical sparse recall", "primaryTools": ["search_by_keywords", "search_graph"]},
        {"slot": "E7", "name": "code_shape", "role": "code intent and structure", "primaryTools": ["search_code"]},
        {"slot": "E8", "name": "graph_relational", "role": "typed graph relation direction", "primaryTools": ["search_connections", "get_graph_path"]},
        {"slot": "E9", "name": "hdc_robust", "role": "typo and corruption robustness", "primaryTools": ["search_robust", "search_cross_embedder_anomalies"]},
        {"slot": "E10", "name": "paraphrase_multimodal", "role": "paraphrase agreement", "primaryTools": ["compare_embedder_views", "search_cross_embedder_anomalies"]},
        {"slot": "E11", "name": "entity_transe", "role": "entity relationship inference", "primaryTools": ["extract_entities", "search_by_entities", "infer_relationship", "find_related_entities", "get_entity_graph"]},
        {"slot": "E12", "name": "late_interaction_colbert", "role": "token-level MaxSim", "primaryTools": ["search_by_tokens", "search_graph"]},
        {"slot": "E13", "name": "splade_expansion", "role": "sparse expansion", "primaryTools": ["search_by_expansion", "search_graph"]},
        {"slot": "E14", "name": "bge_m3_dense", "role": "multilingual dense semantic", "primaryTools": ["search_by_embedder", "search_graph", "get_memory_fingerprint"]}
    ])
}

fn learner_state_embedders() -> Value {
    json!([
        {"slot": "E15", "name": "speech_affect", "role": "voice affect state", "primaryTools": ["record_session_observation", "list_learner_embedders"]},
        {"slot": "E16", "name": "face_affect", "role": "facial engagement and affect", "primaryTools": ["record_session_observation", "list_learner_embedders"]},
        {"slot": "E17", "name": "text_affect", "role": "language affect state", "primaryTools": ["record_session_observation", "list_learner_embedders"]},
        {"slot": "E18", "name": "ppg_hrv", "role": "heart-rate variability and coherence", "primaryTools": ["record_session_observation", "compute_delta_e"]},
        {"slot": "E19", "name": "eda_stress", "role": "stress floor and arousal", "primaryTools": ["record_session_observation", "resolve_learner_retrieval_policy"]},
        {"slot": "E20", "name": "eeg_plasticity", "role": "plasticity window", "primaryTools": ["record_session_observation", "get_goal_distance"]},
        {"slot": "E21", "name": "eeg_robust", "role": "robust cognitive-state signal", "primaryTools": ["record_session_observation", "compile_learner_constellation"]}
    ])
}

fn capability_records() -> Value {
    json!([
        {"id": "multi_embedder_memory", "status": "available", "tools": ["store_memory", "store_memories", "search_graph", "get_memory_fingerprint", "list_embedder_indexes"], "sourceOfTruth": ["fingerprints", "content", "source_metadata", "emb_0..emb_13"]},
        {"id": "state_conditioned_retrieval", "status": "available_when_learner_state_exists", "tools": ["resolve_learner_retrieval_policy", "search_graph", "create_weight_profile"], "sourceOfTruth": ["learner_state_history", "learner_delta_log", "custom_weight_profiles"]},
        {"id": "learner_measurement", "status": "available", "tools": ["register_learner", "record_session_observation", "compute_delta_s", "compute_delta_c", "compute_delta_e", "compute_L", "count_learner_state"], "sourceOfTruth": ["learner_profile", "fingerprints_learner", "learner_state_history", "learner_delta_log"]},
        {"id": "closed_loop_training", "status": "available_when_events_exist", "tools": ["record_learning_event", "export_learner_training_dataset", "export_training_corpus", "count_training_records"], "sourceOfTruth": ["learning_events", "learner_training_datasets", "training_records"]},
        {"id": "graph_learning_policy", "status": "available_when_graph_edges_and_graph_learning_events_exist", "tools": ["record_graph_learning_event", "resolve_graph_learning_policy", "get_unified_neighbors"], "sourceOfTruth": ["embedder_edges", "typed_edges", "learning_events"]},
        {"id": "constellation_alignment", "status": "available_when_memory_corpus_exists", "tools": ["compile_constellation", "derive_constellation", "score_against_constellation", "compile_learner_constellation"], "sourceOfTruth": ["constellations", "constellation_by_selector", "learner_constellations"]},
        {"id": "contrastive_anomaly_curriculum", "status": "available_when_edges_or_memory_exist", "tools": ["mine_contrastive_pairs", "derive_anomalies_from_edges", "list_contrastive_pairs", "count_contrastive_pairs"], "sourceOfTruth": ["contrastive_pairs", "contrastive_by_kind", "contrastive_by_anchor", "typed_edge_records"]},
        {"id": "typed_edge_training", "status": "available_when_graph_edges_exist", "tools": ["export_typed_edges_corpus", "list_typed_edge_records"], "sourceOfTruth": ["typed_edge_records"]},
        {"id": "dynamicjepa_world_modeling", "status": "available", "tools": dynamicjepa_tool_names(), "sourceOfTruth": DJ_CF_NAMES},
        {"id": "provenance_audit", "status": "available", "tools": ["get_audit_trail", "get_merge_history", "get_provenance_chain"], "sourceOfTruth": ["audit_log", "audit_by_target", "merge_history", "embedding_registry"]},
    ])
}

fn source_of_truth() -> Value {
    json!({
        "contentGraph": ["fingerprints", "content", "source_metadata", "topic_profiles", "e1_matryoshka_128", "e6_sparse_inverted", "e12_late_interaction", "e13_splade_inverted", "emb_0", "emb_1", "emb_2", "emb_3", "emb_4", "emb_5", "emb_6", "emb_7", "emb_8", "emb_9", "emb_10", "emb_11", "emb_12", "emb_13"],
        "learnerState": ["learner_profile", "learner_constellations", "fingerprints_learner", "learner_m_per_trace", "learner_state_history", "learner_goal_states", "learner_retrieval_log", "learner_k_sleep", "goal_centroids", "learner_delta_log", "learner_audit"],
        "training": ["training_records", "learning_events", "learner_training_datasets", "constellations", "constellation_by_selector", "contrastive_pairs", "contrastive_by_kind", "contrastive_by_anchor", "typed_edge_records", "typed_edge_validations"],
        "provenance": ["audit_log", "audit_by_target", "merge_history", "importance_history", "embedding_registry"],
        "dynamicjepa": DJ_CF_NAMES
    })
}

fn dynamicjepa_runtime_state(store: &RocksDbTeleologicalStore) -> Result<Value, String> {
    let db = store.dynamicjepa_db();
    let counts = snapshot_dj_counts(db).map_err(|e| format!("snapshot_dj_counts failed: {e}"))?;
    let domains = list_domain_packs(db, 100, 0)
        .map_err(|e| format!("list_domain_packs(limit=100, offset=0) failed: {e}"))?;
    let registered_domain_packs = domains
        .iter()
        .map(|domain| {
            json!({
                "id": domain.id.as_str(),
                "version": domain.version,
                "title": domain.title,
                "instrumentCount": domain.instrument_specs.len(),
                "adapterCount": domain.adapter_specs.len(),
                "objectiveCount": domain.objective_specs.len()
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "columnFamilyCount": DJ_CF_NAMES.len(),
        "columnFamilies": DJ_CF_NAMES,
        "counts": counts,
        "domainPackCount": registered_domain_packs.len(),
        "registeredDomainPacks": registered_domain_packs,
        "trainingRunCount": count_training_runs(db).map_err(|e| format!("count_training_runs failed: {e}"))?,
        "modelArtifactCount": count_model_artifacts(db).map_err(|e| format!("count_model_artifacts failed: {e}"))?,
        "verificationRunCount": count_verification_runs(db).map_err(|e| format!("count_verification_runs failed: {e}"))?
    }))
}

fn dynamicjepa_tool_names() -> Vec<&'static str> {
    vec![
        tool_names::DYNAMICJEPA_REGISTER_DOMAIN_PACK,
        tool_names::DYNAMICJEPA_LIST_DOMAIN_PACKS,
        tool_names::DYNAMICJEPA_GET_DOMAIN_PACK,
        tool_names::DYNAMICJEPA_INGEST_EVENT,
        tool_names::DYNAMICJEPA_RUN_ADAPTER,
        tool_names::DYNAMICJEPA_MATERIALIZE_PANEL,
        tool_names::DYNAMICJEPA_GET_PANEL,
        tool_names::DYNAMICJEPA_LIST_INSTRUMENT_READINGS,
        tool_names::DYNAMICJEPA_CREATE_BINDING,
        tool_names::DYNAMICJEPA_LIST_BINDINGS,
        tool_names::DYNAMICJEPA_COMPILE_TRAJECTORIES,
        tool_names::DYNAMICJEPA_GET_TRAJECTORY,
        tool_names::DYNAMICJEPA_LIST_TRAJECTORIES,
        tool_names::DYNAMICJEPA_COMPILE_DATASET,
        tool_names::DYNAMICJEPA_GET_DATASET_SHARD,
        tool_names::DYNAMICJEPA_INSPECT_DATASET_ROW,
        tool_names::DYNAMICJEPA_TRAIN,
        tool_names::DYNAMICJEPA_GET_TRAINING_RUN,
        tool_names::DYNAMICJEPA_GET_ARTIFACT,
        tool_names::DYNAMICJEPA_PREDICT,
        tool_names::DYNAMICJEPA_PLAN,
        tool_names::DYNAMICJEPA_RECORD_SURPRISE,
        tool_names::DYNAMICJEPA_BUILD_CONSTELLATION,
        tool_names::DYNAMICJEPA_LIST_CONSTELLATIONS,
        tool_names::DYNAMICJEPA_GET_CONSTELLATION,
        tool_names::DYNAMICJEPA_CALIBRATE_THRESHOLD,
        tool_names::DYNAMICJEPA_RECALIBRATE_THRESHOLD,
        tool_names::DYNAMICJEPA_COMPUTE_MC_RATIO,
        tool_names::DYNAMICJEPA_AUDIT_PAIRWISE_MI,
        tool_names::DYNAMICJEPA_CROSS_DOMAIN_TRANSFER,
        tool_names::DYNAMICJEPA_BUILD_SEMANTIC_INDEX,
        tool_names::DYNAMICJEPA_VALIDATE_CORPUS_DIVERSITY,
        tool_names::DYNAMICJEPA_ATTRIBUTE_TEST_DELTA,
        tool_names::DYNAMICJEPA_COMPARE_SHADOW_UTILITY,
        tool_names::DYNAMICJEPA_GET_PREDICTION,
        tool_names::DYNAMICJEPA_GET_PLAN_TRACE,
        tool_names::DYNAMICJEPA_GET_SURPRISE,
        tool_names::DYNAMICJEPA_INSPECT_COUNTS,
        tool_names::DYNAMICJEPA_INSPECT_CF,
    ]
}
