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
/// Returns retained causal descriptions linked to source memories.
pub const SEARCH_CAUSAL_RELATIONSHIPS: &str = "search_causal_relationships";

// ========== MAINTENANCE TOOLS (Data repair and cleanup) ==========
/// Repair corrupted causal relationships by removing entries that fail deserialization.
/// Scans CF_CAUSAL_RELATIONSHIPS and deletes truncated/corrupted entries.
pub const REPAIR_CAUSAL_RELATIONSHIPS: &str = "repair_causal_relationships";

// ========== GRAPH TOOLS (E8 Upgrade - Phase 4) ==========
pub const SEARCH_CONNECTIONS: &str = "search_connections";
pub const GET_GRAPH_PATH: &str = "get_graph_path";

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
pub const MEJEPA_LOAD_LEARNER_STATE: &str = "mejepa_load_learner_state";
pub const MEJEPA_ROUTING_LOOKUP: &str = "mejepa_routing_lookup";
pub const MEJEPA_VRAM_BUDGET_REPORT: &str = "mejepa_vram_budget_report";
/// Phase 4 inference verification tool.
pub const MEJEPA_VERIFY: &str = "mcp__cgreality__mejepa_verify";
/// Phase 4 inference live-prediction readback tool.
pub const MEJEPA_PREDICT_LATEST: &str = "mcp__cgreality__mejepa_predict_latest";
/// Phase B non-mutating what-if prediction tool.
pub const MEJEPA_PREDICT_WHAT_IF: &str = "mcp__cgreality__mejepa_predict_what_if";
/// Phase G latent-space action search across candidate patches.
pub const MEJEPA_SEARCH_LATENT_ACTIONS: &str = "mcp__cgreality__mejepa_search_latent_actions";
/// Phase G counterfactual repair ranking across candidate patches.
pub const MEJEPA_RANK_CANDIDATES: &str = "mcp__cgreality__mejepa_rank_candidates";
/// TASK-EK-003 structural-hole mincut panel over ME-JEPA embedder/constellation/fingerprint graphs.
pub const MEJEPA_MINCUT_PANEL: &str = "mcp__cgreality__mejepa_mincut_panel";
/// TASK-EK-016 bedrock consistency verifier over patch diffs and chunk foundationality scores.
pub const MEJEPA_CHECK_BEDROCK_CONSISTENCY: &str =
    "mcp__cgreality__mejepa_check_bedrock_consistency";
/// TASK-EK-017 library-level and cross-library foundationality readback.
pub const MEJEPA_LIBRARY_FOUNDATIONALITY: &str = "mcp__cgreality__mejepa_library_foundationality";
/// TASK-EK-002 frozen-instrument proposal surface from residual absence signals.
pub const MEJEPA_PROPOSE_INSTRUMENT: &str = "mcp__cgreality__mejepa_propose_instrument";
/// TASK-EK-018B ranked dynamic-embedder proposal queue from absence signals.
pub const MEJEPA_PENDING_EMBEDDER_PROPOSALS: &str =
    "mcp__cgreality__mejepa_pending_embedder_proposals";
/// TASK-EK-018H pending dynamic-embedder promotions awaiting operator approval.
pub const MEJEPA_PENDING_EMBEDDER_APPROVALS: &str =
    "mcp__cgreality__mejepa_pending_embedder_approvals";
/// TASK-EK-002 operator/falsification decision gate for instrument proposals.
pub const MEJEPA_PROMOTE_INSTRUMENT_PROPOSAL: &str =
    "mcp__cgreality__mejepa_promote_instrument_proposal";
