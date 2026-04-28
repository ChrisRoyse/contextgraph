//! Capability discovery tool definitions.

use serde_json::json;

use crate::tools::types::ToolDefinition;

/// Return capability discovery tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "get_capability_matrix",
            "Return the MCP capability matrix: all E1-E14 content embedders, E15-E21 learner-state slots, tool groups, source-of-truth RocksDB CFs, and optional runtime counts. Use this before broad agent work so clients can choose the smallest correct tool.",
            json!({
                "type": "object",
                "properties": {
                    "includeRuntimeState": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true, read live source-of-truth counts from RocksDB and daemon state. Fails closed if runtime state cannot be read."
                    },
                    "includeToolSchemas": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, include full MCP input schemas for every exposed tool. Default false keeps the response compact."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_output_schema(json!({
            "type": "object",
            "required": ["version", "mcp", "embedders", "capabilities"],
            "properties": {
                "version": {"type": "integer"},
                "mcp": {"type": "object"},
                "embedders": {"type": "object"},
                "capabilities": {"type": "array"},
                "runtime": {"type": "object"}
            }
        })),
    ]
}
