//! E6 keyword search tool definitions.
//!
//! Per PRD v6 and CLAUDE.md, E6 (V_selectivity) provides:
//! - Exact keyword matches via sparse vector representation
//! - Term-level precision that E1 may dilute through semantic averaging
//!
//! Tools:
//! - search_by_keywords: Find memories matching specific keywords using E6 sparse embeddings

use serde_json::json;

use crate::tools::types::ToolDefinition;

/// Get all keyword tool definitions.
///
/// Returns 1 tool:
/// - search_by_keywords
pub fn definitions() -> Vec<ToolDefinition> {
    vec![search_by_keywords_definition()]
}

/// Definition for search_by_keywords tool.
fn search_by_keywords_definition() -> ToolDefinition {
    ToolDefinition::new(
        "search_by_keywords",
        "Find memories matching specific keywords using E6 sparse embeddings. ENHANCES E1 semantic search with keyword-level precision for exact term matches. Use for \"keyword queries (exact terms, jargon)\" per constitution. Optionally expands terms with E13 SPLADE (fast→quick). Returns nodes matching the query with relevance scores.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The keyword query to search for. Can be a phrase or multiple keywords."
                },
                "topK": {
                    "type": "integer",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum number of results to return (1-50, default: 10)."
                },
                "minScore": {
                    "type": "number",
                    "default": 0.1,
                    "minimum": 0,
                    "maximum": 1,
                    "description": "Minimum blended score threshold (0-1, default: 0.1). Results below this are filtered."
                },
                "blendWithSemantic": {
                    "type": "number",
                    "default": 0.3,
                    "minimum": 0,
                    "maximum": 1,
                    "description": "E6 keyword weight in blend (0-1, default: 0.3). Higher = more keyword emphasis. 0.0=pure E1 semantic, 1.0=pure E6 keyword."
                },
                "useSpladeExpansion": {
                    "type": "boolean",
                    "default": true,
                    "description": "Use E13 SPLADE for term expansion (default: true). Expands query terms to related terms (fast→quick)."
                },
                "includeContent": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include full content text in results (default: false)."
                },
                "strategy": {
                    "type": "string",
                    "enum": ["e1_only", "multi_space", "pipeline"],
                    "default": "multi_space",
                    "description": "Search strategy: 'e1_only' (E1 only), 'multi_space' (default, multi-embedder fusion), 'pipeline' (E13 recall -> E1 -> E12 rerank)."
                }
            },
            "required": ["query"]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_count() {
        assert_eq!(definitions().len(), 1);
    }

    #[test]
    fn test_search_by_keywords_definition() {
        let def = search_by_keywords_definition();
        assert_eq!(def.name, "search_by_keywords");
        assert!(!def.description.is_empty());

        // Verify required parameters
        let required = def.input_schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("query")));

        // Verify properties exist
        let props = def.input_schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("topK"));
        assert!(props.contains_key("minScore"));
        assert!(props.contains_key("blendWithSemantic"));
        assert!(props.contains_key("useSpladeExpansion"));
        assert!(props.contains_key("includeContent"));
    }

    #[test]
    fn test_definition_has_constitution_reference() {
        let def = search_by_keywords_definition();
        assert!(def.description.contains("E6"));
        assert!(def.description.contains("keyword"));
    }
}