/// Phase B persisted-prediction explanation tool.
pub const MEJEPA_EXPLAIN_PREDICTION: &str = "mcp__cgreality__mejepa_explain_prediction";
/// Phase F per-prediction provenance inspection tool.
pub const MEJEPA_INSPECT_PREDICTION: &str = "mcp__cgreality__mejepa_inspect_prediction";
/// TASK-SKILL-007 consequence-to-evidence trace readback.
pub const MEJEPA_CONSEQUENCE_TRACE: &str = "mcp__cgreality__mejepa_consequence_trace";
/// TASK-SKILL-007 reverse lookup from evidence to consequence predictions.
pub const MEJEPA_EVIDENCE_TO_CONSEQUENCES: &str = "mcp__cgreality__mejepa_evidence_to_consequences";
/// Phase F byte-for-byte replay of persisted ME-JEPA predictions.
pub const MEJEPA_REPLAY_PREDICTION: &str = "mcp__cgreality__mejepa_replay_prediction";
/// TASK-PY-G-045 Q5 shift-log replay engine for predicted reality impact.
pub const MEJEPA_REALITY_IMPACT: &str = "mcp__cgreality__mejepa_reality_impact";
/// Phase A agent feedback ledger writer for live ME-JEPA predictions.
pub const MEJEPA_RECORD_AGENT_FEEDBACK: &str = "mcp__cgreality__mejepa_record_agent_feedback";
/// Phase 7 replay of a durable reality-shift log entry through the live subscriber.
pub const MEJEPA_OBSERVE_SHIFT: &str = "mcp__cgreality__mejepa_observe_shift";
/// Phase 7 shift-subscriber health, lag, watermark, and cache snapshot.
pub const MEJEPA_SUBSCRIBER_STATUS: &str = "mcp__cgreality__mejepa_subscriber_status";
/// Phase 7 full signal-capture audit bundle for a processed attempt.
pub const MEJEPA_CAPTURE_AUDIT: &str = "mcp__cgreality__mejepa_capture_audit";
/// Phase 4b TCT constellation source-of-truth inspector.
pub const MEJEPA_CONSTELLATION_INSPECT: &str = "mcp__cgreality__mejepa_constellation_inspect";
/// Phase 5 self-healing source-of-truth status inspector.
pub const MEJEPA_HEAL_STATUS: &str = "mcp__cgreality__mejepa_heal_status";
/// Phase F aggregate status across ME-JEPA runtime, subscriber, heal, quota, and VRAM surfaces.
pub const MEJEPA_DAEMON_STATUS: &str = "mcp__cgreality__mejepa_daemon_status";
/// Phase F operator pause surface for temporarily pausing ME-JEPA predictions.
pub const MEJEPA_PAUSE_PREDICTIONS: &str = "mcp__cgreality__mejepa_pause_predictions";
/// Phase F operator override surface for gold-labeling a specific ME-JEPA prediction.
pub const MEJEPA_OPERATOR_OVERRIDE_PREDICTION: &str =
    "mcp__cgreality__mejepa_operator_override_prediction";
/// TASK-EK-014 operator-contribution reporting surface.
pub const MEJEPA_OPERATOR_CONTRIBUTIONS: &str = "mcp__cgreality__mejepa_operator_contributions";
/// Phase 5 operator-locked rollback to a promoted witness-chain offset.
pub const MEJEPA_ROLLBACK_TO: &str = "mcp__cgreality__mejepa_rollback_to";
/// Phase E approval gate for catastrophic ME-JEPA self-optimization promotions.
pub const MEJEPA_PROMOTE_APPROVAL: &str = "mcp__cgreality__mejepa_promote_approval";
/// Phase 6 nightly hygiene GC orchestrator.
pub const MEJEPA_GC_RUN: &str = "mcp__cgreality__mejepa_gc_run";
/// Phase 6 quota source-of-truth readback.
pub const MEJEPA_QUOTA_STATUS: &str = "mcp__cgreality__mejepa_quota_status";
/// Phase 6 old witness-chain Merkle compression.
pub const MEJEPA_WITNESS_COMPRESS: &str = "mcp__cgreality__mejepa_witness_compress";
/// Phase 8 public holdout evaluation runner.
pub const MEJEPA_EVAL_RUN: &str = "mcp__cgreality__mejepa_eval_run";
/// Phase 8 latest ship-gate status readback.
pub const MEJEPA_SHIP_GATE_STATUS: &str = "mcp__cgreality__mejepa_ship_gate_status";
/// Phase F weekly evaluation dashboard from CF_MEJEPA_EVAL_REPORTS plus export files.
pub const MEJEPA_WEEKLY_EVAL_DASHBOARD: &str = "mcp__cgreality__mejepa_weekly_eval_dashboard";
/// Phase 8 active-learning queue status readback.
pub const MEJEPA_ACTIVE_LEARNING_QUEUE: &str = "mcp__cgreality__mejepa_active_learning_queue";
/// Phase G fingerprint catalog list readback.
pub const MEJEPA_FINGERPRINT_LIST: &str = "mcp__cgreality__mejepa_fingerprint_list";
/// Phase G fingerprint catalog single-record inspection.
pub const MEJEPA_FINGERPRINT_INSPECT: &str = "mcp__cgreality__mejepa_fingerprint_inspect";
/// Phase G fingerprint catalog classifier surface.
pub const MEJEPA_FINGERPRINT_CLASSIFY: &str = "mcp__cgreality__mejepa_fingerprint_classify";
/// Phase G Unknown/OOD fingerprint cluster proposal readback.
pub const MEJEPA_FINGERPRINT_SUGGEST_NEW: &str = "mcp__cgreality__mejepa_fingerprint_suggest_new";
/// Phase G operator label surface for Unknown fingerprint candidates.
pub const MEJEPA_FINGERPRINT_LABEL: &str = "mcp__cgreality__mejepa_fingerprint_label";
/// Phase G canonical-reference promotion surface for fingerprint catalog rows.
pub const MEJEPA_FINGERPRINT_PROMOTE_CANONICAL: &str =
    "mcp__cgreality__mejepa_fingerprint_promote_canonical";
