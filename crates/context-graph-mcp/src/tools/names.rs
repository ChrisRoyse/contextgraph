//! Tool names as constants for dispatch matching.
//!
//! Per PRD v6 Section 10, these MCP tools should be exposed:
//! - Core: inject_context, search_graph, store_memory, get_memetic_status
//! - Topic: get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts
//! - Consolidation: trigger_consolidation
//! - Curation: merge_concepts, forget_concept, boost_importance
//!
//! Constants marked with `#[allow(dead_code)]` are defined for future handler
//! implementations. See registry.rs for handler registration status.

// ========== CORE TOOLS (PRD Section 10.1) ==========
// Note: inject_context was merged into store_memory. When rationale is provided,
// the same validation (1-1024 chars) and response format is used.
pub const STORE_MEMORY: &str = "store_memory";
pub const STORE_MEMORIES: &str = "store_memories";
pub const GET_MEMETIC_STATUS: &str = "get_memetic_status";
pub const SEARCH_GRAPH: &str = "search_graph";

// ========== CONSOLIDATION TOOLS (PRD Section 10.1) ==========
pub const TRIGGER_CONSOLIDATION: &str = "trigger_consolidation";

// ========== TOPIC TOOLS (PRD Section 10.2) ==========
pub const GET_TOPIC_PORTFOLIO: &str = "get_topic_portfolio";
pub const GET_TOPIC_STABILITY: &str = "get_topic_stability";
pub const DETECT_TOPICS: &str = "detect_topics";
pub const GET_DIVERGENCE_ALERTS: &str = "get_divergence_alerts";
// ANALYZE_FINGERPRINTS tool will be added in future phase as diagnostic tool

// ========== CURATION TOOLS (PRD Section 10.3) ==========
pub const MERGE_CONCEPTS: &str = "merge_concepts";
pub const FORGET_CONCEPT: &str = "forget_concept";
pub const BOOST_IMPORTANCE: &str = "boost_importance";

// ========== FILE WATCHER TOOLS (File index management) ==========
pub const LIST_WATCHED_FILES: &str = "list_watched_files";
pub const GET_FILE_WATCHER_STATS: &str = "get_file_watcher_stats";
pub const DELETE_FILE_CONTENT: &str = "delete_file_content";
pub const RECONCILE_FILES: &str = "reconcile_files";

// ========== SEQUENCE TOOLS (E4 Integration - Phase 1) ==========
pub const GET_CONVERSATION_CONTEXT: &str = "get_conversation_context";
pub const GET_SESSION_TIMELINE: &str = "get_session_timeline";
pub const TRAVERSE_MEMORY_CHAIN: &str = "traverse_memory_chain";
pub const COMPARE_SESSION_STATES: &str = "compare_session_states";

// ========== CAUSAL TOOLS (E5 Priority 1 Enhancement) ==========
pub const SEARCH_CAUSES: &str = "search_causes";
pub const SEARCH_EFFECTS: &str = "search_effects";
pub const GET_CAUSAL_CHAIN: &str = "get_causal_chain";
/// Search causal relationships by description similarity with provenance.
/// Returns LLM-generated descriptions linked to source memories.
pub const SEARCH_CAUSAL_RELATIONSHIPS: &str = "search_causal_relationships";

// ========== CAUSAL DISCOVERY TOOLS (LLM-based causal analysis) ==========
/// Manually trigger causal discovery agent to analyze memories.
/// Uses Qwen2.5 LLM for causal analysis with GBNF grammar constraints.
pub const TRIGGER_CAUSAL_DISCOVERY: &str = "trigger_causal_discovery";
/// Get status and statistics of the causal discovery agent.
pub const GET_CAUSAL_DISCOVERY_STATUS: &str = "get_causal_discovery_status";

// ========== MAINTENANCE TOOLS (Data repair and cleanup) ==========
/// Repair corrupted causal relationships by removing entries that fail deserialization.
/// Scans CF_CAUSAL_RELATIONSHIPS and deletes truncated/corrupted entries.
pub const REPAIR_CAUSAL_RELATIONSHIPS: &str = "repair_causal_relationships";

// ========== GRAPH TOOLS (E8 Upgrade - Phase 4) ==========
pub const SEARCH_CONNECTIONS: &str = "search_connections";
pub const GET_GRAPH_PATH: &str = "get_graph_path";

