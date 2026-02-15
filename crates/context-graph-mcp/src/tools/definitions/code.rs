//! E7 code search tool definitions.
//!
//! Per PRD v6 and CLAUDE.md, E7 (V_correctness) provides:
//! - Code patterns and function signatures via 1536D dense embeddings
//! - Code-specific understanding that E1 misses by treating code as natural language
//!
//! Tools:
//! - search_code: Find memories containing code patterns using E7 dense embeddings

use serde_json::json;

use crate::tools::types::ToolDefinition;

/// Get all code tool definitions.
///
/// Returns 1 tool:
/// - search_code
pub fn definitions() -> Vec<ToolDefinition> {
    vec![search_code_definition()]
}

/// Definition for search_code tool.
fn search_code_definition() -> ToolDefinition {
    ToolDefinition::new(
        "search_code",
        "Find memories containing code patterns using E7 dense embeddings (1536D). ENHANCES E1 semantic search with code-specific understanding. Use for \"code queries (implementations, functions)\" per constitution. Detects programming language from query. Returns nodes matching the query with relevance scores and detected language info.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The code query to search for. Can describe functionality, patterns, or specific code constructs."
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
                    "default": 0.2,
                    "minimum": 0,
                    "maximum": 1,
                    "description": "Minimum blended score threshold (0-1, default: 0.2). Results below this are filtered."
                },
                "blendWithSemantic": {
                    "type": "number",
                    "default": 0.4,
                    "minimum": 0,
                    "maximum": 1,
                    "description": "E7 code weight in blend (0-1, default: 0.4). Higher = more code-specific emphasis. 0.0=pure E1 semantic, 1.0=pure E7 code."
                },
                "includeContent": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include full content text in results (default: false)."
                },
                "searchMode": {
                    "type": "string",
                    "enum": ["e7Only", "pipeline", "semantic"],
                    "default": "semantic",
                    "description": "Code search strategy: 'semantic' (default, hybrid E1+E7 blend), 'e7Only' (pure E7 code search), 'pipeline' (full E13->E1->E7->E12 pipeline)."
                },
                "languageHint": {
                    "type": "string",
                    "description": "Optional programming language hint to boost language-specific results. Supports: rust, python, javascript, typescript, go, java, cpp, sql."
                },
                "includeAstContext": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include AST context (scope chain, entity type) in results if available (default: false). Only affects chunks created by AST chunker."
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
    fn test_search_code_definition() {
        let def = search_code_definition();
        assert_eq!(def.name, "search_code");
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
        assert!(props.contains_key("includeContent"));
    }

    #[test]
    fn test_definition_has_constitution_reference() {
        let def = search_code_definition();
        assert!(def.description.contains("E7"));
        assert!(def.description.contains("code"));
    }
}