/// Phase G operator-triggered per-fingerprint tau recalibration.
pub const MEJEPA_FINGERPRINT_RECALIBRATE: &str = "mcp__cgreality__mejepa_fingerprint_recalibrate";
/// Phase G per-fingerprint EWC/CBP catalog stability readback.
pub const MEJEPA_FINGERPRINT_CATALOG_STABILITY: &str =
    "mcp__cgreality__mejepa_fingerprint_catalog_stability";
/// Phase G reward-signal completeness audit readback.
pub const MEJEPA_AUDIT_REWARD_SIGNALS: &str = "mcp__cgreality__mejepa_audit_reward_signals";
/// Phase G/EK-001 compression-progress readback from CF_MEJEPA_TRAIN_CERTS.
pub const MEJEPA_COMPRESSION_PROGRESS: &str = "mcp__cgreality__mejepa_compression_progress";
/// TASK-PY-G-063 project reality-compiler ingest surface.
pub const MEJEPA_PROJECT_INGEST: &str = "mcp__cgreality__mejepa_project_ingest";
/// TASK-PY-G-064 per-project reality compile report surface.
pub const MEJEPA_PROJECT_REPORT: &str = "mcp__cgreality__mejepa_project_report";
/// Phase 8 patch similarity graph builder/readback.
pub const MEJEPA_EVAL_BUILD_GRAPH: &str = "mcp__cgreality__mejepa_eval_build_graph";
/// UTML pairwise mutual-information audit with CSV source-of-truth readback.
pub const MEJEPA_AUDIT_PAIRWISE_MI: &str = "mcp__cgreality__mejepa_audit_pairwise_mi";
/// TASK-PY-G-120 skill-to-code readback over #414 chunk-skill memberships.
pub const MEJEPA_SKILL_TO_CODE: &str = "mcp__cgreality__mejepa_skill_to_code";
/// TASK-PY-G-120 chunk/code-to-skill readback over #414 chunk-skill memberships.
pub const MEJEPA_CODE_TO_SKILL: &str = "mcp__cgreality__mejepa_code_to_skill";
/// TASK-PY-G-120 boolean set query over persisted chunk-skill memberships.
pub const MEJEPA_SKILL_SET_QUERY: &str = "mcp__cgreality__mejepa_skill_set_query";
/// TASK-PY-G-120 chunk-skill coverage audit for active-learning gaps.
pub const MEJEPA_SKILL_COVERAGE_AUDIT: &str = "mcp__cgreality__mejepa_skill_coverage_audit";
/// TASK-PY-G-120 chunk-as-star readback over constellation memberships.
pub const MEJEPA_CHUNK_AS_STAR: &str = "mcp__cgreality__mejepa_chunk_as_star";
/// TASK-PY-G-120 named constellation membership readback for a chunk.
pub const MEJEPA_CONSTELLATION_MEMBERSHIP: &str = "mcp__cgreality__mejepa_constellation_membership";
/// TASK-PY-G-120 refactoring blast-radius traversal over chunk-skill memberships.
pub const MEJEPA_SKILL_IMPACT: &str = "mcp__cgreality__mejepa_skill_impact";
/// TASK-PY-G-120 skill co-membership graph inspect surface.
pub const MEJEPA_SKILL_GRAPH_INSPECT: &str = "mcp__cgreality__mejepa_skill_graph_inspect";
/// TASK-PY-G-120 zero-cooccurrence conflict candidate graph.
pub const MEJEPA_SKILL_CONFLICT_GRAPH: &str = "mcp__cgreality__mejepa_skill_conflict_graph";
/// TASK-PY-G-120 operator browse surface for the skill catalog.
pub const MEJEPA_SKILL_BROWSE: &str = "mcp__cgreality__mejepa_skill_browse";
/// TASK-PY-G-112B durable mistake-loop writer over online-head state.
pub const MEJEPA_RECORD_MISTAKE: &str = "mcp__cgreality__mejepa_record_mistake";
/// TASK-PY-G-112B durable mistake-loop history readback.
pub const MEJEPA_MISTAKE_HISTORY: &str = "mcp__cgreality__mejepa_mistake_history";
/// TASK-PY-G-112B online-head repeat-mistake status readback.
pub const MEJEPA_MISTAKE_LOOP_STATUS: &str = "mcp__cgreality__mejepa_mistake_loop_status";
/// TASK-PY-G-119 top-K binary-leaf consequence pathway surface.
pub const MEJEPA_PATHWAY_SURFACE: &str = "mcp__cgreality__mejepa_pathway_surface";
/// TASK-PY-G-119 single pathway/tree inspection readback.
pub const MEJEPA_PATHWAY_INSPECT: &str = "mcp__cgreality__mejepa_pathway_inspect";
/// TASK-PY-G-119 operator pathway choice writer.
pub const MEJEPA_PATHWAY_RECORD_CHOICE: &str = "mcp__cgreality__mejepa_pathway_record_choice";
/// TASK-PY-G-119 operator pathway choice history.
pub const MEJEPA_PATHWAY_HISTORY: &str = "mcp__cgreality__mejepa_pathway_history";
/// Phase A bootstrap status readback from ME-JEPA RocksDB sources of truth.
pub const MEJEPA_BOOTSTRAP_STATUS: &str = "mcp__cgreality__mejepa_bootstrap_status";
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

