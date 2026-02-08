//! Intent tool definitions (search_by_intent).
//!
//! E10 Query→Document Retrieval: Uses E5-base-v2 for asymmetric retrieval.
//!
//! Note: The former `find_contextual_matches` tool has been merged into `search_by_intent`
//! which now accepts either `query` or `context` parameter.
//!
//! Constitution Compliance:
//! - ARCH-12: E1 is the semantic foundation, E10 enhances
//! - ARCH-15: Uses E5-base-v2's query/passage prefix-based asymmetry
//! - E10 ENHANCES E1 semantic search (not replaces) via blendWithSemantic parameter
//! - Both query and context modes use query→document direction (user input as "query:", memories as "passage:")

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns intent tool definitions (1 tool - search_by_intent handles both query and context).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // search_by_intent - Find memories with similar intent or relevant to context
        ToolDefinition::new(
            "search_by_intent",
            "Find memories that match a query or goal using E10 (E5-base-v2) asymmetric retrieval. \
             Useful for \"what work had the same goal?\" queries. ENHANCES E1 semantic search \
             with intent-based multiplicative boost (ARCH-17). E1 is THE semantic foundation; \
             E10 modifies scores based on intent alignment (>0.5 = boost, <0.5 = reduce). \
             Query encoded as 'query:', memories as 'passage:'. Boost adapts to E1 quality: \
             strong E1 → light boost (refine), weak E1 → strong boost (broaden).",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The intent or goal to search for. Describe what you're trying to accomplish."
                    },
                    "context": {
                        "type": "string",
                        "description": "Alternative to 'query': the context or situation to find relevant memories for. Describe the current situation. Use 'query' for intent-based search, 'context' for situation-based search."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum number of results to return (1-50, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "minScore": {
                        "type": "number",
                        "description": "Minimum similarity score threshold (0-1, default: 0.2). Results below this are filtered.",
                        "default": 0.2,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "blendWithSemantic": {
                        "type": "number",
                        "description": "E10 multiplicative boost strength (ARCH-17). Controls how strongly E10 intent \
                                        alignment modifies E1 scores. Higher = stronger intent influence.",
                        "default": 0.1,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    },
                    "weightProfile": {
                        "type": "string",
                        "description": "Weight profile for retrieval. 'intent_search' (default for query), 'balanced' (default for context), or other profiles like 'code_search', 'causal_reasoning'.",
                        "enum": [
                            "semantic_search", "causal_reasoning", "code_search", "fact_checking",
                            "intent_search", "intent_enhanced", "graph_reasoning",
                            "temporal_navigation", "sequence_navigation", "conversation_history",
                            "category_weighted", "typo_tolerant",
                            "pipeline_stage1_recall", "pipeline_stage2_scoring", "pipeline_full",
                            "balanced"
                        ]
                    },
                    "customWeights": {
                        "type": "object",
                        "description": "Custom weights for 13 embedders (overrides weightProfile). Map of embedder name (E1-E13) to weight value (0.0-1.0). Weights must sum to ~1.0 (±0.01).",
                        "additionalProperties": { "type": "number" }
                    },
                    "excludeEmbedders": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["E1","E2","E3","E4","E5","E6","E7","E8","E9","E10","E11","E12","E13"] },
                        "description": "Embedders to exclude from search. Excluded weights are zeroed and remaining renormalized."
                    }
                },
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_tool_count() {
        // 1 tool: search_by_intent (merged with find_contextual_matches)
        assert_eq!(definitions().len(), 1);
    }

    #[test]
    fn test_search_by_intent_schema() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        // Check required fields
        let required = search
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("query")));

        // Check properties
        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("context")); // New: context as alternative
        assert!(props.contains_key("topK"));
        assert!(props.contains_key("minScore"));
        assert!(props.contains_key("blendWithSemantic"));
        assert!(props.contains_key("includeContent"));
        assert!(props.contains_key("weightProfile")); // New: explicit weight profile
        assert!(props.contains_key("customWeights")); // GAP-2: custom weights support
        assert!(props.contains_key("excludeEmbedders")); // GAP-2: embedder exclusion
    }

    #[test]
    fn test_search_by_intent_defaults() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Verify defaults (blendWithSemantic reduced to 0.1 per E10 optimization)
        assert_eq!(props["topK"]["default"], 10);
        assert_eq!(props["minScore"]["default"], 0.2);
        assert_eq!(props["blendWithSemantic"]["default"], 0.1);
        assert_eq!(props["includeContent"]["default"], false);
    }

    #[test]
    fn test_search_by_intent_has_context_parameter() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Verify context parameter exists
        assert!(props.contains_key("context"));
        let context = &props["context"];
        assert_eq!(context["type"], "string");
    }

    #[test]
    fn test_search_by_intent_has_weight_profile() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Verify weightProfile parameter exists
        assert!(props.contains_key("weightProfile"));
        let profile = &props["weightProfile"];
        assert_eq!(profile["type"], "string");

        // Check it has enum values
        let enum_values = profile["enum"].as_array().unwrap();
        assert!(enum_values.contains(&json!("intent_search")));
        assert!(enum_values.contains(&json!("balanced")));
        assert!(enum_values.contains(&json!("code_search")));
    }

    #[test]
    fn test_tool_description_mentions_e10() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        assert!(
            search.description.contains("E10"),
            "search_by_intent should mention E10"
        );
    }

    #[test]
    fn test_tool_description_mentions_enhances() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        assert!(
            search.description.contains("ENHANCES"),
            "search_by_intent should mention ENHANCES (E10 enhances E1)"
        );
    }

    #[test]
    fn test_blend_with_semantic_bounds() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        let blend = &props["blendWithSemantic"];
        assert_eq!(blend["minimum"], 0);
        assert_eq!(blend["maximum"], 1);
    }

    #[test]
    fn test_query_document_direction_documented() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();

        assert!(
            search.description.contains("E5-base-v2") || search.description.contains("query"),
            "search_by_intent should document E5-base-v2 or query→document pattern"
        );
    }
}
