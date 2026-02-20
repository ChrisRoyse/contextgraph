//! Provenance query tool definitions (Phase P3).
//!
//! Tools:
//! - get_audit_trail: Query audit log by target or time range
//! - get_merge_history: Show merge lineage for a fingerprint
//! - get_provenance_chain: Full provenance chain from embedding to source

use crate::tools::types::ToolDefinition;
use serde_json::json;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "get_audit_trail",
            "Query the audit log for a specific memory or time range. Returns chronological audit records showing all operations performed on the memory (create, merge, delete, boost, etc.).",
            json!({
                "type": "object",
                "properties": {
                    "target_id": {
                        "type": "string",
                        "description": "UUID of the memory to get audit trail for"
                    },
                    "start_time": {
                        "type": "string",
                        "description": "ISO 8601 timestamp for time range start (e.g., '2024-01-01T00:00:00Z')"
                    },
                    "end_time": {
                        "type": "string",
                        "description": "ISO 8601 timestamp for time range end"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum records to return (default: 50, max: 500)",
                        "default": 50
                    }
                },
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_merge_history",
            "Show merge lineage and history for a fingerprint. Returns all merge operations that created or affected this memory, including source memory IDs, strategy used, and operator.",
            json!({
                "type": "object",
                "properties": {
                    "memory_id": {
                        "type": "string",
                        "description": "UUID of the memory to get merge history for"
                    },
                    "include_source_metadata": {
                        "type": "boolean",
                        "description": "Include source metadata for merged sources (default: false)",
                        "default": false
                    }
                },
                "required": ["memory_id"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_provenance_chain",
            "Full provenance chain from embedding to source for a memory. Shows source type, file path, chunk info, operator attribution, causal direction, and creation timestamps.",
            json!({
                "type": "object",
                "properties": {
                    "memory_id": {
                        "type": "string",
                        "description": "UUID of the memory to trace provenance for"
                    },
                    "include_audit": {
                        "type": "boolean",
                        "description": "Include audit trail in provenance chain (default: false)",
                        "default": false
                    },
                    "include_embedding_version": {
                        "type": "boolean",
                        "description": "Include embedding version info if available (default: false)",
                        "default": false
                    },
                    "include_importance_history": {
                        "type": "boolean",
                        "description": "Include importance change history (default: false)",
                        "default": false
                    },
                    "include_merge_history": {
                        "type": "boolean",
                        "description": "Include merge lineage history (default: false)",
                        "default": false
                    },
                    "depth_full": {
                        "type": "boolean",
                        "description": "Include all provenance data (audit, embedding, importance, merge history) (default: false)",
                        "default": false
                    }
                },
                "required": ["memory_id"],
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provenance_definitions_count() {
        let tools = definitions();
        assert_eq!(tools.len(), 3, "Should have 3 provenance tools");
    }

    #[test]
    fn test_all_tools_have_type_object() {
        for tool in definitions() {
            assert_eq!(
                tool.input_schema.get("type").unwrap().as_str().unwrap(),
                "object",
                "Tool {} should have type: object",
                tool.name
            );
        }
    }
}
