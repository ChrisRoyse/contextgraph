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

// ========== CURATION TOOLS (PRD Section 10.3) ==========
pub const MERGE_CONCEPTS: &str = "merge_concepts";
pub const FORGET_CONCEPT: &str = "forget_concept";
pub const BOOST_IMPORTANCE: &str = "boost_importance";

// ========== DREAM TOOLS (PRD Section 10.1) ==========
pub const TRIGGER_DREAM: &str = "trigger_dream";
pub const GET_DREAM_STATUS: &str = "get_dream_status";
