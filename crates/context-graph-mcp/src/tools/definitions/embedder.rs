//! Embedder-first search tool definitions.
//!
//! Per Constitution v6.3, these tools enable AI agents to search using active
//! embedders as the primary perspective. Each active embedder sees the knowledge
//! graph differently - E7 reveals code patterns, E14
//! reveals cross-lingual and long-document semantic matches, etc.
//!
//! ## Philosophy
//!
//! The active embedders are lenses on the same knowledge:
//! - E1 (semantic): Dense semantic similarity - foundation
//! - E5 (causal): Retired; not loaded, routed, searched, or weighted
//! - E6 (keyword): Exact keyword matches
//! - E7 (code): Code patterns, function signatures
//! - E8 (graph): Structural relationships (imports, deps)
//! - E10 (paraphrase): Same meaning, different wording
//! - E11 (entity): Disabled until a verified code-symbol entity embedder exists
//! - E12 (precision): Exact phrase matches (reranking)
//! - E13 (expansion): Term expansion (recall)
//! - E2-E4 (temporal): Recency, periodicity, sequence
//! - E9 (robustness): Noise-robust structure
//! - E14 (multilingual/long-context): BGE-M3 Dense, 1024-D, 8192-token context
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the foundation, but other embedders can be primary for exploration
//! - ARCH-02: All comparisons within same embedder space (no cross-embedder)
//! - Each embedder has its own FAISS/HNSW index on GPU

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns embedder-first search and ME-JEPA learner-state tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // search_by_embedder - Generic search using any embedder as primary
        ToolDefinition::new(
            "search_by_embedder",
            "Search using any active embedder as the primary perspective. Each active embedder \
             sees the knowledge graph differently. E1 finds semantic similarity, E7 finds code patterns, \
             E14 finds multilingual/long-context matches. Use this to explore \
             what a specific embedder sees that others might miss. Per Constitution v6.3 \
             embedder-first search philosophy. E5 causal is retired and E11 entity is disabled; both are rejected if requested.",
            json!({
                "type": "object",
                "required": ["embedder", "query"],
                "properties": {
                    "embedder": {
                        "type": "string",
                        "description": "Which active embedder to use as primary. E1=semantic, E2=recency, \
                                        E3=periodic, E4=sequence, E7=code, E8=graph, \
                                        E9=robustness, E10=paraphrase, \
                                        E14=multilingual/long-context (BGE-M3 Dense, 1024-D, 8192-token context). \
                                        E5 is retired; E11 is disabled. E6/E12/E13 use non-HNSW indexes and are not supported for direct search.",
                        "enum": ["E1", "E2", "E3", "E4", "E7", "E8", "E9", "E10", "E14"]
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
                        "description": "Include similarity scores from active embedders in results (default: false). \
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
             E14 clusters by multilingual/long-context semantics. \
             Use to discover emergent groupings from different perspectives.",
            json!({
                "type": "object",
                "required": ["embedder"],
                "properties": {
                    "embedder": {
                        "type": "string",
                        "description": "Which active embedder's clusters to explore. E14 reveals cross-lingual/long-document clusters. E5 is retired and E11 is disabled. E6/E12/E13 use non-HNSW indexes and are not supported for clustering.",
                        "enum": ["E1", "E2", "E3", "E4", "E7", "E8", "E9", "E10", "E14"]
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
             by only one embedder). Useful for understanding blind spots between active spaces.",
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
                        "description": "Which active embedders to compare (2-5 embedders). E14 is ideal for revealing cross-lingual or long-document blind spots vs E1. E5 is retired and E11 is disabled. E6/E12/E13 use non-HNSW indexes and are not supported.",
                        "items": {
                            "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E7", "E8", "E9", "E10", "E14"]
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
            "List active embedder indexes with their statistics. Shows dimension, index type, \
             vector count, size, and GPU residency for each embedder. Useful for understanding \
             the system's embedding infrastructure and checking index health. E5 causal is retired.",
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
             vector norm (L2), and presence status for each persisted storage slot, plus explicit \
             activeEmbedderCount, storageSlotCount, and disabledEmbedders fields. Asymmetric embedders \
             (E8 graph, E10 paraphrase) show both directional variants. Sparse embedders \
             (E6, E13) show non-zero element count. E14 (BGE-M3 Dense) shows the 1024-D multilingual \
             vector. Use to debug embedding quality, verify which embedders produced vectors, and \
             understand how a memory is represented across active spaces. E5 causal and E11 entity \
             are retained as schema slots only and should report present=false.",
            json!({
                "type": "object",
                "required": ["memoryId"],
                "properties": {
                    "memoryId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the memory to inspect."
                    },
                    "embedders": {
                        "type": "array",
                        "description": "Filter to specific storage slots (default: all 14 slots). E.g., [\"E1\", \"E7\", \"E14\"]. E5 and E11 are disabled inspection-only slots.",
                        "items": {
                            "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13", "E14"]
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
        // create_weight_profile - Create a session-scoped custom weight profile
        ToolDefinition::new(
            "create_weight_profile",
            "Create a named custom embedder weight profile for the current session. Assigns weights \
             to active embedders (E1-E14 with retired E5 and disabled E11 fixed at 0). The profile can be referenced by name in \
             search_graph's weightProfile and get_unified_neighbors. Useful \
             for defining reusable search strategies. Rejects built-in profile names.",
            json!({
                "type": "object",
                "required": ["name", "weights"],
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name for the profile (1-64 chars). Must not conflict with built-in profiles.",
                        "minLength": 1,
                        "maxLength": 64
                    },
                    "weights": {
                        "type": "object",
                        "description": "Per-embedder weights. Keys are E1-E14, values are 0-1 except retired E5 and disabled E11 which must be 0. Must sum to ~1.0. Temporal embedders are independent: E2 (recency — how recently stored), E3 (periodicity — time-of-day/day-of-week patterns), E4 (sequence — conversation ordering). E14 (BGE-M3 Dense) covers multilingual + long-context (8192 tokens).",
                        "properties": {
                            "E1": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E2": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E3": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E4": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E5": { "type": "number", "minimum": 0, "maximum": 0, "description": "Retired and disabled; must be 0" },
                            "E6": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E7": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E8": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E9": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E10": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E11": { "type": "number", "minimum": 0, "maximum": 0, "description": "Disabled; must be 0 until a verified code-symbol entity embedder is installed" },
                            "E12": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E13": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E14": { "type": "number", "minimum": 0, "maximum": 1 }
                        },
                        "additionalProperties": false
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional description of the profile's purpose."
                    }
                },
                "additionalProperties": false
            }),
        ),
        // search_cross_embedder_anomalies - Find blind spots between embedders
        ToolDefinition::new(
            "search_cross_embedder_anomalies",
            "Find memories that score high in one embedder but low in another. Reveals blind \
             spots and perspective disagreements. Example: highEmbedder=E7 (code), \
             lowEmbedder=E1 (semantic) finds code patterns that semantic search misses. \
             Anomaly score = high_score - low_score.",
            json!({
                "type": "object",
                "required": ["query", "highEmbedder", "lowEmbedder"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query."
                    },
                    "highEmbedder": {
                        "type": "string",
                        "description": "Active embedder expected to score HIGH. Use E14 as highEmbedder with E1 as lowEmbedder to find cross-lingual/long-document hits that English-centric semantic search misses. E5 is retired and E11 is disabled.",
                        "enum": ["E1", "E2", "E3", "E4", "E6", "E7", "E8", "E9", "E10", "E12", "E13", "E14"]
                    },
                    "lowEmbedder": {
                        "type": "string",
                        "description": "Active embedder expected to score LOW. E5 is retired and E11 is disabled.",
                        "enum": ["E1", "E2", "E3", "E4", "E6", "E7", "E8", "E9", "E10", "E12", "E13", "E14"]
                    },
                    "highThreshold": {
                        "type": "number",
                        "description": "Minimum score in highEmbedder (0-1, default: 0.5).",
                        "default": 0.5,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "lowThreshold": {
                        "type": "number",
                        "description": "Maximum score in lowEmbedder (0-1, default: 0.3).",
                        "default": 0.3,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum results (1-100, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 100
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include content text in results (default: false).",
                        "default": false
                    },
                    "primaryEmbedder": {
                        "type": "string",
                        "description": "Primary embedder for generalized blind spot detection (default: E1). Try E14 to surface cross-lingual or long-document content E1 misses.",
                        "default": "E1",
                        "enum": ["E1", "E2", "E3", "E4", "E6", "E7", "E8", "E9", "E10", "E12", "E13", "E14"]
                    },
                    "contrastEmbedder": {
                        "type": "string",
                        "description": "Contrast embedder for generalized blind spot detection (default: E9).",
                        "default": "E9",
                        "enum": ["E1", "E2", "E3", "E4", "E6", "E7", "E8", "E9", "E10", "E12", "E13", "E14"]
                    }
                },
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "mejepa_load_learner_state",
            "Fail-closed Phase 1b learner-state load preflight for E15-E21. Validates the Phase 1b models_config.toml schema, SHA-pinned model files, and E17 calibration certificate when loading AffectText. No CPU fallback and no synthetic learner-state output.",
            json!({
                "type": "object",
                "required": ["modelsConfigPath", "embedder"],
                "properties": {
                    "modelsConfigPath": {"type": "string"},
                    "embedder": {"type": "string", "enum": ["e15", "e16", "e17", "e18", "e19", "e20", "e21"]},
                    "calibrationCertPath": {"type": "string"}
                },
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "mejepa_routing_lookup",
            "Lookup the fail-closed AST entity to embedder routing for one language/entity pair. Covers 13 EntityType variants across the 11 Phase 1 languages.",
            json!({
                "type": "object",
                "required": ["language", "entityType"],
                "properties": {
                    "language": {"type": "string", "enum": ["rust", "python", "javascript", "typescript", "go", "java", "c", "cpp", "csharp", "ruby", "php"]},
                    "entityType": {"type": "string", "enum": ["function", "method", "class", "struct", "enum", "trait_or_interface", "impl", "module", "namespace", "test_function", "import", "comment_block", "docstring"]}
                },
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "mejepa_vram_budget_report",
            "Run the CUDA cuMemGetInfo-backed Phase 1b VRAM budget check and include non-authoritative hardened nvidia-smi WSL diagnostics.",
            json!({
                "type": "object",
                "properties": {
                    "budget": {"type": "string", "enum": ["content_set", "full_phase1"], "default": "content_set"}
                },
                "additionalProperties": false
            }),
        ),
    ]
}

/// Returns E12/E13 standalone search tool definitions (2 tools).
pub fn standalone_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "search_by_tokens",
            "Search using E12 ColBERT token-level MaxSim scoring for precise phrase matching.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Query text" },
                    "topK": { "type": "integer", "default": 10, "minimum": 1, "maximum": 100 },
                    "minSimilarity": { "type": "number", "default": 0.3, "minimum": 0.0, "maximum": 1.0 }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "search_by_expansion",
            "Search using E13 SPLADE learned term expansion for enhanced keyword recall.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Query text" },
                    "topK": { "type": "integer", "default": 10, "minimum": 1, "maximum": 100 },
                    "minScore": { "type": "number", "default": 0.1, "minimum": 0.0, "maximum": 1.0 }
                },
                "required": ["query"],
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
        assert_eq!(tools.len(), 10);
        let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "search_by_embedder",
                "get_embedder_clusters",
                "compare_embedder_views",
                "list_embedder_indexes",
                "get_memory_fingerprint",
                "create_weight_profile",
                "search_cross_embedder_anomalies",
                "mejepa_load_learner_state",
                "mejepa_routing_lookup",
                "mejepa_vram_budget_report",
            ]
        );
        for tool in &tools {
            assert!(
                tool.description.contains("embedder")
                    || tool.description.contains("E1")
                    || tool.name.starts_with("mejepa_"),
                "Tool {} should mention embedder or ME-JEPA concepts",
                tool.name
            );
        }
        let search = tools
            .iter()
            .find(|t| t.name == "search_by_embedder")
            .unwrap();
        let required = search
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("embedder")));
        assert!(required.contains(&json!("query")));
        let props = search
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        let embedder_enum = props["embedder"]["enum"].as_array().unwrap();
        // CD-M1 FIX: E6/E12/E13 removed from search_by_embedder (non-HNSW)
        // E14 added post-Phase-C (HNSW-capable 1024-D BGE-M3 Dense)
        assert_eq!(
            embedder_enum,
            &vec![
                json!("E1"),
                json!("E2"),
                json!("E3"),
                json!("E4"),
                json!("E7"),
                json!("E8"),
                json!("E9"),
                json!("E10"),
                json!("E14")
            ]
        );
    }

    #[test]
    fn test_schema_defaults_and_constraints() {
        let tools = definitions();
        let clusters_props = tools
            .iter()
            .find(|t| t.name == "get_embedder_clusters")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap();
        assert_eq!(clusters_props["minClusterSize"]["default"], 3);
        assert_eq!(clusters_props["topClusters"]["default"], 10);
        let compare_props = tools
            .iter()
            .find(|t| t.name == "compare_embedder_views")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap();
        assert_eq!(compare_props["embedders"]["minItems"], 2);
        assert_eq!(compare_props["embedders"]["maxItems"], 5);
    }
}
