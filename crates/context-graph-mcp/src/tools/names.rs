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

// ========== INTENT TOOLS (E10 Intent/Context Upgrade) ==========
// Note: search_by_intent now handles both query-based and context-based searches
// (formerly separate find_contextual_matches tool was merged into search_by_intent)
pub const SEARCH_BY_INTENT: &str = "search_by_intent";

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
// Enables searching using any of the 13 embedders as the primary perspective.
// Each embedder sees the knowledge graph differently - sometimes E11 finds
// what E1 misses, or E7 (code) reveals patterns E5 (causal) doesn't see.
pub const SEARCH_BY_EMBEDDER: &str = "search_by_embedder";
pub const GET_EMBEDDER_CLUSTERS: &str = "get_embedder_clusters";
pub const COMPARE_EMBEDDER_VIEWS: &str = "compare_embedder_views";
pub const LIST_EMBEDDER_INDEXES: &str = "list_embedder_indexes";

// ========== TEMPORAL TOOLS (E2/E3 Integration) ==========
// Temporal search with E2/E3 boost applied POST-retrieval per ARCH-25.
pub const SEARCH_RECENT: &str = "search_recent";
pub const SEARCH_PERIODIC: &str = "search_periodic";

// ========== GRAPH LINKING TOOLS (K-NN Navigation and Typed Edges) ==========
// Tools for navigating the K-NN graph and exploring typed edges derived from
// embedder agreement patterns. Per ARCH-18, E5/E8 use asymmetric similarity.
pub const GET_MEMORY_NEIGHBORS: &str = "get_memory_neighbors";
pub const GET_TYPED_EDGES: &str = "get_typed_edges";
pub const TRAVERSE_GRAPH: &str = "traverse_graph";
/// Unified neighbors using Weighted RRF across all 13 embedders.
/// Per ARCH-21: Multi-space fusion via RRF, not weighted sum.
/// Per AP-60: Temporal embedders (E2-E4) excluded from semantic fusion.
pub const GET_UNIFIED_NEIGHBORS: &str = "get_unified_neighbors";