// ========== TYPED-EDGE TRAINING TOOLS (Phase 4 — F1/F2) ==========
/// Export typed edges as `TypedEdgeTrainingRecord`s persisted to
/// `CF_TYPED_EDGE_RECORDS`.
pub const EXPORT_TYPED_EDGES_CORPUS: &str = "export_typed_edges_corpus";
/// Derive anomaly pairs directly from typed-edge classifications and persist
/// them to `CF_CONTRASTIVE_PAIRS` (F2).
pub const DERIVE_ANOMALIES_FROM_EDGES: &str = "derive_anomalies_from_edges";
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
/// Build immutable DynamicJEPA constellation centroids.
pub const DYNAMICJEPA_BUILD_CONSTELLATION: &str = "dynamicjepa_build_constellation";
/// List DynamicJEPA constellation centroids.
pub const DYNAMICJEPA_LIST_CONSTELLATIONS: &str = "dynamicjepa_list_constellations";
/// Read one DynamicJEPA constellation centroid.
pub const DYNAMICJEPA_GET_CONSTELLATION: &str = "dynamicjepa_get_constellation";
/// Calibrate DynamicJEPA G_tau thresholds.
pub const DYNAMICJEPA_CALIBRATE_THRESHOLD: &str = "dynamicjepa_calibrate_threshold";
/// Supersede a DynamicJEPA G_tau threshold calibration.
pub const DYNAMICJEPA_RECALIBRATE_THRESHOLD: &str = "dynamicjepa_recalibrate_threshold";
/// Aggregate DynamicJEPA audit-log signal_yield rows into MC-ratio evidence.
pub const DYNAMICJEPA_COMPUTE_MC_RATIO: &str = "dynamicjepa_compute_mc_ratio";
/// Estimate persisted DynamicJEPA pairwise mutual information and write release-D evidence.
pub const DYNAMICJEPA_AUDIT_PAIRWISE_MI: &str = "dynamicjepa_audit_pairwise_mi";
/// Run the counter_world to gridworld DynamicJEPA cross-domain transfer pilot.
pub const DYNAMICJEPA_CROSS_DOMAIN_TRANSFER: &str = "dynamicjepa_cross_domain_transfer";
/// Build a persisted compiler/LSP-backed semantic index for a repair repository.
pub const DYNAMICJEPA_BUILD_SEMANTIC_INDEX: &str = "dynamicjepa_build_semantic_index";
/// Validate real SWE-loop DynamicJEPA corpus diversity from persisted RocksDB rows.
pub const DYNAMICJEPA_VALIDATE_CORPUS_DIVERSITY: &str = "dynamicjepa_validate_corpus_diversity";
/// Attribute verifier/test deltas from real coverage and failure-signature evidence.
pub const DYNAMICJEPA_ATTRIBUTE_TEST_DELTA: &str = "dynamicjepa_attribute_test_delta";
/// Compare candidate-vs-active DynamicJEPA live-shadow utility evidence.
pub const DYNAMICJEPA_COMPARE_SHADOW_UTILITY: &str = "dynamicjepa_compare_shadow_utility";
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

