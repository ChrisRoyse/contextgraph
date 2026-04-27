//! Merge concepts tool definitions (TASK-MCP-003).
//!
//! Implements merge_concepts tool for consolidating related concept nodes.
//! Constitution: SEC-06 (30-day reversal), PRD Section 5.3
//!
//! ## Merge Strategies
//! - union: Combine all attributes from source nodes
//! - intersection: Keep only common attributes
//! - weighted_average: Weight by node importance/confidence

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns merge tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition::new(
        "merge_concepts",
        "Merge two or more related concept nodes into a unified node. \
             Supports union (combine all), intersection (common only), or \
             weighted_average (by importance) strategies. Returns reversal_hash \
             for 30-day undo capability. Requires rationale per PRD 0.3.",
        json!({
            "type": "object",
            "required": ["source_ids", "target_name", "rationale"],
            "properties": {
                "source_ids": {
                    "type": "array",
                    "items": { "type": "string", "format": "uuid" },
                    "minItems": 2,
                    "maxItems": 10,
                    "description": "UUIDs of concepts to merge (2-10 required)"
                },
                "target_name": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 256,
                    "description": "Name for the merged concept (1-256 chars)"
                },
                "merge_strategy": {
                    "type": "string",
                    "enum": ["union", "intersection", "weighted_average"],
                    "default": "union",
                    "description": "Strategy: union=combine all, intersection=common only, weighted_average=by importance"
                },
                "rationale": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 1024,
                    "description": "Rationale for merge (REQUIRED per PRD 0.3)"
                },
                "force_merge": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force merge even if priors conflict (use with caution)"
                }
            },
            "additionalProperties": false
        }),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition_exists_with_required_fields() {
        let tools = definitions();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "merge_concepts");
        assert!(tools[0].description.contains("reversal"));
        assert!(tools[0].description.contains("30-day"));
        let required: Vec<&str> = tools[0].input_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(required.contains(&"source_ids"));
        assert!(required.contains(&"target_name"));
        assert!(required.contains(&"rationale"));
        let props = tools[0].input_schema.get("properties").unwrap();
        assert_eq!(props["source_ids"]["minItems"], 2);
        assert_eq!(props["source_ids"]["maxItems"], 10);
        assert_eq!(props["merge_strategy"]["default"], "union");
    }

    #[test]
    fn test_synthetic_valid_input() {
        let input = json!({
            "source_ids": ["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"],
            "target_name": "Merged Auth Concept",
            "merge_strategy": "weighted_average",
            "rationale": "Consolidating duplicate auth patterns",
            "force_merge": false
        });
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let strategies: Vec<&str> = props["merge_strategy"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(strategies.contains(&input["merge_strategy"].as_str().unwrap()));
        assert_eq!(strategies.len(), 3);
    }
}
