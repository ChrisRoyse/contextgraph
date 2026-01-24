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
pub const INJECT_CONTEXT: &str = "inject_context";
pub const STORE_MEMORY: &str = "store_memory";
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

// ========== DREAM TOOLS (PRD Section 10.1) ==========
pub const TRIGGER_DREAM: &str = "trigger_dream";
pub const GET_DREAM_STATUS: &str = "get_dream_status";

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
pub const GET_CAUSAL_CHAIN: &str = "get_causal_chain";

// ========== GRAPH TOOLS (E8 Upgrade - Phase 4) ==========
pub const SEARCH_CONNECTIONS: &str = "search_connections";
pub const GET_GRAPH_PATH: &str = "get_graph_path";

// ========== INTENT TOOLS (E10 Intent/Context Upgrade) ==========
pub const SEARCH_BY_INTENT: &str = "search_by_intent";
pub const FIND_CONTEXTUAL_MATCHES: &str = "find_contextual_matches";

// ========== KEYWORD TOOLS (E6 Keyword Search Enhancement) ==========
pub const SEARCH_BY_KEYWORDS: &str = "search_by_keywords";

// ========== CODE TOOLS (E7 Code Search Enhancement) ==========
pub const SEARCH_CODE: &str = "search_code";

// ========== ENTITY TOOLS (E11 Entity Integration) ==========
pub const EXTRACT_ENTITIES: &str = "extract_entities";
pub const SEARCH_BY_ENTITIES: &str = "search_by_entities";
pub const INFER_RELATIONSHIP: &str = "infer_relationship";
pub const FIND_RELATED_ENTITIES: &str = "find_related_entities";
pub const VALIDATE_KNOWLEDGE: &str = "validate_knowledge";
pub const GET_ENTITY_GRAPH: &str = "get_entity_graph";

// ========== EMBEDDER-FIRST SEARCH TOOLS (Constitution v6.3) ==========
// Enables searching using any of the 13 embedders as the primary perspective.
// Each embedder sees the knowledge graph differently - sometimes E11 finds
// what E1 misses, or E7 (code) reveals patterns E5 (causal) doesn't see.
pub const SEARCH_BY_EMBEDDER: &str = "search_by_embedder";
pub const GET_EMBEDDER_CLUSTERS: &str = "get_embedder_clusters";
pub const COMPARE_EMBEDDER_VIEWS: &str = "compare_embedder_views";
pub const LIST_EMBEDDER_INDEXES: &str = "list_embedder_indexes";
