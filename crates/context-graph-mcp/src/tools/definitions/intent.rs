//! Intent tool definitions (search_by_intent, find_contextual_matches, detect_intent_drift, get_session_intent_history).
//!
//! E10 Query→Document Retrieval: Uses E5-base-v2 for asymmetric retrieval.
//!
//! Constitution Compliance:
//! - ARCH-12: E1 is the semantic foundation, E10 enhances
//! - ARCH-15: Uses E5-base-v2's query/passage prefix-based asymmetry
//! - E10 ENHANCES E1 semantic search (not replaces) via blendWithSemantic parameter
//! - Both tools use query→document direction (user input as "query:", memories as "passage:")
//!
//! Phase 5 Enhancement: Intent drift detection across sessions.
//! - detect_intent_drift: Check if current intent has shifted from recent pattern
//! - get_session_intent_history: Get intent trajectory for a session

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns intent tool definitions (4 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // search_by_intent - Find memories with similar intent
        ToolDefinition::new(
            "search_by_intent",
            "Find memories that match a query or goal using E10 (E5-base-v2) asymmetric retrieval. \
             Useful for \"what work had the same goal?\" queries. ENHANCES E1 semantic search \
             with query→document matching via blendWithSemantic parameter. User query encoded \
             as 'query:', memories as 'passage:'. Default blend of 0.1 means 90% E1 + 10% E10.",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The intent or goal to search for. Describe what you're trying to accomplish."
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
                        "description": "Blend weight for E10 vs E1 semantic (0-1, default: 0.1). \
                                        0.0 = pure E1 semantic, 1.0 = pure E10. \
                                        Default 0.1 means 90% E1 + 10% E10.",
                        "default": 0.1,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    }
                },
                "additionalProperties": false
            }),
        ),
        // find_contextual_matches - Find memories relevant to a context
        ToolDefinition::new(
            "find_contextual_matches",
            "Find memories relevant to a given context or situation using E10 (E5-base-v2). \
             Use for \"what's relevant to this situation?\" queries. ENHANCES E1 semantic search \
             with query→document matching (same direction as search_by_intent). \
             Default blend of 0.1 means 90% E1 semantic + 10% E10.",
            json!({
                "type": "object",
                "required": ["context"],
                "properties": {
                    "context": {
                        "type": "string",
                        "description": "The context or situation to find relevant memories for. Describe the current situation."
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
                        "description": "Blend weight for E10 vs E1 semantic (0-1, default: 0.1). \
                                        0.0 = pure E1 semantic, 1.0 = pure E10. \
                                        Default 0.1 means 90% E1 + 10% E10.",
                        "default": 0.1,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    }
                },
                "additionalProperties": false
            }),
        ),
        // detect_intent_drift - Check for intent shift in session (Phase 5)
        ToolDefinition::new(
            "detect_intent_drift",
            "Detect if the current query/intent has shifted significantly from the recent intent pattern. \
             Uses E10 intent embeddings to track intent trajectory across the session. \
             Returns drift score (0-1) and alerts when threshold is exceeded. \
             Useful for detecting topic changes, session summarization, and context relevance scoring.",
            json!({
                "type": "object",
                "properties": {
                    "sessionId": {
                        "type": "string",
                        "description": "Session ID to check for drift. If omitted, uses CLAUDE_SESSION_ID env var."
                    },
                    "currentIntent": {
                        "type": "string",
                        "description": "Current query/intent to compare against recent pattern. If omitted, uses last recorded intent."
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Drift threshold (0-1, default: 0.4). Drift is detected when (1 - similarity) > threshold. Higher = less sensitive.",
                        "default": 0.4,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "windowSize": {
                        "type": "integer",
                        "description": "Number of recent intents to compute centroid from (default: 5).",
                        "default": 5,
                        "minimum": 1,
                        "maximum": 20
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_session_intent_history - Get intent trajectory for session (Phase 5)
        ToolDefinition::new(
            "get_session_intent_history",
            "Get the intent trajectory for a session, including all recorded intents and drift events. \
             Returns intent snapshots with timestamps, categories, and pairwise similarities. \
             Useful for understanding session flow, debugging drift detection, and session summarization.",
            json!({
                "type": "object",
                "properties": {
                    "sessionId": {
                        "type": "string",
                        "description": "Session ID to get history for. If omitted, uses CLAUDE_SESSION_ID env var."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of recent intents to return (1-100, default: 20).",
                        "default": 20,
                        "minimum": 1,
                        "maximum": 100
                    },
                    "includeStats": {
                        "type": "boolean",
                        "description": "Include trajectory statistics (avg similarity, drift rate, etc). Default: true.",
                        "default": true
                    },
                    "includePairwiseSimilarities": {
                        "type": "boolean",
                        "description": "Include similarity between consecutive intents. Default: false.",
                        "default": false
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
        assert_eq!(definitions().len(), 4);
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
        assert!(props.contains_key("topK"));
        assert!(props.contains_key("minScore"));
        assert!(props.contains_key("blendWithSemantic"));
        assert!(props.contains_key("includeContent"));
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
    fn test_find_contextual_matches_schema() {
        let tools = definitions();
        let find = tools
            .iter()
            .find(|t| t.name == "find_contextual_matches")
            .unwrap();

        // Check required fields
        let required = find
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("context")));

        // Check properties
        let props = find
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("context"));
        assert!(props.contains_key("topK"));
        assert!(props.contains_key("minScore"));
        assert!(props.contains_key("blendWithSemantic"));
        assert!(props.contains_key("includeContent"));
    }

    #[test]
    fn test_find_contextual_matches_defaults() {
        let tools = definitions();
        let find = tools
            .iter()
            .find(|t| t.name == "find_contextual_matches")
            .unwrap();

        let props = find
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
    fn test_tool_descriptions_mention_e10() {
        let tools = definitions();

        for tool in &tools {
            // Both tools should reference E10 or intent/context
            assert!(
                tool.description.contains("E10") || tool.description.contains("intent"),
                "Tool {} should mention E10 or intent",
                tool.name
            );
        }
    }

    #[test]
    fn test_search_tools_mention_enhances() {
        let tools = definitions();

        // Only search tools (not drift detection tools) should mention ENHANCES
        let search_tools = ["search_by_intent", "find_contextual_matches"];
        for name in search_tools {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            assert!(
                tool.description.contains("ENHANCES"),
                "Tool {} should mention ENHANCES (E10 enhances E1)",
                name
            );
        }
    }

    #[test]
    fn test_blend_with_semantic_bounds() {
        let tools = definitions();

        // Only search tools have blendWithSemantic parameter
        let search_tools = ["search_by_intent", "find_contextual_matches"];
        for name in search_tools {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            let props = tool
                .input_schema
                .get("properties")
                .unwrap()
                .as_object()
                .unwrap();

            let blend = &props["blendWithSemantic"];
            assert_eq!(blend["minimum"], 0);
            assert_eq!(blend["maximum"], 1);
        }
    }

    #[test]
    fn test_drift_detection_tools_exist() {
        let tools = definitions();

        // Verify drift detection tools exist
        assert!(
            tools.iter().any(|t| t.name == "detect_intent_drift"),
            "detect_intent_drift tool should exist"
        );
        assert!(
            tools.iter().any(|t| t.name == "get_session_intent_history"),
            "get_session_intent_history tool should exist"
        );
    }

    #[test]
    fn test_detect_intent_drift_schema() {
        let tools = definitions();
        let tool = tools.iter().find(|t| t.name == "detect_intent_drift").unwrap();

        let props = tool
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Check properties exist
        assert!(props.contains_key("sessionId"));
        assert!(props.contains_key("currentIntent"));
        assert!(props.contains_key("threshold"));
        assert!(props.contains_key("windowSize"));

        // Check defaults
        assert_eq!(props["threshold"]["default"], 0.4);
        assert_eq!(props["windowSize"]["default"], 5);
    }

    #[test]
    fn test_get_session_intent_history_schema() {
        let tools = definitions();
        let tool = tools.iter().find(|t| t.name == "get_session_intent_history").unwrap();

        let props = tool
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Check properties exist
        assert!(props.contains_key("sessionId"));
        assert!(props.contains_key("limit"));
        assert!(props.contains_key("includeStats"));
        assert!(props.contains_key("includePairwiseSimilarities"));

        // Check defaults
        assert_eq!(props["limit"]["default"], 20);
        assert_eq!(props["includeStats"]["default"], true);
        assert_eq!(props["includePairwiseSimilarities"]["default"], false);
    }

    #[test]
    fn test_query_document_direction_documented() {
        let tools = definitions();

        // search_by_intent should mention query→document or E5-base-v2
        let search = tools.iter().find(|t| t.name == "search_by_intent").unwrap();
        assert!(
            search.description.contains("E5-base-v2") || search.description.contains("query"),
            "search_by_intent should document E5-base-v2 or query→document pattern"
        );

        // find_contextual_matches should mention same direction as search_by_intent
        let find = tools
            .iter()
            .find(|t| t.name == "find_contextual_matches")
            .unwrap();
        assert!(
            find.description.contains("E5-base-v2") || find.description.contains("same direction"),
            "find_contextual_matches should document E5-base-v2 or same direction"
        );
    }
}
