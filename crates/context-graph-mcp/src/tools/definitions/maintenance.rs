//! Maintenance tool definitions for data repair and cleanup.
//!
//! Tools:
//! - repair_causal_relationships: Remove corrupted causal relationship entries

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns maintenance tool definitions (1 tool).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // repair_causal_relationships
        ToolDefinition::new(
            "repair_causal_relationships",
            "Repair corrupted causal relationships by removing entries that fail deserialization. \
             Scans CF_CAUSAL_RELATIONSHIPS and deletes any truncated or corrupted entries. \
             This is useful after crashes or interrupted writes that may have left incomplete data. \
             Returns (deleted_count, total_scanned) statistics.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maintenance_definitions_count() {
        let tools = definitions();
        assert_eq!(tools.len(), 1, "Should have 1 maintenance tool");
    }

    #[test]
    fn test_repair_causal_relationships_definition() {
        let tools = definitions();
        let repair = tools
            .iter()
            .find(|t| t.name == "repair_causal_relationships")
            .unwrap();

        // Should have description mentioning repair
        assert!(repair.description.contains("corrupted"));
        assert!(repair.description.contains("deserialization"));

        // Should be a no-arg tool (empty properties)
        let props = repair.input_schema.get("properties").unwrap();
        assert!(props.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_tool_has_type_object() {
        let tools = definitions();
        for tool in &tools {
            assert_eq!(
                tool.input_schema.get("type").unwrap().as_str().unwrap(),
                "object",
                "Tool {} should have type: object",
                tool.name
            );
        }
    }
}