// ========== GRAPH DISCOVERY TOOLS (LLM-based relationship discovery) ==========
/// Discover graph relationships between memories using LLM analysis.
/// Uses the graph-agent with CausalDiscoveryLLM for relationship detection.
pub const DISCOVER_GRAPH_RELATIONSHIPS: &str = "discover_graph_relationships";
/// Validate a proposed graph link between two memories using LLM analysis.
pub const VALIDATE_GRAPH_LINK: &str = "validate_graph_link";

// ========== KEYWORD TOOLS (E6 Keyword Search Enhancement) ==========
pub const SEARCH_BY_KEYWORDS: &str = "search_by_keywords";

// ========== CODE TOOLS (E7 Code Search Enhancement) ==========
pub const SEARCH_CODE: &str = "search_code";

// ========== ROBUSTNESS TOOLS (E9 HDC Blind-Spot Detection) ==========
// E9 finds what E1 misses: typos, code identifiers, character-level variations.
// Uses blind-spot detection: surfaces results with high E9 + low E1 scores.
pub const SEARCH_ROBUST: &str = "search_robust";

// ========== ENTITY TOOLS (E11 Entity Integration) ==========
pub const EXTRACT_ENTITIES: &str = "extract_entities";
pub const SEARCH_BY_ENTITIES: &str = "search_by_entities";
pub const INFER_RELATIONSHIP: &str = "infer_relationship";
pub const FIND_RELATED_ENTITIES: &str = "find_related_entities";
pub const VALIDATE_KNOWLEDGE: &str = "validate_knowledge";
pub const GET_ENTITY_GRAPH: &str = "get_entity_graph";

// ========== EMBEDDER-FIRST SEARCH TOOLS (Constitution v6.3) ==========
// Enables searching using any of the 14 embedders as the primary perspective.
// Each embedder sees the knowledge graph differently - sometimes E11 finds
// what E1 misses, or E7 (code) reveals patterns E5 (causal) doesn't see.
pub const SEARCH_BY_EMBEDDER: &str = "search_by_embedder";
pub const GET_EMBEDDER_CLUSTERS: &str = "get_embedder_clusters";
pub const COMPARE_EMBEDDER_VIEWS: &str = "compare_embedder_views";
pub const LIST_EMBEDDER_INDEXES: &str = "list_embedder_indexes";
pub const GET_MEMORY_FINGERPRINT: &str = "get_memory_fingerprint";
/// Create a session-scoped custom weight profile for reuse.
pub const CREATE_WEIGHT_PROFILE: &str = "create_weight_profile";
/// Find memories that score high in one embedder but low in another.
pub const SEARCH_CROSS_EMBEDDER_ANOMALIES: &str = "search_cross_embedder_anomalies";
// ========== TEMPORAL TOOLS (E2/E3 Integration) ==========
// Temporal search with E2/E3 boost applied POST-retrieval per ARCH-25.
pub const SEARCH_RECENT: &str = "search_recent";
pub const SEARCH_PERIODIC: &str = "search_periodic";

// ========== TOKEN/EXPANSION SEARCH TOOLS (E12/E13 Standalone) ==========
pub const SEARCH_BY_TOKENS: &str = "search_by_tokens";
pub const SEARCH_BY_EXPANSION: &str = "search_by_expansion";

// ========== GRAPH LINKING TOOLS (K-NN Navigation and Typed Edges) ==========
// Tools for navigating the K-NN graph and exploring typed edges derived from
// embedder agreement patterns. Per ARCH-18, E5/E8 use asymmetric similarity.
pub const GET_MEMORY_NEIGHBORS: &str = "get_memory_neighbors";
pub const GET_TYPED_EDGES: &str = "get_typed_edges";
pub const TRAVERSE_GRAPH: &str = "traverse_graph";
/// Unified neighbors using Weighted RRF across all 14 embedders.
/// Per ARCH-21: Multi-space fusion via RRF, not weighted sum.
/// Per AP-60: Temporal embedders (E2-E4) excluded from semantic fusion.
pub const GET_UNIFIED_NEIGHBORS: &str = "get_unified_neighbors";

// ========== GRAPH LEARNING TOOLS ==========
/// Persist a graph-navigation outcome as a LearningEvent backed by real
/// EdgeRepository evidence.
pub const RECORD_GRAPH_LEARNING_EVENT: &str = "record_graph_learning_event";
/// Resolve a learned graph ranking policy from persisted graph LearningEvents.
pub const RESOLVE_GRAPH_LEARNING_POLICY: &str = "resolve_graph_learning_policy";

// ========== DAEMON TOOLS (Multi-agent observability) ==========
/// Returns daemon health metrics: active connections, model state, background tasks.
pub const DAEMON_STATUS: &str = "daemon_status";

