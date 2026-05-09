//! Graph tool definitions.
//!
//! E8 Upgrade (Phase 4): Leverage asymmetric E8 embeddings for graph reasoning.
//!
//! Constitution Compliance:
//! - ARCH-15: Uses asymmetric E8 with separate source/target encodings
//! - AP-77: Direction modifiers: source→target=1.2, target→source=0.8

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns graph tool definitions.
/// Local LLM graph discovery was retired in the ME-JEPA pivot, so this surface
/// only exposes deterministic graph read/traversal tools.
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
                    },
                    "includeProvenance": {
                        "type": "boolean",
                        "description": "Include retrieval provenance metadata in results (default: false). Shows connection scoring method, direction modifiers, and E8 similarity details.",
                        "default": false
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_exist_with_required_fields() {
        let tools = definitions();
        assert_eq!(tools.len(), 2);
        let search = tools
            .iter()
            .find(|t| t.name == "search_connections")
            .unwrap();
        assert!(search.description.contains("asymmetric") || search.description.contains("E8"));
        assert!(search.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("query")));
        let path = tools.iter().find(|t| t.name == "get_graph_path").unwrap();
        assert!(path.description.contains("attenuation") || path.description.contains("0.9"));
        assert!(path.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("anchorId")));
    }

    #[test]
    fn test_schema_defaults_and_enums() {
        let tools = definitions();
        let search_props = tools
            .iter()
            .find(|t| t.name == "search_connections")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(search_props["direction"]["default"], "both");
        assert_eq!(search_props["topK"]["default"], 10);
        let path_props = tools
            .iter()
            .find(|t| t.name == "get_graph_path")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(path_props["direction"]["default"], "forward");
        assert_eq!(path_props["maxHops"]["default"], 5);
    }
}
