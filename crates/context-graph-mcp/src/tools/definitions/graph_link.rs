//! Graph linking tool definitions.
//!
//! Defines MCP tools for K-NN graph navigation and typed edge queries:
//! - `get_memory_neighbors`: Get K nearest neighbors in specific embedder space
//! - `get_typed_edges`: Get typed edges from a memory
//! - `traverse_graph`: Multi-hop graph traversal

use serde_json::json;

use super::super::types::ToolDefinition;

/// Get all graph linking tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        get_memory_neighbors_definition(),
        get_typed_edges_definition(),
        traverse_graph_definition(),
        get_unified_neighbors_definition(),
    ]
}

fn get_memory_neighbors_definition() -> ToolDefinition {
    ToolDefinition::new(
        "get_memory_neighbors",
        "Get K nearest neighbors of a memory in a specific embedder space using pre-computed \
         K-NN edges. Returns neighbors sorted by similarity. NOTE: Recently stored memories \
         may return 0 neighbors until the background K-NN graph build runs (~60s). \
         Use get_unified_neighbors for immediate results on recent memories.",
        json!({
            "type": "object",
            "required": ["memory_id"],
            "properties": {
                "memory_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "UUID of the memory to find neighbors for"
                },
                "embedder_id": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 12,
                    "default": 0,
                    "description": "Embedder space to search (0=E1 semantic, 6=E7 code, 7=E8 graph, 9=E10 paraphrase). E5 causal is retired and E11 entity is disabled."
                },
                "top_k": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 10,
                    "description": "Number of neighbors to return (default: 10)"
                },
                "min_similarity": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.0,
                    "description": "Minimum similarity threshold (default: 0.0)"
                },
                "include_content": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include memory content in results (default: false)"
                }
            },
            "additionalProperties": false
        }),
    )
}

fn get_typed_edges_definition() -> ToolDefinition {
    ToolDefinition::new(
        "get_typed_edges",
        "Get typed edges from a memory. Typed edges represent relationships derived from \
         embedder agreement patterns: semantic_similar, code_related, entity_shared, \
         causal_chain, graph_connected, paraphrase_aligned, keyword_overlap, multi_agreement.",
        json!({
            "type": "object",
            "required": ["memory_id"],
            "properties": {
                "memory_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "UUID of the memory to get edges from"
                },
                "edge_type": {
                    "type": "string",
                    "enum": [
                        "semantic_similar",
                        "code_related",
                        "entity_shared",
                        "causal_chain",
                        "graph_connected",
                        "paraphrase_aligned",
                        "keyword_overlap",
                        "multi_agreement"
                    ],
                    "description": "Filter by edge type (optional, returns all types if not specified)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["outgoing", "incoming", "both"],
                    "default": "outgoing",
                    "description": "Edge direction: outgoing (from memory), incoming (to memory), both"
                },
                "min_weight": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.0,
                    "description": "Minimum edge weight threshold (default: 0.0)"
                },
                "include_content": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include memory content in results (default: false)"
                }
            },
            "additionalProperties": false
        }),
    )
}

fn traverse_graph_definition() -> ToolDefinition {
    ToolDefinition::new(
        "traverse_graph",
        "Multi-hop graph traversal starting from a memory. Explores the knowledge graph \
         following typed edges up to a maximum depth. Useful for discovering connected \
         memories, causal chains, or code dependencies.",
        json!({
            "type": "object",
            "required": ["start_memory_id"],
            "properties": {
                "start_memory_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "UUID of the starting memory"
                },
                "max_hops": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "default": 2,
                    "description": "Maximum traversal depth (default: 2, max: 5)"
                },
                "edge_type": {
                    "type": "string",
                    "enum": [
                        "semantic_similar",
                        "code_related",
                        "entity_shared",
                        "causal_chain",
                        "graph_connected",
                        "paraphrase_aligned",
                        "keyword_overlap",
                        "multi_agreement"
                    ],
                    "description": "Filter traversal by edge type (optional)"
                },
                "min_weight": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.3,
                    "description": "Minimum edge weight to follow (default: 0.3)"
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20,
                    "description": "Maximum paths to return (default: 20)"
                },
                "include_content": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include memory content in results (default: false)"
                }
            },
            "additionalProperties": false
        }),
    )
}