// ========== CAPABILITY DISCOVERY TOOLS ==========
/// Return the exposed MCP capability matrix: embedders, learner-state slots,
/// tool groups, source-of-truth CFs, and optional runtime counts.
pub const GET_CAPABILITY_MATRIX: &str = "get_capability_matrix";

// ========== PROVENANCE TOOLS (Phase P3 - Provenance Queries) ==========
/// Query audit log for a specific memory or time range.
pub const GET_AUDIT_TRAIL: &str = "get_audit_trail";
/// Show merge lineage and history for a fingerprint.
pub const GET_MERGE_HISTORY: &str = "get_merge_history";
/// Full provenance chain from embedding to source for a memory.
pub const GET_PROVENANCE_CHAIN: &str = "get_provenance_chain";

// ========== TRAINING TOOLS (Training data export) ==========
/// Export memories as fully-labeled `TrainingRecord`s persisted to
/// `CF_TRAINING_RECORDS`. Output is RocksDB-only (no external formats).
pub const EXPORT_TRAINING_CORPUS: &str = "export_training_corpus";
/// List UUIDs currently stored in `CF_TRAINING_RECORDS` with basic shape stats.
pub const LIST_TRAINING_RECORDS: &str = "list_training_records";
/// Fetch one training record by memory UUID (optionally without heavy vectors).
pub const GET_TRAINING_RECORD: &str = "get_training_record";
/// Count rows currently stored in `CF_TRAINING_RECORDS`.
pub const COUNT_TRAINING_RECORDS: &str = "count_training_records";

// ========== LEARNING-AS-UTL TOOLS ==========
/// Persist one before/after learning event to `CF_LEARNING_EVENTS`.
pub const RECORD_LEARNING_EVENT: &str = "record_learning_event";
/// List UUIDs currently stored in `CF_LEARNING_EVENTS`.
pub const LIST_LEARNING_EVENTS: &str = "list_learning_events";
/// Fetch one Learning-as-UTL event by UUID.
pub const GET_LEARNING_EVENT: &str = "get_learning_event";
/// Count rows currently stored in `CF_LEARNING_EVENTS`.
pub const COUNT_LEARNING_EVENTS: &str = "count_learning_events";
/// List deterministic Learning-as-UTL signal embedders.
pub const LIST_LEARNING_SIGNAL_EMBEDDERS: &str = "list_learning_signal_embedders";
/// Compute deterministic Learning-as-UTL features and signal embeddings from an inline transition.
pub const COMPUTE_LEARNING_SIGNALS: &str = "compute_learning_signals";
/// Embed one persisted LearningEvent with deterministic signal embedders.
pub const EMBED_LEARNING_EVENT_SIGNALS: &str = "embed_learning_event_signals";
/// Predict a candidate learning transition's outcome from nearest persisted
/// LearningEvent cases.
pub const ESTIMATE_LEARNING_OUTCOME: &str = "estimate_learning_outcome";
/// Compile learner/learning CF rows into a matrix-shaped RocksDB dataset.
pub const EXPORT_LEARNER_TRAINING_DATASET: &str = "export_learner_training_dataset";
/// List matrix-shaped learner training datasets.
pub const LIST_LEARNER_TRAINING_DATASETS: &str = "list_learner_training_datasets";
/// Fetch one learner training dataset by dataset UUID.
pub const GET_LEARNER_TRAINING_DATASET: &str = "get_learner_training_dataset";
/// Count learner training datasets.
pub const COUNT_LEARNER_TRAINING_DATASETS: &str = "count_learner_training_datasets";

