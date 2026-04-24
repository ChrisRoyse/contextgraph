//! Request handlers for MCP methods.
//!
//! Per PRD v6 Section 10, only these MCP tools are supported:
//! - Core: inject_context, store_memory, get_memetic_status, search_graph
//! - Consolidation: trigger_consolidation
//! - Curation: merge_concepts

mod core;
mod merge;
pub mod tools;

#[cfg(test)]
mod tests;

pub use self::core::Handlers;
pub(crate) use self::tools::daemon_tools::DaemonState;
