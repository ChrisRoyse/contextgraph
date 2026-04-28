//! Tool type definitions for MCP protocol.

use serde::{Deserialize, Serialize};

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

    /// Optional JSON Schema defining structured tool output.
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
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
            output_schema: None,
        }
    }

    /// Attach an MCP `outputSchema` for clients that validate structured output.
    pub fn with_output_schema(mut self, output_schema: serde_json::Value) -> Self {
        self.output_schema = Some(output_schema);
        self
    }
}
