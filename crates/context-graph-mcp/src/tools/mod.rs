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
//!   - `core`: Core tools (inject_context, store_memory, search_graph, get_memetic_status)
//!   - `topic`: Topic tools (get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts)
//!   - `curation`: Curation tools (merge_concepts, forget_concept, boost_importance)

pub mod aliases;
pub mod definitions;
pub mod names;
pub mod registry;
pub mod types;

pub use self::definitions::get_tool_definitions;
pub use self::names as tool_names;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_serialization() {
        let tools = get_tool_definitions();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("inject_context"));
        assert!(json.contains("inputSchema"));
    }

    #[test]
    fn test_inject_context_schema() {
        let tools = get_tool_definitions();
        let inject = tools.iter().find(|t| t.name == "inject_context").unwrap();

        let schema = &inject.input_schema;
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
        assert!(required.iter().any(|v| v.as_str() == Some("rationale")));
    }
}
