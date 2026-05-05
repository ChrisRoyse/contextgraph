//! MCP tool definitions following the MCP 2024-11-05 protocol specification.
//!
//! This module defines the tools available through the MCP server's `tools/list`
//! and `tools/call` endpoints.
//!
//! # Module Structure
//!
//! - `types`: Core type definitions (`ToolDefinition`)
//! - `names`: Tool name constants for dispatch matching
//! - `registry`: Centralized tool registry with O(1) lookup
//! - `definitions`: Tool definitions organized by category
//!   - `core`: Core tools (store_memory, search_graph, get_memetic_status)
//!   - `topic`: Topic tools (get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts)
//!   - `curation`: Curation tools (merge_concepts, forget_concept, boost_importance)
//!
//! Note: inject_context was merged into store_memory. When rationale is provided,
//! the same validation (1-1024 chars) and response format is used.

pub mod aliases;
pub mod definitions;
pub mod names;
pub mod types;

use std::sync::atomic::{AtomicBool, Ordering};

pub use self::definitions::get_tool_definitions;
pub use self::names as tool_names;

static REALITY_LOOP_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_reality_loop_mode(enabled: bool) {
    REALITY_LOOP_MODE.store(enabled, Ordering::SeqCst);
}

pub fn is_reality_loop_mode() -> bool {
    REALITY_LOOP_MODE.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_serialization() {
        let tools = get_tool_definitions();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("store_memory"));
        assert!(json.contains("inputSchema"));
    }

    #[test]
    fn test_store_memory_schema() {
        let tools = get_tool_definitions();
        let store = tools.iter().find(|t| t.name == "store_memory").unwrap();

        let schema = &store.input_schema;
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
        // rationale is optional now (merged from inject_context)
        let props = schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("rationale"));
    }
}
