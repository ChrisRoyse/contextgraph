//! Embedder-first search tool definitions.
//!
//! Per Constitution v6.3, these tools enable AI agents to search using any of the
//! 13 embedders as the primary perspective. Each embedder sees the knowledge graph
//! differently - E11 finds what E1 misses, E7 reveals code patterns, etc.
//!
//! ## Philosophy
//!
//! The 13 embedders are 13 lenses on the same knowledge:
//! - E1 (semantic): Dense semantic similarity - foundation
//! - E5 (causal): Cause-effect relationships
//! - E6 (keyword): Exact keyword matches
//! - E7 (code): Code patterns, function signatures
//! - E8 (graph): Structural relationships (imports, deps)
//! - E10 (intent): Goal alignment, similar purpose
//! - E11 (entity): Entity knowledge via KEPLER
//! - E12 (precision): Exact phrase matches (reranking)
//! - E13 (expansion): Term expansion (recall)
//! - E2-E4 (temporal): Recency, periodicity, sequence
//! - E9 (robustness): Noise-robust structure
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the foundation, but other embedders can be primary for exploration
//! - ARCH-02: All comparisons within same embedder space (no cross-embedder)
//! - Each embedder has its own FAISS/HNSW index on GPU

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns embedder-first search tool definitions (5 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // search_by_embedder - Generic search using any embedder as primary
        ToolDefinition::new(
            "search_by_embedder",
            "Search using any embedder (E1-E13) as the primary perspective. Each of the 13 embedders \
             sees the knowledge graph differently. E1 finds semantic similarity, E11 finds entity \
             relationships, E7 finds code patterns, E5 finds causal chains. Use this to explore \
             what a specific embedder sees that others might miss. Per Constitution v6.3 \
             embedder-first search philosophy.",
            json!({
                "type": "object",
                "required": ["embedder", "query"],
                "properties": {
                    "embedder": {
                        "type": "string",
                        "description": "Which embedder to use as primary (E1-E13). E1=semantic, E2=recency, \
                                        E3=periodic, E4=sequence, E5=causal, E6=keyword, E7=code, E8=graph, \
                                        E9=robustness, E10=intent, E11=entity, E12=precision, E13=expansion.",
                        "enum": ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"]
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to find similar memories in the selected embedder's space."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum number of results to return (1-100, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 100
                    },
                    "minSimilarity": {
                        "type": "number",
                        "description": "Minimum similarity threshold (0-1, default: 0). Results below this are filtered.",
                        "default": 0,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    },
                    "includeAllScores": {
                        "type": "boolean",
                        "description": "Include similarity scores from all 13 embedders in results (default: false). \
                                        Useful for understanding how different embedders view the same memory.",
                        "default": false
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_embedder_clusters - Explore clusters in a specific embedder's space
        ToolDefinition::new(
            "get_embedder_clusters",
            "Explore clusters of memories in a specific embedder's space. Each embedder creates \
             different clusters based on what it sees - E7 (code) clusters by implementation patterns, \
             E11 (entity) clusters by entity relationships, E5 (causal) clusters by cause-effect chains. \
             Use to discover emergent groupings from different perspectives.",
            json!({
                "type": "object",
                "required": ["embedder"],
                "properties": {
                    "embedder": {
                        "type": "string",
                        "description": "Which embedder's clusters to explore (E1-E13).",
                        "enum": ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"]
                    },
                    "minClusterSize": {
                        "type": "integer",
                        "description": "Minimum memories per cluster (default: 3, per HDBSCAN min_cluster_size).",
                        "default": 3,
                        "minimum": 2,
                        "maximum": 50
                    },
                    "topClusters": {
                        "type": "integer",
                        "description": "Maximum number of clusters to return (default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "includeSamples": {
                        "type": "boolean",
                        "description": "Include sample memories from each cluster (default: true).",
                        "default": true
                    },
                    "samplesPerCluster": {
                        "type": "integer",
                        "description": "Number of sample memories per cluster (default: 3).",
                        "default": 3,
                        "minimum": 1,
                        "maximum": 10
                    }
                },
                "additionalProperties": false
            }),
        ),
        // compare_embedder_views - Compare how different embedders rank the same query
        ToolDefinition::new(
            "compare_embedder_views",
            "Compare how different embedders rank the same query. Shows rankings from each embedder \
             side-by-side, highlighting agreement (same top results) and unique finds (memories found \
             by only one embedder). Useful for understanding blind spots - e.g., what E11 (entity) \
             finds that E1 (semantic) misses.",
            json!({
                "type": "object",
                "required": ["query", "embedders"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query to compare across embedders."
                    },
                    "embedders": {
                        "type": "array",
                        "description": "Which embedders to compare (2-5 embedders).",
                        "items": {
                            "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"]
                        },
                        "minItems": 2,
                        "maxItems": 5
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Number of top results per embedder to compare (default: 5).",
                        "default": 5,
                        "minimum": 1,
                        "maximum": 20
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include content text in results (default: false).",
                        "default": false
                    }
                },
                "additionalProperties": false
            }),
        ),
        // list_embedder_indexes - List all embedder indexes with stats
        ToolDefinition::new(
            "list_embedder_indexes",
            "List all 13 embedder indexes with their statistics. Shows dimension, index type, \
             vector count, size, and GPU residency for each embedder. Useful for understanding \
             the system's embedding infrastructure and checking index health.",
            json!({
                "type": "object",
                "properties": {
                    "includeDetails": {
                        "type": "boolean",
                        "description": "Include detailed stats like memory usage and query latency (default: true).",
                        "default": true
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_memory_fingerprint - Introspect per-embedder vectors for a specific memory
        ToolDefinition::new(
            "get_memory_fingerprint",
            "Retrieve the per-embedder fingerprint vectors for a specific memory. Returns dimension, \
             vector norm (L2), and presence status for each of the 13 embedders. Asymmetric embedders \
             (E5 causal, E8 graph, E10 intent) show both directional variants. Sparse embedders \
             (E6, E13) show non-zero element count. Use to debug embedding quality, verify which \
             embedders produced vectors, and understand how a memory is represented across all 13 spaces.",
            json!({
                "type": "object",
                "required": ["memory_id"],
                "properties": {
                    "memory_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the memory to inspect."
                    },
                    "embedders": {
                        "type": "array",
                        "description": "Filter to specific embedders (default: all 13). E.g., [\"E1\", \"E5\", \"E7\"].",
                        "items": {
                            "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"]
                        }
                    },
                    "includeVectorNorms": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include L2 norm of each vector (default: true)."
                    },
                    "includeContent": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include the memory's content text (default: false)."
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
    fn test_embedder_tool_count() {
        // 5 embedder-first search tools
        assert_eq!(definitions().len(), 5);
    }

    #[test]
    fn test_search_by_embedder_schema() {
        let tools = definitions();
        let search = tools.iter().find(|t| t.name == "search_by_embedder").unwrap();

        // Check required fields
        let required = search
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("embedder")));
        assert!(required.contains(&json!("query")));

        // Check embedder enum has all 13 values
        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        let embedder_enum = props["embedder"]["enum"].as_array().unwrap();
        assert_eq!(embedder_enum.len(), 13);
    }

    #[test]
    fn test_get_embedder_clusters_schema() {
        let tools = definitions();
        let clusters = tools.iter().find(|t| t.name == "get_embedder_clusters").unwrap();

        // Check required fields
        let required = clusters
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("embedder")));

        // Check defaults
        let props = clusters
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(props["minClusterSize"]["default"], 3);
        assert_eq!(props["topClusters"]["default"], 10);
    }

    #[test]
    fn test_compare_embedder_views_schema() {
        let tools = definitions();
        let compare = tools.iter().find(|t| t.name == "compare_embedder_views").unwrap();

        // Check required fields
        let required = compare
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("query")));
        assert!(required.contains(&json!("embedders")));

        // Check embedders array constraints
        let props = compare
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(props["embedders"]["minItems"], 2);
        assert_eq!(props["embedders"]["maxItems"], 5);
    }

    #[test]
    fn test_list_embedder_indexes_schema() {
        let tools = definitions();
        let list = tools.iter().find(|t| t.name == "list_embedder_indexes").unwrap();

        // No required fields
        let required = list.input_schema.get("required");
        assert!(required.is_none());

        // Check includeDetails default
        let props = list
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(props["includeDetails"]["default"], true);
    }

    #[test]
    fn test_all_tools_mention_embedder() {
        let tools = definitions();

        for tool in &tools {
            assert!(
                tool.description.contains("embedder") || tool.description.contains("E1"),
                "Tool {} should mention embedder concepts",
                tool.name
            );
        }
    }
}