// ========== UTL LEARNER-STATE TOOLS ==========
/// Register a learner profile in `CF_LEARNER_PROFILE`.
pub const REGISTER_LEARNER: &str = "register_learner";
/// Persist a learner session observation and state vector.
pub const RECORD_SESSION_OBSERVATION: &str = "record_session_observation";
/// Compute the UTL Delta S surprise signal.
pub const COMPUTE_DELTA_S: &str = "compute_delta_s";
/// Compute the UTL Delta C coherence signal.
pub const COMPUTE_DELTA_C: &str = "compute_delta_c";
/// Compute the UTL Delta E embodied-state signal.
pub const COMPUTE_DELTA_E: &str = "compute_delta_e";
/// Compute UTL L = DeltaS * DeltaC * DeltaE.
pub const COMPUTE_L: &str = "compute_L";
/// Read a persisted learner M(t) trace.
pub const GET_LEARNER_M: &str = "get_learner_M";
/// Compute next-review timestamp for a learner trace.
pub const NEXT_REVIEW_FOR_TRACE: &str = "next_review_for_trace";
/// List the canonical E1-E21 content + learner-state embedder matrix.
pub const LIST_LEARNER_EMBEDDERS: &str = "list_learner_embedders";
/// Inspect cached E15-E21 model assets and real calibration datasets.
pub const PREFLIGHT_LEARNER_ASSETS: &str = "preflight_learner_assets";
/// Count all learner-state RocksDB column families.
pub const COUNT_LEARNER_STATE: &str = "count_learner_state";
/// Read persisted learner profile/session state/delta rows.
pub const GET_LEARNER_STATE: &str = "get_learner_state";
/// Persist a sleep-derived k(tau) row.
pub const RECORD_LEARNER_K_SLEEP: &str = "record_learner_k_sleep";
/// Persist a retrieval-practice row tied to a learner trace.
pub const RECORD_LEARNER_RETRIEVAL: &str = "record_learner_retrieval";
/// Persist an expert goal centroid for a skill and learner modality.
pub const UPSERT_GOAL_CENTROID: &str = "upsert_goal_centroid";
/// Compare a learner session embedding to a persisted goal centroid.
pub const GET_GOAL_DISTANCE: &str = "get_goal_distance";
/// Compile and persist a learner regulated-state baseline constellation.
pub const COMPILE_LEARNER_CONSTELLATION: &str = "compile_learner_constellation";
/// Resolve the state-conditioned retrieval weight profile for a persisted learner session.
pub const RESOLVE_LEARNER_RETRIEVAL_POLICY: &str = "resolve_learner_retrieval_policy";

// ========== CONSTELLATION TOOLS (Phase 2 compiler) ==========
/// Compile a constellation (per-embedder centroids + spread stats) from a
/// caller-selected set of memories and persist it to `CF_CONSTELLATIONS`.
pub const COMPILE_CONSTELLATION: &str = "compile_constellation";
/// List constellation UUIDs stored in `CF_CONSTELLATIONS`, with optional
/// shape info.
pub const LIST_CONSTELLATIONS: &str = "list_constellations";
/// Fetch a single constellation by UUID with optional centroid arrays.
pub const GET_CONSTELLATION: &str = "get_constellation";
/// Score a candidate memory against an existing constellation.
pub const SCORE_AGAINST_CONSTELLATION: &str = "score_against_constellation";
/// Persist a derived constellation through interpolation/add/difference or
/// anti-pole mining from real candidate memories.
pub const DERIVE_CONSTELLATION: &str = "derive_constellation";
/// Delete a constellation and its selector-index entry.
pub const DELETE_CONSTELLATION: &str = "delete_constellation";

// ========== CONTRASTIVE PAIR TOOLS (Phase 3 miner) ==========
/// Mine cross-embedder anomaly pairs from the existing corpus and persist
/// them to `CF_CONTRASTIVE_PAIRS`.
pub const MINE_CONTRASTIVE_PAIRS: &str = "mine_contrastive_pairs";
/// List stored contrastive pair keys with paging and optional kind / anchor
/// filters.
pub const LIST_CONTRASTIVE_PAIRS: &str = "list_contrastive_pairs";
/// Fetch one contrastive pair by composite `(anchorId, negativeId)` key.
pub const GET_CONTRASTIVE_PAIR: &str = "get_contrastive_pair";
/// Count stored contrastive pairs, optionally filtered by anomaly kind.
pub const COUNT_CONTRASTIVE_PAIRS: &str = "count_contrastive_pairs";

// ========== TYPED-EDGE TRAINING TOOLS (Phase 4 — F1/F2/F4) ==========
/// Export typed edges as `TypedEdgeTrainingRecord`s persisted to
/// `CF_TYPED_EDGE_RECORDS`.
pub const EXPORT_TYPED_EDGES_CORPUS: &str = "export_typed_edges_corpus";
/// Derive anomaly pairs directly from typed-edge classifications and persist
/// them to `CF_CONTRASTIVE_PAIRS` (F2).
pub const DERIVE_ANOMALIES_FROM_EDGES: &str = "derive_anomalies_from_edges";
/// Run LLM validation on typed edges whose auto-derived weight is below the
/// configured ceiling and persist verdicts to `CF_TYPED_EDGE_VALIDATIONS` (F4).
pub const VALIDATE_LOW_CONFIDENCE_EDGES: &str = "validate_low_confidence_edges";
/// List composite keys (`source`, `target`, `edge_type`) currently stored in
/// `CF_TYPED_EDGE_RECORDS` with optional full-record hydration.
pub const LIST_TYPED_EDGE_RECORDS: &str = "list_typed_edge_records";

