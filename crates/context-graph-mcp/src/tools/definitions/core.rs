//! Core tool definitions per PRD v6 Section 10.
//!
//! Tools: store_memory, get_memetic_status, search_graph, trigger_consolidation
//!
//! Note: inject_context was merged into store_memory. When `rationale` is provided,
//! the same validation (1-1024 chars) and response format is used.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns core tool definitions (4 tools - inject_context merged into store_memory).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // store_memory - store a memory node directly
        // Note: inject_context merged into this tool. When rationale is provided,
        // the same validation and response format is used.
        ToolDefinition::new(
            "store_memory",
            "Store a memory node directly in the knowledge graph without UTL processing.",
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    },
                    "rationale": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 1024,
                        "description": "Why this context is relevant and should be stored (OPTIONAL, 1-1024 chars). When provided, included in response."
                    },
                    "importance": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.5,
                        "description": "Importance score for the memory [0.0, 1.0]"
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Session ID for session-scoped storage. If omitted, uses CLAUDE_SESSION_ID env var."
                    },
                    "operatorId": {
                        "type": "string",
                        "description": "Operator/user ID for audit provenance tracking"
                    }
                },
                "required": ["content"]
            }),
        ),
        // get_memetic_status - get system state and metrics
        ToolDefinition::new(
            "get_memetic_status",
            "Get current system status including fingerprint count, number of embedders (13), \
             storage backend and size, and layer status from LayerStatusProvider.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        // search_graph - semantic search with E5 causal and E10 paraphrase asymmetric similarity (ARCH-15, AP-77)
        ToolDefinition::new(
            "search_graph",
            "Search the knowledge graph using multi-space semantic similarity. \
             For causal queries ('why', 'what happens'), automatically applies \
             asymmetric E5 similarity with direction modifiers (cause→effect 1.2x, \
             effect→cause 0.8x). For paraphrase queries, applies E10 asymmetric similarity \
             (paraphrase→context 1.2x, context→paraphrase 0.8x). Returns nodes matching the query with relevance scores.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query text"
                    },
                    "topK": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 10,
                        "description": "Maximum number of results to return"
                    },
                    "minSimilarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.0,
                        "description": "Minimum similarity threshold [0.0, 1.0]"
                    },
                    "includeContent": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include content text in results"
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["e1_only", "multi_space", "pipeline"],
                        "default": "multi_space",
                        "description": "Search strategy: e1_only (fast, E1 only), multi_space (default - E1 + enhancers via Weighted RRF, uses weightProfile), pipeline (E13 recall → E1 dense → E12 rerank)"
                    },
                    "weightProfile": {
                        "type": "string",
                        "enum": [
                            "semantic_search", "causal_reasoning", "code_search", "fact_checking",
                            "graph_reasoning", "temporal_navigation", "sequence_navigation",
                            "conversation_history", "category_weighted", "typo_tolerant",
                            "pipeline_stage1_recall", "pipeline_stage2_scoring", "pipeline_full",
                            "balanced"
                        ],
                        "description": "Weight profile for multi-space search. All 14 profiles available. Pipeline profiles (pipeline_*) are for staged retrieval. balanced gives equal weight to all embedders."
                    },
                    "customWeights": {
                        "type": "object",
                        "description": "Custom per-embedder weights (overrides weightProfile). Each value 0-1, must sum to ~1.0. Omitted embedders default to 0.",
                        "properties": {
                            "E1":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E2":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E3":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E4":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E5":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E6":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E7":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E8":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E9":  { "type": "number", "minimum": 0, "maximum": 1 },
                            "E10": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E11": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E12": { "type": "number", "minimum": 0, "maximum": 1 },
                            "E13": { "type": "number", "minimum": 0, "maximum": 1 }
                        },
                        "additionalProperties": false
                    },
                    "excludeEmbedders": {
                        "type": "array",
                        "description": "Embedders to exclude from fusion (their weight becomes 0, remaining renormalized).",
                        "items": {
                            "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"]
                        }
                    },
                    "includeEmbedderBreakdown": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include per-embedder scores and contribution breakdown in results. Shows which embedders contributed most to each result's ranking."
                    },
                    "enableRerank": {
                        "type": "boolean",
                        "default": false,
                        "description": "Enable ColBERT E12 re-ranking (Stage 3)"
                    },
                    "enableAsymmetricE5": {
                        "type": "boolean",
                        "default": true,
                        "description": "Enable asymmetric E5 causal reranking for detected causal queries"
                    },
                    "causalDirection": {
                        "type": "string",
                        "enum": ["auto", "cause", "effect", "none"],
                        "default": "auto",
                        "description": "Causal direction: auto (detect from query), cause (seeking causes), effect (seeking effects), none (disable)"
                    },
                    "enableQueryExpansion": {
                        "type": "boolean",
                        "default": false,
                        "description": "Expand causal queries with related terms for better recall"
                    },
                    "temporalWeight": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.0,
                        "description": "Weight for temporal post-retrieval boost [0.0, 1.0]"
                    },
                    "conversationContext": {
                        "type": "object",
                        "description": "Convenience wrapper for sequence-based retrieval. Auto-anchors to current conversation turn.",
                        "properties": {
                            "anchorToCurrentTurn": {
                                "type": "boolean",
                                "default": true,
                                "description": "Auto-anchor to current session sequence (overrides sequenceAnchor)"
                            },
                            "turnsBack": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 100,
                                "default": 10,
                                "description": "Number of turns to look back from anchor"
                            },
                            "turnsForward": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 100,
                                "default": 0,
                                "description": "Number of turns to look forward from anchor"
                            }
                        }
                    },
                    "sessionScope": {
                        "type": "string",
                        "enum": ["current", "all", "recent"],
                        "default": "all",
                        "description": "Session scope: current (this session only), all (any session), recent (last 24h across sessions)"
                    },
                    "decayHalfLifeSecs": {
                        "type": "integer",
                        "description": "Half-life in seconds for exponential temporal decay (default: 86400 = 1 day). Only used with decayFunction='exponential'."
                    },
                    "lastHours": {
                        "type": "number",
                        "description": "Filter results to the last N hours. Shortcut for temporal window filtering."
                    },
                    "lastDays": {
                        "type": "number",
                        "description": "Filter results to the last N days. Shortcut for temporal window filtering."
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Filter results to a specific session ID."
                    },
                    "periodicBoost": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "description": "Weight for E3 periodic matching boost (0-1). Boosts results matching the target time pattern."
                    },
                    "targetHour": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 23,
                        "description": "Target hour of day (0-23) for periodic matching. Used with periodicBoost."
                    },
                    "targetDayOfWeek": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 6,
                        "description": "Target day of week (0=Sun, 6=Sat) for periodic matching. Used with periodicBoost."
                    },
                    "sequenceAnchor": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of anchor memory for E4 sequence-based retrieval. Finds memories near this point in the conversation."
                    },
                    "sequenceDirection": {
                        "type": "string",
                        "enum": ["before", "after", "around"],
                        "description": "Direction for sequence-based retrieval relative to sequenceAnchor: 'before', 'after', or 'around' (both directions)."
                    },
                    "includeProvenance": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include retrieval provenance metadata in results (default: false). Shows strategy, weight profile, query classification, and per-embedder contributions."
                    },
                    "decayFunction": {
                        "type": "string",
                        "enum": ["linear", "exponential", "step", "none", "no_decay"],
                        "default": "linear",
                        "description": "Temporal decay function: linear, exponential, step, none, no_decay"
                    },
                    "temporalScale": {
                        "type": "string",
                        "enum": ["micro", "meso", "macro", "long", "archival"],
                        "default": "meso",
                        "description": "Temporal scale for decay: micro, meso, macro, long, archival"
                    }
                },
                "required": ["query"]
            }),
        ),
        // trigger_consolidation - trigger memory consolidation (PRD Section 10.1)
        ToolDefinition::new(
            "trigger_consolidation",
            "Trigger memory consolidation to merge similar memories and reduce redundancy. \
             Uses similarity-based, temporal, or semantic strategies to identify merge candidates. \
             Helps optimize memory storage and improve retrieval efficiency.",
            json!({
                "type": "object",
                "properties": {
                    "strategy": {
                        "type": "string",
                        "enum": ["similarity", "temporal", "semantic"],
                        "default": "similarity",
                        "description": "Consolidation strategy to use"
                    },
                    "min_similarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.85,
                        "description": "Minimum similarity threshold for consolidation candidates"
                    },
                    "max_memories": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10000,
                        "default": 100,
                        "description": "Maximum memories to process in one batch"
                    }
                },
                "required": []
            }),
        ),
    ]
}
