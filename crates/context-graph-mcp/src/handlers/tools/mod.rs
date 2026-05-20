//! MCP tool call handlers.
//!
//! PRD v6 Section 10 MCP Tools:
//! - store_memory, search_graph (memory_tools.rs) - inject_context merged into store_memory
//! - get_memetic_status (status_tools.rs)
//! - trigger_consolidation (consolidation.rs)
//! - merge_concepts (../merge.rs)
//! - get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts (topic_tools.rs)
//! - forget_concept, boost_importance (curation_tools.rs)
//! - list_watched_files, get_file_watcher_stats, delete_file_content, reconcile_files (file_watcher_tools.rs)
//! - get_conversation_context, get_session_timeline, traverse_memory_chain, compare_session_states (sequence_tools.rs)
//! - search_causes, get_causal_chain (causal_tools.rs) - E5 Causal Priority 1
//! - search_by_keywords (keyword_tools.rs) - E6 Keyword Search Enhancement
//! - search_code (code_tools.rs) - E7 Code Search Enhancement
//! - search_connections, get_graph_path (graph_tools.rs) - E8 Upgrade Phase 4
//! - search_robust (robustness_tools.rs) - E9 HDC Blind-Spot Detection
//! - extract_entities, search_by_entities, infer_relationship, find_related_entities, validate_knowledge, get_entity_graph (entity_tools.rs) - E11 Entity Integration
//! - search_by_embedder, get_embedder_clusters, compare_embedder_views, list_embedder_indexes (embedder_tools.rs) - Constitution v6.3 Embedder-First Search
//! - search_recent (temporal_tools.rs) - E2 V_freshness Temporal Search
//! - get_memory_neighbors, get_typed_edges, traverse_graph (graph_link_tools.rs) - K-NN Graph Linking

mod capability_tools;
mod causal_relationship_tools;
mod causal_tools;
mod code_tools;
pub(crate) mod consolidation;
mod constellation_tools;
mod contrastive_tools;
mod curation_tools;
pub(crate) mod daemon_tools;
mod dispatch;
mod dynamicjepa_tools;
mod embedder_tools;
mod entity_tools;
mod file_watcher_tools;
pub(crate) mod graph_learning_tools;
mod graph_link_tools;
mod graph_tools;
pub(crate) mod helpers;
mod keyword_tools;
mod learning_tools;
// Intentionally placed here (alphabetical within private modules is not required;
// this sits next to helpers which consumes it).
mod maintenance_tools;
mod mejepa_agent_identity;
mod mejepa_compression_progress_tools;
pub mod mejepa_eval_tools;
mod mejepa_fingerprint_tools;
mod mejepa_hygiene_tools;
mod mejepa_mistake_loop_tools;
mod mejepa_pathway_tools;
mod mejepa_phase7_storage;
mod mejepa_phase7_tools;
mod mejepa_reward_audit_tools;
mod mejepa_skill_linkage_tools;
mod mejepa_tools;
mod mejepa_utml_tools;
mod mejepa_weekly_dashboard_status;
mod mejepa_weekly_dashboard_tools;
mod memory_tools;
mod provenance_tools;
pub(crate) mod reality_loop;
mod robustness_tools;
mod sequence_tools;
mod status_tools;
mod temporal_tools;
mod topic_tools;
mod training_tools;
mod typed_edges_tools;
mod utl_tools;
pub(crate) mod validate;

/// Test-only shared environment-variable lock.
///
/// Several integration tests across this module set/remove process-wide env
/// vars (`CONTEXTGRAPH_MEJEPA_INFER_DB`, `CONTEXTGRAPH_MEJEPA_PANEL_DB`, etc.).
/// Cargo runs tests in parallel by default, so two tests in *different*
/// submodules can race even though each submodule serializes within itself.
/// This shared lock is the cross-module synchronization point.
#[cfg(test)]
pub(super) mod test_env_lock {
    use std::sync::{Mutex, MutexGuard};

    pub static LOCK: Mutex<()> = Mutex::new(());

    pub fn lock() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }
}

#[cfg(test)]
mod mejepa_fingerprint_tools_fsv_tests {
    #[test]
    fn fingerprint_mcp_tools_write_fsv_artifact() {
        super::mejepa_fingerprint_tools::run_fingerprint_mcp_tools_write_fsv_artifact();
    }
}

#[cfg(test)]
mod mejepa_reward_audit_tools_fsv_tests {
    #[test]
    fn reward_signal_audit_writes_fsv_artifact() {
        super::mejepa_reward_audit_tools::run_reward_signal_audit_write_fsv_artifact();
    }
}

#[cfg(test)]
mod mejepa_compression_progress_tools_fsv_tests {
    #[test]
    fn compression_progress_writes_fsv_artifact() {
        super::mejepa_compression_progress_tools::run_compression_progress_write_fsv_artifact();
    }
}

// DTOs for PRD v6 gap tools (TASK-GAP-005)
pub mod causal_dtos;
pub mod code_dtos;
pub mod curation_dtos;
pub mod dynamicjepa_dtos;
pub mod embedder_dtos;
pub mod entity_dtos;
pub mod graph_dtos;
pub mod graph_link_dtos;
pub mod keyword_dtos;
pub mod provenance_dtos;
pub mod robustness_dtos;
pub mod temporal_dtos;
pub mod topic_dtos;