// ========== CGREALITY REALITY-LOOP TOOLS ==========
pub const REALITY_LATEST_ROOT: &str = "reality_latest_root";
pub const REALITY_ATTEMPT_SUMMARY: &str = "reality_attempt_summary";
pub const REALITY_OFFICIAL_REPORT: &str = "reality_official_report";
pub const REALITY_PROBLEM_PACKET: &str = "reality_problem_packet";
pub const REALITY_SIGNAL: &str = "reality_signal";
pub const DYNAMICJEPA_REALITY_FOR_ATTEMPT: &str = "dynamicjepa_reality_for_attempt";
pub const REALITY_FAILURE: &str = "reality_failure";
pub const REALITY_TRIGGER_DECISION: &str = "reality_trigger_decision";
pub const REALITY_HARNESS_TRANSITIONS: &str = "reality_harness_transitions";
pub const REALITY_COMPARE_ATTEMPTS: &str = "reality_compare_attempts";
pub const REALITY_AUDIT_TRAIL: &str = "reality_audit_trail";
pub const REALITY_REPLAY_ARTIFACT: &str = "reality_replay_artifact";
pub const REALITY_QUERY_LEDGER: &str = "reality_query_ledger";
pub const HARNESS_OPEN_WINDOW: &str = "harness_open_window";
pub const HARNESS_APPLY_LINE_WINDOW_EDIT: &str = "harness_apply_line_window_edit";
pub const HARNESS_RUN_COMMAND: &str = "harness_run_command";
pub const HARNESS_GIT_DIFF: &str = "harness_git_diff";
pub const HARNESS_GIT_STATUS: &str = "harness_git_status";
pub const HARNESS_VERIFY_STATE: &str = "harness_verify_state";
pub const OPTIMIZER_RECORD_DECISION: &str = "optimizer_record_decision";
pub const OPTIMIZER_RECORD_RECOMMENDATION: &str = "optimizer_record_recommendation";
pub const OPTIMIZER_RECORD_HARNESS_TRANSITION: &str = "optimizer_record_harness_transition";
pub const OPTIMIZER_BANDIT_SELECT: &str = "optimizer_bandit_select";
pub const OPTIMIZER_BANDIT_RECORD_REWARD: &str = "optimizer_bandit_record_reward";
pub const OPTIMIZER_BANDIT_STATE: &str = "optimizer_bandit_state";
pub const OPTIMIZER_RECALL_RECOMMENDATIONS: &str = "optimizer_recall_recommendations";
pub const OPTIMIZER_COMPUTE_INFLUENCE: &str = "optimizer_compute_influence";
pub const OPTIMIZER_WITNESS_CHAIN_VERIFY: &str = "optimizer_witness_chain_verify";
pub const OPTIMIZER_WITNESS_CHAIN_DIFF: &str = "optimizer_witness_chain_diff";
pub const OPTIMIZER_WITNESS_CHAIN_REPAIR_LEGACY: &str = "optimizer_witness_chain_repair_legacy";
pub const REALITY_SHIFT_LOG: &str = "reality_shift_log";
pub const REALITY_SHIFT_COMPARE_TO_MY_VIEW: &str = "reality_shift_compare_to_my_view";
// Phase 15: autoresearch engine
pub const EXPERIMENT_REGISTRY_LIST: &str = "experiment_registry_list";
pub const EXPERIMENT_REGISTRY_GET: &str = "experiment_registry_get";
pub const CHAMPION_STATE_GET: &str = "champion_state_get";
pub const ATTEMPTS_HISTORY_QUERY: &str = "attempts_history_query";
pub const ATTEMPTS_QUERY_REFLEXION: &str = "attempts_query_reflexion";
pub const ATTEMPTS_CRITIQUE_SUMMARY: &str = "attempts_critique_summary";
pub const ATTEMPTS_SUCCESS_STRATEGIES: &str = "attempts_success_strategies";
pub const ATTEMPTS_SYNTHESIZE: &str = "attempts_synthesize";
pub const EXPERIMENT_REGISTRY_PROPOSE: &str = "experiment_registry_propose";
pub const EXPERIMENT_REGISTRY_UPDATE_OUTCOME: &str = "experiment_registry_update_outcome";
pub const CHAMPION_STATE_PROMOTE: &str = "champion_state_promote";
