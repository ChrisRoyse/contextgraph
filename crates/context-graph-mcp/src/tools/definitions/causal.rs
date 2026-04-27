//! Causal tool definitions (search_causes, get_causal_chain).
//!
//! E5 Priority 1 Enhancement: Leverage asymmetric E5 embeddings for causal reasoning.
//!
//! Constitution Compliance:
//! - ARCH-15: Uses asymmetric E5 with separate cause/effect encodings
//! - AP-77: Direction modifiers: cause→effect=1.2, effect→cause=0.8

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns causal tool definitions (4 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // search_causal_relationships - Search LLM-generated causal descriptions with provenance
        ToolDefinition::new(
            "search_causal_relationships",
            "Search for causal relationships using E5 asymmetric similarity. \
             Returns LLM-generated 1-3 paragraph descriptions explaining causal mechanisms, \
             with full provenance linking to source memories. Use for understanding causal \
             relationships with rich explanations and evidence.",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language query about causal relationships. E.g., 'What causes memory problems?' or 'Effects of stress on health'."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["cause", "effect", "all"],
                        "description": "Filter by causal direction: 'cause' (X causes Y), 'effect' (X is caused by Y), or 'all' (no filter). Default: 'all'.",
                        "default": "all"
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum number of results (1-100, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 100
                    },
                    "includeSource": {
                        "type": "boolean",
                        "description": "Include original source content in results (default: true). Set to false for smaller response.",
                        "default": true
                    },
                    "includeProvenance": {
                        "type": "boolean",
                        "description": "Include retrieval provenance metadata in results (default: false). Shows search mode, embedder weights, LLM provenance.",
                        "default": false
                    },
                    "sourceWeight": {
                        "type": "number",
                        "description": "Weight for source-anchored embeddings in hybrid search (0-1, default: 0.6). Prevents LLM output clustering.",
                        "default": 0.6,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "explanationWeight": {
                        "type": "number",
                        "description": "Weight for explanation embeddings in hybrid search (0-1, default: 0.4).",
                        "default": 0.4,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "multiEmbedder": {
                        "type": "boolean",
                        "description": "Enable multi-embedder search for maximum accuracy (default: false). Uses E1+E5+E8+E11 with consensus scoring. Requires direction to be 'cause' or 'effect'.",
                        "default": false
                    },
                    "minConsensus": {
                        "type": "number",
                        "description": "Minimum consensus threshold for multi-embedder search (0-1, default: 0.0). Results below this across embedders are filtered.",
                        "default": 0.0,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "e1Weight": {
                        "type": "number",
                        "description": "E1 semantic weight in multi-embedder mode (0-1, default: 0.30).",
                        "default": 0.30,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "e5Weight": {
                        "type": "number",
                        "description": "E5 causal weight in multi-embedder mode (0-1, default: 0.35).",
                        "default": 0.35,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "e8Weight": {
                        "type": "number",
                        "description": "E8 graph weight in multi-embedder mode (0-1, default: 0.15).",
                        "default": 0.15,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "e11Weight": {
                        "type": "number",
                        "description": "E11 entity weight in multi-embedder mode (0-1, default: 0.20).",
                        "default": 0.20,
                        "minimum": 0,
                        "maximum": 1
                    }
                },
                "additionalProperties": false
            }),
        ),
        // search_causes - Abductive reasoning to find likely causes
        ToolDefinition::new(
            "search_causes",
            "Abductive reasoning to find likely causes of observed effects. \
             Uses asymmetric E5 similarity with 0.8x effect→cause dampening (per AP-77). \
             Returns ranked causes with abductive scores. Use for \"why did X happen?\" queries.",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The observed effect to find causes for. Describe what happened that you want to explain."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum number of causes to return (1-50, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "minScore": {
                        "type": "number",
                        "description": "Minimum abductive score threshold (0-1, default: 0.1). Results below this are filtered.",
                        "default": 0.1,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    },
                    "filterCausalDirection": {
                        "type": "string",
                        "enum": ["cause", "effect", "unknown"],
                        "description": "Filter results by persisted causal direction. Omit for no filtering."
                    },
                    "searchScope": {
                        "type": "string",
                        "enum": ["memories", "relationships", "all"],
                        "description": "Search scope: 'memories' (fingerprint HNSW, default), 'relationships' (CF_CAUSAL_RELATIONSHIPS E5 brute-force), or 'all' (both merged by score).",
                        "default": "memories"
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["e1_only", "multi_space", "pipeline"],
                        "description": "Search strategy: 'e1_only' (E1 only), 'multi_space' (default, multi-embedder fusion), or 'pipeline' (E13 SPLADE recall -> E1 -> E12 ColBERT rerank)."
                    },
                    "rerankWeight": {
                        "type": "number",
                        "description": "E12 rerank weight for blending with fusion score (0-1, default: 0.4). Only used when strategy='pipeline'.",
                        "default": 0.4,
                        "minimum": 0,
                        "maximum": 1
                    }
                },
                "additionalProperties": false
            }),
        ),
        // search_effects - Find effects/consequences of a cause
        ToolDefinition::new(
            "search_effects",
            "Find effects/consequences of a given cause using E5 asymmetric embeddings. \
             Uses 1.2x cause→effect boost (per AP-77) for forward causal reasoning. \
             Returns ranked effects with predictive scores. Use for \"what will X cause?\" queries.",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The cause to find effects for. Describe the action or event whose consequences you want to predict."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum number of effects to return (1-50, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "minScore": {
                        "type": "number",
                        "description": "Minimum predictive score threshold (0-1, default: 0.1). Results below this are filtered.",
                        "default": 0.1,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full content text in results (default: false).",
                        "default": false
                    },
                    "filterCausalDirection": {
                        "type": "string",
                        "enum": ["cause", "effect", "unknown"],
                        "description": "Filter results by persisted causal direction. Omit for no filtering."
                    },
                    "searchScope": {
                        "type": "string",
                        "enum": ["memories", "relationships", "all"],
                        "description": "Search scope: 'memories' (fingerprint HNSW, default), 'relationships' (CF_CAUSAL_RELATIONSHIPS E5 brute-force), or 'all' (both merged by score).",
                        "default": "memories"
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["e1_only", "multi_space", "pipeline"],
                        "description": "Search strategy: 'e1_only' (E1 only), 'multi_space' (default, multi-embedder fusion), or 'pipeline' (E13 SPLADE recall -> E1 -> E12 ColBERT rerank)."
                    },
                    "rerankWeight": {
                        "type": "number",
                        "description": "E12 rerank weight for blending with fusion score (0-1, default: 0.4). Only used when strategy='pipeline'.",
                        "default": 0.4,
                        "minimum": 0,
                        "maximum": 1
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_causal_chain - Build transitive causal chains
        ToolDefinition::new(
            "get_causal_chain",
            "Build and visualize transitive causal chains from an anchor point. \
             Iteratively searches for causally-related memories using asymmetric E5 similarity. \
             Applies hop attenuation (0.9^hop) for chain scoring. Use for causal path visualization.",
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
                        "description": "Direction to traverse: forward (cause→effect) or backward (effect→cause). Default: forward.",
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
        assert_eq!(tools.len(), 4);
        // All mention E5 or asymmetric
        for tool in &tools {
            assert!(tool.description.contains("asymmetric") || tool.description.contains("E5"));
        }
        let causes = tools.iter().find(|t| t.name == "search_causes").unwrap();
        assert!(causes.description.contains("AP-77") || causes.description.contains("0.8"));
        let chain = tools.iter().find(|t| t.name == "get_causal_chain").unwrap();
        assert!(chain.description.contains("attenuation") || chain.description.contains("0.9"));
    }

    #[test]
    fn test_schema_defaults_and_enums() {
        let tools = definitions();
        let causes_props = tools
            .iter()
            .find(|t| t.name == "search_causes")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(causes_props["topK"]["default"], 10);
        assert_eq!(causes_props["minScore"]["default"], 0.1);
        let chain_props = tools
            .iter()
            .find(|t| t.name == "get_causal_chain")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(chain_props["direction"]["default"], "forward");
        assert_eq!(chain_props["maxHops"]["default"], 5);
    }
}
