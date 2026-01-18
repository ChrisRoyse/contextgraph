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

mod consolidation;
mod curation_tools;
mod dispatch;
mod dream_tools;
mod helpers;
mod memory_tools;
mod status_tools;
mod topic_tools;

// DTOs for PRD v6 gap tools (TASK-GAP-005)
pub mod curation_dtos;
pub mod dream_dtos;
pub mod topic_dtos;
