//! E9 robustness search tool definitions.
//!
//! Per Constitution v6.5, E9 (V_robustness/HDC) provides:
//! - Noise-robust structural pattern matching via character trigrams
//! - Typo tolerance that E1 semantic search misses
//! - Character-level similarity for code identifiers and variations
//!
//! E9 uses Hyperdimensional Computing (HDC) with 10,000-bit binary hypervectors
//! projected to 1024D for storage compatibility. Character trigrams preserve
//! similarity despite spelling errors, casing variations, and morphological changes.
//!
//! ## What E9 Finds That E1 Misses
//!
//! - Typos: "authetication" matches "authentication" via character overlap
//! - Casing: `ParseConfig`, `parseConfig`, `parse_config` share structure
//! - Variations: "run", "running", "runner" share "run" trigrams
//!
//! Tools:
//! - search_robust: Find memories matching query despite typos/variations

use serde_json::json;

use crate::tools::types::ToolDefinition;

/// Get all robustness tool definitions.
///
/// Returns 1 tool:
/// - search_robust
pub fn definitions() -> Vec<ToolDefinition> {
    vec![search_robust_definition()]
}

/// Definition for search_robust tool.
fn search_robust_definition() -> ToolDefinition {
    ToolDefinition::new(
        "search_robust",
        "Find memories using E9 noise-robust structural matching. ENHANCES E1 semantic search \
         with typo tolerance via character trigram hypervectors. E9 preserves similarity \
         despite spelling errors, casing variations, and morphological changes. Use for \
         'noisy queries (typos, variations)' per constitution. Example: query 'authetication' \
         finds 'authentication' via character overlap E1 would miss.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Query text (typos are OK - E9 is noise-tolerant). \
                                    Minimum 3 characters for trigram encoding.",
                    "minLength": 3
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
                    "description": "Minimum blended score threshold (0-1, default: 0.1). \
                                    Results below this are filtered."
                },
                "e9DiscoveryThreshold": {
                    "type": "number",
                    "default": 0.15,
                    "minimum": 0,
                    "maximum": 1,
                    "description": "Minimum E9 score for a result to be marked as 'E9 discovery' \
                                    (0-1, default: 0.15). Results with E9 score >= this AND \
                                    E1 score < e1WeaknessThreshold are blind spots E9 found."
                },
                "e1WeaknessThreshold": {
                    "type": "number",
                    "default": 0.5,
                    "minimum": 0,
                    "maximum": 1,
                    "description": "Maximum E1 score for a result to be considered 'missed' by E1 \
                                    (0-1, default: 0.5). If E1 score >= this, E1 would have found it."
                },
                "includeContent": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include full content text in results (default: false)."
                },
                "includeE9Score": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include separate E9 and E1 scores in results (default: true). \
                                    Useful for understanding where E9 helped find matches E1 missed."
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
    fn test_search_robust_definition() {
        let def = search_robust_definition();
        assert_eq!(def.name, "search_robust");
        assert!(!def.description.is_empty());
        assert!(def.description.contains("E9"));
        assert!(def.description.contains("typo"));

        // Verify required parameters
        let required = def.input_schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("query")));

        // Verify properties exist
        let props = def.input_schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("topK"));
        assert!(props.contains_key("minScore"));
        assert!(props.contains_key("e9DiscoveryThreshold"));
        assert!(props.contains_key("e1WeaknessThreshold"));
        assert!(props.contains_key("includeContent"));
        assert!(props.contains_key("includeE9Score"));
    }

    #[test]
    fn test_query_min_length() {
        let def = search_robust_definition();
        let props = def.input_schema.get("properties").unwrap().as_object().unwrap();
        let query_props = &props["query"];

        // E9 requires at least 3 characters for trigram encoding
        assert_eq!(query_props["minLength"], 3);
    }

    #[test]
    fn test_threshold_defaults() {
        let def = search_robust_definition();
        let props = def.input_schema.get("properties").unwrap().as_object().unwrap();

        // E9 discovery threshold
        let e9_threshold = &props["e9DiscoveryThreshold"];
        assert_eq!(e9_threshold["default"], 0.15, "Default E9 discovery threshold should be 0.15");
        assert_eq!(e9_threshold["minimum"], 0.0);
        assert_eq!(e9_threshold["maximum"], 1.0);

        // E1 weakness threshold
        let e1_threshold = &props["e1WeaknessThreshold"];
        assert_eq!(e1_threshold["default"], 0.5, "Default E1 weakness threshold should be 0.5");
        assert_eq!(e1_threshold["minimum"], 0.0);
        assert_eq!(e1_threshold["maximum"], 1.0);
    }

    #[test]
    fn test_definition_mentions_constitution() {
        let def = search_robust_definition();
        assert!(
            def.description.contains("constitution"),
            "Tool description should reference constitution"
        );
    }

    #[test]
    fn test_definition_explains_e9_advantage() {
        let def = search_robust_definition();
        assert!(
            def.description.contains("character") || def.description.contains("trigram"),
            "Tool should explain character-level matching"
        );
        assert!(
            def.description.contains("E1") && def.description.contains("miss"),
            "Tool should explain what E1 misses"
        );
    }
}
