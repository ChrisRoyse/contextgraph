//! MCP tool definitions following the MCP 2024-11-05 protocol specification.
//!
//! This module defines the tools available through the MCP server's `tools/list`
//! and `tools/call` endpoints.

use serde::{Deserialize, Serialize};
use serde_json::json;

/// MCP tool definition following the protocol specification.
///
/// Each tool has a name, description, and JSON Schema for input validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name
    pub name: String,

    /// Human-readable description of what the tool does
    pub description: String,

    /// JSON Schema defining the tool's input parameters
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Get all tool definitions for the `tools/list` response.
///
/// Returns the complete list of MCP tools exposed by the Context Graph server.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        // inject_context - primary context injection tool
        ToolDefinition::new(
            "inject_context",
            "Inject context into the knowledge graph with UTL processing. \
             Analyzes content for learning potential and stores with computed metrics.",
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to inject into the knowledge graph"
                    },
                    "rationale": {
                        "type": "string",
                        "description": "Why this context is relevant and should be stored"
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "code", "image", "audio", "structured", "mixed"],
                        "default": "text",
                        "description": "The type/modality of the content"
                    },
                    "importance": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.5,
                        "description": "Importance score for the memory [0.0, 1.0]"
                    }
                },
                "required": ["content", "rationale"]
            }),
        ),

        // store_memory - store a memory node directly
        ToolDefinition::new(
            "store_memory",
            "Store a memory node directly in the knowledge graph without UTL processing.",
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    },
                    "importance": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.5,
                        "description": "Importance score for the memory [0.0, 1.0]"
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "code", "image", "audio", "structured", "mixed"],
                        "default": "text",
                        "description": "The type/modality of the content"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional tags for categorization"
                    }
                },
                "required": ["content"]
            }),
        ),

        // get_memetic_status - get UTL metrics and system state
        ToolDefinition::new(
            "get_memetic_status",
            "Get current UTL metrics, entropy, coherence, and overall system cognitive state.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),

        // get_graph_manifest - describe the 5-layer architecture
        ToolDefinition::new(
            "get_graph_manifest",
            "Get the 5-layer bio-nervous system architecture description and current layer statuses.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),

        // search_graph - semantic search
        ToolDefinition::new(
            "search_graph",
            "Search the knowledge graph using semantic similarity. \
             Returns nodes matching the query with relevance scores.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query text"
                    },
                    "topK": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 10,
                        "description": "Maximum number of results to return"
                    },
                    "minSimilarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.0,
                        "description": "Minimum similarity threshold [0.0, 1.0]"
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "code", "image", "audio", "structured", "mixed"],
                        "description": "Filter results by modality"
                    }
                },
                "required": ["query"]
            }),
        ),
        // utl_status - query UTL system state
        ToolDefinition::new(
            "utl_status",
            "Query current UTL (Unified Theory of Learning) system state including lifecycle phase, \
             entropy, coherence, learning score, Johari quadrant, and consolidation phase.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
    ]
}

/// Tool names as constants for dispatch matching.
pub mod tool_names {
    pub const INJECT_CONTEXT: &str = "inject_context";
    pub const STORE_MEMORY: &str = "store_memory";
    pub const GET_MEMETIC_STATUS: &str = "get_memetic_status";
    pub const GET_GRAPH_MANIFEST: &str = "get_graph_manifest";
    pub const SEARCH_GRAPH: &str = "search_graph";
    pub const UTL_STATUS: &str = "utl_status";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 6);

        let tool_names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"inject_context"));
        assert!(tool_names.contains(&"store_memory"));
        assert!(tool_names.contains(&"get_memetic_status"));
        assert!(tool_names.contains(&"get_graph_manifest"));
        assert!(tool_names.contains(&"search_graph"));
        assert!(tool_names.contains(&"utl_status"));
    }

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
