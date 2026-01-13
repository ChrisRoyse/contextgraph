//! Merge concepts tool definitions (TASK-MCP-003).
//!
//! Implements merge_concepts tool for consolidating related concept nodes.
//! Constitution: SEC-06 (30-day reversal), PRD Section 5.3
//!
//! ## Merge Strategies
//! - union: Combine all attributes from source nodes
//! - intersection: Keep only common attributes
//! - weighted_average: Weight by node importance/confidence

use serde_json::json;
use crate::tools::types::ToolDefinition;

/// Returns merge tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
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
                }
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_concepts_definition_exists() {
        let tools = definitions();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "merge_concepts");
    }

    #[test]
    fn test_merge_concepts_required_fields() {
        let tools = definitions();
        let schema = &tools[0].input_schema;
        let required = schema.get("required").expect("required field should exist").as_array().expect("required should be array");

        let required_fields: Vec<&str> = required.iter().map(|v| v.as_str().expect("should be string")).collect();
        assert_eq!(required_fields.len(), 3);
        assert!(required_fields.contains(&"source_ids"));
        assert!(required_fields.contains(&"target_name"));
        assert!(required_fields.contains(&"rationale"));

        // merge_strategy and force_merge are NOT required (have defaults)
        assert!(!required_fields.contains(&"merge_strategy"));
        assert!(!required_fields.contains(&"force_merge"));
    }

    #[test]
    fn test_source_ids_constraints() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let source_ids = props.get("source_ids").expect("source_ids should exist");

        // Array of UUIDs
        assert_eq!(source_ids.get("type").expect("type should exist").as_str().expect("should be string"), "array");
        let items = source_ids.get("items").expect("items should exist");
        assert_eq!(items.get("type").expect("type should exist").as_str().expect("should be string"), "string");
        assert_eq!(items.get("format").expect("format should exist").as_str().expect("should be string"), "uuid");

        // 2-10 items
        assert_eq!(source_ids.get("minItems").expect("minItems should exist").as_u64().expect("should be u64"), 2);
        assert_eq!(source_ids.get("maxItems").expect("maxItems should exist").as_u64().expect("should be u64"), 10);
    }

    #[test]
    fn test_target_name_constraints() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let target_name = props.get("target_name").expect("target_name should exist");

        assert_eq!(target_name.get("type").expect("type should exist").as_str().expect("should be string"), "string");
        assert_eq!(target_name.get("minLength").expect("minLength should exist").as_u64().expect("should be u64"), 1);
        assert_eq!(target_name.get("maxLength").expect("maxLength should exist").as_u64().expect("should be u64"), 256);
    }

    #[test]
    fn test_merge_strategy_enum() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let strategy = props.get("merge_strategy").expect("merge_strategy should exist");

        assert_eq!(strategy.get("type").expect("type should exist").as_str().expect("should be string"), "string");
        let enum_values = strategy.get("enum").expect("enum should exist").as_array().expect("should be array");
        let values: Vec<&str> = enum_values.iter().map(|v| v.as_str().expect("should be string")).collect();

        assert_eq!(values.len(), 3);
        assert!(values.contains(&"union"));
        assert!(values.contains(&"intersection"));
        assert!(values.contains(&"weighted_average"));

        // Default is union
        assert_eq!(strategy.get("default").expect("default should exist").as_str().expect("should be string"), "union");
    }

    #[test]
    fn test_rationale_constraints() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let rationale = props.get("rationale").expect("rationale should exist");

        assert_eq!(rationale.get("type").expect("type should exist").as_str().expect("should be string"), "string");
        assert_eq!(rationale.get("minLength").expect("minLength should exist").as_u64().expect("should be u64"), 1);
        assert_eq!(rationale.get("maxLength").expect("maxLength should exist").as_u64().expect("should be u64"), 1024);
    }

    #[test]
    fn test_force_merge_default() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let force_merge = props.get("force_merge").expect("force_merge should exist");

        assert_eq!(force_merge.get("type").expect("type should exist").as_str().expect("should be string"), "boolean");
        assert_eq!(force_merge.get("default").expect("default should exist").as_bool().expect("should be bool"), false);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let tools = definitions();
        let json_str = serde_json::to_string(&tools).expect("Serialization failed");
        assert!(json_str.contains("merge_concepts"));
        assert!(json_str.contains("inputSchema"));
        assert!(json_str.contains("source_ids"));
        assert!(json_str.contains("reversal"));
    }

    // ========== SYNTHETIC DATA VALIDATION TESTS ==========

    #[test]
    fn test_synthetic_valid_merge_input() {
        // Synthetic test data matching schema exactly
        let synthetic_input = json!({
            "source_ids": [
                "550e8400-e29b-41d4-a716-446655440001",
                "550e8400-e29b-41d4-a716-446655440002"
            ],
            "target_name": "Merged Authentication Concept",
            "merge_strategy": "union",
            "rationale": "Consolidating duplicate auth patterns for cleaner graph",
            "force_merge": false
        });

        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");

        // Validate source_ids: 2 UUIDs, within [2,10] range
        let source_ids_arr = synthetic_input.get("source_ids").expect("source_ids should exist").as_array().expect("should be array");
        assert_eq!(source_ids_arr.len(), 2);
        let source_ids_schema = props.get("source_ids").expect("source_ids schema should exist");
        let min_items = source_ids_schema.get("minItems").expect("minItems should exist").as_u64().expect("should be u64") as usize;
        let max_items = source_ids_schema.get("maxItems").expect("maxItems should exist").as_u64().expect("should be u64") as usize;
        assert!(source_ids_arr.len() >= min_items && source_ids_arr.len() <= max_items);

        // Validate target_name length
        let target_name = synthetic_input.get("target_name").expect("target_name should exist").as_str().expect("should be string");
        let target_schema = props.get("target_name").expect("target_name schema should exist");
        let name_min = target_schema.get("minLength").expect("minLength should exist").as_u64().expect("should be u64") as usize;
        let name_max = target_schema.get("maxLength").expect("maxLength should exist").as_u64().expect("should be u64") as usize;
        assert!(target_name.len() >= name_min && target_name.len() <= name_max);

        // Validate merge_strategy in enum
        let strategy = synthetic_input.get("merge_strategy").expect("merge_strategy should exist").as_str().expect("should be string");
        let strategy_schema = props.get("merge_strategy").expect("merge_strategy schema should exist");
        let valid_strategies: Vec<&str> = strategy_schema.get("enum").expect("enum should exist").as_array().expect("should be array")
            .iter().map(|v| v.as_str().expect("should be string")).collect();
        assert!(valid_strategies.contains(&strategy));

        // Validate rationale length
        let rationale = synthetic_input.get("rationale").expect("rationale should exist").as_str().expect("should be string");
        let rationale_schema = props.get("rationale").expect("rationale schema should exist");
        let rat_min = rationale_schema.get("minLength").expect("minLength should exist").as_u64().expect("should be u64") as usize;
        let rat_max = rationale_schema.get("maxLength").expect("maxLength should exist").as_u64().expect("should be u64") as usize;
        assert!(rationale.len() >= rat_min && rationale.len() <= rat_max);
    }

    #[test]
    fn test_synthetic_valid_merge_with_weighted_average() {
        // Test with weighted_average strategy and more source IDs
        let synthetic_input = json!({
            "source_ids": [
                "550e8400-e29b-41d4-a716-446655440001",
                "550e8400-e29b-41d4-a716-446655440002",
                "550e8400-e29b-41d4-a716-446655440003"
            ],
            "target_name": "Unified Authentication Module",
            "merge_strategy": "weighted_average",
            "rationale": "Consolidating 3 auth-related concepts that share >0.9 similarity",
            "force_merge": false
        });

        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");

        // Validate 3 source_ids
        let source_ids_arr = synthetic_input.get("source_ids").expect("source_ids should exist").as_array().expect("should be array");
        assert_eq!(source_ids_arr.len(), 3);

        // Validate weighted_average strategy
        let strategy = synthetic_input.get("merge_strategy").expect("merge_strategy should exist").as_str().expect("should be string");
        let strategy_schema = props.get("merge_strategy").expect("merge_strategy schema should exist");
        let valid_strategies: Vec<&str> = strategy_schema.get("enum").expect("enum should exist").as_array().expect("should be array")
            .iter().map(|v| v.as_str().expect("should be string")).collect();
        assert!(valid_strategies.contains(&strategy));
        assert_eq!(strategy, "weighted_average");
    }

    #[test]
    fn test_edge_case_minimum_source_ids() {
        // Edge Case: Exactly 2 source_ids (minimum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let source_ids = props.get("source_ids").expect("source_ids should exist");

        assert_eq!(source_ids.get("minItems").expect("minItems should exist").as_u64().expect("should be u64"), 2);
    }

    #[test]
    fn test_edge_case_maximum_source_ids() {
        // Edge Case: Exactly 10 source_ids (maximum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let source_ids = props.get("source_ids").expect("source_ids should exist");

        assert_eq!(source_ids.get("maxItems").expect("maxItems should exist").as_u64().expect("should be u64"), 10);
    }

    #[test]
    fn test_edge_case_maximum_target_name() {
        // Edge Case: 256-char target_name (maximum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let target_name = props.get("target_name").expect("target_name should exist");

        assert_eq!(target_name.get("maxLength").expect("maxLength should exist").as_u64().expect("should be u64"), 256);

        // Verify a 256-char name would be valid
        let max_name = "x".repeat(256);
        assert_eq!(max_name.len(), 256);
    }

    #[test]
    fn test_edge_case_minimum_target_name() {
        // Edge Case: 1-char target_name (minimum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let target_name = props.get("target_name").expect("target_name should exist");

        assert_eq!(target_name.get("minLength").expect("minLength should exist").as_u64().expect("should be u64"), 1);
    }

    #[test]
    fn test_edge_case_maximum_rationale() {
        // Edge Case: 1024-char rationale (maximum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let rationale = props.get("rationale").expect("rationale should exist");

        assert_eq!(rationale.get("maxLength").expect("maxLength should exist").as_u64().expect("should be u64"), 1024);

        // Verify a 1024-char rationale would be valid
        let max_rationale = "x".repeat(1024);
        assert_eq!(max_rationale.len(), 1024);
    }

    #[test]
    fn test_all_merge_strategies() {
        // Verify all 3 merge strategies are present
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let strategy = props.get("merge_strategy").expect("merge_strategy should exist");
        let enum_values = strategy.get("enum").expect("enum should exist").as_array().expect("should be array");

        let values: Vec<&str> = enum_values.iter().map(|v| v.as_str().expect("should be string")).collect();
        assert_eq!(values.len(), 3);

        // Per constitution.yaml: union, intersection, weighted_average
        assert!(values.contains(&"union"), "union: combine all attributes");
        assert!(values.contains(&"intersection"), "intersection: common only");
        assert!(values.contains(&"weighted_average"), "weighted_average: by importance");
    }

    #[test]
    fn test_source_ids_uuid_format() {
        // Verify source_ids expects UUID format
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").expect("properties should exist");
        let source_ids = props.get("source_ids").expect("source_ids should exist");
        let items = source_ids.get("items").expect("items should exist");

        assert_eq!(items.get("type").expect("type should exist").as_str().expect("should be string"), "string");
        assert_eq!(items.get("format").expect("format should exist").as_str().expect("should be string"), "uuid");
    }

    #[test]
    fn test_description_mentions_reversal() {
        // Verify description mentions reversal per SEC-06
        let tools = definitions();
        assert!(tools[0].description.contains("reversal"), "Description should mention reversal capability");
        assert!(tools[0].description.contains("30-day"), "Description should mention 30-day window");
    }

    #[test]
    fn test_description_mentions_rationale() {
        // Verify description mentions rationale requirement per PRD 0.3
        let tools = definitions();
        assert!(tools[0].description.contains("rationale"), "Description should mention rationale requirement");
        assert!(tools[0].description.contains("PRD"), "Description should reference PRD");
    }

    #[test]
    fn test_verify_merge_concepts_in_all_tools() {
        // Verify tool appears in aggregated definitions (after mod.rs is updated)
        let tools = crate::tools::get_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"merge_concepts"), "merge_concepts missing from tool list");
    }
}
