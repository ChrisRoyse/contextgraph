//! Graph tool definitions (search_connections, get_graph_path, discover_graph_relationships, validate_graph_link).
//!
//! E8 Upgrade (Phase 4): Leverage asymmetric E8 embeddings for graph reasoning.
//!
//! Graph Discovery (LLM-based): Uses context-graph-graph-agent for relationship detection.
//!
//! Constitution Compliance:
//! - ARCH-15: Uses asymmetric E8 with separate source/target encodings
//! - AP-77: Direction modifiers: source→target=1.2, target→source=0.8

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns graph tool definitions (4 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // search_connections - Find connected memories
        ToolDefinition::new(
            "search_connections",
            "Find memories connected to a given concept using asymmetric E8 similarity. \
             Searches for source connections (what points TO this), target connections \
             (what this points TO), or both. Uses 1.2x/0.8x direction modifiers per AP-77. \
             Use for \"what imports X?\", \"what does X use?\", \"what connects to X?\" queries.",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The concept to find connections for. Can be a concept name or structural query."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["source", "target", "both"],
                        "description": "Connection direction: source (what points TO this), target (what this points TO), both. Default: both.",
                        "default": "both"
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum number of connections to return (1-50, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "minScore": {
                        "type": "number",
                        "description": "Minimum connection score threshold (0-1, default: 0.1). Results below this are filtered.",
                        "default": 0.1,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    },
                    "filterGraphDirection": {
                        "type": "string",
                        "enum": ["source", "target", "unknown"],
                        "description": "Filter results by persisted graph direction. Omit for no filtering."
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_graph_path - Multi-hop graph traversal
        ToolDefinition::new(
            "get_graph_path",
            "Build and visualize multi-hop graph paths from an anchor point. \
             Iteratively searches for connected memories using asymmetric E8 similarity. \
             Applies hop attenuation (0.9^hop) for path scoring. \
             Use for dependency chain visualization, connectivity exploration.",
            json!({
                "type": "object",
                "required": ["anchorId"],
                "properties": {
                    "anchorId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the starting memory (anchor point)."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["forward", "backward"],
                        "description": "Direction to traverse: forward (source→target) or backward (target→source). Default: forward.",
                        "default": "forward"
                    },
                    "maxHops": {
                        "type": "integer",
                        "description": "Maximum number of hops to traverse (1-10, default: 5).",
                        "default": 5,
                        "minimum": 1,
                        "maximum": 10
                    },
                    "minSimilarity": {
                        "type": "number",
                        "description": "Minimum similarity threshold for each hop (0-1, default: 0.3).",
                        "default": 0.3,
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
        // discover_graph_relationships - LLM-based relationship discovery
        ToolDefinition::new(
            "discover_graph_relationships",
            "Discover graph relationships between memories using LLM analysis. \
             Uses the graph-agent with shared CausalDiscoveryLLM (Qwen2.5-3B) for relationship detection. \
             Supports 20 relationship types across 4 domains: Code (imports, calls, implements), \
             Legal (cites, overrules, interprets), Academic (cites, applies, extends), General. \
             Returns discovered relationships with confidence scores, categories, and directions.",
            json!({
                "type": "object",
                "required": ["memory_ids"],
                "properties": {
                    "memory_ids": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "format": "uuid"
                        },
                        "description": "UUIDs of memories to analyze for relationships (2-50).",
                        "minItems": 2,
                        "maxItems": 50
                    },
                    "relationship_types": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": [
                                "contains", "scoped_by",
                                "depends_on", "imports", "requires",
                                "references", "cites", "interprets", "distinguishes",
                                "implements", "complies_with", "fulfills",
                                "extends", "modifies", "supersedes", "overrules",
                                "calls", "applies", "used_by"
                            ]
                        },
                        "description": "Filter to specific relationship types. Omit to discover all types."
                    },
                    "relationship_categories": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["containment", "dependency", "reference", "implementation", "extension", "invocation"]
                        },
                        "description": "Filter by relationship categories. Omit for all categories."
                    },
                    "content_domain": {
                        "type": "string",
                        "enum": ["code", "legal", "academic", "general"],
                        "description": "Hint for content domain. Auto-detected if not specified."
                    },
                    "min_confidence": {
                        "type": "number",
                        "description": "Minimum confidence threshold for discovered relationships (0-1, default: 0.7).",
                        "default": 0.7,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "batch_size": {
                        "type": "integer",
                        "description": "Maximum number of candidate pairs to analyze (1-100, default: 50).",
                        "default": 50,
                        "minimum": 1,
                        "maximum": 100
                    }
                },
                "additionalProperties": false
            }),
        ),
        // validate_graph_link - Single-pair LLM validation
        ToolDefinition::new(
            "validate_graph_link",
            "Validate a proposed graph link between two memories using LLM analysis. \
             Uses the graph-agent with shared CausalDiscoveryLLM (Qwen2.5-3B) for validation. \
             Supports 20 relationship types across Code, Legal, Academic, and General domains. \
             Returns validation result with confidence score, detected relationship type, category, and direction.",
            json!({
                "type": "object",
                "required": ["source_id", "target_id"],
                "properties": {
                    "source_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the source memory (the one that 'points to')."
                    },
                    "target_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the target memory (the one that 'is pointed to')."
                    },
                    "expected_relationship_type": {
                        "type": "string",
                        "enum": [
                            "contains", "scoped_by",
                            "depends_on", "imports", "requires",
                            "references", "cites", "interprets", "distinguishes",
                            "implements", "complies_with", "fulfills",
                            "extends", "modifies", "supersedes", "overrules",
                            "calls", "applies", "used_by"
                        ],
                        "description": "Expected relationship type to validate. Omit to detect any relationship."
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
    fn test_graph_tool_count() {
        assert_eq!(definitions().len(), 4);
    }

    #[test]
    fn test_search_connections_schema() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_connections").unwrap();

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
        assert!(props.contains_key("direction"));
        assert!(props.contains_key("topK"));
        assert!(props.contains_key("minScore"));
        assert!(props.contains_key("includeContent"));
        assert!(props.contains_key("filterGraphDirection"));
    }

    #[test]
    fn test_search_connections_defaults() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_connections").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Verify defaults
        assert_eq!(props["direction"]["default"], "both");
        assert_eq!(props["topK"]["default"], 10);
        assert_eq!(props["minScore"]["default"], 0.1);
        assert_eq!(props["includeContent"]["default"], false);
    }

    #[test]
    fn test_get_graph_path_schema() {
        let tools = definitions();
        let path = tools.iter().find(|t| t.name == "get_graph_path").unwrap();

        // Check required fields
        let required = path
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("anchorId")));

        // Check properties
        let props = path
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("anchorId"));
        assert!(props.contains_key("direction"));
        assert!(props.contains_key("maxHops"));
        assert!(props.contains_key("minSimilarity"));
        assert!(props.contains_key("includeContent"));
    }

    #[test]
    fn test_get_graph_path_defaults() {
        let tools = definitions();
        let path = tools.iter().find(|t| t.name == "get_graph_path").unwrap();

        let props = path
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        // Verify defaults
        assert_eq!(props["direction"]["default"], "forward");
        assert_eq!(props["maxHops"]["default"], 5);
        assert_eq!(props["minSimilarity"]["default"], 0.3);
        assert_eq!(props["includeContent"]["default"], false);
    }

    #[test]
    fn test_direction_enum_values_search() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_connections").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        let direction_enum = props["direction"]["enum"].as_array().unwrap();
        assert!(direction_enum.contains(&json!("source")));
        assert!(direction_enum.contains(&json!("target")));
        assert!(direction_enum.contains(&json!("both")));
    }

    #[test]
    fn test_direction_enum_values_path() {
        let tools = definitions();
        let path = tools.iter().find(|t| t.name == "get_graph_path").unwrap();

        let props = path
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        let direction_enum = props["direction"]["enum"].as_array().unwrap();
        assert!(direction_enum.contains(&json!("forward")));
        assert!(direction_enum.contains(&json!("backward")));
    }

    #[test]
    fn test_filter_graph_direction_enum() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_connections").unwrap();

        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();

        let filter_enum = props["filterGraphDirection"]["enum"].as_array().unwrap();
        assert!(filter_enum.contains(&json!("source")));
        assert!(filter_enum.contains(&json!("target")));
        assert!(filter_enum.contains(&json!("unknown")));
    }

    #[test]
    fn test_tool_descriptions_mention_e8_or_asymmetric() {
        let tools = definitions();

        for tool in &tools {
            // Both tools should reference E8 or asymmetric similarity
            assert!(
                tool.description.contains("asymmetric") || tool.description.contains("E8"),
                "Tool {} should mention asymmetric E8",
                tool.name
            );
        }
    }

    #[test]
    fn test_search_connections_mentions_direction_modifiers() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_connections").unwrap();

        // Should mention direction modifiers or AP-77
        assert!(
            search.description.contains("1.2") || search.description.contains("AP-77"),
            "search_connections should mention direction modifiers"
        );
    }

    #[test]
    fn test_get_graph_path_mentions_attenuation() {
        let tools = definitions();
        let path = tools.iter().find(|t| t.name == "get_graph_path").unwrap();

        // Should mention hop attenuation
        assert!(
            path.description.contains("attenuation") || path.description.contains("0.9"),
            "get_graph_path should mention hop attenuation"
        );
    }
}