fn get_unified_neighbors_definition() -> ToolDefinition {
    ToolDefinition::new(
        "get_unified_neighbors",
        "Find neighbors using Weighted RRF fusion across active embedders, providing a unified \
         view where neighbors are ranked by how consistently multiple embedders agree they are \
         related. Unlike get_memory_neighbors (single embedder), this shows what active embedders \
         agree on. Per ARCH-21: Uses Weighted RRF, not weighted sum. Per AP-60: Temporal \
         embedders (E2-E4) are excluded from semantic fusion. E5 causal is retired and E11 entity is disabled; both are fixed at zero weight.",
        json!({
            "type": "object",
            "required": ["memory_id"],
            "properties": {
                "memory_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "UUID of the memory to find unified neighbors for"
                },
                "weight_profile": {
                    "type": "string",
                    "enum": [
                        "semantic_search", "causal_reasoning", "code_search", "fact_checking",
                        "graph_reasoning", "temporal_navigation", "sequence_navigation",
                        "conversation_history", "category_weighted", "typo_tolerant",
                        "pipeline_stage1_recall", "pipeline_stage2_scoring", "pipeline_full",
                        "balanced",
                        "multilingual_search", "long_context", "translation_finder",
                        "affect_repair", "affect_priming", "affect_neutral"
                    ],
                    "default": "semantic_search",
                    "description": "Weight profile for RRF fusion. Temporal profiles: temporal_navigation (E2+E3+E4 balanced), sequence_navigation (E4-heavy), conversation_history (E4+E1). Multilingual/long-doc profiles (E14 BGE-M3 Dense): multilingual_search (E14=0.40 cross-lingual), long_context (E14=0.45, only embedder seeing past 512 tokens), translation_finder (E14=0.70 for translation pairs). Learner-state profiles: affect_repair, affect_priming, affect_neutral. Use customWeights for fine-grained E2/E3/E4/E14 control."
                },
                "top_k": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 10,
                    "description": "Number of neighbors to return (default: 10)"
                },
                "min_score": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.0,
                    "description": "Minimum RRF score threshold (default: 0.0)"
                },
                "include_content": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include memory content in results (default: false)"
                },
                "include_embedder_breakdown": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include per-embedder scores and ranks in results (default: true)"
                },
                "custom_weights": {
                    "type": "object",
                    "description": "Custom per-embedder weights (overrides weight_profile). Each active value is 0-1, retired E5 and disabled E11 must be 0, and weights must sum to ~1.0.",
                    "properties": {
                        "E1":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E2":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E3":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E4":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E5":  { "type": "number", "minimum": 0, "maximum": 0, "description": "Retired and disabled; must be 0" },
                        "E6":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E7":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E8":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E9":  { "type": "number", "minimum": 0, "maximum": 1 },
                        "E10": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E11": { "type": "number", "minimum": 0, "maximum": 0, "description": "Disabled; must be 0 until a verified code-symbol entity embedder is installed" },
                        "E12": { "type": "number", "minimum": 0, "maximum": 1 },
                        "E13": { "type": "number", "minimum": 0, "maximum": 1 },
                        "E14": { "type": "number", "minimum": 0, "maximum": 1, "description": "BGE-M3 Dense — multilingual 1024-D CLS-pooled (XLM-RoBERTa-Large, 8192-token context)" }
                    },
                    "additionalProperties": false
                },
                "exclude_embedders": {
                    "type": "array",
                    "description": "Embedders to exclude from fusion (their weight becomes 0, remaining renormalized).",
                    "items": {
                        "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E6", "E7", "E8", "E9", "E10", "E12", "E13", "E14"]
                    }
                },
                "learning_policy": {
                    "type": "object",
                    "description": "Explicit learned graph policy to apply. Matching persisted graph LearningEvents are required; if evidence is missing, the tool errors instead of falling back to base RRF.",
                    "properties": {
                        "policy_scope": {
                            "type": "string",
                            "description": "Policy scope used by record_graph_learning_event."
                        },
                        "min_evidence": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 10000,
                            "default": 1
                        },
                        "max_scan": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 200000,
                            "default": 10000
                        },
                        "learning_rate": {
                            "type": "number",
                            "minimum": 0.0,
                            "maximum": 1.0,
                            "default": 0.35
                        },
                        "include_evidence": {
                            "type": "boolean",
                            "default": false
                        }
                    },
                    "required": ["policy_scope"],
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_exist_with_required_fields() {
        let defs = definitions();
        assert_eq!(defs.len(), 4);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"get_memory_neighbors"));
        assert!(names.contains(&"get_typed_edges"));
        assert!(names.contains(&"traverse_graph"));
        assert!(names.contains(&"get_unified_neighbors"));
        for def in &defs {
            assert!(def.input_schema.is_object());
            assert!(def.input_schema.get("type").is_some());
            assert!(def.input_schema.get("properties").is_some());
        }
    }

    #[test]
    fn test_unified_neighbors_schema() {
        let defs = definitions();
        let unified = defs
            .iter()
            .find(|d| d.name == "get_unified_neighbors")
            .unwrap();
        let props = unified.input_schema.get("properties").unwrap();
        assert!(props.get("memory_id").is_some());
        assert!(props.get("weight_profile").is_some());
        assert!(unified.description.contains("RRF"));
        assert!(unified.description.contains("ARCH-21"));
    }
}
