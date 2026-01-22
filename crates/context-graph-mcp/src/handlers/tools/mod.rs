//! MCP tool call handlers.
//!
//! PRD v6 Section 10 MCP Tools:
//! - inject_context, store_memory, search_graph (memory_tools.rs)
//! - get_memetic_status (status_tools.rs)
//! - trigger_consolidation (consolidation.rs)
//! - merge_concepts (../merge.rs)
//! - get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts (topic_tools.rs)
//! - forget_concept, boost_importance (curation_tools.rs)
//! - trigger_dream, get_dream_status (dream_tools.rs)
//! - list_watched_files, get_file_watcher_stats, delete_file_content, reconcile_files (file_watcher_tools.rs)
//! - get_conversation_context, get_session_timeline, traverse_memory_chain, compare_session_states (sequence_tools.rs)
//! - search_causes, get_causal_chain (causal_tools.rs) - E5 Causal Priority 1

mod causal_tools;
mod consolidation;
mod curation_tools;
mod dispatch;
mod dream_tools;
mod file_watcher_tools;
mod helpers;
mod memory_tools;
mod sequence_tools;
mod status_tools;
mod topic_tools;

// DTOs for PRD v6 gap tools (TASK-GAP-005)
pub mod causal_dtos;
pub mod curation_dtos;
pub mod dream_dtos;
pub mod topic_dtos;