// ========== DYNAMICJEPA TOOLS (5090jepa Phase 9) ==========
/// Register a DynamicJEPA domain pack TOML into the DynamicJEPA RocksDB CFs.
pub const DYNAMICJEPA_REGISTER_DOMAIN_PACK: &str = "dynamicjepa_register_domain_pack";
/// List registered DynamicJEPA domain packs.
pub const DYNAMICJEPA_LIST_DOMAIN_PACKS: &str = "dynamicjepa_list_domain_packs";
/// Read one registered DynamicJEPA domain pack.
pub const DYNAMICJEPA_GET_DOMAIN_PACK: &str = "dynamicjepa_get_domain_pack";
/// Ingest a JSONL fixture and run its registered adapter.
pub const DYNAMICJEPA_INGEST_EVENT: &str = "dynamicjepa_ingest_event";
/// Run the adapter for a persisted raw event.
pub const DYNAMICJEPA_RUN_ADAPTER: &str = "dynamicjepa_run_adapter";
/// Materialize one or more latent panels.
pub const DYNAMICJEPA_MATERIALIZE_PANEL: &str = "dynamicjepa_materialize_panel";
/// Read one latent panel.
pub const DYNAMICJEPA_GET_PANEL: &str = "dynamicjepa_get_panel";
/// List instrument readings for a raw event.
pub const DYNAMICJEPA_LIST_INSTRUMENT_READINGS: &str = "dynamicjepa_list_instrument_readings";
/// Persist a deterministic binding between source-of-truth rows.
pub const DYNAMICJEPA_CREATE_BINDING: &str = "dynamicjepa_create_binding";
/// List persisted DynamicJEPA bindings.
pub const DYNAMICJEPA_LIST_BINDINGS: &str = "dynamicjepa_list_bindings";
/// Compile transitions/panels into trajectories.
pub const DYNAMICJEPA_COMPILE_TRAJECTORIES: &str = "dynamicjepa_compile_trajectories";
/// Read one trajectory.
pub const DYNAMICJEPA_GET_TRAJECTORY: &str = "dynamicjepa_get_trajectory";
/// List trajectories for a domain.
pub const DYNAMICJEPA_LIST_TRAJECTORIES: &str = "dynamicjepa_list_trajectories";
/// Compile one-step dataset shards.
pub const DYNAMICJEPA_COMPILE_DATASET: &str = "dynamicjepa_compile_dataset";
/// Read one dataset shard.
pub const DYNAMICJEPA_GET_DATASET_SHARD: &str = "dynamicjepa_get_dataset_shard";
/// Inspect one dataset row with decoded linked records.
pub const DYNAMICJEPA_INSPECT_DATASET_ROW: &str = "dynamicjepa_inspect_dataset_row";
/// Train a CUDA-only tiny DynamicJEPA artifact from persisted dataset shards.
pub const DYNAMICJEPA_TRAIN: &str = "dynamicjepa_train";
/// Read one training run.
pub const DYNAMICJEPA_GET_TRAINING_RUN: &str = "dynamicjepa_get_training_run";
/// Read one model artifact and optionally verify files.
pub const DYNAMICJEPA_GET_ARTIFACT: &str = "dynamicjepa_get_artifact";
/// Persist one hash-verified prediction.
pub const DYNAMICJEPA_PREDICT: &str = "dynamicjepa_predict";
/// Persist a plan trace with candidates, predictions, and guards.
pub const DYNAMICJEPA_PLAN: &str = "dynamicjepa_plan";
/// Compare predicted and observed outcomes and persist surprise when needed.
pub const DYNAMICJEPA_RECORD_SURPRISE: &str = "dynamicjepa_record_surprise";
/// Read one persisted prediction.
pub const DYNAMICJEPA_GET_PREDICTION: &str = "dynamicjepa_get_prediction";
/// Read one persisted plan trace.
pub const DYNAMICJEPA_GET_PLAN_TRACE: &str = "dynamicjepa_get_plan_trace";
/// Read one persisted surprise event.
pub const DYNAMICJEPA_GET_SURPRISE: &str = "dynamicjepa_get_surprise";
/// Count all DynamicJEPA column families.
pub const DYNAMICJEPA_INSPECT_COUNTS: &str = "dynamicjepa_inspect_counts";
/// Decode rows from one DynamicJEPA column family.
pub const DYNAMICJEPA_INSPECT_CF: &str = "dynamicjepa_inspect_cf";
